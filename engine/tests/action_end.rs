use inno_engine::*;

mod common;
use common::*;

fn achievements(f: &mut Fixture, player: Player, count: usize) {
    for _ in 0..count {
        f.tech(Color::Blue, 1, Zone::Achievements(player), vec![]);
    }
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("action completion did not terminate");
}

#[test]
fn both_players_fade_nonactive_first_before_any_achievement_victory_check() {
    for active in [A, B] {
        let mut f = Fixture::default();
        let a_faded = f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
        f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
        let b_faded = f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
        f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
        achievements(&mut f, A, 7);
        achievements(&mut f, B, 7);
        let mut position = f.position();
        position.begin_turn(active);
        let mut game = Game::new(position, Expansions::FIGURES).unwrap();
        game.finish_action().unwrap();
        let mut events = Vec::new();
        let before = game.clone();
        assert_eq!(game.begin_turn(active.opponent()), Err(Error::Busy));
        assert_eq!(game, before);
        assert_eq!(decision(&mut game, &mut events).player, active.opponent());
        game.choose(0).unwrap();
        assert_eq!(decision(&mut game, &mut events).player, active);
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).executor, active.opponent());
        game.choose(0).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Winner(active.opponent())
        );
        assert_eq!(events.len(), 2);
        assert_eq!(moved(&events[1]).executor, active);
        assert_eq!(game.position().cards(Zone::Score(A)), &[a_faded]);
        assert_eq!(game.position().cards(Zone::Score(B)), &[b_faded]);
        assert!(events.iter().all(|e| !moved(e).inside_karma));
        assert!(game.position().check_conservation());
    }
}

#[test]
fn finish_action_preserves_turn_ownership_and_free_action_ordinals() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    game.finish_action().unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, B);
    assert!(game.position().meiji_active(A));
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    for _ in 0..2 {
        game.begin_action(ActionKind::Free).unwrap();
        game.finish_action().unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(game.position().meiji_active(A));
    }
    game.begin_action(ActionKind::Regular).unwrap();
    assert!(!game.position().meiji_active(A));
}

#[test]
fn special_eligibility_is_checked_after_fading_not_before_it() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let faded = f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    for color in [Color::Blue, Color::Green, Color::Purple] {
        f.tech(color, 8, Zone::Board(A, color), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(game.position().eligible_for_special(A, Special::Universe));
    game.finish_action().unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, faded);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        game.position().special_location(Special::Universe),
        SpecialLocation::Available
    );
    assert_eq!(game.position().achievement_count(A), 0);
}

#[test]
fn specials_resolve_for_both_players_before_victory_even_if_the_first_claim_reaches_threshold() {
    let mut f = Fixture::default();
    for color in Color::ALL {
        f.tech(color, 8, Zone::Board(B, color), vec![]);
    }
    for color in [Color::Red, Color::Blue, Color::Green, Color::Yellow] {
        let card = f.tech(color, 1, Zone::Board(A, color), vec![]);
        f.cards[usize::from(card.0)].symbols = four_slots([Symbol::Icon(Icon::Efficiency); 4]);
        f.cards[usize::from(card.0)].effects[0].kind = EffectKind::Demand;
    }
    achievements(&mut f, A, 5);
    achievements(&mut f, B, 5);
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    base.finish_action().unwrap();
    figures.finish_action().unwrap();
    let mut events = Vec::new();
    let mut figure_events = Vec::new();
    assert_eq!(settle(&mut base, &mut events), Status::Winner(A));
    assert_eq!(settle(&mut figures, &mut figure_events), Status::Winner(A));
    assert_eq!(base.position(), figures.position());
    assert_eq!(events, figure_events);
    assert_eq!(
        events
            .iter()
            .map(|e| match e {
                Event::SpecialMoved {
                    special,
                    destination: SpecialLocation::Owned(player),
                    ..
                } => (*player, *special),
                _ => panic!("expected special claim"),
            })
            .collect::<Vec<_>>(),
        [
            (B, Special::Universe),
            (A, Special::Monument),
            (A, Special::World)
        ]
    );
    assert_eq!(base.position().achievement_count(A), 7);
    assert_eq!(base.position().achievement_count(B), 6);
}

#[test]
fn a_shared_special_is_awarded_to_the_nonactive_player() {
    for active in [A, B] {
        let mut f = Fixture::default();
        for player in [A, B] {
            for color in Color::ALL {
                f.tech(color, 8, Zone::Board(player, color), vec![]);
            }
        }
        let mut position = f.position();
        position.begin_turn(active);
        let mut game = Game::new(position, Expansions::BASE).unwrap();
        game.finish_action().unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().special_location(Special::Universe),
            SpecialLocation::Owned(active.opponent())
        );
        assert_eq!(events.len(), 1);
        // Completing another action cannot claim the same token again.
        game.finish_action().unwrap();
        settle(&mut game, &mut events);
        assert_eq!(events.len(), 1);
    }
}

#[test]
fn duke_does_not_skip_specials_or_the_win_reaction_after_replacing_fading() {
    let mut f = Fixture::default();
    let duke = f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    let trebek = f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    for color in [Color::Red, Color::Blue, Color::Green] {
        f.tech(color, 8, Zone::Board(A, color), vec![]);
    }
    f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
    achievements(&mut f, A, 6);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.finish_action().unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B); // Jackie's scoring choice, not A fading
    assert_eq!(events.len(), 1);
    assert_eq!(
        game.position().special_location(Special::Universe),
        SpecialLocation::Owned(A)
    );
    assert_eq!(game.position().top_figures(A).count(), 2);
    game.choose(0).unwrap();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    let scored = game.position().cards(Zone::Score(B));
    assert!(scored.contains(&duke) && scored.contains(&trebek));
    assert!(events[1..].iter().all(|e| moved(e).inside_karma));
}

#[test]
fn cloning_at_every_global_action_end_boundary_preserves_fading_and_terminal_resolution() {
    let mut f = Fixture::default();
    f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    for color in [Color::Red, Color::Blue, Color::Green] {
        f.tech(color, 8, Zone::Board(A, color), vec![]);
    }
    f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
    achievements(&mut f, A, 6);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.finish_action().unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
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
            Status::Decision(menu) => {
                let before = game.clone();
                assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
                assert_eq!(game, before);
                game.choose(0).unwrap();
            }
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("action end did not terminate");
}
