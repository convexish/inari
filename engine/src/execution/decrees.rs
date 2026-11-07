//! Decree effects, independently resumable from their action's hand payment.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BlindReturn {
    pub targets: Vec<Target>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Begin,
    Draw { value: u8, remaining: u8 },
    Score { remaining: u8 },
    Tops { value: Option<u8>, colors: u8 },
    Claim,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DecreeProgress {
    decree: Decree,
    context: Context,
    claim: bool,
    stage: Stage,
    visited: Vec<HeldCard>,
}

impl DecreeProgress {
    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            decree: _,
            context,
            claim: _,
            stage: _,
            visited,
        } = self;
        map.context(context);
        for card in visited {
            map.held(card);
        }
    }

    pub fn return_only(decree: Decree, context: Context) -> Self {
        Self {
            stage: Stage::Done,
            ..Self::new(decree, context, false)
        }
    }

    pub fn new(decree: Decree, context: Context, claim: bool) -> Self {
        Self {
            decree,
            context,
            claim,
            stage: Stage::Begin,
            visited: Vec::new(),
        }
    }
}

impl Game {
    pub(super) fn select_decree_value(&mut self, value: u8) {
        let Some(Frame::Decree(progress)) = self.frames.last_mut() else {
            unreachable!("War caller");
        };
        progress.stage = Stage::Tops {
            value: Some(value),
            colors: 0,
        };
    }

    pub(super) fn return_decree_card(&mut self, target: Target) {
        let reference = self.capture(target.card);
        let color = self.position.card(target.card).color;
        let Some(Frame::Decree(progress)) = self.frames.last_mut() else {
            unreachable!("Decree return caller");
        };
        match &mut progress.stage {
            Stage::Score { remaining } => *remaining -= 1,
            Stage::Tops { colors, .. } => *colors |= 1 << color as u8,
            _ => unreachable!("Decree return stage"),
        }
        progress.visited.push(reference);
        let context = progress.context;
        let operation = self.movement(Verb::Return, target, Destination::Deck, context);
        self.frames
            .push(Frame::referenced_operation(operation, context, reference));
    }

    fn decree_return_choices(&mut self, context: Context, targets: Vec<Target>) {
        let mut tasks: Vec<Task> = Vec::new();
        let mut options = Vec::new();
        for target in targets {
            match self.position.descriptor(target, context.controller) {
                known @ TargetDescriptor::Known { .. } => {
                    options.push(Alternative::Move {
                        verb: Verb::Return,
                        target: known,
                    });
                    tasks.push(Task::DecreeReturn(target));
                }
                TargetDescriptor::Hidden { source, back, .. } => {
                    let option = Alternative::BlindMove {
                        verb: Verb::Return,
                        source,
                        back,
                    };
                    if let Some(index) = options.iter().position(|o| *o == option) {
                        let Task::DecreeBlindReturn(group) = &mut tasks[index] else {
                            unreachable!("blind return group");
                        };
                        group.targets.push(target);
                    } else {
                        options.push(option);
                        tasks.push(Task::DecreeBlindReturn(Box::new(BlindReturn {
                            targets: vec![target],
                        })));
                    }
                }
            }
        }
        // Canonical public-back order, never order by a hidden front. Known
        // board choices retain ordinary color order. Sorting only the blind
        // entries also keeps swapped hidden worlds' menus identical.
        let mut choices: Vec<_> = options.into_iter().zip(tasks).collect();
        choices.sort_by_key(|(option, _)| match option {
            Alternative::BlindMove { source, back, .. } => {
                let origin = match source {
                    Zone::Score(_) => 0,
                    Zone::Hand(_) => 1,
                    _ => 2,
                };
                (0, origin, back.age, back.set)
            }
            _ => (1, 0, 0, CardSet::Base),
        });
        let (options, tasks) = choices.into_iter().unzip();
        self.frames.push(Frame::Choose {
            player: context.controller,
            options: DecisionOptions::new(options, context),
            tasks,
        });
    }

    pub(super) fn resume_decree(&mut self, mut progress: Box<DecreeProgress>) -> Result<(), Error> {
        let context = progress.context;
        match progress.stage {
            Stage::Begin => {
                match progress.decree {
                    Decree::Expansion => {
                        progress.stage = Stage::Claim;
                        self.frames.push(Frame::Decree(progress));
                        let operations = Color::ALL
                            .into_iter()
                            .filter(|&color| self.can_splay(context.executor, color, Splay::Up))
                            .map(|color| Operation::Splay {
                                player: context.executor,
                                color,
                                direction: Splay::Up,
                            })
                            .collect();
                        self.choose_operations(context, operations, false);
                        return Ok(());
                    }
                    Decree::Trade | Decree::Advancement => {
                        // Trade: three cards one higher than the highest top;
                        // Advancement: one card two higher (printed cards).
                        let (increase, remaining) = if progress.decree == Decree::Trade {
                            (1, 3)
                        } else {
                            (2, 1)
                        };
                        progress.stage = Stage::Draw {
                            value: self.position.highest_top_value(context.executor) + increase,
                            remaining,
                        };
                    }
                    Decree::War => {
                        progress.stage = Stage::Tops {
                            value: None,
                            colors: 0,
                        };
                        self.frames.push(Frame::Decree(progress));
                        let minimum = u8::from(!self.position.age_zero_introduced());
                        self.frames.push(Frame::Choose {
                            player: context.controller,
                            options: DecisionOptions::new(
                                (minimum..=11).map(Alternative::Value).collect(),
                                context,
                            ),
                            tasks: (minimum..=11).map(Task::DecreeValue).collect(),
                        });
                        return Ok(());
                    }
                    Decree::Rivalry => progress.stage = Stage::Score { remaining: 3 },
                }
                self.frames.push(Frame::Decree(progress));
            }
            Stage::Draw { value, remaining } => {
                if remaining == 0 {
                    progress.stage = Stage::Claim;
                    self.frames.push(Frame::Decree(progress));
                } else {
                    progress.stage = Stage::Draw {
                        value,
                        remaining: remaining - 1,
                    };
                    self.frames.push(Frame::Decree(progress));
                    self.frames.push(Frame::operation(
                        Operation::Draw { value, set: None },
                        context,
                    ));
                }
            }
            Stage::Score { remaining } => {
                let targets = self
                    .position
                    .targets(LogicalZone::Score(context.executor.opponent()))
                    .filter(|t| !contains_card(&progress.visited, t.card))
                    .collect::<Vec<_>>();
                if remaining == 0 || targets.is_empty() {
                    progress.stage = Stage::Tops {
                        value: None,
                        colors: 0,
                    };
                    self.frames.push(Frame::Decree(progress));
                } else {
                    self.frames.push(Frame::Decree(progress));
                    self.decree_return_choices(context, targets);
                }
            }
            Stage::Tops { value, colors } => {
                let targets = Color::ALL
                    .into_iter()
                    .filter(|&color| colors & (1 << color as u8) == 0)
                    .filter_map(|color| {
                        let physical = Zone::Board(context.executor.opponent(), color);
                        let card = *self.position.cards(physical).last()?;
                        let definition = self.position.card(card);
                        let matches = value.map_or(definition.figure.is_some(), |value| {
                            definition.value == value
                        });
                        (matches && !contains_card(&progress.visited, card)).then_some(Target {
                            card,
                            physical,
                            logical: LogicalZone::Physical(physical),
                        })
                    })
                    .collect::<Vec<_>>();
                if targets.is_empty() {
                    progress.stage = Stage::Claim;
                    self.frames.push(Frame::Decree(progress));
                } else {
                    self.frames.push(Frame::Decree(progress));
                    self.decree_return_choices(context, targets);
                }
            }
            Stage::Claim => {
                let decree = progress.decree;
                let claim = progress.claim;
                progress.stage = Stage::Done;
                self.frames.push(Frame::Decree(progress));
                if claim {
                    self.frames
                        .push(Frame::operation(Operation::ClaimDecree { decree }, context));
                }
            }
            Stage::Done => self.complete(ReturnValue::Unit),
        }
        Ok(())
    }
}
