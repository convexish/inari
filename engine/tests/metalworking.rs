use inno_engine::*;

mod common;
use common::*;

fn authority(f: &mut Fixture, color: Color, zone: Zone) -> CardId {
    let card = f.tech(color, 1, zone, vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    card
}

fn every_boundary(mut game: Game) {
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = settle(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..200 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(settle(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("finite Metalworking did not finish");
}

#[test]
fn metalworking_draw_reveal_score_repeat_matches_in_both_kernels() {
    let mut f = Fixture::default();
    let code = f.metalworking(Zone::Board(A, Color::Red));
    let whiff = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let second = authority(&mut f, Color::Green, Zone::Deck(Deck::base(1)));
    let first = authority(&mut f, Color::Red, Zone::Deck(Deck::base(1)));
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    base.self_execute(code, Context::new(A)).unwrap();
    figures.self_execute(code, Context::new(A)).unwrap();
    every_boundary(base.clone());
    let mut base_events = Vec::new();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut base, &mut base_events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        settle(&mut figures, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(base.position(), figures.position());
    assert_eq!(base_events, events);
    assert_eq!(events.len(), 8);
    for (offset, card) in [(0, first), (3, second), (6, whiff)] {
        assert_eq!(moved(&events[offset]).card, card);
        assert_eq!(moved(&events[offset]).verb, None);
        assert_eq!(
            events[offset + 1],
            Event::Revealed {
                card,
                source: Zone::Hand(A),
                executor: A,
                inside_karma: false
            }
        );
        if card != whiff {
            assert_eq!(moved(&events[offset + 2]).card, card);
            assert_eq!(moved(&events[offset + 2]).verb, Some(Verb::Score));
        }
    }
    assert_eq!(base.position().cards(Zone::Hand(A)), &[whiff]);
    assert_eq!(base.position().cards(Zone::Score(A)), &[first, second]);
}

#[test]
fn shen_replaces_scoring_but_not_metalworkings_repetition_even_if_splay_is_impossible() {
    for red_size in [1, 2] {
        let mut f = Fixture::default();
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        if red_size == 2 {
            f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        }
        let code = f.metalworking(Zone::Board(A, Color::Red));
        let whiff = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        // Exercise both an absent stack and an already-right stack on repeat.
        let second = authority(
            &mut f,
            if red_size == 2 {
                Color::Red
            } else {
                Color::Purple
            },
            Zone::Deck(Deck::base(1)),
        );
        let first = authority(&mut f, Color::Red, Zone::Deck(Deck::base(1)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        every_boundary(game.clone());
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().cards(Zone::Hand(A)),
            &[first, second, whiff]
        );
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        assert_eq!(
            game.position().splay(A, Color::Red),
            if red_size == 2 {
                Splay::Right
            } else {
                Splay::None
            }
        );
        assert_eq!(game.position().score(A), red_size + 1); // visible cards, not hand
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Revealed { .. }))
                .count(),
            3
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Moved(m) if m.verb.is_none()))
                .count(),
            3
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Score)))
        );
        if red_size == 2 {
            assert_eq!(
                events[2],
                Event::Splayed {
                    player: A,
                    color: Color::Red,
                    from: Splay::None,
                    to: Splay::Right,
                    executor: A,
                    inside_karma: true
                }
            );
        }
    }
}

#[test]
fn shen_replaces_a_left_splay_and_does_not_satisfy_the_original_operation() {
    let mut f = Fixture::default();
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut events = Vec::new();
    for attempt in 0..2 {
        game.start(
            Operation::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Left,
            },
            Context::new(A),
        )
        .unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Replaced,
                card: None,
                value: None,
                short: false,
            }))
        );
        assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
        assert_eq!(events.len(), 1); // second replacement is itself inapplicable
        assert_eq!(
            game.position().visible_card_count(A),
            3,
            "attempt {attempt}"
        );
    }
}

#[test]
fn shens_continuous_points_disappear_when_covered_and_figures_still_score() {
    let mut f = Fixture::default();
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let figure = f.figure(Figure::Michelangelo, Zone::Hand(A));
    let covering = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().score(A), 2);
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(figure, Zone::Hand(A)),
            destination: Zone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert_eq!(game.position().score(A), 6);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Score));
    game.start(
        meld(covering, Zone::Hand(A), A, Color::Green),
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().score(A), 4);
}

#[test]
fn reveal_is_not_a_draw_or_move_and_does_not_grant_zone_inspection() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let card = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let hidden = target(card, Zone::Hand(B));
    let position = f.position();
    let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    game.start(Operation::Reveal { target: hidden }, Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(card),
            value: Some(1),
            short: false,
        }))
    );
    assert_eq!(game.position(), &position);
    assert_eq!(
        events,
        [Event::Revealed {
            card,
            source: Zone::Hand(B),
            executor: A,
            inside_karma: false
        }]
    );
    // Historical identity evidence belongs to the tracker; live inspection of
    // the opponent's hand does not become available from one reveal.
    assert!(matches!(
        game.position().descriptor(hidden, A),
        TargetDescriptor::Hidden { .. }
    ));
}

#[test]
fn repeat_retains_karma_suppression_in_foreign_self_execution() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.metalworking(Zone::Board(B, Color::Red));
    let whiff = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let first = authority(&mut f, Color::Red, Zone::Deck(Deck::base(1)));
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Score(A)), &[first]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[whiff]);
    assert_eq!(events.len(), 6);
    for event in &events[..5] {
        assert!(match event {
            Event::Moved(m) => m.executor == A && m.inside_karma,
            Event::Revealed {
                executor,
                inside_karma,
                ..
            } => *executor == A && *inside_karma,
            _ => false,
        });
    }
}

#[test]
fn revealing_a_hidden_hand_card_neither_names_nor_relocates_a_pending_meld() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(
        Color::Red,
        1,
        Zone::Board(B, Color::Red),
        vec![Instruction::Reveal {
            from: Selector::Hand(Who::Opponent),
            optional: false,
            all: false,
            unless_proven: None,
            keep: None,
            by: None,
        }],
    );
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let other = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
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
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(
        menu.options[0],
        Alternative::Reveal {
            target: TargetDescriptor::Hidden {
                source: Zone::Hand(B),
                slot: 0,
                back: Deck {
                    set: CardSet::Base,
                    age: 2
                },
            }
        }
    );
    assert_eq!(game.pending_melds(A)[0].card, CardView::Back(Deck::base(2)));
    game.choose(0).unwrap();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[incoming, other]);
    // A revealed a blind hand slot. It cannot tell whether that card is the
    // one B is melding, even though here it is.
    assert_eq!(game.pending_melds(A)[0].card, CardView::Back(Deck::base(2)));
    assert_eq!(
        events[0],
        Event::Revealed {
            card: incoming,
            source: Zone::Hand(B),
            executor: A,
            inside_karma: true
        }
    );
    let mut fork = game.clone();
    let mut suffix = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        settle(&mut fork, &mut suffix)
    );
    assert_eq!(game, fork);
    assert_eq!(suffix, events[1..]);
    assert_eq!(game.position().top(B, Color::Blue), Some(incoming));
    assert_eq!(game.position().cards(Zone::Hand(B)), &[other]);
}

#[test]
fn an_exact_repeat_loop_is_drawn_in_base_and_resumable_budgets_agree() {
    // A bare Repeat recreates the whole game exactly, so the second Repeat ends the
    // game as a mandatory loop in every mode, not only Figures. Small resumable budgets
    // reach the same draw as one large one.
    let mut f = Fixture::default();
    let code = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![Instruction::Repeat { keep: 0 }],
    );
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let mut fork = game.clone();
    let mut steps = 0;
    let status = loop {
        steps += 1;
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => continue,
            status => break status,
        }
    };
    assert_eq!(status, Status::Draw(DrawReason::Loop));
    assert!(steps <= 8, "drawn after {steps} steps");
    assert_eq!(
        fork.advance(100, &mut Vec::new()).unwrap(),
        Status::Draw(DrawReason::Loop)
    );
    assert_eq!(game, fork);
    assert!(events.is_empty());
}
