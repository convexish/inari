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
    panic!("action did not terminate");
}

#[test]
fn two_regular_actions_complete_before_advancing_to_the_other_players_turn() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut events = Vec::new();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert_eq!(game.position().regular_actions_started(), 1);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().turn_owner(), A);
    game.take_action(Action::Meld { card }, ActionKind::Regular)
        .unwrap();
    while events.len() < 2 {
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    assert_eq!(game.position().turn_owner(), A);
    assert_eq!(game.position().regular_actions_started(), 2);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().top(A, Color::Red), Some(card));
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(game.position().regular_actions_started(), 0);
    assert!(game.position().check_conservation());
}

#[test]
fn opening_turn_has_one_first_action_then_ordinary_two_action_turns() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let b_card = f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert_eq!(game.position().regular_actions_started(), 1);
    assert!(game.position().meiji_active(A));
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(game.position().turn_owner(), B);
    assert!(!game.position().meiji_active(A));
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    finish(&mut game, &mut events);
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(game.position().regular_actions_started(), 1);
    game.take_action(Action::Meld { card: b_card }, ActionKind::Regular)
        .unwrap();
    finish(&mut game, &mut events);
    assert_eq!(game.position().turn_owner(), A);
    assert!(game.position().meiji_active(A));
}

#[test]
fn second_regular_meld_cannot_admit_meijis_borrowed_card_but_free_meld_can() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let own = f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    let borrowed = f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    let other = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut events = Vec::new();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    finish(&mut game, &mut events);
    for card in [borrowed, other] {
        let before = game.clone();
        assert_eq!(
            game.take_action(Action::Meld { card }, ActionKind::Regular),
            Err(Error::InvalidAction)
        );
        assert_eq!(game, before);
        game.take_action(Action::Meld { card }, ActionKind::Free)
            .unwrap();
        finish(&mut game, &mut events);
        assert_eq!(game.position().regular_actions_started(), 1);
        assert_eq!(game.position().turn_owner(), A);
        assert!(game.position().meiji_active(A));
    }
    game.take_action(Action::Meld { card: own }, ActionKind::Regular)
        .unwrap();
    assert!(!game.position().meiji_active(A));
    finish(&mut game, &mut events);
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(moved(&events[1]).source, Zone::Hand(B));
}

#[test]
fn free_actions_do_not_reopen_a_closed_meiji_window_or_advance_an_exhausted_fixture_turn() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let borrowed = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    let before = game.clone();
    assert_eq!(
        game.take_action(Action::Draw, ActionKind::Regular),
        Err(Error::InvalidAction)
    );
    assert_eq!(
        game.take_action(Action::Meld { card: borrowed }, ActionKind::Free),
        Err(Error::InvalidAction)
    );
    assert_eq!(game, before);
    game.take_action(Action::Draw, ActionKind::Free).unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().regular_actions_started(), 2);
    assert_eq!(game.position().turn_owner(), A);
    assert!(!game.position().meiji_active(A));
}

#[test]
fn invalid_or_busy_actions_leave_state_and_rng_unchanged() {
    let mut f = Fixture::default();
    let achievement = f.tech(Color::Blue, 1, Zone::AvailableAchievements, vec![]);
    let own = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let foreign = f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    let before = game.clone();
    for action in [
        Action::Achieve { card: achievement },
        Action::Achieve { card: own },
        Action::Meld { card: foreign },
        Action::Meld {
            card: CardId(u16::MAX),
        },
        Action::Achieve {
            card: CardId(u16::MAX),
        },
    ] {
        assert_eq!(
            game.take_action(action, ActionKind::Regular),
            Err(Error::InvalidAction)
        );
        assert_eq!(game, before);
    }
    game.take_action(Action::Meld { card: own }, ActionKind::Regular)
        .unwrap();
    let before = game.clone();
    assert_eq!(
        game.take_action(Action::Draw, ActionKind::Free),
        Err(Error::Busy)
    );
    assert_eq!(game.begin_opening_turn(B), Err(Error::Busy));
    assert_eq!(game, before);
}

fn achieve_fixture() -> (Fixture, CardId) {
    let mut f = Fixture::default();
    f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Blue, 5, Zone::Score(A), vec![]);
    f.tech(Color::Green, 4, Zone::Board(B, Color::Green), vec![]);
    let achievement = f.tech(Color::Red, 1, Zone::AvailableAchievements, vec![]);
    (f, achievement)
}

#[test]
fn achieve_action_draws_a_figure_for_the_opponent_at_their_own_top_value() {
    for kind in [ActionKind::Regular, ActionKind::Free] {
        let (mut f, achievement) = achieve_fixture();
        let base = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        let figure = f.figure(Figure::YiSunSin, Zone::Deck(Deck::figures(4)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(Action::Achieve { card: achievement }, kind)
            .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().cards(Zone::Achievements(A)), &[achievement]);
        assert_eq!(game.position().cards(Zone::Hand(B)), &[figure]);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), &[base]);
        assert_eq!(events.len(), 2);
        assert_eq!(moved(&events[1]).executor, B);
        assert!(!moved(&events[1]).inside_karma);
        assert_eq!(game.position().turn_owner(), A);
        assert_eq!(
            game.position().regular_actions_started(),
            u8::from(kind == ActionKind::Regular)
        );
        assert!(matches!(
            game.position()
                .descriptor(target(achievement, Zone::Achievements(A)), A),
            TargetDescriptor::Hidden { .. }
        ));
    }
}

/// Among available achievements with identical backs, the Achieve action claims one at
/// random; another back never.
#[test]
fn achieve_claims_at_random_among_identical_backs() {
    let mut claimed = [false; 2];
    for seed in 0..32 {
        let (mut f, achievement) = achieve_fixture();
        let twin = f.tech(Color::Blue, 1, Zone::AvailableAchievements, vec![]);
        f.tech(Color::Green, 2, Zone::AvailableAchievements, vec![]);
        let mut game = Game::seeded(f.position(), Expansions::BASE, seed).unwrap();
        game.take_action(Action::Achieve { card: achievement }, ActionKind::Regular)
            .unwrap();
        finish(&mut game, &mut Vec::new());
        let won = game.position().cards(Zone::Achievements(A));
        assert!(won == [achievement] || won == [twin], "{won:?}");
        claimed[usize::from(won == [twin])] = true;
    }
    assert_eq!(claimed, [true, true]);
}

#[test]
fn achieve_reward_falls_back_to_base_but_base_mode_and_effect_claims_do_not_reward() {
    let (mut f, achievement) = achieve_fixture();
    let fallback = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    let mut effect = Game::new(position, Expansions::FIGURES).unwrap();
    figures
        .take_action(Action::Achieve { card: achievement }, ActionKind::Regular)
        .unwrap();
    finish(&mut figures, &mut Vec::new());
    assert_eq!(figures.position().cards(Zone::Hand(B)), &[fallback]);
    base.take_action(Action::Achieve { card: achievement }, ActionKind::Regular)
        .unwrap();
    finish(&mut base, &mut Vec::new());
    effect
        .start(
            Operation::Claim {
                target: target(achievement, Zone::AvailableAchievements),
                eligibility: ClaimEligibility::Available,
            },
            Context::new(A),
        )
        .unwrap();
    finish(&mut effect, &mut Vec::new());
    assert!(base.position().cards(Zone::Hand(B)).is_empty());
    assert!(effect.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(effect.position().regular_actions_started(), 0);
}

#[test]
fn opponent_reward_exhaustion_preempts_achievement_victory_and_turn_advancement() {
    let (mut f, achievement) = achieve_fixture();
    for _ in 0..6 {
        f.tech(Color::Blue, 2, Zone::Achievements(A), vec![]);
    }
    f.tech(Color::Red, 11, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    game.take_action(Action::Achieve { card: achievement }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    assert_eq!(game.position().achievement_count(A), 7);
    assert_eq!(game.position().turn_owner(), A);
    assert_eq!(game.position().regular_actions_started(), 1);
    assert_eq!(events.len(), 1);
    assert_eq!(settle(&mut game, &mut events), Status::Winner(B));
    assert_eq!(
        game.take_action(Action::Draw, ActionKind::Regular),
        Err(Error::Busy)
    );
}

fn when_fixture() -> (Game, CardId) {
    let mut f = Fixture::default();
    let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    (game, alexander)
}

#[test]
fn when_reactions_and_fading_finish_before_turn_advancement() {
    let (mut game, alexander) = when_fixture();
    game.take_action(Action::Meld { card: alexander }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    // Alexander's When scores B's two top Figures one origin at a time,
    // leaving nothing to choose; the first decision is A's fading.
    assert_eq!(decision(&mut game, &mut events).player, A);
    assert_eq!(events.len(), 3);
    assert!(events[1..].iter().all(|e| moved(e).inside_karma));
    assert_eq!(game.position().turn_owner(), A);
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4);
    assert!(!moved(&events[3]).inside_karma);
    assert_eq!(game.position().turn_owner(), B);
}

#[test]
fn cloning_at_every_action_boundary_retains_when_fading_and_turn_progress() {
    let (mut game, alexander) = when_fixture();
    game.take_action(Action::Meld { card: alexander }, ActionKind::Regular)
        .unwrap();
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
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("action did not terminate");
}
