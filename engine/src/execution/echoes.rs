//! Echoes promotion: step 9 of a performed Meld action, after its fading,
//! special-achievement and victory checks.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Choose,
    Dogma,
}

/// One Meld action's promotion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Promotion {
    /// The card the Meld action melded.
    original: CardId,
    context: Context,
    stage: Stage,
    last: Option<HeldOutcome>,
}

impl Promotion {
    pub(super) fn new(original: CardId, context: Context) -> Self {
        Self {
            original,
            context,
            stage: Stage::Choose,
            last: None,
        }
    }

    pub(super) fn receive(&mut self, value: HeldReturn) {
        self.last = value.operation();
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            original,
            context,
            stage: _,
            last,
        } = self;
        map.physical(original);
        map.context(context);
        map.outcome_option(last);
    }
}

impl Game {
    pub(super) fn resume_promotion(&mut self, mut promotion: Box<Promotion>) -> Result<(), Error> {
        let context = promotion.context;
        let player = context.executor;
        match promotion.stage {
            Stage::Choose => {
                // The melded card must still be on top when step 9 is reached.
                let original = self.position.card(promotion.original);
                if self.position.top(player, original.color) != Some(promotion.original) {
                    self.complete(ReturnValue::Unit);
                    return Ok(());
                }
                // Mandatory: one Forecast card of value at most the original's.
                let limit = self.position.value(promotion.original);
                let forecast = Zone::Forecast(player);
                let melds: Vec<_> = self
                    .position
                    .cards(forecast)
                    .iter()
                    .filter(|&&card| self.position.value(card) <= limit)
                    .map(|&card| Operation::Move {
                        verb: Verb::Meld,
                        target: Target {
                            card,
                            physical: forecast,
                            logical: LogicalZone::Physical(forecast),
                        },
                        destination: Zone::Board(player, self.position.card(card).color),
                    })
                    .collect();
                if melds.is_empty() {
                    self.complete(ReturnValue::Unit);
                    return Ok(());
                }
                promotion.stage = Stage::Dogma;
                self.frames.push(Frame::Promote(promotion));
                self.choose_operations(context, melds, false);
            }
            Stage::Dogma => {
                // Only a performed promotion meld earns the free Dogma, and never for a
                // City, Figure or Battleship Yamato. It is a separate free action, not
                // the first or second.
                let promoted = promotion
                    .last
                    .filter(|outcome| outcome.disposition == Disposition::Performed)
                    .and_then(|outcome| outcome.card)
                    .filter(|&card| {
                        self.position.can_dogma(card)
                            && self.position.top(player, self.position.card(card).color)
                                == Some(card)
                    });
                let Some(card) = promoted else {
                    self.complete(ReturnValue::Unit);
                    return Ok(());
                };
                let action = Action::Dogma {
                    card,
                    auspice: false,
                };
                let body = self.free_dogma_body(card, context)?;
                self.push_action_returns();
                self.frames.push(Frame::ActionTail {
                    action,
                    context,
                    kind: ActionKind::Free,
                    advance_turn: false,
                    stage: 0,
                    last: None,
                    shared: false,
                    endorsed: false,
                    melded: false,
                    promoted: true,
                });
                self.frames.push(body);
            }
        }
        Ok(())
    }

    /// A free Dogma action's start: the endorsement offer when there is one
    /// (Auspice follows it), else the Auspice choice when offered, else the
    /// Dogma itself.
    fn free_dogma_body(&self, card: CardId, context: Context) -> Result<Frame, Error> {
        let action = Action::Dogma {
            card,
            auspice: false,
        };
        if let Some(prep) = self.dogma_preparation(action, ActionKind::Free)? {
            return Ok(Frame::DogmaPrep(Box::new(prep)));
        }
        let request = self.action_operation(action, ActionKind::Free)?;
        if self.auspice_offered(context.executor, card, None) {
            let source_owner = self
                .position
                .dogma_source_owner(context.executor, card)
                .ok_or(Error::InvalidAction)?;
            return Ok(Frame::DogmaPrep(Box::new(cities::DogmaPrep::auspice(
                card,
                Context {
                    source_owner,
                    source_card: Some(card),
                    source_evidence: Evidence::PUBLIC,
                    ..context
                },
            ))));
        }
        Ok(Frame::operation(request, context))
    }

    /// Whether the executing effect's card was promoted for this free Dogma action: the
    /// nearest action is that promotion's Dogma, and its Dogma was not replaced. Shared
    /// and copied executions of the promoted card inside that Dogma are foreseen too;
    /// other cards never.
    pub(super) fn foreseen(&self, context: Context) -> bool {
        let Some(source) = context.source_card else {
            return false;
        };
        let mut dogma = None;
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Dogma(running) => dogma = Some(running.card),
                Frame::ActionTail {
                    action: Action::Dogma { card, .. },
                    promoted,
                    ..
                } => return *promoted && *card == source && dogma == Some(source),
                Frame::ActionTail { .. } => return false,
                _ => {}
            }
        }
        false
    }
}

impl Promotion {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.reference(
            K::Promotion,
            parent,
            Captured::new(self.original, Evidence::PUBLIC),
            &[(F::Stage, self.stage as u64)],
        );
        p.context(id, self.context);
        if let Some(last) = self.last {
            p.outcome(id, last.with(ReturnValue::Operation(last.inner)));
        }
    }
}
