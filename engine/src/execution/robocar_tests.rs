//! Opposing back selection in Robocar and Queen Victoria.
use super::shortcuts::common::*;
use super::*;
use crate::program::build;

fn start(position: Position, figures: bool, robocar: CardId, seed: u64, own_turn: bool) -> Game {
    let mut game = Game::seeded(
        position,
        Expansions::ECHOES.union(Expansions::with_figures(figures)),
        seed,
    )
    .unwrap()
    .with_tracking()
    .unwrap();
    if !own_turn {
        game.begin_turn(B).unwrap();
    }
    game.start(
        Operation::ExecuteEffect {
            card: robocar,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    game
}

#[test]
fn robocar_back_menu_samples_identical_backs_with_native_rng() {
    for seed in 0..12 {
        let mut f = Fixture::default();
        let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
        let same = [
            f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]),
            f.tech(Color::Blue, 1, Zone::Hand(A), vec![]),
        ];
        let echo = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
        f.cards[usize::from(echo.0)].set = CardSet::Echoes;
        f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
        let mut game = start(f.position(), false, source, seed, false);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        assert_eq!(
            menu.options,
            [
                Alternative::BlindSelect {
                    source: Zone::Hand(A),
                    back: Deck::base(1)
                },
                Alternative::BlindSelect {
                    source: Zone::Hand(A),
                    back: Deck {
                        set: CardSet::Echoes,
                        age: 1
                    }
                },
                Alternative::BlindSelect {
                    source: Zone::Hand(A),
                    back: Deck::base(2)
                },
            ]
        );
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, Event::Revealed { .. }))
        );
        let mut expected_rng = game.rng.clone();
        let chosen = same[(expected_rng.next_u64() % 2) as usize];
        game.choose(0).unwrap();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(game.rng, expected_rng);
        assert_eq!(
            game.position.locate(chosen),
            Some(Zone::Board(A, game.position.card(chosen).color))
        );
        assert_eq!(
            game.position.cards(Zone::Hand(A)).len(),
            3,
            "off-turn Robocar never repeats"
        );
        assert!(game.position.check_conservation());
    }
}

#[test]
fn robocar_meld_stays_private_through_a_first_karma_choice() {
    let mut f = Fixture::default();
    let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
    let hand = [
        f.add(base_card("B001").unwrap(), Zone::Hand(A)),
        f.add(base_card("B002").unwrap(), Zone::Hand(A)),
    ];
    let figure = figure_card("F087").unwrap();
    f.add(figure.clone(), Zone::Board(B, figure.color));
    let victim = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(victim.0)].symbols[0] = Symbol::Icon(Icon::Industry);
    let mut game = start(f.position(), true, source, 4, false);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events); // one back: automatic sample, then Kalashnikov
    assert_eq!(menu.player, B);
    assert_eq!(menu.options.len(), 2);
    assert_eq!(game.pending_melds(B)[0].card, CardView::Back(Deck::base(1)));
    let CardView::Known(chosen) = game.pending_melds(A)[0].card else {
        panic!("owner knows its card")
    };
    assert!(hand.contains(&chosen));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Revealed { .. }))
    );
    let mut sampled = Vec::new();
    for seed in 0..24 {
        let world = game.determinize(B, seed).unwrap();
        let CardView::Known(card) = world.pending_melds(A)[0].card else {
            panic!("sample owner")
        };
        if !sampled.contains(&card) {
            sampled.push(card);
        }
    }
    assert_eq!(
        sampled.len(),
        2,
        "First does not pin the hidden sampled operand"
    );
    let mut clone = game.clone();
    for g in [&mut game, &mut clone] {
        g.choose(0).unwrap();
        assert!(matches!(settle(g, &mut Vec::new()), Status::Complete(_)));
        assert_eq!(
            g.position.locate(victim),
            Some(Zone::Score(B)),
            "Kalashnikov actually reacts"
        );
        assert!(g.position.check_conservation());
    }
    assert_eq!(game, clone);
}

#[test]
fn robocar_self_executes_and_repeats_only_after_a_performed_meld_on_its_turn() {
    for own_turn in [false, true] {
        let mut f = Fixture::default();
        let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
        let picked = f.tech(Color::Blue, 1, Zone::Hand(A), vec![build::draw(2)]);
        let drawn = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = start(f.position(), false, source, 4, own_turn);
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        assert_eq!(
            game.position.locate(picked),
            Some(Zone::Board(A, Color::Blue))
        );
        assert_eq!(
            game.position.locate(drawn),
            Some(if own_turn {
                Zone::Board(A, Color::Yellow)
            } else {
                Zone::Deck(Deck::base(2))
            })
        );
        assert!(game.position.check_conservation());
    }
    let mut f = Fixture::default();
    let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
    let figure = figure_card(Figure::SejongTheGreat.code()).unwrap();
    f.add(figure.clone(), Zone::Board(A, figure.color));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let picked = f.tech(Color::Blue, 1, Zone::Hand(A), vec![build::draw(3)]);
    f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let undrawn = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = start(f.position(), true, source, 4, true);
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert_eq!(
        game.position.locate(picked),
        Some(Zone::Deck(Deck::base(1)))
    );
    assert_eq!(
        game.position.locate(undrawn),
        Some(Zone::Deck(Deck::base(3))),
        "a replaced meld does not execute the chosen card"
    );
}

#[test]
fn robocar_ark_loop_draws_at_persistent_work_cap_with_clone_resume() {
    let mut f = Fixture::default();
    let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
    // Ark's first clause has an empty hand. Its second clause transfers
    // itself from a board back to hand; Robocar then selects it again.
    let ark = f.tech(
        Color::Purple,
        1,
        Zone::Hand(A),
        vec![
            build::return_(build::HAND),
            build::transfer(
                build::filtered(
                    &Selector::Tops(Who::Either),
                    &[Filter::Has(Test::Is(Reg::Source))],
                ),
                Destination::Hand(Who::Executor),
            ),
        ],
    );
    let mut game = start(f.position(), false, source, 1, true);
    game.automatic_work = AUTOMATIC_WORK_LIMIT - 200;
    let mut clone = game.clone();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(201, &mut events).unwrap(),
        Status::Draw(DrawReason::Loop)
    );
    let mut cloned_events = Vec::new();
    for _ in 0..201 {
        if clone.advance(1, &mut cloned_events).unwrap() == Status::Draw(DrawReason::Loop) {
            break;
        }
    }
    assert_eq!(events, cloned_events);
    assert_eq!(game, clone);
    assert!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Moved(m) if m.card == ark))
            .count()
            > 4
    );
    assert!(game.position.check_conservation());
    assert!(game.frames.is_empty());
}

#[test]
fn robocar_catherine_sanitation_loop_draws_once_both_decks_are_empty() {
    // Random Echoes + Figures + Cities seed 8261: Robocar melds the only hand
    // card while Catherine the Great returns the previous yellow top,
    // alternating Greta Thunberg and Sanitation. Sanitation's 7-or-8 choice
    // was the loop's only decision; with both decks empty none remains.
    for nonempty_seven in [false, true] {
        let mut f = Fixture::default();
        let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
        f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
        let sanitation = f.add(base_card("B069").unwrap(), Zone::Board(A, Color::Yellow));
        let greta = f.figure(Figure::GretaThunberg, Zone::Hand(A));
        let seven =
            nonempty_seven.then(|| f.add(base_card("B068").unwrap(), Zone::Deck(Deck::base(7))));
        let mut game = start(f.position(), true, source, 0, true);
        if nonempty_seven {
            // A nonempty deck is a real choice; junking it ends the choices.
            let menu = decision(&mut game, &mut Vec::new());
            assert_eq!(menu.options, [Alternative::Value(7), Alternative::Value(8)]);
            game.choose(0).unwrap();
        }
        let mut events = Vec::new();
        assert_eq!(game.advance(3000, &mut events).unwrap(), Status::WorkLimit);
        if let Some(seven) = seven {
            assert_eq!(game.position.locate(seven), Some(Zone::Junk));
        }
        let melds = |card| {
            events
                .iter()
                .filter(|e| matches!(e, Event::Moved(m) if m.card == card && m.verb == Some(Verb::Meld)))
                .count()
        };
        assert!(melds(sanitation) > 2 && melds(greta) > 2);
        // Without a decision the automatic-work cap ends the loop.
        game.automatic_work = AUTOMATIC_WORK_LIMIT - 200;
        assert_eq!(
            game.advance(201, &mut Vec::new()).unwrap(),
            Status::Draw(DrawReason::Loop)
        );
        assert!(game.position.check_conservation());
    }
}

#[test]
fn a_real_robocar_choice_resets_automatic_work() {
    let mut f = Fixture::default();
    let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
    f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let mut game = start(f.position(), false, source, 0, true);
    let _ = decision(&mut game, &mut Vec::new());
    game.automatic_work = AUTOMATIC_WORK_LIMIT;
    assert!(matches!(
        game.advance(1, &mut Vec::new()).unwrap(),
        Status::Decision(_)
    ));
    assert_eq!(game.automatic_work, 0);
    game.choose(0).unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
}

#[test]
fn robocar_singleton_sampling_keeps_the_stream_for_the_following_blind_pick() {
    for seed in 0..8 {
        let mut f = Fixture::default();
        let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
        let first = f.tech(
            Color::Blue,
            1,
            Zone::Hand(A),
            vec![build::draws(Expr::Const(2), 2, Then::Keep)],
        );
        let bottom = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
        let top = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = start(f.position(), false, source, seed, true);
        let mut rng = game.rng.clone();
        rng.next_u64(); // initial singleton hand pick
        let chosen = [top, bottom][(rng.next_u64() % 2) as usize];
        rng.next_u64(); // final singleton hand pick
        let mut events = Vec::new();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        let melded: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Moved(m) if m.verb == Some(Verb::Meld) => Some(m.card),
                _ => None,
            })
            .collect();
        assert_eq!(melded[0], first);
        assert_eq!(melded[1], chosen);
        assert_eq!(game.rng, rng);
    }
}

#[test]
fn queen_victoria_groups_opposing_score_backs_and_keeps_the_pick_private() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let queen = f.figure(Figure::QueenVictoria, Zone::Board(A, Color::Purple));
        let own = f.add(figure_card("F003").unwrap(), Zone::Score(A));
        // Put the larger back first so physical order cannot sort the menu.
        f.add(figure_card("F016").unwrap(), Zone::Score(B));
        for code in ["F001", "F002"] {
            f.add(figure_card(code).unwrap(), Zone::Score(B));
        }
        f.add(base_card("B002").unwrap(), Zone::Score(B));
        // Unknown deck cards keep singleton score backs from naming a face.
        for (code, age) in [("F004", 1), ("F017", 2)] {
            f.add(figure_card(code).unwrap(), Zone::Deck(Deck::figures(age)));
        }
        let scored = f.add(base_card("B001").unwrap(), Zone::Hand(A));
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 73).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        game.start(
            Operation::Move {
                verb: Verb::Score,
                target: target(scored, Zone::Hand(A)),
                destination: Zone::Score(A),
            },
            Context::new(A),
        )
        .unwrap();
        let expected = vec![
            Alternative::BlindSelect {
                source: Zone::Score(B),
                back: Deck::figures(1),
            },
            Alternative::BlindSelect {
                source: Zone::Score(B),
                back: Deck::figures(2),
            },
            Alternative::Select {
                target: TargetDescriptor::Known {
                    card: own,
                    source: Zone::Score(A),
                },
            },
        ];
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.player, A);
        assert_eq!(menu.options, expected);
        assert_eq!(menu.source_card(A), Some(queen));
        let mut worlds = vec![game.clone()];
        if tracked {
            let observation = game.observe(A).unwrap();
            for seed in 0..8 {
                let sampled = game.determinize(A, seed).unwrap();
                assert_eq!(sampled.observe(A).unwrap(), observation);
                worlds.push(sampled);
                worlds.push(game.sample_world(A, seed).unwrap().world);
            }
        }
        let mut hidden_picks = std::collections::BTreeSet::new();
        for world in worlds {
            for choice in 0..3 {
                let mut picked = world.clone();
                assert_eq!(decision(&mut picked, &mut Vec::new()).options, expected);
                let mut rng = picked.rng.clone();
                let chosen = if choice < 2 {
                    let candidates: Vec<_> = picked
                        .position
                        .cards(Zone::Score(B))
                        .iter()
                        .copied()
                        .filter(|&card| {
                            picked.position.card(card).origin_deck()
                                == Deck::figures(choice as u8 + 1)
                        })
                        .collect();
                    candidates[(rng.next_u64() % candidates.len() as u64) as usize]
                } else {
                    own
                };
                picked.choose(choice).unwrap();
                let mut events = Vec::new();
                let branch = decision(&mut picked, &mut events);
                assert_eq!(branch.player, A);
                assert_eq!(
                    branch.options,
                    [Alternative::Branch(0), Alternative::Branch(1)]
                );
                assert_eq!(branch.source_card(A), Some(queen));
                assert!(branch.option_label(0, A).is_some());
                assert!(branch.option_label(1, A).is_some());
                assert_eq!(picked.rng, rng);
                assert!(
                    events.is_empty(),
                    "selection neither moves nor reveals a card"
                );
                let reference = picked
                    .frames
                    .iter()
                    .find_map(|frame| match frame {
                        Frame::Program {
                            source: Source::Karma { card, .. },
                            chosen: SelectionParameter::Card(reference),
                            ..
                        } if *card == queen => Some(*reference),
                        _ => None,
                    })
                    .expect("Queen Victoria retains her selected Figure");
                assert_eq!(
                    reference.inner, chosen,
                    "the native sample is the retained pick"
                );
                assert_eq!(picked.reference_visible(reference, A), choice == 2);
                assert!(picked.reference_visible(reference, if choice < 2 { B } else { A }));
                if choice == 0 {
                    hidden_picks.insert(chosen);
                }
                for branch in 0..2 {
                    let mut finished = picked.clone();
                    finished.choose(branch).unwrap();
                    let mut events = Vec::new();
                    assert!(matches!(
                        settle(&mut finished, &mut events),
                        Status::Complete(_)
                    ));
                    let destination = if branch == 0 {
                        Zone::Score(A)
                    } else {
                        Zone::AvailableAchievements
                    };
                    assert_eq!(finished.position.locate(chosen), Some(destination));
                    assert_eq!(finished.position.locate(scored), Some(Zone::Score(A)));
                    assert!(events.iter().any(|event| matches!(event, Event::Moved(m)
                        if m.card == chosen && m.destination == destination && m.inside_karma)));
                    assert!(
                        events.iter().any(|event| matches!(event, Event::Moved(m)
                        if m.card == scored && m.destination == Zone::Score(A) && !m.inside_karma))
                    );
                    assert_eq!(finished.rng, rng);
                    assert!(finished.position.check_conservation());
                    if tracked && finished.knowledge.is_some() {
                        for player in Player::ALL {
                            assert!(
                                finished
                                    .tracker(player)
                                    .unwrap()
                                    .check_truth(&finished.position)
                            );
                        }
                    }
                }
            }
        }
        if tracked {
            assert!(
                hidden_picks.len() > 1,
                "sampled worlds must vary the private selection"
            );
        }
    }
}

#[test]
fn queen_victoria_forced_singleton_advances_the_next_blind_pick_stream() {
    for tracked in [false, true] {
        for seed in 0..8 {
            let mut f = Fixture::default();
            f.figure(Figure::QueenVictoria, Zone::Board(A, Color::Purple));
            let singleton = f.add(figure_card("F001").unwrap(), Zone::Score(B));
            let later = ["F002", "F003"]
                .map(|code| f.add(figure_card(code).unwrap(), Zone::Deck(Deck::figures(1))));
            let originals =
                ["B001", "B002"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let mut game = Game::seeded(f.position(), Expansions::FIGURES, seed).unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            let mut rng = game.rng.clone();
            rng.next_u64(); // Queen Victoria's first, forced singleton pick.
            let second = later[(rng.next_u64() % 2) as usize];
            for (index, original) in originals.into_iter().enumerate() {
                game.start(
                    Operation::Move {
                        verb: Verb::Score,
                        target: target(original, Zone::Hand(A)),
                        destination: Zone::Score(A),
                    },
                    Context::new(A),
                )
                .unwrap();
                let mut events = Vec::new();
                let menu = decision(&mut game, &mut events);
                assert_eq!(
                    menu.options,
                    [Alternative::Branch(0), Alternative::Branch(1)]
                );
                assert!(events.is_empty(), "the forced blind pick remains private");
                game.choose(1).unwrap(); // Supply the Figure, keeping our score Figure-free.
                assert!(matches!(
                    settle(&mut game, &mut events),
                    Status::Complete(_)
                ));
                assert_eq!(
                    game.position
                        .locate(if index == 0 { singleton } else { second }),
                    Some(Zone::AvailableAchievements)
                );
                assert_eq!(game.position.locate(original), Some(Zone::Score(A)));
                if index == 0 {
                    for card in later {
                        game.start(
                            Operation::Move {
                                verb: Verb::Transfer,
                                target: target(card, Zone::Deck(Deck::figures(1))),
                                destination: Zone::Score(B),
                            },
                            Context::new(B),
                        )
                        .unwrap();
                        assert!(matches!(
                            settle(&mut game, &mut Vec::new()),
                            Status::Complete(_)
                        ));
                    }
                }
            }
            assert_eq!(
                game.rng, rng,
                "each blind hand/score choice consumes one native draw"
            );
            assert!(game.position.check_conservation());
            if tracked {
                for player in Player::ALL {
                    assert!(game.tracker(player).unwrap().check_truth(&game.position));
                }
            }
        }
    }
}
