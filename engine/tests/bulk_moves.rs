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
    panic!("bulk movement did not terminate");
}

fn program(f: &mut Fixture, instructions: Vec<Instruction>) -> CardId {
    f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        instructions,
    )
}

fn transfer_all(from: Selector) -> Instruction {
    build::verb(Verb::Transfer, from)
        .to(Destination::Hand(Who::Executor))
        .all()
}

#[test]
fn base_bulk_transfers_match_in_both_kernels_and_report_a_performed_instruction() {
    let mut f = Fixture::default();
    let first = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let second = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
    let code = program(
        &mut f,
        vec![
            transfer_all(Selector::Hand(Who::Opponent)),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    base.self_execute(code, Context::new(A)).unwrap();
    figures.self_execute(code, Context::new(A)).unwrap();
    let mut base_events = Vec::new();
    let mut figure_events = Vec::new();
    assert_eq!(finish(&mut base, &mut base_events), Status::Winner(A));
    assert_eq!(finish(&mut figures, &mut figure_events), Status::Winner(A));
    assert_eq!(base.position(), figures.position());
    assert_eq!(base_events, figure_events);
    assert_eq!(base_events.len(), 2);
    assert_eq!(base.position().cards(Zone::Hand(A)), &[first, second]);
    assert!(base_events.iter().all(|e| matches!(e, Event::Moved(m)
        if m.verb == Some(Verb::Transfer) && !m.inside_karma)));
    assert!(base.position().check_conservation());
}

#[test]
fn transfers_finish_yi_once_per_card_while_an_exchange_triggers_yi_once_total() {
    for exchange in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        let first = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let second = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
        let later_draw = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        let first_draw = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
        let instruction = if exchange {
            Instruction::Exchange(ExchangeStep {
                left: build::HAND,
                right: Selector::Hand(Who::Opponent),
                counts: &[None, None],
                optional: false,
            })
        } else {
            transfer_all(Selector::Hand(Who::Opponent))
        };
        let code = program(
            &mut f,
            vec![
                instruction,
                build::if_(build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        // Exchanged cards are all revealed, then tucked.
        let revealed: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(
            revealed,
            if exchange {
                vec![first, second]
            } else {
                vec![]
            }
        );
        if exchange {
            assert!(matches!(events[0], Event::Revealed { .. }));
            assert!(matches!(events[1], Event::Revealed { .. }));
        }
        let events: Vec<_> = events
            .into_iter()
            .filter(|e| matches!(e, Event::Moved(_)))
            .collect();
        let cards: Vec<_> = events.iter().map(|e| moved(e).card).collect();
        assert_eq!(
            cards,
            if exchange {
                vec![first, second, first_draw, first_draw]
            } else {
                vec![
                    first, first_draw, first_draw, second, later_draw, later_draw,
                ]
            }
        );
        assert!(events.iter().all(|e| moved(e).inside_karma));
        assert_eq!(
            events.iter().filter(|e| moved(e).verb.is_none()).count(),
            if exchange { 1 } else { 2 }
        );
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert!(game.position().check_conservation());
    }
}

#[test]
fn bulk_selection_rechecks_newly_exposed_tops_after_each_reaction() {
    let mut f = Fixture::default();
    let yi = f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let covered = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let top = f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    let later_draw = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let first_draw = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let code = program(
        &mut f,
        vec![transfer_all(build::filtered(
            &Selector::TopOf(Who::Opponent, Color::Red),
            &[Filter::Lacks(Test::Is(Reg::Source))],
        ))],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [top, first_draw, first_draw, covered, later_draw, later_draw]
    );
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        &[covered, top, yi]
    );
    assert!(game.position().cards(Zone::Board(B, Color::Red)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn an_eligible_card_is_not_retried_after_a_replacement_moves_it() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let original = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let drawn = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let code = program(
        &mut f,
        vec![transfer_all(build::filtered(
            &Selector::TopOf(Who::Executor, Color::Blue),
            &[Filter::Lacks(Test::Is(Reg::Source))],
        ))],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(100, &mut events).unwrap(),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [original, drawn, drawn]
    );
    assert_eq!(game.position().top(A, Color::Blue), Some(original));
    assert!(game.position().check_conservation());
}

#[test]
fn prevented_cards_are_considered_once_and_do_not_satisfy_if_performed() {
    let mut f = Fixture::default();
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    let first = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let second = f.tech(Color::Green, 3, Zone::Score(A), vec![]);
    let code = program(
        &mut f,
        vec![
            build::verb(Verb::Return, build::SCORE)
                .to(Destination::Deck)
                .all(),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    game.choose(0).unwrap();
    assert_eq!(
        game.advance(100, &mut events).unwrap(),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Score(A)), &[first, second]);
}

#[test]
fn bulk_transfers_in_foreign_self_execution_keep_karma_suppression() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![transfer_all(build::SCORE)],
    );
    let first = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let second = f.tech(Color::Purple, 3, Zone::Score(A), vec![]);
    let incoming = f.tech(Color::Green, 5, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Green),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [first, second, incoming]
    );
    for event in &events[..2] {
        assert_eq!(moved(event).verb, Some(Verb::Transfer));
        assert_eq!(moved(event).executor, A);
        assert!(moved(event).inside_karma);
    }
    assert!(!moved(&events[2]).inside_karma);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[first, second]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn hidden_worlds_produce_identical_bulk_menus_and_invalid_choices_do_not_mutate() {
    let make = |reverse| {
        let mut f = Fixture::default();
        let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let green = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
        if reverse {
            f.placements.swap(usize::from(blue.0), usize::from(green.0));
        }
        f.tech(Color::Yellow, 3, Zone::Hand(B), vec![]);
        let code = program(&mut f, vec![transfer_all(Selector::Hand(Who::Opponent))]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let mut events = Vec::new();
    let menu = decision(&mut first, &mut events);
    assert_eq!(menu, decision(&mut second, &mut events));
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    assert_eq!(
        menu.options,
        [2, 3].map(|age| Alternative::BlindMove {
            verb: Verb::Transfer,
            source: Zone::Hand(B),
            back: Deck::base(age),
        })
    );
    let before = first.clone();
    assert_eq!(first.choose(2), Err(Error::InvalidChoice));
    assert_eq!(first, before);
    assert!(events.is_empty());
}

#[test]
fn cloning_at_each_bulk_boundary_preserves_reactions_progress_and_events() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    for (player, color) in [(A, Color::Blue), (B, Color::Green)] {
        let id = f.tech(color, 1, Zone::Board(player, color), vec![]);
        f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    }
    let code = program(&mut f, vec![transfer_all(Selector::Hand(Who::Opponent))]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    assert_eq!(result, Status::Complete(ReturnValue::Unit));
    let mut events = Vec::new();
    for _ in 0..200 {
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
    panic!("resolution did not terminate");
}
