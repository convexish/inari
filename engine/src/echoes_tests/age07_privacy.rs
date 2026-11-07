//! Live batch choices use the same observer-local anonymous domains as single choices.
use super::*;
use crate::program::build::*;
use shortcuts::common::*;
const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);
const PACKET: Slot = Slot::effect(0);
const MIXED: Selector = Selector::Union(&[Selector::Slot(PACKET), Selector::Junk]);
fn fixture(public: bool, reactor: Player) -> (Game, [CardId; 2]) {
    packet_fixture(public, reactor, false)
}
fn packet_fixture(
    public: bool,
    reactor: Player,
    retain_before_reveal: bool,
) -> (Game, [CardId; 2]) {
    let mut f = Fixture::default();
    let mut body = vec![draw(9)];
    if public && retain_before_reveal {
        body[0] = draw(9).keep(PACKET);
        body.push(reveal(LAST));
    } else if public {
        body.push(reveal(LAST).keep(PACKET));
    } else {
        body[0] = draw(9).keep(PACKET);
    }
    body.push(junk(LAST));
    body.push(draw(9));
    if public && retain_before_reveal {
        *body.last_mut().unwrap() = draw(9).add(PACKET);
        body.push(reveal(LAST));
    } else if public {
        body.push(reveal(LAST).add(PACKET));
    } else {
        *body.last_mut().unwrap() = draw(9).add(PACKET);
    }
    body.extend([
        junk(LAST),
        transfer(MIXED, Destination::Hand(Who::Executor))
            .exactly(Expr::Const(1))
            .shortcut(Shortcut::AnyOrder),
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
    let mut game = Game::seeded(f.position(), BOTH, 93)
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
fn menu(game: &mut Game) -> Decision {
    decision(game, &mut vec![])
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
fn finish(game: &mut Game) {
    for _ in 0..30 {
        match game.advance(10000, &mut vec![]).unwrap() {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => {
                assert!(game.position.check_conservation());
                return;
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("batch scenario loop")
}

#[test]
fn live_batch_mixed_named_remainder_rebuilds_and_stays_conditioned_through_first() {
    for public in [false, true] {
        for reactor in Player::ALL {
            let (mut game, packet) = fixture(public, reactor);
            let original = menu(&mut game);
            assert_eq!(original.options.len(), 3);
            assert!(matches!(
                original.options[0],
                Alternative::BlindMove {
                    verb: Verb::Transfer,
                    source: Zone::Junk,
                    back: Deck { age: 9, .. }
                }
            ));
            let mut moved_named = false;
            for seed in 0..24 {
                for mut sampled in [
                    game.determinize(A, seed).unwrap(),
                    game.sample_world(A, seed).unwrap().world,
                ] {
                    assert_eq!(menu(&mut sampled), original);
                    let Some(Frame::Choose { tasks, .. }) = sampled.frames.last() else {
                        unreachable!()
                    };
                    let Task::MoveAllBlind(group) = &tasks[0] else {
                        unreachable!()
                    };
                    assert_eq!(group.targets.len(), 2);
                    assert!(
                        group
                            .targets
                            .iter()
                            .all(|target| !packet.contains(&target.card))
                    );
                    moved_named |=
                        sampled.position.cards(Zone::Junk) != game.position.cards(Zone::Junk);
                    let mut expected_rng = sampled.rng.clone();
                    let expected_pick = group.targets
                        [(expected_rng.next_u64() % group.targets.len() as u64) as usize]
                        .card;
                    sampled.choose(0).unwrap();
                    let first = menu(&mut sampled);
                    assert_eq!(sampled.rng, expected_rng);
                    assert_eq!(pending(&sampled).inner, expected_pick);
                    assert_eq!(first.player, reactor);
                    let selected = pending(&sampled);
                    assert!(!packet.contains(&selected.inner));
                    assert!(!selected.evidence.known(reactor));
                    finish(&mut sampled);
                }
            }
            assert!(
                moved_named,
                "exercise different physical placements around retained names"
            );
            game.choose(0).unwrap();
            let first = menu(&mut game);
            assert_eq!(first.player, reactor);
            let selected = pending(&game);
            assert!(!packet.contains(&selected.inner));
            let mut saw_private_overlap = false;
            for seed in 0..64 {
                for mut sampled in [
                    game.determinize(reactor, seed).unwrap(),
                    game.sample_world(reactor, seed).unwrap().world,
                ] {
                    assert_eq!(menu(&mut sampled), first);
                    let selected = pending(&sampled);
                    if public || reactor == A {
                        assert!(!packet.contains(&selected.inner));
                    } else {
                        saw_private_overlap |= packet.contains(&selected.inner);
                    }
                    assert!(!selected.evidence.known(reactor));
                    finish(&mut sampled);
                    assert_eq!(sampled.position.locate(selected.inner), Some(Zone::Hand(A)));
                }
            }
            if !public && reactor == B {
                assert!(
                    saw_private_overlap,
                    "another observer cannot infer the actor's privately named exclusions"
                );
            }
        }
    }
}

#[test]
fn blind_batch_excludes_names_acquired_publicly_after_retaining_private_references() {
    let (mut game, packet) = packet_fixture(true, B, true);
    let original = menu(&mut game);
    assert_eq!(original.options.len(), 3);
    let Some(Frame::Choose { tasks, .. }) = game.frames.last() else {
        unreachable!()
    };
    let Task::MoveAllBlind(group) = &tasks[0] else {
        unreachable!()
    };
    assert_eq!(group.excluded.len(), 2);
    for reference in &group.excluded {
        assert!(!reference.evidence.known(B), "retained before disclosure");
        assert_eq!(
            game.public_tracker()
                .unwrap()
                .pick_identity(reference.evidence.pick().unwrap()),
            Some(reference.inner),
            "the same retained pick is now publicly identified"
        );
    }
    for seed in 0..24 {
        for mut sampled in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            assert_eq!(menu(&mut sampled), original);
            sampled.choose(0).unwrap();
            assert_eq!(menu(&mut sampled).player, B);
            assert!(!packet.contains(&pending(&sampled).inner));
            finish(&mut sampled);
        }
    }
    game.choose(0).unwrap();
    let first = menu(&mut game);
    assert_eq!(first.player, B);
    assert!(!pending(&game).evidence.known(B));
    for seed in 0..64 {
        for mut sampled in [
            game.determinize(B, seed).unwrap(),
            game.sample_world(B, seed).unwrap().world,
        ] {
            assert_eq!(menu(&mut sampled), first);
            let selected = pending(&sampled);
            assert!(
                !packet.contains(&selected.inner),
                "public menu partition remains true in every sampled world"
            );
            assert!(!selected.evidence.known(B));
            finish(&mut sampled);
            assert_eq!(sampled.position.locate(selected.inner), Some(Zone::Hand(A)));
        }
    }
}

#[test]
fn shen_reveals_each_hidden_replacement_operand_only_when_its_color_is_read() {
    let mut f = Fixture::default();
    let card = echoes_card("E068").unwrap();
    let color = card.color;
    let source = f.add(card, Zone::Board(A, color));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.figure(Figure::SunshuAo, Zone::Board(A, Color::Yellow));
    {
        let card = figure_card(Figure::CaiLun.code()).unwrap();
        let color = card.color;
        f.add(card, Zone::Board(B, color));
    }
    f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    for color in [Color::Red, Color::Blue, Color::Purple] {
        f.tech(color, 3, Zone::Score(B), vec![]);
        for value in [1, 2] {
            f.tech(color, value, Zone::Board(A, color), vec![]);
        }
    }
    for _ in 0..4 {
        f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut position = f.position();
    position.turn_moves[A.index()][1] = 5;
    let mut game = Game::seeded(position, BOTH, 73)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let first = menu(&mut game);
    assert_eq!(
        first.options,
        [Alternative::Branch(0), Alternative::Branch(1)]
    );
    game.choose(1).unwrap();
    let mut events = Vec::new();
    let mut reached = false;
    for _ in 0..1000 {
        let mut clone = game.clone();
        let mut actual = Vec::new();
        let mut copied = Vec::new();
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(status, clone.advance(1, &mut copied).unwrap());
        assert_eq!(game, clone);
        assert_eq!(actual, copied);
        events.extend(actual);
        if let Status::Decision(choice) = status {
            let visited = game
                .frames
                .iter()
                .find_map(|f| {
                    if let Frame::MoveAll(batch) = f {
                        (batch.verb == Verb::Score).then_some(&batch.visited)
                    } else {
                        None
                    }
                })
                .unwrap();
            if visited.len() == 2 {
                assert_ne!(visited[0].inner, visited[1].inner);
                assert_eq!(choice.player, A);
                let first = visited[0].inner;
                let second = visited[1].inner;
                assert!(
                    events
                        .iter()
                        .any(|e| matches!(e,Event::Revealed{card,..} if *card==first))
                );
                assert!(
                    !events
                        .iter()
                        .any(|e| matches!(e,Event::Revealed{card,..} if *card==second)),
                    "First reactions precede the reached Instead body"
                );
                for seed in 0..16 {
                    for mut world in [
                        game.determinize(A, seed).unwrap(),
                        game.sample_world(A, seed).unwrap().world,
                    ] {
                        let cards = world
                            .frames
                            .iter()
                            .find_map(|f| {
                                if let Frame::MoveAll(batch) = f {
                                    (batch.verb == Verb::Score).then_some(&batch.visited)
                                } else {
                                    None
                                }
                            })
                            .unwrap();
                        assert_eq!(cards[0].inner, first);
                        assert_ne!(cards[0].inner, cards[1].inner);
                        finish(&mut world);
                    }
                }
                reached = true;
            }
            game.choose(0).unwrap();
        } else if matches!(status, Status::Complete(_)) {
            break;
        }
    }
    assert!(reached);
    assert_eq!(game.position.cards(Zone::Score(B)).len(), 3);
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Revealed { card, .. } = e {
                Some(*card)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(revealed.len(), 3);
    for card in revealed {
        let reveal = events
            .iter()
            .position(|e| matches!(e,Event::Revealed{card:c,..} if *c==card))
            .unwrap();
        let color = game.position.card(card).color;
        assert!(events.iter().enumerate().any(|(i,e)|i>reveal&&matches!(e,Event::Splayed{player:A,color:c,to:Splay::Right,inside_karma:true,..} if *c==color)));
    }
}

#[test]
fn refused_forecast_batch_samples_its_anonymous_attempts_as_distinct_cards() {
    let mut f = Fixture::default();
    let mut card = echoes_card("E032").unwrap();
    card.effects = vec![card.effects[1].clone()];
    card.echo = None;
    f.add(card, Zone::Forecast(A));
    let driver = f.tech(Color::Yellow, 3, Zone::Hand(A), vec![]);
    for _ in 0..5 {
        f.tech(Color::Red, 9, Zone::Forecast(A), vec![]);
    }
    for (age, color) in [
        (9, Color::Red),
        (9, Color::Blue),
        (8, Color::Green),
        (7, Color::Purple),
    ] {
        f.tech(color, age, Zone::Forecast(B), vec![]);
    }
    let mut game = Game::seeded(f.position(), BOTH, 3)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.shortcuts_disabled = true;
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    for _ in 0..2 {
        let choices = menu(&mut game);
        let index = choices
            .options
            .iter()
            .position(|o| {
                matches!(
                    o,
                    Alternative::BlindMove {
                        verb: Verb::Foreshadow,
                        back: Deck { age: 9, .. },
                        ..
                    }
                )
            })
            .unwrap();
        game.choose(index).unwrap();
    }
    let choices = menu(&mut game);
    assert_eq!(choices.options.len(), 2);
    let attempted = |game: &Game| {
        game.frames
            .iter()
            .find_map(|frame| match frame {
                Frame::MoveAll(batch) => Some(batch.visited.clone()),
                _ => None,
            })
            .unwrap()
    };
    // Both refused attempts stayed among B's like-backed Forecast cards; a
    // world keeps them two different cards.
    assert_eq!(attempted(&game).len(), 2);
    for seed in 0..16 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            assert!(world.position.check_conservation());
            // Search worlds keep no tracking; determinized worlds do.
            if let Some(tracker) = world.tracker(A) {
                assert!(tracker.check_truth(&world.position));
                assert_eq!(world.observe(A).unwrap(), game.observe(A).unwrap());
            }
            let visited = attempted(&world);
            assert_ne!(visited[0].inner, visited[1].inner);
            finish(&mut world);
            assert_eq!(world.position.cards(Zone::Forecast(B)).len(), 4);
        }
    }
    finish(&mut game);
    assert_eq!(game.position.cards(Zone::Forecast(B)).len(), 4);
    assert_eq!(game.position.cards(Zone::Forecast(A)).len(), 5);
}

#[test]
fn noether_junks_keep_a_score_batchs_like_backed_attempts_distinct() {
    // B scores four hidden like-backed hand cards. Before each, Emmy Noether
    // has A junk a blind card from B's equally backed score pile, so A loses
    // track of which scored card left: two attempts may share candidates.
    let mut f = Fixture::default();
    f.figure(Figure::EmmyNoether, Zone::Board(A, Color::Green));
    let source = f.tech(
        Color::Yellow,
        10,
        Zone::Board(B, Color::Yellow),
        vec![score(HAND).all()],
    );
    for color in [Color::Red, Color::Blue, Color::Purple, Color::Yellow] {
        f.tech(color, 5, Zone::Hand(B), vec![]);
    }
    for color in [Color::Red, Color::Green] {
        f.tech(color, 5, Zone::Score(B), vec![]);
    }
    // A second back makes each junk a real choice for A.
    f.tech(Color::Green, 6, Zone::Score(B), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 5)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(B),
    )
    .unwrap();
    let visited = |game: &Game| {
        game.frames
            .iter()
            .find_map(|frame| match frame {
                Frame::MoveAll(batch) => Some(batch.visited.clone()),
                _ => None,
            })
            .unwrap()
    };
    let mut sampled = 0;
    loop {
        match game.advance(10000, &mut vec![]).unwrap() {
            Status::Decision(choice) => {
                if choice.player == A && visited(&game).len() >= 2 {
                    for seed in 0..16 {
                        let world = game.determinize(A, seed).unwrap();
                        assert!(world.position.check_conservation());
                        assert!(world.tracker(A).unwrap().check_truth(&world.position));
                        assert_eq!(world.observe(A).unwrap(), game.observe(A).unwrap());
                        let cards: Vec<_> = visited(&world).iter().map(|held| held.inner).collect();
                        for (index, card) in cards.iter().enumerate() {
                            assert!(!cards[index + 1..].contains(card));
                        }
                        let mut world = game.sample_world(A, seed).unwrap().world;
                        finish(&mut world);
                        assert_eq!(world.position.cards(Zone::Score(B)).len(), 3);
                    }
                    sampled += 1;
                }
                game.choose(0).unwrap();
            }
            Status::Complete(_) => break,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(sampled, 3);
    assert!(game.position.cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position.cards(Zone::Score(B)).len(), 3);
    assert_eq!(game.position.cards(Zone::Junk).len(), 4);
}

#[test]
fn reached_karma_proof_restores_last_chosen_and_boxed_registers_before_resuming() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        for value in [1, 2] {
            f.tech(Color::Red, value, Zone::Board(A, Color::Red), vec![]);
        }
        let selected = f.tech(Color::Red, 3, Zone::Score(B), vec![]);
        let untouched = f.tech(Color::Blue, 3, Zone::Score(B), vec![]);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        game.start(
            Operation::Move {
                verb: Verb::Score,
                target: target(selected, Zone::Score(B)),
                destination: Zone::Score(A),
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let mut entered = false;
        for _ in 0..100 {
            if matches!(
                game.frames.last(),
                Some(Frame::Program {
                    source: Source::Karma { .. },
                    pc: 0,
                    ..
                })
            ) {
                entered = true;
                break;
            }
            assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
        }
        assert!(entered);
        assert!(events.is_empty());
        let old = Captured::new(
            ReturnValue::Operation(OperationOutcome::empty(Disposition::Inapplicable)),
            Evidence::NONE,
        );
        let Some(Frame::Program {
            last,
            chosen,
            boxed,
            ..
        }) = game.frames.last_mut()
        else {
            unreachable!()
        };
        *last = old;
        *chosen = SelectionParameter::Value(7);
        boxed.increase(1).unwrap();
        // These are legal saved interpreter registers, intentionally distinct
        // from the proof's own performed-card result and chosen parameters.
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
        assert!(
            game.frames
                .iter()
                .any(|frame| matches!(frame, Frame::KarmaProof { .. }))
        );
        let mut resumed = false;
        for _ in 0..100 {
            let mut cloned = game.clone();
            let mut next = vec![];
            let mut other = vec![];
            let status = game.advance(1, &mut next).unwrap();
            assert_eq!(status, cloned.advance(1, &mut other).unwrap());
            assert_eq!(next, other);
            assert_eq!(game, cloned);
            events.extend(next);
            if let Some(Frame::Program {
                source: Source::Karma { .. },
                pc: 0,
                last,
                chosen,
                boxed,
                retained: Some(bound),
                ..
            }) = game.frames.last()
            {
                assert_eq!(*last, old);
                assert_eq!(*chosen, SelectionParameter::Value(7));
                assert_eq!(boxed.value(1).unwrap(), 2);
                assert!(Player::ALL.into_iter().all(|p| bound.evidence.known(p)));
                resumed = true;
                break;
            }
        }
        assert!(resumed);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e,Event::Revealed{card,..} if *card==selected))
                .count(),
            1
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e,Event::Revealed{card,..} if *card==untouched))
        );
        finish(&mut game);
        assert_eq!(game.position.splay(A, Color::Red), Splay::Right);
        assert_eq!(game.position.cards(Zone::Score(B)).len(), 2);
    }
}

#[test]
fn catherine_verification_events_do_not_depend_on_a_trackers_prior_public_inference() {
    let mut f = Fixture::default();
    f.figure(Figure::CatherineTheGreat, Zone::Board(B, Color::Purple));
    let incoming = f.add(base_card("B011").unwrap(), Zone::Hand(A));
    let top = f.add(base_card("B012").unwrap(), Zone::Board(A, Color::Blue));
    f.add(base_card("B010").unwrap(), Zone::Deck(Deck::base(1)));
    let plain = Game::seeded(f.position(), BOTH, 31).unwrap();
    let mut variants = [plain.clone(), plain.with_tracking().unwrap()];
    let mut traces = Vec::new();
    for game in &mut variants {
        game.start(
            Operation::Reveal {
                target: Target {
                    card: incoming,
                    physical: Zone::Hand(A),
                    logical: LogicalZone::Hand(A),
                },
            },
            Context::new(A),
        )
        .unwrap();
        let mut history = Vec::new();
        assert!(matches!(
            game.advance(1000, &mut history).unwrap(),
            Status::Complete(_)
        ));
        assert_eq!(
            history,
            [Event::Revealed {
                card: incoming,
                source: Zone::Hand(A),
                executor: A,
                inside_karma: false
            }]
        );
        game.start(
            Operation::Move {
                verb: Verb::Meld,
                target: Target {
                    card: incoming,
                    physical: Zone::Hand(A),
                    logical: LogicalZone::Hand(A),
                },
                destination: Zone::Board(A, Color::Blue),
            },
            Context::new(A),
        )
        .unwrap();
        let mut clone = game.clone();
        let mut trace = Vec::new();
        assert!(matches!(
            game.advance(1000, &mut trace).unwrap(),
            Status::Complete(_)
        ));
        let mut tiny = Vec::new();
        loop {
            match clone.advance(1, &mut tiny).unwrap() {
                Status::WorkLimit => {}
                Status::Complete(_) => break,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(tiny, trace);
        assert_eq!(clone.position, game.position);
        assert_eq!(game.position.locate(top), Some(Zone::Hand(A)));
        assert_eq!(
            trace[0],
            Event::Revealed {
                card: incoming,
                source: Zone::Hand(A),
                executor: A,
                inside_karma: true
            }
        );
        assert_eq!(trace.len(), 3);
        traces.push(trace);
    }
    assert_eq!(traces[0], traces[1]);
    assert_eq!(variants[0].position, variants[1].position);
    assert_eq!(variants[0].rng, variants[1].rng);
}

#[test]
fn combustion_transfers_wernhers_junk_the_victim_sees_but_the_demander_cannot() {
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
                    knowledge,
                    ..
                } => Some(Captured::new(
                    target.card,
                    knowledge.as_ref().map_or(Evidence::NONE, |k| k.evidence),
                )),
                _ => None,
            })
            .unwrap()
    }

    for tracked in [false, true] {
        let mut f = Fixture::default();
        let source = f.add(base_card("B066").unwrap(), Zone::Board(A, Color::Red));
        f.figure(Figure::WernherVonBraun, Zone::Board(B, Color::Blue));
        f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
        for color in [Color::Yellow, Color::Purple] {
            let top = f.tech(color, 1, Zone::Board(A, color), vec![]);
            f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
        }
        let same_back = [Color::Red, Color::Blue].map(|color| f.tech(color, 9, Zone::Junk, vec![]));
        let eight = f.tech(Color::Yellow, 8, Zone::Junk, vec![]);
        let own_score = f.tech(Color::Purple, 9, Zone::Score(B), vec![]);
        for color in [Color::Blue, Color::Purple] {
            f.tech(color, 7, Zone::Deck(Deck::base(7)), vec![]);
        }
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 71).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        game.shortcuts_disabled = true;
        let mut context = Context::new(B);
        context.demander = A;
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            context,
        )
        .unwrap();
        let initial = menu(&mut game);
        assert_eq!(initial.player, B);
        // Wernher's owner looks at the junk: every alias is a known choice.
        let known = |card, source| Alternative::Move {
            verb: Verb::Transfer,
            target: TargetDescriptor::Known { card, source },
        };
        assert_eq!(initial.options.len(), 4);
        for option in [
            known(own_score, Zone::Score(B)),
            known(same_back[0], Zone::Junk),
            known(same_back[1], Zone::Junk),
            known(eight, Zone::Junk),
        ] {
            assert!(initial.options.contains(&option), "{option:?}");
        }
        let expected = same_back[1];
        let pick = initial
            .options
            .iter()
            .position(|option| *option == known(expected, Zone::Junk))
            .unwrap();
        game.choose(pick).unwrap();
        let first = menu(&mut game);
        assert_eq!(first.player, A);
        assert_eq!(pending(&game).inner, expected);
        // The demander never saw the junk: which [9] is coming stays hidden.
        assert!(!pending(&game).evidence.known(A));
        assert!(game.frames.iter().any(|frame| matches!(
            frame,
            Frame::Operation {
                request: Operation::Move { target, .. }, ..
            } if target.card == expected && target.physical == Zone::Junk
                && target.logical == LogicalZone::Score(B)
        )));
        if tracked {
            for seed in 0..24 {
                for mut sampled in [
                    game.determinize(A, seed).unwrap(),
                    game.sample_world(A, seed).unwrap().world,
                ] {
                    assert_eq!(menu(&mut sampled), first);
                    let selected = pending(&sampled);
                    assert!(!selected.evidence.known(A));
                    assert_eq!(sampled.position.locate(selected.inner), Some(Zone::Junk));
                    finish(&mut sampled);
                    assert_eq!(
                        sampled.position.locate(selected.inner),
                        Some(Zone::Score(A))
                    );
                }
            }
        }
        let mut clone = game.clone();
        finish(&mut game);
        finish(&mut clone);
        assert_eq!(game.position, clone.position);
        assert_eq!(game.rng, clone.rng);
        assert_eq!(game.position.locate(expected), Some(Zone::Score(A)));
    }
}
