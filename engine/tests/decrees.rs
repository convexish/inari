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
    panic!("Decree did not terminate");
}

fn issue(game: &mut Game, decree: Decree, player: Player) {
    game.start(Operation::IssueDecree { decree }, Context::new(player))
        .unwrap();
}

fn claim(game: &mut Game, decree: Decree, player: Player) {
    game.start(Operation::ClaimDecree { decree }, Context::new(player))
        .unwrap();
    finish(game, &mut Vec::new());
}

#[test]
fn expansion_effect_finishes_before_claim_and_owned_decree_can_be_issued_again() {
    let mut f = Fixture::default();
    for color in [Color::Red, Color::Blue] {
        for _ in 0..2 {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
        }
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Expansion, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(
        menu.options,
        [
            Alternative::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Up
            },
            Alternative::Splay {
                player: A,
                color: Color::Blue,
                direction: Splay::Up
            }
        ]
    );
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Supply
    );
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(matches!(
        events[0],
        Event::Splayed {
            color: Color::Red,
            to: Splay::Up,
            ..
        }
    ));
    assert!(matches!(
        events[1],
        Event::DecreeMoved {
            decree: Decree::Expansion,
            destination: DecreeLocation::Owned(A),
            ..
        }
    ));
    assert_eq!(game.position().achievement_count(A), 1);
    issue(&mut game, Decree::Expansion, A);
    finish(&mut game, &mut events);
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[2],
        Event::Splayed {
            color: Color::Blue,
            ..
        }
    ));
    assert_eq!(game.position().achievement_count(A), 1);
    assert_eq!(game.position().regular_actions_started(), 0);
}

#[test]
fn issuing_opponents_decree_returns_it_without_effect_or_claim() {
    let mut f = Fixture::default();
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    claim(&mut game, Decree::Expansion, B);
    issue(&mut game, Decree::Expansion, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Supply
    );
    assert_eq!(game.position().achievement_count(B), 0);
    assert_eq!(
        events,
        [Event::DecreeMoved {
            decree: Decree::Expansion,
            source: DecreeLocation::Owned(B),
            destination: DecreeLocation::Supply,
            executor: A,
            inside_karma: false
        }]
    );
}

#[test]
fn inapplicable_effect_still_claims_available_decree_but_claims_cannot_duplicate_it() {
    let mut game = Game::new(Fixture::default().position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Expansion, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    game.start(
        Operation::ClaimDecree {
            decree: Decree::Expansion,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert_eq!(game.position().achievement_count(A), 1);
    assert_eq!(game.position().achievement_count(B), 0);
    assert!(
        Special::ALL
            .into_iter()
            .all(|s| game.position().special_location(s) == SpecialLocation::Available)
    );
}

#[test]
fn trade_draws_three_separate_cards_with_age_skips_then_claims() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let second = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let first = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let third = f.tech(Color::Purple, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Trade, A);
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 3);
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[first, second, third]
    );
    assert_eq!(
        game.position().decree_location(Decree::Trade),
        DecreeLocation::Supply
    );
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4);
    assert_eq!(game.position().achievement_count(A), 1);
    assert!(game.position().cards(Zone::Hand(B)).is_empty()); // Not an Achieve action.
}

#[test]
fn advancement_exhaustion_preempts_its_claim_even_if_claiming_would_win() {
    let mut f = Fixture::default();
    f.tech(Color::Blue, 9, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Green, 5, Zone::Score(B), vec![]);
    for _ in 0..6 {
        f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Advancement, A);
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    assert!(events.is_empty());
    assert_eq!(
        game.position().decree_location(Decree::Advancement),
        DecreeLocation::Supply
    );
    assert_eq!(game.position().achievement_count(A), 6);
}

#[test]
fn advancement_uses_two_above_the_current_highest_top() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let drawn = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Advancement, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(events.len(), 2);
}

#[test]
fn war_returns_only_one_top_of_the_chosen_value_per_opposing_color() {
    let mut f = Fixture::default();
    let below = f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    let red = f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    let green = f.tech(Color::Green, 3, Zone::Board(B, Color::Green), vec![]);
    let wrong_age = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
    let own = f.tech(Color::Yellow, 3, Zone::Board(A, Color::Yellow), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::War, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options,
        (1..=11).map(Alternative::Value).collect::<Vec<_>>()
    );
    game.choose(2).unwrap();
    choose_card(&mut game, &mut events, green);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 3);
    assert_eq!(moved(&events[0]).card, green);
    assert_eq!(moved(&events[1]).card, red);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(3))),
        &[red, green]
    );
    assert_eq!(game.position().top(B, Color::Red), Some(below));
    assert_eq!(game.position().top(B, Color::Blue), Some(wrong_age));
    assert_eq!(game.position().top(A, Color::Yellow), Some(own));
}

#[test]
fn war_offers_zero_only_after_introduction_and_allows_an_intentional_noop_value() {
    for introduced in [false, true] {
        let mut f = Fixture::default();
        f.tech(
            Color::Red,
            0,
            if introduced {
                Zone::Deck(Deck::base(0))
            } else {
                Zone::AgeZeroReserve
            },
            vec![],
        );
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        issue(&mut game, Decree::War, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.contains(&Alternative::Value(0)), introduced);
        game.choose(menu.options.len() - 1).unwrap(); // 11, even with no opposing top.
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), 1);
        assert_eq!(game.position().achievement_count(A), 1);
    }
}

#[test]
fn rivalry_samples_three_hidden_score_cards_then_returns_one_figure_per_color() {
    let mut f = Fixture::default();
    let mut remaining = Vec::new();
    for color in [Color::Red, Color::Blue, Color::Green, Color::Yellow] {
        remaining.push(f.tech(color, 1, Zone::Score(B), vec![]));
    }
    let below = f.figure(Figure::AlexanderTheGreat, Zone::Board(B, Color::Red));
    let red = f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let green = f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    let mut rng = Rng::new(42);
    let expected: Vec<_> = (0..3)
        .map(|_| {
            let index = (rng.next_u64() % remaining.len() as u64) as usize;
            remaining.remove(index)
        })
        .collect();
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    issue(&mut game, Decree::Rivalry, A);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, green); // Singleton blind groups auto-sample first.
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 6);
    assert_eq!(moved(&events[3]).card, green);
    assert_eq!(moved(&events[4]).card, red);
    assert_eq!(game.position().cards(Zone::Score(B)), remaining);
    assert_eq!(game.position().top(B, Color::Red), Some(below));
    assert!(game.position().check_conservation());
}

#[test]
fn rivalry_hidden_menus_group_public_backs_and_do_not_disclose_swapped_fronts() {
    let mut menus = Vec::new();
    for swapped in [false, true] {
        let mut f = Fixture::default();
        for color in if swapped {
            [Color::Blue, Color::Red]
        } else {
            [Color::Red, Color::Blue]
        } {
            f.tech(color, 1, Zone::Score(B), vec![]);
        }
        f.tech(Color::Green, 2, Zone::Score(B), vec![]);
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
        issue(&mut game, Decree::Rivalry, A);
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.player, A);
        assert_eq!(menu.options.len(), 2);
        assert!(menu.options.iter().all(|o| matches!(
            o,
            Alternative::BlindMove {
                source: Zone::Score(B),
                ..
            }
        )));
        let before = game.clone();
        assert_eq!(game.choose(2), Err(Error::InvalidChoice));
        assert_eq!(game, before);
        menus.push(menu);
    }
    assert_eq!(menus[0], menus[1]);
}

#[test]
fn rivalry_uses_logical_score_membership_without_exposing_opponents_hand() {
    let mut f = Fixture::default();
    let michelangelo = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let first = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let second = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Rivalry, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::BlindMove {
            source: Zone::Hand(B),
            ..
        }
    )));
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events[..3]
            .iter()
            .map(|e| moved(e).card)
            .collect::<Vec<_>>(),
        [first, second, michelangelo]
    );
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position().score(B), 0);
}

#[test]
fn rivalry_does_not_retry_prevented_returns_or_skip_its_later_top_figure_sentence() {
    let mut f = Fixture::default();
    for _ in 0..4 {
        f.tech(Color::Blue, 1, Zone::Score(B), vec![]);
    }
    let nightingale = f.figure(Figure::Nightingale, Zone::Board(B, Color::Yellow));
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    issue(&mut game, Decree::Rivalry, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, nightingale);
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 4);
    assert_eq!(
        game.position().decree_location(Decree::Rivalry),
        DecreeLocation::Owned(A)
    );
}

#[test]
fn decrees_count_for_achievement_victory_and_exhaustion_tiebreaks_but_not_as_specials() {
    let mut f = Fixture::default();
    for _ in 0..6 {
        f.tech(Color::Blue, 1, Zone::Achievements(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    issue(&mut game, Decree::Expansion, A);
    assert_eq!(
        finish(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    game.finish_action().unwrap();
    assert_eq!(finish(&mut game, &mut Vec::new()), Status::Winner(A));
    let mut tied = Game::new(Fixture::default().position(), Expansions::FIGURES).unwrap();
    claim(&mut tied, Decree::War, B);
    tied.start(
        Operation::Draw {
            value: 12,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    assert_eq!(finish(&mut tied, &mut Vec::new()), Status::Winner(B));
}

#[test]
fn base_mode_rejects_decree_operations_and_positions_with_owned_decrees() {
    let position = Fixture::default().position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    base.start(
        Operation::IssueDecree {
            decree: Decree::War,
        },
        Context::new(A),
    )
    .unwrap();
    assert_eq!(
        base.advance(100, &mut Vec::new()),
        Err(Error::Unsupported("Decrees require Figures"))
    );
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    claim(&mut figures, Decree::War, A);
    assert!(matches!(
        Game::new(figures.position().clone(), Expansions::BASE),
        Err(Error::InvalidSetup(_))
    ));
}

#[test]
fn cloning_at_every_rivalry_boundary_retains_blind_rng_choices_and_token_progress() {
    let mut f = Fixture::default();
    for age in [1, 2, 1, 2, 1] {
        f.tech(Color::Blue, age, Zone::Score(B), vec![]);
    }
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    issue(&mut game, Decree::Rivalry, A);
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
    panic!("Rivalry did not terminate");
}
