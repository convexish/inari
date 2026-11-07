//! Cities rules that follow a committed operation: the City draw after a splay and the
//! five event-earned special achievements. Each runs as an ordinary operation before
//! the caller resumes, so karma can react to it, a terminal result interrupts the
//! caller, and the original operation's result survives (`Frame::ReturnOutcome`).

use super::*;
use crate::program::build::{draw_age, filtered, junk};

/// After a performed Meld action, the Cities steps of the trigger order: the City's
/// icons (step 2), the new-color City draw (step 3) and Figures' When karma (step 5).
/// Each needs the melded card still on top when it is reached; the City's icons are
/// checked once and then run together. Step 1, the event-earned claim, already ran as
/// the meld's consequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MeldAction {
    pub(super) card: CardId,
    pub(super) player: Player,
    /// The committed meld started this color's pile.
    pub(super) new_color: bool,
    /// The card the meld covered, as it was at the commit (Artifacts dig).
    pub(super) covered: Option<artifacts::Covered>,
    /// Opposing Museum count when the seizure was offered.
    pub(super) opposing_museums: u8,
    pub(super) context: Context,
    pub(super) stage: MeldStage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeldStage {
    /// Entering step 2.
    Icons,
    /// Running the City's icons from this position of [`ICON_ORDER`].
    Icon(u8),
    NewColorDraw,
    /// Artifacts step 4: offer a seizure, then dig.
    Dig,
    DigDraw,
    When,
}

/// The printed reading order of a City's positions: top-left, top-center,
/// top-right, then the bottom row. Action icons sit top-center and
/// bottom-center, so top-center runs first.
const ICON_ORDER: [usize; SLOTS] = [0, 4, 5, 1, 2, 3];

/// A Search packet: the revealed cards that did not match, still in place on
/// top of their Base deck until the player returns them to its bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CitySearch {
    pub(super) city: CardId,
    pub(super) context: Context,
    pub(super) icon: Icon,
    pub(super) deck: Deck,
    pub(super) revealed: bool,
    pub(super) pending: Vec<CardId>,
    /// The player's next return, taken when the frame resumes.
    pub(super) chosen: Option<CardId>,
}

const SOURCE_VALUE: Expr = Expr::Value(Reg::Source);
const AVAILABLE: Selector = Selector::AvailableAchievements;

const fn splay(who: Who, direction: Splay) -> Instruction {
    Instruction::Splay(SplayStep {
        who,
        choices: SplayChoices::Fixed {
            colors: ColorSet::Of(Reg::Source),
            direction,
            from: None,
        },
        targets: SplayTargets::Changes,
        optional: false,
        all: false,
    })
}

// The shared rule programs of the City icons, run with the City as source.
// Ages are computed from the City's value, never boxed literals.
/// Draw a card of value one higher than the City.
const PLUS: &[Instruction] = &[draw_age(Expr::Add(&SOURCE_VALUE, 1))];
/// Junk all cards in the Base deck one higher, then draw two higher whether
/// or not anything was junked.
const UPLIFT: &[Instruction] = &[
    junk(Selector::BaseDeck(Expr::Add(&SOURCE_VALUE, 1)))
        .all()
        .shortcut(Shortcut::DeckTop),
    draw_age(Expr::Add(&SOURCE_VALUE, 2)),
];
/// Junk an available standard achievement of the City's value.
const JUNK: &[Instruction] = &[junk(filtered(
    &AVAILABLE,
    &[Filter::Has(Test::Value(Cmp::Equal, SOURCE_VALUE))],
))];
const LEFT: &[Instruction] = &[splay(Who::Executor, Splay::Left)];
const RIGHT: &[Instruction] = &[splay(Who::Executor, Splay::Right)];
const UP: &[Instruction] = &[splay(Who::Executor, Splay::Up)];
/// The opponent unsplays the City's color.
const UNSPLAY: &[Instruction] = &[splay(Who::Opponent, Splay::None)];

pub(super) fn icon_program(icon: CityIcon) -> &'static [Instruction] {
    match icon {
        CityIcon::Plus => PLUS,
        CityIcon::Uplift => UPLIFT,
        CityIcon::Junk => JUNK,
        CityIcon::Left => LEFT,
        CityIcon::Right => RIGHT,
        CityIcon::Up => UP,
        CityIcon::Unsplay => UNSPLAY,
        CityIcon::Flag | CityIcon::Fountain => &[],
    }
}

impl Game {
    /// Publicly revealed Search cards still awaiting their return to the
    /// deck bottom. Both players may inspect these while choosing the order;
    /// other deck cards, including completed returns, are excluded.
    pub fn city_search_returns(&self) -> impl Iterator<Item = CardId> + '_ {
        self.frames
            .iter()
            .filter_map(|frame| match frame {
                Frame::CitySearch(search) => Some(search.pending.iter().copied()),
                _ => None,
            })
            .flatten()
    }

    /// Schedule a committed operation's Cities consequences outside the
    /// common continuation loop. A Meld action defers When to its pipeline;
    /// an effect meld keeps it immediate.
    pub(super) fn finish_city_operation(
        &mut self,
        request: Operation,
        context: Context,
        outcome: HeldOutcome,
    ) {
        let pipeline = self.meld_action(request, context, outcome.inner);
        let melded = if pipeline.is_some() {
            None
        } else {
            self.when_melded(request, context, outcome.inner)
        };
        let city = self.city_consequence(request, context, outcome.inner);
        let proof = self.city_junk_proof(request, context, outcome.inner);
        if melded.is_none() && city.is_none() && proof.is_none() && pipeline.is_none() {
            self.complete(outcome.returned());
        } else {
            // The original result returns after its consequences.
            self.frames.push(Frame::ReturnOutcome(outcome));
            if let Some(pipeline) = pipeline {
                self.frames.push(Frame::MeldAction(Box::new(pipeline)));
            }
            // A City is never a Figure, so a claim and an immediate When
            // never both run.
            if let Some(hook) = melded {
                self.frames.push(Frame::Karma {
                    hook,
                    stage: 0,
                    visited: Vec::new(),
                    last: None,
                });
            }
            if let Some((request, context)) = city {
                self.frames.push(Frame::operation(request, context));
            }
            // The proof runs first, before the claim it decides.
            if let Some(target) = proof {
                self.frames
                    .push(Frame::operation(Operation::Reveal { target }, context));
            }
        }
    }

    /// The junk is face down, so a City junked from hand while Glory or
    /// Victory is available is revealed: its Flag or Fountain (or neither)
    /// decides the claim.
    fn city_junk_proof(
        &self,
        request: Operation,
        context: Context,
        outcome: OperationOutcome,
    ) -> Option<Target> {
        if !self.position.expansions().cities() || outcome.disposition != Disposition::Performed {
            return None;
        }
        let Operation::Move {
            verb: Verb::Junk,
            target,
            ..
        } = request
        else {
            return None;
        };
        let player = context.executor;
        if target.physical != Zone::Hand(player) && target.logical != LogicalZone::Hand(player) {
            return None;
        }
        if self.position.card(target.card).set != CardSet::Cities
            || [Special::Glory, Special::Victory]
                .into_iter()
                .all(|special| {
                    self.position.special_location(special) != SpecialLocation::Available
                })
        {
            return None;
        }
        let physical = self.position.locate(target.card)?;
        Some(Target {
            card: target.card,
            physical,
            logical: LogicalZone::Physical(physical),
        })
    }

    /// The Cities rule this committed operation triggers, if any. It inherits
    /// karma suppression for its own reactions, but suppression never stops
    /// the rule itself.
    pub(super) fn city_consequence(
        &self,
        request: Operation,
        context: Context,
        outcome: OperationOutcome,
    ) -> Option<(Operation, Context)> {
        if !self.position.expansions().cities() || outcome.disposition != Disposition::Performed {
            return None;
        }
        match request {
            // A performed splay to a direction on any board draws a City for
            // the player who splayed, unless they already hold one (their
            // logical hand, including a hand borrowed through Meiji). The draw
            // asks for Cities at their live highest top value: Base routing
            // skips empty ages first, then falls back to Base if that Cities
            // deck is empty.
            Operation::Splay { direction, .. } if direction != Splay::None => {
                self.city_draw(context)
            }
            // Melding a City bearing an arrow into a pile already splayed that
            // way earns Tradition, Repute or Fame, for an effect meld too.
            Operation::Move {
                verb: Verb::Meld,
                target,
                destination: Zone::Board(owner, color),
            } => {
                let (arrow, special) = match self.position.splay(owner, color) {
                    Splay::Left => (CityIcon::Left, Special::Tradition),
                    Splay::Right => (CityIcon::Right, Special::Repute),
                    Splay::Up => (CityIcon::Up, Special::Fame),
                    Splay::None | Splay::Aslant => return None,
                };
                self.bears(target.card, arrow)
                    .then(|| Self::city_claim(special, context.rules_for(owner)))
            }
            // Junking a City bearing a Flag or Fountain from your hand earns
            // Glory or Victory. Only the junking player can earn it: from their
            // own hand or one they are using (Meiji), never a victim's.
            Operation::Move {
                verb: Verb::Junk,
                target,
                ..
            } => {
                let player = context.executor;
                if target.physical != Zone::Hand(player)
                    && target.logical != LogicalZone::Hand(player)
                {
                    return None;
                }
                [
                    (CityIcon::Flag, Special::Glory),
                    (CityIcon::Fountain, Special::Victory),
                ]
                .into_iter()
                .find(|&(icon, _)| self.bears(target.card, icon))
                .map(|(_, special)| Self::city_claim(special, context.rules_for(player)))
            }
            _ => None,
        }
    }

    /// A City draw for the executor, unless they already hold a City (their
    /// logical hand, including a hand borrowed through Meiji). It asks for
    /// Cities at their live highest top value: Base routing skips empty ages
    /// first, then falls back to Base if that Cities deck is empty.
    fn city_draw(&self, context: Context) -> Option<(Operation, Context)> {
        let player = context.executor;
        let holds_city = self
            .position
            .targets(LogicalZone::Hand(player))
            .any(|target| self.position.card(target.card).set == CardSet::Cities);
        (!holds_city).then(|| {
            let value = self.position.highest_top_value(player);
            let draw = Operation::Draw {
                value,
                set: Some(CardSet::Cities),
            };
            (draw, context)
        })
    }

    /// The Cities pipeline of a performed Meld action, if this committed
    /// operation is one: the action's own meld sits directly on its tail.
    pub(super) fn meld_action(
        &self,
        request: Operation,
        context: Context,
        outcome: OperationOutcome,
    ) -> Option<MeldAction> {
        let expansions = self.position.expansions();
        if !(expansions.cities() || expansions.artifacts())
            || outcome.disposition != Disposition::Performed
        {
            return None;
        }
        let Operation::Move {
            verb: Verb::Meld,
            target,
            destination: Zone::Board(player, color),
        } = request
        else {
            return None;
        };
        matches!(
            self.frames.last(),
            Some(Frame::ActionTail {
                action: Action::Meld { .. },
                ..
            })
        )
        .then(|| MeldAction {
            card: target.card,
            player,
            new_color: self.position.cards(Zone::Board(player, color)).len() == 1,
            covered: expansions
                .artifacts()
                .then(|| self.covered_by_meld(player, target.card))
                .flatten(),
            opposing_museums: 0,
            context,
            stage: MeldStage::Icons,
        })
    }

    pub(super) fn resume_meld_action(&mut self, mut meld: Box<MeldAction>) -> Result<(), Error> {
        let color = self.position.card(meld.card).color;
        let top = self.position.top(meld.player, color) == Some(meld.card);
        match meld.stage {
            MeldStage::Icons => {
                let city = self.position.card(meld.card).set == CardSet::Cities;
                meld.stage = if top && city {
                    MeldStage::Icon(0)
                } else {
                    MeldStage::NewColorDraw
                };
                self.frames.push(Frame::MeldAction(meld));
            }
            MeldStage::Icon(next) => {
                let symbols = self.position.card(meld.card).symbols;
                let found = ICON_ORDER
                    .iter()
                    .enumerate()
                    .skip(usize::from(next))
                    .find_map(|(i, &slot)| match symbols[slot] {
                        Symbol::City(CityIcon::Flag | CityIcon::Fountain) => None,
                        Symbol::City(icon) => Some((i, Some(icon), None)),
                        Symbol::Search(icon) => Some((i, None, Some(icon))),
                        _ => None,
                    });
                let Some((i, icon, search)) = found else {
                    meld.stage = MeldStage::NewColorDraw;
                    self.frames.push(Frame::MeldAction(meld));
                    return Ok(());
                };
                meld.stage = MeldStage::Icon(i as u8 + 1);
                let context = Context {
                    source_card: Some(meld.card),
                    source_evidence: Evidence::PUBLIC,
                    source_owner: meld.player,
                    boxed_action: BoxedValues::ZERO,
                    ..meld.context
                };
                let card = meld.card;
                self.frames.push(Frame::MeldAction(meld));
                if let Some(icon) = search {
                    let age = self.position.value(card);
                    self.frames.push(Frame::CitySearch(Box::new(CitySearch {
                        city: card,
                        context,
                        icon,
                        deck: Deck::base(age),
                        revealed: false,
                        pending: Vec::new(),
                        chosen: None,
                    })));
                } else if let Some(icon) = icon {
                    // A rule execution, not a card effect: no effect karma.
                    self.frames.push(Frame::Program {
                        source: Source::CityIcon { card, icon },
                        pc: 0,
                        context,
                        last: HeldReturn::UNIT,
                        retained: None,
                        chosen: SelectionParameter::None,
                        boxed: BoxedValues::ZERO,
                    });
                }
            }
            MeldStage::NewColorDraw => {
                meld.stage = MeldStage::Dig;
                let draw = (meld.new_color && top && self.position.expansions().cities())
                    .then(|| self.city_draw(meld.context))
                    .flatten();
                self.frames.push(Frame::MeldAction(meld));
                if let Some((request, context)) = draw {
                    self.frames.push(Frame::operation(request, context));
                }
            }
            MeldStage::Dig => {
                // Seizure is offered at the original covered value even when
                // the dig itself would be ignored.
                let offer = match meld.covered {
                    Some(covered) if top && self.dig_eligible(meld.card, covered) => {
                        Some(self.seizable(meld.player, covered.value))
                    }
                    _ => None,
                };
                let Some(seizable) = offer else {
                    meld.stage = MeldStage::When;
                    self.frames.push(Frame::MeldAction(meld));
                    return Ok(());
                };
                meld.stage = MeldStage::DigDraw;
                meld.opposing_museums = self
                    .position
                    .cards(Zone::Museum(meld.player.opponent()))
                    .len() as u8;
                let context = meld.context;
                self.frames.push(Frame::MeldAction(meld));
                if !seizable.is_empty() {
                    self.choose_operations(context, seizable, true);
                }
            }
            MeldStage::DigDraw => {
                meld.stage = MeldStage::When;
                let seized = self
                    .position
                    .cards(Zone::Museum(meld.player.opponent()))
                    .len()
                    < usize::from(meld.opposing_museums);
                let dig = (!seized)
                    .then(|| meld.covered.and_then(|c| self.dig(meld.player, c.value)))
                    .flatten();
                let context = meld.context;
                self.frames.push(Frame::MeldAction(meld));
                if let Some(request) = dig {
                    self.frames.push(Frame::operation(request, context));
                }
            }
            MeldStage::When => match self.when_hook(meld.card, meld.player, meld.context) {
                Some(hook) if top => self.frames.push(Frame::Karma {
                    hook,
                    stage: 0,
                    visited: Vec::new(),
                    last: None,
                }),
                _ => self.complete(ReturnValue::Unit),
            },
        }
        Ok(())
    }

    /// Search: reveal the top cards of the Base deck of the City's age (as many as its
    /// value), take those printing the Search icon's standard icon into hand, and
    /// return the rest to that deck's bottom in the player's order. No part of it
    /// triggers or is affected by karma or expansion draw rules: nothing here is
    /// discovered as an operation, and matching reads printed icons.
    pub(super) fn resume_city_search(
        &mut self,
        mut search: Box<CitySearch>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let context = search.context;
        let player = context.executor;
        let source = Zone::Deck(search.deck);
        if !search.revealed {
            search.revealed = true;
            let cards = self.position.cards(source);
            let packet: Vec<CardId> = cards
                .iter()
                .rev()
                .take(usize::from(search.deck.age).min(cards.len()))
                .copied()
                .collect();
            for &card in &packet {
                if let Some(knowledge) = &mut self.knowledge {
                    for tracker in &mut knowledge.trackers {
                        tracker.reveal(&self.position, card, source, None);
                    }
                }
                events.push(Event::Revealed {
                    card,
                    source,
                    executor: player,
                    inside_karma: context.suppress_karma(),
                });
            }
            for card in packet {
                if self.position.card(card).printed_icon_count(search.icon) > 0 {
                    self.place(
                        Some(Verb::Transfer),
                        card,
                        source,
                        Zone::Hand(player),
                        tracking::Departure::Position,
                        context,
                        events,
                    )?;
                } else {
                    search.pending.push(card);
                }
            }
            self.refresh_tracking()?;
        }
        if let Some(card) = search.chosen.take() {
            let at = search
                .pending
                .iter()
                .position(|&c| c == card)
                .expect("pending Search card");
            search.pending.remove(at);
            self.return_search_card(card, context, events)?;
        }
        match search.pending.len() {
            0 => self.complete(ReturnValue::Unit),
            1 => {
                let card = search.pending.pop().expect("one pending card");
                self.return_search_card(card, context, events)?;
                self.complete(ReturnValue::Unit);
            }
            _ => {
                let options = search
                    .pending
                    .iter()
                    .map(|&card| Alternative::Move {
                        verb: Verb::Return,
                        target: TargetDescriptor::Known { card, source },
                    })
                    .collect();
                let tasks = search
                    .pending
                    .iter()
                    .map(|&card| Task::SearchReturn(card))
                    .collect();
                self.frames.push(Frame::CitySearch(search));
                self.frames.push(Frame::Choose {
                    player: context.controller,
                    tasks,
                    options: DecisionOptions::new(options, context),
                });
            }
        }
        Ok(())
    }

    /// The chosen Search card goes to its deck's bottom next.
    pub(super) fn choose_search_return(&mut self, card: CardId) {
        let Some(Frame::CitySearch(search)) = self.frames.last_mut() else {
            unreachable!("Search return caller");
        };
        search.chosen = Some(card);
    }

    fn return_search_card(
        &mut self,
        card: CardId,
        context: Context,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let deck = Zone::Deck(self.position.card(card).origin_deck());
        self.place(
            Some(Verb::Return),
            card,
            deck,
            deck,
            tracking::Departure::Position,
            context,
            events,
        )?;
        Ok(())
    }

    /// A running Dogma action is endorsed (public: endorsement is announced).
    pub(crate) fn endorsed_action(&self) -> bool {
        self.frames
            .iter()
            .any(|frame| matches!(frame, Frame::ActionTail { endorsed: true, .. }))
    }

    /// A City printing this icon anywhere on its face.
    fn bears(&self, card: CardId, icon: CityIcon) -> bool {
        let card = self.position.card(card);
        card.set == CardSet::Cities && card.printed().contains(&Symbol::City(icon))
    }

    /// An earned claim: the event, not the board, qualifies it, so only the
    /// token's live availability is checked when it commits (after any First
    /// reaction, which cannot cancel it).
    fn city_claim(special: Special, context: Context) -> (Operation, Context) {
        let claim = Operation::ClaimSpecial {
            special,
            check_eligibility: false,
        };
        (claim, context)
    }

    /// With Cities, each of the next `draws` splays (to a direction) may draw
    /// for the executor. Order never changes those draws (a splay moves no
    /// top card), but an exhausting draw would end the game with an
    /// order-dependent board. The public proof: enough Base cards at or above
    /// the request age for every remaining draw, since each draw takes at
    /// most one of them.
    pub(super) fn city_draws_cannot_exhaust(&self, executor: Player, draws: usize) -> bool {
        if !self.position.expansions().cities() {
            return true;
        }
        let age = self.position.highest_top_value(executor);
        (age..=11)
            .map(|age| self.position.cards(Zone::Deck(Deck::base(age))).len())
            .sum::<usize>()
            >= draws
    }
}

/// A Cities Dogma action's preparation: choose the card, endorse or not (paying a hand
/// card), then Auspice, then lock in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DogmaPrep {
    pub(super) card: CardId,
    /// The Dogma's context, with its source admitted before payment.
    pub(super) context: Context,
    pub(super) stage: PrepStage,
    pub(super) endorse: Option<bool>,
    pub(super) icon: Option<Icon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PrepStage {
    Offer,
    Pay,
    Auspice,
    LockIn,
}

impl DogmaPrep {
    /// A Dogma with no endorsement offer that still asks for Auspice (a
    /// promoted card's free Dogma).
    pub(super) fn auspice(card: CardId, context: Context) -> Self {
        Self {
            card,
            context,
            stage: PrepStage::Auspice,
            endorse: None,
            icon: None,
        }
    }
}

const ENDORSE: &[(u8, &str)] = &[(0, "Endorse this Dogma action"), (0, "Do not endorse")];

impl Game {
    /// The endorsement limit for this player and printed featured icon: the
    /// highest value among their top Cities printing that icon anywhere
    /// (a Search icon included). `None` without such a City.
    pub(super) fn endorsement_limit(&self, player: Player, icon: Icon) -> Option<u8> {
        Color::ALL
            .into_iter()
            .filter_map(|color| self.position.top(player, color))
            .map(|card| self.position.card(card))
            .filter(|card| card.set == CardSet::Cities && card.printed_icon_count(icon) > 0)
            .map(|card| card.value)
            .max()
    }

    /// The hand cards (logical: a Meiji-borrowed hand too) that can pay to
    /// endorse a Dogma of this card now.
    fn endorsement_payments(&self, player: Player, card: CardId) -> Vec<Target> {
        let Some(limit) = self
            .position
            .catalog
            .get(usize::from(card.0))
            .and_then(|card| card.featured)
            .and_then(|icon| self.endorsement_limit(player, icon))
        else {
            return Vec::new();
        };
        self.position
            .targets(LogicalZone::Hand(player))
            .filter(|target| self.position.value(target.card) <= limit)
            .collect()
    }

    /// Whether the turn's endorsement is on offer for this Dogma action.
    pub(super) fn endorsement_offered(&self, player: Player, card: CardId) -> bool {
        self.position.expansions().cities()
            && !self.position.endorsed
            && !self.endorsement_payments(player, card).is_empty()
    }

    /// Whether a Dogma of the card offers Auspice (a top Figure of the
    /// player's prints the card's featured icon): only when Avatar in place
    /// of the featured icon can change play, through sharing or through a
    /// top Figure's Dogma karma that would fire and could read the icon or
    /// change icon counts before sharing locks. `ordinal` is the regular
    /// action the Dogma would be (or is), None for a free Dogma.
    pub(super) fn auspice_offered(
        &self,
        player: Player,
        card: CardId,
        ordinal: Option<u8>,
    ) -> bool {
        let Some(featured) = self.position.card(card).featured else {
            return false;
        };
        if !self.position.figures()
            || !self
                .position
                .top_figures(player)
                .any(|figure| self.position.card(figure).printed_icon_count(featured) > 0)
        {
            return false;
        }
        featured != Icon::Avatar
            && (self.dogma_karma_may_use_icon(player, ordinal)
                || self.opponent_shares(player, card, featured)
                    != self.opponent_shares(player, card, Icon::Avatar))
    }

    /// The ordinal of the regular action in progress when it is this
    /// player's own, for an Auspice choice made after an endorsement offer.
    pub(super) fn regular_action_in_progress(&self, player: Player) -> Option<u8> {
        (1..=2).find(|&ordinal| self.own_regular_action(player, ordinal).is_some())
    }

    /// Whether a top Figure's Dogma or share-draw karma that would fire for
    /// this player's Dogma could read the featured icon or move cards on a
    /// board before sharing locks.
    fn dogma_karma_may_use_icon(&self, player: Player, ordinal: Option<u8>) -> bool {
        let watched = self.position.watched();
        if watched & (On::Dogma(None).bit() | On::ShareDraw.bit()) == 0 {
            return false;
        }
        Player::ALL.into_iter().any(|owner| {
            self.position
                .top_figures(owner)
                .filter_map(|card| self.position.card(card).figure)
                .flat_map(|figure| figure.def().karma)
                .any(|karma| karma_may_use_icon(karma, owner == player, ordinal, watched))
        })
    }

    /// Whether the opponent shares this player's Dogma of the card using the
    /// icon, before any majority override. A showcased source adds its
    /// printed icons to its owner's count for this comparison only.
    pub(super) fn opponent_shares(&self, player: Player, card: CardId, icon: Icon) -> bool {
        let displayed = if self.position.locate(card) == Some(Zone::Display(player)) {
            self.position.card(card).printed_icon_count(icon)
        } else {
            0
        };
        self.position.icon_count(player.opponent(), icon)
            >= self.position.icon_count(player, icon) + displayed
    }

    /// A Dogma action with an endorsement offer is prepared in stages; its
    /// Auspice choice follows the endorsement, so it cannot be preselected.
    pub(super) fn dogma_preparation(
        &self,
        action: Action,
        kind: ActionKind,
    ) -> Result<Option<DogmaPrep>, Error> {
        let player = self.position.turn_owner;
        let Action::Dogma { card, auspice } = action else {
            return Ok(None);
        };
        if !self.endorsement_offered(player, card) {
            return Ok(None);
        }
        if auspice {
            return Err(Error::InvalidAction);
        }
        // Admit the source now, as the plain action does.
        let Operation::Dogma { .. } = self.action_operation(action, kind)? else {
            unreachable!("Dogma action operation");
        };
        let source_owner = self
            .position
            .dogma_source_owner(player, card)
            .or_else(|| self.showcase_source(player, card).then_some(player))
            .ok_or(Error::InvalidAction)?;
        Ok(Some(DogmaPrep {
            card,
            context: Context {
                source_owner,
                source_card: Some(card),
                source_evidence: Evidence::PUBLIC,
                ..Context::new(player)
            },
            stage: PrepStage::Offer,
            endorse: None,
            icon: None,
        }))
    }

    pub(super) fn resume_dogma_prep(
        &mut self,
        mut prep: Box<DogmaPrep>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let context = prep.context;
        let player = context.executor;
        match prep.stage {
            PrepStage::Offer => {
                prep.stage = PrepStage::Pay;
                self.frames.push(Frame::DogmaPrep(prep));
                let mut options = DecisionOptions::new(
                    vec![Alternative::Branch(0), Alternative::Branch(1)],
                    context,
                );
                options.branches = ENDORSE;
                self.frames.push(Frame::Choose {
                    player,
                    tasks: vec![Task::Endorse(true), Task::Endorse(false)],
                    options,
                });
            }
            PrepStage::Pay => {
                prep.stage = PrepStage::Auspice;
                let endorse = prep.endorse == Some(true);
                let card = prep.card;
                self.frames.push(Frame::DogmaPrep(prep));
                if endorse {
                    // Choosing to endorse uses the turn's endorsement and doubles the
                    // action even if a karma alters the payment.
                    self.position.endorsed = true;
                    for frame in self.frames.iter_mut().rev() {
                        if let Frame::ActionTail { endorsed, .. } = frame {
                            *endorsed = true;
                            break;
                        }
                    }
                    events.push(Event::Endorsed { player, card });
                    let payments = self
                        .endorsement_payments(player, card)
                        .into_iter()
                        .map(|target| Operation::Move {
                            verb: Verb::Junk,
                            target,
                            destination: Zone::Junk,
                        })
                        .collect();
                    self.choose_operations(context, payments, false);
                }
            }
            PrepStage::Auspice => {
                prep.stage = PrepStage::LockIn;
                let featured = self.position.card(prep.card).featured.expect("Dogma card");
                let auspice = self.auspice_offered(
                    player,
                    prep.card,
                    self.regular_action_in_progress(player),
                );
                self.frames.push(Frame::DogmaPrep(prep));
                if auspice {
                    self.frames.push(Frame::Choose {
                        player,
                        tasks: vec![Task::DogmaIcon(featured), Task::DogmaIcon(Icon::Avatar)],
                        options: DecisionOptions::new(
                            vec![Alternative::Icon(featured), Alternative::Icon(Icon::Avatar)],
                            context,
                        ),
                    });
                }
            }
            PrepStage::LockIn => {
                let featured = self.position.card(prep.card).featured;
                let icon = prep.icon.or(featured).expect("Dogma card");
                // The action tail supplies the icon to sharing reactions and
                // replacement actions after this preparation has finished.
                let Some(Frame::ActionTail {
                    action: Action::Dogma { auspice, .. },
                    ..
                }) = self.frames.last_mut()
                else {
                    unreachable!("prepared Dogma action's tail");
                };
                *auspice = icon == Icon::Avatar;
                self.frames.push(Frame::operation(
                    Operation::Dogma {
                        card: prep.card,
                        icon,
                    },
                    context,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn choose_endorsement(&mut self, endorse: bool) {
        let Some(Frame::DogmaPrep(prep)) = self.frames.last_mut() else {
            unreachable!("endorsement caller");
        };
        prep.endorse = Some(endorse);
    }

    pub(super) fn choose_dogma_icon(&mut self, icon: Icon) {
        let Some(Frame::DogmaPrep(prep)) = self.frames.last_mut() else {
            unreachable!("Auspice caller");
        };
        prep.icon = Some(icon);
    }
}

/// Whether a Figure's karma, watching this player's Dogma, could make Auspice
/// matter: it fires for the Dogma (actor, action ordinal) and reads the icon
/// (a Dogma trigger naming an icon, a share-draw trigger, a chosen-icon
/// test) or could change a board's icons before sharing locks (melds, tucks,
/// splays, executions, moves from a board, or moves another karma watches).
fn karma_may_use_icon(
    karma: &Karma,
    owned_by_executor: bool,
    ordinal: Option<u8>,
    watched: u32,
) -> bool {
    if karma.reaction == Reaction::Melded {
        return false;
    }
    let fires_for_actor = match karma.actor {
        Who::Executor => owned_by_executor,
        Who::Opponent => !owned_by_executor,
        Who::Either => true,
        Who::Demander => false,
    };
    if !fires_for_actor
        || karma
            .when
            .is_some_and(|when| required_ordinal(when).is_some_and(|n| ordinal != Some(n)))
    {
        return false;
    }
    match karma.on {
        On::Dogma(Some(_)) | On::ShareDraw => true,
        On::Dogma(None) => match karma.body {
            Body::Native(_) => true,
            Body::Program(program) => program
                .iter()
                .any(|instruction| instruction_may_use_icon(instruction, watched)),
        },
        _ => false,
    }
}

/// The regular-action ordinal a karma condition requires, when it names one.
fn required_ordinal(cond: &Cond) -> Option<u8> {
    match cond {
        Cond::Action(ordinal) => Some(*ordinal),
        Cond::All(conds) => conds.iter().find_map(required_ordinal),
        _ => None,
    }
}

fn instruction_may_use_icon(instruction: &Instruction, watched: u32) -> bool {
    match instruction {
        Instruction::Move(step) => {
            matches!(step.verb, Verb::Meld | Verb::Tuck | Verb::Dig)
                || matches!(step.to, Destination::Board(_) | Destination::Origin(_))
                || selector_may_use_icon(&step.from)
                || step.when.is_some()
                || step.reveal_if_empty.is_some()
                || watched & On::Move(&[]).bit() != 0
                || (step.verb == Verb::Achieve && watched & On::Claim.bit() != 0)
        }
        Instruction::Draw(step) => step.then == Then::Achieve || watched & On::Draw.bit() != 0,
        Instruction::Reveal { unless_proven, .. } => unless_proven.is_some(),
        // A condition may read the chosen icon.
        Instruction::If(..) => true,
        Instruction::Choose { .. }
        | Instruction::Bind(_)
        | Instruction::Jump(_)
        | Instruction::Repeat { .. }
        | Instruction::Block(_)
        | Instruction::Win(_)
        | Instruction::EndAction
        | Instruction::Flip => false,
        Instruction::Splay(_)
        | Instruction::Exchange(_)
        | Instruction::Execute(_)
        | Instruction::Special(_)
        | Instruction::Subgame
        | Instruction::Native(_) => true,
    }
}

/// Whether a selection may take cards off a board or test the chosen icon.
fn selector_may_use_icon(selector: &Selector) -> bool {
    match selector {
        Selector::Hand(_)
        | Selector::Score(_)
        | Selector::Achievements(_)
        | Selector::StandardAchievements
        | Selector::Junk
        | Selector::DeckTops
        | Selector::BaseDeck(_)
        | Selector::SetDeck(..)
        | Selector::BaseDeckTop(_)
        | Selector::Forecast(_)
        | Selector::Safe(_)
        | Selector::Museum(_) => false,
        Selector::Tops(_)
        | Selector::TopOf(..)
        | Selector::Board(..)
        | Selector::AvailableAchievements
        | Selector::InPlay
        | Selector::Owned(_)
        | Selector::Reg(_)
        | Selector::Slot(_) => true,
        Selector::Union(selectors) => selectors.iter().any(selector_may_use_icon),
        Selector::Filtered(inner, filters) => {
            selector_may_use_icon(inner)
                || filters.iter().any(|filter| match filter {
                    Filter::Has(test) | Filter::Lacks(test) => matches!(
                        test,
                        Test::ChosenIcon | Test::ShowsChosenIcon | Test::FeaturedChosenIcon
                    ),
                    Filter::Lowest | Filter::Highest => false,
                })
        }
    }
}
