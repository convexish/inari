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
    panic!("When reaction did not finish");
}

#[test]
fn first_then_meld_then_when_are_distinct_scopes_and_newly_exposed_tops_are_scored() {
    let mut f = Fixture::default();
    let before = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
    let buried = f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let kalashnikov = f.figure(Figure::Kalashnikov, Zone::Board(B, Color::Red));
    let michelangelo = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(alexander, Zone::Hand(A), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, B);
    assert!(events.is_empty());
    game.choose(0).unwrap(); // First: B scores A's old red top.
    // Alexander's When then scores B's three top Figures one origin at a
    // time, with nothing left to choose.
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(alexander),
            value: Some(2),
            short: false,
        }))
    );
    assert_eq!(moved(&events[0]).card, before);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(moved(&events[1]).card, alexander);
    assert!(!moved(&events[1]).inside_karma);
    let mut scored = game.position().cards(Zone::Score(A)).to_vec();
    scored.sort();
    let mut expected = [kalashnikov, buried, michelangelo];
    expected.sort();
    assert_eq!(scored, expected);
    assert!(
        events[2..]
            .iter()
            .all(|e| moved(e).inside_karma && moved(e).executor == A)
    );
    assert_eq!(game.position().top(A, Color::Red), Some(alexander));
    assert!(game.position().check_conservation());
}

#[test]
fn when_children_do_not_replace_the_original_meld_result_or_advance_the_parent_early() {
    let mut f = Fixture::default();
    let code = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            build::draw_age(Expr::Add(&build::LAST_VALUE, 1)),
        ],
    );
    let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
    let scored = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let drawn = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let untouched = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [alexander, scored, drawn]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(5))),
        &[untouched]
    );
}

#[test]
fn melding_alexander_inside_karma_does_not_trigger_his_when_even_after_covering_the_source() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(
        Color::Red,
        1,
        Zone::Board(B, Color::Red),
        vec![build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor))],
    );
    let protected = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [alexander, incoming]
    );
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().top(B, Color::Yellow), Some(protected));
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn tucking_or_a_replaced_meld_does_not_trigger_when() {
    for replaced in [false, true] {
        let mut f = Fixture::default();
        let source = if replaced {
            Zone::Score(A)
        } else {
            Zone::Hand(A)
        };
        if replaced {
            f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
        }
        let alexander = f.figure(Figure::AlexanderTheGreat, source);
        let protected = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Move {
                verb: if replaced { Verb::Meld } else { Verb::Tuck },
                target: target(alexander, source),
                destination: Zone::Board(A, Color::Red),
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(events.len(), usize::from(!replaced));
        assert_eq!(game.position().top(B, Color::Yellow), Some(protected));
    }
}

#[test]
fn emma_unsplays_every_board_before_the_callers_next_instruction() {
    let mut f = Fixture::default();
    for player in [A, B] {
        for color in [Color::Blue, Color::Green] {
            for age in [1, 2] {
                f.tech(color, age, Zone::Board(player, color), vec![]);
            }
        }
    }
    let code = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let emma = f.figure(Figure::EmmaWatson, Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut events = Vec::new();
    for player in [A, B] {
        for color in [Color::Blue, Color::Green] {
            game.start(
                Operation::Splay {
                    player,
                    color,
                    direction: Splay::Up,
                },
                Context::new(player),
            )
            .unwrap();
            settle(&mut game, &mut events);
        }
    }
    events.clear();
    game.self_execute(code, Context::new(A)).unwrap();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert_eq!(events.len(), 5);
    assert_eq!(moved(&events[0]).card, emma);
    assert!(events[1..].iter().all(|e| matches!(
        e,
        Event::Splayed {
            to: Splay::None,
            executor: A,
            inside_karma: true,
            ..
        }
    )));
    for player in [A, B] {
        for color in Color::ALL {
            assert_eq!(game.position().splay(player, color), Splay::None);
        }
    }
}

#[test]
fn cloning_preserves_the_committed_result_while_when_is_suspended() {
    let mut f = Fixture::default();
    let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(alexander, Zone::Hand(A), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..150 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("When failed to finish");
}
