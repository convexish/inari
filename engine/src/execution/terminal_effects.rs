use super::*;

/// Only the four printed late-game effects allocate this progress record.
/// Original operation results live here, outside reaction children and Program's
/// last-result register. In particular, repeating Fusion never resets its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TerminalEffect {
    pub(super) native: Native,
    pub(super) context: Context,
    pub(super) last: HeldReturn,
    stage: u8,
    value: u8,
    color: Color,
    card: Option<HeldCard>,
    returned: u8,
    sum: u16,
    visited: Vec<HeldCard>,
}

impl TerminalEffect {
    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            native: _,
            context,
            last,
            stage: _,
            value: _,
            color: _,
            card,
            returned: _,
            sum: _,
            visited,
        } = self;
        map.context(context);
        map.returned(last);
        map.held_optional(card);
        for card in visited {
            map.held(card);
        }
    }

    pub(super) fn new(native: Native, context: Context, boxed: BoxedValues) -> Result<Self, Error> {
        Ok(Self {
            native,
            context,
            last: HeldReturn::UNIT,
            stage: 0,
            value: if native == Native::SpaceTraffic {
                boxed.value(11)?
            } else {
                11
            },
            color: Color::Red,
            card: None,
            returned: 0,
            sum: 0,
            visited: Vec::new(),
        })
    }

    fn original(&self) -> Option<HeldOutcome> {
        self.last
            .operation()
            .filter(|result| result.disposition == Disposition::Performed)
    }
}

impl Game {
    pub(super) fn choose_fusion_value(&mut self, value: u8) {
        let Some(Frame::TerminalEffect(progress)) = self.frames.last_mut() else {
            unreachable!("Fusion value caller");
        };
        progress.value = value;
        progress.stage = 0;
        progress.last = HeldReturn::UNIT;
    }

    fn continue_terminal(&mut self, progress: Box<TerminalEffect>, request: Operation) {
        let context = progress.context;
        self.frames.push(Frame::TerminalEffect(progress));
        self.frames.push(Frame::operation(request, context));
    }

    /// Physical ownership, never aliases such as an Each-granted logical hand
    /// or achievement: hand, score, Forecast, Safe, display and stored
    /// Artifacts, then the board. BoardDepth is the explicit permission to
    /// select buried cards; ordinary Physical board targets remain top-only.
    pub(super) fn owned_targets(&self, player: Player, achievements: bool) -> Vec<Target> {
        let mut targets = Vec::new();
        for physical in [
            Zone::Hand(player),
            Zone::Score(player),
            Zone::Forecast(player),
            Zone::Safe(player),
            Zone::Display(player),
            Zone::Museum(player),
            Zone::Achievements(player),
        ] {
            if !achievements && physical == Zone::Achievements(player) {
                continue;
            }
            targets.extend(
                self.position
                    .cards(physical)
                    .iter()
                    .copied()
                    .map(|card| Target {
                        card,
                        physical,
                        logical: LogicalZone::Physical(physical),
                    }),
            );
        }
        for color in Color::ALL {
            let physical = Zone::Board(player, color);
            targets.extend(
                self.position
                    .cards(physical)
                    .iter()
                    .rev()
                    .copied()
                    .map(|card| Target {
                        card,
                        physical,
                        logical: LogicalZone::BoardDepth(player, color),
                    }),
            );
        }
        targets
    }

    /// Removal can expose a buried Figure. A top-only guard is insufficient.
    /// With no Figure anywhere on either board, these physical junk/score
    /// sweeps cannot create one, invoke a hook, alter another candidate, consume
    /// RNG or end the game. Only then is one canonical order sufficient.
    pub(super) fn terminal_sweep_clear(&self) -> bool {
        #[cfg(test)]
        if self.shortcuts_disabled {
            return false;
        }
        !self.position.figures()
            || Player::ALL.into_iter().all(|player| {
                Color::ALL.into_iter().all(|color| {
                    self.position
                        .cards(Zone::Board(player, color))
                        .iter()
                        .all(|&card| self.position.card(card).figure.is_none())
                })
            })
    }

    pub(super) fn finish_dogma_action(&mut self) {
        // Keep the real action tail, including fading/specials/victory and the
        // regular-action counter. Discard every copied child and participant.
        // A Meld-action karma can execute a card that super-executes Fission:
        // no Dogma action exists then, but its enclosing action must still end.
        let boundary = self
            .frames
            .iter()
            .rposition(|frame| {
                matches!(
                    frame,
                    Frame::ActionTail {
                        action: Action::Dogma { .. },
                        ..
                    }
                )
            })
            .or_else(|| {
                self.frames
                    .iter()
                    .rposition(|frame| matches!(frame, Frame::ActionTail { .. }))
            });
        if let Some(boundary) = boundary {
            let shared = self.frames[boundary + 1..]
                .iter()
                .any(|frame| matches!(frame, Frame::Dogma(dogma) if dogma.shared));
            self.frames.truncate(boundary + 1);
            self.complete(ReturnValue::Dogma { shared });
        } else {
            // Low-level effect/Dogma entry points deliberately have no action
            // tail. They still must not resume a surrounding copied program.
            self.frames.clear();
            self.completed = HeldReturn::UNIT;
        }
    }

    pub(super) fn eliminate(
        &mut self,
        player: Player,
        context: Context,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        // Elimination is not an opponent's proposed victory. The eliminated
        // civilization no longer supplies karma; rule cleanup is unconditional.
        // Commit ordinary factual movements, rather than silently clearing piles.
        for target in self.owned_targets(player, true) {
            self.commit(
                Operation::Move {
                    verb: Verb::Junk,
                    target,
                    destination: Zone::Junk,
                },
                context,
                false,
                events,
            )?;
        }
        for special in self.position.specials() {
            if self.position.special_location(special) == SpecialLocation::Owned(player) {
                self.move_special(special, SpecialLocation::Junk, context, events);
            }
        }
        // Claimed Museums are achievements: junked with the civilization,
        // never back into the supply.
        self.position.junk_claimed_museums(player);
        for decree in Decree::ALL {
            if self.position.decree_location(decree) == DecreeLocation::Owned(player) {
                let source = DecreeLocation::Owned(player);
                self.position
                    .decrees
                    .as_mut()
                    .expect("owned decree allocation")[decree as usize] = DecreeLocation::Junk;
                events.push(Event::DecreeMoved {
                    decree,
                    source,
                    destination: DecreeLocation::Junk,
                    executor: context.executor,
                    inside_karma: context.suppress_karma(),
                });
            }
        }
        self.terminal = Some(Terminal::Winner(
            player.opponent(),
            Victory::new(WinReason::Elimination { loser: player }, context),
        ));
        self.frames.clear();
        Ok(())
    }

    pub(super) fn resume_terminal_effect(
        &mut self,
        mut progress: Box<TerminalEffect>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let context = progress.context;
        let player = context.executor;
        match progress.native {
            Native::MaxVerstappen => unreachable!("Max Verstappen runs inside his karma program"),
            Native::Fusion => {
                if progress.stage == 0 {
                    let operations = self
                        .select(
                            Selector::Tops(Who::Executor),
                            context,
                            Regs::EMPTY,
                            Excluded::default(),
                        )?
                        .into_iter()
                        .filter(|target| self.position.value(target.card) == progress.value)
                        .map(|target| {
                            self.movement(
                                Verb::Score,
                                target,
                                Destination::Score(Who::Executor),
                                context,
                            )
                        })
                        .collect();
                    progress.stage = 1;
                    self.frames.push(Frame::TerminalEffect(progress));
                    self.choose_operations(context, operations, false);
                } else if let Some(value) = progress.original().and_then(|result| result.value) {
                    let minimum = u8::from(!self.position.age_zero_introduced());
                    let values: Vec<_> = [1, 2]
                        .into_iter()
                        .filter_map(|decrease| value.checked_sub(decrease))
                        .filter(|&value| value >= minimum)
                        .collect();
                    if values.is_empty() {
                        self.complete(ReturnValue::Unit);
                    } else {
                        let options = values.iter().copied().map(Alternative::Value).collect();
                        let tasks = values.into_iter().map(Task::FusionValue).collect();
                        self.frames.push(Frame::TerminalEffect(progress));
                        self.frames.push(Frame::Choose {
                            player: context.controller,
                            options: DecisionOptions::new(options, context),
                            tasks,
                        });
                    }
                } else {
                    self.complete(ReturnValue::Unit);
                }
            }
            Native::Reclamation => {
                if progress.stage == 1 {
                    if let Some(value) = progress.original().and_then(|result| result.value) {
                        progress.returned += 1;
                        progress.sum += u16::from(value);
                    }
                    progress.stage = 0;
                }
                if progress.stage == 0 {
                    // The three bottom red cards, each considered once. The
                    // player orders returns that share a deck (deck bottoms);
                    // different decks commute while no Figure is on a board.
                    let physical = Zone::Board(player, progress.color);
                    let mut targets: Vec<_> = self
                        .position
                        .cards(physical)
                        .iter()
                        .copied()
                        .filter(|card| !contains_card(&progress.visited, *card))
                        .take(3 - progress.visited.len().min(3))
                        .map(|card| Target {
                            card,
                            physical,
                            logical: LogicalZone::BoardDepth(player, progress.color),
                        })
                        .collect();
                    if self.terminal_sweep_clear()
                        && let Some(group) = targets.iter().map(|&t| self.origin_group(t)).min()
                    {
                        targets.retain(|&target| self.origin_group(target) == group);
                    }
                    if targets.len() > 1 {
                        let (options, tasks) = targets
                            .into_iter()
                            .map(|target| {
                                (
                                    Alternative::Move {
                                        verb: Verb::Return,
                                        target: self
                                            .position
                                            .descriptor(target, context.controller),
                                    },
                                    Task::TerminalPick(target),
                                )
                            })
                            .unzip();
                        self.frames.push(Frame::TerminalEffect(progress));
                        self.frames.push(Frame::Choose {
                            player: context.controller,
                            options: DecisionOptions::new(options, context),
                            tasks,
                        });
                    } else if let Some(&target) = targets.first() {
                        let reference = self.capture(target.card);
                        progress.visited.push(reference);
                        progress.stage = 1;
                        let request =
                            self.movement(Verb::Return, target, Destination::Deck, context);
                        self.frames.push(Frame::TerminalEffect(progress));
                        self.frames
                            .push(Frame::referenced_operation(request, context, reference));
                    } else {
                        let value = progress.sum.div_ceil(2) as u8; // at most ceil(3*11/2) = 17
                        progress.stage = 2;
                        self.continue_terminal(progress, Operation::Draw { value, set: None });
                    }
                } else if progress.stage == 2 {
                    let card = progress.original().and_then(|result| result.card);
                    let target = card.and_then(|card| {
                        self.selected_target(card, LogicalZone::Hand(context.executor))
                    });
                    if let Some(target) = target {
                        progress.card = progress.original().and_then(HeldOutcome::card_ref);
                        progress.color = self.position.card(target.card).color;
                        progress.stage = 3;
                        let request = self.movement(
                            Verb::Meld,
                            target,
                            Destination::Board(Who::Executor),
                            context,
                        );
                        self.continue_terminal(progress, request);
                    } else {
                        self.complete(ReturnValue::Unit);
                    }
                } else if progress.returned == 3
                    && progress.original().and_then(|result| result.card)
                        == progress.card.map(|card| card.inner)
                {
                    progress.stage = 0;
                    progress.returned = 0;
                    progress.sum = 0;
                    progress.visited.clear();
                    progress.last = HeldReturn::UNIT;
                    self.frames.push(Frame::TerminalEffect(progress));
                } else {
                    self.complete(ReturnValue::Unit);
                }
            }
            Native::SpaceTraffic => {
                match progress.stage {
                    0 => {
                        progress.stage = 1;
                        let value = progress.value;
                        self.continue_terminal(progress, Operation::Draw { value, set: None });
                    }
                    1 => {
                        let target =
                            progress
                                .original()
                                .and_then(|result| result.card)
                                .and_then(|card| {
                                    self.selected_target(card, LogicalZone::Hand(context.executor))
                                });
                        if let Some(target) = target {
                            progress.card = progress.original().and_then(HeldOutcome::card_ref);
                            progress.color = self.position.card(target.card).color;
                            progress.stage = 2;
                            let request = self.movement(
                                Verb::Tuck,
                                target,
                                Destination::Board(Who::Executor),
                                context,
                            );
                            self.continue_terminal(progress, request);
                        } else {
                            self.complete(ReturnValue::Unit);
                        }
                    }
                    2 => {
                        if progress.original().and_then(|result| result.card)
                            != progress.card.map(|card| card.inner)
                        {
                            self.complete(ReturnValue::Unit);
                            return Ok(());
                        }
                        // All current tuck hooks happen before the original
                        // commit. Its immediate neighbor is still factual here.
                        let stack = self.position.cards(Zone::Board(player, progress.color));
                        let under_eleven = progress
                            .card
                            .and_then(|card| stack.iter().position(|&id| id == card.inner))
                            .and_then(|index| stack.get(index + 1))
                            .is_some_and(|&card| self.position.value(card) == progress.value);
                        if under_eleven {
                            self.eliminate(player, context, events)?;
                        } else {
                            progress.stage = 3;
                            self.frames.push(Frame::TerminalEffect(progress));
                        }
                    }
                    3 => {
                        let physical = Zone::Board(player, progress.color);
                        let stack = self.position.cards(physical);
                        let mut targets: Vec<_> = stack[..stack.len().saturating_sub(5)]
                            .iter()
                            .copied()
                            .filter(|card| !contains_card(&progress.visited, *card))
                            .map(|card| Target {
                                card,
                                physical,
                                logical: LogicalZone::BoardDepth(player, progress.color),
                            })
                            .collect();
                        if targets.is_empty() {
                            progress.stage = 4;
                            let color = progress.color;
                            self.continue_terminal(
                                progress,
                                Operation::Splay {
                                    player,
                                    color,
                                    direction: Splay::Aslant,
                                },
                            );
                        } else {
                            if self.terminal_sweep_clear() {
                                targets.truncate(1);
                            }
                            // Reuse the visited-card task; dispatch differs by
                            // this exact native phase, not by card menu index.
                            let (options, tasks) = targets
                                .into_iter()
                                .map(|target| {
                                    (
                                        Alternative::Move {
                                            verb: Verb::Score,
                                            target: self
                                                .position
                                                .descriptor(target, context.controller),
                                        },
                                        Task::TerminalPick(target),
                                    )
                                })
                                .unzip();
                            self.frames.push(Frame::TerminalEffect(progress));
                            self.frames.push(Frame::Choose {
                                player: context.controller,
                                options: DecisionOptions::new(options, context),
                                tasks,
                            });
                        }
                    }
                    4 => {
                        if self.position.score(player) >= self.position.score(player.opponent()) {
                            self.complete(ReturnValue::Unit);
                        } else {
                            progress.stage = 0;
                            progress.card = None;
                            progress.visited.clear();
                            progress.last = HeldReturn::UNIT;
                            self.frames.push(Frame::TerminalEffect(progress));
                        }
                    }
                    _ => unreachable!("Space Traffic phase"),
                }
            }
        }
        Ok(())
    }

    /// A picked card of a native phase: Space Traffic scores it, Reclamation
    /// returns it.
    pub(super) fn choose_terminal_pick(&mut self, target: Target) {
        let reference = self.capture(target.card);
        let Some(Frame::TerminalEffect(progress)) = self.frames.last_mut() else {
            unreachable!("native pick caller");
        };
        progress.visited.push(reference);
        let context = progress.context;
        let (verb, to) = if progress.native == Native::Reclamation {
            progress.stage = 1;
            (Verb::Return, Destination::Deck)
        } else {
            (Verb::Score, Destination::Score(Who::Executor))
        };
        let request = self.movement(verb, target, to, context);
        self.frames
            .push(Frame::referenced_operation(request, context, reference));
    }
}
