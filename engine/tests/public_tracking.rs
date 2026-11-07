//! Public history must survive hidden-world sampling without importing either
//! player's private observations.
use inno_engine::*;

mod common;
use common::*;

fn run(game: &mut Game, operation: Operation, executor: Player) {
    game.start(operation, Context::new(executor)).unwrap();
    assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
    assert!(game.public_tracker().unwrap().check_truth(game.position()));
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
    }
}

fn move_card(game: &mut Game, card: CardId, source: Zone, destination: Zone, executor: Player) {
    run(
        game,
        Operation::Move {
            verb: Verb::Transfer,
            target: target(card, source),
            destination,
        },
        executor,
    );
}

fn hands() -> Game {
    let mut f = Fixture::default();
    for (index, code) in ["B001", "B002", "B003", "B004", "B005", "B006"]
        .into_iter()
        .enumerate()
    {
        let zone = match index {
            0..=1 => Zone::Hand(A),
            2..=3 => Zone::Hand(B),
            _ => Zone::Deck(Deck::base(1)),
        };
        f.add(base_card(code).unwrap(), zone);
    }
    Game::new(f.position(), Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap()
}

fn public_history(game: &Game) -> Vec<Vec<BeliefSlot>> {
    let mut zones = vec![
        Zone::Junk,
        Zone::AvailableAchievements,
        Zone::AgeZeroReserve,
    ];
    for player in Player::ALL {
        zones.extend([
            Zone::Hand(player),
            Zone::Score(player),
            Zone::Achievements(player),
        ]);
        zones.extend(Color::ALL.map(|color| Zone::Board(player, color)));
    }
    for set in CardSet::ALL {
        zones.extend((0..Deck::AGES as u8).map(|age| Zone::Deck(Deck { set, age })));
    }
    let public = game.public_tracker().unwrap();
    assert!(public.check_truth(game.position()));
    zones.into_iter().map(|zone| public.belief(zone)).collect()
}

#[test]
fn a_publicly_revealed_next_draw_survives_opponent_reconstruction() {
    let mut f = Fixture::default();
    let deck = Zone::Deck(Deck::base(1));
    let cards: Vec<_> = ["B001", "B002", "B003", "B004", "B005"]
        .into_iter()
        .map(|code| f.add(base_card(code).unwrap(), deck))
        .collect();
    let top = cards[4];
    let mut game = Game::new(f.position(), Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    run(
        &mut game,
        Operation::Reveal {
            target: target(top, deck),
        },
        A,
    );
    for viewer in Player::ALL {
        assert_eq!(
            game.tracker(viewer).unwrap().belief(deck)[4].known(),
            Some(top)
        );
        for seed in 0..8 {
            let world = game.determinize(viewer, seed).unwrap();
            for observer in Player::ALL {
                assert_eq!(
                    world.tracker(observer).unwrap().belief(deck)[4].known(),
                    Some(top)
                );
                assert_eq!(
                    world.observe(observer).unwrap(),
                    game.observe(observer).unwrap()
                );
            }
            let nested = world.determinize(viewer.opponent(), seed + 8).unwrap();
            for observer in Player::ALL {
                assert_eq!(
                    nested.tracker(observer).unwrap().belief(deck)[4].known(),
                    Some(top)
                );
            }
        }
    }
}

#[test]
fn private_cards_stay_unknown_to_the_public_but_cross_player_transfers_are_shared() {
    let mut game = hands();
    assert_eq!(game.public_tracker().unwrap().viewer(), None);
    for viewer in Player::ALL {
        assert_eq!(game.tracker(viewer).unwrap().viewer(), Some(viewer));
        assert!(
            game.tracker(viewer)
                .unwrap()
                .belief(Zone::Hand(viewer))
                .iter()
                .all(|s| s.known().is_some())
        );
        assert!(
            game.public_tracker()
                .unwrap()
                .belief(Zone::Hand(viewer))
                .iter()
                .all(|s| s.known().is_none())
        );
    }
    // Both participants see the transferred face without inspecting either
    // whole hand. It must therefore enter common knowledge.
    move_card(&mut game, CardId(0), Zone::Hand(A), Zone::Hand(B), A);
    let public = game.public_tracker().unwrap().belief(Zone::Hand(B));
    assert_eq!(
        public
            .iter()
            .filter_map(BeliefSlot::known)
            .collect::<Vec<_>>(),
        [CardId(0)]
    );
    for viewer in Player::ALL {
        let world = game.determinize(viewer, 3).unwrap();
        assert_eq!(
            world.public_tracker().unwrap().belief(Zone::Hand(B)),
            public
        );
        assert!(
            world
                .tracker(viewer.opponent())
                .unwrap()
                .belief(Zone::Hand(B))
                .iter()
                .any(|s| s.known() == Some(CardId(0)))
        );
    }
}

#[test]
fn publicly_known_hand_members_follow_identities_through_anonymous_permutations() {
    let mut game = hands();
    let hand = Zone::Hand(B);
    for card in [CardId(2), CardId(3)] {
        run(
            &mut game,
            Operation::Reveal {
                target: target(card, hand),
            },
            B,
        );
    }
    let public = game.public_tracker().unwrap().belief(hand);
    assert_eq!(
        public
            .iter()
            .filter_map(BeliefSlot::known)
            .collect::<Vec<_>>(),
        [CardId(2), CardId(3)]
    );
    let mut orders = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let world = game.determinize(A, seed).unwrap();
        orders.insert(world.position().cards(hand).to_vec());
        assert_eq!(world.public_tracker().unwrap().belief(hand), public);
        assert!(
            world
                .public_tracker()
                .unwrap()
                .check_truth(world.position())
        );
        let nested = world.determinize(B, seed + 20).unwrap();
        assert_eq!(nested.public_tracker().unwrap().belief(hand), public);
        assert!(
            nested
                .public_tracker()
                .unwrap()
                .check_truth(nested.position())
        );
    }
    assert_eq!(
        orders.len(),
        2,
        "public membership does not label hidden hand slots"
    );
}

#[test]
fn ambiguous_private_departures_do_not_import_the_real_players_memory() {
    let make = |moved| {
        let mut game = hands();
        run(
            &mut game,
            Operation::Reveal {
                target: target(CardId(0), Zone::Hand(A)),
            },
            A,
        );
        move_card(
            &mut game,
            moved,
            Zone::Hand(A),
            Zone::Deck(Deck::base(1)),
            A,
        );
        game
    };
    let left = make(CardId(0));
    let right = make(CardId(1));
    let deck = Zone::Deck(Deck::base(1));
    assert_ne!(left.tracker(A), right.tracker(A));
    assert_eq!(left.tracker(B), right.tracker(B));
    assert_eq!(
        left.public_tracker().unwrap().belief(deck),
        right.public_tracker().unwrap().belief(deck)
    );
    let mut uncertain = 0;
    for seed in 0..24 {
        let l = left.determinize(B, seed).unwrap();
        let r = right.determinize(B, seed).unwrap();
        assert_eq!(
            l, r,
            "a private departure's real identity must not reach any simulated belief"
        );
        for viewer in Player::ALL {
            assert_eq!(l.observe(viewer).unwrap(), r.observe(viewer).unwrap());
        }
        uncertain += usize::from(
            l.tracker(A)
                .unwrap()
                .belief(deck)
                .last()
                .unwrap()
                .known()
                .is_none(),
        );
        assert_eq!(
            l.public_tracker().unwrap().belief(deck),
            left.public_tracker().unwrap().belief(deck)
        );
    }
    assert!(
        uncertain > 0,
        "the simulated player must not inherit its real private memory"
    );
}

#[test]
fn a_public_return_is_remembered_and_a_shuffle_forgets_its_position() {
    let mut game = hands();
    let board = Zone::Board(A, game.position().card(CardId(0)).color);
    let deck = Zone::Deck(Deck::base(1));
    move_card(&mut game, CardId(0), Zone::Hand(A), board, A);
    run(
        &mut game,
        Operation::Move {
            verb: Verb::Return,
            target: target(CardId(0), board),
            destination: deck,
        },
        A,
    );
    assert_eq!(
        game.public_tracker().unwrap().belief(deck)[0].known(),
        Some(CardId(0))
    );
    for viewer in Player::ALL {
        let world = game.determinize(viewer, 4).unwrap();
        assert_eq!(
            world.tracker(viewer.opponent()).unwrap().belief(deck)[0].known(),
            Some(CardId(0))
        );
    }
    run(
        &mut game,
        Operation::Shuffle {
            deck: Deck::base(1),
        },
        A,
    );
    let public = game.public_tracker().unwrap().belief(deck);
    assert!(
        public
            .iter()
            .all(|s| s.known().is_none() && s.candidates.contains(&CardId(0)))
    );
    assert_eq!(
        game.determinize(B, 5)
            .unwrap()
            .public_tracker()
            .unwrap()
            .belief(deck),
        public
    );
}

#[test]
fn every_supported_mode_preserves_public_history_through_nested_samples_and_play() {
    let mut samples = 0;
    for expansions in [
        Expansions::BASE,
        Expansions::FIGURES,
        Expansions::CITIES,
        Expansions::FIGURES.union(Expansions::CITIES),
        Expansions::ARTIFACTS,
        Expansions::ARTIFACTS.union(Expansions::FIGURES),
        Expansions::ARTIFACTS
            .union(Expansions::ECHOES)
            .union(Expansions::FIGURES)
            .union(Expansions::CITIES),
    ] {
        for seed in 0..2 {
            let mut game = Game::new_game(seed, expansions)
                .unwrap()
                .with_tracking()
                .unwrap();
            let mut rng = Rng::new(seed ^ 0xC0110);
            for step in 0..400 {
                let status = game.advance(1 << 16, &mut Vec::new()).unwrap();
                let viewer = match &status {
                    Status::Decision(menu) => menu.player,
                    Status::Complete(_) => game.position().turn_owner(),
                    _ => break,
                };
                if step % 11 == 0 {
                    let mut world = game.determinize(viewer, rng.next_u64()).unwrap();
                    assert_eq!(public_history(&world), public_history(&game));
                    assert_eq!(
                        world.observe(viewer).unwrap(),
                        game.observe(viewer).unwrap()
                    );
                    let next_viewer = if matches!(status, Status::Decision(_)) {
                        viewer
                    } else {
                        viewer.opponent()
                    };
                    let nested = world.determinize(next_viewer, rng.next_u64()).unwrap();
                    assert_eq!(public_history(&nested), public_history(&world));
                    assert_eq!(
                        nested.observe(next_viewer).unwrap(),
                        world.observe(next_viewer).unwrap()
                    );
                    match status {
                        Status::Decision(_) => world.choose(0).unwrap(),
                        _ => world
                            .take_action(world.legal_actions()[0], ActionKind::Regular)
                            .unwrap(),
                    }
                    world.advance(1 << 16, &mut Vec::new()).unwrap();
                    assert!(
                        world
                            .public_tracker()
                            .unwrap()
                            .check_truth(world.position())
                    );
                    for observer in Player::ALL {
                        assert!(
                            world
                                .tracker(observer)
                                .unwrap()
                                .check_truth(world.position())
                        );
                    }
                    samples += 1;
                }
                match status {
                    Status::Decision(menu) => game
                        .choose((rng.next_u64() % menu.options.len() as u64) as usize)
                        .unwrap(),
                    _ => {
                        let actions = game.legal_actions();
                        let action = actions[(rng.next_u64() % actions.len() as u64) as usize];
                        game.take_action(action, ActionKind::Regular).unwrap();
                    }
                }
            }
        }
    }
    assert!(
        samples > 100,
        "each mode must exercise sampling and continued play"
    );
}
