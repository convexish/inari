use inno_engine::*;

mod common;
use common::*;

fn card(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let definition = base_card(code).unwrap();
    let color = definition.color;
    f.add(definition, Zone::Board(player, color))
}

fn figure(f: &mut Fixture, value: Figure, player: Player) -> CardId {
    let definition = figure_card(value.code()).unwrap();
    let color = definition.color;
    f.add(definition, Zone::Board(player, color))
}

fn invoke(game: &mut Game, source: CardId, effect: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn action(game: &mut Game, source: CardId) {
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = menu
                    .options
                    .iter()
                    .position(|o| *o == Alternative::Decline)
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("unfinished native demand");
}

fn done(game: &mut Game, events: &mut Vec<Event>) {
    assert_eq!(finish(game, events), Status::Complete(ReturnValue::Unit));
}

fn known(menu: &Decision) -> Vec<CardId> {
    menu.options
        .iter()
        .filter_map(|o| match o {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(*card),
            _ => None,
        })
        .collect()
}

fn moves(events: &[Event]) -> Vec<&Movement> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some(m),
            _ => None,
        })
        .collect()
}

#[test]
fn archery_draws_before_selecting_every_highest_tie_and_uses_the_actual_demander() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B001", A);
    let high = [
        card(&mut f, "B037", Zone::Hand(B)),
        card(&mut f, "B042", Zone::Hand(B)),
    ];
    let drawn = card(&mut f, "B008", Zone::Deck(Deck::base(1)));
    let initial = f.position();
    for index in 0..2 {
        let mut game = Game::new(initial.clone(), Expansions::BASE).unwrap();
        invoke(&mut game, source, 0, B, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        assert_eq!(known(&menu), high);
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).card, drawn);
        assert!(game.choose(2).is_err());
        game.choose(index).unwrap();
        done(&mut game, &mut events);
        let movement = moved(&events[1]);
        assert_eq!(movement.executor, B);
        assert_eq!(movement.source, Zone::Hand(B));
        assert_eq!(movement.destination, Zone::Hand(A));
        assert_eq!(game.position().cards(Zone::Hand(A)), [high[index]]);
    }
}

#[test]
fn archery_preserves_hidden_standard_supply_and_plain_values_ignore_archimedes() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B001", A);
    figure(&mut f, Figure::Archimedes, A);
    figure(&mut f, Figure::JohnLoudonMcAdam, A);
    let supply = [
        card(&mut f, "B004", Zone::AvailableAchievements),
        card(&mut f, "B023", Zone::AvailableAchievements),
    ];
    let three = card(&mut f, "B034", Zone::AvailableAchievements);
    let owned = card(&mut f, "B008", Zone::Achievements(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 1, A, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(
        known(&menu).is_empty(),
        "achievement fronts must remain hidden"
    );
    assert!(!menu.options.contains(&Alternative::Decline));
    game.choose(0).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Junk), [supply[0]]);
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        [supply[1], three]
    );
    assert_eq!(game.position().cards(Zone::Achievements(A)), [owned]);
    assert_eq!(game.position().top(A, Color::Red), Some(source));
    assert!(
        Special::ALL
            .into_iter()
            .all(|s| game.position().special_location(s) == SpecialLocation::Available)
    );
}

#[test]
fn archery_empty_supply_does_nothing_and_draw_exhaustion_precedes_transfer() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B001", A);
    let held = card(&mut f, "B042", Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 1, A, A);
    done(&mut game, &mut Vec::new());
    invoke(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(B)), [held]);
}

#[test]
fn mapmaking_counts_original_transfer_and_adjusts_filter_and_reward_in_separate_scopes() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B021", A);
    figure(&mut f, Figure::Archimedes, A);
    let plain_one = card(&mut f, "B004", Zone::Score(B));
    let transferred = card(&mut f, "B023", Zone::Score(B));
    let reward = card(&mut f, "B024", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    action(&mut game, source);
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(B)), [plain_one]);
    assert_eq!(game.position().cards(Zone::Score(A)), [transferred, reward]);
    assert_eq!(
        moves(&events)
            .iter()
            .map(|m| (m.card, m.verb, m.executor))
            .collect::<Vec<_>>(),
        [
            (transferred, Some(Verb::Transfer), B),
            (reward, None, A),
            (reward, Some(Verb::Score), A),
        ]
    );
}

#[test]
fn mapmaking_uses_the_demanders_archimedes_not_the_victims_in_dogma_and_super_execution() {
    for owner in [A, B] {
        for copied in [false, true] {
            let mut f = Fixture::default();
            // Super-executing a foreign source must still use A's karma.
            let source = board(&mut f, "B021", if copied { B } else { A });
            figure(&mut f, Figure::Archimedes, owner);
            let ones = [
                card(&mut f, "B004", Zone::Score(B)),
                card(&mut f, "B008", Zone::Score(B)),
            ];
            let twos = [
                card(&mut f, "B023", Zone::Score(B)),
                card(&mut f, "B024", Zone::Score(B)),
            ];
            let plain_reward = card(&mut f, "B010", Zone::Deck(Deck::base(1)));
            let boosted_reward = card(&mut f, "B022", Zone::Deck(Deck::base(2)));
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            if copied {
                let mut context = Context::new(A);
                context.source_owner = B;
                game.super_execute(source, context).unwrap();
            } else {
                action(&mut game, source);
            }
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            let eligible = if owner == A { twos } else { ones };
            assert_eq!(menu.player, B, "the victim still chooses the transfer");
            assert_eq!(known(&menu), eligible);
            assert_eq!(menu.options.len(), 2);
            assert!(events.is_empty());
            let mut fork = game.clone();
            game.choose(1).unwrap();
            done(&mut game, &mut events);
            let reward = if owner == A {
                boosted_reward
            } else {
                plain_reward
            };
            assert_eq!(game.position().cards(Zone::Score(A)), [eligible[1], reward]);
            assert_eq!(game.position().cards(Zone::Score(B)).len(), 3);
            assert_eq!(moved(&events[0]).executor, B);
            assert_eq!(moved(&events[0]).destination, Zone::Score(A));
            assert_eq!(events.len(), 3);

            // A pending victim choice and the adjusted boxed scope survive
            // cloning and one-step work budgets unchanged.
            fork.choose(1).unwrap();
            let mut replay = Vec::new();
            let mut status = Status::WorkLimit;
            for _ in 0..1000 {
                status = fork.advance(1, &mut replay).unwrap();
                assert!(fork.position().check_conservation());
                if status != Status::WorkLimit {
                    break;
                }
            }
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(fork, game);
            assert_eq!(replay, events);
        }
    }
}

#[test]
fn mapmaking_and_gunpowder_shared_followups_read_the_same_empty_demand_summary() {
    for code in ["B021", "B037"] {
        let mut f = Fixture::default();
        let source = board(&mut f, code, A);
        if code == "B021" {
            board(&mut f, "B041", B);
        } else {
            board(&mut f, "B057", B);
        }
        card(&mut f, "B004", Zone::Score(B));
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        action(&mut game, source);
        let mut events = Vec::new();
        done(&mut game, &mut events);
        assert!(
            events.is_empty(),
            "no demand ran, so neither shared invocation may draw"
        );
    }
}

#[test]
fn navigation_keeps_all_boxed_value_ties_without_a_decline_or_false_exhaustion() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B041", A);
    figure(&mut f, Figure::Archimedes, A);
    let two = card(&mut f, "B023", Zone::Score(B));
    let matches = [
        card(&mut f, "B034", Zone::Score(B)),
        card(&mut f, "B035", Zone::Score(B)),
        card(&mut f, "B042", Zone::Score(B)),
    ];
    let five = card(&mut f, "B052", Zone::Score(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3);
    assert_eq!(known(&menu), matches);
    assert!(!menu.options.contains(&Alternative::Decline));
    game.choose(1).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [matches[1]]);
    assert_eq!(
        game.position().cards(Zone::Score(B)),
        [two, matches[0], matches[2], five]
    );
}

#[test]
fn monotheism_compares_colors_with_demander_not_executor_or_source_owner() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B024", B);
    board(&mut f, "B008", A);
    let excluded = board(&mut f, "B021", B);
    let reward = card(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    // The retained printed source itself is a legal top to transfer: source
    // ownership does not change the player saying "my board" in the demand.
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [source]);
    assert_eq!(game.position().top(B, Color::Purple), None);
    assert_eq!(game.position().top(B, Color::Green), Some(excluded));
    assert_eq!(game.position().top(B, Color::Yellow), Some(reward));
    assert_eq!(
        moves(&events).iter().map(|m| m.verb).collect::<Vec<_>>(),
        [Some(Verb::Transfer), None, Some(Verb::Tuck)]
    );
    assert!(moves(&events).iter().all(|m| m.executor == B));
}

#[test]
fn monotheism_demand_with_no_new_color_has_no_reward_but_its_second_effect_is_independent() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B024", A);
    board(&mut f, "B035", B);
    let reward = card(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert!(events.is_empty());
    invoke(&mut game, source, 1, B, A);
    done(&mut game, &mut events);
    assert_eq!(game.position().top(B, Color::Yellow), Some(reward));
}

#[test]
fn top_icon_filters_use_printed_cards_and_keep_all_eligible_noncolor_choices() {
    for (code, candidates, excluded, plain, reward_code, age, destination, verb) in [
        (
            "B044",
            ["B003", "B012"],
            "B014",
            "B004",
            "B042",
            4,
            Destination::Board(Who::Demander),
            Verb::Meld,
        ),
        (
            "B050",
            ["B037", "B052"],
            "B061",
            "B004",
            "B050",
            5,
            Destination::Board(Who::Demander),
            Verb::Score,
        ),
        (
            "B080",
            ["B037", "B052"],
            "B061",
            "B004",
            "B082",
            8,
            Destination::Score(Who::Demander),
            Verb::Meld,
        ),
    ] {
        let mut f = Fixture::default();
        let source = board(&mut f, code, A);
        let eligible = candidates.map(|c| board(&mut f, c, B));
        let excluded = board(&mut f, excluded, B);
        let plain = board(&mut f, plain, B);
        let reward = card(&mut f, reward_code, Zone::Deck(Deck::base(age)));
        let initial = f.position();
        for index in 0..2 {
            let mut game = Game::new(initial.clone(), Expansions::BASE).unwrap();
            invoke(&mut game, source, 0, B, A);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            let offered = known(&menu);
            assert_eq!(offered.len(), 2);
            assert!(eligible.iter().all(|id| offered.contains(id)));
            assert!(!offered.contains(&excluded));
            assert!(!offered.contains(&plain));
            game.choose(
                offered
                    .iter()
                    .position(|id| *id == eligible[index])
                    .unwrap(),
            )
            .unwrap();
            done(&mut game, &mut events);
            let ms = moves(&events);
            assert_eq!(
                ms.iter().map(|m| m.verb).collect::<Vec<_>>(),
                [Some(Verb::Transfer), None, Some(verb)]
            );
            assert!(ms.iter().all(|m| m.executor == B));
            let color = game.position().card(eligible[index]).color;
            assert_eq!(
                ms[0].destination,
                if destination == Destination::Board(Who::Demander) {
                    Zone::Board(A, color)
                } else {
                    Zone::Score(A)
                }
            );
            assert_eq!(ms[2].card, reward);
            assert_ne!(game.position().top(B, color), Some(eligible[index]));
        }
    }
}

#[test]
fn gunpowder_uses_printed_authority_not_effective_authority_on_a_top_card() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B037", A);
    let gilgamesh = figure(&mut f, Figure::Gilgamesh, B);
    let ineligible = board(&mut f, "B008", B);
    let eligible = board(&mut f, "B005", B);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    // Gilgamesh's top Red card has printed Authority too; both printed ties
    // survive, but the effective icon modifier cannot make Sailing eligible.
    let menu = decision(&mut game, &mut events);
    assert_eq!(known(&menu).len(), 2);
    assert!(known(&menu).contains(&gilgamesh));
    assert!(known(&menu).contains(&eligible));
    assert!(!known(&menu).contains(&ineligible));
    game.choose(known(&menu).iter().position(|id| *id == eligible).unwrap())
        .unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [eligible]);
}

#[test]
fn feudalism_junks_available_specials_only_after_a_successful_original_transfer() {
    for pay in [false, true] {
        let mut f = Fixture::default();
        let source = board(&mut f, "B035", A);
        let ineligible = card(&mut f, "B004", Zone::Hand(B));
        let payment = pay.then(|| card(&mut f, "B001", Zone::Hand(B)));
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.start(
            Operation::ClaimSpecial {
                special: Special::World,
                check_eligibility: false,
            },
            Context::new(A),
        )
        .unwrap();
        finish(&mut game, &mut Vec::new());
        invoke(&mut game, source, 0, B, A);
        let mut events = Vec::new();
        done(&mut game, &mut events);
        assert_eq!(
            game.position().special_location(Special::World),
            SpecialLocation::Owned(A)
        );
        for special in [
            Special::Monument,
            Special::Empire,
            Special::Wonder,
            Special::Universe,
        ] {
            assert_eq!(
                game.position().special_location(special),
                if pay {
                    SpecialLocation::Junk
                } else {
                    SpecialLocation::Available
                }
            );
        }
        assert_eq!(game.position().cards(Zone::Hand(B)), [ineligible]);
        assert_eq!(game.position().cards(Zone::Hand(A)), payment.as_slice());
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(
                    e,
                    Event::SpecialMoved {
                        executor: B,
                        destination: SpecialLocation::Junk,
                        ..
                    }
                ))
                .count(),
            if pay { 4 } else { 0 }
        );
    }
}

#[test]
fn feudalism_splay_is_optional_and_only_an_actual_splay_earns_its_boxed_draw() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let source = board(&mut f, "B035", A);
        board(&mut f, "B004", A);
        board(&mut f, "B005", A);
        figure(&mut f, Figure::Archimedes, A);
        let reward = card(&mut f, "B042", Zone::Deck(Deck::base(4)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut game, source, 1, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options,
            [
                Alternative::Splay {
                    player: A,
                    color: Color::Yellow,
                    direction: Splay::Left
                },
                Alternative::Decline
            ]
        );
        game.choose(if accept { 0 } else { 1 }).unwrap();
        done(&mut game, &mut events);
        assert_eq!(
            game.position().cards(Zone::Hand(A)),
            if accept {
                std::slice::from_ref(&reward)
            } else {
                &[]
            }
        );
        invoke(&mut game, source, 1, A, A);
        if accept {
            let before = events.len();
            done(&mut game, &mut events);
            assert_eq!(
                events.len(),
                before,
                "already-left is not a performed splay"
            );
        } else {
            finish(&mut game, &mut events);
        }
    }
}

#[test]
fn enterprise_and_banking_keep_their_independent_optional_green_splay() {
    for code in ["B044", "B050"] {
        for accept in [false, true] {
            let mut f = Fixture::default();
            board(&mut f, "B008", A);
            board(&mut f, "B021", A);
            let source = board(&mut f, code, A);
            let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
            invoke(&mut game, source, 1, A, A);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(
                menu.options,
                [
                    Alternative::Splay {
                        player: A,
                        color: Color::Green,
                        direction: Splay::Right
                    },
                    Alternative::Decline
                ]
            );
            game.choose(if accept { 0 } else { 1 }).unwrap();
            done(&mut game, &mut events);
            assert_eq!(
                game.position().splay(A, Color::Green),
                if accept { Splay::Right } else { Splay::None }
            );
        }
    }
}

#[test]
fn corporations_independent_draw_and_meld_has_the_executors_attribution() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B080", A);
    let reward = card(&mut f, "B082", Zone::Deck(Deck::base(8)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 1, B, A);
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().top(B, Color::Blue), Some(reward));
    assert!(moves(&events).iter().all(|m| m.executor == B));
}

#[test]
fn private_front_filters_fail_closed_but_highest_back_selection_remains_private() {
    let mut f = Fixture::default();
    let feudalism = board(&mut f, "B035", A);
    card(&mut f, "B001", Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut context = Context::new(B);
    context.controller = A;
    context.demander = A;
    game.start(
        Operation::ExecuteEffect {
            card: feudalism,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    assert_eq!(
        game.advance(1000, &mut Vec::new()),
        Err(Error::Unsupported("private filtered card selection"))
    );

    let mut f = Fixture::default();
    let archery = board(&mut f, "B001", A);
    let ties = ["B037", "B042"].map(|code| card(&mut f, code, Zone::Hand(B)));
    let drawn = card(&mut f, "B008", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: archery,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    // A sees two [4] backs: one native sample transfers, with no menu.
    let mut events = Vec::new();
    assert!(!matches!(
        settle(&mut game, &mut events),
        Status::Decision(_)
    ));
    let taken = game.position().cards(Zone::Hand(A));
    assert_eq!(taken.len(), 1);
    assert!(ties.contains(&taken[0]));
    let kept = if taken[0] == ties[0] {
        ties[1]
    } else {
        ties[0]
    };
    let mut hand = game.position().cards(Zone::Hand(B)).to_vec();
    hand.sort();
    let mut expected = vec![kept, drawn];
    expected.sort();
    assert_eq!(hand, expected);
}

#[test]
fn original_transfer_summary_survives_a_first_reaction_but_not_instead_replacements() {
    for replacement in [false, true] {
        let mut f = Fixture::default();
        let source = board(&mut f, "B021", A);
        board(&mut f, "B012", A); // Three Prosperity exceed Nobel's printed two.
        figure(
            &mut f,
            if replacement {
                Figure::Nightingale
            } else {
                Figure::AlfredNobel
            },
            B,
        );
        let payment = card(&mut f, "B004", Zone::Score(B));
        let reward = card(&mut f, "B008", Zone::Deck(Deck::base(1)));
        let junked = card(&mut f, "B075", Zone::Deck(Deck::base(7)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        action(&mut game, source);
        let mut events = Vec::new();
        done(&mut game, &mut events);
        if replacement {
            assert!(events.is_empty());
            assert_eq!(game.position().cards(Zone::Score(B)), [payment]);
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), [reward]);
        } else {
            let ms = moves(&events);
            assert_eq!(
                ms.iter()
                    .map(|m| (m.card, m.verb, m.inside_karma))
                    .collect::<Vec<_>>(),
                [
                    (junked, Some(Verb::Junk), true),
                    (payment, Some(Verb::Transfer), false),
                    (reward, None, false),
                    (reward, Some(Verb::Score), false),
                ]
            );
            assert_eq!(game.position().cards(Zone::Score(A)), [payment, reward]);
        }
    }
}

#[test]
fn compound_yi_replacement_cannot_earn_mapmaking_or_gunpowder_followup_even_after_source_removal() {
    for code in ["B021", "B037"] {
        let mut f = Fixture::default();
        let source = board(&mut f, code, A);
        figure(&mut f, Figure::YiSunSin, B);
        let original = if code == "B021" {
            card(&mut f, "B004", Zone::Score(B))
        } else {
            board(&mut f, "B005", B)
        };
        let reaction = card(&mut f, "B042", Zone::Deck(Deck::base(4)));
        let untouched = if code == "B021" {
            card(&mut f, "B008", Zone::Deck(Deck::base(1)))
        } else {
            card(&mut f, "B023", Zone::Deck(Deck::base(2)))
        };
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        action(&mut game, source);
        let mut events = Vec::new();
        done(&mut game, &mut events);
        let ms = moves(&events);
        assert_eq!(ms.len(), 4);
        assert_eq!(ms[0].card, original);
        assert_eq!(ms[1].card, reaction);
        assert_eq!(ms[2].card, reaction);
        assert!(ms.iter().all(|m| m.inside_karma));
        assert!(!ms.iter().any(|m| m.verb == Some(Verb::Transfer)));
        assert_eq!(
            game.position()
                .cards(Zone::Deck(Deck::base(if code == "B021" { 1 } else { 2 }))),
            [untouched]
        );
        if code == "B021" {
            assert_eq!(game.position().cards(Zone::Score(B)), [source]);
        }
    }
}

#[test]
fn gunpowders_actual_transfer_earns_a_score_but_reward_exhaustion_keeps_the_transfer() {
    for supply in [false, true] {
        let mut f = Fixture::default();
        let source = board(&mut f, "B037", A);
        let original = board(&mut f, "B005", B);
        let reward = supply.then(|| card(&mut f, "B023", Zone::Deck(Deck::base(2))));
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        action(&mut game, source);
        let mut events = Vec::new();
        let status = finish(&mut game, &mut events);
        assert_eq!(moved(&events[0]).card, original);
        assert_eq!(moved(&events[0]).executor, B);
        assert_eq!(game.position().top(B, Color::Yellow), None);
        if let Some(reward) = reward {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(game.position().cards(Zone::Score(A)), [original, reward]);
        } else {
            assert_eq!(status, Status::Winner(A));
            assert_eq!(game.position().cards(Zone::Score(A)), [original]);
            assert_eq!(events.len(), 1);
        }
    }
}

#[test]
fn compound_replacement_does_not_satisfy_any_conditional_transfer_reward() {
    for (code, payment_code, hand) in [
        ("B024", "B004", false),
        ("B035", "B001", true),
        ("B044", "B005", false),
        ("B050", "B052", false),
        ("B080", "B052", false),
    ] {
        let mut f = Fixture::default();
        let source = board(&mut f, code, A);
        figure(&mut f, Figure::YiSunSin, A);
        let payment = if hand {
            card(&mut f, payment_code, Zone::Hand(B))
        } else {
            board(&mut f, payment_code, B)
        };
        let reaction = card(&mut f, "B042", Zone::Deck(Deck::base(4)));
        // No printed reward supply: erroneously earning one would exhaust.
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut game, source, 0, B, A);
        let mut events = Vec::new();
        done(&mut game, &mut events);
        let ms = moves(&events);
        assert!(ms.iter().all(|m| m.inside_karma));
        assert_eq!(ms[0].card, payment);
        assert_eq!(ms[0].verb, Some(Verb::Tuck));
        assert_eq!(ms[1].card, reaction);
        assert!(!ms.iter().any(|m| m.verb == Some(Verb::Transfer)));
        assert!(
            Special::ALL
                .into_iter()
                .all(|s| game.position().special_location(s) == SpecialLocation::Available)
        );
    }
}

#[test]
fn empty_filtered_demands_do_not_draw_or_exhaust() {
    for code in [
        "B021", "B024", "B035", "B037", "B041", "B044", "B050", "B080",
    ] {
        let mut f = Fixture::default();
        let source = board(&mut f, code, A);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        invoke(&mut game, source, 0, B, A);
        let mut events = Vec::new();
        done(&mut game, &mut events);
        assert!(events.is_empty());
    }
}

#[test]
fn archery_available_membership_honors_mc_adam_aliases_without_exposing_supply_fronts() {
    let mut f = Fixture::default();
    let source = board(&mut f, "B001", A);
    figure(&mut f, Figure::JohnLoudonMcAdam, A);
    // Isolated membership/value-composition property: the audited native
    // catalog has no age-1/2 Industry card. No fake gameplay hook is introduced.
    let alias = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    f.cards[usize::from(alias.0)].symbols[0] = Symbol::Icon(Icon::Industry);
    let supply = card(&mut f, "B004", Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 1, A, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert_eq!(known(&menu), [alias]);
    game.choose(menu.options.iter().position(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == alias)).unwrap()).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().top(A, Color::Blue), None);
    assert_eq!(game.position().cards(Zone::Junk), [alias]);
    assert_eq!(game.position().cards(Zone::AvailableAchievements), [supply]);
    assert_eq!(moved(&events[0]).source, Zone::Board(A, Color::Blue));
}

#[test]
fn galileo_uses_live_available_achievement_aliases_from_their_actual_board_origins() {
    let mut f = Fixture::default();
    figure(&mut f, Figure::GalileoGalilei, A);
    figure(&mut f, Figure::JohnLoudonMcAdam, A);
    let alias = board(&mut f, "B037", A);
    let supply = card(&mut f, "B034", Zone::AvailableAchievements);
    let melding = card(&mut f, "B008", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(melding, Zone::Hand(A), A, Color::Green),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert_eq!(known(&menu), [alias]);
    game.choose(menu.options.iter().position(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == alias)).unwrap()).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(_))
    ));
    assert_eq!(game.position().top(A, Color::Red), None);
    assert_eq!(game.position().cards(Zone::Junk), [alias]);
    assert_eq!(game.position().cards(Zone::AvailableAchievements), [supply]);
    assert_eq!(game.position().top(A, Color::Green), Some(melding));
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(moved(&events[0]).source, Zone::Board(A, Color::Red));
}
