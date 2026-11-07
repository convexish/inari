use inno_engine::*;

mod common;
use common::*;

#[test]
fn kalashnikov_scores_foreign_red_then_resumes_original_meld_once() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    let red = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let incoming = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    assert!(events.is_empty());
    let mut cloned = game.clone();
    assert_eq!(game.choose(9), Err(Error::InvalidChoice));
    assert_eq!(game, cloned);
    game.choose(0).unwrap();
    cloned.choose(0).unwrap();
    let result = settle(&mut game, &mut events);
    let mut cloned_events = Vec::new();
    assert_eq!(settle(&mut cloned, &mut cloned_events), result);
    assert_eq!(game, cloned);
    assert_eq!(events, cloned_events);
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, red);
    assert_eq!(moved(&events[0]).destination, Zone::Score(A));
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(moved(&events[1]).card, incoming);
    assert!(!moved(&events[1]).inside_karma);
    assert!(game.position().check_conservation());
}

#[test]
fn kalashnikov_self_execution_keeps_owner_controller_and_suppression_across_choices() {
    let mut f = Fixture::default();
    let kalashnikov = f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    // Synthetic non-demand program exercises the same choice boundary as
    // Fission's foreign self-execution without claiming a complete Base port.
    let red = f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![
            build::verb(
                Verb::Return,
                build::filtered(
                    &Selector::Tops(Who::Either),
                    &[Filter::Lacks(Test::Is(Reg::Source))],
                ),
            )
            .to(Destination::Deck),
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
        ],
    );
    f.cards[usize::from(red.0)].effects.insert(
        0,
        Effect {
            kind: EffectKind::Demand,
            instructions: vec![Instruction::Win(WinRule::Executor)],
        },
    );
    let spare = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    let owner_hand = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let incoming = f.tech(Color::Yellow, 4, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(B);
    context.controller = A;
    game.start(meld(incoming, Zone::Hand(B), B, Color::Yellow), context)
        .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    let child_menu = decision(&mut game, &mut events);
    assert_eq!(child_menu.player, A);
    assert_eq!(child_menu.options.len(), 2);
    assert!(child_menu.options.iter().all(|option| !matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == red)));
    choose_card(&mut game, &mut events, kalashnikov);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        vec![kalashnikov, owner_hand, incoming]
    );
    assert_eq!(moved(&events[0]).executor, A);
    assert!(moved(&events[1]).inside_karma); // Suppression survives source removal.
    assert!(!moved(&events[2]).inside_karma);
    assert_eq!(game.position().top(B, Color::Green), Some(spare));
    assert_eq!(game.position().top(A, Color::Blue), Some(owner_hand));
}

#[test]
fn kalashnikov_meld_follows_original_target_back_out_of_the_deck() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![
            build::verb(
                Verb::Return,
                build::filtered(
                    &Selector::TopOf(Who::Either, Color::Green),
                    &[Filter::Lacks(Test::Is(Reg::Source))],
                ),
            )
            .to(Destination::Deck),
        ],
    );
    let incoming = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Board(B, Color::Green), B, Color::Green),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(incoming),
            value: Some(2),
            short: false,
        }))
    );
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[1]).source, Zone::Deck(Deck::base(2)));
    assert!(game.position().cards(Zone::Deck(Deck::base(2))).is_empty());
    assert_eq!(game.position().top(B, Color::Green), Some(incoming));
}

#[test]
fn hidden_meld_follows_the_exact_card_below_a_deck_without_revealing_it() {
    let make = |choose_green| {
        let mut f = Fixture::default();
        f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
        f.tech(
            Color::Red,
            9,
            Zone::Board(B, Color::Red),
            vec![build::verb(Verb::Return, Selector::Hand(Who::Opponent)).to(Destination::Deck)],
        );
        // Identical catalogs, different hidden placements and identities.
        let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let green = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
        let (incoming, other, color) = if choose_green {
            f.placements[usize::from(blue.0)].1 = Zone::Deck(Deck::base(2));
            f.placements[usize::from(green.0)].1 = Zone::Hand(B);
            (green, blue, Color::Green)
        } else {
            (blue, green, Color::Blue)
        };
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(meld(incoming, Zone::Hand(B), B, color), Context::new(B))
            .unwrap();
        (game, incoming, other, color)
    };
    let hidden = vec![PendingMeld {
        player: B,
        card: CardView::Back(Deck::base(2)),
    }];
    let mut menus = Vec::new();
    for choose_green in [false, true] {
        let (mut game, incoming, other, color) = make(choose_green);
        let mut events = Vec::new();
        menus.push(decision(&mut game, &mut events));
        assert!(events.is_empty());
        assert_eq!(game.pending_melds(A), hidden);
        assert_eq!(
            game.pending_melds(B),
            vec![PendingMeld {
                player: B,
                card: CardView::Known(incoming),
            }]
        );
        // Selecting a meld did not remove the card from its real hand.
        assert_eq!(game.position().cards(Zone::Hand(B)), &[incoming]);
        game.choose(1).unwrap();
        advance_to_events(&mut game, &mut events, 1);
        assert_eq!(moved(&events[0]).card, incoming);
        assert_eq!(moved(&events[0]).destination, Zone::Deck(Deck::base(2)));
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(2))),
            &[incoming, other]
        );
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(game.pending_melds(A), hidden);
        assert_eq!(game.pending_melds(B)[0].card, CardView::Known(incoming));

        let mut cloned = game.clone();
        let mut clone_events = Vec::new();
        let result = settle(&mut game, &mut events);
        assert_eq!(
            result,
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Performed,
                card: Some(incoming),
                value: Some(2),
                short: false,
            }))
        );
        assert_eq!(settle(&mut cloned, &mut clone_events), result);
        assert_eq!(game, cloned);
        assert_eq!(events[1..], clone_events);
        assert_eq!(moved(&events[1]).source, Zone::Deck(Deck::base(2)));
        assert_eq!(moved(&events[1]).destination, Zone::Board(B, color));
        assert!(!moved(&events[1]).inside_karma);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[other]);
        assert_eq!(game.position().top(B, color), Some(incoming));
        assert!(game.pending_melds(A).is_empty());
        assert!(matches!(
            game.position().descriptor(target(incoming, Zone::Board(B, color)), A),
            TargetDescriptor::Known { card, .. } if card == incoming
        ));
        assert!(game.position().check_conservation());
    }
    assert_eq!(menus[0], menus[1]);
}

#[test]
fn junk_descriptors_do_not_grant_inspection_of_card_fronts() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Blue, 2, Zone::Junk, vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    for viewer in Player::ALL {
        let expected = TargetDescriptor::Hidden {
            source: Zone::Junk,
            slot: 0,
            back: Deck {
                set: CardSet::Base,
                age: 2,
            },
        };
        assert_eq!(
            position
                .clone()
                .with_expansions(Expansions::BASE)
                .descriptor(target(card, Zone::Junk), viewer),
            expected
        );
        assert_eq!(
            position.descriptor(target(card, Zone::Junk), viewer),
            expected
        );
    }
}

#[test]
fn interrupted_meld_follows_selection_through_each_supported_zone() {
    for (verb, to, zone) in [
        (
            Verb::Transfer,
            Destination::Hand(Who::Executor),
            Zone::Hand(A),
        ),
        (
            Verb::Score,
            Destination::Score(Who::Executor),
            Zone::Score(A),
        ),
        (
            Verb::Meld,
            Destination::Board(Who::Executor),
            Zone::Board(A, Color::Blue),
        ),
        (Verb::Return, Destination::Deck, Zone::Deck(Deck::base(2))),
        (Verb::Junk, Destination::Junk, Zone::Junk),
    ] {
        let mut f = Fixture::default();
        f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
        f.tech(
            Color::Red,
            9,
            Zone::Board(B, Color::Red),
            vec![build::verb(verb, Selector::Hand(Who::Opponent)).to(to)],
        );
        let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        // Another hidden age-2 card, so A cannot name the meld by elimination.
        f.tech(Color::Green, 2, Zone::Score(B), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.start(
            meld(incoming, Zone::Hand(B), B, Color::Blue),
            Context::new(B),
        )
        .unwrap();
        let mut events = Vec::new();
        decision(&mut game, &mut events);
        game.choose(1).unwrap();
        advance_to_events(&mut game, &mut events, 1);
        assert_eq!(game.position().cards(zone), &[incoming]);
        // A hidden return or junk does not reveal the selected front to the
        // reacting player. Its original owner still remembers the identity.
        assert_eq!(
            game.pending_melds(A)[0].card,
            if matches!(zone, Zone::Deck(_) | Zone::Junk) {
                CardView::Back(Deck::base(2))
            } else {
                CardView::Known(incoming)
            }
        );
        assert_eq!(game.pending_melds(B)[0].card, CardView::Known(incoming));
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(events.len(), 2);
        assert_eq!(moved(&events[1]).card, incoming);
        assert_eq!(moved(&events[1]).source, zone);
        assert_eq!(game.position().top(B, Color::Blue), Some(incoming));
        assert!(game.position().cards(zone).is_empty());
        assert!(game.position().check_conservation());
    }
}

#[test]
fn interrupted_meld_extracts_selected_card_even_when_it_became_covered() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![
            build::verb(Verb::Meld, Selector::Hand(Who::Opponent))
                .to(Destination::Board(Who::Executor)),
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
        ],
    );
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let cover = f.tech(Color::Blue, 3, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    advance_to_events(&mut game, &mut events, 2);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        &[incoming, cover]
    );
    assert_eq!(game.pending_melds(A)[0].card, CardView::Known(incoming));
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 3);
    assert_eq!(moved(&events[2]).source, Zone::Board(A, Color::Blue));
    assert_eq!(moved(&events[2]).card, incoming);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Blue)), &[cover]);
    assert_eq!(game.position().top(B, Color::Blue), Some(incoming));
    assert!(game.position().check_conservation());
}

#[test]
fn pending_meld_remembers_identity_learned_during_the_reaction() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![
            build::verb(Verb::Score, Selector::Hand(Who::Opponent))
                .to(Destination::Score(Who::Executor)),
            build::verb(Verb::Return, build::SCORE).to(Destination::Deck),
        ],
    );
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    assert!(matches!(game.pending_melds(A)[0].card, CardView::Back(_)));
    game.choose(1).unwrap();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(game.position().cards(Zone::Score(A)), &[incoming]);
    assert_eq!(game.pending_melds(A)[0].card, CardView::Known(incoming));
    advance_to_events(&mut game, &mut events, 2);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[incoming]
    );
    assert_eq!(game.pending_melds(A)[0].card, CardView::Known(incoming));
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 3);
    assert_eq!(game.position().top(B, Color::Blue), Some(incoming));
}

#[test]
fn stale_meld_selection_is_rejected_before_a_first_reaction() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(Color::Red, 9, Zone::Board(B, Color::Red), vec![]);
    let incoming = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert!(events.is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[incoming]
    );
}

#[test]
fn victory_inside_kalashnikov_unwinds_the_suspended_meld() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![Instruction::Win(WinRule::Executor)],
    );
    let incoming = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(B)), &[incoming]);
}

#[test]
fn michelangelo_alias_can_be_selected_and_moved_exactly_once() {
    let mut f = Fixture::default();
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let hand = f.tech(Color::Blue, 3, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 2, Zone::Score(A), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.score(A), 5);
    assert_eq!(
        position.clone().with_expansions(Expansions::BASE).score(A),
        2
    );
    let selected = position
        .targets(LogicalZone::Score(A))
        .find(|t| t.card == hand)
        .unwrap();
    assert_eq!(selected.physical, Zone::Hand(A));
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: selected,
            destination: Zone::Score(B),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).source, Zone::Hand(A));
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().score(A), 2);
    assert_eq!(game.position().score(B), 3);
    assert!(game.position().check_conservation());
}

#[test]
fn michelangelo_bicycle_takes_only_the_physical_score_pile_into_hand() {
    for reverse in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
        let hand = [
            f.tech(Color::Blue, 2, Zone::Hand(A), vec![]),
            f.tech(Color::Green, 3, Zone::Hand(A), vec![]),
        ];
        let score = [
            f.tech(Color::Red, 4, Zone::Score(A), vec![]),
            f.tech(Color::Purple, 5, Zone::Score(A), vec![]),
        ];
        // Both group orientations select the same simultaneous physical exchange.
        let (left, right) = if reverse {
            (build::SCORE, build::HAND)
        } else {
            (build::HAND, build::SCORE)
        };
        let bicycle = f.tech(
            Color::Green,
            8,
            Zone::Board(A, Color::Green),
            vec![Instruction::Exchange(ExchangeStep {
                left,
                right,
                counts: &[None, None],
                optional: false,
            })],
        );
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(bicycle, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let mut cloned = game.clone();
        let mut cloned_events = Vec::new();
        let result = settle(&mut cloned, &mut cloned_events);
        let mut completed = false;
        for _ in 0..100 {
            let status = game.advance(1, &mut events).unwrap();
            assert!(game.position().check_conservation());
            // No boundary exposes a half exchange or a copied hand card.
            assert!(events.is_empty() || events.len() == 1);
            if events.is_empty() {
                assert_eq!(game.position().cards(Zone::Hand(A)), &hand);
                assert_eq!(game.position().cards(Zone::Score(A)), &score);
            } else {
                assert_eq!(
                    game.position().cards(Zone::Hand(A)),
                    &[hand[0], hand[1], score[0], score[1]]
                );
                assert!(game.position().cards(Zone::Score(A)).is_empty());
                assert_eq!(exchanged(&events[0]).len(), 2);
            }
            if status != Status::WorkLimit {
                assert_eq!(status, result);
                completed = true;
                break;
            }
        }
        assert!(completed);
        assert_eq!(game, cloned);
        assert_eq!(events, cloned_events);
        assert_eq!(
            game.position().cards(Zone::Hand(A)),
            &[hand[0], hand[1], score[0], score[1]]
        );
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(
            exchanged(&events[0])
                .iter()
                .map(|m| m.card)
                .collect::<Vec<_>>(),
            score
        );
        assert!(
            exchanged(&events[0])
                .iter()
                .all(|m| m.source == Zone::Score(A) && m.destination == Zone::Hand(A))
        );
        assert_eq!(game.position().score(A), 14);
    }
}

#[test]
fn michelangelo_does_not_freeze_an_ordinary_score_from_hand() {
    let mut f = Fixture::default();
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let card = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().score(A), 2);
    let target = game
        .position()
        .targets(LogicalZone::Hand(A))
        .next()
        .unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target,
            destination: Zone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(card),
            value: Some(2),
            short: false,
        }))
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[card]);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().score(A), 2);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Score));
}

#[test]
fn meiji_hand_exchange_freezes_cards_in_both_hands_only_during_his_window() {
    for (owner, regular_actions) in [(A, 0), (A, 2), (B, 0)] {
        for reverse in [false, true] {
            let mut f = Fixture::default();
            f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
            let mine = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
            let theirs = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.begin_turn(owner).unwrap();
            for _ in 0..regular_actions {
                game.begin_action(ActionKind::Regular).unwrap();
            }
            game.begin_action(ActionKind::Free).unwrap();
            let (left, right) = if reverse {
                (LogicalZone::Hand(B), LogicalZone::Hand(A))
            } else {
                (LogicalZone::Hand(A), LogicalZone::Hand(B))
            };
            game.start(Operation::ExchangeAll { left, right }, Context::new(A))
                .unwrap();
            let mut events = Vec::new();
            assert!(matches!(
                settle(&mut game, &mut events),
                Status::Complete(_)
            ));
            if owner == A && regular_actions < 2 {
                assert!(game.position().cards(Zone::Hand(A)).is_empty());
                assert_eq!(game.position().cards(Zone::Hand(B)), &[theirs, mine]);
                assert_eq!(events.len(), 1);
                assert_eq!(exchanged(&events[0]).len(), 1);
                assert_eq!(exchanged(&events[0])[0].card, mine);
            } else {
                assert_eq!(game.position().cards(Zone::Hand(A)), &[theirs]);
                assert_eq!(game.position().cards(Zone::Hand(B)), &[mine]);
                assert_eq!(events.len(), 1);
                assert_eq!(exchanged(&events[0]).len(), 2);
            }
            assert!(matches!(
                events[0],
                Event::Exchanged {
                    executor: A,
                    inside_karma: false,
                    ..
                }
            ));
            assert!(game.position().check_conservation());
        }
    }
}

#[test]
fn base_exchange_swaps_physical_piles_in_both_kernels() {
    let mut f = Fixture::default();
    let hand = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let score = f.tech(Color::Green, 3, Zone::Score(A), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    let operation = Operation::ExchangeAll {
        left: LogicalZone::Hand(A),
        right: LogicalZone::Score(A),
    };
    base.start(operation, Context::new(A)).unwrap();
    figures.start(operation, Context::new(A)).unwrap();
    let mut base_events = Vec::new();
    let mut figures_events = Vec::new();
    assert_eq!(
        settle(&mut base, &mut base_events),
        settle(&mut figures, &mut figures_events)
    );
    assert_eq!(base.position(), figures.position());
    assert_eq!(base_events, figures_events);
    assert_eq!(
        base_events,
        vec![Event::Exchanged {
            executor: A,
            inside_karma: false,
            movements: vec![
                ExchangeMovement {
                    card: hand,
                    source: Zone::Hand(A),
                    destination: Zone::Score(A)
                },
                ExchangeMovement {
                    card: score,
                    source: Zone::Score(A),
                    destination: Zone::Hand(A)
                },
            ],
        }]
    );
    assert_eq!(base.position().cards(Zone::Hand(A)), &[score]);
    assert_eq!(base.position().cards(Zone::Score(A)), &[hand]);
}

#[test]
fn exchanging_completely_overlapping_piles_moves_nothing() {
    let mut f = Fixture::default();
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let card = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExchangeAll {
            left: LogicalZone::Hand(A),
            right: LogicalZone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn meiji_borrowed_hand_grants_inspection_without_changing_physical_ownership() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let borrowed = f.tech(Color::Blue, 4, Zone::Hand(B), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert!(
        position
            .clone()
            .with_expansions(Expansions::BASE)
            .targets(LogicalZone::Hand(A))
            .next()
            .is_none()
    );
    assert!(matches!(
        position
            .clone()
            .with_expansions(Expansions::BASE)
            .descriptor(target(borrowed, Zone::Hand(B)), A),
        TargetDescriptor::Hidden { .. }
    ));
    let selected = position.targets(LogicalZone::Hand(A)).next().unwrap();
    assert_eq!(selected.card, borrowed);
    assert_eq!(selected.physical, Zone::Hand(B));
    assert!(
        matches!(position.descriptor(selected, A), TargetDescriptor::Known { card, .. } if card == borrowed)
    );
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Meld,
            target: selected,
            destination: Zone::Board(A, Color::Blue),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert_eq!(moved(&events[0]).source, Zone::Hand(B));
    assert_eq!(game.position().top(A, Color::Blue), Some(borrowed));
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
}

#[test]
fn meiji_access_follows_turn_owner_and_regular_action_ordinal() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let borrowed = f.tech(Color::Blue, 4, Zone::Hand(B), vec![]);
    let mut position = f.position().with_expansions(Expansions::FIGURES);
    let selected = target(borrowed, Zone::Hand(B));
    assert_eq!(position.targets(LogicalZone::Hand(A)).count(), 1);
    assert!(matches!(
        position.descriptor(selected, A),
        TargetDescriptor::Known { .. }
    ));
    position.begin_action(ActionKind::Free).unwrap();
    position.begin_action(ActionKind::Regular).unwrap();
    position.begin_action(ActionKind::Free).unwrap();
    assert!(position.meiji_active(A));
    position.begin_action(ActionKind::Regular).unwrap();
    assert!(!position.meiji_active(A));
    assert!(position.targets(LogicalZone::Hand(A)).next().is_none());
    assert!(matches!(
        position.descriptor(selected, A),
        TargetDescriptor::Hidden { .. }
    ));
    position.begin_action(ActionKind::Free).unwrap();
    assert!(!position.meiji_active(A));
    position.begin_turn(B);
    assert!(!position.meiji_active(A));
    assert!(position.targets(LogicalZone::Hand(A)).next().is_none());
    position.begin_turn(A);
    assert!(position.meiji_active(A));
    assert_eq!(position.cards(Zone::Hand(B)), &[borrowed]);
}

#[test]
fn selected_borrowed_meld_still_occurs_when_meiji_is_removed() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Board(B, Color::Purple));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![
            build::verb(
                Verb::Return,
                build::filtered(
                    &Selector::TopOf(Who::Either, Color::Purple),
                    &[Filter::Lacks(Test::Is(Reg::Source))],
                ),
            )
            .to(Destination::Deck),
        ],
    );
    let borrowed = f.tech(Color::Blue, 4, Zone::Hand(A), vec![]);
    let mut position = f.position().with_expansions(Expansions::FIGURES);
    position.begin_turn(B);
    let selected = position.targets(LogicalZone::Hand(B)).next().unwrap();
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Meld,
            target: selected,
            destination: Zone::Board(B, Color::Blue),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    assert_eq!(game.pending_melds(B)[0].card, CardView::Known(borrowed));
    game.choose(1).unwrap();
    advance_to_events(&mut game, &mut events, 1);
    assert!(!game.position().meiji_active(B));
    assert!(
        game.position()
            .targets(LogicalZone::Hand(B))
            .next()
            .is_none()
    );
    // The live grant is gone, but the chosen card's learned identity remains.
    assert_eq!(game.pending_melds(B)[0].card, CardView::Known(borrowed));
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(borrowed),
            value: Some(4),
            short: false,
        }))
    );
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, meiji);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().top(B, Color::Blue), Some(borrowed));
}

#[test]
fn meiji_reveals_a_pending_meld_when_uncovered_during_the_reaction() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let cover = f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![
            build::verb(
                Verb::Return,
                build::filtered(
                    &Selector::TopOf(Who::Executor, Color::Purple),
                    &[Filter::Lacks(Test::Is(Reg::Source))],
                ),
            )
            .to(Destination::Deck),
        ],
    );
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    // An effect meld by B during A's turn; Meiji is initially covered.
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    assert!(!game.position().meiji_active(A));
    assert!(matches!(game.pending_melds(A)[0].card, CardView::Back(_)));
    game.choose(1).unwrap();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(moved(&events[0]).card, cover);
    assert!(game.position().meiji_active(A));
    assert_eq!(game.position().cards(Zone::Hand(B)), &[incoming]);
    assert_eq!(game.pending_melds(A)[0].card, CardView::Known(incoming));
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().top(B, Color::Blue), Some(incoming));
}

#[test]
fn meiji_does_not_feed_michelangelo_another_continuous_modifiers_additions() {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let own = f.tech(Color::Blue, 3, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 8, Zone::Hand(B), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.targets(LogicalZone::Hand(A)).count(), 2);
    let mut score = position.targets(LogicalZone::Score(A));
    assert_eq!(score.next().map(|target| target.card), Some(own));
    assert!(score.next().is_none());
    assert_eq!(position.score(A), 3);
}

#[test]
fn inactive_meiji_does_not_disclose_an_opponents_hidden_hand() {
    let make = |color| {
        let mut f = Fixture::default();
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        let card = f.tech(color, 4, Zone::Hand(B), vec![]);
        let mut position = f.position().with_expansions(Expansions::FIGURES);
        position.begin_turn(B);
        position.descriptor(target(card, Zone::Hand(B)), A)
    };
    assert_eq!(make(Color::Blue), make(Color::Green));
    assert!(matches!(make(Color::Blue), TargetDescriptor::Hidden { .. }));
}

fn meiji_winning_position(unique: bool) -> (Position, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    for (color, value) in [
        (Color::Red, 1),
        (Color::Blue, 2),
        (Color::Green, 3),
        (Color::Yellow, if unique { 4 } else { 3 }),
    ] {
        f.tech(color, value, Zone::Board(A, color), vec![]);
    }
    let incoming = f.tech(Color::Red, 10, Zone::Hand(A), vec![]);
    (f.position().with_expansions(Expansions::FIGURES), incoming)
}

#[test]
fn meiji_replaces_meld_only_with_five_distinct_top_values() {
    for unique in [true, false] {
        let (position, incoming) = meiji_winning_position(unique);
        let mut game = Game::new(position, Expansions::FIGURES).unwrap();
        game.start(
            meld(incoming, Zone::Hand(A), A, Color::Red),
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let result = settle(&mut game, &mut events);
        if unique {
            assert_eq!(result, Status::Winner(A));
            assert!(events.is_empty());
            assert_eq!(game.position().cards(Zone::Hand(A)), &[incoming]);
        } else {
            assert!(matches!(result, Status::Complete(_)));
            assert_eq!(events.len(), 1);
        }
    }
}

#[test]
fn jackie_scores_newly_exposed_figures_and_wins_before_original_victory() {
    let mut f = Fixture::default();
    let jackie = f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    let covered = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let top = f.figure(Figure::Nightingale, Zone::Board(B, Color::Yellow));
    f.tech(Color::Blue, 10, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ProposeWin {
            winner: B,
            reason: WinReason::Achievements,
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    let victory = game.victory().unwrap();
    assert_eq!(victory.reason, WinReason::Karma);
    assert_eq!(victory.source_card(A), Some(jackie));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        vec![top, covered]
    );
    assert!(events.iter().all(|e| moved(e).inside_karma)); // Nightingale cannot block.
    assert_eq!(game.position().top(A, Color::Red), Some(jackie));
    assert!(game.position().check_conservation());
}

#[test]
fn jackie_original_victory_survives_when_points_are_insufficient() {
    let mut f = Fixture::default();
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    f.tech(Color::Blue, 10, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ProposeWin {
            winner: B,
            reason: WinReason::Achievements,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(B));
    assert_eq!(game.victory().unwrap().reason, WinReason::Achievements);
}

#[test]
fn jackie_recomputes_points_after_scoring_his_own_michelangelo() {
    let mut f = Fixture::default();
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    let michelangelo = f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    f.tech(Color::Blue, 10, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 8, Zone::Score(B), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.score(A), 10);
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.start(
        Operation::ProposeWin {
            winner: B,
            reason: WinReason::Effect,
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(B));
    assert_eq!(moved(&events[0]).card, michelangelo);
    assert_eq!(game.position().score(A), 4);
}

#[test]
fn jackie_tie_counts_as_most_points() {
    let mut f = Fixture::default();
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ProposeWin {
            winner: B,
            reason: WinReason::Effect,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(A));
}

#[test]
fn jackie_pending_bulk_choice_clones_exactly() {
    let mut f = Fixture::default();
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Board(B, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ProposeWin {
            winner: B,
            reason: WinReason::Effect,
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, A);
    let mut clone = game.clone();
    game.choose(1).unwrap();
    clone.choose(1).unwrap();
    let mut cloned_events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        settle(&mut clone, &mut cloned_events)
    );
    assert_eq!(game, clone);
    assert_eq!(events, cloned_events);
}

#[test]
fn jackie_scores_figure_before_drawing_without_retriggering() {
    let mut f = Fixture::default();
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    let hand_figure = f.figure(Figure::Michelangelo, Zone::Hand(A));
    let drawn = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            set: None,
            value: 2,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        vec![hand_figure, drawn]
    );
    assert!(moved(&events[0]).inside_karma);
    assert!(!moved(&events[1]).inside_karma);
}

#[test]
fn meiji_victory_inside_replacement_does_not_trigger_opposing_jackie() {
    let (position, incoming) = meiji_winning_position(true);
    let mut cards = position.clone();
    // Rebuild the fixture to add Jackie while keeping setup conservation strict.
    let mut catalog = vec![];
    let mut placements = vec![];
    for (id, zone) in [
        (CardId(0), Zone::Board(A, Color::Purple)),
        (CardId(1), Zone::Board(A, Color::Red)),
        (CardId(2), Zone::Board(A, Color::Blue)),
        (CardId(3), Zone::Board(A, Color::Green)),
        (CardId(4), Zone::Board(A, Color::Yellow)),
        (incoming, Zone::Hand(A)),
    ] {
        catalog.push(cards.card(id).clone());
        placements.push((id, zone));
    }
    let mut f = Fixture {
        cards: catalog,
        placements,
    };
    f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
    cards = f.position().with_expansions(Expansions::FIGURES);
    let mut game = Game::new(cards, Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(A), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert!(events.is_empty());
}

#[test]
fn nightingale_protects_score_cards_but_does_not_intercept_tuck() {
    // Both Figures are yellow, so use one player's Michelangelo and the other
    // player's Nightingale only in their separate actual score-pile cases.
    // The stronger simultaneous-own-top combination is physically impossible.
    let mut f = Fixture::default();
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    let card = f.tech(Color::Blue, 4, Zone::Score(A), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    for verb in [Verb::Return, Verb::Tuck] {
        let selected = position.targets(LogicalZone::Score(A)).next().unwrap();
        let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Move {
                verb,
                target: selected,
                destination: if verb == Verb::Tuck {
                    Zone::Board(A, Color::Blue)
                } else {
                    Zone::Deck(Deck::base(4))
                },
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let result = settle(&mut game, &mut events);
        if verb == Verb::Return {
            assert_eq!(
                result,
                Status::Complete(ReturnValue::Operation(OperationOutcome {
                    disposition: Disposition::Replaced,
                    card: None,
                    value: None,
                    short: false,
                }))
            );
            assert!(events.is_empty());
            assert_eq!(game.position().cards(Zone::Score(A)), &[card]);
        } else {
            assert_eq!(events.len(), 1);
        }
    }
}

#[test]
fn replaced_operation_cannot_satisfy_callers_if_performed() {
    let mut f = Fixture::default();
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    f.tech(Color::Blue, 4, Zone::Score(A), vec![]);
    let program = f.tech(
        Color::Red,
        3,
        Zone::Board(A, Color::Red),
        vec![
            build::verb(Verb::Return, build::SCORE).to(Destination::Deck),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(program, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
}

#[test]
fn competing_karmas_are_chosen_by_turn_owner_then_run_as_figure_owner() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::Nightingale, Zone::Board(B, Color::Yellow));
    let red = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let incoming = f.tech(Color::Blue, 4, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Score(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    game.choose(0).unwrap(); // Kalashnikov. Nightingale must not also execute.
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    settle(&mut game, &mut events);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        vec![red, incoming]
    );
    assert_eq!(game.position().top(B, Color::Blue), Some(incoming));
}

#[test]
fn every_automatic_boundary_can_yield_and_resume_without_changing_results() {
    let mut f = Fixture::default();
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ProposeWin {
            winner: B,
            reason: WinReason::Effect,
        },
        Context::new(B),
    )
    .unwrap();
    let mut reference = game.clone();
    let mut expected = Vec::new();
    let result = settle(&mut reference, &mut expected);
    let mut actual = Vec::new();
    for _ in 0..100 {
        let snapshot = game.clone();
        assert_eq!(game.advance(0, &mut actual).unwrap(), Status::WorkLimit);
        assert_eq!(game, snapshot);
        let status = game.advance(1, &mut actual).unwrap();
        if status != Status::WorkLimit {
            assert_eq!(status, result);
            assert_eq!(actual, expected);
            assert_eq!(game, reference);
            return;
        }
    }
    panic!("bounded fixture did not terminate");
}

#[test]
fn no_figures_and_figures_kernels_agree_on_base_programs() {
    let mut f = Fixture::default();
    let program = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![
            build::verb(Verb::Return, build::HAND).to(Destination::Deck),
            build::if_(build::DID, 1),
            build::draw(2),
        ],
    );
    f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let drawn = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    let mut plain = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    plain.self_execute(program, Context::new(A)).unwrap();
    figures.self_execute(program, Context::new(A)).unwrap();
    let mut left = Vec::new();
    let mut right = Vec::new();
    assert_eq!(
        settle(&mut plain, &mut left),
        settle(&mut figures, &mut right)
    );
    assert_eq!(left, right);
    assert_eq!(plain.position(), figures.position());
    assert_eq!(plain.position().cards(Zone::Hand(A)), &[drawn]);
}

#[test]
fn no_figures_kernel_rejects_figures_positions() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    assert!(matches!(
        Game::new(f.position(), Expansions::BASE),
        Err(Error::InvalidSetup(_))
    ));
}

#[test]
fn setup_rejects_duplicate_or_missing_physical_cards() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let catalog = f.cards.clone();
    assert!(Position::new(catalog.clone(), &[], A).is_err());
    assert!(Position::new(catalog, &[(card, Zone::Hand(A)), (card, Zone::Hand(B))], A,).is_err());
}
