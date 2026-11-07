//! Verification reveals and mixed standard/special achievement quotas.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConditionalVerification {
    pub(super) task: Task,
    pub(super) condition: &'static Cond,
    pub(super) regs: Regs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct JunkAvailable {
    remaining: u16,
    context: Context,
    visited: Vec<HeldCard>,
    specials: u32,
    shortcut: Option<Shortcut>,
    pub(super) outcome: HeldBulk,
}

impl JunkAvailable {
    pub(super) fn new(remaining: u16, shortcut: Option<Shortcut>, context: Context) -> Self {
        Self {
            remaining,
            context,
            visited: Vec::new(),
            specials: 0,
            shortcut,
            outcome: HeldBulk::default(),
        }
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        map.context(&mut self.context);
        for card in &mut self.visited {
            map.held(card);
        }
        map.bulk(&mut self.outcome);
    }
}

impl Game {
    pub(super) fn verify_pending_move_for(
        &mut self,
        verify: bool,
        when: Option<&'static Cond>,
        regs: Regs,
    ) -> Result<(), Error> {
        if !verify {
            return Ok(());
        }
        match when {
            None => {
                self.verify_pending_move();
                Ok(())
            }
            Some(condition) => self.verify_pending_move_when(condition, regs),
        }
    }

    /// Resolve a conditional proof after an ordinary choice and admission.
    /// The operation is removed only while evaluating a synchronous query:
    /// its caller's slots remain accessible and no reaction can observe the
    /// temporary Bound or the current instruction's projected record.
    pub(super) fn verify_pending_move_when(
        &mut self,
        condition: &'static Cond,
        regs: Regs,
    ) -> Result<(), Error> {
        if matches!(condition, Cond::All([])) {
            self.verify_pending_move();
            return Ok(());
        }
        if let Some(Frame::Choose { tasks, .. }) = self.frames.last_mut() {
            for task in tasks {
                if matches!(
                    task,
                    Task::Operation(Operation::Move { .. } | Operation::Claim { .. }, _)
                        | Task::CapturedOperation(..)
                        | Task::BlindOperations(_)
                ) {
                    *task = Task::VerifyWhen(Box::new(ConditionalVerification {
                        task: task.clone(),
                        condition,
                        regs,
                    }));
                }
            }
            return Ok(());
        }
        let Some(Frame::Operation {
            request,
            context,
            knowledge,
            ..
        }) = self.frames.last()
        else {
            return Ok(());
        };
        let (request, context, knowledge) = (*request, *context, *knowledge);
        let target = match request {
            Operation::Move {
                target,
                destination,
                verb,
            } => {
                if (verb == Verb::Transfer && target.physical == destination)
                    || (target.physical != destination && !self.position.has_room(destination))
                {
                    return Ok(());
                }
                target
            }
            Operation::Claim { target, .. } => {
                if !self.claim_is_applicable(request, context)? {
                    return Ok(());
                }
                target
            }
            _ => return Ok(()),
        };
        if !self.valid_source(target)? {
            return Ok(());
        }
        let reference = knowledge.map_or_else(
            || self.capture(target.card),
            |known| HeldCard::new(target.card, known.evidence),
        );
        if condition.reads_card_front() && !self.reference_visible(reference, context.controller) {
            return Err(Error::Unsupported("private conditional verification"));
        }
        let operation = self.frames.pop().expect("verified operation");
        let result = self.verification_condition(
            *condition,
            context,
            Regs {
                bound: Some(reference),
                ..regs
            },
        );
        // A witness test that does not reveal tells the other observers only
        // which identities this operand is not: those that would have been
        // revealed in its place.
        let witness = condition.witness_only();
        let excluded = match (&result, reference.evidence.pick()) {
            (Ok(false), Some(_)) if witness => {
                let back = self.position.card(target.card).origin_deck();
                let mut excluded = Vec::new();
                for index in 0..self.position.catalog.len() {
                    let candidate = CardId(index as u16);
                    if candidate == target.card
                        || self.position.card(candidate).origin_deck() != back
                    {
                        continue;
                    }
                    let shown = self.verification_condition(
                        *condition,
                        context,
                        Regs {
                            bound: Some(HeldCard::new(candidate, Evidence::PUBLIC)),
                            ..regs
                        },
                    );
                    match shown {
                        Ok(true) => excluded.push(candidate),
                        Ok(false) => {}
                        Err(error) => {
                            self.frames.push(operation);
                            return Err(error);
                        }
                    }
                }
                excluded
            }
            _ => Vec::new(),
        };
        self.frames.push(operation);
        let reveal = result?;
        if let (Some(pick), Some(knowledge)) = (reference.evidence.pick(), self.knowledge.as_mut())
        {
            // Public: every observer's beliefs, the controller's included,
            // must refine the common ones.
            for tracker in &mut knowledge.trackers {
                for &card in &excluded {
                    tracker.exclude_pick_identity(&self.position, pick, card);
                }
            }
        }
        let Some(Frame::Operation { knowledge, .. }) = self.frames.last_mut() else {
            unreachable!()
        };
        let known = knowledge.get_or_insert(OperandKnowledge {
            evidence: reference.evidence,
            conditioned: false,
            private_query: None,
            verify: false,
        });
        // The predicate can read a whole private hand, even when its selected
        // witness is public. Until this original finishes, other observers
        // must not resample an already-decided disclosure branch. A witness
        // test's outcome is public instead: a reveal, or the exclusions above.
        if !witness {
            known.private_query = Some(context.controller);
            known.conditioned |= condition.reads_card_front();
        }
        if reveal {
            self.verify_pending_move();
        }
        Ok(())
    }

    /// Mark the chosen movement without changing its ordinary choice menu.
    /// The reveal happens only after source and destination admission.
    pub(super) fn verify_pending_move(&mut self) {
        let card = match self.frames.last() {
            Some(Frame::Operation {
                request: Operation::Move { target, .. } | Operation::Claim { target, .. },
                knowledge: None,
                ..
            }) => Some(target.card),
            _ => None,
        };
        let captured = card.map(|card| self.capture(card));
        match self.frames.last_mut() {
            Some(Frame::Operation {
                request: Operation::Move { .. } | Operation::Claim { .. },
                knowledge,
                ..
            }) => {
                let known = knowledge.get_or_insert_with(|| OperandKnowledge {
                    evidence: captured.expect("uncaptured movement").evidence,
                    conditioned: false,
                    private_query: None,
                    verify: false,
                });
                known.verify = true;
            }
            Some(Frame::Choose { tasks, .. }) => {
                for task in tasks {
                    if matches!(
                        task,
                        Task::Operation(Operation::Move { .. } | Operation::Claim { .. }, _)
                            | Task::CapturedOperation(..)
                            | Task::BlindOperations(_)
                    ) {
                        *task = Task::Verify(Box::new(task.clone()));
                    }
                }
            }
            _ => {} // Empty or declined movement needs no witness.
        }
    }

    pub(super) fn junk_achievement(&mut self, target: Option<Target>, special: Option<Special>) {
        let held = target.map(|target| self.capture(target.card));
        let Some(Frame::JunkAvailable(batch)) = self.frames.last_mut() else {
            unreachable!("achievement junk quota")
        };
        batch.remaining -= 1;
        let context = batch.context;
        let operation = if let Some(target) = target {
            batch.visited.push(held.expect("selected card"));
            self.movement(Verb::Junk, target, Destination::Junk, context)
        } else {
            let special = special.expect("selected token");
            batch.specials |= 1 << special.index();
            Operation::JunkAvailableSpecial { special }
        };
        self.frames.push(Frame::operation(operation, context));
    }

    pub(super) fn resume_junk_available(&mut self, batch: Box<JunkAvailable>) -> Result<(), Error> {
        if batch.remaining == 0 {
            self.complete(batch.outcome.returned());
            return Ok(());
        }
        let cards = self.select(
            Selector::AvailableAchievements,
            batch.context,
            Regs::EMPTY,
            Excluded {
                cards: &batch.visited,
                ..Excluded::default()
            },
        )?;
        let specials: Vec<_> = self
            .position
            .specials()
            .filter(|&special| {
                self.position.special_location(special) == SpecialLocation::Available
                    && batch.specials & (1 << special.index()) == 0
            })
            .collect();
        let count = cards.len() + specials.len();
        let mut options = Vec::new();
        let mut tasks = Vec::new();
        for target in cards {
            if self
                .position
                .can_view(target.physical, batch.context.controller)
            {
                options.push(Alternative::Move {
                    verb: Verb::Junk,
                    target: self.position.descriptor(target, batch.context.controller),
                });
                tasks.push(Task::JunkAchievement(target));
                continue;
            }
            let back = self.position.card(target.card).origin_deck();
            let option = Alternative::BlindMove {
                verb: Verb::Junk,
                source: target.physical,
                back,
            };
            if let Some(index) = options.iter().position(|prior| *prior == option) {
                let Task::JunkAchievementBlind(group) = &mut tasks[index] else {
                    unreachable!("achievement back group")
                };
                group.targets.push(target);
            } else {
                let index = options.partition_point(|prior| matches!(prior, Alternative::BlindMove { back: prior, .. } if (prior.age, prior.set) <= (back.age, back.set)));
                options.insert(index, option);
                tasks.insert(
                    index,
                    Task::JunkAchievementBlind(Box::new(BlindReturn {
                        targets: vec![target],
                    })),
                );
            }
        }
        for special in specials {
            options.push(Alternative::JunkSpecial(special));
            tasks.push(Task::JunkAchievementSpecial(special));
        }
        if tasks.is_empty() {
            self.complete(batch.outcome.returned());
            return Ok(());
        }
        // One quota includes cards (duplicates count) and singular tokens.
        // A short quota retains every choice of subset. Back-group sampling
        // is never suppressed, even when every remaining achievement goes.
        let reduce = batch.shortcut == Some(Shortcut::AnyOrder)
            && usize::from(batch.remaining) >= count
            && self.position.dependencies_clear(
                &[
                    ShortcutDependency::Move(Verb::Junk),
                    ShortcutDependency::JunkSpecial,
                    ShortcutDependency::ClaimAchievement,
                    ShortcutDependency::ClaimReaction,
                ],
                batch.context,
                false,
            );
        #[cfg(test)]
        let reduce = reduce && !self.shortcuts_disabled;
        if reduce {
            options.truncate(1);
            tasks.truncate(1);
        }
        let context = batch.context;
        self.frames.push(Frame::JunkAvailable(batch));
        self.frames.push(Frame::Choose {
            player: context.controller,
            options: DecisionOptions::new(options, context),
            tasks,
        });
        Ok(())
    }
}

/// Reveals have no meaningful order, but a kept packet must record every
/// disclosed identity rather than only the last child's result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RevealPacket {
    targets: Vec<Target>,
    next: usize,
    context: Context,
    pub(super) last: HeldReturn,
    outcome: HeldBulk,
    /// A failed mandatory selection reveals its proof before returning an
    /// inapplicable result. Keep its private branch confined to this packet.
    pub(super) empty_selection: Option<u8>,
}

impl RevealPacket {
    pub(super) fn new(targets: Vec<Target>, context: Context) -> Self {
        Self {
            targets,
            next: 0,
            context,
            last: HeldReturn::UNIT,
            outcome: HeldBulk::default(),
            empty_selection: None,
        }
    }
    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        if let Some(known) = &mut self.empty_selection {
            map.scalar(known);
        }
        map.context(&mut self.context);
        for target in &mut self.targets {
            map.physical(&mut target.card);
        }
        map.returned(&mut self.last);
        map.bulk(&mut self.outcome);
    }

    fn returned(&self) -> HeldReturn {
        if self.empty_selection.is_some() {
            ReturnValue::Operation(OperationOutcome::empty(Disposition::Inapplicable)).into()
        } else {
            self.outcome.returned()
        }
    }
}
impl Game {
    pub(super) fn resume_reveal_packet(&mut self, mut packet: Box<RevealPacket>) {
        if let Some(outcome) = packet.last.operation() {
            packet.outcome.record_original(outcome);
            if packet.empty_selection.is_none()
                && let Some(card) = packet.last.card()
            {
                self.record_kept(card, outcome.value);
            }
        }
        packet.last = HeldReturn::UNIT;
        if let Some(&target) = packet.targets.get(packet.next) {
            packet.next += 1;
            let context = packet.context;
            let reference = self.capture(target.card);
            self.frames.push(Frame::RevealAll(packet));
            self.frames.push(Frame::Operation {
                request: Operation::Reveal { target },
                context,
                phase: Phase::Discover,
                knowledge: Some(OperandKnowledge {
                    evidence: reference.evidence,
                    conditioned: false,
                    private_query: None,
                    verify: false,
                }),
                short: false,
            });
        } else {
            self.complete(packet.returned());
        }
    }
}

#[cfg(test)]
#[path = "verification_tests.rs"]
mod tests;

impl RevealPacket {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.add(
            K::Reveal,
            parent,
            None,
            &[
                (F::Cursor, self.next as u64),
                (F::Count, self.targets.len() as u64),
            ],
        );
        p.context(id, self.context);
        p.outcome(id, self.returned());
        for (ordinal, target) in self.targets.iter().enumerate() {
            // `next` advanced when the child was started, before its reveal.
            let evidence = if ordinal + 1 < self.next {
                Evidence::PUBLIC
            } else {
                Evidence::selection(&p.game.position, target.physical)
            };
            p.reference(
                K::Record,
                id,
                Captured::new(target.card, evidence),
                &[(F::Ordinal, ordinal as u64)],
            );
        }
    }
}

impl JunkAvailable {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.add(
            K::Batch,
            parent,
            None,
            &[
                (F::Verb, Verb::Junk as u64 + 1),
                (F::Remaining, u64::from(self.remaining)),
                (F::Value, u64::from(self.specials)),
            ],
        );
        p.context(id, self.context);
        p.outcome(id, self.outcome.returned());
        for (ordinal, &card) in self.visited.iter().enumerate() {
            p.reference(K::Record, id, card, &[(F::Ordinal, ordinal as u64)]);
        }
    }
}
