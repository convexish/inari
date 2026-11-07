use inno_engine::*;

mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..20 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("fading did not finish");
}

#[test]
fn fading_requeries_newly_exposed_figures_and_leaves_one_top() {
    let mut f = Fixture::default();
    let covered = f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let top = f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    let kept = f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let other_player = f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.fade(Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options.len(), 2);
    game.choose(0).unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, top);
    assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == covered)));
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[top, covered]);
    assert_eq!(game.position().top_figures(A).collect::<Vec<_>>(), [kept]);
    assert_eq!(
        game.position().top_figures(B).collect::<Vec<_>>(),
        [other_player]
    );
    assert!(
        events
            .iter()
            .all(|e| moved(e).verb == Some(Verb::Score) && !moved(e).inside_karma)
    );
    assert!(game.position().check_conservation());
}

#[test]
fn duke_replaces_the_requirement_without_choices_movement_or_retries() {
    let mut f = Fixture::default();
    f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let position = f.position();
    let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    game.fade(Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(20, &mut events).unwrap(),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position(), &position);
    assert!(events.is_empty());
    game.start(Operation::RequireFade { player: A }, Context::new(A))
        .unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Replaced,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert!(events.is_empty());
}

#[test]
fn uncovering_duke_during_fading_stops_before_a_second_fade_choice() {
    let mut f = Fixture::default();
    let duke = f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    let top = f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let other = f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.fade(Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, top);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(game.position().cards(Zone::Score(A)), &[top]);
    assert_eq!(
        game.position().top_figures(A).collect::<Vec<_>>(),
        [other, duke]
    );
}

#[test]
fn duke_does_not_protect_another_player_or_an_ordinary_score() {
    let mut f = Fixture::default();
    let duke = f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    let faded = f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.fade(Context::new(B)).unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, B);
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(B)), &[faded]);
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(duke, Zone::Board(A, Color::Purple)),
            destination: Zone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), &[duke]);
    assert_eq!(events.len(), 2);
}

#[test]
fn duke_wins_instead_of_melding_a_figure_with_four_figures_already_top() {
    for enough in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
        f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
        f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
        if enough {
            f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        }
        // A child win remains suppressed; opposing Jackie cannot intercept it.
        f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
        let incoming = f.figure(Figure::Nightingale, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(incoming, Zone::Hand(A), A, Color::Yellow),
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let result = settle(&mut game, &mut events);
        if enough {
            assert_eq!(result, Status::Winner(A));
            assert!(events.is_empty());
            assert_eq!(game.position().cards(Zone::Hand(A)), &[incoming]);
        } else {
            assert!(matches!(result, Status::Complete(_)));
            assert_eq!(events.len(), 1);
            assert_eq!(game.position().top(A, Color::Yellow), Some(incoming));
        }
    }
}

#[test]
fn base_and_single_top_positions_have_no_fading_requirement() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.fade(Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    let mut f = Fixture::default();
    f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(Operation::RequireFade { player: A }, Context::new(A))
        .unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
}

#[test]
fn fading_clones_before_each_requirement_choice_and_score_commit() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.fade(Context::new(A)).unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    assert_eq!(
        finish(&mut expected, &mut expected_events),
        Status::Complete(ReturnValue::Unit)
    );
    let mut events = Vec::new();
    for _ in 0..100 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(
            finish(&mut fork, &mut suffix),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => {
                assert_eq!(game, expected);
                return;
            }
            status => panic!("unexpected {status:?}"),
        }
        assert!(game.position().check_conservation());
    }
    panic!("fading failed to finish");
}
