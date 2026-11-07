use std::sync::{Arc, OnceLock};

use super::*;

/// An immutable authored catalog and its unshuffled decks.
struct Catalog {
    cards: Arc<[Card]>,
    decks: [Vec<CardId>; 12],
    /// Each other set's decks, by [`CardSet::index`] minus one.
    set_decks: [[Vec<CardId>; 12]; 5],
    age_zero: Vec<CardId>,
}

/// Base first, so Base identities match the Base catalog in every mode;
/// then Figures and Age 0 (matching `new_figures`), then Cities, Echoes and
/// Artifacts. Appending a set preserves every physical ID of the
/// corresponding mode without it. Shuffle order is independent of this
/// storage order.
fn catalog_cards(expansions: Expansions) -> Result<Vec<Card>, Error> {
    let base = BASE_METADATA
        .iter()
        .map(|metadata| base_card(metadata.code));
    let figures = FIGURE_METADATA
        .iter()
        .map(|metadata| figure_card(metadata.code))
        .chain(
            AGE_ZERO_METADATA
                .iter()
                .map(|metadata| age_zero_card(metadata.code)),
        )
        .filter(|_| expansions.figures());
    let cities = CITY_METADATA
        .iter()
        .map(|metadata| city_card(metadata.code))
        .filter(|_| expansions.cities());
    let echoes = ECHOES_METADATA
        .iter()
        .filter(|_| expansions.echoes())
        .map(|metadata| echoes_card(metadata.code));
    let artifacts = ARTIFACTS_METADATA
        .iter()
        .filter(|_| expansions.artifacts())
        .map(|metadata| artifacts_card(metadata.code));
    let unseen = UNSEEN_METADATA
        .iter()
        .filter(|_| expansions.unseen())
        .map(|metadata| unseen_card(metadata.code));
    base.chain(figures)
        .chain(cities)
        .chain(echoes)
        .chain(artifacts)
        .chain(unseen)
        .collect()
}

/// One shared catalog per expansion combination, built on first use.
static CATALOGS: [OnceLock<Result<Catalog, Error>>; Expansions::COMBINATIONS] =
    [const { OnceLock::new() }; Expansions::COMBINATIONS];

impl Catalog {
    fn new(cards: Vec<Card>) -> Self {
        let cards: Arc<[Card]> = cards.into();
        let ids = |set: CardSet, age: usize| {
            let mut ids: Vec<_> = cards
                .iter()
                .enumerate()
                .filter(move |(_, card)| card.set == set && usize::from(card.value) == age)
                .map(|(id, _)| CardId(id as u16))
                .collect();
            // Shuffle age-one Echoes in replay order: E001, E012, E013,
            // E002..E011, E014, E015. Card IDs stay in code order.
            if set == CardSet::Echoes && age == 1 && !ids.is_empty() {
                ids[1..13].rotate_right(2);
            }
            ids
        };
        // Age 0 starts outside the game, not as a Base deck.
        let decks = std::array::from_fn(|age| {
            if age == 0 {
                Vec::new()
            } else {
                ids(CardSet::Base, age)
            }
        });
        let set_decks =
            std::array::from_fn(|i| std::array::from_fn(|age| ids(CardSet::ALL[i + 1], age)));
        let age_zero = ids(CardSet::Base, 0);
        Self {
            cards,
            decks,
            set_decks,
            age_zero,
        }
    }

    fn get(expansions: Expansions) -> Result<&'static Self, Error> {
        CATALOGS[expansions.index()]
            .get_or_init(|| catalog_cards(expansions).map(Catalog::new))
            .as_ref()
            .map_err(Clone::clone)
    }
}

/// A commitment stays in its owner's hand until both players have chosen.
/// Keeping it in a continuation avoids permanent setup state on every game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Opening {
    pub(super) picks: [Option<CardId>; 2],
}

impl Game {
    /// Deal the complete two-player Base game, then await simultaneous opening
    /// commitments through `advance` / `choose`. `CardId(i)` is BASE_METADATA[i].
    /// After both starting melds, use `take_action` for the first one-action turn
    /// and subsequent two-action turns. No Figures or Age 0 cards are included.
    ///
    /// The immutable authored catalog is initialized once and shared by games.
    /// Missing authored programs fail explicitly; setup never substitutes stubs.
    pub fn new_base(seed: u64) -> Result<Self, Error> {
        Self::new_game(seed, Expansions::BASE)
    }

    /// Benchmark only: the Base deal under Figures rules, with no Figures or
    /// Age 0 cards. This is not a Figures setup (the Figures rules still
    /// apply, e.g. seven achievements to win); it measures what the Figures
    /// machinery costs when no Figure is in play.
    #[doc(hidden)]
    pub fn base_deal_with_figures_rules(seed: u64) -> Result<Self, Error> {
        Self::deal_base(Catalog::get(Expansions::BASE)?, Expansions::FIGURES, seed)
    }

    /// Deal a complete two-player Base + Figures game. The Base deal is
    /// identical to `new_base(seed)`; the eleven Figures decks shuffle
    /// afterwards. All five Decrees start in the supply and Age 0 waits
    /// outside the game until Bear Grylls introduces it.
    pub fn new_figures(seed: u64) -> Result<Self, Error> {
        Self::new_game(seed, Expansions::FIGURES)
    }

    /// Deal a complete two-player game with Echoes, Figures and Cities in any
    /// combination. Base is always included; its deal is identical to
    /// `new_base(seed)`; then each enabled set's decks shuffle in set order
    /// (Echoes, Cities, Unseen, Artifacts, Figures), ascending ages. Disabled sets use no
    /// RNG. Echoes and Cities each add five special achievements; all ten
    /// starting standard achievements and all four starting hand cards are Base.
    pub fn new_game(seed: u64, expansions: Expansions) -> Result<Self, Error> {
        let mut game = Self::deal_base(Catalog::get(expansions)?, expansions, seed)?;
        for set in CardSet::ALL {
            if set == CardSet::Base || !expansions.includes(set) {
                continue;
            }
            let decks = game
                .position
                .set_decks_mut(set)
                .expect("enabled set's storage");
            for deck in decks.iter_mut().skip(1) {
                game.rng.shuffle(deck);
            }
        }
        Ok(game)
    }

    fn deal_base(catalog: &Catalog, expansions: Expansions, seed: u64) -> Result<Self, Error> {
        let mut position = Position::empty(Arc::clone(&catalog.cards), Player::One);
        position.decks = catalog.decks.clone();
        for set in CardSet::ALL.into_iter().skip(1) {
            if let Some(decks) = position.set_decks_mut(set) {
                decks.clone_from_slice(&catalog.set_decks[set.index() - 1]);
            }
        }
        if let Some(age_zero) = position.age_zero.as_mut() {
            age_zero.reserve = catalog.age_zero.clone();
        }
        let mut game = Self::seeded(position, expansions, seed)?;
        // Replay order: ascending ages, Fisher-Yates, pop from
        // the back. Reserve after all shuffles, then deal two to each seat.
        for deck in &mut game.position.decks {
            game.rng.shuffle(deck);
        }
        game.position.available_achievements.reserve_exact(10);
        for age in 1..=10 {
            let card = game.position.decks[age].pop().expect("Base age exists");
            game.position.available_achievements.push(card);
        }
        for player in Player::ALL {
            let hand = &mut game.position.players[player.index()].hand;
            hand.reserve_exact(2);
            for _ in 0..2 {
                hand.push(game.position.decks[1].pop().expect("Base starting hand"));
            }
        }
        game.position.rebuild_locations();
        game.frames
            .push(Frame::Opening(Opening { picks: [None; 2] }));
        Ok(game)
    }
}

impl Game {
    pub(super) fn resume_opening(
        &mut self,
        opening: Opening,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        if let Some(player) = Player::ALL
            .into_iter()
            .find(|player| opening.picks[player.index()].is_none())
        {
            let hand = self.position.cards(Zone::Hand(player));
            let tasks = hand
                .iter()
                .map(|&card| Task::OpeningMeld { player, card })
                .collect();
            let options = hand
                .iter()
                .map(|&card| Alternative::Move {
                    verb: Verb::Meld,
                    target: self.position.descriptor(
                        Target {
                            card,
                            physical: Zone::Hand(player),
                            logical: LogicalZone::Hand(player),
                        },
                        player,
                    ),
                })
                .collect();
            self.frames.push(Frame::Opening(opening));
            self.frames.push(Frame::Choose {
                player,
                tasks,
                options: DecisionOptions::new(options, Context::new(player)),
            });
            return Ok(());
        }
        let picks = opening
            .picks
            .map(|card| card.expect("both opening commitments"));
        let first = if BASE_METADATA[usize::from(picks[0].0)].title
            < BASE_METADATA[usize::from(picks[1].0)].title
        {
            Player::One
        } else {
            Player::Two
        };
        // Both reveals happen in this single continuation step. Starting melds
        // are not actions: no ActionTail, dogma, sharing reward, or end checks.
        for player in Player::ALL {
            let card = picks[player.index()];
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
        }
        self.position.begin_opening_turn(first);
        if self.narrated {
            events.push(Event::TurnBegan { player: first });
        }
        self.complete(ReturnValue::Unit);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_games_share_authored_catalog_and_keep_the_post_shuffle_stream() {
        let mut game = Game::new_base(7).unwrap();
        let other = Game::new_base(42).unwrap();
        assert!(Arc::ptr_eq(&game.position.catalog, &other.position.catalog));
        let mut expected = Rng::new(7);
        // Fifteen Age 1 cards, ten in each of the other ten ages.
        for _ in 0..104 {
            expected.next_u64();
        }
        assert_eq!(game.rng, expected);
        let mut events = Vec::new();
        for _ in 0..2 {
            assert!(matches!(
                game.advance(100, &mut events).unwrap(),
                Status::Decision(_)
            ));
            game.choose(0).unwrap();
        }
        assert_eq!(
            game.advance(100, &mut events).unwrap(),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.rng, expected);
        let deck = Deck::base(1);
        let mut expected_deck = game.position.cards(Zone::Deck(deck)).to_vec();
        expected.shuffle(&mut expected_deck);
        game.start(Operation::Shuffle { deck }, Context::new(Player::One))
            .unwrap();
        assert!(matches!(
            game.advance(100, &mut events).unwrap(),
            Status::Complete(_)
        ));
        assert_eq!(game.position.cards(Zone::Deck(deck)), expected_deck);
        assert_eq!(game.rng, expected);
    }

    #[test]
    fn eight_catalog_caches_share_programs_and_leave_exact_native_rng_streams() {
        let e = Expansions::ECHOES;
        let f = Expansions::FIGURES;
        let c = Expansions::CITIES;
        for expansions in [
            Expansions::BASE,
            f,
            c,
            f.union(c),
            e,
            e.union(f),
            e.union(c),
            e.union(f).union(c),
        ] {
            for seed in [0, 7, u64::MAX] {
                let mut game = Game::new_game(seed, expansions).unwrap();
                let other = Game::new_game(seed.wrapping_add(1), expansions).unwrap();
                assert!(Arc::ptr_eq(&game.position.catalog, &other.position.catalog));
                let mut expected = Rng::new(seed);
                for _ in 0..104 * (1 + expansions.count()) {
                    expected.next_u64();
                }
                assert_eq!(game.rng, expected);
                let mut events = Vec::new();
                for _ in 0..2 {
                    assert!(matches!(
                        game.advance(100, &mut events).unwrap(),
                        Status::Decision(_)
                    ));
                    game.choose(0).unwrap();
                }
                assert_eq!(
                    game.advance(100, &mut events).unwrap(),
                    Status::Complete(ReturnValue::Unit)
                );
                assert_eq!(game.rng, expected, "opening consumes no hidden draws");
                assert_eq!(events.len(), 2);
            }
        }
    }
}
