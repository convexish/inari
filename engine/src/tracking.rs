//! Observer beliefs over physical cards, independent of logical-zone aliases.
//!
//! A slot's mask is local to a printed (set, age) roster, never to global IDs.
//! Decks are positional, but a shuffle hides positions until a card arrives at
//! a known one. Unordered hidden piles conservatively forget arrival order
//! among uncertain same-back cards; known members are retained until an
//! ambiguous departure. No internal continuation reference is evidence.
//!
//! Each live hidden reference is a [`Pick`] with its own candidate slots,
//! built only from what this observer saw. Wherever the observer cannot tell
//! cards apart, their candidates merge exactly as their identity masks do.

use std::sync::Arc;

use crate::{CardId, CardSet, Color, Deck, Error, Player, Position, Rng, Zone};

mod matching;

/// Every printed back: twelve ages in each of the six sets.
const BACKS: usize = CardSet::ALL.len() * Deck::AGES;
/// Hands, scores, achievements, the supply, the Age 0 reserve, ten board
/// piles, a deck per back, the junk, two Forecasts, two displays, two
/// Museum areas and two Safes.
const ZONES: usize = 18 + BACKS + 9;
/// Two private observers and the common/public observer.
pub(crate) const COMMON: usize = 2;
pub(crate) const OBSERVERS: usize = COMMON + 1;

pub(crate) fn zones() -> [Zone; ZONES] {
    std::array::from_fn(|i| match i {
        0..=1 => Zone::Hand(Player::ALL[i]),
        2..=3 => Zone::Score(Player::ALL[i - 2]),
        4..=5 => Zone::Achievements(Player::ALL[i - 4]),
        6 => Zone::AvailableAchievements,
        7 => Zone::AgeZeroReserve,
        8..=17 => Zone::Board(Player::ALL[(i - 8) / 5], Color::ALL[(i - 8) % 5]),
        _ if i < 18 + BACKS => Zone::Deck(Deck {
            set: CardSet::ALL[(i - 18) / Deck::AGES],
            age: ((i - 18) % Deck::AGES) as u8,
        }),
        _ if i == 18 + BACKS => Zone::Junk,
        _ if i < 21 + BACKS => Zone::Forecast(Player::ALL[i - 19 - BACKS]),
        _ if i < 23 + BACKS => Zone::Display(Player::ALL[i - 21 - BACKS]),
        _ if i < 25 + BACKS => Zone::Museum(Player::ALL[i - 23 - BACKS]),
        _ => Zone::Safe(Player::ALL[i - 25 - BACKS]),
    })
}

fn zone_index(zone: Zone) -> usize {
    match zone {
        Zone::Hand(p) => p.index(),
        Zone::Score(p) => 2 + p.index(),
        Zone::Achievements(p) => 4 + p.index(),
        Zone::AvailableAchievements => 6,
        Zone::AgeZeroReserve => 7,
        Zone::Board(p, c) => 8 + p.index() * 5 + c as usize,
        Zone::Deck(d) => 18 + back_index(d),
        Zone::Junk => 18 + BACKS,
        Zone::Forecast(p) => 19 + BACKS + p.index(),
        Zone::Display(p) => 21 + BACKS + p.index(),
        Zone::Museum(p) => 23 + BACKS + p.index(),
        Zone::Safe(p) => 25 + BACKS + p.index(),
    }
}

fn back_index(back: Deck) -> usize {
    back.index()
}

/// Hands, scores, achievements, the supply, the junk, Forecasts, displays and
/// Museums forget arrival order. Decks, board piles and Safes keep it.
/// A zone a player holds, which a subgame sets aside; the rest is supply the
/// subgame shares with its parent.
pub(crate) fn player_zone(zone: Zone) -> bool {
    !matches!(
        zone,
        Zone::AvailableAchievements | Zone::AgeZeroReserve | Zone::Deck(_) | Zone::Junk
    )
}

pub(crate) fn unordered(zone: Zone) -> bool {
    !matches!(
        zone,
        Zone::Deck(_) | Zone::Board(..) | Zone::AgeZeroReserve | Zone::Safe(_)
    )
}

/// Live picks at once (one byte of evidence holds a pick). The engine
/// reclaims unreferenced picks between steps.
pub(crate) const PICKS: usize = 63;

/// One card an instruction named, followed by each observer through its own
/// candidate slots rather than through the real card. References copied from
/// one another share their pick; separate picks may still name one card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Pick(u8);

impl Pick {
    pub(crate) fn new(index: u8) -> Self {
        debug_assert!(usize::from(index) < PICKS);
        Self(index)
    }

    pub(crate) fn index(self) -> usize {
        usize::from(self.0)
    }

    fn bit(self) -> u64 {
        1 << self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    back: Deck,
    mask: u16,
    /// Picks whose card this may be, as far as the viewer can tell.
    picks: u64,
}

/// One hidden world: the sampled card at each real position, each live
/// pick's sampled card, and the sampling observer's beliefs re-aligned with
/// the sampled positions.
pub(crate) struct Sample {
    /// The sampled card at each real card's physical position.
    pub(crate) cards: Vec<CardId>,
    /// Each live pick's card: one of its candidate slots, chosen uniformly.
    pub(crate) picks: [Option<CardId>; PICKS],
    pub(crate) tracker: Tracker,
}

/// Observer-safe candidate support. Slots in unordered piles have canonical
/// (back, candidates) order, NOT physical indices or identity-dependent IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeliefSlot {
    pub back: Deck,
    pub candidates: Vec<CardId>,
}

impl BeliefSlot {
    pub fn known(&self) -> Option<CardId> {
        (self.candidates.len() == 1).then(|| self.candidates[0])
    }
}

/// Sound subset beliefs for one player or the public observer. Obtain it from
/// `Game::tracker` or `Game::public_tracker`; only the engine mutates it.
#[derive(Clone, Debug)]
pub struct Tracker {
    viewer: Option<Player>,
    rosters: Arc<[Vec<CardId>; BACKS]>,
    slots: [Vec<Slot>; ZONES],
    /// In a subgame's tracker, the slots of the cards its suspended parents
    /// hold: in no zone here, but every card has a slot, so each back's
    /// all-different matching over its roster stays exact. Never sampled or
    /// refreshed; they keep the candidates they had when set aside.
    aside: Vec<Slot>,
    /// Identities an observer knows a retained pick excluded when selected.
    /// A pick has one printed back, so each mask uses that back's roster.
    /// Unlike slot membership, this survives shuffles and ambiguous moves.
    pick_exclusions: [u16; PICKS],
    dirty: u128,
    /// The last `close` changed nothing and nothing has changed since, so
    /// with no dirty back another `close` would change nothing either.
    closed: bool,
    /// The position generation and visibility of this tracker's last refresh
    /// and close through [`Knowledge::refresh`]. Meaningful only while
    /// `closed`: any change since clears that.
    seen: Option<(u64, u16)>,
}

/// Equality ignores `closed` and `seen`, which only record skippable work.
impl PartialEq for Tracker {
    fn eq(&self, other: &Self) -> bool {
        self.viewer == other.viewer
            && self.rosters == other.rosters
            && self.slots == other.slots
            && self.aside == other.aside
            && self.pick_exclusions == other.pick_exclusions
            && self.dirty == other.dirty
    }
}

impl Eq for Tracker {}

impl Tracker {
    /// `None` denotes common knowledge, with no player's private inspection.
    pub fn viewer(&self) -> Option<Player> {
        self.viewer
    }

    fn can_view(&self, position: &Position, zone: Zone) -> bool {
        match self.viewer {
            Some(viewer) => position.can_view(zone, viewer),
            None => Player::ALL.into_iter().all(|p| position.can_view(zone, p)),
        }
    }

    pub fn belief(&self, zone: Zone) -> Vec<BeliefSlot> {
        let mut slots = self.slots[zone_index(zone)].clone();
        if unordered(zone) {
            slots.sort_by_key(|s| (back_index(s.back), s.mask));
        }
        slots
            .into_iter()
            .map(|s| BeliefSlot {
                back: s.back,
                candidates: self.rosters[back_index(s.back)]
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| s.mask & (1 << i) != 0)
                    .map(|(_, &card)| card)
                    .collect(),
            })
            .collect()
    }

    /// `belief` without materializing candidates: each slot's back and
    /// candidate mask over `roster(back)`, in `belief`'s order.
    pub(crate) fn belief_masks(&self, zone: Zone) -> Vec<(Deck, u16)> {
        let mut slots: Vec<(Deck, u16)> = self.slots[zone_index(zone)]
            .iter()
            .map(|s| (s.back, s.mask))
            .collect();
        if unordered(zone) {
            slots.sort_by_key(|&(back, mask)| (back_index(back), mask));
        }
        slots
    }

    pub(crate) fn roster(&self, back: Deck) -> &[CardId] {
        &self.rosters[back_index(back)]
    }

    pub(crate) fn from_position(
        position: &Position,
        viewer: Option<Player>,
    ) -> Result<Self, Error> {
        let mut rosters: [Vec<CardId>; BACKS] = std::array::from_fn(|_| Vec::new());
        for (i, card) in position.catalog.iter().enumerate() {
            rosters[back_index(card.origin_deck())].push(CardId(i as u16));
        }
        if rosters.iter().any(|r| r.len() > 15) {
            return Err(Error::Unsupported(
                "tracking supports at most 15 cards per printed back",
            ));
        }
        let mut tracker = Self {
            viewer,
            rosters: Arc::new(rosters),
            slots: std::array::from_fn(|_| Vec::new()),
            aside: Vec::new(),
            pick_exclusions: [0; PICKS],
            dirty: (1 << BACKS) - 1,
            closed: false,
            seen: None,
        };
        for zone in zones() {
            tracker.slots[zone_index(zone)] = position
                .cards(zone)
                .iter()
                .map(|&card| {
                    let back = position.card(card).origin_deck();
                    Slot {
                        back,
                        mask: (1 << tracker.rosters[back_index(back)].len()) - 1,
                        picks: 0,
                    }
                })
                .collect();
        }
        tracker.refresh(position);
        tracker.close()?;
        Ok(tracker)
    }

    /// Common history plus this player's current sampled inspection rights.
    /// Never copy the real opponent's private observations into a world.
    pub(crate) fn for_player(&self, position: &Position, viewer: Player) -> Result<Self, Error> {
        debug_assert!(self.viewer.is_none());
        let mut tracker = self.clone();
        tracker.viewer = Some(viewer);
        tracker.seen = None;
        tracker.refresh(position);
        tracker.close()?;
        Ok(tracker)
    }

    /// Align common constraints with a sampled world's physical storage.
    /// Ordered slots keep their positions. In unordered piles, known members
    /// follow their identities; uncertain same-back slots share support and
    /// picks, so their real-world physical indices cannot leak into a sample.
    pub(crate) fn in_world(&self, position: &Position) -> Result<Self, Error> {
        debug_assert!(self.viewer.is_none());
        let mut tracker = self.clone();
        tracker.closed = false;
        tracker.seen = None;
        for zone in zones().into_iter().filter(|&zone| unordered(zone)) {
            let original = &self.slots[zone_index(zone)];
            let mut masks = [0; BACKS];
            let mut picks = [0; BACKS];
            let mut remaining = [0usize; BACKS];
            for slot in original.iter().filter(|s| s.mask.count_ones() > 1) {
                let back = back_index(slot.back);
                masks[back] |= slot.mask;
                picks[back] |= slot.picks;
                remaining[back] += 1;
            }
            let mut aligned = Vec::with_capacity(original.len());
            for &card in position.cards(zone) {
                let (back, bit) = self.bit(position, card);
                if let Some(slot) = original.iter().find(|s| s.back == back && s.mask == bit) {
                    aligned.push(*slot);
                } else {
                    let index = back_index(back);
                    if remaining[index] == 0 || masks[index] & bit == 0 {
                        return Err(Error::InvalidSetup("sample contradicts public history"));
                    }
                    remaining[index] -= 1;
                    aligned.push(Slot {
                        back,
                        mask: masks[index],
                        picks: picks[index],
                    });
                }
            }
            if remaining.iter().any(|&count| count != 0) || aligned.len() != original.len() {
                return Err(Error::InvalidSetup("sample contradicts public history"));
            }
            tracker.slots[zone_index(zone)] = aligned;
        }
        if !tracker.check_truth(position) {
            return Err(Error::InvalidSetup("sample contradicts public history"));
        }
        Ok(tracker)
    }

    fn bit(&self, position: &Position, card: CardId) -> (Deck, u16) {
        let back = position.card(card).origin_deck();
        let i = self.rosters[back_index(back)]
            .iter()
            .position(|&c| c == card)
            .expect("roster card");
        (back, 1 << i)
    }

    /// Check every true identity, all-different feasibility, and zone shape.
    /// This diagnostic consumes omniscient state; it is not an observation.
    pub fn check_truth(&self, position: &Position) -> bool {
        if !position.check_conservation() {
            return false;
        }
        for zone in zones() {
            let slots = &self.slots[zone_index(zone)];
            let cards = position.cards(zone);
            if slots.len() != cards.len() {
                return false;
            }
            for (slot, &card) in slots.iter().zip(cards) {
                let (back, bit) = self.bit(position, card);
                if slot.back != back || slot.mask & bit == 0 {
                    return false;
                }
            }
        }
        true // The actual assignment itself witnesses all-different feasibility.
    }

    /// Private identity and live-pick candidates must refine common support.
    /// Only picks still allocated by Knowledge constrain support.
    pub(crate) fn refines(&self, common: &Self, live: u64) -> bool {
        if self
            .pick_exclusions
            .iter()
            .zip(&common.pick_exclusions)
            .enumerate()
            .any(|(pick, (private, public))| live & (1 << pick) != 0 && public & !private != 0)
        {
            return false;
        }
        self.slots
            .iter()
            .zip(&common.slots)
            .chain([(&self.aside, &common.aside)])
            .all(|(private, public)| {
                private.len() == public.len()
                    && private.iter().zip(public).all(|(a, b)| {
                        a.back == b.back && a.mask & !b.mask == 0 && a.picks & live & !b.picks == 0
                    })
            })
    }

    pub(crate) fn knows(&self, position: &Position, card: CardId, zone: Zone) -> bool {
        let Some(i) = position.cards(zone).iter().position(|&c| c == card) else {
            return false;
        };
        self.slots[zone_index(zone)][i].mask.count_ones() == 1
    }

    pub(crate) fn reference_stable(&self, position: &Position, card: CardId) -> bool {
        let Some(zone) = position.locate(card) else {
            return position.cards(Zone::AgeZeroReserve).contains(&card);
        };
        if self.can_view(position, zone) {
            return true;
        }
        if !self.knows(position, card, zone) {
            return false;
        }
        !unordered(zone)
            || position
                .cards(zone)
                .iter()
                .filter(|&&c| position.card(c).origin_deck() == position.card(card).origin_deck())
                .count()
                == 1
    }

    /// Current inspection rights teach identities; losing permission does not
    /// erase them. Meiji is checked independently of hand/score membership.
    pub(crate) fn refresh(&mut self, position: &Position) {
        for zone in zones() {
            if !self.can_view(position, zone) && zone != Zone::AgeZeroReserve {
                continue;
            }
            for (i, &card) in position.cards(zone).iter().enumerate() {
                let (back, bit) = self.bit(position, card);
                let slot = &mut self.slots[zone_index(zone)][i];
                if slot.mask != bit {
                    slot.mask = bit;
                    self.dirty |= 1 << back_index(back);
                }
            }
        }
    }

    /// Reveal a physical card. In an unordered pile the viewer cannot identify
    /// its former anonymous slot: merge compatible slots, not its true index,
    /// and any pick on one of them may be the revealed card. `pick` is the
    /// reference the reveal named, if any: that card is the revealed one.
    pub(crate) fn reveal(
        &mut self,
        position: &Position,
        card: CardId,
        zone: Zone,
        pick: Option<Pick>,
    ) {
        let (back, bit) = self.bit(position, card);
        let i = position
            .cards(zone)
            .iter()
            .position(|&c| c == card)
            .expect("revealed card");
        let hidden = unordered(zone) && !self.can_view(position, zone);
        self.closed = false;
        let slots = &mut self.slots[zone_index(zone)];
        if hidden {
            // Nothing moves: every slot keeps its physical picks (a pick
            // bound to one slot stays that card, as the public tracker has
            // it). Only the other compatible identities narrow.
            let compatible = |s: &Slot| s.back == back && s.mask & bit != 0;
            let union = slots
                .iter()
                .filter(|s| compatible(s))
                .fold(0, |u, s| u | s.mask);
            for (j, slot) in slots.iter_mut().enumerate() {
                if j != i && compatible(slot) {
                    slot.mask = union & !bit;
                }
            }
        }
        slots[i].mask = bit;
        if let Some(pick) = pick {
            self.forget(pick);
            self.slots[zone_index(zone)][i].picks |= pick.bit();
        }
        self.dirty |= 1 << back_index(back);
        self.normalize(zone);
    }

    /// Prepare one departure before physical mutation. Non-top deck searches
    /// reveal membership, not an index; their following shuffle forgets order.
    /// An unseen departure from a hidden pile may be any same-back member, so
    /// their picks merge too. `pick` names the departing card: it follows it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract(
        &mut self,
        position: &Position,
        card: CardId,
        source: Zone,
        destination: Zone,
        public: bool,
        positional: bool,
        pick: Option<Pick>,
    ) -> Slot {
        let (back, bit) = self.bit(position, card);
        let i = position
            .cards(source)
            .iter()
            .position(|&c| c == card)
            .expect("moving card");
        let sees =
            |viewer| position.can_view(source, viewer) || position.can_view(destination, viewer);
        // A hand-to-hand transfer is public in two-player play: both players
        // see the identity, even though neither whole hand is publicly open.
        let visible = public
            || match self.viewer {
                Some(viewer) => sees(viewer),
                None => Player::ALL.into_iter().all(sees),
            };
        self.closed = false;
        if matches!(source, Zone::Deck(_)) && !positional {
            self.shuffle(source);
        }
        if visible {
            self.reveal(position, card, source, None);
        } else if unordered(source) {
            let slots = &mut self.slots[zone_index(source)];
            let (union, picks) = slots
                .iter()
                .filter(|s| s.back == back)
                .fold((0, 0), |(u, p), s| (u | s.mask, p | s.picks));
            for slot in slots.iter_mut().filter(|s| s.back == back) {
                slot.mask = union;
                slot.picks = picks;
            }
        }
        let mut slot = self.slots[zone_index(source)].remove(i);
        if visible {
            slot.mask = bit;
        }
        if let Some(pick) = pick {
            self.forget(pick);
            slot.picks |= pick.bit();
        }
        self.dirty |= 1 << back_index(back);
        self.normalize(source);
        slot
    }

    pub(crate) fn insert(&mut self, zone: Zone, slot: Slot, bottom: bool) {
        self.closed = false;
        let slots = &mut self.slots[zone_index(zone)];
        if bottom {
            slots.insert(0, slot);
        } else {
            slots.push(slot);
        }
        self.normalize(zone);
    }

    fn normalize(&mut self, zone: Zone) {
        if !unordered(zone) {
            return;
        }
        let slots = &mut self.slots[zone_index(zone)];
        let mut unions = [0; BACKS];
        for slot in slots.iter().filter(|s| s.mask.count_ones() > 1) {
            unions[back_index(slot.back)] |= slot.mask;
        }
        for slot in slots.iter_mut().filter(|s| s.mask.count_ones() > 1) {
            slot.mask = unions[back_index(slot.back)];
        }
    }

    pub(crate) fn shuffle(&mut self, zone: Zone) {
        self.closed = false;
        let slots = &mut self.slots[zone_index(zone)];
        let (union, picks) = slots
            .iter()
            .fold((0, 0), |(u, p), s| (u | s.mask, p | s.picks));
        for slot in slots {
            slot.mask = union;
            slot.picks = picks;
            self.dirty |= 1 << back_index(slot.back);
        }
    }

    /// A new pick of `card` where it lies: in a hidden unordered pile, any
    /// member with its back; elsewhere, its own position.
    pub(crate) fn mark(&mut self, position: &Position, card: CardId, pick: Pick) {
        let Some(zone) = position.locate(card) else {
            return;
        };
        let i = position
            .cards(zone)
            .iter()
            .position(|&c| c == card)
            .expect("picked card");
        let hidden = unordered(zone) && !self.can_view(position, zone);
        self.closed = false;
        let slots = &mut self.slots[zone_index(zone)];
        let back = slots[i].back;
        for (j, slot) in slots.iter_mut().enumerate() {
            if j == i || (hidden && slot.back == back) {
                slot.picks |= pick.bit();
            }
        }
    }

    fn forget(&mut self, pick: Pick) {
        // A move/reveal resets positional candidates, not selection evidence.
        for slot in self.slots.iter_mut().flatten() {
            slot.picks &= !pick.bit();
        }
    }

    /// Only call with an identity this observer's explicit selection evidence
    /// excludes. A private chooser's menu partition is not common knowledge.
    pub(crate) fn exclude_pick_identity(&mut self, position: &Position, pick: Pick, card: CardId) {
        let (_, bit) = self.bit(position, card);
        self.pick_exclusions[pick.index()] |= bit;
        self.closed = false;
    }

    /// Drop picks no reference holds any more.
    pub(crate) fn clear_picks(&mut self, dead: u64) {
        self.closed = false;
        for (pick, excluded) in self.pick_exclusions.iter_mut().enumerate() {
            if dead & (1 << pick) != 0 {
                *excluded = 0;
            }
        }
        for slot in self.slots.iter_mut().flatten().chain(&mut self.aside) {
            slot.picks &= !dead;
        }
    }

    /// Picks with at least one candidate slot.
    pub(crate) fn tracked_picks(&self) -> u64 {
        self.slots
            .iter()
            .flatten()
            .fold(0, |picks, slot| picks | slot.picks)
    }

    /// A public failure showed the pick's card has left `zone` (an attempt
    /// on it found it gone): none of that zone's slots can hold it any more.
    pub(crate) fn absent(&mut self, zone: Zone, pick: Pick) {
        self.closed = false;
        for slot in &mut self.slots[zone_index(zone)] {
            slot.picks &= !pick.bit();
        }
    }

    /// Make `card`'s own slot a candidate of `pick`, keeping the others.
    pub(crate) fn include(&mut self, position: &Position, card: CardId, pick: Pick) {
        if let Some(zone) = position.locate(card) {
            let i = position
                .cards(zone)
                .iter()
                .position(|&c| c == card)
                .expect("located card");
            self.slots[zone_index(zone)][i].picks |= pick.bit();
            self.closed = false;
        }
    }

    /// Diagnostic: the real card's slot is among the pick's candidates.
    /// Consumes omniscient state; it is not an observation.
    pub(crate) fn pick_holds(&self, position: &Position, card: CardId, pick: Pick) -> bool {
        let Some(zone) = position.locate(card) else {
            return true;
        };
        let i = position
            .cards(zone)
            .iter()
            .position(|&c| c == card)
            .expect("located card");
        let (_, bit) = self.bit(position, card);
        self.slots[zone_index(zone)][i].picks & pick.bit() != 0
            && self.pick_exclusions[pick.index()] & bit == 0
    }

    /// Whether two retained picks still have a common possible physical
    /// witness, using only this observer's candidate slots and identity domains.
    pub(crate) fn picks_may_coincide(&self, left: Pick, right: Pick) -> bool {
        self.slots.iter().flatten().any(|slot| {
            slot.picks & left.bit() != 0
                && slot.picks & right.bit() != 0
                && slot.mask
                    & !self.pick_exclusions[left.index()]
                    & !self.pick_exclusions[right.index()]
                    != 0
        })
    }

    /// Whether a retained pick may be this card, by this observer's candidate
    /// slots and identity domains (a card of another back never can).
    pub(crate) fn pick_may_be(&self, position: &Position, pick: Pick, card: CardId) -> bool {
        let (back, bit) = self.bit(position, card);
        self.pick_exclusions[pick.index()] & bit == 0
            && self.slots.iter().flatten().any(|slot| {
                slot.picks & pick.bit() != 0 && slot.back == back && slot.mask & bit != 0
            })
    }

    /// The pick's card, if this observer can name it: exactly one allowed
    /// identity across its candidate slots, even without a known position.
    pub(crate) fn pick_identity(&self, pick: Pick) -> Option<CardId> {
        let mut back = None;
        let mut identities = 0;
        for slot in self
            .slots
            .iter()
            .flatten()
            .filter(|slot| slot.picks & pick.bit() != 0)
        {
            debug_assert!(back.is_none_or(|back| back == slot.back));
            back = Some(slot.back);
            identities |= slot.mask & !self.pick_exclusions[pick.index()];
        }
        (identities.count_ones() == 1).then(|| {
            self.rosters[back_index(back.expect("candidate back"))]
                [identities.trailing_zeros() as usize]
        })
    }

    /// Explicit retained naming evidence identifies the pick even when its
    /// current physical position was lost to a shuffle or an ambiguous move.
    pub(crate) fn identify_pick(&mut self, position: &Position, pick: Pick, card: CardId) {
        let (_, bit) = self.bit(position, card);
        self.pick_exclusions[pick.index()] |= !bit;
        self.closed = false;
    }

    /// The zones a retained pick's card may occupy, by this observer's
    /// candidate slots and identity domain. The real card is always among them.
    pub(crate) fn pick_zones(&self, pick: Pick) -> Vec<Zone> {
        let excluded = self.pick_exclusions[pick.index()];
        zones()
            .into_iter()
            .zip(&self.slots)
            .filter(|(_, slots)| {
                slots
                    .iter()
                    .any(|slot| slot.picks & pick.bit() != 0 && slot.mask & !excluded != 0)
            })
            .map(|(zone, _)| zone)
            .collect()
    }

    /// The zones where this observer believes `card` may lie.
    pub(crate) fn card_zones(&self, position: &Position, card: CardId) -> Vec<Zone> {
        let (back, bit) = self.bit(position, card);
        zones()
            .into_iter()
            .zip(&self.slots)
            .filter(|(_, slots)| {
                slots
                    .iter()
                    .any(|slot| slot.back == back && slot.mask & bit != 0)
            })
            .map(|(zone, _)| zone)
            .collect()
    }

    /// A retained reference still names one place in this zone. This does not
    /// require knowing the card's front; shuffles erase this fact naturally.
    pub(crate) fn pick_position(&self, zone: Zone, pick: Pick) -> Option<usize> {
        let mut candidates = self.slots.iter().enumerate().flat_map(|(z, slots)| {
            slots
                .iter()
                .enumerate()
                .filter_map(move |(i, slot)| (slot.picks & pick.bit() != 0).then_some((z, i)))
        });
        let (z, i) = candidates.next()?;
        (z == zone_index(zone) && candidates.next().is_none()).then_some(i)
    }

    pub(crate) fn introduce_age_zero(&mut self) {
        self.closed = false;
        let reserve = std::mem::take(&mut self.slots[zone_index(Zone::AgeZeroReserve)]);
        self.slots[zone_index(Zone::Deck(Deck::base(0)))].extend(reserve);
        self.shuffle(Zone::Deck(Deck::base(0)));
    }

    pub(crate) fn search(&mut self, position: &Position, deck: Deck, code: &str, found: bool) {
        let Some(i) = position.catalog.iter().position(|c| c.code == code) else {
            return;
        };
        let (back, bit) = self.bit(position, CardId(i as u16));
        self.closed = false;
        for zone in zones() {
            if (zone == Zone::Deck(deck)) == found {
                continue;
            }
            for slot in &mut self.slots[zone_index(zone)] {
                if slot.back == back {
                    slot.mask &= !bit;
                }
            }
        }
        self.dirty |= 1 << back_index(back);
    }

    /// This observer's beliefs for a subgame on the same supply: the supply
    /// slots keep their candidates (deductions already made about the decks
    /// survive) and the parent players' zones are set aside. Sound: every
    /// true card stays among its slot's candidates. Pick support is dropped:
    /// a pick's exclusions constrain its one remaining slot, which would
    /// misidentify a supply card once the pick's other slots are set aside.
    /// A parent reference into the supply therefore has no pick after the
    /// merge, and a search at that moment is refused rather than misled.
    pub(crate) fn fork_for_subgame(&self, child: &Position) -> Result<Self, Error> {
        // Every physical card keeps a slot, so each back's all-different
        // matching stays exact; the set-aside cards' slots just have no zone.
        let mut tracker = self.clone();
        for zone in zones().into_iter().filter(|&zone| player_zone(zone)) {
            let slots = std::mem::take(&mut tracker.slots[zone_index(zone)]);
            tracker.aside.extend(slots);
        }
        tracker.clear_picks(u64::MAX);
        tracker.invalidate();
        tracker.refresh(child);
        tracker.close()?;
        Ok(tracker)
    }

    /// Take back the supply as a finished subgame left it. The parent's own
    /// player slots are untouched; the caller moves the subgame's player
    /// cards one by one ([`Self::take_slot`], [`Self::insert`]).
    pub(crate) fn adopt_supply(&mut self, child: &Self) {
        for zone in zones().into_iter().filter(|&zone| !player_zone(zone)) {
            self.slots[zone_index(zone)].clone_from(&child.slots[zone_index(zone)]);
        }
        self.invalidate();
    }

    /// The slot of this zone's first card, leaving the zone's remaining slots
    /// aligned with its remaining cards.
    pub(crate) fn take_slot(&mut self, zone: Zone) -> Slot {
        self.slots[zone_index(zone)].remove(0)
    }

    /// A slot by zone index, `ZONES` naming the set-aside list.
    fn slot(&self, zone: usize, index: usize) -> &Slot {
        if zone == ZONES {
            &self.aside[index]
        } else {
            &self.slots[zone][index]
        }
    }

    fn slot_mut(&mut self, zone: usize, index: usize) -> &mut Slot {
        if zone == ZONES {
            &mut self.aside[index]
        } else {
            &mut self.slots[zone][index]
        }
    }

    /// Every back needs a refresh and a close.
    pub(crate) fn invalidate(&mut self) {
        self.dirty = (1 << BACKS) - 1;
        self.closed = false;
        self.seen = None;
    }

    pub(crate) fn close(&mut self) -> Result<(), Error> {
        if self.closed && self.dirty == 0 {
            debug_assert!(
                {
                    let mut full = self.clone();
                    full.closed = false;
                    full.close().is_ok() && full == *self
                },
                "a skipped close must change nothing"
            );
            return Ok(());
        }
        // Only a close that changes nothing reaches a fixed point: a change
        // can enable more pick restrictions on the next pass.
        self.closed = false;
        let mut changed = false;
        // A retained pick with only one possible slot constrains that slot's
        // identity. Normalize may have broadened it after a later arrival;
        // restoring this observer-local domain never identifies a hidden
        // physical position that the observer did not already distinguish.
        for pick in 0..PICKS {
            let excluded = self.pick_exclusions[pick];
            if excluded == 0 {
                continue;
            }
            let mut candidates = self.slots.iter().enumerate().flat_map(|(z, slots)| {
                slots.iter().enumerate().filter_map(move |(i, slot)| {
                    (slot.picks & (1 << pick) != 0 && slot.mask & !excluded != 0).then_some((z, i))
                })
            });
            let Some((zone, index)) = candidates.next() else {
                continue;
            };
            if candidates.next().is_some() {
                continue;
            }
            let slot = &mut self.slots[zone][index];
            if slot.mask & excluded != 0 {
                slot.mask &= !excluded;
                self.dirty |= 1 << back_index(slot.back);
                changed = true;
            }
        }
        while self.dirty != 0 {
            let back = self.dirty.trailing_zeros() as usize;
            self.dirty &= !(1 << back);
            if self.rosters[back].is_empty() {
                continue;
            }
            // Zone `ZONES` is the set-aside list: part of the matching, like
            // any other slot of the back.
            let locations: Vec<_> = self
                .slots
                .iter()
                .chain([&self.aside])
                .enumerate()
                .flat_map(|(z, slots)| {
                    slots
                        .iter()
                        .enumerate()
                        .filter(move |(_, s)| back_index(s.back) == back)
                        .map(move |(i, _)| (z, i))
                })
                .collect();
            let mut masks: Vec<_> = locations
                .iter()
                .map(|&(z, i)| self.slot(z, i).mask)
                .collect();
            matching::close(&mut masks)?;
            for ((z, i), mask) in locations.into_iter().zip(masks) {
                let slot = self.slot_mut(z, i);
                changed |= slot.mask != mask;
                slot.mask = mask;
            }
        }
        self.closed = !changed;
        Ok(())
    }

    /// The sampled card at each physical position, and a card for each live
    /// pick. Uses no true front to choose candidates; truth only locates
    /// positions after sampling. Known positions are fixed by singleton
    /// constraints. Hidden unordered slots are ordered by (support, picks),
    /// never by physical index, so worlds this observer cannot tell apart
    /// sample identically from one seed.
    pub(crate) fn sample(
        &self,
        position: &Position,
        live: u64,
        distinct: &[u64; PICKS],
        pinned: &[Option<CardId>; PICKS],
        rng: &mut Rng,
    ) -> Result<Sample, Error> {
        let mut cards: Vec<_> = (0..position.catalog.len())
            .map(|i| CardId(i as u16))
            .collect();
        let mut tracker = self.clone();
        tracker.closed = false;
        tracker.seen = None;
        let mut candidates: [Vec<CardId>; PICKS] = std::array::from_fn(|_| Vec::new());
        // One pass buckets every back's slots in zone order. Hidden unordered
        // runs are then ordered canonically, never by physical index.
        let mut buckets: Vec<Vec<(Zone, usize, Slot)>> = vec![Vec::new(); BACKS];
        for zone in zones() {
            for (i, slot) in self.slots[zone_index(zone)].iter().enumerate() {
                buckets[back_index(slot.back)].push((zone, i, *slot));
            }
        }
        for (back, locations) in buckets.iter_mut().enumerate() {
            if locations.is_empty() {
                continue;
            }
            for run in locations.chunk_by_mut(|a, b| a.0 == b.0) {
                let zone = run[0].0;
                if unordered(zone) && !self.can_view(position, zone) {
                    run.sort_by_key(|(_, _, s)| (s.mask, s.picks));
                }
            }
            // Set-aside slots (a subgame) complete the matching after the
            // placed slots; `assignment` is zipped with `locations`, so they
            // take no position and, holding no picks, name no candidate.
            let mut masks: Vec<_> = locations.iter().map(|&(_, _, slot)| slot.mask).collect();
            masks.extend(
                self.aside
                    .iter()
                    .filter(|slot| back_index(slot.back) == back)
                    .map(|slot| slot.mask),
            );
            let present = locations
                .iter()
                .fold(0, |bits, (_, _, slot)| bits | slot.picks)
                & live;
            let constrained = (0..PICKS).fold(0, |bits, pick| {
                bits | if self.pick_exclusions[pick] != 0 {
                    present & (1 << pick)
                } else {
                    0
                }
            });
            let assignment = if constrained == 0 {
                // Preserve sampling order and RNG consumption when no retained
                // selection carries an observer-local identity exclusion.
                matching::sample(&masks, rng)?
            } else {
                // A physical world must leave every restricted pick at least
                // one allowed candidate, and allow its existing all-different
                // cohort to assign distinct identities. This is conditioning
                // on this observer's domains, never on actual hidden IDs.
                let active = (0..PICKS)
                    .filter(|pick| constrained & (1 << pick) != 0)
                    .fold(constrained, |bits, pick| bits | distinct[pick] & present);
                let members: Vec<_> = (0..PICKS)
                    .filter(|pick| active & (1 << pick) != 0)
                    .collect();
                let requirements: Vec<_> = members
                    .iter()
                    .map(|&pick| {
                        let rows = locations.iter().enumerate().fold(
                            0u16,
                            |rows, (index, (_, _, slot))| {
                                rows | if slot.picks & (1 << pick) != 0 {
                                    1 << index
                                } else {
                                    0
                                }
                            },
                        );
                        let mut allowed =
                            ((1u16 << self.rosters[back].len()) - 1) & !self.pick_exclusions[pick];
                        if let Some(card) = pinned[pick] {
                            allowed &= self.rosters[back]
                                .iter()
                                .position(|&candidate| candidate == card)
                                .map_or(0, |index| 1 << index);
                        }
                        (rows, allowed)
                    })
                    .collect();
                let mut remaining = active;
                let mut groups = Vec::new();
                while remaining != 0 {
                    let first = remaining.trailing_zeros() as usize;
                    let group = (distinct[first] | (1 << first)) & active;
                    remaining &= !group;
                    let group: Vec<_> = members
                        .iter()
                        .enumerate()
                        .filter_map(|(index, pick)| (group & (1 << pick) != 0).then_some(index))
                        .collect();
                    if group.len() > 1 {
                        groups.push(group);
                    }
                }
                matching::sample_constrained(&masks, &requirements, &groups, rng)?
            };
            let assigned: Vec<_> = locations.iter().copied().zip(assignment).collect();
            for &((_, _, slot), identity) in &assigned {
                let mut picks = slot.picks & live;
                while picks != 0 {
                    let pick = picks.trailing_zeros() as usize;
                    picks &= picks - 1;
                    if self.pick_exclusions[pick] & (1 << identity) == 0 {
                        candidates[pick].push(self.rosters[back][identity]);
                    }
                }
            }
            for run in assigned.chunk_by(|a, b| a.0.0 == b.0.0) {
                let zone = run[0].0.0;
                let mut local = run.to_vec();
                let mut indices: Vec<_> = local.iter().map(|((_, i, _), _)| *i).collect();
                indices.sort_unstable();
                // Remembered membership is NOT a labelled physical slot. An
                // unknown position in a hidden unordered hand must vary even
                // when all of its members are individually remembered.
                if unordered(zone) && !self.can_view(position, zone) {
                    matching::shuffle(&mut local, rng);
                }
                for (index, ((_, _, slot), identity)) in indices.into_iter().zip(local) {
                    cards[usize::from(position.cards(zone)[index].0)] =
                        self.rosters[back][identity];
                    tracker.slots[zone_index(zone)][index] = slot;
                }
            }
        }
        let mut picks = [None; PICKS];
        for (pick, candidates) in candidates.iter().enumerate() {
            if !candidates.is_empty() {
                let i = (rng.next_u64() % candidates.len() as u64) as usize;
                picks[pick] = Some(candidates[i]);
            }
        }
        // Independent picks retain their existing sampling behavior. A group
        // adds one disjoint clique of all-different picks; reclaiming members
        // leaves a smaller clique. Sample its references uniformly using the
        // same exact matching solver as physical slots, one back at a time.
        // Dummy unrestricted rows extend each partial assignment equally, so
        // they do not bias the cohort toward any original physical position.
        let mut remaining = live;
        while remaining != 0 {
            let first = remaining.trailing_zeros() as usize;
            remaining &= !(1 << first);
            let group = distinct[first] & live;
            if group == 0 {
                continue;
            }
            let group = group | (1 << first);
            remaining &= !group;
            let members: Vec<_> = (0..PICKS)
                .filter(|&pick| group & (1 << pick) != 0)
                .collect();
            if members
                .iter()
                .any(|&pick| distinct[pick] & live != group & !(1 << pick))
            {
                return Err(Error::Unsupported("overlapping distinct retained cohorts"));
            }
            for &pick in &members {
                if let Some(card) = pinned[pick] {
                    candidates[pick] = vec![card];
                }
            }
            for roster in self.rosters.iter() {
                let same_back: Vec<_> = members
                    .iter()
                    .copied()
                    .filter(|&pick| {
                        candidates[pick]
                            .first()
                            .is_some_and(|card| roster.contains(card))
                    })
                    .collect();
                if same_back.len() < 2 {
                    continue;
                }
                let mut masks: Vec<_> = same_back
                    .iter()
                    .map(|&pick| {
                        roster.iter().enumerate().fold(0u16, |mask, (index, card)| {
                            mask | (u16::from(candidates[pick].contains(card)) << index)
                        })
                    })
                    .collect();
                masks.resize(roster.len(), (1u16 << roster.len()) - 1);
                let assignment = matching::sample(&masks, rng)?;
                for (&pick, identity) in same_back.iter().zip(assignment) {
                    picks[pick] = Some(roster[identity]);
                }
            }
        }
        Ok(Sample {
            cards,
            picks,
            tracker,
        })
    }
}

/// Private and common histories, optional so rules-only play keeps cheap clones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Knowledge {
    pub(crate) trackers: [Tracker; OBSERVERS],
    /// A public named search identifies the subsequent transfer, but not the
    /// card's former deck index. Cleared when that transfer commits.
    pub(crate) named: Option<(Deck, CardId)>,
    /// Picks in use; bit `p` is [`Pick`] `p`.
    pub(crate) picks: u64,
    /// Explicit without-replacement cohorts. Pick identities are public
    /// relationships; no hidden card identity or position is stored here.
    pub(crate) distinct: [u64; PICKS],
    /// The pick naming the card the next physical move takes, if a
    /// reference named it. Consumed by that move.
    pub(crate) moving: Option<Pick>,
    /// The latest physically moved card and its pick, for its result.
    pub(crate) moved: Option<(CardId, Pick)>,
}

impl Knowledge {
    pub(crate) fn new(position: &Position) -> Result<Self, Error> {
        Ok(Self {
            trackers: [
                Tracker::from_position(position, Some(Player::One))?,
                Tracker::from_position(position, Some(Player::Two))?,
                Tracker::from_position(position, None)?,
            ],
            named: None,
            picks: 0,
            distinct: [0; PICKS],
            moving: None,
            moved: None,
        })
    }

    /// Refresh and close every observer's beliefs against the current position.
    /// A tracker unchanged since its last refresh and close here, against the
    /// same cards and visibility, would change nothing and is skipped.
    pub(crate) fn refresh(&mut self, position: &Position) -> Result<(), Error> {
        let seen = Some((position.generation(), position.visibility()));
        for tracker in &mut self.trackers {
            if tracker.seen == seen && tracker.closed && tracker.dirty == 0 {
                debug_assert!(
                    {
                        let mut full = tracker.clone();
                        full.refresh(position);
                        full.close().is_ok() && full == *tracker
                    },
                    "a skipped refresh must change nothing"
                );
            } else {
                tracker.refresh(position);
                tracker.close()?;
                tracker.seen = seen;
            }
            debug_assert!(tracker.check_truth(position));
        }
        debug_assert!(
            self.trackers[..COMMON]
                .iter()
                .all(|tracker| tracker.refines(&self.trackers[COMMON], self.picks)),
            "private beliefs must refine public knowledge"
        );
        Ok(())
    }

    /// Beliefs for a subgame on this position's supply ([`Tracker::fork_for_subgame`]).
    /// The parent's picks stay allocated so the child never reuses their
    /// bits; its own references start with none.
    pub(crate) fn fork_for_subgame(&self, child: &Position) -> Result<Self, Error> {
        Ok(Self {
            trackers: [
                self.trackers[0].fork_for_subgame(child)?,
                self.trackers[1].fork_for_subgame(child)?,
                self.trackers[2].fork_for_subgame(child)?,
            ],
            named: None,
            picks: self.picks,
            distinct: [0; PICKS],
            moving: None,
            moved: None,
        })
    }

    /// A fresh pick, if one is free. Without one the reference stays
    /// untracked and sampling refuses it rather than pin its card.
    pub(crate) fn allocate(&mut self) -> Option<Pick> {
        let free = !self.picks & ((1 << PICKS) - 1);
        if free == 0 {
            return None;
        }
        let pick = free.trailing_zeros() as u8;
        self.picks |= 1 << pick;
        Some(Pick(pick))
    }

    /// Keep only the `live` picks.
    pub(crate) fn retain_picks(&mut self, live: u64) {
        let dead = self.picks & !live;
        if dead != 0 {
            for tracker in &mut self.trackers {
                tracker.clear_picks(dead);
            }
        }
        self.picks &= live;
        for (pick, peers) in self.distinct.iter_mut().enumerate() {
            *peers = if live & (1 << pick) == 0 {
                0
            } else {
                *peers & live
            };
        }
    }
}
