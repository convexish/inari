//! Self/super-execution is an effect keyword, not a nested Dogma action.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    ChainDraw,
    ChainClaim,
    Effects,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CopiedExecution {
    card: HeldCard,
    mode: ExecutionMode,
    /// The next schedule position (a few echoes and dogmas: never near
    /// `u16::MAX`), narrow to keep the common frame budget.
    next_effect: u16,
    context: Context,
    stage: Stage,
    last: Option<HeldOutcome>,
    demand_transfers: u32,
    demand_returns: u32,
    /// Packed flags, keeping the frame small: [`Self::CHANGED`] (something
    /// changed inside this execution, nested executions and reactions
    /// included: The Big Bang's "if this causes any change"),
    /// [`Self::MOVED`] (something other than a reveal changed),
    /// [`Self::REPORT`] (started by an `Execute` instruction, whose program
    /// reads the activity), [`Self::IGNORE_REVEALS`] (a reveal alone is
    /// not activity for that program) and [`Self::STARTED`] (a narrated game
    /// announced it).
    flags: u8,
    /// Echoes frozen with the card's dogmas when any would run, and the
    /// execution slots.
    extra: Option<Box<ExecutionExtra>>,
}

impl CopiedExecution {
    const CHANGED: u8 = 1;
    const MOVED: u8 = 2;
    const REPORT: u8 = 4;
    const IGNORE_REVEALS: u8 = 8;
    const STARTED: u8 = 16;

    pub(super) fn note_change(&mut self, reveal: bool) {
        self.flags |= Self::CHANGED;
        if !reveal {
            self.flags |= Self::MOVED;
        }
    }

    /// Report the activity to the `Execute` instruction that started this.
    pub(super) fn report(&mut self, count_reveals: bool) {
        self.flags |= Self::REPORT;
        if !count_reveals {
            self.flags |= Self::IGNORE_REVEALS;
        }
    }

    /// What the execution reports: a performed operation when anything
    /// changed, otherwise nothing ("if this causes any change").
    fn outcome(&self) -> ReturnValue {
        let active = if self.flags & Self::IGNORE_REVEALS == 0 {
            self.flags & Self::CHANGED != 0
        } else {
            self.flags & Self::MOVED != 0
        };
        if self.flags & Self::REPORT != 0 && active {
            // One performed operation and no card reference, so "if this
            // causes any change" reads as a performed result.
            ReturnValue::Bulk(BulkOutcome {
                performed: 1,
                replaced: 0,
                inapplicable: 0,
                distinct_values: 0,
                card: None,
                value: None,
            })
        } else {
            ReturnValue::Unit
        }
    }

    pub(super) fn new(
        card: HeldCard,
        mode: ExecutionMode,
        mut context: Context,
        chain: bool,
        extra: Option<Box<ExecutionExtra>>,
    ) -> Self {
        context.demander = context.executor;
        Self {
            card,
            mode,
            next_effect: 0,
            context,
            stage: if chain {
                Stage::ChainDraw
            } else {
                Stage::Effects
            },
            last: None,
            demand_transfers: 0,
            demand_returns: 0,
            flags: 0,
            extra,
        }
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            card,
            mode: _,
            next_effect: _,
            context,
            stage: _,
            last,
            demand_transfers: _,
            demand_returns: _,
            flags: _,
            extra,
        } = self;
        // The executing card selects the copied programs: it keeps its identity.
        map.source(card.inner, card.evidence);
        map.erase(&mut card.evidence);
        map.context(context);
        map.outcome_option(last);
        // Board echoes are public; the executed card keeps its identity.
        if let Some(extra) = extra {
            extra.memory.remap(map);
        }
    }

    /// Execution slots, if any were recorded.
    pub(super) fn memory(&self) -> Option<&slots::Memory> {
        self.extra.as_ref().map(|extra| &extra.memory)
    }

    pub(super) fn memory_mut(&mut self) -> &mut slots::Memory {
        &mut self.extra.get_or_insert_with(Box::default).memory
    }

    /// The card whose effects run (public once executed).
    pub(super) fn card(&self) -> HeldCard {
        self.card
    }

    /// The controller this execution imposes on every decision inside it.
    pub(super) fn override_controller(&self) -> Option<Player> {
        self.context.controlled().then_some(self.context.controller)
    }

    /// The viewer this execution lets look at that owner's cards, if any.
    pub(super) fn inspection(&self) -> Option<(Player, Player)> {
        (self.context.inspect() && self.context.controller != self.context.executor)
            .then_some((self.context.controller, self.context.executor))
    }

    pub(super) fn receive(&mut self, value: HeldReturn) {
        self.last = value.operation();
    }

    /// The effect at a schedule position: (source card, effect index).
    fn entry(&self, position: u16) -> Option<(CardId, u16)> {
        match ExecutionExtra::scheduled(&self.extra) {
            Some(schedule) => schedule.get(usize::from(position)).copied(),
            None => Some((self.card.inner, position)),
        }
    }

    /// Whether this execution is running that effect now.
    pub(super) fn owns_invocation(&self, card: CardId, effect: u16) -> bool {
        self.stage == Stage::Effects
            && self
                .next_effect
                .checked_sub(1)
                .and_then(|position| self.entry(position))
                == Some((card, effect))
    }

    pub(super) fn note_move(
        &mut self,
        invocation: Option<(CardId, u16)>,
        context: Context,
        verb: Verb,
    ) -> Result<(), Error> {
        if invocation.is_some_and(|(card, effect)| self.owns_invocation(card, effect))
            && context.demander == self.context.executor
        {
            let count = match verb {
                Verb::Transfer => &mut self.demand_transfers,
                Verb::Return => &mut self.demand_returns,
                _ => return Ok(()),
            };
            *count = count
                .checked_add(1)
                .ok_or(Error::Unsupported("demand movement count overflow"))?;
        }
        Ok(())
    }

    pub(super) fn prior_moves(&self, context: Context, verb: Verb) -> u32 {
        if context.source_card == Some(self.card.inner) && context.demander == self.context.executor
        {
            match verb {
                Verb::Transfer => self.demand_transfers,
                Verb::Return => self.demand_returns,
                _ => 0,
            }
        } else {
            0
        }
    }
}

impl Game {
    /// The execution's events, for the log: its start, once, and its end.
    fn announce_copied(&self, execution: &mut CopiedExecution, events: &mut Vec<Event>) {
        if self.narrated && execution.flags & CopiedExecution::STARTED == 0 {
            execution.flags |= CopiedExecution::STARTED;
            events.push(Event::ExecutionStarted {
                executor: execution.context.executor,
                card: execution.card.inner,
                mode: execution.mode,
                source: execution.context.source_card,
                inside_karma: execution.context.suppress_karma(),
            });
        }
    }

    fn finish_copied(&mut self, execution: &CopiedExecution, events: &mut Vec<Event>) {
        if self.narrated {
            events.push(Event::ExecutionEnded {
                executor: execution.context.executor,
                card: execution.card.inner,
            });
        }
        self.complete(execution.outcome());
    }

    pub(super) fn resume_copied(
        &mut self,
        mut execution: CopiedExecution,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        self.announce_copied(&mut execution, events);
        match execution.stage {
            Stage::ChainDraw => {
                execution.stage = Stage::ChainClaim;
                if self.narrated {
                    events.push(Event::ChainAwarded {
                        executor: execution.context.executor,
                        card: execution.card.inner,
                    });
                }
                let context = execution.context.rules_for(execution.context.executor);
                self.frames.push(Frame::Execute(execution));
                // The Chain award is an ordinary age 11, not a
                // printed card-effect box modified by Archimedes or Kepler.
                self.frames.push(Frame::operation(
                    Operation::Draw {
                        value: 11,
                        set: None,
                    },
                    context,
                ));
            }
            Stage::ChainClaim => {
                let context = execution.context.rules_for(execution.context.executor);
                let target = execution
                    .last
                    .filter(|result| result.disposition == Disposition::Performed)
                    .and_then(|result| result.card)
                    .and_then(|card| {
                        self.position
                            .targets(LogicalZone::Hand(context.executor))
                            .find(|target| target.card == card)
                    });
                execution.stage = Stage::Effects;
                self.frames.push(Frame::Execute(execution));
                if let Some(target) = target {
                    self.frames.push(Frame::operation(
                        Operation::Claim {
                            target,
                            eligibility: ClaimEligibility::Ignore,
                        },
                        context,
                    ));
                }
            }
            Stage::Effects => {
                if self.position.card(execution.card.inner).figure.is_some() {
                    self.finish_copied(&execution, events);
                    return Ok(());
                }
                if let ExecutionMode::Effect(wanted) = execution.mode {
                    // The wanted non-demand dogma effect, once; none: nothing.
                    let card = execution.card.inner;
                    let index = self
                        .position
                        .card(card)
                        .effects
                        .iter()
                        .enumerate()
                        .filter(|(_, effect)| effect.kind == EffectKind::NonDemand)
                        .nth(usize::from(wanted))
                        .map(|(index, _)| index as u16);
                    let Some(index) = index.filter(|_| execution.next_effect == 0) else {
                        self.finish_copied(&execution, events);
                        return Ok(());
                    };
                    // The schedule is the card's own effects, so the owned
                    // invocation is the selected index (Chain for a keyword
                    // inside it), not the first non-demand ordinal.
                    execution.next_effect = index + 1;
                    let mut context = execution.context;
                    context.source_card = Some(card);
                    context.source_evidence = execution.card.evidence;
                    let program = Frame::effect(card, index, context);
                    self.frames.push(Frame::Execute(execution));
                    self.frames.push(program);
                    return Ok(());
                }
                // Self-execution skips demands; super-execution runs them.
                // Neither runs a compel (Artifacts).
                let next = (execution.next_effect..u16::MAX).find_map(|position| {
                    let Some((source, index)) = execution.entry(position) else {
                        return Some(None);
                    };
                    let effect = self.position.card(source).effect(index);
                    let Some(effect) = effect else {
                        return Some(None);
                    };
                    (match effect.kind {
                        EffectKind::NonDemand => true,
                        EffectKind::Demand => execution.mode == ExecutionMode::SuperExecute,
                        EffectKind::Compel => false,
                    })
                    .then_some(Some((
                        position,
                        source,
                        index,
                        effect.demand(),
                    )))
                });
                let Some(Some((position, source, index, demand))) = next else {
                    self.finish_copied(&execution, events);
                    return Ok(());
                };
                execution.next_effect = position + 1;
                if demand {
                    execution.demand_transfers = 0;
                    execution.demand_returns = 0;
                }
                let mut context = execution.context;
                context.source_card = Some(source);
                // A buried echo's card shows on its board.
                context.source_evidence = if source == execution.card.inner {
                    execution.card.evidence
                } else {
                    Evidence::PUBLIC
                };
                if demand {
                    context.executor = execution.context.executor.opponent();
                    // The victim decides, unless a controller override is
                    // in force (Hitchhiking).
                    if !context.controlled() {
                        context.controller = context.executor;
                    }
                }
                let program = Frame::effect(source, index, context);
                self.frames.push(Frame::Execute(execution));
                self.frames.push(program);
            }
        }
        Ok(())
    }

    /// The nearest card-effect invocation owns Chain eligibility, including
    /// keywords in its intervening karma. A copied demand does not inherit
    /// eligibility from a suspended outer non-demand effect.
    fn chain_applies(&self) -> bool {
        let mut invocation = None;
        for frame in self.frames.iter().rev() {
            match frame {
                // Karma is transparent: its keywords belong to the nearest
                // executing card effect.
                Frame::Program {
                    source: Source::Effect { card, effect },
                    ..
                } => {
                    if invocation.is_some()
                        || self
                            .position
                            .card(*card)
                            .effect(*effect)
                            .is_some_and(|e| e.demand())
                    {
                        return false;
                    }
                    invocation = Some((*card, *effect));
                }
                Frame::Execute(execution) => {
                    return invocation
                        .is_some_and(|(card, effect)| execution.owns_invocation(card, effect));
                }
                Frame::Dogma(_) | Frame::ActionTail { .. } => return false,
                _ => {}
            }
        }
        false
    }

    pub(super) fn push_copied(&mut self, card: CardId, mode: ExecutionMode, context: Context) {
        let reference = self.capture(card);
        self.push_copied_reference(reference, mode, context);
    }

    pub(super) fn push_copied_reference(
        &mut self,
        card: HeldCard,
        mode: ExecutionMode,
        context: Context,
    ) {
        // Executing a card reads its text to everyone, wherever it lies. This
        // is knowledge, not a Reveal operation: nothing moves or triggers.
        self.disclose(card.inner);
        let card = Captured::new(card.inner, Evidence::PUBLIC);
        // One copied effect is not a self-execution: no Chain, no echoes.
        let single = matches!(mode, ExecutionMode::Effect(_));
        // A card without dogma effects (a Figure, a City, Battleship Yamato) executes
        // nothing, not even the echo effects visible in its pile, and earns no Chain.
        let printed = self.position.card(card.inner);
        let effectless = printed.figure.is_some() || printed.effects.is_empty();
        let chain = !single && !effectless && self.chain_applies();
        let extra = if single || effectless {
            None
        } else {
            self.echo_schedule(card.inner)
        };
        let execution = CopiedExecution::new(card, mode, context, chain, extra);
        self.inspecting |= execution.inspection().is_some();
        self.frames.push(Frame::Execute(execution));
    }
}

impl CopiedExecution {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) -> usize {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.reference(
            K::Execution,
            parent,
            self.card,
            &[
                (F::Mode, self.mode.code()),
                (F::Stage, self.stage as u64),
                (F::Performed, u64::from(self.demand_transfers)),
                (F::Count, u64::from(self.demand_returns)),
            ],
        );
        p.context(id, self.context);
        p.schedule(
            id,
            ExecutionExtra::scheduled(&self.extra),
            usize::from(self.next_effect),
        );
        if let Some(memory) = self.memory() {
            memory.project(p, id, Life::Execution);
        }
        if let Some(last) = self.last {
            p.outcome(id, last.with(ReturnValue::Operation(last.inner)));
        }
        id
    }
}
