use inno_engine::*;

const A: Player = Player::One;
const B: Player = Player::Two;

fn settle(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        let status = game.advance(100, events).unwrap();
        if status != Status::WorkLimit {
            return status;
        }
    }
    panic!("setup or action did not settle");
}

fn decision(game: &mut Game, events: &mut Vec<Event>) -> Decision {
    let Status::Decision(decision) = settle(game, events) else {
        panic!("expected an opening commitment");
    };
    decision
}

fn pick(game: &mut Game, decision: &Decision, card: CardId) {
    let expected = Alternative::Move {
        verb: Verb::Meld,
        target: TargetDescriptor::Known {
            card,
            source: Zone::Hand(decision.player),
        },
    };
    let index = decision
        .options
        .iter()
        .position(|option| *option == expected)
        .unwrap();
    game.choose(index).unwrap();
}

fn open(game: &mut Game, picks: [CardId; 2], events: &mut Vec<Event>) {
    for player in Player::ALL {
        let pending = decision(game, events);
        assert_eq!(pending.player, player);
        pick(game, &pending, picks[player.index()]);
    }
    assert_eq!(settle(game, events), Status::Complete(ReturnValue::Unit));
}

fn draw(game: &mut Game, events: &mut Vec<Event>) {
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert_eq!(settle(game, events), Status::Complete(ReturnValue::Unit));
}

#[test]
fn frozen_seed_seven_deal_reserves_and_physical_ids_match_retained_fixture() {
    // Independent frozen source: engine/tests/rng_parity.rs, seed 7.
    let game = Game::new_base(7).unwrap();
    let position = game.position();
    let ids = |raw: &[u16]| raw.iter().copied().map(CardId).collect::<Vec<_>>();
    assert_eq!(
        position.cards(Zone::Deck(Deck::base(1))),
        ids(&[1, 2, 4, 0, 11, 14, 8, 6, 13, 5])
    );
    assert_eq!(
        position.cards(Zone::Deck(Deck::base(10))),
        ids(&[95, 98, 99, 96, 100, 104, 103, 101, 102])
    );
    assert_eq!(
        position.cards(Zone::AvailableAchievements),
        ids(&[12, 15, 30, 37, 50, 61, 69, 82, 89, 97])
    );
    assert_eq!(position.cards(Zone::Hand(A)), ids(&[10, 9]));
    assert_eq!(position.cards(Zone::Hand(B)), ids(&[3, 7]));
    for (index, metadata) in BASE_METADATA.iter().enumerate() {
        let card = position.card(CardId(index as u16));
        assert_eq!(card, &base_card(metadata.code).unwrap());
        assert_eq!(card.value, metadata.value);
        assert!(card.figure.is_none());
    }
    assert!(position.check_conservation());
}

#[test]
fn all_real_cards_are_conserved_and_only_ages_one_through_ten_are_reserved() {
    for seed in [0, 1, 7, 42, u64::MAX] {
        let game = Game::new_base(seed).unwrap();
        let position = game.position();
        let mut all = Vec::new();
        let supply = position.cards(Zone::AvailableAchievements);
        assert_eq!(supply.len(), 10);
        assert_eq!(
            supply
                .iter()
                .map(|&id| position.card(id).value)
                .collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
        all.extend_from_slice(supply);
        for age in 0..=11 {
            let deck = position.cards(Zone::Deck(Deck::base(age)));
            assert_eq!(
                deck.len(),
                match age {
                    0 => 0,
                    1 | 11 => 10,
                    _ => 9,
                }
            );
            assert!(deck.iter().all(|&id| position.card(id).value == age));
            assert!(position.cards(Zone::Deck(Deck::figures(age))).is_empty());
            all.extend_from_slice(deck);
        }
        for player in Player::ALL {
            let hand = position.cards(Zone::Hand(player));
            assert_eq!(hand.len(), 2);
            assert!(hand.iter().all(|&id| position.card(id).value == 1));
            all.extend_from_slice(hand);
            assert!(position.cards(Zone::Score(player)).is_empty());
            assert!(position.cards(Zone::Achievements(player)).is_empty());
            for color in Color::ALL {
                assert!(position.cards(Zone::Board(player, color)).is_empty());
            }
        }
        all.sort();
        assert_eq!(all, (0..115).map(CardId).collect::<Vec<_>>());
        assert!(position.cards(Zone::AgeZeroReserve).is_empty());
        assert!(!position.age_zero_introduced());
        assert!(position.cards(Zone::Junk).is_empty());
        assert!(
            Special::ALL
                .into_iter()
                .all(|special| position.special_location(special) == SpecialLocation::Available)
        );
        assert!(position.check_conservation());
    }
}

#[test]
fn opening_commitments_are_private_until_both_players_choose() {
    let mut left = Game::new_base(7).unwrap();
    let mut right = left.clone();
    let mut left_events = Vec::new();
    let mut right_events = Vec::new();
    let first = decision(&mut left, &mut left_events);
    assert_eq!(first, decision(&mut right, &mut right_events));
    assert_eq!(first.player, A);
    assert_eq!(first.options.len(), 2);
    pick(&mut left, &first, CardId(10));
    pick(&mut right, &first, CardId(9));
    let before_reveal = left.position().clone();
    let second = decision(&mut left, &mut left_events);
    assert_eq!(second, decision(&mut right, &mut right_events));
    assert_eq!(second.player, B);
    assert_eq!(second.options.len(), 2);
    for card in [CardId(3), CardId(7)] {
        assert!(second.options.contains(&Alternative::Move {
            verb: Verb::Meld,
            target: TargetDescriptor::Known {
                card,
                source: Zone::Hand(B)
            },
        }));
    }
    assert_eq!(left.position(), right.position());
    assert!(left_events.is_empty());
    assert!(right_events.is_empty());
    for viewer in Player::ALL {
        assert_eq!(left.pending_melds(viewer), right.pending_melds(viewer));
        assert!(left.pending_melds(viewer).is_empty());
    }
    // The real hand-view descriptor does not expose even the committed front.
    assert!(matches!(
        left.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .descriptor(
                Target {
                    card: CardId(10),
                    physical: Zone::Hand(A),
                    logical: LogicalZone::Hand(A),
                },
                B
            ),
        TargetDescriptor::Hidden { .. }
    ));
    pick(&mut left, &second, CardId(3));
    assert_eq!(left.position(), &before_reveal);
    assert_eq!(
        left.advance(1, &mut left_events).unwrap(),
        Status::WorkLimit
    );
    assert_eq!(left_events.len(), 2);
    assert_eq!(left.position().top(A, Color::Blue), Some(CardId(10)));
    assert_eq!(left.position().top(B, Color::Yellow), Some(CardId(3)));
    assert_eq!(
        settle(&mut left, &mut left_events),
        Status::Complete(ReturnValue::Unit)
    );
}

#[test]
fn starting_melds_are_not_actions_and_title_order_not_id_order_selects_the_starter() {
    // Seed 7 gives TOOLS/POTTERY to A and AGRICULTURE/SAILING to B.
    // Agriculture's physical ID is lower in both cases; POTTERY still beats SAILING.
    for (picks, first) in [
        ([CardId(10), CardId(3)], B),
        ([CardId(10), CardId(7)], B),
        ([CardId(9), CardId(3)], B),
        ([CardId(9), CardId(7)], A),
    ] {
        let mut game = Game::new_base(7).unwrap();
        let mut events = Vec::new();
        open(&mut game, picks, &mut events);
        assert_eq!(game.position().turn_owner(), first);
        assert_eq!(game.position().regular_actions_started(), 0);
        assert_eq!(events.len(), 2, "only the two starting melds");
        for player in Player::ALL {
            let card = picks[player.index()];
            assert_eq!(
                events[player.index()],
                Event::Moved(Movement {
                    verb: Some(Verb::Meld),
                    card,
                    source: Zone::Hand(player),
                    destination: Zone::Board(player, game.position().card(card).color),
                    executor: player,
                    inside_karma: false,
                })
            );
            assert_eq!(game.position().cards(Zone::Hand(player)).len(), 1);
            assert!(game.position().cards(Zone::Score(player)).is_empty());
            assert!(game.position().cards(Zone::Achievements(player)).is_empty());
        }
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))).len(), 10);
        assert_eq!(game.position().cards(Zone::AvailableAchievements).len(), 10);
        assert!(game.position().check_conservation());
        game.take_action(Action::Draw, ActionKind::Regular).unwrap();
        assert_eq!(game.position().regular_actions_started(), 1);
        assert_eq!(game.position().turn_owner(), first);
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().turn_owner(), first.opponent());
        assert_eq!(game.position().regular_actions_started(), 0);
        draw(&mut game, &mut events);
        assert_eq!(game.position().turn_owner(), first.opponent());
        assert_eq!(game.position().regular_actions_started(), 1);
        draw(&mut game, &mut events);
        assert_eq!(game.position().turn_owner(), first);
        assert_eq!(game.position().regular_actions_started(), 0);
        draw(&mut game, &mut events);
        assert_eq!(game.position().turn_owner(), first);
        assert_eq!(game.position().regular_actions_started(), 1);
        draw(&mut game, &mut events);
        assert_eq!(game.position().turn_owner(), first.opponent());
        assert!(game.position().check_conservation());
    }
}

#[test]
fn invalid_opening_choices_and_premature_actions_are_atomic() {
    let mut game = Game::new_base(7).unwrap();
    let before = game.clone();
    assert_eq!(game.choose(0), Err(Error::NoDecision));
    assert_eq!(game, before);
    let mut events = Vec::new();
    for card in [CardId(10), CardId(3)] {
        let pending = decision(&mut game, &mut events);
        let before = game.clone();
        assert_eq!(
            game.choose(pending.options.len()),
            Err(Error::InvalidChoice)
        );
        assert_eq!(game, before);
        assert_eq!(game.choose(usize::MAX), Err(Error::InvalidChoice));
        assert_eq!(game, before);
        assert_eq!(
            game.take_action(Action::Draw, ActionKind::Regular),
            Err(Error::Busy)
        );
        assert_eq!(game, before);
        assert_eq!(game.begin_turn(B), Err(Error::Busy));
        assert_eq!(game, before);
        pick(&mut game, &pending, card);
    }
    let before = game.clone();
    assert_eq!(
        game.take_action(Action::Draw, ActionKind::Regular),
        Err(Error::Busy)
    );
    assert_eq!(game, before);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
}

#[test]
fn deterministic_setup_clones_resume_at_every_opening_work_boundary() {
    for seed in [0, 7, 42, u64::MAX] {
        let mut game = Game::new_base(seed).unwrap();
        assert_eq!(game, Game::new_base(seed).unwrap());
        let mut events = Vec::new();
        let mut commitments = 0;
        loop {
            let mut cloned = game.clone();
            let mut cloned_events = Vec::new();
            let mut step_events = Vec::new();
            let status = game.advance(1, &mut step_events).unwrap();
            assert_eq!(status, cloned.advance(1, &mut cloned_events).unwrap());
            assert_eq!(game, cloned);
            assert_eq!(step_events, cloned_events);
            events.extend(step_events);
            assert!(game.position().check_conservation());
            match status {
                Status::Decision(pending) => {
                    let option = commitments % pending.options.len();
                    game.choose(option).unwrap();
                    cloned.choose(option).unwrap();
                    assert_eq!(game, cloned);
                    commitments += 1;
                }
                Status::Complete(ReturnValue::Unit) => break,
                Status::WorkLimit => {}
                other => panic!("unexpected setup status: {other:?}"),
            }
        }
        assert_eq!(commitments, 2);
        assert_eq!(events.len(), 2);
        let mut cloned = game.clone();
        let mut clone_events = Vec::new();
        events.clear();
        draw(&mut game, &mut events);
        draw(&mut cloned, &mut clone_events);
        assert_eq!(game, cloned);
        assert_eq!(events, clone_events);
    }
}
