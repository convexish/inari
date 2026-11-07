//! Knowledge belongs to a retained reference, not to the card's current slot.
//! These wrappers are interpreter-only; public outcomes remain plain rule data.
use super::*;
use crate::tracking::Pick;
use std::ops::{BitOr, BitOrAssign, Deref};

/// What each observer knows about one retained reference: whether it can
/// name the card (sampling keeps that identity), and the reference's
/// [`Pick`]. A reference an observer cannot name samples through its pick:
/// the candidate places that observer's tracker kept for it, never the real
/// card's place. References copied from one another carry one pick;
/// references from separate picks are separate even when they name one
/// physical card.
///
/// Bits 0-1 are "known to" Player One/Two, bits 2-7 the pick plus one
/// (zero is none). Bit 8 retains an explicitly selected deck position until
/// that original card moves, including through a replacement's `the card`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Evidence(u16);

impl Evidence {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const PUBLIC: Self = Self(0b11);
    const KNOWN: u16 = 0b11;
    const POSITIONAL: u16 = 1 << 8;

    pub(crate) const fn known_to(viewer: Player) -> Self {
        Self(1 << viewer.index())
    }

    pub(crate) fn known(self, viewer: Player) -> bool {
        self.0 & Self::known_to(viewer).0 != 0
    }

    pub(crate) fn pick(self) -> Option<Pick> {
        ((self.0 as u8) >> 2).checked_sub(1).map(Pick::new)
    }

    pub(crate) fn with_pick(self, pick: Option<Pick>) -> Self {
        let pick = pick.map_or(0, |pick| pick.index() as u16 + 1);
        Self(self.0 & (Self::KNOWN | Self::POSITIONAL) | pick << 2)
    }

    pub(crate) fn positional(self) -> bool {
        self.0 & Self::POSITIONAL != 0
    }

    pub(crate) fn at_position(self) -> Self {
        Self(self.0 | Self::POSITIONAL)
    }

    pub(crate) fn after_move(self) -> Self {
        Self(self.0 & !Self::POSITIONAL)
    }

    /// A pick from `zone`, or a card that just arrived there: viewers who can
    /// inspect the zone know the card; the others follow its pick.
    pub(crate) fn selection(position: &Position, zone: Zone) -> Self {
        Player::ALL
            .into_iter()
            .filter(|&viewer| position.can_view(zone, viewer))
            .fold(Self::NONE, |evidence, viewer| {
                evidence | Self::known_to(viewer)
            })
    }
}

/// Knowledge unions; the left operand's pick wins, the right one's fills in.
impl BitOr for Evidence {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        let flags = Self::KNOWN | Self::POSITIONAL;
        Self(self.0 & flags | other.0 & flags).with_pick(self.pick().or(other.pick()))
    }
}

impl BitOrAssign for Evidence {
    fn bitor_assign(&mut self, other: Self) {
        *self = *self | other;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Captured<T> {
    pub(super) inner: T,
    pub(super) evidence: Evidence,
}

impl<T> Captured<T> {
    pub(super) const fn new(value: T, evidence: Evidence) -> Self {
        Self {
            inner: value,
            evidence,
        }
    }

    pub(super) fn with<U>(self, value: U) -> Captured<U> {
        Captured::new(value, self.evidence)
    }
}

impl<T> Deref for Captured<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

pub(super) type HeldCard = Captured<CardId>;
pub(super) type HeldOutcome = Captured<OperationOutcome>;
pub(super) type HeldBulk = Captured<BulkOutcome>;
pub(super) type HeldReturn = Captured<ReturnValue>;

impl HeldReturn {
    pub(super) const UNIT: Self = Self::new(ReturnValue::Unit, Evidence::NONE);

    pub(super) fn operation(self) -> Option<HeldOutcome> {
        self.inner.operation().map(|outcome| self.with(outcome))
    }

    pub(super) fn card(self) -> Option<HeldCard> {
        match self.inner {
            ReturnValue::Operation(outcome) => outcome.card,
            ReturnValue::Bulk(outcome) => outcome.card,
            _ => None,
        }
        .map(|card| self.with(card))
    }
}

impl HeldOutcome {
    pub(super) fn returned(self) -> HeldReturn {
        self.with(ReturnValue::Operation(self.inner))
    }

    pub(super) fn card_ref(self) -> Option<HeldCard> {
        self.card.map(|card| self.with(card))
    }
}

impl Default for HeldBulk {
    fn default() -> Self {
        Self::new(BulkOutcome::default(), Evidence::NONE)
    }
}

impl HeldBulk {
    pub(super) fn record(&mut self, disposition: Disposition) {
        self.inner.record(disposition);
    }

    pub(super) fn record_original(&mut self, outcome: HeldOutcome) {
        self.inner.record_original(outcome.inner);
        if outcome.disposition == Disposition::Performed {
            self.evidence = outcome.evidence;
        }
    }

    pub(super) fn returned(self) -> HeldReturn {
        self.with(ReturnValue::Bulk(self.inner))
    }
}

impl From<ReturnValue> for HeldReturn {
    fn from(value: ReturnValue) -> Self {
        // Non-card outcomes have no provenance. Card-bearing results MUST be
        // captured at selection/commit or forwarded from the child that made
        // them, never reconstructed from their current hidden location.
        assert!(
            match value {
                ReturnValue::Operation(outcome) => outcome.card.is_none(),
                ReturnValue::Bulk(outcome) => outcome.card.is_none(),
                _ => true,
            },
            "card-bearing result needs explicit reference provenance"
        );
        Self::new(value, Evidence::NONE)
    }
}

/// The retained reference naming this card, if the list has one.
pub(super) fn find_card(cards: &[HeldCard], card: CardId) -> Option<HeldCard> {
    cards.iter().find(|held| held.inner == card).copied()
}

pub(super) fn contains_card(cards: &[HeldCard], card: CardId) -> bool {
    cards.iter().any(|held| held.inner == card)
}

impl Game {
    pub(super) fn rules_frames(&self) -> Vec<Frame> {
        let mut frames = self.frames.clone();
        tracking::remap::Remap::rules(&self.position).frames(&mut frames);
        frames
    }

    /// What each observer knows of a pick of this card from where it lies,
    /// without a pick: for a reference that adopts an existing one's pick.
    pub(super) fn selection_evidence(&self, card: CardId) -> Evidence {
        self.position.locate(card).map_or(Evidence::NONE, |zone| {
            Evidence::selection(&self.position, zone)
        })
    }

    /// Test adapter: a reference without a pick, for untracked fixtures.
    #[cfg(test)]
    pub(super) fn held(&self, card: CardId) -> HeldCard {
        Captured::new(card, self.selection_evidence(card))
    }

    /// A new reference picked from the card's current zone. An observer who
    /// cannot name it follows it through a fresh pick: candidates for where
    /// the picked card lies, as far as that observer can tell.
    pub(super) fn capture(&mut self, card: CardId) -> HeldCard {
        let evidence = self.selection_evidence(card);
        let pick = if evidence == Evidence::PUBLIC {
            None
        } else {
            self.knowledge.as_mut().and_then(|knowledge| {
                let pick = knowledge.allocate()?;
                for tracker in &mut knowledge.trackers {
                    tracker.mark(&self.position, card, pick);
                }
                Some(pick)
            })
        };
        Captured::new(card, evidence.with_pick(pick))
    }

    /// An anonymous menu branch excludes explicit named alternatives only
    /// for its chooser, or for every observer when those names were public.
    /// Store identities in each observer's pick domain, never global private
    /// reference inequalities or the true anonymous cohort's card IDs.
    pub(super) fn constrain_anonymous_choice(
        &mut self,
        reference: HeldCard,
        excluded: &[HeldCard],
        chooser: Player,
    ) {
        // Disclosure may happen after the named reference was retained. Its
        // evidence bits then stay private, but the common tracker can name
        // that same pick. An independently known identity or another private
        // observer's knowledge does not establish a public branch exclusion.
        let exclusions: Vec<_> = excluded
            .iter()
            .map(|&excluded| {
                let public = Player::ALL
                    .into_iter()
                    .all(|viewer| excluded.evidence.known(viewer))
                    || excluded.evidence.pick().is_some_and(|pick| {
                        self.public_tracker().is_some_and(|tracker| {
                            tracker.pick_identity(pick) == Some(excluded.inner)
                        })
                    });
                (excluded, public)
            })
            .collect();
        if let Some(pick) = reference.evidence.pick()
            && let Some(knowledge) = &mut self.knowledge
        {
            for tracker in &mut knowledge.trackers {
                for &(excluded, public) in &exclusions {
                    let actor = tracker.viewer() == Some(chooser)
                        && (excluded.evidence.known(chooser)
                            || excluded
                                .evidence
                                .pick()
                                .is_some_and(|other| tracker.pick_identity(other).is_some()));
                    if public || actor {
                        tracker.exclude_pick_identity(&self.position, pick, excluded.inner);
                    }
                }
            }
        }
    }

    /// A reference to a card that just arrived where it now is: the pick its
    /// physical move followed, if `moved` names this card.
    pub(super) fn arrived(&self, card: CardId, moved: Option<(CardId, Pick)>) -> Evidence {
        self.position
            .locate(card)
            .map_or(Evidence::NONE, |zone| {
                Evidence::selection(&self.position, zone)
            })
            .with_pick(moved.and_then(|(moved, pick)| (moved == card).then_some(pick)))
    }
}
