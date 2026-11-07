//! Artifacts rules procedures: the dig or seizure after a Meld action, the
//! showcase at the start of a turn, rotation and the Museum Check. These are
//! scheduler frames and semantic operations, not card-language instructions.
use super::*;

/// A turn's showcase: the displayed Artifact may be dogma'd as a free action,
/// then rotates into a Museum (or hand), which may trigger a Museum Check,
/// then achievement victory is checked ("between actions").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Showcase {
    pub(super) player: Player,
    pub(super) card: CardId,
    pub(super) stage: ShowcaseStage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShowcaseStage {
    Offer,
    Rotate,
    Check,
    Victory,
    Finish,
}

/// A Museum Check: award one Museum to the unique largest holder, then the
/// current player returns every stored Artifact, then the award settles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MuseumCheck {
    pub(super) context: Context,
    pub(super) stage: u8,
}

/// The card a Meld action covered, as it was when the meld committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Covered {
    pub(super) value: u8,
    pub(super) image: Option<usize>,
}

impl Game {
    /// Schedule the turn-start procedure for a player whose display holds an
    /// Artifact. Called after the turn's allowances reset.
    pub(super) fn start_turn_procedure(&mut self, player: Player) {
        if self.position.expansions().artifacts()
            && let Some(card) = self.position.showcase_card(player)
        {
            self.frames.push(Frame::Showcase(Box::new(Showcase {
                player,
                card,
                stage: ShowcaseStage::Offer,
            })));
        }
    }

    /// Whether this card is the showcased Artifact a pending showcase admits
    /// as a free Dogma source.
    pub(super) fn showcase_source(&self, player: Player, card: CardId) -> bool {
        self.position.cards(Zone::Display(player)) == [card]
            && matches!(
                self.frames.last(),
                Some(Frame::Showcase(showcase))
                    if showcase.card == card && showcase.stage == ShowcaseStage::Rotate
            )
    }

    /// The displayed Artifact whose showcase Dogma is running, wherever that
    /// showcase sits beneath the current frames.
    pub(super) fn showcased_source(&self, player: Player, card: CardId) -> bool {
        self.position.cards(Zone::Display(player)) == [card]
            && self
                .frames
                .iter()
                .any(|frame| matches!(frame, Frame::Showcase(showcase) if showcase.card == card))
    }

    pub(super) fn resume_showcase(&mut self, mut showcase: Box<Showcase>) -> Result<(), Error> {
        let player = showcase.player;
        let card = showcase.card;
        let context = Context::new(player);
        match showcase.stage {
            ShowcaseStage::Offer => {
                showcase.stage = ShowcaseStage::Rotate;
                self.frames.push(Frame::Showcase(showcase));
                if !self.position.can_dogma(card) {
                    return Ok(());
                }
                let mut tasks = vec![Task::Showcase {
                    card,
                    auspice: false,
                }];
                let mut options = vec![Alternative::Showcase {
                    card,
                    auspice: false,
                }];
                // With an endorsement offer, Auspice is chosen after it.
                if !self.endorsement_offered(player, card)
                    && self.auspice_offered(player, card, None)
                {
                    tasks.push(Task::Showcase {
                        card,
                        auspice: true,
                    });
                    options.push(Alternative::Showcase {
                        card,
                        auspice: true,
                    });
                }
                tasks.push(Task::Decline);
                options.push(Alternative::Decline);
                self.frames.push(Frame::Choose {
                    player,
                    tasks,
                    options: DecisionOptions::new(options, context),
                });
            }
            ShowcaseStage::Rotate => {
                showcase.stage = ShowcaseStage::Check;
                self.frames.push(Frame::Showcase(showcase));
                // A card moved away during the Dogma is not rotated.
                if self.position.cards(Zone::Display(player)) != [card] {
                    return Ok(());
                }
                let physical = Zone::Display(player);
                let destination = if self.position.available_museums() > 0 {
                    Zone::Museum(player)
                } else {
                    Zone::Hand(player)
                };
                self.frames.push(Frame::operation(
                    Operation::Move {
                        verb: Verb::Rotate,
                        target: Target {
                            card,
                            physical,
                            logical: LogicalZone::Physical(physical),
                        },
                        destination,
                    },
                    context,
                ));
            }
            ShowcaseStage::Check => {
                showcase.stage = ShowcaseStage::Victory;
                self.frames.push(Frame::Showcase(showcase));
                // After every rotation: a check whenever no Museum is available.
                if self.position.available_museums() == 0 {
                    self.frames
                        .push(Frame::MuseumCheck(MuseumCheck { context, stage: 0 }));
                }
            }
            ShowcaseStage::Victory => {
                showcase.stage = ShowcaseStage::Finish;
                self.frames.push(Frame::Showcase(showcase));
                self.frames.push(Frame::operation(
                    Operation::CheckAchievementVictory,
                    context,
                ));
            }
            ShowcaseStage::Finish => self.complete(ReturnValue::Unit),
        }
        Ok(())
    }

    /// Start the showcase's free Dogma action with the displayed Artifact.
    pub(super) fn start_showcase_dogma(
        &mut self,
        card: CardId,
        auspice: bool,
    ) -> Result<(), Error> {
        self.start_action(Action::Dogma { card, auspice }, ActionKind::Free, false)
    }

    pub(super) fn resume_museum_check(&mut self, mut check: MuseumCheck) -> Result<(), Error> {
        let context = check.context;
        match check.stage {
            0 => {
                check.stage = 1;
                self.frames.push(Frame::MuseumCheck(check));
                let stored = Player::ALL.map(|p| self.position.cards(Zone::Museum(p)).len());
                let winner = match stored[0].cmp(&stored[1]) {
                    std::cmp::Ordering::Greater => Some(Player::One),
                    std::cmp::Ordering::Less => Some(Player::Two),
                    std::cmp::Ordering::Equal => None,
                };
                if let Some(winner) = winner {
                    self.frames.push(Frame::operation(
                        Operation::ClaimMuseum { player: winner },
                        context.rules_for(winner),
                    ));
                }
            }
            1 => {
                // The current player returns all players' stored Artifacts,
                // one at a time, re-selected from live state after each.
                // Returns to distinct decks commute, under the ordinary
                // origin reduction and its Figure guard; same-deck orders
                // stay a real choice.
                check.stage = 2;
                self.frames.push(Frame::MuseumCheck(check));
                let mut batch = MoveAll::new(
                    Verb::Return,
                    Selector::Museum(Who::Either),
                    Destination::Deck,
                    context,
                );
                batch.shortcut = Some(Shortcut::ByOrigin {
                    descending: false,
                    quota: false,
                });
                self.frames.push(Frame::MoveAll(Box::new(batch)));
            }
            _ => {
                self.position.settle_museum_claim();
                self.complete(ReturnValue::Unit);
            }
        }
        Ok(())
    }

    /// The card under a Meld action's card right after the meld committed.
    pub(super) fn covered_by_meld(&self, player: Player, card: CardId) -> Option<Covered> {
        let pile = self
            .position
            .cards(Zone::Board(player, self.position.card(card).color));
        let under =
            (pile.len() >= 2 && pile[pile.len() - 1] == card).then(|| pile[pile.len() - 2])?;
        Some(Covered {
            value: self.position.value(under),
            image: self.position.card(under).image_slot(),
        })
    }

    /// One dig is eligible when the melded card's value is no greater than
    /// the covered card's, or both print their image in the same position.
    pub(super) fn dig_eligible(&self, melded: CardId, covered: Covered) -> bool {
        let card = self.position.card(melded);
        self.position.value(melded) <= covered.value
            || (card.image_slot().is_some() && card.image_slot() == covered.image)
    }

    /// Opposing Museum Artifacts of the original dig value: seizing one
    /// cancels the dig.
    pub(super) fn seizable(&self, player: Player, value: u8) -> Vec<Operation> {
        let physical = Zone::Museum(player.opponent());
        self.position
            .cards(physical)
            .iter()
            .filter(|&&card| self.position.value(card) == value)
            .map(|&card| Operation::Move {
                verb: Verb::Seize,
                target: Target {
                    card,
                    physical,
                    logical: LogicalZone::Physical(physical),
                },
                destination: Zone::Museum(player),
            })
            .collect()
    }

    /// The dig for this player at this value, if any: skip ages whose Base
    /// deck is empty, then take the top of that Artifact deck onto the
    /// display. No Base fallback and no exhaustion result.
    pub(super) fn dig(&self, player: Player, value: u8) -> Option<Operation> {
        if !self.position.cards(Zone::Display(player)).is_empty() {
            return None;
        }
        let age = (value..=11)
            .find(|&age| !self.position.cards(Zone::Deck(Deck::base(age))).is_empty())?;
        let deck = Deck {
            set: CardSet::Artifacts,
            age,
        };
        let physical = Zone::Deck(deck);
        let &card = self.position.cards(physical).last()?;
        Some(Operation::Move {
            verb: Verb::Dig,
            target: Target {
                card,
                physical,
                logical: LogicalZone::Physical(physical),
            },
            destination: Zone::Display(player),
        })
    }
}
