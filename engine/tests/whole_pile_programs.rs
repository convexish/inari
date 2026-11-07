use inno_engine::*;
mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("whole-pile program failed to finish");
}

#[test]
fn bicycle_preserves_optional_decline_and_commits_one_atomic_exchange() {
    let mut f = Fixture::default();
    let bicycle = f.add(base_card("B070").unwrap(), Zone::Board(A, Color::Green));
    let hand = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let score = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let initial = f.position();
    let mut game = Game::new(initial.clone(), Expansions::BASE).unwrap();
    game.self_execute(bicycle, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        decision(&mut game, &mut events).options,
        [
            Alternative::Exchange {
                left: LogicalZone::Hand(A),
                right: LogicalZone::Score(A)
            },
            Alternative::Decline,
        ]
    );
    let mut declined = game.clone();
    declined.choose(1).unwrap();
    assert_eq!(
        settle(&mut declined, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(declined.position(), &initial);
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[score]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[hand]);
    assert!(
        matches!(&events[..], [Event::Exchanged { movements, inside_karma: false, .. }] if movements.len() == 2)
    );
}

#[test]
fn bicycle_allows_one_empty_side_but_skips_an_entirely_overlapping_exchange() {
    for physical_score in [false, true] {
        let mut f = Fixture::default();
        let bicycle = f.add(base_card("B070").unwrap(), Zone::Board(A, Color::Green));
        f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
        f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
        let hand = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        if !physical_score {
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.self_execute(bicycle, Context::new(A)).unwrap();
            let mut events = Vec::new();
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert!(events.is_empty());
            assert_eq!(game.position().cards(Zone::Hand(A)), &[hand]);
        } else {
            // No replacement source: the overlap stays put and only score moves.
            let yi = f
                .cards
                .iter()
                .position(|c| c.figure == Some(Figure::YiSunSin))
                .unwrap();
            f.placements[yi].1 = Zone::Deck(Deck::figures(4));
            let score = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.self_execute(bicycle, Context::new(A)).unwrap();
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(game.position().cards(Zone::Hand(A)), &[hand, score]);
            assert!(game.position().cards(Zone::Score(A)).is_empty());
            assert!(
                matches!(&events[..], [Event::Exchanged { movements, .. }] if movements.len() == 1 && movements[0].card == score)
            );
        }
    }
    let mut f = Fixture::default();
    let bicycle = f.add(base_card("B070").unwrap(), Zone::Board(A, Color::Green));
    let score = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(bicycle, Context::new(A)).unwrap();
    assert_eq!(
        finish(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[score]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn bicycle_moves_borrowed_physical_cards_once_without_exposing_them_in_the_choice() {
    let mut f = Fixture::default();
    let bicycle = f.add(base_card("B070").unwrap(), Zone::Board(A, Color::Green));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let own = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let borrowed = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let score = f.tech(Color::Yellow, 3, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(bicycle, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        decision(&mut game, &mut events).options,
        [
            Alternative::Exchange {
                left: LogicalZone::Hand(A),
                right: LogicalZone::Score(A)
            },
            Alternative::Decline,
        ]
    );
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[score]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position().cards(Zone::Score(A)), &[own, borrowed]);
    assert!(game.position().check_conservation());
}

#[test]
fn bicycle_choice_precedes_yis_single_exchange_replacement() {
    let mut f = Fixture::default();
    let bicycle = f.add(base_card("B070").unwrap(), Zone::Board(A, Color::Green));
    let yi = f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let hand = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let score = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let reward = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(bicycle, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        decision(&mut game, &mut events).options[0],
        Alternative::Exchange { .. }
    ));
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        game.position().cards(Zone::Board(B, Color::Red)),
        &[hand, yi]
    );
    assert_eq!(game.position().cards(Zone::Board(B, Color::Blue)), &[score]);
    assert_eq!(
        game.position().cards(Zone::Board(B, Color::Green)),
        &[reward]
    );
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert!(events.iter().all(|e| !matches!(e, Event::Exchanged { .. })));
}

#[test]
fn stem_cells_commits_all_scoring_or_declines_but_always_runs_its_separate_draw() {
    for decline in [false, true] {
        let mut f = Fixture::default();
        let source = f.add(base_card("B099").unwrap(), Zone::Board(A, Color::Yellow));
        let hand: Vec<_> = [1, 2, 4]
            .into_iter()
            .map(|value| f.tech(Color::Red, value, Zone::Hand(A), vec![]))
            .collect();
        let drawn = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        game.choose(if decline {
            menu.options
                .iter()
                .position(|o| *o == Alternative::Decline)
                .unwrap()
        } else {
            0
        })
        .unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if decline {
            let mut expected = hand;
            expected.push(drawn);
            assert_eq!(game.position().cards(Zone::Hand(A)), expected);
            assert!(game.position().cards(Zone::Score(A)).is_empty());
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                [drawn]
            );
        } else {
            assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
            assert_eq!(game.position().cards(Zone::Score(A)), hand);
            let mut expected = hand;
            expected.push(drawn);
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                expected
            );
        }
    }
}

#[test]
fn stem_cells_replaced_scores_are_considered_once_and_do_not_cancel_the_draw() {
    let mut f = Fixture::default();
    let source = f.add(base_card("B099").unwrap(), Zone::Board(A, Color::Yellow));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    let mut hand: Vec<_> = [1, 2, 4]
        .into_iter()
        .map(|value| f.tech(Color::Red, value, Zone::Hand(A), vec![]))
        .collect();
    let drawn = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events[..hand.len()],
        hand.iter()
            .map(|&card| Event::Revealed {
                card,
                source: Zone::Hand(A),
                executor: A,
                inside_karma: true
            })
            .collect::<Vec<_>>()
    );
    let events = &events[hand.len()..];
    hand.push(drawn);
    assert_eq!(game.position().cards(Zone::Hand(A)), hand);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [drawn]
    );
}
