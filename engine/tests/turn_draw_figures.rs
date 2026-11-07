use inno_engine::*;

mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = menu
                    .options
                    .iter()
                    .position(|option| {
                        matches!(
                            option,
                            Alternative::Karma {
                                figure: Figure::HuangDi,
                                ..
                            }
                        )
                    })
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("turn/draw Figure invocation did not finish");
}

fn draw(value: u8) -> Instruction {
    build::draw(value)
}

fn source(f: &mut Fixture, player: Player, instructions: Vec<Instruction>) -> CardId {
    let card = f.tech(Color::Red, 1, Zone::Board(player, Color::Red), instructions);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    card
}

fn karma_moves(events: &[Event]) -> Vec<&Movement> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(movement) if movement.inside_karma => Some(movement),
            _ => None,
        })
        .collect()
}

#[test]
fn first_draw_requires_the_real_own_first_regular_action_not_fixture_or_free_timing() {
    for figure in [Figure::Shennong, Figure::FuXi] {
        for mode in 0..8 {
            let mut f = Fixture::default();
            let color = figure_card(figure.code()).unwrap().color;
            f.figure(figure, Zone::Board(A, color));
            let code = source(
                &mut f,
                if matches!(mode, 4 | 7) { B } else { A },
                vec![draw(1)],
            );
            if mode == 7 {
                f.cards[usize::from(code.0)].symbols = [Symbol::Blank; SLOTS];
            }
            f.tech(Color::Purple, 1, Zone::Hand(A), vec![]);
            for _ in 0..4 {
                f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
                f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
            }
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            if matches!(mode, 1..=3) {
                game.begin_action(ActionKind::Regular).unwrap();
            }
            if matches!(mode, 4 | 7) {
                game.begin_turn(B).unwrap();
            }
            if mode == 2 {
                game.start(
                    Operation::Draw {
                        value: 1,
                        set: None,
                    },
                    Context::new(A),
                )
                .unwrap();
            } else if mode == 5 {
                game.self_execute(code, Context::new(A)).unwrap();
            } else {
                game.take_action(
                    Action::Dogma {
                        card: code,
                        auspice: false,
                    },
                    if matches!(mode, 3 | 6) {
                        ActionKind::Free
                    } else {
                        ActionKind::Regular
                    },
                )
                .unwrap();
            }
            let mut events = Vec::new();
            assert!(matches!(
                finish(&mut game, &mut events),
                Status::Complete(_)
            ));
            let reactions = karma_moves(&events);
            assert_eq!(
                reactions.len(),
                if mode == 0 { 2 } else { 0 },
                "{figure:?}, mode {mode}"
            );
            if mode == 0 {
                assert_eq!(reactions[0].verb, None);
                assert_eq!(reactions[1].verb, Some(Verb::Score));
                assert_eq!(reactions[0].card, reactions[1].card);
                assert_eq!(reactions[0].executor, A);
            }
            assert!(game.position().check_conservation());
        }
    }
}

#[test]
fn shennong_uses_resolved_one_and_live_meiji_hand_count_including_zero() {
    for (count, meiji, resolved) in [(0, false, 1), (2, false, 1), (2, true, 1), (2, true, 2)] {
        let mut f = Fixture::default();
        f.figure(Figure::Shennong, Zone::Board(A, Color::Yellow));
        if meiji {
            f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        }
        let code = source(&mut f, A, vec![draw(1)]);
        for _ in 0..count {
            f.tech(
                Color::Green,
                1,
                Zone::Hand(if meiji { B } else { A }),
                vec![],
            );
        }
        // Non-ones never contribute. Meiji's effective hand must not change origins.
        let ignored = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
        let original = f.tech(
            Color::Blue,
            resolved,
            Zone::Deck(Deck::base(resolved)),
            vec![],
        );
        let computed = if resolved == 1 {
            Some(f.tech(Color::Green, count, Zone::Deck(Deck::base(count)), vec![]))
        } else {
            None
        };
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: code,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            finish(&mut game, &mut events),
            Status::Complete(_)
        ));
        let reactions = karma_moves(&events);
        if let Some(computed) = computed {
            assert_eq!(
                reactions.iter().map(|m| m.card).collect::<Vec<_>>(),
                [computed, computed]
            );
            assert_eq!(reactions[0].source, Zone::Deck(Deck::base(count)));
            assert!(game.position().cards(Zone::Score(A)).contains(&computed));
        } else {
            assert!(reactions.is_empty());
        }
        assert!(game.position().cards(Zone::Hand(A)).contains(&original));
        assert!(game.position().cards(Zone::Hand(B)).contains(&ignored));
        assert!(game.position().check_conservation());
    }
}

#[test]
fn shennong_zero_computation_falls_forward_without_an_age_zero_deck() {
    let mut f = Fixture::default();
    f.figure(Figure::Shennong, Zone::Board(A, Color::Yellow));
    let code = source(&mut f, A, vec![draw(1)]);
    let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let scored = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: code,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[original]);
}

#[test]
fn fu_xi_fixed_box_ignores_archimedes_and_original_compound_keeps_its_draw_identity() {
    let mut f = Fixture::default();
    f.figure(Figure::FuXi, Zone::Board(A, Color::Green));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let code = source(
        &mut f,
        A,
        vec![
            draw(2),
            build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
        ],
    );
    let fixed = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let original = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: code,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    let movements: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.card == fixed || m.card == original => {
                Some((m.card, m.verb, m.inside_karma))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        movements,
        [
            (fixed, None, true),
            (fixed, Some(Verb::Score), true),
            (original, None, false),
            (original, Some(Verb::Score), false)
        ]
    );
}

#[test]
fn first_reactions_preserve_original_figures_set_and_reresolve_after_consuming_last_base_age() {
    let mut f = Fixture::default();
    f.figure(Figure::FuXi, Zone::Board(A, Color::Green));
    let code = source(&mut f, A, vec![build::draw(2).set(CardSet::Figures)]);
    let first = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let original = f.figure(Figure::AlfredNobel, Zone::Deck(Deck::figures(7)));
    // The requested Figures age follows Base resolution, not an independently available Figure deck.
    f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
    let at_three = f.figure(Figure::TranHuangDao, Zone::Deck(Deck::figures(3)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: code,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Score(A)), &[first]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[at_three]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(7))),
        &[original]
    );
}

#[test]
fn huang_di_uses_actual_featured_icon_and_executor_even_for_foreign_dogma() {
    for foreign in [false, true] {
        for auspice in [false, true] {
            let mut f = Fixture::default();
            f.figure(Figure::HuangDi, Zone::Board(A, Color::Blue));
            if auspice {
                f.figure(Figure::Shennong, Zone::Board(A, Color::Yellow));
            }
            if foreign {
                f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
            }
            let card = source(&mut f, if foreign { B } else { A }, vec![]);
            f.cards[usize::from(card.0)].featured = Some(Icon::Health);
            f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Avatar);
            let reward = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.take_action(Action::Dogma { card, auspice }, ActionKind::Free)
                .unwrap();
            let mut events = Vec::new();
            assert!(matches!(
                finish(&mut game, &mut events),
                Status::Complete(_)
            ));
            let draws: Vec<_> = karma_moves(&events)
                .into_iter()
                .filter(|m| m.verb.is_none())
                .collect();
            assert_eq!(draws.len(), usize::from(!auspice));
            if !auspice {
                assert_eq!(draws[0].card, reward);
                assert_eq!(draws[0].executor, A);
            }
            assert!(game.position().check_conservation());
        }
    }
    let mut f = Fixture::default();
    f.figure(Figure::HuangDi, Zone::Board(A, Color::Blue));
    let card = source(&mut f, B, vec![]);
    let reward = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Dogma {
            card,
            icon: Icon::Health,
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert!(karma_moves(&events).is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[reward]);
}

#[test]
fn ericsson_unsplays_only_opponent_right_source_color_before_dogma() {
    for direction in [Splay::None, Splay::Left, Splay::Right, Splay::Up] {
        for executor in [A, B] {
            let mut f = Fixture::default();
            f.figure(Figure::JohnEricsson, Zone::Board(A, Color::Red));
            let code = f.tech(Color::Blue, 1, Zone::Board(executor, Color::Blue), vec![]);
            for player in [A, B] {
                f.tech(Color::Blue, 1, Zone::Board(player, Color::Blue), vec![]);
                f.tech(Color::Blue, 1, Zone::Board(player, Color::Blue), vec![]);
                f.tech(Color::Green, 1, Zone::Board(player, Color::Green), vec![]);
                f.tech(Color::Green, 1, Zone::Board(player, Color::Green), vec![]);
            }
            // Make the selected code a live top with two cards beneath it.
            let code2 = f.tech(Color::Blue, 1, Zone::Board(executor, Color::Blue), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            for (player, color) in [(A, Color::Blue), (B, Color::Blue), (B, Color::Green)] {
                game.start(
                    Operation::Splay {
                        player,
                        color,
                        direction,
                    },
                    Context::new(A),
                )
                .unwrap();
                finish(&mut game, &mut Vec::new());
            }
            game.start(
                Operation::Dogma {
                    card: code2,
                    icon: Icon::Authority,
                },
                Context::new(executor),
            )
            .unwrap();
            let mut events = Vec::new();
            assert!(matches!(
                finish(&mut game, &mut events),
                Status::Complete(_)
            ));
            let expected = if executor == A && direction == Splay::Right {
                Splay::None
            } else {
                direction
            };
            assert_eq!(game.position().splay(B, Color::Blue), expected);
            assert_eq!(game.position().splay(A, Color::Blue), direction);
            assert_eq!(game.position().splay(B, Color::Green), direction);
            assert!(
                game.position()
                    .cards(Zone::Board(executor, Color::Blue))
                    .contains(&code)
            );
        }
    }
}

#[test]
fn ericsson_opponent_draw_tucks_as_owner_suppresses_other_hooks_and_retains_original_result() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnEricsson, Zone::Board(A, Color::Red));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let tucked = f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(B);
    context.controller = A;
    context.source_owner = A;
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(original),
            value: Some(1),
            short: false,
        }))
    );
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).card, moved(e).executor, moved(e).inside_karma))
            .collect::<Vec<_>>(),
        [(tucked, A, true), (tucked, A, true), (original, B, false)]
    );
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red))[0], tucked);
    assert!(game.position().has_figure(A, Figure::JohnEricsson));
    assert!(game.position().check_conservation());
}

#[test]
fn competing_first_hooks_choose_one_and_clone_every_compound_operation_boundary() {
    let mut f = Fixture::default();
    f.figure(Figure::Shennong, Zone::Board(A, Color::Yellow));
    f.figure(Figure::FuXi, Zone::Board(A, Color::Green));
    let code = source(&mut f, A, vec![draw(1)]);
    f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    for value in [1, 1, 2] {
        f.tech(Color::Blue, value, Zone::Deck(Deck::base(value)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: code,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut prefix = Vec::new();
    let menu = decision(&mut game, &mut prefix);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    for index in 0..2 {
        let mut branch = game.clone();
        branch.choose(index).unwrap();
        let mut expected = branch.clone();
        let mut all = Vec::new();
        let status = finish(&mut expected, &mut all);
        assert_eq!(karma_moves(&all).len(), 2);
        let mut prefix = Vec::new();
        for _ in 0..200 {
            let mut fork = branch.clone();
            let mut suffix = Vec::new();
            assert_eq!(finish(&mut fork, &mut suffix), status);
            assert_eq!(fork, expected);
            assert_eq!(
                prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
                all
            );
            match branch.advance(1, &mut prefix).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(_) => branch.choose(0).unwrap(),
                end => {
                    assert_eq!(end, status);
                    break;
                }
            }
        }
    }
}

#[test]
fn exhausted_ericsson_child_is_terminal_before_tuck_or_original_draw() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnEricsson, Zone::Board(A, Color::Red));
    let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        Context::new(B),
    )
    .unwrap();
    let mut fork = game.clone();
    let mut events = Vec::new();
    let terminal = finish(&mut game, &mut events);
    assert!(matches!(terminal, Status::Winner(_) | Status::Draw(_)));
    assert!(events.is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        &[original]
    );
    assert_eq!(finish(&mut fork, &mut Vec::new()), terminal);
    assert_eq!(fork, game);
}

#[test]
fn native_any_two_decree_permissions_are_live_and_require_two_figures() {
    for (figure, decree) in [
        (Figure::FuXi, Decree::Trade),
        (Figure::HuangDi, Decree::Advancement),
    ] {
        for covered in [false, true] {
            for count in 1..=2 {
                let mut f = Fixture::default();
                let color = figure_card(figure.code()).unwrap().color;
                f.figure(figure, Zone::Board(A, color));
                if covered {
                    f.tech(color, 1, Zone::Board(A, color), vec![]);
                }
                f.figure(Figure::Shennong, Zone::Hand(A));
                if count == 2 {
                    f.figure(Figure::Tigernmas, Zone::Hand(A));
                }
                for _ in 0..3 {
                    f.tech(Color::Red, 5, Zone::Deck(Deck::base(5)), vec![]);
                }
                let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
                if covered || count == 1 {
                    let before = game.clone();
                    assert_eq!(
                        game.take_action(Action::Decree, ActionKind::Free),
                        Err(Error::InvalidAction)
                    );
                    assert_eq!(game, before);
                    continue;
                }
                game.take_action(Action::Decree, ActionKind::Free).unwrap();
                let mut events = Vec::new();
                assert!(matches!(
                    finish(&mut game, &mut events),
                    Status::Complete(_)
                ));
                assert_eq!(
                    game.position().decree_location(decree),
                    DecreeLocation::Owned(A)
                );
                assert!(game.position().check_conservation());
            }
        }
    }
}

#[test]
fn draw_hazards_restore_oars_choices_but_leave_tools_return_reduction_intact() {
    for figure in [
        Figure::Shennong,
        Figure::FuXi,
        Figure::HuangDi,
        Figure::JohnEricsson,
    ] {
        let mut f = Fixture::default();
        let color = figure_card(figure.code()).unwrap().color;
        f.figure(figure, Zone::Board(B, color));
        let oars = f.oars(Zone::Board(A, Color::Red));
        let hand: Vec<_> = [4, 2]
            .into_iter()
            .map(|age| {
                let card = f.tech(Color::Blue, age, Zone::Hand(B), vec![]);
                f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
                card
            })
            .collect();
        for _ in 0..2 {
            f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(B);
        context.demander = A;
        context.source_owner = A;
        game.start(
            Operation::ExecuteEffect {
                card: oars,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        if figure == Figure::HuangDi {
            assert!(matches!(
                finish(&mut game, &mut events),
                Status::Complete(_)
            ));
            let transfers: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::Moved(m) if m.verb == Some(Verb::Transfer) => Some(m.card),
                    _ => None,
                })
                .collect();
            assert_eq!(transfers, [hand[1], hand[0]]);
        } else {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, B);
            assert_eq!(menu.options.len(), 2);
            assert!(events.is_empty());
        }

        let mut f = Fixture::default();
        f.figure(figure, Zone::Board(B, color));
        let tools = f.tools_first(Zone::Board(A, Color::Blue));
        for age in [3, 1, 2] {
            f.tech(Color::Red, age, Zone::Hand(A), vec![]);
        }
        f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(tools, Context::new(A)).unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.options.len(), 2, "{figure:?}");
        assert!(menu.options.contains(&Alternative::Decline));
    }
}

#[test]
fn source_covering_during_first_action_removes_the_draw_hook_immediately() {
    for figure in [Figure::Shennong, Figure::FuXi] {
        let mut f = Fixture::default();
        let color = figure_card(figure.code()).unwrap().color;
        f.figure(figure, Zone::Board(A, color));
        let cover = f.tech(color, 1, Zone::Hand(A), vec![]);
        let code = source(
            &mut f,
            A,
            vec![
                build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
                draw(1),
            ],
        );
        let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let untouched = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: code,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            finish(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert!(karma_moves(&events).is_empty());
        assert_eq!(game.position().top(A, color), Some(cover));
        assert_eq!(game.position().cards(Zone::Hand(A)), &[original]);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(2))),
            &[untouched]
        );
    }
}

#[test]
fn ericsson_does_not_react_to_own_draws_or_while_covered() {
    for mode in [0, 2] {
        let mut f = Fixture::default();
        f.figure(Figure::JohnEricsson, Zone::Board(A, Color::Red));
        if mode == 2 {
            f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        }
        let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let untouched = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
        let context = Context::new(if mode == 0 { A } else { B });
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Draw {
                value: 1,
                set: None,
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            finish(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).card, original);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(7))),
            &[untouched]
        );
    }
}

#[test]
fn ordinary_first_draw_action_runs_compound_first_before_the_original_draw() {
    for figure in [Figure::Shennong, Figure::FuXi] {
        let mut f = Fixture::default();
        let color = figure_card(figure.code()).unwrap().color;
        f.figure(figure, Zone::Board(A, color));
        f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let age = if figure == Figure::Shennong { 1 } else { 2 };
        let reward = f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(Action::Draw, ActionKind::Regular).unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            finish(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(
            events
                .iter()
                .map(|e| (moved(e).card, moved(e).inside_karma))
                .collect::<Vec<_>>(),
            [(reward, true), (reward, true), (original, false)]
        );
        assert_eq!(game.position().cards(Zone::Score(A)), &[reward]);
    }
}

#[test]
fn earned_free_draw_inside_first_regular_tail_does_not_repeat_fu_xi() {
    let mut f = Fixture::default();
    f.figure(Figure::FuXi, Zone::Board(A, Color::Green));
    let code = source(&mut f, A, vec![draw(1)]);
    f.cards[usize::from(code.0)].symbols = [Symbol::Blank; SLOTS];
    for _ in 0..3 {
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    let untouched = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let scored = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: code,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        karma_moves(&events)
            .iter()
            .map(|m| m.card)
            .collect::<Vec<_>>(),
        [scored, scored]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[untouched]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
    assert_eq!(game.position().regular_actions_started(), 1);
}

#[test]
fn ericsson_foreign_dogma_uses_executor_and_selected_color_with_real_competing_brin() {
    let mut f = Fixture::default();
    let ericsson = f.figure(Figure::JohnEricsson, Zone::Board(A, Color::Red));
    f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let card = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: B,
            color: Color::Blue,
            direction: Splay::Right,
        },
        Context::new(B),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    game.start(
        Operation::Dogma {
            card,
            icon: Icon::Authority,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    let index = menu
        .options
        .iter()
        .position(|option| {
            *option
                == Alternative::Karma {
                    source: ericsson,
                    figure: Figure::JohnEricsson,
                }
        })
        .unwrap();
    game.choose(index).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().splay(B, Color::Blue), Splay::None);
    assert_eq!(game.position().top(B, Color::Blue), Some(card));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
}

#[test]
fn huang_di_dogma_hook_is_not_a_copied_execution_hook_and_obeys_covering() {
    for mode in [0, 2] {
        let mut f = Fixture::default();
        f.figure(Figure::HuangDi, Zone::Board(A, Color::Blue));
        if mode == 0 {
            f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        }
        let card = source(&mut f, A, vec![]);
        f.cards[usize::from(card.0)].featured = Some(Icon::Health);
        let reward = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if mode == 2 {
            game.self_execute(card, Context::new(A)).unwrap();
        } else {
            let context = Context::new(A);
            game.start(
                Operation::Dogma {
                    card,
                    icon: Icon::Health,
                },
                context,
            )
            .unwrap();
        }
        let mut events = Vec::new();
        assert!(matches!(
            finish(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert!(events.is_empty());
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[reward]);
    }
}
