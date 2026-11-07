//! Choose the complete cohort before any original or reaction can move cards.
//! Selection is a set; the ensuing ordinary batch chooses meaningful orders.
use super::*;

pub(super) fn blind_order(option: &Alternative) -> (u8, u8, u8, CardSet) {
    match option {
        Alternative::BlindSelect { source, back } => {
            let origin = match source {
                Zone::Hand(p) => p.index() as u8,
                Zone::Score(p) => 2 + p.index() as u8,
                Zone::Forecast(p) => 4 + p.index() as u8,
                _ => 6,
            };
            (0, origin, back.age, back.set)
        }
        _ => (1, 0, 0, CardSet::Base),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GroupSelection {
    step: Option<Move>,
    context: Context,
    regs: Regs,
    candidates: Vec<Target>,
    selected: Vec<Captured<Target>>,
    size: u8,
    constraint: Option<GroupConstraint>,
    optional: bool,
    /// "Up to": the chooser may stop after any pick even under a
    /// constraint; an exact group must reach its size.
    flexible: bool,
    /// Visible subsets are selected in identity order. This imposes no
    /// execution order: the consumer chooses that only after commitment.
    canonical: bool,
    stopped: bool,
}

impl GroupSelection {
    pub(super) fn selected_cards(&self) -> impl Iterator<Item = HeldCard> + '_ {
        self.selected.iter().map(|card| card.with(card.card))
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        map.context(&mut self.context);
        map.regs(&mut self.regs);
        for target in &mut self.candidates {
            map.physical(&mut target.card);
        }
        for target in &mut self.selected {
            map.reference(&mut target.inner.card, &mut target.evidence);
        }
    }

    fn accepts(&self, position: &Position, selected: &[Target], target: Target) -> bool {
        if selected.iter().any(|prior| prior.card == target.card) {
            return false;
        }
        if self.canonical
            && selected
                .last()
                .is_some_and(|prior| prior.card >= target.card)
        {
            return false;
        }
        let card = position.card(target.card);
        let value = position.value(target.card);
        selected.iter().all(|prior| {
            let prior_value = position.value(prior.card);
            let prior = position.card(prior.card);
            match self.constraint {
                None => true,
                Some(GroupConstraint::SameValue) => prior_value == value,
                Some(GroupConstraint::DifferentValues) => prior_value != value,
                Some(GroupConstraint::SameValueDifferentColors) => {
                    prior_value == value && prior.color != card.color
                }
                Some(GroupConstraint::SameColor) => prior.color == card.color,
            }
        })
    }

    /// Exact groups admit only picks that can still complete the prerequisite.
    fn completable(&self, position: &Position, selected: &mut Vec<Target>) -> bool {
        if self.flexible || self.constraint.is_none() || selected.len() == usize::from(self.size) {
            return true;
        }
        for &target in &self.candidates {
            if self.accepts(position, selected, target) {
                selected.push(target);
                let complete = self.completable(position, selected);
                selected.pop();
                if complete {
                    return true;
                }
            }
        }
        false
    }
}

impl Game {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_group(
        &mut self,
        from: Selector,
        size: u8,
        constraint: Option<GroupConstraint>,
        optional: bool,
        step: Option<Move>,
        flexible: bool,
        context: Context,
        regs: Regs,
    ) -> Result<(), Error> {
        let mut candidates = self.select(from, context, regs, Excluded::default())?;
        let canonical = candidates
            .iter()
            .all(|target| self.position.can_view(target.physical, context.controller));
        // Values are on backs. Colors are not: do not turn an exact-group
        // availability or completion menu into a hidden-front oracle.
        if !canonical
            && matches!(
                constraint,
                Some(GroupConstraint::SameValueDifferentColors | GroupConstraint::SameColor)
            )
        {
            return Err(Error::Unsupported("private filtered card selection"));
        }
        if canonical {
            candidates.sort_by_key(|target| target.card);
        }
        self.frames
            .push(Frame::SelectGroup(Box::new(GroupSelection {
                step,
                context,
                regs,
                candidates,
                selected: Vec::new(),
                size,
                constraint,
                optional,
                flexible,
                canonical,
                stopped: false,
            })));
        Ok(())
    }

    pub(super) fn choose_group_card(&mut self, target: Target) {
        let mut reference = self.capture(target.card).with(target);
        // Even a publicly named member needs a pick to constrain the other
        // members if a later reaction moves it into a hidden pile.
        if reference.evidence.pick().is_none()
            && let Some(knowledge) = &mut self.knowledge
            && let Some(pick) = knowledge.allocate()
        {
            for tracker in &mut knowledge.trackers {
                tracker.mark(&self.position, target.card, pick);
            }
            reference.evidence = reference.evidence.with_pick(Some(pick));
        }
        let Some(Frame::SelectGroup(group)) = self.frames.last_mut() else {
            unreachable!("group choice caller");
        };
        assert!(
            !group.selected.iter().any(|prior| prior.card == target.card),
            "a group cannot select the same original twice"
        );
        if let Some(knowledge) = &mut self.knowledge
            && let Some(pick) = reference.evidence.pick()
        {
            for prior in &group.selected {
                if let Some(other) = prior.evidence.pick() {
                    knowledge.distinct[pick.index()] |= 1 << other.index();
                    knowledge.distinct[other.index()] |= 1 << pick.index();
                }
            }
        }
        group.selected.push(reference);
    }

    pub(super) fn refresh_sampled_group_menu(&mut self) -> Result<(), Error> {
        let refresh = matches!(
            self.frames.as_slice(),
            [.., Frame::SelectGroup(_), Frame::Choose { .. }]
        ) || matches!(
            self.frames.as_slice(),
            [.., Frame::MoveAll(batch), Frame::Choose { tasks, .. }]
                if batch.frozen.is_some() || tasks.iter().any(|task| matches!(task, Task::MoveAllBlind(_)))
        );
        if refresh {
            self.frames.pop();
            let frame = self.frames.pop().expect("sampled group caller");
            let mut events = Vec::new();
            self.resume(frame, &mut events)?;
            debug_assert!(events.is_empty(), "rebuilding a menu performs no operation");
        }
        Ok(())
    }

    pub(super) fn stop_group(&mut self) {
        let Some(Frame::SelectGroup(group)) = self.frames.last_mut() else {
            unreachable!("group stop caller");
        };
        group.stopped = true;
    }

    pub(super) fn resume_group(&mut self, group: Box<GroupSelection>) -> Result<(), Error> {
        if !group.stopped && group.selected.len() < usize::from(group.size) {
            let mut selected: Vec<_> = group.selected.iter().map(|t| t.inner).collect();
            let targets: Vec<_> = group
                .candidates
                .iter()
                .copied()
                .filter(|&target| {
                    if !group.accepts(&self.position, &selected, target) {
                        return false;
                    }
                    selected.push(target);
                    let complete = group.completable(&self.position, &mut selected);
                    selected.pop();
                    complete
                })
                .collect();
            let mut options = Vec::new();
            let mut tasks = Vec::new();
            for target in targets {
                match self.position.descriptor(target, group.context.controller) {
                    TargetDescriptor::Hidden { source, back, .. }
                        if !matches!(source, Zone::Safe(_)) =>
                    {
                        let option = Alternative::BlindSelect { source, back };
                        if let Some(index) = options.iter().position(|prior| *prior == option) {
                            let Task::GroupBlind(blind) = &mut tasks[index] else {
                                unreachable!("group blind selection");
                            };
                            blind.targets.push(target);
                        } else {
                            options.push(option);
                            tasks.push(Task::GroupBlind(Box::new(BlindReturn {
                                targets: vec![target],
                            })));
                        }
                    }
                    descriptor => {
                        options.push(Alternative::Select { target: descriptor });
                        tasks.push(Task::GroupCard(target));
                    }
                }
            }
            if !tasks.is_empty() {
                let mut choices: Vec<_> = options.into_iter().zip(tasks).collect();
                choices.sort_by_key(|(option, _)| blind_order(option));
                let (mut options, mut tasks): (Vec<_>, Vec<_>) = choices.into_iter().unzip();
                if (group.optional && group.selected.is_empty() && !group.context.must())
                    || ((group.constraint.is_none() || group.flexible)
                        && !group.selected.is_empty())
                {
                    options.push(Alternative::Decline);
                    tasks.push(Task::StopGroup);
                }
                let context = group.context;
                self.frames.push(Frame::SelectGroup(group));
                self.frames.push(Frame::Choose {
                    player: context.controller,
                    options: DecisionOptions::new(options, context),
                    tasks,
                });
                return Ok(());
            }
        }
        if let Some(step) = group.step {
            if group.selected.len() != usize::from(group.size) || group.selected.is_empty() {
                self.complete(ReturnValue::Bulk(BulkOutcome::default()));
            } else {
                let mut batch = MoveAll::new(step.verb, step.from, step.to, group.context);
                batch.regs = group.regs;
                batch.shortcut = step.shortcut;
                batch.verify = step.verify;
                batch.when = step.when;
                batch.frozen = Some(group.selected);
                self.frames.push(Frame::MoveAll(Box::new(batch)));
            }
        } else {
            self.record_chosen_group(group.selected.iter().map(|t| t.with(t.card)).collect());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;

impl GroupSelection {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.add(
            K::Group,
            parent,
            None,
            &[
                (F::Limit, u64::from(self.size)),
                (F::Count, self.selected.len() as u64),
                (F::Optional, u64::from(self.optional)),
                (F::Stopped, u64::from(self.stopped)),
                (
                    F::Constraint,
                    self.constraint
                        .map_or(0, |constraint| constraint as u64 + 1),
                ),
            ],
        );
        p.context(id, self.context);
        for (ordinal, target) in self.selected.iter().enumerate() {
            p.reference(
                K::Record,
                id,
                target.with(target.card),
                &[(F::Ordinal, ordinal as u64)],
            );
        }
    }
}
