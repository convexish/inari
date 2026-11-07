//! Martian Internet's printed rule: a subgame on the parent's supply, whose winner's
//! cards join the parent. The draw variant, which training and every search world use,
//! never reaches this module. A subgame is the same [`Game`] with its position, frames
//! and knowledge swapped for the child's; the parent is frozen behind a shared pointer
//! and restored when the child reaches a terminal. Nesting is a longer chain.

use super::*;
use crate::tracking::{Knowledge, OBSERVERS, Slot, player_zone};

/// A suspended game, frozen while its subgame runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Parent {
    position: Position,
    frames: Vec<Frame>,
    knowledge: Option<Box<Knowledge>>,
    completed: HeldReturn,
    repeats: Vec<Recurrence>,
    reveal_scope: Option<RevealScope>,
    inspecting: bool,
    /// Picks the parent's continuation holds; the child keeps them allocated
    /// and never reclaims them.
    live_picks: u64,
    /// The execution that started the subgame.
    context: Context,
    outer: Option<Arc<Parent>>,
}

impl Parent {
    pub(super) fn live_picks(&self) -> u64 {
        self.live_picks
    }

    pub(super) fn position(&self) -> &Position {
        &self.position
    }

    pub(super) fn tracker(&self, viewer: Player) -> Option<&crate::Tracker> {
        self.knowledge
            .as_ref()
            .map(|knowledge| &knowledge.trackers[viewer.index()])
    }

    pub(super) fn outer(&self) -> Option<&Arc<Parent>> {
        self.outer.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SubgameStep {
    /// With exactly one card of value 9 or higher left, the subgame cannot start and
    /// that card is junked instead.
    JunkOne { context: Context },
    /// Setup: Player One, then Player Two, draws and melds a 9 (stages 0 and
    /// 1); then the lower card alphabetically moves first with one action.
    Opening { stage: u8, context: Context },
}

impl SubgameStep {
    pub(super) fn context_mut(&mut self) -> &mut Context {
        match self {
            Self::JunkOne { context } | Self::Opening { context, .. } => context,
        }
    }
}

/// Museums a parent's players hold: stored, claimed, junked or being awarded.
/// Unavailable to the subgame, which counts them as junked in its own storage.
fn held_museums(artifacts: &Artifacts) -> u8 {
    artifacts
        .museums
        .iter()
        .map(|stored| stored.len() as u8)
        .sum::<u8>()
        + artifacts.claimed.iter().sum::<u8>()
        + artifacts.junked
        + u8::from(artifacts.pending.is_some())
}

/// Each player's zones in the order a merge handles them: capacity-limited
/// zones after the board, whose splays set the capacity.
fn held_zones(player: Player) -> impl Iterator<Item = Zone> {
    [
        Zone::Hand(player),
        Zone::Score(player),
        Zone::Achievements(player),
    ]
    .into_iter()
    .chain(
        Color::ALL
            .into_iter()
            .map(move |color| Zone::Board(player, color)),
    )
    .chain([
        Zone::Forecast(player),
        Zone::Safe(player),
        Zone::Display(player),
        Zone::Museum(player),
    ])
}

impl Game {
    /// Play Martian Internet's printed subgame instead of the draw variant.
    /// Search worlds always use the draw variant, so this changes human play
    /// and duels that ask for it, never training or the tree.
    pub fn with_subgames(mut self, subgames: bool) -> Self {
        self.subgames = subgames;
        self
    }

    pub fn subgames(&self) -> bool {
        self.subgames
    }

    /// How many suspended games this one is nested in (0 ordinarily).
    /// Tests: the next automatic frame hits the mandatory-loop cap.
    #[cfg(test)]
    pub(crate) fn exhaust_automatic_work(&mut self) {
        self.automatic_work = super::AUTOMATIC_WORK_LIMIT;
    }

    pub fn subgame_depth(&self) -> usize {
        let mut depth = 0;
        let mut parent = self.parent.as_ref();
        while let Some(frozen) = parent {
            depth += 1;
            parent = frozen.outer();
        }
        depth
    }

    /// The suspended game `levels` up (1 is the immediate parent), as it was
    /// when its subgame began; its supply is stale while the subgame runs.
    pub fn parent_position(&self, levels: usize) -> Option<&Position> {
        let mut parent = self.parent.as_ref()?;
        for _ in 1..levels {
            parent = parent.outer()?;
        }
        Some(parent.position())
    }

    /// The suspended game's beliefs for this viewer `levels` up, as they
    /// were when its subgame began.
    pub fn parent_tracker(&self, levels: usize, viewer: Player) -> Option<&crate::Tracker> {
        let mut parent = self.parent.as_ref()?;
        for _ in 1..levels {
            parent = parent.outer()?;
        }
        parent.tracker(viewer)
    }

    /// Martian Internet's effect under the printed rule, with the program frame already
    /// pushed past it. Fewer than two cards of value 9 or higher means no subgame;
    /// exactly one is junked instead.
    pub(super) fn begin_subgame(&mut self, context: Context) -> Result<(), Error> {
        let supply: usize = (9..=11)
            .map(|age| self.position.cards(Zone::Deck(Deck::base(age))).len())
            .sum();
        if supply == 0 {
            self.complete(ReturnValue::Unit);
            return Ok(());
        }
        // Junking the card or playing a subgame is something happening, so a
        // sharer earns the executor's bonus draw.
        self.note_shared_change(context);
        if supply == 1 {
            self.frames
                .push(Frame::Subgame(SubgameStep::JunkOne { context }));
            return Ok(());
        }
        self.park(context)?;
        self.frames
            .push(Frame::Subgame(SubgameStep::Opening { stage: 0, context }));
        Ok(())
    }

    /// Freeze this game and become its subgame: the supply moves to a fresh
    /// position, every card a player holds is set aside, and the trackers
    /// are forked ([`Knowledge::fork_for_subgame`]).
    fn park(&mut self, context: Context) -> Result<(), Error> {
        let parent = &self.position;
        let mut child = Position::empty(Arc::clone(&parent.catalog), Player::One)
            .with_expansions(parent.expansions);
        child.decks.clone_from(&parent.decks);
        child.set_decks.clone_from(&parent.set_decks);
        child.age_zero.clone_from(&parent.age_zero);
        child.junk.clone_from(&parent.junk);
        child
            .available_achievements
            .clone_from(&parent.available_achievements);
        // Specials and Decrees a parent player owns are unavailable here;
        // the parent's own storage returns at the merge.
        child.specials = parent.specials.map(|location| match location {
            SpecialLocation::Available => SpecialLocation::Available,
            SpecialLocation::Owned(_) | SpecialLocation::Junk => SpecialLocation::Junk,
        });
        child.decrees = parent.decrees.as_ref().map(|decrees| {
            Box::new(decrees.map(|location| match location {
                DecreeLocation::Supply => DecreeLocation::Supply,
                DecreeLocation::Owned(_) | DecreeLocation::Junk => DecreeLocation::Junk,
            }))
        });
        if let Some(artifacts) = parent.artifacts.as_deref() {
            child.artifacts = Some(Box::new(Artifacts {
                junked: held_museums(artifacts),
                ..Artifacts::default()
            }));
        }
        let mut absent = parent
            .absent
            .clone()
            .unwrap_or_else(|| vec![false; parent.catalog.len()].into_boxed_slice());
        for zone in Player::ALL.into_iter().flat_map(held_zones) {
            for &card in parent.cards(zone) {
                absent[usize::from(card.0)] = true;
            }
        }
        child.absent = Some(absent);
        child.refresh_boards();
        let knowledge = self
            .knowledge
            .as_ref()
            .map(|knowledge| knowledge.fork_for_subgame(&child).map(Box::new))
            .transpose()?;
        let live_picks = self
            .knowledge
            .as_ref()
            .map_or(0, |knowledge| knowledge.picks);
        let parent = Parent {
            position: std::mem::replace(&mut self.position, child),
            frames: std::mem::take(&mut self.frames),
            knowledge: std::mem::replace(&mut self.knowledge, knowledge),
            completed: std::mem::replace(&mut self.completed, HeldReturn::UNIT),
            repeats: std::mem::take(&mut self.repeats),
            reveal_scope: self.reveal_scope.take(),
            inspecting: std::mem::replace(&mut self.inspecting, false),
            live_picks,
            context,
            outer: self.parent.take(),
        };
        self.parent = Some(Arc::new(parent));
        Ok(())
    }

    pub(super) fn resume_subgame(
        &mut self,
        step: SubgameStep,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        match step {
            SubgameStep::JunkOne { context } => {
                let deck = (9..=11)
                    .map(Deck::base)
                    .find(|&deck| !self.position.cards(Zone::Deck(deck)).is_empty())
                    .expect("one card of value 9 or higher");
                let card = *self
                    .position
                    .cards(Zone::Deck(deck))
                    .last()
                    .expect("a non-empty deck");
                self.place(
                    Some(Verb::Junk),
                    card,
                    Zone::Deck(deck),
                    Zone::Junk,
                    tracking::Departure::Position,
                    context,
                    events,
                )?;
                self.complete(ReturnValue::Unit);
            }
            SubgameStep::Opening { stage, context } => {
                if stage == 0 {
                    events.push(Event::SubgameStarted {
                        executor: context.executor,
                        depth: self.subgame_depth(),
                    });
                }
                if let Some(&player) = Player::ALL.get(usize::from(stage)) {
                    // Setup moves, like a normal opening: no karma, sharing
                    // or end checks. The pre-check guarantees the card.
                    let drawn = self.commit(
                        Operation::Draw {
                            value: 9,
                            set: Some(CardSet::Base),
                        },
                        Context::new(player),
                        false,
                        events,
                    )?;
                    let card = drawn.card.expect("a card of value 9 or higher remains");
                    self.commit(
                        Operation::Move {
                            verb: Verb::Meld,
                            target: Target {
                                card,
                                physical: Zone::Hand(player),
                                logical: LogicalZone::Hand(player),
                            },
                            destination: Zone::Board(player, self.position.card(card).color),
                        },
                        Context::new(player),
                        false,
                        events,
                    )?;
                    self.frames.push(Frame::Subgame(SubgameStep::Opening {
                        stage: stage + 1,
                        context,
                    }));
                    return Ok(());
                }
                let title = |player: Player| {
                    let card = Color::ALL
                        .into_iter()
                        .find_map(|color| self.position.top(player, color))
                        .expect("a melded 9");
                    let code = self.position.card(card).code;
                    BASE_METADATA
                        .iter()
                        .find(|metadata| metadata.code == code)
                        .map_or(code, |metadata| metadata.title)
                };
                let first = if title(Player::One) < title(Player::Two) {
                    Player::One
                } else {
                    Player::Two
                };
                self.position.begin_opening_turn(first);
                if self.narrated {
                    events.push(Event::TurnBegan { player: first });
                }
                self.complete(ReturnValue::Unit);
            }
        }
        Ok(())
    }

    /// The subgame reached a terminal: merge it into the suspended parent and
    /// resume the parent's program after the effect.
    pub(super) fn end_subgame(
        &mut self,
        terminal: Terminal,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let depth = self.subgame_depth();
        let winner = match terminal {
            Terminal::Winner(player, _) => Some(player),
            Terminal::Draw(_) => None,
        };
        events.push(Event::SubgameEnded { winner, depth });
        let Parent {
            position,
            frames,
            knowledge,
            completed: _,
            repeats,
            reveal_scope,
            inspecting,
            live_picks: _,
            context,
            outer,
        } = Arc::unwrap_or_clone(self.parent.take().expect("a suspended parent"));
        let child = std::mem::replace(&mut self.position, position);
        let child_knowledge = std::mem::replace(&mut self.knowledge, knowledge);
        self.frames = frames;
        self.repeats = repeats;
        self.reveal_scope = reveal_scope;
        self.inspecting = inspecting;
        self.parent = outer;
        self.terminal = None;
        self.repeats.clear();
        self.automatic_work = 0;
        self.merge(child, child_knowledge, winner, context, events);
        self.complete(ReturnValue::Unit);
        self.refresh_tracking()
    }

    /// Take the supply back as the subgame left it, give the winner their
    /// cards, specials, Decrees and Museums, and junk everything else.
    fn merge(
        &mut self,
        mut child: Position,
        mut child_knowledge: Option<Box<Knowledge>>,
        winner: Option<Player>,
        context: Context,
        events: &mut Vec<Event>,
    ) {
        let parent = &mut self.position;
        parent.decks = std::mem::take(&mut child.decks);
        parent.set_decks = std::mem::take(&mut child.set_decks);
        parent.age_zero = child.age_zero.take();
        parent.junk = std::mem::take(&mut child.junk);
        parent.available_achievements = std::mem::take(&mut child.available_achievements);
        if let (Some(knowledge), Some(child_knowledge)) =
            (&mut self.knowledge, &mut child_knowledge)
        {
            // The subgame's own references are over; the parent's stay.
            let dead = child_knowledge.picks & !knowledge.picks;
            for (tracker, child_tracker) in knowledge
                .trackers
                .iter_mut()
                .zip(&mut child_knowledge.trackers)
            {
                child_tracker.clear_picks(dead);
                tracker.adopt_supply(child_tracker);
            }
        }
        // Specials and Decrees claimed in the subgame: the winner's stay
        // theirs, the loser's are junked, as are any junked there.
        let specials: Vec<_> = self.position.specials().collect();
        for special in specials {
            let destination = match (
                self.position.special_location(special),
                child.special_location(special),
            ) {
                (SpecialLocation::Available, SpecialLocation::Owned(player)) => {
                    Some(if Some(player) == winner {
                        SpecialLocation::Owned(player)
                    } else {
                        SpecialLocation::Junk
                    })
                }
                (SpecialLocation::Available, SpecialLocation::Junk) => Some(SpecialLocation::Junk),
                _ => None,
            };
            if let Some(destination) = destination {
                self.move_special(special, destination, context, events);
            }
        }
        for decree in Decree::ALL {
            let destination = match (
                self.position.decree_location(decree),
                child.decree_location(decree),
            ) {
                (DecreeLocation::Supply, DecreeLocation::Owned(player)) => {
                    Some(if Some(player) == winner {
                        DecreeLocation::Owned(player)
                    } else {
                        DecreeLocation::Junk
                    })
                }
                (DecreeLocation::Supply, DecreeLocation::Junk) => Some(DecreeLocation::Junk),
                _ => None,
            };
            if let Some(destination) = destination {
                self.position
                    .decrees
                    .get_or_insert_with(|| Box::new([DecreeLocation::Supply; 5]))
                    [decree as usize] = destination;
                events.push(Event::DecreeMoved {
                    decree,
                    source: DecreeLocation::Supply,
                    destination,
                    executor: context.executor,
                    inside_karma: false,
                });
            }
        }
        // Museums: the winner keeps claimed ones; the loser's, and Museums
        // junked in the subgame, leave the game. Stored Artifacts move with
        // the cards below.
        if let Some(child_artifacts) = child.artifacts.as_deref() {
            let artifacts = self.position.artifacts.get_or_insert_default();
            let mut junked = child_artifacts
                .junked
                .saturating_sub(held_museums(artifacts))
                + u8::from(child_artifacts.pending.is_some());
            for player in Player::ALL {
                if Some(player) == winner {
                    artifacts.claimed[player.index()] += child_artifacts.claimed[player.index()];
                } else {
                    junked += child_artifacts.claimed[player.index()];
                }
            }
            artifacts.junked += junked;
        }
        for player in Player::ALL {
            let keeps = Some(player) == winner;
            for zone in held_zones(player) {
                let cards = child.cards(zone).to_vec();
                if cards.is_empty() {
                    continue;
                }
                let splay = child.splay(player, zone_color(zone));
                let onto_empty = keeps && self.position.cards(zone).is_empty();
                for card in cards {
                    let slots = child_knowledge
                        .as_mut()
                        .map(|knowledge| knowledge.trackers.each_mut().map(|t| t.take_slot(zone)));
                    // Full Forecast or Safe, an occupied display (appendix),
                    // and everything the loser held: junked.
                    let destination = if keeps && self.position.has_room(zone) {
                        zone
                    } else {
                        Zone::Junk
                    };
                    self.adopt(card, zone, destination, slots, player, events);
                }
                if let (true, Zone::Board(_, color)) = (onto_empty, zone) {
                    // A color the parent did not have keeps the subgame's
                    // splay; otherwise the parent's splay stays.
                    self.position.players[player.index()].splays[color as usize] = splay;
                }
            }
        }
        self.position.refresh_boards();
    }

    /// One card leaving a finished subgame, with its tracker slots.
    fn adopt(
        &mut self,
        card: CardId,
        source: Zone,
        destination: Zone,
        slots: Option<[Slot; OBSERVERS]>,
        owner: Player,
        events: &mut Vec<Event>,
    ) {
        debug_assert!(player_zone(source));
        // A Museum is a card too: a stored Artifact junked takes it along.
        if matches!(source, Zone::Museum(_)) && destination == Zone::Junk {
            self.position.junk_emptied_museum();
        }
        self.track_insert(destination, slots, false);
        self.position.zone_mut(destination).push(card);
        self.position.set_location(card, destination);
        if let Zone::Board(player, _) = destination {
            self.position.refresh_board(player);
        }
        events.push(Event::Transferred {
            card,
            source,
            destination,
            owner,
        });
    }
}

/// A board zone's color; any color for the rest (unused then).
fn zone_color(zone: Zone) -> Color {
    match zone {
        Zone::Board(_, color) => color,
        _ => Color::ALL[0],
    }
}
