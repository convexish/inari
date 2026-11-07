use super::*;

impl Game {
    /// Record successful originals in their owning demand, never descendants.
    pub(super) fn note_demand_move(&mut self, context: Context, verb: Verb) -> Result<(), Error> {
        let mut invocation = None;
        for frame in self.frames.iter_mut().rev() {
            match frame {
                Frame::Program {
                    source,
                    context: scope,
                    ..
                } => {
                    let Source::Effect { card, effect } = *source else {
                        return Ok(());
                    };
                    if invocation.is_some()
                        || *scope != context
                        || !self
                            .position
                            .card(card)
                            .effect(effect)
                            .is_some_and(|e| e.demand())
                    {
                        return Ok(());
                    }
                    invocation = Some((card, effect));
                }
                Frame::Dogma(dogma) => {
                    if invocation.is_some()
                        && invocation == dogma.current()
                        && context.demander == dogma.player
                    {
                        let count = match verb {
                            Verb::Transfer => &mut dogma.demand_transfers,
                            Verb::Return => &mut dogma.demand_returns,
                            _ => return Ok(()),
                        };
                        *count = count
                            .checked_add(1)
                            .ok_or(Error::Unsupported("demand movement count overflow"))?;
                    }
                    return Ok(());
                }
                Frame::Execute(execution) => {
                    execution.note_move(invocation, context, verb)?;
                    return Ok(());
                }
                Frame::Karma { .. } | Frame::ActionTail { .. } => {
                    return Ok(());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Each copied execution owns its summary: self-execution skips demands,
    /// while super-execution records them without inheriting an outer action.
    pub(super) fn prior_demand_transfers(&self, context: Context) -> u32 {
        self.prior_demand_moves(context, Verb::Transfer)
    }

    pub(super) fn prior_demand_returns(&self, context: Context) -> u32 {
        self.prior_demand_moves(context, Verb::Return)
    }

    fn prior_demand_moves(&self, context: Context, verb: Verb) -> u32 {
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Dogma(dogma) => {
                    return if context.source_card == Some(dogma.card)
                        && context.demander == dogma.player
                    {
                        match verb {
                            Verb::Transfer => dogma.demand_transfers,
                            Verb::Return => dogma.demand_returns,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                }
                Frame::Execute(execution) => return execution.prior_moves(context, verb),
                Frame::Program { .. } | Frame::Karma { .. } | Frame::ActionTail { .. } => return 0,
                _ => {}
            }
        }
        0
    }
}
