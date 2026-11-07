//! Single movements share native back choices and retained anonymous evidence.
use super::*;
use crate::program::build::*;
use shortcuts::common::*;

fn menu(game: &mut Game, events: &mut Vec<Event>) -> Decision {
    decision(game, events)
}
fn complete(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..400 {
        let mut cloned = game.clone();
        let mut next = Vec::new();
        let status = game.advance(1, &mut next).unwrap();
        let mut comparison = Vec::new();
        assert_eq!(cloned.advance(1, &mut comparison).unwrap(), status);
        assert_eq!(cloned, *game);
        assert_eq!(next, comparison);
        events.extend(next);
        match status {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::WorkLimit => {}
            _ => {
                assert!(game.position.check_conservation());
                for player in Player::ALL {
                    if let Some(tracker) = game.tracker(player) {
                        assert!(tracker.check_truth(&game.position));
                    }
                }
                return status;
            }
        }
    }
    panic!("single movement did not complete")
}

#[test]
fn medicine_junks_one_native_back_sample_and_preserves_rod_instead() {
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
    for tracked in [false, true] {
        for rod in [false, true] {
            let mut f = Fixture::default();
            let source = f.add(base_card("B029").unwrap(), Zone::Board(A, Color::Yellow));
            let base = ["B026", "B027"]
                .map(|code| f.add(base_card(code).unwrap(), Zone::AvailableAchievements));
            let echo = f.add(echoes_card("E031").unwrap(), Zone::AvailableAchievements);
            let four = f.add(base_card("B036").unwrap(), Zone::AvailableAchievements);
            if rod {
                f.figure(Figure::RodSerling, Zone::Board(B, Color::Purple));
            }
            let mut game = Game::seeded(
                f.position(),
                Expansions::ECHOES.union(Expansions::FIGURES),
                19,
            )
            .unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 1,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            let decision = menu(&mut game, &mut events);
            assert_eq!(
                decision.options,
                [
                    Alternative::BlindMove {
                        verb: Verb::Junk,
                        source: Zone::AvailableAchievements,
                        back: Deck::base(3)
                    },
                    Alternative::BlindMove {
                        verb: Verb::Junk,
                        source: Zone::AvailableAchievements,
                        back: Deck {
                            set: CardSet::Echoes,
                            age: 3
                        }
                    },
                    Alternative::BlindMove {
                        verb: Verb::Junk,
                        source: Zone::AvailableAchievements,
                        back: Deck::base(4)
                    },
                ]
            );
            assert!(events.is_empty());
            let mut roots = vec![game.clone()];
            if tracked {
                for seed in 0..8 {
                    roots.push(game.determinize(A, seed).unwrap());
                    roots.push(game.sample_world(A, seed).unwrap().world);
                }
            }
            for root in roots {
                for choice in 0..3 {
                    let mut branch = root.clone();
                    let mut expected_rng = branch.rng.clone();
                    let expected = if choice == 0 {
                        let candidates: Vec<_> = branch
                            .position
                            .cards(Zone::AvailableAchievements)
                            .iter()
                            .copied()
                            .filter(|card| base.contains(card))
                            .collect();
                        candidates[(expected_rng.next_u64() % 2) as usize]
                    } else if choice == 1 {
                        echo
                    } else {
                        four
                    };
                    branch.choose(choice).unwrap();
                    let mut events = Vec::new();
                    let status = complete(&mut branch, &mut events);
                    assert_eq!(
                        branch.rng, expected_rng,
                        "singleton backs do not consume chance"
                    );
                    assert!(
                        !events
                            .iter()
                            .any(|event| matches!(event, Event::Revealed { .. }))
                    );
                    if rod && choice < 2 {
                        assert!(matches!(status, Status::Winner(B)));
                        assert_eq!(
                            branch.position.locate(expected),
                            Some(Zone::AvailableAchievements)
                        );
                        assert!(
                            !events.iter().any(
                                |event| matches!(event, Event::Moved(m) if m.card == expected)
                            )
                        );
                    } else {
                        assert!(matches!(status, Status::Complete(_)));
                        assert_eq!(branch.position.cards(Zone::Junk), [expected]);
                        assert_eq!(
                            events,
                            [Event::Moved(Movement {
                                verb: Some(Verb::Junk),
                                card: expected,
                                source: Zone::AvailableAchievements,
                                destination: Zone::Junk,
                                executor: A,
                                inside_karma: false
                            })]
                        );
                    }
                }
            }
        }
    }
}

const PACKET: Slot = Slot::effect(0);
const MIXED: Selector = Selector::Union(&[Selector::Slot(PACKET), Selector::Junk]);
fn mixed_fixture(public: bool, reactor: Player) -> (Game, [CardId; 2]) {
    let mut f = Fixture::default();
    let mut body = vec![draw(9).keep(PACKET)];
    if public {
        body.push(reveal(LAST));
    }
    body.extend([junk(LAST), draw(9).add(PACKET)]);
    if public {
        body.push(reveal(LAST));
    }
    body.extend([
        junk(LAST),
        transfer(MIXED, Destination::Hand(Who::Executor)),
        choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
    ]);
    let source = f.tech(Color::Blue, 7, Zone::Board(A, Color::Blue), body);
    for color in [Color::Red, Color::Green] {
        f.tech(color, 9, Zone::Junk, vec![]);
    }
    let packet = [Color::Yellow, Color::Purple]
        .map(|color| f.tech(color, 9, Zone::Deck(Deck::base(9)), vec![]));
    f.figure(Figure::AlfredNobel, Zone::Board(reactor, Color::Green));
    for color in [Color::Blue, Color::Purple] {
        f.tech(color, 7, Zone::Deck(Deck::base(7)), vec![]);
    }
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 93)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.shortcuts_disabled = true;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    (game, packet)
}
fn pending(game: &Game) -> HeldCard {
    game.frames
        .iter()
        .find_map(|frame| match frame {
            Frame::Operation {
                request:
                    Operation::Move {
                        verb: Verb::Transfer,
                        target,
                        ..
                    },
                knowledge: Some(knowledge),
                ..
            } => Some(Captured::new(target.card, knowledge.evidence)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn single_transfer_retains_mixed_named_and_private_cohorts_through_nobel_first() {
    for public in [false, true] {
        for reactor in Player::ALL {
            let (mut game, packet) = mixed_fixture(public, reactor);
            let original = menu(&mut game, &mut vec![]);
            assert_eq!(original.options.len(), 3);
            assert_eq!(
                original.options[0],
                Alternative::BlindMove {
                    verb: Verb::Transfer,
                    source: Zone::Junk,
                    back: Deck::base(9)
                }
            );
            assert!(original.options[1..].iter().all(|option| matches!(option, Alternative::Move { verb: Verb::Transfer, target: TargetDescriptor::Known { card, source: Zone::Junk } } if packet.contains(card))));
            let mut physical_permuted = false;
            for seed in 0..12 {
                for mut sample in [
                    game.determinize(A, seed).unwrap(),
                    game.sample_world(A, seed).unwrap().world,
                ] {
                    assert_eq!(menu(&mut sample, &mut vec![]), original);
                    let Some(Frame::Choose { tasks, .. }) = sample.frames.last() else {
                        panic!("menu");
                    };
                    let Task::BlindOperations(group) = &tasks[0] else {
                        panic!("single blind group");
                    };
                    let candidates: Vec<_> = group
                        .operations
                        .iter()
                        .map(|(operation, reference)| {
                            assert!(reference.is_none());
                            let Operation::Move {
                                verb: Verb::Transfer,
                                target,
                                ..
                            } = operation
                            else {
                                panic!("transfer");
                            };
                            assert!(!packet.contains(&target.card));
                            target.card
                        })
                        .collect();
                    assert_eq!(candidates.len(), 2);
                    physical_permuted |=
                        sample.position.cards(Zone::Junk) != game.position.cards(Zone::Junk);
                    for named_choice in 1..3 {
                        let mut named = sample.clone();
                        let Alternative::Move {
                            target: TargetDescriptor::Known { card, .. },
                            ..
                        } = original.options[named_choice]
                        else {
                            panic!("retained name");
                        };
                        let rng = named.rng.clone();
                        named.choose(named_choice).unwrap();
                        assert_eq!(menu(&mut named, &mut vec![]).player, reactor);
                        let selected = pending(&named);
                        assert_eq!(selected.inner, card);
                        assert!(selected.evidence.known(A));
                        // Slots were captured before the public reveal. Their
                        // original private bits need not be rewritten; a
                        // tracked world names the same public pick in COMMON.
                        if public && named.public_tracker().is_some() {
                            assert!(named.reference_visible(selected, B));
                        } else if !public {
                            assert!(!selected.evidence.known(B));
                        }
                        assert_eq!(named.rng, rng, "named selection consumes no chance");
                        complete(&mut named, &mut vec![]);
                        assert_eq!(named.position.locate(card), Some(Zone::Hand(A)));
                    }
                    let mut expected_rng = sample.rng.clone();
                    let expected = candidates[(expected_rng.next_u64() % 2) as usize];
                    sample.choose(0).unwrap();
                    let first = menu(&mut sample, &mut vec![]);
                    assert_eq!(first.player, reactor);
                    assert_eq!(sample.rng, expected_rng);
                    assert_eq!(pending(&sample).inner, expected);
                    assert!(!pending(&sample).evidence.known(reactor));
                    complete(&mut sample, &mut vec![]);
                    assert_eq!(sample.position.locate(expected), Some(Zone::Hand(A)));
                }
            }
            assert!(physical_permuted);
            game.choose(0).unwrap();
            let first = menu(&mut game, &mut vec![]);
            let mut private_overlap = false;
            for seed in 0..48 {
                for mut sample in [
                    game.determinize(reactor, seed).unwrap(),
                    game.sample_world(reactor, seed).unwrap().world,
                ] {
                    assert_eq!(menu(&mut sample, &mut vec![]), first);
                    let selected = pending(&sample);
                    if public || reactor == A {
                        assert!(!packet.contains(&selected.inner));
                    } else {
                        private_overlap |= packet.contains(&selected.inner);
                    }
                    assert!(!selected.evidence.known(reactor));
                    complete(&mut sample, &mut vec![]);
                    assert_eq!(sample.position.locate(selected.inner), Some(Zone::Hand(A)));
                }
            }
            if !public && reactor == B {
                assert!(private_overlap, "private menu membership remains private");
            }
        }
    }
}
