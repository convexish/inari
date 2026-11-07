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
    panic!("reaction did not terminate");
}

fn run(game: &mut Game, request: Operation, player: Player) -> Status {
    game.start(request, Context::new(player)).unwrap();
    finish(game, &mut Vec::new())
}

fn native(f: &mut Fixture, figure: Figure, player: Player) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

#[test]
fn cabral_world_availability_and_own_monotheism_control_replacement() {
    for world in 0..4 {
        for mono in 0..4 {
            let mut f = Fixture::default();
            native(&mut f, Figure::PedroAlvaresCabral, A);
            let source = f.tech(
                Color::Red,
                1,
                Zone::Board(A, Color::Red),
                vec![build::draw(3)],
            );
            f.cards[source.0 as usize].symbols[0] = Symbol::Icon(Icon::Authority);
            let reward = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
            if mono != 0 {
                let owner = if mono == 2 { B } else { A };
                f.add(
                    base_card("B024").unwrap(),
                    Zone::Board(owner, Color::Purple),
                );
                if mono == 3 {
                    f.tech(Color::Purple, 3, Zone::Board(A, Color::Purple), vec![]);
                }
            }
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            if world == 1 || world == 2 {
                run(
                    &mut game,
                    Operation::ClaimSpecial {
                        special: Special::World,
                        check_eligibility: false,
                    },
                    if world == 1 { A } else { B },
                );
            } else if world == 3 {
                run(
                    &mut game,
                    Operation::JunkAvailableSpecial {
                        special: Special::World,
                    },
                    B,
                );
            }
            let status = run(
                &mut game,
                Operation::Dogma {
                    card: source,
                    icon: Icon::Authority,
                },
                A,
            );
            if world != 0 && mono == 1 {
                assert_eq!(status, Status::Winner(A));
            } else {
                assert!(matches!(status, Status::Complete(_)));
            }
            if world == 0 {
                assert_eq!(
                    game.position().special_location(Special::World),
                    SpecialLocation::Owned(A)
                );
            }
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), &[reward]);
        }
    }
}

#[test]
fn cabral_only_live_owner_and_age_one_are_replaced() {
    for (owner, covered, age, replaced) in [
        (A, false, 1, true),
        (B, false, 1, false),
        (A, true, 1, false),
        (A, false, 2, false),
    ] {
        let mut f = Fixture::default();
        native(&mut f, Figure::PedroAlvaresCabral, owner);
        if covered {
            f.tech(Color::Green, 5, Zone::Board(owner, Color::Green), vec![]);
        }
        let source = f.tech(
            Color::Red,
            age,
            Zone::Board(A, Color::Red),
            vec![build::draw(3)],
        );
        f.cards[source.0 as usize].symbols[0] = Symbol::Icon(Icon::Authority);
        let drawn = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        run(
            &mut game,
            Operation::Dogma {
                card: source,
                icon: Icon::Authority,
            },
            A,
        );
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&drawn),
            !replaced
        );
    }
}

#[test]
fn darwin_all_claim_forms_win_before_original_and_suppress_opposing_jackie() {
    for form in 0..4 {
        let mut f = Fixture::default();
        native(&mut f, Figure::CharlesDarwin, A);
        native(&mut f, Figure::JackieChan, B);
        let card = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let request = match form {
            0 => Operation::Claim {
                target: target(card, Zone::Hand(A)),
                eligibility: ClaimEligibility::Ignore,
            },
            1 => Operation::Move {
                verb: Verb::Achieve,
                target: target(card, Zone::Hand(A)),
                destination: Zone::Achievements(A),
            },
            2 => Operation::ClaimSpecial {
                special: Special::World,
                check_eligibility: false,
            },
            _ => Operation::ClaimDecree {
                decree: Decree::War,
            },
        };
        assert_eq!(run(&mut game, request, A), Status::Winner(A));
        assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
        assert_eq!(game.position().achievement_count(A), 0);
        assert_eq!(
            game.position()
                .top(B, Color::Red)
                .map(|c| game.position().card(c).figure),
            Some(Some(Figure::JackieChan))
        );
    }
}

#[test]
fn darwin_behind_draws_then_returns_original_claim_result_and_clone_lifetime() {
    let mut f = Fixture::default();
    native(&mut f, Figure::CharlesDarwin, A);
    f.tech(Color::Red, 1, Zone::Achievements(B), vec![]);
    let card = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let reward = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Claim {
            target: target(card, Zone::Hand(A)),
            eligibility: ClaimEligibility::Ignore,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(moved(&events[0]).card, reward);
    assert!(moved(&events[0]).inside_karma);
    let mut clone = game.clone();
    let expected = Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(2),
        short: false,
    }));
    assert_eq!(finish(&mut game, &mut events), expected);
    assert_eq!(finish(&mut clone, &mut Vec::new()), expected);
    assert_eq!(game.position(), clone.position());
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[card]);
    assert!(!moved(events.last().unwrap()).inside_karma);
}

#[test]
fn darwin_uses_virtual_each_achievements_not_physical_tokens() {
    for virtual_owner in [A, B] {
        let mut f = Fixture::default();
        native(&mut f, Figure::CharlesDarwin, A);
        native(&mut f, Figure::NelsonMandela, virtual_owner);
        let avatar = f.tech(
            Color::Purple,
            1,
            Zone::Board(virtual_owner, Color::Purple),
            vec![],
        );
        f.cards[avatar.0 as usize].symbols[0] = Symbol::Icon(Icon::Avatar);
        let card = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        let draw = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let count_a = game.position().effective_achievement_count(A);
        let count_b = game.position().effective_achievement_count(B);
        assert_eq!(
            (count_a, count_b),
            if virtual_owner == A { (1, 0) } else { (0, 1) }
        );
        let status = run(
            &mut game,
            Operation::Claim {
                target: target(card, Zone::Hand(A)),
                eligibility: ClaimEligibility::Ignore,
            },
            A,
        );
        assert_eq!(status == Status::Winner(A), count_a >= count_b);
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&draw),
            count_a < count_b
        );
    }
}

#[test]
fn invalid_claims_cannot_earn_darwin_first() {
    let mut f = Fixture::default();
    native(&mut f, Figure::CharlesDarwin, A);
    let card = f.tech(Color::Red, 8, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for request in [
        Operation::Claim {
            target: target(card, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Available,
        },
        Operation::Move {
            verb: Verb::Achieve,
            target: target(card, Zone::Hand(A)),
            destination: Zone::Achievements(A),
        },
        Operation::ClaimSpecial {
            special: Special::World,
            check_eligibility: true,
        },
    ] {
        assert!(matches!(run(&mut game, request, A), Status::Complete(_)));
    }
    run(
        &mut game,
        Operation::ClaimDecree {
            decree: Decree::War,
        },
        B,
    );
    assert!(matches!(
        run(
            &mut game,
            Operation::ClaimDecree {
                decree: Decree::War
            },
            A
        ),
        Status::Complete(_)
    ));
}

#[test]
fn pele_requires_both_colors_tied_for_highest_and_preserves_original_score() {
    for (yellow, green, win) in [
        (Some(9), Some(9), true),
        (Some(10), Some(10), true),
        (None, Some(10), false),
        (Some(10), None, false),
        (Some(8), Some(8), false),
        (Some(9), Some(10), false),
    ] {
        let mut f = Fixture::default();
        native(&mut f, Figure::Pele, A);
        native(&mut f, Figure::JackieChan, B);
        for (color, value) in [(Color::Yellow, yellow), (Color::Green, green)] {
            if let Some(value) = value {
                f.tech(color, value, Zone::Board(A, color), vec![]);
            }
        }
        let card = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let status = run(
            &mut game,
            Operation::Move {
                verb: Verb::Score,
                target: target(card, Zone::Hand(A)),
                destination: Zone::Score(A),
            },
            A,
        );
        assert_eq!(status == Status::Winner(A), win);
        assert_eq!(game.position().cards(Zone::Score(A)).contains(&card), !win);
    }
}

#[test]
fn miyamoto_exact_efficiency_and_actual_featured_icon_not_avatar() {
    for count in 1..=7 {
        for icon in [Icon::Efficiency, Icon::Avatar, Icon::Authority] {
            let mut f = Fixture::default();
            native(&mut f, Figure::ShigeruMiyamoto, A);
            native(&mut f, Figure::JackieChan, B);
            let source = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
            f.cards[source.0 as usize].featured = Some(icon);
            let mut remaining = count - 1; // Miyamoto's printed Efficiency.
            for color in [Color::Red, Color::Blue] {
                let card = if color == Color::Red {
                    source
                } else {
                    f.tech(color, 2, Zone::Board(A, color), vec![])
                };
                for symbol in &mut f.cards[card.0 as usize].symbols[..4] {
                    if remaining > 0 {
                        *symbol = Symbol::Icon(Icon::Efficiency);
                        remaining -= 1;
                    }
                }
            }
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            assert_eq!(game.position().icon_count(A, Icon::Efficiency), count);
            let status = run(&mut game, Operation::Dogma { card: source, icon }, A);
            assert_eq!(
                status == Status::Winner(A),
                icon == Icon::Efficiency && matches!(count, 1 | 3 | 6)
            );
        }
    }
}

#[test]
fn any_two_native_figures_grant_only_the_printed_decree() {
    for (figure, decree) in [
        (Figure::PedroAlvaresCabral, Decree::Trade),
        (Figure::CharlesDarwin, Decree::Advancement),
        (Figure::Pele, Decree::Rivalry),
    ] {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        f.figure(Figure::Homer, Zone::Hand(A));
        f.figure(Figure::Shennong, Zone::Hand(A));
        if decree == Decree::Trade {
            for code in ["B050", "B052", "B054"] {
                f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(5)));
            }
        } else if decree == Decree::Advancement {
            f.add(base_card("B090").unwrap(), Zone::Deck(Deck::base(9)));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_opening_turn(A).unwrap();
        game.take_action(Action::Decree, ActionKind::Regular)
            .unwrap();
        let mut events = Vec::new();
        // The hand is junked in any order, and the only permitted decree may
        // be automatically selected.
        let status = finish(&mut game, &mut events);
        if figure == Figure::CharlesDarwin {
            assert_eq!(status, Status::Winner(A));
            assert_eq!(
                game.position().decree_location(decree),
                DecreeLocation::Supply
            );
            assert_eq!(game.position().cards(Zone::Hand(A)).len(), 1);
        } else {
            assert!(matches!(status, Status::Complete(_)));
            assert_eq!(
                game.position().decree_location(decree),
                DecreeLocation::Owned(A)
            );
        }
    }
}

#[test]
fn miyamoto_effective_counts_include_maxwell_each() {
    for held in 0..=3 {
        let mut f = Fixture::default();
        native(&mut f, Figure::ShigeruMiyamoto, A);
        native(&mut f, Figure::JamesClerkMaxwell, A);
        let source = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        for _ in 0..held {
            f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(game.position().icon_count(A, Icon::Efficiency), 2 + held);
        let status = run(
            &mut game,
            Operation::Dogma {
                card: source,
                icon: Icon::Efficiency,
            },
            A,
        );
        assert_eq!(status == Status::Winner(A), held == 1);
    }
}

#[test]
fn darwin_competing_hedy_hook_clones_and_rechecks_before_claim() {
    let mut f = Fixture::default();
    native(&mut f, Figure::CharlesDarwin, A);
    native(&mut f, Figure::HedyLamarr, A);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ClaimDecree {
            decree: Decree::War,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let darwin = menu.options.iter().position(|option| matches!(option,
        Alternative::Karma { source, .. } if game.position().card(*source).figure == Some(Figure::CharlesDarwin)));
    // Selecting Darwin makes terminal-before-original observable regardless of
    // board/color discovery order; its proposal cannot run the other First.
    let index = darwin.expect("both independent First hooks must be offered");
    let mut clone = game.clone();
    game.choose(index).unwrap();
    clone.choose(index).unwrap();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert_eq!(settle(&mut clone, &mut Vec::new()), Status::Winner(A));
    assert_eq!(game.position(), clone.position());
    assert_eq!(
        game.position().decree_location(Decree::War),
        DecreeLocation::Supply
    );
}

#[test]
fn native_stem_cells_reduces_unrelated_figures_but_keeps_pele_score_choices() {
    for figure in [
        Figure::PedroAlvaresCabral,
        Figure::CharlesDarwin,
        Figure::Pele,
        Figure::ShigeruMiyamoto,
    ] {
        let mut f = Fixture::default();
        let source = f.add(base_card("B099").unwrap(), Zone::Board(B, Color::Yellow));
        native(&mut f, figure, A);
        for age in [1, 2, 4] {
            f.tech(Color::Red, age, Zone::Hand(A), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            Context::new(A),
        )
        .unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(
            menu.options.len(),
            if figure == Figure::Pele { 4 } else { 2 }
        );
    }
}

#[test]
fn miyamoto_regular_and_free_actions_use_real_featured_icon_and_auspice() {
    for kind in [ActionKind::Regular, ActionKind::Free] {
        for auspice in [false, true] {
            let mut f = Fixture::default();
            native(&mut f, Figure::ShigeruMiyamoto, A);
            let source = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
            f.cards[source.0 as usize].featured = Some(Icon::Efficiency);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.begin_opening_turn(A).unwrap();
            game.take_action(
                Action::Dogma {
                    card: source,
                    auspice,
                },
                kind,
            )
            .unwrap();
            let status = finish(&mut game, &mut Vec::new());
            assert_eq!(status == Status::Winner(A), !auspice);
        }
    }
}

#[test]
fn darwin_and_pele_require_live_owned_figures_and_valid_operations() {
    for figure in [Figure::CharlesDarwin, Figure::Pele] {
        for (owner, covered) in [(B, false), (A, true)] {
            let mut f = Fixture::default();
            let identity = native(&mut f, figure, owner);
            let color = f.cards[identity.0 as usize].color;
            if covered {
                f.tech(color, 10, Zone::Board(owner, color), vec![]);
            }
            for color in [Color::Yellow, Color::Green] {
                f.tech(color, 10, Zone::Board(A, color), vec![]);
            }
            let card = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let request = if figure == Figure::CharlesDarwin {
                Operation::Claim {
                    target: target(card, Zone::Hand(A)),
                    eligibility: ClaimEligibility::Ignore,
                }
            } else {
                Operation::Move {
                    verb: Verb::Score,
                    target: target(card, Zone::Hand(A)),
                    destination: Zone::Score(A),
                }
            };
            assert!(matches!(run(&mut game, request, A), Status::Complete(_)));
        }
    }
    let mut f = Fixture::default();
    native(&mut f, Figure::Pele, A);
    for color in [Color::Yellow, Color::Green] {
        f.tech(color, 10, Zone::Board(A, color), vec![]);
    }
    let card = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(matches!(
        run(
            &mut game,
            Operation::Move {
                verb: Verb::Score,
                target: target(card, Zone::Hand(A)),
                destination: Zone::Score(A)
            },
            A
        ),
        Status::Complete(_)
    ));
}

#[test]
fn cabral_replaces_regular_and_free_dogma_without_a_turn_ordinal_condition() {
    for kind in [ActionKind::Regular, ActionKind::Free] {
        let mut f = Fixture::default();
        native(&mut f, Figure::PedroAlvaresCabral, A);
        let source = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_opening_turn(A).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            kind,
        )
        .unwrap();
        assert!(matches!(
            finish(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        assert_eq!(
            game.position().special_location(Special::World),
            SpecialLocation::Owned(A)
        );
        assert_eq!(
            game.position().turn_owner(),
            if kind == ActionKind::Regular { B } else { A }
        );
    }
}

#[test]
fn miyamoto_covered_or_foreign_does_not_propose_a_win() {
    for (owner, covered) in [(A, true), (B, false)] {
        let mut f = Fixture::default();
        native(&mut f, Figure::ShigeruMiyamoto, owner);
        if covered {
            f.tech(Color::Yellow, 10, Zone::Board(A, Color::Yellow), vec![]);
        }
        let source = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        f.cards[source.0 as usize].symbols[0] = Symbol::Icon(Icon::Efficiency);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(game.position().icon_count(A, Icon::Efficiency), 1);
        assert!(matches!(
            run(
                &mut game,
                Operation::Dogma {
                    card: source,
                    icon: Icon::Efficiency
                },
                A
            ),
            Status::Complete(_)
        ));
    }
}
