use inno_engine::*;

mod common;
use common::*;

fn native(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

fn icons(f: &mut Fixture, player: Player, color: Color, icon: Icon, count: usize) -> CardId {
    let card = f.tech(color, 1, Zone::Board(player, color), vec![]);
    f.cards[usize::from(card.0)].symbols[..count].fill(Symbol::Icon(icon));
    card
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("conditional program did not finish");
}

fn movements(events: &[Event]) -> Vec<(Option<Verb>, CardId)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.verb, m.card)),
            _ => None,
        })
        .collect()
}

#[test]
fn city_states_checks_four_icons_before_payment_and_rewards_only_the_transfer() {
    for authority in [3, 4] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B013", A);
        let payment = icons(&mut f, B, Color::Red, Icon::Authority, 2);
        icons(&mut f, B, Color::Blue, Icon::Authority, authority - 2);
        let reward = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.super_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        if authority == 4 {
            choose_card(&mut game, &mut events, payment);
        }
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if authority == 4 {
            assert_eq!(
                movements(&events),
                [(Some(Verb::Transfer), payment), (None, reward)]
            );
            assert_eq!(game.position().top(A, Color::Red), Some(payment));
            assert_eq!(game.position().cards(Zone::Hand(B)), [reward]);
        } else {
            assert!(events.is_empty());
            assert_eq!(game.position().top(B, Color::Red), Some(payment));
        }
    }
}

#[test]
fn calendar_uses_strict_card_counts_not_points() {
    for (score, hand, draws) in [(0, 0, false), (1, 1, false), (2, 1, true)] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B022", A);
        for _ in 0..score {
            f.tech(Color::Red, 11, Zone::Score(A), vec![]);
        }
        for _ in 0..hand {
            f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
        }
        let second = f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]);
        let first = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        let expected: &[(Option<Verb>, CardId)] = if draws {
            &[(None, first), (None, second)]
        } else {
            &[]
        };
        assert_eq!(movements(&events), expected);
    }
}

#[test]
fn calendar_reads_meiji_logical_hand_before_deciding_to_draw() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B022", A);
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
}

#[test]
fn calendar_reads_michelangelo_logical_score_without_double_counting() {
    let mut f = Fixture::default();
    let source = f.add(base_card("B022").unwrap(), Zone::Hand(A));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    f.tech(Color::Red, 1, Zone::Score(A), vec![]);
    let second = f.tech(Color::Red, 5, Zone::Deck(Deck::base(5)), vec![]);
    let first = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(movements(&events), [(None, first), (None, second)]);
}

#[test]
fn invention_alternate_wonder_claim_counts_left_splays_and_survives_declining() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        for color in Color::ALL {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
            f.tech(color, 2, Zone::Board(A, color), vec![]);
        }
        let source = native(&mut f, "B040", A);
        let reward = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        for color in Color::ALL {
            game.start(
                Operation::Splay {
                    player: A,
                    color,
                    direction: Splay::Left,
                },
                Context::new(A),
            )
            .unwrap();
            finish(&mut game, &mut Vec::new());
        }
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 6);
        let chosen = if accept {
            0
        } else {
            menu.options
                .iter()
                .position(|option| *option == Alternative::Decline)
                .unwrap()
        };
        game.choose(chosen).unwrap();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().special_location(Special::Wonder),
            SpecialLocation::Owned(A)
        );
        assert_eq!(
            game.position().cards(Zone::Score(A)).contains(&reward),
            accept
        );
        assert_eq!(
            game.position().splay(A, Color::Red),
            if accept { Splay::Right } else { Splay::Left }
        );
    }
}

#[test]
fn astronomy_reveals_every_draw_but_melds_and_repeats_only_green_or_blue() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B054", A);
    let red = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
    let blue = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let green = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        movements(&events),
        [
            (None, green),
            (Some(Verb::Meld), green),
            (None, blue),
            (Some(Verb::Meld), blue),
            (None, red)
        ]
    );
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, [green, blue, red]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [red]);
    assert_eq!(
        game.position().special_location(Special::Universe),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn astronomy_universal_condition_excludes_purple_and_accepts_an_empty_set() {
    for (value, claim) in [(None, true), (Some(5), false), (Some(6), true)] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B054", A);
        if let Some(value) = value {
            f.tech(Color::Red, value, Zone::Board(A, Color::Red), vec![]);
        }
        f.tech(Color::Yellow, 6, Zone::Deck(Deck::base(6)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        assert_eq!(
            finish(&mut game, &mut Vec::new()),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().special_location(Special::Universe),
            if claim {
                SpecialLocation::Owned(A)
            } else {
                SpecialLocation::Available
            }
        );
    }
}

#[test]
fn publications_restores_only_junked_tokens_and_never_offers_owned_ones() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B073", A);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.start(
        Operation::ClaimSpecial {
            special: Special::Wonder,
            check_eligibility: false,
        },
        Context::new(B),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    let mut events = Vec::new();
    for restore in [false, true] {
        game.self_execute(source, Context::new(A)).unwrap();
        let menu = decision(&mut game, &mut events);
        assert!(
            !menu
                .options
                .contains(&Alternative::JunkSpecial(Special::Wonder))
        );
        assert!(
            !menu
                .options
                .contains(&Alternative::RestoreSpecial(Special::Wonder))
        );
        let choice = if restore {
            Alternative::RestoreSpecial(Special::Universe)
        } else {
            Alternative::JunkSpecial(Special::Universe)
        };
        game.choose(
            menu.options
                .iter()
                .position(|option| *option == choice)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().special_location(Special::Universe),
            if restore {
                SpecialLocation::Available
            } else {
                SpecialLocation::Junk
            }
        );
    }
    game.start(
        Operation::RestoreSpecial {
            special: Special::Wonder,
        },
        Context::new(A),
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
    assert_eq!(
        game.position().special_location(Special::Wonder),
        SpecialLocation::Owned(B)
    );
}

#[test]
fn self_service_zero_ratio_is_literal_and_virtual_achievements_count() {
    for virtual_count in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B101", A);
        if virtual_count {
            f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
            for age in [7, 8] {
                f.tech(Color::Red, age, Zone::Score(A), vec![]);
            }
            f.tech(Color::Yellow, 1, Zone::Achievements(B), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(game.position().achievement_count(A), 0);
        game.self_execute(source, Context::new(A)).unwrap();
        assert_eq!(finish(&mut game, &mut Vec::new()), Status::Winner(A));
    }
}

#[test]
fn self_service_does_not_select_itself_when_the_ratio_fails() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B101", A);
    f.tech(Color::Red, 1, Zone::Achievements(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
}

#[test]
fn bioengineering_requeries_both_players_after_the_selected_score() {
    for (own, other, winner) in [(0, 0, None), (0, 2, Some(B)), (2, 2, None), (3, 1, Some(A))] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B102", A);
        icons(&mut f, A, Color::Yellow, Icon::Health, own);
        let payment = icons(&mut f, B, Color::Red, Icon::Health, 2);
        icons(&mut f, B, Color::Green, Icon::Health, other);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        if other > 0 {
            choose_card(&mut game, &mut events, payment);
        }
        let status = finish(&mut game, &mut events);
        assert_eq!(
            status,
            winner.map_or(Status::Complete(ReturnValue::Unit), Status::Winner)
        );
        assert_eq!(movements(&events), [(Some(Verb::Score), payment)]);
        assert_eq!(game.position().cards(Zone::Score(A)), [payment]);
    }
}

#[test]
fn globalization_requires_the_health_condition_for_each_player_and_a_unique_score() {
    for (own_health, other_health, points, wins) in [
        (3, 0, 1, true),
        (4, 0, 1, false),
        (0, 1, 1, false),
        (0, 0, 0, false),
    ] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B098", A);
        icons(&mut f, A, Color::Red, Icon::Health, own_health);
        icons(&mut f, B, Color::Green, Icon::Health, other_health);
        if points > 0 {
            f.tech(Color::Yellow, points, Zone::Score(A), vec![]);
        }
        let drawn = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            if wins {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
        assert_eq!(
            movements(&events),
            [(None, drawn), (Some(Verb::Meld), drawn)]
        );
    }
}

#[test]
fn software_executes_the_second_original_card_not_the_first_or_scored_card() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B103", A);
    let second = f.tech(
        Color::Red,
        9,
        Zone::Deck(Deck::base(9)),
        vec![build::draw(1)],
    );
    let first = f.tech(
        Color::Yellow,
        9,
        Zone::Deck(Deck::base(9)),
        vec![Instruction::Win(WinRule::Executor)],
    );
    let copied_draw = f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
    let scored = f.tech(
        Color::Green,
        10,
        Zone::Deck(Deck::base(10)),
        vec![Instruction::Win(WinRule::Executor)],
    );
    let chain = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        movements(&events),
        [
            (None, scored),
            (Some(Verb::Score), scored),
            (None, first),
            (Some(Verb::Meld), first),
            (None, second),
            (Some(Verb::Meld), second),
            (None, chain),
            (Some(Verb::Achieve), chain),
            (None, copied_draw)
        ]
    );
}

#[test]
fn ai_requires_both_named_tops_and_uses_the_unique_lowest_score_not_the_executor() {
    for (other, covered, winner) in [
        (9, false, Some(B)),
        (10, false, None),
        (11, false, Some(A)),
        (9, true, None),
    ] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B104", A);
        native(&mut f, "B097", A);
        native(&mut f, "B103", B);
        if covered {
            f.tech(Color::Red, 10, Zone::Board(A, Color::Red), vec![]);
        }
        f.tech(Color::Yellow, other, Zone::Score(B), vec![]);
        f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        assert_eq!(
            finish(&mut game, &mut Vec::new()),
            winner.map_or(Status::Complete(ReturnValue::Unit), Status::Winner)
        );
        assert_eq!(
            game.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .score(A),
            10
        );
    }
}

#[test]
fn ai_compares_signed_effective_scores_under_native_gilgamesh() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B104", A);
    native(&mut f, "B097", A);
    native(&mut f, "B103", B);
    f.figure(Figure::Gilgamesh, Zone::Board(B, Color::Red));
    for color in [Color::Yellow, Color::Green, Color::Blue] {
        icons(&mut f, A, color, Icon::Authority, 4);
    }
    f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    assert_eq!(finish(&mut game, &mut Vec::new()), Status::Winner(A));
    assert_eq!(game.position().score(A), -2);
    assert_eq!(game.position().score(B), 1); // Gilgamesh's printed bonus.
}
