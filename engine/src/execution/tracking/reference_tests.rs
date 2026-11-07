use super::*;

/// A retained reference predates its reveal, so the non-owner must name it
/// through the pick's public support, not a PUBLIC bit on the reference.
fn retained_hand_pick(owner: Player, revealed: bool) -> (Game, HeldCard) {
    let mut source = base_card("B012").unwrap();
    source.effects[0].instructions = vec![
        build::choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
        build::choose(Who::Opponent, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
        build::return_(Selector::Reg(Reg::Bound)),
    ];
    let board = Zone::Board(owner, source.color);
    let hand = Zone::Hand(owner);
    let position = Position::new(
        vec![
            source,
            base_card("B001").unwrap(),
            base_card("B002").unwrap(),
            base_card("B003").unwrap(),
            base_card("B004").unwrap(),
        ],
        &[
            (CardId(0), board),
            (CardId(1), hand),
            (CardId(2), hand),
            (CardId(3), hand),
            (CardId(4), Zone::Deck(Deck::base(1))),
        ],
        owner,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    let picked = game.capture(CardId(1));
    let pick = picked.evidence.pick().unwrap();
    assert!(!picked.evidence.known(owner.opponent()));
    if revealed {
        game.knowledge.as_mut().unwrap().moving = Some(pick);
        game.commit(
            Operation::Reveal {
                target: Target {
                    card: picked.inner,
                    physical: hand,
                    logical: LogicalZone::Physical(hand),
                },
            },
            Context::new(owner),
            false,
            &mut Vec::new(),
        )
        .unwrap();
        game.refresh_tracking().unwrap();
        assert_eq!(
            game.public_tracker().unwrap().pick_identity(pick),
            Some(picked.inner)
        );
    }
    game.frames.push(Frame::Program {
        source: Source::Effect {
            card: CardId(0),
            effect: 0,
        },
        pc: 0,
        context: Context::new(owner),
        last: HeldReturn::UNIT,
        retained: Some(picked),
        chosen: SelectionParameter::None,
        boxed: BoxedValues::default(),
    });
    let Status::Decision(menu) = game.advance(1000, &mut Vec::new()).unwrap() else {
        panic!("owner's choice");
    };
    assert_eq!(menu.player, owner);
    (game, picked)
}

#[test]
fn reconstructed_opponent_keeps_a_publicly_identified_hand_pick() {
    for owner in Player::ALL {
        let (game, picked) = retained_hand_pick(owner, true);
        let pick = picked.evidence.pick().unwrap();
        let other = owner.opponent();
        for seed in 0..16 {
            let world = game.determinize(owner, seed).unwrap();
            // Sound zone beliefs alone cannot detect loss of pick identity.
            assert_eq!(
                world.tracker(other).unwrap().belief(Zone::Hand(owner)),
                game.tracker(other).unwrap().belief(Zone::Hand(owner))
            );
            assert_eq!(
                world.public_tracker().unwrap().pick_identity(pick),
                Some(picked.inner)
            );
            assert_eq!(
                world.tracker(other).unwrap().pick_identity(pick),
                Some(picked.inner)
            );
            assert_eq!(world.context_refs(other), game.context_refs(other));
        }
    }
}

#[test]
fn nested_samples_preserve_public_pick_identity_without_pinning_private_picks() {
    for owner in Player::ALL {
        for revealed in [false, true] {
            let (game, picked) = retained_hand_pick(owner, revealed);
            let mut identities = std::collections::BTreeSet::new();
            for seed in 0..32 {
                let mut world = game.determinize(owner, seed).unwrap();
                world.choose(0).unwrap();
                let Status::Decision(menu) = world.advance(1000, &mut Vec::new()).unwrap() else {
                    panic!("opponent's choice");
                };
                assert_eq!(menu.player, owner.opponent());
                let mut nested = world.determinize(owner.opponent(), seed + 32).unwrap();
                let Frame::Program {
                    retained: Some(retained),
                    ..
                } = &nested.frames[0]
                else {
                    panic!("retained reference");
                };
                let sampled_card = retained.inner;
                identities.insert(sampled_card);
                if revealed {
                    assert_eq!(
                        sampled_card, picked.inner,
                        "public pick rebound at seed {seed}"
                    );
                }
                nested.choose(0).unwrap();
                let mut events = Vec::new();
                assert!(matches!(
                    nested.advance(1000, &mut events).unwrap(),
                    Status::Complete(_)
                ));
                assert!(events.iter().any(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Return) && m.card == sampled_card)));
                assert_eq!(
                    nested.position.cards(Zone::Deck(Deck::base(1)))[0],
                    sampled_card
                );
                assert!(
                    nested
                        .public_tracker()
                        .unwrap()
                        .check_truth(&nested.position)
                );
                for viewer in Player::ALL {
                    assert!(
                        nested
                            .tracker(viewer)
                            .unwrap()
                            .check_truth(&nested.position)
                    );
                }
            }
            if !revealed {
                assert!(
                    identities.len() > 1,
                    "a private pick must still be resampled"
                );
            }
        }
    }
}

#[test]
fn sampled_trackers_clear_retired_pick_support_before_reusing_its_id() {
    for owner in Player::ALL {
        let (mut game, _) = retained_hand_pick(owner, true);
        // This reference is not retained by the game, so sampling retires it.
        let retired = game.capture(CardId(2)).evidence.pick().unwrap();
        let mut world = game.determinize(owner, 7).unwrap();
        let fresh = world.capture(CardId(4)).evidence.pick().unwrap();
        assert_eq!(fresh, retired, "the free pick id is reused");
        assert_eq!(
            world.tracker(owner).unwrap().pick_identity(fresh),
            Some(CardId(4)),
            "the new deck pick must not inherit a retired hand candidate"
        );
        world.refresh_tracking().unwrap();
    }
}

/// A running public program retains a returned hand card, then the source
/// itself returns and that deck shuffles before the next real choice.
fn returned_register(bulk: bool) -> Game {
    let mut source = base_card("B001").unwrap();
    let source_color = source.color;
    source.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![
            Instruction::Bind(Binding::Card(Selector::Reg(Reg::Last))),
            Instruction::Choose {
                who: Who::Executor,
                choice: Choice::Value(ValueChoices::Any),
                optional: false,
                keep: None,
                alias: None,
            },
            Instruction::Move(Move {
                verb: Verb::Meld,
                from: Selector::Reg(Reg::Bound),
                to: Destination::Board(Who::Executor),
                amount: Amount::One { optional: false },
                shortcut: None,
                verify: false,
                reveal_if_empty: None,
                by: None,
                when: None,
                keep: None,
            }),
        ],
    }];
    let hand = Zone::Hand(Player::One);
    let board = Zone::Board(Player::One, source_color);
    let deck = Zone::Deck(Deck::base(1));
    let position = Position::new(
        vec![
            source,
            base_card("B002").unwrap(),
            base_card("B003").unwrap(),
        ],
        &[(CardId(0), board), (CardId(1), hand), (CardId(2), deck)],
        Player::One,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    let context = Context::new(Player::One);
    game.start(
        Operation::Move {
            verb: Verb::Return,
            target: Target {
                card: CardId(1),
                physical: hand,
                logical: LogicalZone::Physical(hand),
            },
            destination: deck,
        },
        context,
    )
    .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Complete(_)
    ));
    let result = game.completed;
    assert!(
        result.evidence.known(Player::One) && !result.evidence.known(Player::Two),
        "private return is known to its owner only"
    );
    assert!(
        result.evidence.pick().is_some(),
        "the other observer follows it"
    );
    let last = if bulk {
        let mut batch = HeldBulk::default();
        batch.record_original(result.operation().unwrap());
        // An unsuccessful descendant cannot overwrite the last successful
        // original's identity or its per-observer knowledge.
        batch.record_original(Captured::new(
            OperationOutcome::empty(Disposition::Replaced),
            Evidence::PUBLIC,
        ));
        batch.returned()
    } else {
        result
    };
    game.frames.push(Frame::Program {
        source: Source::Effect {
            card: CardId(0),
            effect: 0,
        },
        pc: 0,
        context: Context {
            source_card: Some(CardId(0)),
            source_evidence: Evidence::PUBLIC,
            ..context
        },
        last,
        retained: None,
        chosen: SelectionParameter::None,
        boxed: BoxedValues::default(),
    });
    game.commit(
        Operation::Move {
            verb: Verb::Return,
            target: Target {
                card: CardId(0),
                physical: board,
                logical: LogicalZone::Physical(board),
            },
            destination: deck,
        },
        context,
        false,
        &mut Vec::new(),
    )
    .unwrap();
    game.commit(
        Operation::Shuffle {
            deck: Deck::base(1),
        },
        context,
        false,
        &mut Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Decision(_)
    ));
    game
}

#[test]
fn original_and_bulk_results_keep_identity_through_bind_source_return_and_shuffle() {
    for bulk in [false, true] {
        let game = returned_register(bulk);
        let before = game.clone();
        let mut positions = std::collections::BTreeSet::new();
        for seed in 0..32 {
            let mut world = game.determinize(Player::One, seed).unwrap();
            let stack = world.position.cards(Zone::Deck(Deck::base(1)));
            positions.insert(stack.iter().position(|&card| card == CardId(1)).unwrap());
            let Frame::Program {
                retained,
                last,
                context,
                ..
            } = &world.frames[0]
            else {
                panic!("program");
            };
            let retained = retained.unwrap();
            assert_eq!(retained.inner, CardId(1));
            assert!(retained.evidence.known(Player::One));
            assert_eq!(last.card(), Some(retained));
            assert_eq!(context.source_card, Some(CardId(0)));
            world.choose(0).unwrap();
            assert!(matches!(
                world.advance(1000, &mut Vec::new()).unwrap(),
                Status::Complete(_)
            ));
            let color = world.position.card(CardId(1)).color;
            assert_eq!(world.position.top(Player::One, color), Some(CardId(1)));
            for viewer in Player::ALL {
                assert!(world.tracker(viewer).unwrap().check_truth(&world.position));
            }
        }
        assert_eq!(
            positions.len(),
            3,
            "a retained identity must not fix its deck index"
        );
        assert_eq!(game, before, "sampling consumes neither real state nor RNG");
    }
}

#[test]
fn a_draw_after_first_does_not_inherit_the_previous_deck_tops_identity() {
    let deck = Zone::Deck(Deck::base(1));
    let position = Position::new(
        vec![
            base_card("B001").unwrap(),
            base_card("B002").unwrap(),
            base_card("B003").unwrap(),
        ],
        &[(CardId(0), deck), (CardId(1), deck), (CardId(2), deck)],
        Player::One,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    let context = Context::new(Player::One);
    let old_top = Target {
        card: CardId(2),
        physical: deck,
        logical: LogicalZone::Physical(deck),
    };
    game.start(Operation::Reveal { target: old_top }, context)
        .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Complete(_)
    ));
    assert!(
        Player::ALL
            .into_iter()
            .all(|p| game.completed.evidence.known(p))
    );
    // A First child takes the revealed top before the original draw commits.
    // A draw is not the exact-identity promise made by a pending meld.
    game.frames.push(Frame::Operation {
        request: Operation::Draw {
            value: 1,
            set: None,
        },
        context,
        phase: Phase::Commit,
        knowledge: Some(OperandKnowledge {
            evidence: Evidence::PUBLIC,
            conditioned: false,
            private_query: None,
            verify: false,
        }),
        short: false,
    });
    game.commit(
        Operation::Move {
            verb: Verb::Score,
            target: old_top,
            destination: Zone::Score(Player::One),
        },
        context,
        false,
        &mut Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Complete(_)
    ));
    let drawn = game.completed.card().unwrap();
    assert_eq!(drawn.inner, CardId(1));
    assert!(drawn.evidence.known(Player::One) && !drawn.evidence.known(Player::Two));
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(&game.position));
    }
}

#[test]
fn retained_identity_evidence_is_observer_specific() {
    let mut game = returned_register(false);
    // A foreign controller at this same suspended choice must not inherit the
    // returning owner's private identity evidence from the register wrapper.
    // It saw the card go to the deck, so the reference follows that card's
    // own tracked slot: varied identity, never the real one pinned.
    let Some(Frame::Choose { player, .. }) = game.frames.last_mut() else {
        panic!("choice");
    };
    *player = Player::Two;
    let mut identities = std::collections::BTreeSet::new();
    for seed in 0..32 {
        let world = game.determinize(Player::Two, seed).unwrap();
        let Frame::Program { retained, last, .. } = &world.frames[0] else {
            panic!("program");
        };
        let retained = retained.unwrap();
        assert_eq!(last.card(), Some(retained));
        assert_eq!(
            world.position.locate(retained.inner),
            Some(Zone::Deck(Deck::base(1)))
        );
        identities.insert(retained.inner);
        for viewer in Player::ALL {
            assert!(world.tracker(viewer).unwrap().check_truth(&world.position));
        }
    }
    assert!(identities.len() > 1, "the real identity is not pinned");
}

#[test]
fn a_reference_keeps_a_known_identity_and_otherwise_takes_its_picks_card() {
    let mut game = returned_register(false);
    let pick = game.knowledge.as_mut().unwrap().allocate().unwrap();
    let mut picks = [None; crate::tracking::PICKS];
    picks[pick.index()] = Some(CardId(0));
    let map = remap::Remap::new(
        remap::Mode::Sample {
            cards: &[CardId(0), CardId(2), CardId(1)],
            picks: &picks,
        },
        Player::One,
        &game.position,
        vec![false; 3],
    );
    let mut remembered = Captured::new(
        CardId(1),
        Evidence::known_to(Player::One).with_pick(Some(pick)),
    );
    let mut followed = Captured::new(CardId(1), Evidence::NONE.with_pick(Some(pick)));
    map.held(&mut remembered);
    map.held(&mut followed);
    assert_eq!(remembered.inner, CardId(1));
    assert_eq!(
        followed.inner,
        CardId(0),
        "an unnamed reference takes its pick's card, not its position's"
    );
    assert!(map.opaque.get().is_none());
    let mut untracked = Captured::new(CardId(1), Evidence::NONE);
    map.held(&mut untracked);
    assert_eq!(map.opaque.get(), Some(remap::UNTRACKED));
}

#[test]
fn visited_cards_are_named_references_and_metadata_is_not_a_loop_witness() {
    let mut game = returned_register(false);
    game.frames.pop(); // next fixture boundary is after a bulk return
    let mut batch = MoveAll::new(
        Verb::Return,
        Selector::Hand(Who::Executor),
        Destination::Deck,
        Context::new(Player::One),
    );
    batch
        .visited
        .push(Captured::new(CardId(1), Evidence::known_to(Player::One)));
    game.frames.push(Frame::MoveAll(Box::new(batch)));
    super::tests::pause(&mut game);
    for seed in 0..8 {
        let world = game.determinize(Player::One, seed).unwrap();
        let Frame::MoveAll(batch) = &world.frames[1] else {
            panic!("batch");
        };
        assert_eq!(
            batch.visited,
            [Captured::new(CardId(1), Evidence::known_to(Player::One))]
        );
    }
    let rules = game.rules_frames();
    let Frame::Program {
        retained,
        last,
        context,
        ..
    } = &mut game.frames[0]
    else {
        panic!("program");
    };
    retained.as_mut().unwrap().evidence = Evidence::NONE;
    last.evidence = Evidence::NONE;
    context.source_evidence = Evidence::NONE;
    let Frame::MoveAll(batch) = &mut game.frames[1] else {
        panic!("batch");
    };
    batch.visited[0].evidence = Evidence::PUBLIC;
    assert_eq!(game.rules_frames(), rules);
    assert_ne!(
        game.frames, rules,
        "ordinary clone equality still checks metadata"
    );
}

#[test]
fn an_entered_reaction_cannot_discard_its_already_consumed_front() {
    let emma = figure_card(Figure::EmmaWatson.code()).unwrap();
    let incoming = base_card("B001").unwrap();
    let color = incoming.color;
    let mut top = base_card("B002").unwrap();
    top.color = color; // explicit scenario: Emma has a card of this color to return
    let position = Position::new(
        vec![emma, incoming, top, base_card("B003").unwrap()],
        &[
            (CardId(0), Zone::Board(Player::One, Color::Purple)),
            (CardId(1), Zone::Hand(Player::Two)),
            (CardId(2), Zone::Board(Player::Two, color)),
            (CardId(3), Zone::Deck(Deck::base(1))),
        ],
        Player::One,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    let hand = Zone::Hand(Player::Two);
    game.start(
        Operation::Move {
            verb: Verb::Meld,
            target: Target {
                card: CardId(1),
                physical: hand,
                logical: LogicalZone::Physical(hand),
            },
            destination: Zone::Board(Player::Two, color),
        },
        Context::new(Player::Two),
    )
    .unwrap();
    let mut entered = false;
    for _ in 0..10 {
        game.advance(1, &mut Vec::new()).unwrap();
        if game
            .frames
            .iter()
            .any(|frame| matches!(frame, Frame::Karma { stage: 1, .. }))
        {
            entered = true;
            break;
        }
    }
    assert!(entered);
    assert_eq!(game.determinize(Player::One, 0), Err(Error::Busy));
    // Probe the same guard with a synthetic pause after Emma read the color.
    // WorkLimit itself remains deliberately unsupported as a search root.
    super::tests::pause(&mut game);
    for seed in 0..8 {
        assert_eq!(
            game.determinize(Player::One, seed),
            Err(Error::Unsupported(remap::HIDDEN_FRONT))
        );
    }
}

#[test]
fn a_draw_karma_operand_is_the_sampled_deck_top_not_a_tracked_card() {
    // Galileo's First reaction to a draw pauses on its value choice (a
    // nonempty 4 keeps it real). Its operand is "the card the draw would
    // take": whatever the sampled world has on top, even though setup
    // shuffles hide every deck position.
    let galileo = figure_card(Figure::GalileoGalilei.code()).unwrap();
    let mut places = vec![(CardId(0), Zone::Board(Player::One, galileo.color))];
    let mut cards = vec![galileo];
    for code in ["B001", "B002", "B003", "B004", "B005"] {
        places.push((CardId(cards.len() as u16), Zone::Deck(Deck::base(1))));
        cards.push(base_card(code).unwrap());
    }
    places.push((CardId(cards.len() as u16), Zone::Deck(Deck::base(4))));
    cards.push(base_card("B036").unwrap());
    let position = Position::new(cards, &places, Player::One).unwrap();
    let mut game = Game::new(position, Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        Context::new(Player::One),
    )
    .unwrap();
    let Status::Decision(menu) = game.advance(1000, &mut Vec::new()).unwrap() else {
        panic!("Galileo's value choice");
    };
    assert_eq!(menu.options, [Alternative::Value(4), Alternative::Value(5)]);
    let mut tops = std::collections::BTreeSet::new();
    for seed in 0..32 {
        let world = game.determinize(Player::One, seed).unwrap();
        let operand = world
            .frames
            .iter()
            .find_map(|frame| match frame {
                Frame::Karma { hook, .. } => hook.event.operand(),
                _ => None,
            })
            .expect("entered draw karma");
        let top = *world
            .position
            .cards(Zone::Deck(Deck::base(1)))
            .last()
            .unwrap();
        assert_eq!(operand.inner, top);
        tops.insert(top);
    }
    assert!(tops.len() > 1, "the hidden top is not pinned");
}

#[test]
fn a_pick_and_the_result_of_moving_it_stay_one_card() {
    // Player Two scores a hidden hand card it picked. Player One saw a pick
    // from Two's hand and that card arrive in Two's score pile. The move took
    // the picked card, so both references share its pick.
    let codes = ["B001", "B002", "B003", "B004", "B005"];
    let hand = Zone::Hand(Player::Two);
    let score = Zone::Score(Player::Two);
    let places: Vec<_> = (0..codes.len())
        .map(|i| (CardId(i as u16), if i < 3 { hand } else { score }))
        .collect();
    let cards = codes.iter().map(|code| base_card(code).unwrap()).collect();
    let position = Position::new(cards, &places, Player::One).unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    let context = Context::new(Player::Two);
    let picked = game.capture(CardId(1));
    assert!(!picked.evidence.known(Player::One) && picked.evidence.pick().is_some());
    game.knowledge.as_mut().unwrap().moving = picked.evidence.pick();
    let outcome = game
        .commit(
            Operation::Move {
                verb: Verb::Score,
                target: Target {
                    card: CardId(1),
                    physical: hand,
                    logical: LogicalZone::Physical(hand),
                },
                destination: score,
            },
            context,
            false,
            &mut Vec::new(),
        )
        .unwrap();
    let moved = game.knowledge.as_mut().unwrap().moved.take();
    let arrived = Captured::new(outcome, game.arrived(CardId(1), moved));
    assert_eq!(arrived.evidence.pick(), picked.evidence.pick());
    let mut batch = MoveAll::new(
        Verb::Score,
        Selector::Hand(Who::Executor),
        Destination::Score(Who::Executor),
        context,
    );
    batch.visited.push(picked);
    batch.outcome.record_original(arrived);
    game.frames.push(Frame::MoveAll(Box::new(batch)));
    super::tests::pause(&mut game);
    let mut identities = std::collections::BTreeSet::new();
    for seed in 0..32 {
        let world = game.determinize(Player::One, seed).unwrap();
        let Frame::MoveAll(batch) = &world.frames[0] else {
            panic!("batch");
        };
        let visited = batch.visited[0].inner;
        assert_eq!(batch.outcome.card, Some(visited));
        assert_eq!(world.position.locate(visited), Some(score));
        identities.insert(visited);
    }
    assert!(identities.len() > 1, "the scored card is not pinned");
}

/// Player Two would meld its hand card 0 (a karma paused the meld), then
/// returned its hand card `returned` to the deck. Player One sees only an
/// age-1 back leave Two's hand: it cannot tell whether it was the meld's
/// card. Two also transfers `shown` to One's hand, if given: One sees that
/// card, but again not whether it is the meld's card.
fn pending_meld_then_hidden_departure(returned: CardId, shown: Option<CardId>) -> Game {
    let codes = ["B001", "B002", "B003", "B004", "B005", "B006"];
    let hand = Zone::Hand(Player::Two);
    let deck = Zone::Deck(Deck::base(1));
    let places: Vec<_> = (0..codes.len())
        .map(|i| (CardId(i as u16), if i < 4 { hand } else { deck }))
        .collect();
    let cards: Vec<_> = codes.iter().map(|code| base_card(code).unwrap()).collect();
    let color = cards[0].color;
    let position = Position::new(cards, &places, Player::Two).unwrap();
    let mut game = Game::new(position, Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    let context = Context::new(Player::Two);
    let operand = game.capture(CardId(0));
    game.frames.push(Frame::Operation {
        request: Operation::Move {
            verb: Verb::Meld,
            target: Target {
                card: CardId(0),
                physical: hand,
                logical: LogicalZone::Physical(hand),
            },
            destination: Zone::Board(Player::Two, color),
        },
        context,
        phase: Phase::Commit,
        knowledge: Some(OperandKnowledge {
            evidence: operand.evidence,
            conditioned: false,
            private_query: None,
            verify: false,
        }),
        short: false,
    });
    let mut moves = vec![(returned, deck)];
    moves.extend(shown.map(|card| (card, Zone::Hand(Player::One))));
    for (card, destination) in moves {
        // As the operation step does: a fresh pick of Two's choice, which
        // the move follows into its result.
        let picked = game.capture(card);
        game.knowledge.as_mut().unwrap().moving = picked.evidence.pick();
        let outcome = game
            .commit(
                Operation::Move {
                    verb: if destination == deck {
                        Verb::Return
                    } else {
                        Verb::Transfer
                    },
                    target: Target {
                        card,
                        physical: hand,
                        logical: LogicalZone::Physical(hand),
                    },
                    destination,
                },
                context,
                false,
                &mut Vec::new(),
            )
            .unwrap();
        let moved = game.knowledge.as_mut().unwrap().moved.take();
        let evidence = game.arrived(card, moved) | picked.evidence;
        game.frames
            .push(Frame::ReturnOutcome(Captured::new(outcome, evidence)));
    }
    super::tests::pause(&mut game);
    game
}

fn pending_operand(world: &Game) -> CardId {
    match world.frames[0] {
        Frame::Operation {
            request: Operation::Move { target, .. },
            ..
        } => target.card,
        _ => panic!("pending meld"),
    }
}

#[test]
fn which_hidden_card_left_does_not_reach_the_sample() {
    // The real games differ only in which of Two's hand cards went to the
    // deck. One's samples must not differ at all.
    let operand_left = pending_meld_then_hidden_departure(CardId(0), None);
    let other_left = pending_meld_then_hidden_departure(CardId(1), None);
    assert_ne!(operand_left.position, other_left.position);
    let mut operand_in_deck = 0;
    let mut operand_returned = 0;
    for seed in 0..64 {
        let world = operand_left.determinize(Player::One, seed).unwrap();
        assert_eq!(
            world,
            other_left.determinize(Player::One, seed).unwrap(),
            "seed {seed}"
        );
        let operand = pending_operand(&world);
        let Frame::ReturnOutcome(returned) = world.frames[1] else {
            panic!("return result");
        };
        operand_in_deck +=
            usize::from(world.position.locate(operand) != Some(Zone::Hand(Player::Two)));
        operand_returned += usize::from(returned.card == Some(operand));
        for viewer in Player::ALL {
            assert!(world.tracker(viewer).unwrap().check_truth(&world.position));
        }
    }
    // One of four same-back cards left, so the meld's card is sometimes the
    // returned one (in the deck) and sometimes still in the hand.
    assert!(operand_in_deck > 0 && operand_in_deck < 64);
    assert_eq!(operand_in_deck, operand_returned);
}

#[test]
fn a_card_arriving_in_view_does_not_name_the_meld_it_might_be() {
    // Two transfers one hand card to One. One sees it, but not whether it is
    // the card Two is melding: what One may know and sample is the same.
    let operand_shown = pending_meld_then_hidden_departure(CardId(1), Some(CardId(0)));
    let other_shown = pending_meld_then_hidden_departure(CardId(1), Some(CardId(2)));
    for viewer in Player::ALL {
        assert_eq!(
            operand_shown.pending_melds(viewer)[0].card,
            if viewer == Player::Two {
                CardView::Known(CardId(0))
            } else {
                CardView::Back(Deck::base(1))
            }
        );
    }
    assert_eq!(
        operand_shown.context_refs(Player::One),
        other_shown.context_refs(Player::One)
    );
    // The shown cards differ, so the samples may too. How often the meld's
    // card is the one One saw must not: one of four candidates either way.
    let shown = |game: &Game, card| {
        (0..128)
            .filter(|&seed| pending_operand(&game.determinize(Player::One, seed).unwrap()) == card)
            .count()
    };
    for count in [
        shown(&operand_shown, CardId(0)),
        shown(&other_shown, CardId(2)),
    ] {
        assert!((16..=64).contains(&count), "{count} of 128");
    }
}

#[test]
fn references_sharing_a_pick_stay_one_card_when_one_is_known() {
    // One of Two's hidden hand cards is picked (One cannot name it), then
    // transferred into One's hand (now One can), returned, and shuffled into
    // a deck with another card. Both references share the pick: one card.
    let hand = Zone::Hand(Player::Two);
    let deck = Zone::Deck(Deck::base(1));
    let position = Position::new(
        vec![base_card("B001").unwrap(), base_card("B002").unwrap()],
        &[(CardId(0), hand), (CardId(1), deck)],
        Player::Two,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    let context = Context::new(Player::Two);
    let picked = game.capture(CardId(0));
    assert!(!picked.evidence.known(Player::One));
    let mut result = HeldReturn::UNIT;
    for (source, destination) in [
        (hand, Zone::Hand(Player::One)),
        (Zone::Hand(Player::One), deck),
    ] {
        game.knowledge.as_mut().unwrap().moving = picked.evidence.pick();
        let outcome = game
            .commit(
                Operation::Move {
                    verb: Verb::Transfer,
                    target: Target {
                        card: CardId(0),
                        physical: source,
                        logical: LogicalZone::Physical(source),
                    },
                    destination,
                },
                context,
                false,
                &mut Vec::new(),
            )
            .unwrap();
        let moved = game.knowledge.as_mut().unwrap().moved.take();
        if source == hand {
            let evidence = game.arrived(CardId(0), moved) | picked.evidence;
            assert!(evidence.known(Player::One));
            result = Captured::new(outcome, evidence).returned();
        }
    }
    game.commit(
        Operation::Shuffle {
            deck: Deck::base(1),
        },
        context,
        false,
        &mut Vec::new(),
    )
    .unwrap();
    game.frames.push(Frame::Program {
        source: Source::Effect {
            card: CardId(1),
            effect: 0,
        },
        pc: 0,
        context: Context {
            source_card: Some(CardId(1)),
            source_evidence: Evidence::PUBLIC,
            ..context
        },
        last: result,
        retained: Some(picked),
        chosen: SelectionParameter::None,
        boxed: BoxedValues::default(),
    });
    super::tests::pause(&mut game);
    for seed in 0..32 {
        let world = game.determinize(Player::One, seed).unwrap();
        let Frame::Program { retained, last, .. } = &world.frames[0] else {
            panic!("program");
        };
        assert_eq!(retained.unwrap().inner, CardId(0), "seed {seed}");
        assert_eq!(last.card().unwrap().inner, CardId(0));
    }
}

#[test]
fn a_sampled_world_can_be_sampled_again_by_either_observer() {
    // After One's private draw, the completed result holds a pick. The world
    // either observer samples must keep it followable by both observers.
    let deck = Zone::Deck(Deck::base(1));
    let position = Position::new(
        ["B001", "B002", "B003"]
            .into_iter()
            .map(|code| base_card(code).unwrap())
            .collect(),
        &[(CardId(0), deck), (CardId(1), deck), (CardId(2), deck)],
        Player::One,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        Context::new(Player::One),
    )
    .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Complete(_)
    ));
    let drawn = game.completed.card().unwrap();
    assert!(drawn.evidence.known(Player::One) && !drawn.evidence.known(Player::Two));
    for viewer in Player::ALL {
        for seed in 0..8 {
            let world = game.determinize(viewer, seed).unwrap();
            for again in Player::ALL {
                let nested = world.determinize(again, seed + 100).unwrap();
                let card = nested.completed.card().unwrap().inner;
                assert_eq!(nested.position.locate(card), Some(Zone::Hand(Player::One)));
                for observer in Player::ALL {
                    assert!(
                        nested
                            .tracker(observer)
                            .unwrap()
                            .check_truth(&nested.position)
                    );
                }
            }
        }
    }
}
