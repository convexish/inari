use inno_engine::*;

mod common;
use common::*;

fn draw(value: u8) -> Instruction {
    build::draw(value)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("effect did not terminate");
}

#[test]
fn archimedes_modifies_writings_printed_box_for_the_executor_not_the_source_owner() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let writing = f.writing(Zone::Board(B, Color::Blue));
    let unmodified = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let modified = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.source_owner = B;
    game.self_execute(writing, context).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[modified]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[unmodified]
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).executor, A);
    assert!(!moved(&events[0]).inside_karma); // The original effect resumes outside its First reaction.
}

#[test]
fn agriculture_computed_age_is_not_a_boxed_value_and_control_counts_are_unmodified() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let agriculture = f.agriculture(Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let scored = f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]);
    let untouched = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(agriculture, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(4))),
        &[untouched]
    );
    assert_eq!(events.len(), 3);
}

#[test]
fn boxed_scope_survives_archimedes_leaving_but_does_not_leak_into_the_next_effect() {
    let mut f = Fixture::default();
    let archimedes = f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let source = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![
            build::verb(Verb::Score, Selector::TopOf(Who::Executor, Color::Blue))
                .to(Destination::Score(Who::Executor)),
            draw(1),
        ],
    );
    f.cards[usize::from(source.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![draw(1)],
    });
    let modified = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let plain = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert!(!game.position().has_figure(A, Figure::Archimedes));
    assert_eq!(game.position().cards(Zone::Score(A)), &[archimedes]);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[modified, plain]);
    assert_eq!(events.len(), 3);
}

fn metalworking_fixture() -> (Game, CardId, CardId, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let source = f.metalworking(Zone::Board(A, Color::Red));
    let stop = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let hit = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    f.cards[usize::from(hit.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    (
        Game::new(f.position(), Expansions::FIGURES).unwrap(),
        source,
        hit,
        stop,
    )
}

#[test]
fn metalworking_repeat_retains_one_effect_scope_without_reapplying_the_increase() {
    let (mut game, source, hit, stop) = metalworking_fixture();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[hit]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[stop]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))).len(), 1);
    assert_eq!(events.len(), 5);
    assert!(matches!(events[1], Event::Revealed { card, .. } if card == hit));
    assert!(matches!(events[4], Event::Revealed { card, .. } if card == stop));
}

#[test]
fn the_demanders_archimedes_modifies_the_effect_but_the_victim_performs_its_operations() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let source = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![
            draw(1),
            build::verb(Verb::Transfer, build::LAST).to(Destination::Hand(Who::Demander)),
        ],
    );
    f.cards[usize::from(source.0)].effects[0].kind = EffectKind::Demand;
    f.cards[usize::from(source.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    let card = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let plain = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), &[plain]);
    assert_eq!(events.len(), 2);
    assert!(
        events
            .iter()
            .all(|e| moved(e).executor == B && !moved(e).inside_karma)
    );
}

#[test]
fn shared_invocations_have_independent_scopes_and_the_free_draw_has_no_effect_boost() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let source = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![draw(1)]);
    let shared = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]); // Keep the reward age nonempty.
    let modified = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let reward = f.figure(Figure::AlexanderTheGreat, Zone::Deck(Deck::figures(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[modified, reward]);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[shared]);
    assert_eq!(events.len(), 3);
    assert_eq!(moved(&events[2]).source, Zone::Deck(Deck::figures(2)));
}

#[test]
fn effect_invocation_inside_kalashnikov_does_not_trigger_archimedes() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    let program = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![draw(1)]);
    let outer = f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let untouched = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(outer, Zone::Hand(B), B, Color::Green), Context::new(B))
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let choice = menu
        .options
        .iter()
        .position(|o| *o == Alternative::SelfExecute(program))
        .unwrap();
    game.choose(choice).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[untouched]
    );
    assert_eq!(events.len(), 2);
    assert!(moved(&events[0]).inside_karma);
}

#[test]
fn archimedes_grants_advancement_with_two_figures_but_does_not_modify_its_computed_age() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    f.figure(Figure::EmmaWatson, Zone::Hand(A));
    f.figure(Figure::BearGrylls, Zone::Hand(A)); // Same value, neither blue.
    let drawn = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let untouched = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Decree, ActionKind::Free).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        game.position().decree_location(Decree::Advancement),
        DecreeLocation::Owned(A)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(5))),
        &[untouched]
    );
    assert_eq!(events.len(), 4);
}

#[test]
fn an_ordinary_draw_action_is_not_an_effect_invocation() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let drawn = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert_eq!(
        finish(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
}

#[test]
fn modified_age_twelve_exhausts_but_unrepresentable_literals_fault_instead_of_wrapping() {
    for literal in [11, u8::MAX] {
        let mut f = Fixture::default();
        f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
        f.tech(Color::Green, 1, Zone::Score(B), vec![]);
        let source = f.tech(
            Color::Red,
            1,
            Zone::Board(A, Color::Red),
            vec![draw(literal), Instruction::Win(WinRule::Executor)],
        );
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        if literal == 11 {
            assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
        } else {
            assert_eq!(
                game.advance(1000, &mut events),
                Err(Error::Unsupported("boxed value overflow"))
            );
            let faulted = game.clone();
            assert_eq!(
                game.advance(1000, &mut events),
                Err(Error::Unsupported("boxed value overflow"))
            );
            assert_eq!(game, faulted);
        }
        assert!(events.is_empty());
    }
}

#[test]
fn cloning_every_archimedes_boundary_retains_literal_scope_and_repeat_progress() {
    let (mut game, source, _, _) = metalworking_fixture();
    game.self_execute(source, Context::new(A)).unwrap();
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
    panic!("Archimedes did not terminate");
}
