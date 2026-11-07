//! Complete authored Echoes setup, preserving the existing four modes.
use inno_engine::*;

const A: Player = Player::One;
const B: Player = Player::Two;
const E: Expansions = Expansions::ECHOES;
const F: Expansions = Expansions::FIGURES;
const C: Expansions = Expansions::CITIES;
const MODES: [Expansions; 8] = [
    Expansions::BASE,
    F,
    C,
    F.union(C),
    E,
    E.union(F),
    E.union(C),
    E.union(F).union(C),
];

fn settle(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        let mut clone = game.clone();
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(clone.advance(1, &mut expected).unwrap(), status);
        assert_eq!(actual, expected);
        assert_eq!(*game, clone);
        events.extend(actual);
        assert!(game.position().check_conservation());
        for player in Player::ALL {
            if let Some(tracker) = game.tracker(player) {
                assert!(tracker.check_truth(game.position()));
            }
        }
        if status != Status::WorkLimit {
            return status;
        }
    }
    panic!("setup probe did not settle");
}

fn open(game: &mut Game) {
    let mut events = Vec::new();
    for player in Player::ALL {
        let Status::Decision(menu) = settle(game, &mut events) else {
            panic!("opening choice expected");
        };
        assert_eq!(menu.player, player);
        game.choose(0).unwrap();
    }
    assert_eq!(
        settle(game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 2);
}

#[test]
fn complete_echoes_catalog_has_all_printed_programs_and_no_empty_substitutes() {
    assert_eq!(ECHOES_METADATA.len(), 115);
    let mut echoes = 0;
    let mut dogmas = 0;
    let mut demands = 0;
    for metadata in ECHOES_METADATA {
        let card = echoes_card(metadata.code)
            .unwrap_or_else(|error| panic!("{}: {error:?}", metadata.code));
        assert_eq!(card.code, metadata.code);
        assert_eq!(card.color, metadata.color);
        assert_eq!(card.value, metadata.value);
        assert_eq!(card.set, CardSet::Echoes);
        assert_eq!(card.printed(), metadata.symbols);
        assert_eq!(card.featured, metadata.featured);
        assert_eq!(
            card.echo.is_some(),
            metadata.symbols.contains(&Symbol::Echo)
        );
        if let Some(echo) = &card.echo {
            assert!(!echo.demand());
            assert!(!echo.instructions.is_empty(), "{}", card.code);
            echoes += 1;
        }
        assert!(!card.effects.is_empty(), "{}", card.code);
        for effect in &card.effects {
            assert!(!effect.instructions.is_empty(), "{}", card.code);
            dogmas += 1;
            demands += usize::from(effect.demand());
        }
    }
    assert_eq!((echoes, dogmas, demands), (63, 200, 35));
}

#[test]
fn all_eight_catalogs_preserve_existing_ids_and_deal_only_base_cards() {
    for seed in [0, 7, 42] {
        for without_echoes in MODES[..4].iter().copied() {
            let before = Game::new_game(seed, without_echoes).unwrap();
            let after = Game::new_game(seed, without_echoes.union(E)).unwrap();
            let prior_len = 115
                + 130 * usize::from(without_echoes.figures())
                + 115 * usize::from(without_echoes.cities());
            for i in 0..prior_len {
                let id = CardId(i as u16);
                assert_eq!(before.position().card(id), after.position().card(id));
            }
            let ids: Vec<_> = (0..=11)
                .flat_map(|age| {
                    after
                        .position()
                        .cards(Zone::Deck(Deck {
                            set: CardSet::Echoes,
                            age,
                        }))
                        .iter()
                        .copied()
                })
                .collect();
            let mut ids = ids;
            ids.sort();
            assert_eq!(
                ids,
                (prior_len..prior_len + 115)
                    .map(|i| CardId(i as u16))
                    .collect::<Vec<_>>()
            );
        }
        let base = Game::new_base(seed).unwrap();
        for expansions in MODES {
            let game = Game::new_game(seed, expansions).unwrap();
            let p = game.position();
            assert_eq!(p.expansions(), expansions);
            for zone in [Zone::Hand(A), Zone::Hand(B), Zone::AvailableAchievements]
                .into_iter()
                .chain((0..=11).map(|age| Zone::Deck(Deck::base(age))))
            {
                assert_eq!(p.cards(zone), base.position().cards(zone));
                assert!(
                    p.cards(zone)
                        .iter()
                        .all(|&card| p.card(card).set == CardSet::Base)
                );
            }
            for set in CardSet::ALL.into_iter().filter(|set| *set != CardSet::Base) {
                let count: usize = (0..=11)
                    .map(|age| p.cards(Zone::Deck(Deck { set, age })).len())
                    .sum();
                assert_eq!(count, if expansions.includes(set) { 115 } else { 0 });
            }
            for player in Player::ALL {
                assert!(p.cards(Zone::Forecast(player)).is_empty());
                assert!(p.cards(Zone::Score(player)).is_empty());
                assert!(p.cards(Zone::Achievements(player)).is_empty());
                assert_eq!(p.cards(Zone::Hand(player)).len(), 2);
                for color in Color::ALL {
                    assert!(p.cards(Zone::Board(player, color)).is_empty());
                }
            }
            assert_eq!(
                p.cards(Zone::AgeZeroReserve).len(),
                if expansions.figures() { 15 } else { 0 }
            );
            assert!(!p.age_zero_introduced());
            assert!(p.cards(Zone::Junk).is_empty());
            assert!(p.check_conservation());
        }
    }
}

#[test]
fn expansion_shuffles_follow_base_then_echoes_cities_figures_in_ascending_ages() {
    // Independent Fisher-Yates expectation using the frozen native stream.
    fn shuffle<T>(rng: &mut Rng, cards: &mut [T]) {
        for i in (1..cards.len()).rev() {
            let j = (rng.next_u64() % (i + 1) as u64) as usize;
            cards.swap(i, j);
        }
    }
    // Frozen direct native Rust output: all_catalog()[physical_id].code.
    // Adding Figures must preserve every Base,
    // Echoes and Cities deck because its shuffles occur after these sets.
    for expansions in MODES {
        let game = Game::new_game(7, expansions).unwrap();
        let mode = match (expansions.echoes(), expansions.cities()) {
            (false, false) => "base",
            (false, true) => "cities",
            (true, false) => "echoes",
            (true, true) => "echoes+cities",
        };
        let mut checked = 0;
        for line in include_str!("fixtures/echoes_setup_seed7.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 3);
            if fields[0] != mode {
                continue;
            }
            let set = match &fields[1][..1] {
                "B" => CardSet::Base,
                "E" => CardSet::Echoes,
                "C" => CardSet::Cities,
                _ => panic!("unknown fixture deck"),
            };
            let age = fields[1][1..].parse().unwrap();
            let actual = game
                .position()
                .cards(Zone::Deck(Deck { set, age }))
                .iter()
                .map(|&id| game.position().card(id).code)
                .collect::<Vec<_>>();
            let expected: Vec<_> = if fields[2] == "-" {
                Vec::new()
            } else {
                fields[2].split(',').collect()
            };
            assert_eq!(actual, expected, "{expansions:?} {}", fields[1]);
            checked += 1;
        }
        assert_eq!(
            checked,
            12 * (1 + usize::from(expansions.echoes()) + usize::from(expansions.cities()))
        );
    }
    for seed in [0, 7, u64::MAX] {
        for expansions in MODES {
            let game = Game::new_game(seed, expansions).unwrap();
            let position = game.position();
            let mut rng = Rng::new(seed);
            // Base's fifteen age-one cards and ten cards in each other age.
            for _ in 0..104 {
                rng.next_u64();
            }
            for set in [CardSet::Echoes, CardSet::Cities, CardSet::Figures] {
                if !expansions.includes(set) {
                    continue;
                }
                for age in 1..=11 {
                    let mut expected: Vec<_> = match set {
                        CardSet::Echoes if age == 1 => vec![
                            "E001", "E012", "E013", "E002", "E003", "E004", "E005", "E006", "E007",
                            "E008", "E009", "E010", "E011", "E014", "E015",
                        ],
                        CardSet::Echoes => ECHOES_METADATA
                            .iter()
                            .filter(|m| m.value == age)
                            .map(|m| m.code)
                            .collect(),
                        CardSet::Cities => CITY_METADATA
                            .iter()
                            .filter(|m| m.value == age)
                            .map(|m| m.code)
                            .collect(),
                        CardSet::Figures => FIGURE_METADATA
                            .iter()
                            .filter(|m| m.value == age)
                            .map(|m| m.code)
                            .collect(),
                        _ => unreachable!(),
                    };
                    shuffle(&mut rng, &mut expected);
                    let actual: Vec<_> = position
                        .cards(Zone::Deck(Deck { set, age }))
                        .iter()
                        .map(|&card| position.card(card).code)
                        .collect();
                    assert_eq!(actual, expected, "{seed} {expansions:?} {set:?} {age}");
                }
            }
        }
    }
}

#[test]
fn echo_modes_keep_opening_commitments_private_and_reveal_only_two_base_melds() {
    for expansions in MODES[4..].iter().copied() {
        let mut left = Game::new_game(7, expansions)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut right = left.clone();
        let mut left_events = Vec::new();
        let mut right_events = Vec::new();
        let Status::Decision(first) = settle(&mut left, &mut left_events) else {
            panic!("first commitment");
        };
        assert_eq!(
            settle(&mut right, &mut right_events),
            Status::Decision(first)
        );
        left.choose(0).unwrap();
        right.choose(1).unwrap();
        let next = settle(&mut left, &mut left_events);
        assert_eq!(next, settle(&mut right, &mut right_events));
        let Status::Decision(second) = next else {
            panic!("second commitment");
        };
        assert_eq!(second.player, B);
        assert_eq!(left.position(), right.position());
        assert_eq!(left.observe(B).unwrap(), right.observe(B).unwrap());
        assert!(left_events.is_empty() && right_events.is_empty());
        let status_before = left.clone();
        assert_eq!(
            left.take_action(Action::Draw, ActionKind::Regular),
            Err(Error::Busy)
        );
        assert_eq!(left, status_before);
        left.choose(0).unwrap();
        right.choose(0).unwrap();
        for (game, events) in [
            (&mut left, &mut left_events),
            (&mut right, &mut right_events),
        ] {
            assert_eq!(settle(game, events), Status::Complete(ReturnValue::Unit));
            assert_eq!(events.len(), 2);
            assert_eq!(game.position().turn_owner(), B); // Agriculture precedes Tools/Pottery.
            assert_eq!(game.position().regular_actions_started(), 0);
            for event in events.iter() {
                let Event::Moved(movement) = event else {
                    panic!("only the starting melds");
                };
                assert_eq!(movement.verb, Some(Verb::Meld));
                assert_eq!(game.position().card(movement.card).set, CardSet::Base);
                assert!(!movement.inside_karma);
            }
            for player in Player::ALL {
                assert_eq!(game.position().cards(Zone::Hand(player)).len(), 1);
                assert!(game.position().cards(Zone::Forecast(player)).is_empty());
            }
        }
    }
}

#[test]
fn full_setups_enable_only_their_specials_and_win_at_six_plus_expansions() {
    for expansions in MODES {
        let mut game = Game::new_game(7, expansions)
            .unwrap()
            .with_tracking()
            .unwrap();
        let expected: Vec<_> = Special::ALL
            .into_iter()
            .chain(Special::ECHOES.into_iter().filter(|_| expansions.echoes()))
            .chain(Special::CITIES.into_iter().filter(|_| expansions.cities()))
            .chain(Special::UNSEEN.into_iter().filter(|_| expansions.unseen()))
            .collect();
        assert_eq!(game.position().specials().collect::<Vec<_>>(), expected);
        assert!(
            expected
                .into_iter()
                .all(|special| game.position().special_location(special)
                    == SpecialLocation::Available)
        );
        if expansions.figures() {
            for decree in Decree::ALL {
                assert_eq!(
                    game.position().decree_location(decree),
                    DecreeLocation::Supply
                );
            }
        }
        open(&mut game);
        let threshold = 6 + expansions.count();
        for count in 1..=threshold {
            let card = game.position().cards(Zone::AvailableAchievements)[0];
            game.start(
                Operation::Claim {
                    target: Target {
                        card,
                        physical: Zone::AvailableAchievements,
                        logical: LogicalZone::AvailableAchievements(A),
                    },
                    eligibility: ClaimEligibility::Ignore,
                },
                Context::new(A),
            )
            .unwrap();
            assert!(matches!(
                settle(&mut game, &mut Vec::new()),
                Status::Complete(_)
            ));
            assert_eq!(game.position().achievement_count(A), count);
            game.start(Operation::CheckAchievementVictory, Context::new(A))
                .unwrap();
            let status = settle(&mut game, &mut Vec::new());
            if count == threshold {
                assert_eq!(status, Status::Winner(A));
                assert_eq!(game.victory().unwrap().reason, WinReason::Achievements);
            } else {
                assert!(matches!(status, Status::Complete(_)));
            }
        }
    }
}
