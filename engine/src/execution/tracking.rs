//! Tracking hooks and hidden-world construction. Presentation logs are neither
//! retained nor replayed: knowledge changes at the physical commit boundary.
use super::*;
use crate::tracking::{COMMON, Knowledge, OBSERVERS, PICKS, Pick, Slot, Tracker};

#[cfg(test)]
mod reference_tests;
pub(super) mod remap;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) enum Departure {
    Unordered,
    Position,
    /// An explicitly selected position, possibly changed by intervening
    /// operations. A shuffle can erase its positional knowledge.
    Retained(Option<Pick>),
}

/// A hidden world for one observer; see [`Game::sample_world`].
pub(crate) struct Sampled {
    pub(crate) world: Game,
    tracker: Tracker,
    live: u64,
    /// Each live pick's card in the world.
    picks: [Option<CardId>; PICKS],
    /// The all-different cohorts sampling respected, live batches' included.
    distinct: [u64; PICKS],
}

/// Seed-independent reference checks for one immutable search root. Never
/// reuse this plan after changing the root or for a different observer.
pub(crate) struct SamplingPlan {
    viewer: Player,
    stable: Vec<bool>,
    live: u64,
    pinned: [Option<CardId>; PICKS],
    distinct: [u64; PICKS],
}

impl Game {
    /// Attach snapshot knowledge before play. Call immediately after new_base /
    /// new_figures (before the opening), or on an idle acceptance position.
    /// This cannot reconstruct past revelations; it never claims to do so.
    pub fn with_tracking(mut self) -> Result<Self, Error> {
        if self.tracked() {
            return Ok(self);
        }
        if !self.frames.is_empty()
            && !matches!(
                self.frames.as_slice(),
                [Frame::Opening(Opening {
                    picks: [None, None]
                })]
            )
        {
            return Err(Error::Busy);
        }
        self.knowledge = Some(Box::new(Knowledge::new(&self.position)?));
        Ok(self)
    }

    /// Sample a hidden world at a real decision or completed invocation.
    /// The observer's full subset state is retained. The other observer is
    /// rebuilt from common history and sampled current visibility, never the
    /// real opponent's private facts.
    /// Internal WorkLimit states are not search roots; finish advancing first.
    /// A retained reference the observer can name keeps its identity; any
    /// other takes a card its pick's candidates allow. A hidden source that
    /// cannot be pinned, a karma that already read a hidden operand's front,
    /// or a hidden reference without a pick returns Unsupported before any
    /// sampling.
    /// Neither the real game nor its gameplay RNG is consumed by sampling, and
    /// the world's own gameplay RNG continues the sampling stream instead.
    pub fn determinize(&self, viewer: Player, seed: u64) -> Result<Self, Error> {
        self.track_sample(viewer, self.sample_world(viewer, seed)?)
    }

    pub(crate) fn determinize_prepared(
        &self,
        plan: &SamplingPlan,
        seed: u64,
    ) -> Result<Self, Error> {
        self.track_sample(plan.viewer, self.sample_world_prepared(plan, seed)?)
    }

    fn track_sample(&self, viewer: Player, sample: Sampled) -> Result<Self, Error> {
        let Sampled {
            mut world,
            mut tracker,
            live,
            picks,
            distinct,
        } = sample;
        // Sampling retires references outside the retained continuation. Clear
        // their support before Knowledge can reuse their pick IDs.
        tracker.clear_picks(!live);
        let mut common = self
            .public_tracker()
            .expect("tracked sampling")
            .in_world(&world.position)?;
        common.clear_picks(!live);
        // References may retain a named identity while its physical location
        // changes in the sample. Adding its sampled slot loses correlation;
        // using the real slot would import hidden history.
        for (index, card) in picks.iter().enumerate() {
            if let Some(card) = *card {
                common.include(&world.position, card, Pick::new(index as u8));
            }
        }
        let other = common.for_player(&world.position, viewer.opponent())?;
        let mut knowledge = Knowledge {
            trackers: if viewer == Player::One {
                [tracker, other, common]
            } else {
                [other, tracker, common]
            },
            named: None,
            picks: live,
            distinct,
            moving: None,
            moved: None,
        };
        knowledge.retain_picks(live);
        // The world's references keep their picks, so both observers must
        // still follow each one. The sampling observer's candidates were
        // re-aligned with the sampled positions; a pick a reference names may
        // have landed elsewhere, so its card's slot joins them. The other
        // observer already inherited common pick support, including each
        // sampled card above. Do not mark these as fresh hidden selections:
        // that would spread a publicly identified pick over its whole pile.
        for (index, card) in picks.iter().enumerate() {
            if let Some(card) = *card {
                let pick = Pick::new(index as u8);
                knowledge.trackers[viewer.index()].include(&world.position, card, pick);
            }
        }
        world.knowledge = Some(Box::new(knowledge));
        world.refresh_tracking()?;
        Ok(world)
    }

    /// A sampled world without beliefs, as search simulations use, with the
    /// sampling observer's tracker re-aligned with it and the live picks'
    /// cards. Same contract as [`Game::determinize`].
    pub(crate) fn sample_world(&self, viewer: Player, seed: u64) -> Result<Sampled, Error> {
        let plan = self.prepare_sampling(viewer)?;
        self.sample_world_prepared(&plan, seed)
    }

    pub(crate) fn prepare_sampling(&self, viewer: Player) -> Result<SamplingPlan, Error> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        if let Some(Frame::Choose { player, .. }) = self.frames.last()
            && *player != viewer
        {
            return Err(Error::Unsupported(
                "sample a decision for its acting observer",
            ));
        }
        if !self.frames.is_empty() && !matches!(self.frames.last(), Some(Frame::Choose { .. })) {
            return Err(Error::Busy);
        }
        let tracker = self.tracker(viewer).ok_or(Error::Unsupported(
            "determinization requires tracking from setup",
        ))?;
        // A batch visits every original exactly once, so its visited
        // references are different cards. Once anonymous ones could share a
        // candidate (a replacement left one in its source, or a later move
        // mixed them with like-backed cards), sampling needs that public
        // distinctness: add the batch's ambiguous references as one cohort.
        // The sampler supports disjoint cohorts only, so joining one to an
        // unrelated retained cohort still refuses, before consuming RNG.
        let mut distinct = self.knowledge.as_ref().expect("tracked sampling").distinct;
        for frame in &self.frames {
            if let Frame::MoveAll(batch) = frame
                && batch.quotas.is_none()
            {
                let mut cohort = 0u64;
                for (index, left) in batch.visited.iter().enumerate() {
                    for right in &batch.visited[index + 1..] {
                        let (Some(a), Some(b)) = (left.evidence.pick(), right.evidence.pick())
                        else {
                            continue;
                        };
                        let named = |held: &HeldCard| {
                            held.evidence.known(viewer)
                                || held
                                    .evidence
                                    .pick()
                                    .is_some_and(|pick| tracker.pick_identity(pick).is_some())
                        };
                        if a != b
                            && !(named(left) && named(right))
                            && distinct[a.index()] & (1 << b.index()) == 0
                            && tracker.picks_may_coincide(a, b)
                        {
                            cohort |= 1 << a.index() | 1 << b.index();
                        }
                    }
                }
                let mut members = cohort;
                while members != 0 {
                    let pick = members.trailing_zeros() as usize;
                    members &= members - 1;
                    if distinct[pick] & !cohort != 0 {
                        return Err(Error::Unsupported(
                            "sampling a batch with overlapping unresolved attempted cards",
                        ));
                    }
                    distinct[pick] |= cohort & !(1 << pick);
                }
            }
        }
        let stable: Vec<_> = (0..self.position.catalog.len())
            .map(|i| tracker.reference_stable(&self.position, CardId(i as u16)))
            .collect();
        // Seed-independent preflight: neither a historical selection nor a
        // consumed hidden predicate authorizes pinning its real identity.
        let mut scan = remap::Remap::new(remap::Mode::Scan, viewer, &self.position, stable.clone());
        scan.tracker = Some(tracker);
        scan.frames(&mut self.frames.clone());
        scan.completed(&mut self.completed.clone());
        if let Some(why) = scan.opaque.get() {
            return Err(Error::Unsupported(why));
        }
        let pinned = *scan.pinned.borrow();
        let named = pinned
            .iter()
            .enumerate()
            .filter(|(_, card)| card.is_some())
            .fold(0u64, |named, (pick, _)| named | 1 << pick);
        if scan.hidden.get() & !named & !tracker.tracked_picks() != 0 {
            return Err(Error::Unsupported(remap::UNTRACKED));
        }
        Ok(SamplingPlan {
            viewer,
            stable,
            live: scan.live.get(),
            pinned,
            distinct,
        })
    }

    pub(crate) fn sample_world_prepared(
        &self,
        plan: &SamplingPlan,
        seed: u64,
    ) -> Result<Sampled, Error> {
        let viewer = plan.viewer;
        let tracker = self.tracker(viewer).expect("prepared tracked root");
        let mut rng = Rng::new(seed);
        let sample = tracker.sample(
            &self.position,
            plan.live,
            &plan.distinct,
            &plan.pinned,
            &mut rng,
        )?;
        // A pick some reference names is that card for every reference to it.
        let mut picks = sample.picks;
        for (pick, card) in picks.iter_mut().zip(plan.pinned) {
            *pick = card.or(*pick);
        }
        let mut world = Self {
            position: self.position.clone(),
            frames: self.frames.clone(),
            completed: self.completed,
            terminal: self.terminal,
            fault: None,
            rng: self.rng.clone(),
            knowledge: None,
            repeats: Vec::new(),
            automatic_work: self.automatic_work,
            reveal_scope: self.reveal_scope,
            inspecting: self.inspecting,
            // Search worlds keep no log.
            narrated: false,
            // A simulation treats Martian Internet as a draw and a subgame's
            // end as the game's end; it never rejoins a parent.
            subgames: false,
            parent: None,
            #[cfg(test)]
            shortcuts_disabled: self.shortcuts_disabled,
        };
        // Future shuffles and blind picks must not replay the real game's
        // hidden randomness: the world continues the sampling stream.
        world.rng = rng;
        for zone in crate::tracking::zones() {
            // Optional expansion storage is absent in Base games.
            if world.position.cards(zone).is_empty() {
                continue;
            }
            for card in world.position.zone_mut(zone) {
                *card = sample.cards[usize::from(card.0)];
            }
        }
        // A revealed card is known, so it samples as itself; keep the
        // reveal pointing at the sampled identity regardless.
        if let Some(artifacts) = world.position.artifacts.as_mut()
            && let Some(card) = artifacts.revealed.as_mut()
        {
            *card = sample.cards[usize::from(card.0)];
        }
        world.position.refresh_boards();
        // A subgame world may swap identities between the supply and the
        // parent players' set-aside cards; the absent set follows the sample.
        world.position.refresh_absent();
        let mut map = remap::Remap::new(
            remap::Mode::Sample {
                cards: &sample.cards,
                picks: &picks,
            },
            viewer,
            &world.position,
            plan.stable.clone(),
        );
        // The same naming as the preflight, so it cannot refuse here.
        map.tracker = Some(tracker);
        map.frames(&mut world.frames);
        debug_assert!(map.opaque.get().is_none());
        map.completed(&mut world.completed);
        // Pending physical menu members are complements of the retained
        // cohort, not independently retained references. Rebuild those menus
        // from its sampled identities, so a remaining back cannot select a
        // card already chosen under a different physical remapping.
        world.refresh_sampled_group_menu()?;
        world.refresh_blind_choice_groups()?;
        world.refresh_move_selected_menu()?;
        world.refresh_blind_operation_groups()?;
        // Decisions/completions already clear mandatory-loop history. Never
        // copy an omniscient earlier hidden world into a simulation's witness:
        // the world above starts with none.
        Ok(Sampled {
            world,
            tracker: sample.tracker,
            live: plan.live,
            picks,
            distinct: plan.distinct,
        })
    }

    /// Reclaim picks no retained reference holds. Call only between steps,
    /// when every reference lives in the continuation.
    pub(super) fn collect_picks(&mut self) {
        let Some(knowledge) = &self.knowledge else {
            return;
        };
        if knowledge.picks.count_ones() < 32 {
            return;
        }
        let scan = remap::Remap::new(
            remap::Mode::Scan,
            Player::One,
            &self.position,
            vec![true; self.position.catalog.len()],
        );
        scan.frames(&mut self.frames.clone());
        scan.completed(&mut self.completed.clone());
        // A suspended parent's continuation keeps its picks through a subgame.
        let live = scan.live.get() | self.parent.as_ref().map_or(0, |parent| parent.live_picks());
        if let Some(knowledge) = &mut self.knowledge {
            knowledge.retain_picks(live);
        }
    }

    /// Drop private and public beliefs, for example in a sampled simulation.
    pub fn untracked(mut self) -> Self {
        self.knowledge = None;
        self
    }

    /// Whether this game keeps belief trackers. Knowledge proofs, picks and
    /// provenance masks exist only in a tracked game; an untracked game (every
    /// search world) treats every scalar and branch as public and does no
    /// proof work, so this one predicate gates all of it.
    #[inline]
    pub(crate) fn tracked(&self) -> bool {
        self.knowledge.is_some()
    }

    pub fn tracker(&self, viewer: Player) -> Option<&Tracker> {
        self.knowledge.as_ref().map(|k| &k.trackers[viewer.index()])
    }

    /// History shared by both players, without either player's private cards.
    pub fn public_tracker(&self) -> Option<&Tracker> {
        self.knowledge.as_ref().map(|k| &k.trackers[COMMON])
    }

    // Keep the optional check at the caller; untracked play has no closure work.
    #[inline]
    pub(super) fn refresh_tracking(&mut self) -> Result<(), Error> {
        if let Some(knowledge) = &mut self.knowledge
            && let Err(error) = knowledge.refresh(&self.position)
        {
            self.fault = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// Every observer learns this card's identity where it lies.
    pub(super) fn disclose(&mut self, card: CardId) {
        let Some(zone) = self.position.locate(card) else {
            return;
        };
        if let Some(knowledge) = &mut self.knowledge {
            for tracker in &mut knowledge.trackers {
                tracker.reveal(&self.position, card, zone, None);
            }
        }
        // A contradiction faults the game; the caller's next step reports it.
        let _ = self.refresh_tracking();
    }

    /// The departing card follows the pick the moving reference named, or a
    /// fresh pick when some observer cannot see the move, so its result can
    /// be followed. Hidden departures merge candidates in the trackers.
    pub(super) fn track_extract(
        &mut self,
        card: CardId,
        source: Zone,
        destination: Zone,
        departure: Departure,
    ) -> Option<[Slot; OBSERVERS]> {
        // Untracked games (search worlds, random play) skip the visibility test.
        self.knowledge.as_ref()?;
        let seen = Player::ALL.into_iter().all(|viewer| {
            self.position.can_view(source, viewer) || self.position.can_view(destination, viewer)
        });
        self.knowledge.as_mut().map(|k| {
            let public = k
                .named
                .is_some_and(|(deck, found)| source == Zone::Deck(deck) && card == found);
            if public {
                k.named = None;
            }
            let pick = match k.moving.take() {
                Some(pick) => Some(pick),
                None if seen => None,
                None => k.allocate(),
            };
            k.moved = pick.map(|pick| (card, pick));
            k.trackers.each_mut().map(|t| {
                let positional = match departure {
                    Departure::Unordered => false,
                    Departure::Position => true,
                    Departure::Retained(Some(pick)) => t.pick_position(source, pick).is_some(),
                    Departure::Retained(None) => false,
                };
                t.extract(
                    &self.position,
                    card,
                    source,
                    destination,
                    public || pick.is_some_and(|pick| t.pick_identity(pick) == Some(card)),
                    positional,
                    pick,
                )
            })
        })
    }

    pub(super) fn track_insert(
        &mut self,
        destination: Zone,
        slots: Option<[Slot; OBSERVERS]>,
        bottom: bool,
    ) {
        if let (Some(k), Some(slots)) = (&mut self.knowledge, slots) {
            for (tracker, slot) in k.trackers.iter_mut().zip(slots) {
                tracker.insert(destination, slot, bottom);
            }
        }
    }
}
