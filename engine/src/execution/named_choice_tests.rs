//! Revealed packet choices retain identities without exposing a whole hand.
use super::shortcuts::common::{A, B, Fixture, decision, settle};
use super::*;
use crate::program::build::*;

const PACKET: Slot = Slot::effect(0);
const MIXED: Selector = Selector::Union(&[
    Selector::Slot(PACKET),
    filtered(&HAND, &[Filter::Lacks(Test::InSlot(PACKET))]),
]);
const LATE_REVEAL: Selector = Selector::Union(&[
    filtered(&BOUND, &[Filter::Has(Test::Color(Color::Blue))]),
    Selector::Slot(PACKET),
    filtered(
        &HAND,
        &[
            Filter::Lacks(Test::Is(Reg::Bound)),
            Filter::Lacks(Test::InSlot(PACKET)),
        ],
    ),
]);

#[derive(Clone, Copy)]
enum Case {
    Collaboration,
    Packet,
    PacketBeforeReveal,
    LateReveal,
}

fn fixture(case: Case, tracked: bool) -> (Game, [CardId; 2], [CardId; 2], Option<CardId>) {
    let mut f = Fixture::default();
    let mut source = base_card("B090").unwrap();
    let mixed = !matches!(case, Case::Collaboration);
    if matches!(case, Case::Packet | Case::PacketBeforeReveal) {
        source.effects[0].instructions = vec![
            draw(9),
            reveal(LAST).keep(PACKET),
            draw(9),
            reveal(LAST).add(PACKET),
            choose(Who::Demander, Choice::Card(&MIXED)),
            meld(Selector::Reg(Reg::Chosen)),
        ];
        if matches!(case, Case::PacketBeforeReveal) {
            source.effects[0].instructions.insert(
                1,
                choose(Who::Demander, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
            );
        }
    }
    if matches!(case, Case::LateReveal) {
        source.effects[0].instructions = vec![
            draw(9),
            bind(LAST),
            reveal(LAST),
            draw(9),
            reveal(LAST).keep(PACKET),
            choose(Who::Demander, Choice::Card(&LATE_REVEAL)),
            meld(Selector::Reg(Reg::Chosen)),
        ];
    }
    let source = f.add(source, Zone::Board(A, Color::Green));
    let hidden = ["B088", "B094"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(B)));
    for code in ["B087", "B093"] {
        f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(9)));
    }
    let revealed =
        ["B086", "B092"].map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(9))));
    let victim = if mixed {
        f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
        let mut card = base_card("B001").unwrap();
        card.symbols[0] = Symbol::Icon(Icon::Industry);
        Some(f.add(card, Zone::Board(B, Color::Red)))
    } else {
        None
    };
    let mut game = Game::seeded(f.position(), Expansions::with_figures(mixed), 73).unwrap();
    if tracked {
        game = game.with_tracking().unwrap();
    }
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context {
            demander: A,
            ..Context::new(B)
        },
    )
    .unwrap();
    (game, revealed, hidden, victim)
}

fn named(card: CardId) -> Alternative {
    Alternative::Select {
        target: TargetDescriptor::Known {
            card,
            source: Zone::Hand(B),
        },
    }
}

fn menu(game: &mut Game) -> Decision {
    decision(game, &mut Vec::new())
}

fn exact_menu(game: &mut Game, expected: &[Alternative]) -> Decision {
    let menu = menu(game);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options, expected);
    assert!(
        !game.position.can_view(Zone::Hand(B), A),
        "no whole-hand inspection grant"
    );
    menu
}

#[test]
fn collaboration_revealed_pair_stays_named_through_clones_and_samples() {
    for tracked in [false, true] {
        let (mut game, revealed, hidden, _) = fixture(Case::Collaboration, tracked);
        let mut events = Vec::new();
        let initial = decision(&mut game, &mut events);
        let expected = vec![named(revealed[1]), named(revealed[0])];
        assert_eq!(initial.options, expected);
        assert_eq!(initial.player, A);
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed { card, .. } => Some(*card),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [revealed[1], revealed[0]]
        );
        let mut worlds = vec![game.clone(), game.clone()];
        let mut permuted = false;
        if tracked {
            let observation = game.observe(A).unwrap();
            for &card in &hidden {
                assert!(
                    !game
                        .tracker(A)
                        .unwrap()
                        .knows(&game.position, card, Zone::Hand(B))
                );
            }
            for seed in 0..24 {
                let mut world = game.determinize(A, seed).unwrap();
                permuted |=
                    world.position.cards(Zone::Hand(B)) != game.position.cards(Zone::Hand(B));
                exact_menu(&mut world, &expected);
                assert_eq!(world.observe(A).unwrap(), observation, "seed {seed}");
                worlds.push(world);
            }
            assert!(permuted, "sample must reorder the same-back physical hand");
        }
        for (index, mut world) in worlds.into_iter().enumerate() {
            let choice = index % 2;
            let before_rng = world.rng.clone();
            exact_menu(&mut world, &expected);
            world.choose(choice).unwrap();
            assert!(matches!(
                settle(&mut world, &mut Vec::new()),
                Status::Complete(_)
            ));
            let chosen = revealed[1 - choice];
            let other = revealed[choice];
            assert_eq!(
                world.position.locate(chosen),
                Some(Zone::Board(A, world.position.card(chosen).color))
            );
            assert_eq!(
                world.position.locate(other),
                Some(Zone::Board(B, world.position.card(other).color))
            );
            assert_eq!(world.position.cards(Zone::Hand(B)).len(), 2);
            assert_eq!(
                world.rng, before_rng,
                "naming a revealed card never samples a hidden back"
            );
            assert!(world.position.check_conservation());
            if tracked {
                for p in Player::ALL {
                    assert!(world.tracker(p).unwrap().check_truth(&world.position));
                }
            }
        }
    }
}

#[test]
fn mixed_named_references_and_hidden_hand_choices_keep_distinct_evidence_through_first() {
    for tracked in [false, true] {
        let (mut game, revealed, hidden, victim) = fixture(Case::Packet, tracked);
        let victim = victim.unwrap();
        let expected = vec![
            Alternative::BlindSelect {
                source: Zone::Hand(B),
                back: Deck::base(9),
            },
            named(revealed[1]),
            named(revealed[0]),
        ];
        exact_menu(&mut game, &expected);
        let mut worlds = vec![game.clone(), game.clone()];
        if tracked {
            let observation = game.observe(A).unwrap();
            for seed in 0..16 {
                let mut world = game.determinize(A, seed).unwrap();
                exact_menu(&mut world, &expected);
                assert_eq!(world.observe(A).unwrap(), observation);
                worlds.push(world);
            }
        }
        for original in worlds {
            for choice in 0..3 {
                let mut world = original.clone();
                let before_rng = world.rng.clone();
                world.choose(choice).unwrap();
                let first = menu(&mut world);
                assert_eq!(first.player, A);
                let pending = world.pending_melds(A);
                assert_eq!(pending.len(), 1);
                if choice == 0 {
                    assert_eq!(pending[0].card, CardView::Back(Deck::base(9)));
                    let mut sampled_rng = before_rng;
                    sampled_rng.next_u64();
                    assert_eq!(
                        world.rng, sampled_rng,
                        "blind choice uses one native sample"
                    );
                    let CardView::Known(card) = world.pending_melds(B)[0].card else {
                        panic!("owner knows selection")
                    };
                    assert!(
                        !revealed.contains(&card),
                        "blind remainder excludes the named packet"
                    );
                    if !tracked {
                        assert!(hidden.contains(&card));
                    }
                } else {
                    assert_eq!(pending[0].card, CardView::Known(revealed[2 - choice]));
                    assert_eq!(world.rng, before_rng);
                }
                let take = first.options.iter().position(|option| matches!(option,
                    Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == victim
                )).unwrap();
                let mut clone = world.clone();
                let mut events = Vec::new();
                world.choose(take).unwrap();
                clone.choose(take).unwrap();
                assert!(matches!(
                    settle(&mut world, &mut events),
                    Status::Complete(_)
                ));
                assert!(matches!(
                    settle(&mut clone, &mut Vec::new()),
                    Status::Complete(_)
                ));
                assert_eq!(world, clone);
                assert_eq!(
                    world.position.locate(victim),
                    Some(Zone::Score(A)),
                    "Kalashnikov actually scores"
                );
                assert!(events.iter().any(|event| matches!(event,
                    Event::Moved(m) if m.card == victim && m.inside_karma
                )));
                assert!(world.position.check_conservation());
            }
        }
    }
}

#[test]
fn later_reveal_keeps_a_named_reference_in_untracked_search_samples() {
    let (mut game, revealed, _, _) = fixture(Case::LateReveal, true);
    let expected = vec![
        Alternative::BlindSelect {
            source: Zone::Hand(B),
            back: Deck::base(9),
        },
        named(revealed[1]),
        named(revealed[0]),
    ];
    exact_menu(&mut game, &expected);
    let retained = game
        .frames
        .iter()
        .find_map(|frame| match frame {
            Frame::Program { retained, .. } => *retained,
            _ => None,
        })
        .unwrap();
    assert!(!retained.evidence.known(A), "binding predates the reveal");
    assert_eq!(
        game.tracker(A)
            .unwrap()
            .pick_identity(retained.evidence.pick().unwrap()),
        Some(revealed[1])
    );
    for seed in 0..24 {
        for mut world in [
            game.sample_world(A, seed).unwrap().world,
            game.determinize(A, seed).unwrap(),
        ] {
            exact_menu(&mut world, &expected);
            world.choose(1).unwrap();
            let first = menu(&mut world);
            assert_eq!(first.player, A);
            assert_eq!(world.pending_melds(A)[0].card, CardView::Known(revealed[1]));
            assert!(!world.position.can_view(Zone::Hand(B), A));
            assert!(world.position.check_conservation());
        }
    }
}

#[test]
fn revealed_packets_keep_future_choices_public_after_sampling_before_reveal() {
    for tracked in [false, true] {
        let (mut game, _, _, victim) = fixture(Case::PacketBeforeReveal, tracked);
        let victim = victim.unwrap();
        let mut events = Vec::new();
        let before = decision(&mut game, &mut events);
        assert_eq!(before.player, A);
        assert_eq!(
            before.options,
            [Alternative::Value(1), Alternative::Value(2)]
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Revealed { .. }))
        );
        assert!(
            game.context_refs(A).iter().any(|reference| {
                reference.role == observe::Role::Register
                    && reference.detail == 1
                    && reference.sight == observe::Sight::Back(Deck::base(9))
            }),
            "the first drawn card is still private at the sampling boundary"
        );
        let mut worlds = vec![game.clone()];
        if tracked {
            for seed in 0..16 {
                let search = game.sample_world(A, seed).unwrap().world;
                assert!(search.knowledge.is_none());
                worlds.push(search);
                worlds.push(game.determinize(A, seed).unwrap());
            }
        }
        for mut world in worlds {
            let before_rng = world.rng.clone();
            world.choose(0).unwrap();
            let mut events = Vec::new();
            let offered = decision(&mut world, &mut events);
            let packet = events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed { card, .. } => Some(*card),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(packet.len(), 2);
            assert_ne!(packet[0], packet[1]);
            assert_eq!(offered.player, A);
            assert_eq!(
                offered.options,
                [
                    Alternative::BlindSelect {
                        source: Zone::Hand(B),
                        back: Deck::base(9)
                    },
                    named(packet[0]),
                    named(packet[1]),
                ]
            );
            assert_eq!(
                world.rng, before_rng,
                "revealed choices consume no blind sample"
            );
            assert!(!world.position.can_view(Zone::Hand(B), A));
            for choice in 0..3 {
                let mut branch = world.clone();
                let before_rng = branch.rng.clone();
                branch.choose(choice).unwrap();
                let first = menu(&mut branch);
                assert_eq!(first.player, A);
                let CardView::Known(selected) = branch.pending_melds(B)[0].card else {
                    panic!("the hand owner knows the selected card");
                };
                if choice == 0 {
                    assert_eq!(
                        branch.pending_melds(A)[0].card,
                        CardView::Back(Deck::base(9))
                    );
                    assert!(!packet.contains(&selected));
                    let mut expected_rng = before_rng;
                    expected_rng.next_u64();
                    assert_eq!(branch.rng, expected_rng);
                } else {
                    assert_eq!(selected, packet[choice - 1]);
                    assert_eq!(branch.pending_melds(A)[0].card, CardView::Known(selected));
                    assert_eq!(branch.rng, before_rng);
                }
                let take = first
                    .options
                    .iter()
                    .position(|option| {
                        matches!(option,
                            Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. }
                                if *card == victim
                        )
                    })
                    .unwrap();
                branch.choose(take).unwrap();
                let mut events = Vec::new();
                assert!(matches!(
                    settle(&mut branch, &mut events),
                    Status::Complete(_)
                ));
                assert_eq!(branch.position.locate(victim), Some(Zone::Score(A)));
                assert_eq!(
                    branch.position.locate(selected),
                    Some(Zone::Board(B, branch.position.card(selected).color))
                );
                assert!(events.iter().any(|event| matches!(event,
                    Event::Moved(m) if m.card == victim && m.inside_karma
                )));
                assert!(branch.position.check_conservation());
                if branch.knowledge.is_some() {
                    for player in Player::ALL {
                        assert!(
                            branch
                                .tracker(player)
                                .unwrap()
                                .check_truth(&branch.position)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn anonymous_card_choice_keeps_public_packet_excluded_through_first_samples() {
    let (mut game, revealed, _, victim) = fixture(Case::Packet, true);
    let victim = victim.unwrap();
    let expected = [
        Alternative::BlindSelect {
            source: Zone::Hand(B),
            back: Deck::base(9),
        },
        named(revealed[1]),
        named(revealed[0]),
    ];
    exact_menu(&mut game, &expected);
    game.choose(0).unwrap();
    let first = menu(&mut game);
    assert_eq!(first.player, A);
    let take = first.options.iter().position(|option| matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == victim
    )).unwrap();
    for seed in 0..48 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            assert_eq!(menu(&mut world), first);
            let selected = world
                .frames
                .iter()
                .find_map(|frame| match frame {
                    Frame::Operation {
                        request:
                            Operation::Move {
                                verb: Verb::Meld,
                                target,
                                ..
                            },
                        ..
                    } => Some(target.card),
                    _ => None,
                })
                .unwrap();
            assert!(
                !revealed.contains(&selected),
                "anonymous choice remains outside the publicly named packet"
            );
            assert!(matches!(world.pending_melds(A)[0].card, CardView::Back(_)));
            world.choose(take).unwrap();
            let mut events = Vec::new();
            assert!(matches!(
                settle(&mut world, &mut events),
                Status::Complete(_)
            ));
            assert_eq!(
                world.position.locate(selected),
                Some(Zone::Board(B, world.position.card(selected).color))
            );
            assert_eq!(world.position.locate(victim), Some(Zone::Score(A)));
            assert!(events.iter().any(
                |event| matches!(event, Event::Moved(m) if m.card == victim && m.inside_karma)
            ));
            assert!(world.position.check_conservation());
        }
    }
}
