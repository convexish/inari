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
    panic!("Decree action did not terminate");
}

fn select_decree(game: &mut Game, events: &mut Vec<Event>, decree: Decree) {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Decree(decree))
        .unwrap();
    game.choose(index).unwrap();
}

fn ordinary_hand(f: &mut Fixture, player: Player) -> [CardId; 3] {
    [
        Figure::Homer,
        Figure::AlexanderTheGreat,
        Figure::Michelangelo,
    ]
    .map(|figure| f.figure(figure, Zone::Hand(player)))
}

#[test]
fn ordinary_decree_junks_the_entire_hand_before_selecting_from_initial_figure_colors() {
    let mut f = Fixture::default();
    ordinary_hand(&mut f, A);
    let base = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    // The hand goes to the unordered junk with no order to choose.
    let menu = decision(&mut game, &mut events);
    assert_eq!(events.len(), 4);
    assert_eq!(menu.player, A);
    assert_eq!(
        menu.options,
        [
            Alternative::Decree(Decree::War),
            Alternative::Decree(Decree::Expansion),
            Alternative::Decree(Decree::Rivalry)
        ]
    );
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert!(game.position().cards(Zone::Junk).contains(&base));
    assert!(
        events
            .iter()
            .all(|e| moved(e).verb == Some(Verb::Junk) && !moved(e).inside_karma)
    );
    assert_eq!(game.position().turn_owner(), A);
    assert_eq!(game.position().regular_actions_started(), 1);
    game.choose(1).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Owned(A)
    );
    assert!(game.position().cards(Zone::Hand(B)).is_empty()); // No Achieve-action Figure draw.
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(events.len(), 5);
}

fn homer_hand() -> (Fixture, [CardId; 3], CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let figures = [
        Figure::AlexanderTheGreat,
        Figure::ShenKuo,
        Figure::Michelangelo,
    ]
    .map(|figure| f.figure(figure, Zone::Hand(A)));
    let base = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    (f, figures, base)
}

#[test]
fn homer_tucks_every_figure_but_preserves_initial_colors_and_nonfigure_payment() {
    let (f, figures, base) = homer_hand();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    for card in figures {
        choose_card(&mut game, &mut events, card);
    }
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options,
        [
            Alternative::Decree(Decree::War),
            Alternative::Decree(Decree::Expansion),
            Alternative::Decree(Decree::Trade)
        ]
    );
    assert_eq!(events.len(), 4);
    assert!(
        events[..3]
            .iter()
            .all(|e| moved(e).verb == Some(Verb::Tuck) && moved(e).inside_karma)
    );
    assert_eq!(moved(&events[3]).card, base);
    assert_eq!(moved(&events[3]).verb, Some(Verb::Junk));
    assert!(!moved(&events[3]).inside_karma);
    assert_eq!(game.position().cards(Zone::Junk), &[base]);
    assert_eq!(game.position().top_figures(A).count(), 4); // No fading mid-payment.
    game.choose(1).unwrap();
    let fade = decision(&mut game, &mut events);
    assert_eq!(fade.player, A);
    assert_eq!(events.len(), 5);
    assert!(matches!(
        events[4],
        Event::DecreeMoved {
            decree: Decree::Expansion,
            ..
        }
    ));
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Owned(A)
    );
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().top_figures(A).count(), 1);
    assert_eq!(events.len(), 8);
    assert_eq!(game.position().regular_actions_started(), 1);
}

#[test]
fn invalid_decrees_are_atomic_and_require_three_different_figure_values() {
    for hand in [
        vec![],
        vec![Figure::Homer, Figure::AlexanderTheGreat],
        vec![Figure::YiSunSin, Figure::Michelangelo, Figure::Homer],
    ] {
        let mut f = Fixture::default();
        for figure in hand {
            f.figure(figure, Zone::Hand(A));
        }
        for value in [1, 2, 3] {
            f.tech(Color::Blue, value, Zone::Hand(A), vec![]);
        }
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
        let before = game.clone();
        for kind in [ActionKind::Regular, ActionKind::Free] {
            assert_eq!(
                game.take_action(Action::Decree, kind),
                Err(Error::InvalidAction)
            );
            assert_eq!(game, before);
        }
    }
    let mut base = Game::new(Fixture::default().position(), Expansions::BASE).unwrap();
    let before = base.clone();
    assert_eq!(
        base.take_action(Action::Decree, ActionKind::Regular),
        Err(Error::InvalidAction)
    );
    assert_eq!(base, before);
}

#[test]
fn nightingale_allows_two_same_age_figures_without_a_matching_color() {
    let mut f = Fixture::default();
    f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    let emma = f.figure(Figure::EmmaWatson, Zone::Hand(A));
    f.figure(Figure::BearGrylls, Zone::Hand(A)); // Both 11, neither yellow.
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Decree, ActionKind::Free).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, emma);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4); // Two junk moves, splay, claim; no When on junking.
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::Up);
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Owned(A)
    );
    assert_eq!(game.position().regular_actions_started(), 0);
    assert_eq!(game.position().turn_owner(), A);
}

#[test]
fn covered_or_opponents_nightingale_does_not_grant_decree_permission() {
    for owner in [A, B] {
        let mut f = Fixture::default();
        f.figure(Figure::Nightingale, Zone::Board(owner, Color::Yellow));
        if owner == A {
            f.tech(Color::Yellow, 8, Zone::Board(A, Color::Yellow), vec![]);
        }
        f.figure(Figure::EmmaWatson, Zone::Hand(A));
        f.figure(Figure::BearGrylls, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let before = game.clone();
        assert_eq!(
            game.take_action(Action::Decree, ActionKind::Regular),
            Err(Error::InvalidAction)
        );
        assert_eq!(game, before);
    }
}

#[test]
fn permission_adds_to_ordinary_options_without_discarding_initial_colors() {
    let mut f = Fixture::default();
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    let figures = [Figure::Homer, Figure::AlexanderTheGreat, Figure::ShenKuo]
        .map(|figure| f.figure(figure, Zone::Hand(A)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    for card in &figures[..2] {
        choose_card(&mut game, &mut events, *card);
    }
    let menu = decision(&mut game, &mut events);
    assert_eq!(events.len(), 3);
    assert_eq!(
        menu.options,
        [
            Alternative::Decree(Decree::War),
            Alternative::Decree(Decree::Expansion),
            Alternative::Decree(Decree::Trade),
            Alternative::Decree(Decree::Rivalry)
        ]
    );
    game.choose(3).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
}

#[test]
fn meijis_borrowed_hand_pays_first_or_free_decrees_but_not_a_second_regular_decree() {
    for kind in [ActionKind::Regular, ActionKind::Free] {
        let mut f = Fixture::default();
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        let figures = ordinary_hand(&mut f, B);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if kind == ActionKind::Free {
            game.begin_action(ActionKind::Regular).unwrap();
            let before = game.clone();
            assert_eq!(
                game.take_action(Action::Decree, ActionKind::Regular),
                Err(Error::InvalidAction)
            );
            assert_eq!(game, before);
        }
        game.take_action(Action::Decree, kind).unwrap();
        let mut events = Vec::new();
        for card in &figures[..2] {
            choose_card(&mut game, &mut events, *card);
        }
        select_decree(&mut game, &mut events, Decree::Expansion);
        assert_eq!(events.len(), 3);
        assert!(
            events
                .iter()
                .all(|e| moved(e).source == Zone::Hand(B) && moved(e).executor == A)
        );
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(game.position().regular_actions_started(), 1);
        assert!(game.position().meiji_active(A));
    }
}

#[test]
fn second_regular_decree_uses_only_own_initial_hand_and_does_not_junk_borrowed_cards() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    for figure in [
        Figure::AlexanderTheGreat,
        Figure::ShenKuo,
        Figure::Michelangelo,
    ] {
        f.figure(figure, Zone::Hand(A));
    }
    let foreign = f.figure(Figure::Homer, Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options,
        [
            Alternative::Decree(Decree::War),
            Alternative::Decree(Decree::Expansion),
            Alternative::Decree(Decree::Trade)
        ]
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[foreign]);
    game.choose(1).unwrap();
    finish(&mut game, &mut events);
    assert_eq!(game.position().turn_owner(), B);
    assert!(events[..3].iter().all(|e| moved(e).source == Zone::Hand(A)));
}

#[test]
fn payment_is_required_even_when_the_chosen_decree_only_returns_an_opponents_token() {
    let mut f = Fixture::default();
    ordinary_hand(&mut f, A);
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ClaimDecree {
            decree: Decree::Expansion,
        },
        Context::new(B),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    select_decree(&mut game, &mut events, Decree::Expansion);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4);
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Supply
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().achievement_count(A), 0);
}

#[test]
fn terminal_decree_effect_preserves_payment_but_aborts_claim_and_turn_advancement() {
    let mut f = Fixture::default();
    for figure in [Figure::Homer, Figure::AlexanderTheGreat, Figure::ShenKuo] {
        f.figure(figure, Zone::Hand(A));
    }
    f.tech(Color::Blue, 5, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    select_decree(&mut game, &mut events, Decree::Trade);
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    assert_eq!(events.len(), 3);
    assert_eq!(game.position().cards(Zone::Junk).len(), 3);
    assert_eq!(
        game.position().decree_location(Decree::Trade),
        DecreeLocation::Supply
    );
    assert_eq!(game.position().turn_owner(), A);
    assert_eq!(game.position().regular_actions_started(), 1);
}

#[test]
fn cloning_every_decree_action_boundary_retains_initial_permissions_payment_and_fading() {
    let (f, _, _) = homer_hand();
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.begin_opening_turn(A).unwrap();
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..300 {
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
                assert_eq!(
                    game.take_action(Action::Decree, ActionKind::Free),
                    Err(Error::Busy)
                );
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
    panic!("Decree action did not terminate");
}
