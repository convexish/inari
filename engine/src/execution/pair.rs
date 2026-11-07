//! Two cards committed together before either moves (Hypersonics).
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MatchingPair {
    verb: Verb,
    from: Selector,
    to: Destination,
    context: Context,
    regs: Regs,
    targets: Option<[Target; 2]>,
    next: usize,
    pub(super) outcome: HeldBulk,
}

impl MatchingPair {
    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            verb: _,
            from: _,
            to: _,
            context,
            regs,
            targets: _,
            next: _,
            outcome,
        } = self;
        map.context(context);
        map.regs(regs);
        // This instruction requires a public-board cohort at selection. Both
        // operands remain named identities after returns or reactions; their
        // remembered origin does not constrain a current hidden slot.
        map.bulk(outcome);
    }
}

impl Game {
    pub(super) fn begin_matching_pair(
        &mut self,
        verb: Verb,
        from: Selector,
        to: Destination,
        context: Context,
        regs: Regs,
    ) -> Result<(), Error> {
        let targets = self.select(from, context, regs, Excluded::default())?;
        // Matching fronts is authorized only for public board cohorts. This
        // instruction must not turn a foreign hidden zone into a color oracle.
        if targets
            .iter()
            .any(|target| !matches!(target.physical, Zone::Board(_, _)))
        {
            return Err(Error::Unsupported(
                "matching pair requires public board cards",
            ));
        }
        let mut options = Vec::new();
        let mut tasks = Vec::new();
        for &first in &targets {
            let card = self.position.card(first.card);
            for &second in &targets {
                let other = self.position.card(second.card);
                if first.card != second.card
                    && card.color != other.color
                    && self.position.value(first.card) == self.position.value(second.card)
                {
                    options.push(Alternative::MatchingPair {
                        first: self.position.descriptor(first, context.controller),
                        second: self.position.descriptor(second, context.controller),
                    });
                    tasks.push(Task::MatchingPair { first, second });
                }
            }
        }
        if tasks.is_empty() {
            self.complete(ReturnValue::Bulk(BulkOutcome::default()));
            return Ok(());
        }
        self.frames.push(Frame::MatchingPair(Box::new(MatchingPair {
            verb,
            from,
            to,
            context,
            regs,
            targets: None,
            next: 0,
            outcome: HeldBulk::default(),
        })));
        self.frames.push(Frame::Choose {
            player: context.controller,
            options: DecisionOptions::new(options, context),
            tasks,
        });
        Ok(())
    }

    pub(super) fn select_matching_pair(&mut self, first: Target, second: Target) {
        let Some(Frame::MatchingPair(mut pair)) = self.frames.pop() else {
            unreachable!("matching pair selection caller");
        };
        pair.targets = Some([first, second]);
        // The original value survives every return and reaction. It is not
        // reconstructed from the last return (which may have been replaced).
        let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
            unreachable!("matching pair program caller");
        };
        *chosen = SelectionParameter::Value(self.position.value(first.card));
        self.frames.push(Frame::MatchingPair(pair));
    }

    pub(super) fn resume_matching_pair(
        &mut self,
        mut pair: Box<MatchingPair>,
    ) -> Result<(), Error> {
        let targets = pair
            .targets
            .expect("matching pair selected before execution");
        if pair.next == targets.len() {
            self.complete(pair.outcome.returned());
            return Ok(());
        }
        let target = targets[pair.next];
        pair.next += 1;
        // Preserve the committed identity and origin. Newly exposed substitutes
        // cannot complete a pair whose second original was moved by a reaction.
        let applicable = self
            .select(pair.from, pair.context, pair.regs, Excluded::default())?
            .contains(&target);
        if applicable {
            let context = pair.context;
            let operation = self.movement(pair.verb, target, pair.to, context);
            self.frames.push(Frame::MatchingPair(pair));
            self.frames.push(Frame::operation(operation, context));
        } else {
            pair.outcome.record(Disposition::Inapplicable);
            self.frames.push(Frame::MatchingPair(pair));
        }
        Ok(())
    }
}
