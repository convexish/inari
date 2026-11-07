use inno_engine::*;

mod common;
use common::*;

fn choice(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|o| match o {
            Alternative::Deck(deck) => *deck == Deck::figures(9),
            Alternative::NameFigure(name) => name.code == "F087",
            _ => false,
        })
        .unwrap_or(0)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(choice(&menu)).unwrap(),
            status => return status,
        }
    }
    panic!("Agriculture did not finish");
}

fn assert_every_boundary(mut game: Game) {
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..200 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        assert_eq!(game.advance(0, &mut events).unwrap(), Status::WorkLimit);
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => {
                let before = game.clone();
                assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
                assert_eq!(game, before);
                game.choose(choice(&menu)).unwrap();
            }
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                assert_eq!(events, expected_events);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("machine boundaries did not terminate");
}

#[test]
fn agriculture_returns_then_draws_then_scores_the_exact_card_in_both_kernels() {
    // The computed value starts at 3; skipping an empty age must not turn the
    // consequent into a failed return or score a different card already held.
    for draw_age in [3, 5] {
        let mut f = Fixture::default();
        let code = f.agriculture(Zone::Board(A, Color::Yellow));
        let returned = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
        let retained = f.tech(Color::Red, 4, Zone::Hand(A), vec![]);
        let deck_top = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
        let drawn = f.tech(
            Color::Purple,
            draw_age,
            Zone::Deck(Deck::base(draw_age)),
            vec![],
        );
        let position = f.position();
        let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
        let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
        base.self_execute(code, Context::new(A)).unwrap();
        figures.self_execute(code, Context::new(A)).unwrap();
        assert_every_boundary(base.clone());
        let mut base_events = Vec::new();
        let mut events = Vec::new();
        let menu = decision(&mut figures, &mut events);
        assert_eq!(menu.options.len(), 3);
        assert_eq!(menu.options.last(), Some(&Alternative::Decline));
        figures.choose(0).unwrap();
        advance_to_events(&mut figures, &mut events, 2);
        assert_eq!(figures.position().cards(Zone::Hand(A)), &[retained, drawn]);
        assert!(figures.position().cards(Zone::Score(A)).is_empty());
        assert_eq!(
            settle(&mut figures, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            finish(&mut base, &mut base_events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(base.position(), figures.position());
        assert_eq!(base_events, events);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events
                .iter()
                .map(|e| (moved(e).verb, moved(e).card))
                .collect::<Vec<_>>(),
            [
                (Some(Verb::Return), returned),
                (None, drawn),
                (Some(Verb::Score), drawn)
            ]
        );
        assert_eq!(
            figures.position().cards(Zone::Deck(Deck::base(2))),
            &[returned, deck_top]
        );
        assert_eq!(figures.position().cards(Zone::Hand(A)), &[retained]);
        assert_eq!(figures.position().cards(Zone::Score(A)), &[drawn]);
        assert!(figures.position().check_conservation());
    }
}

#[test]
fn a_single_optional_return_can_be_declined_and_an_empty_hand_does_nothing() {
    for has_card in [false, true] {
        let mut f = Fixture::default();
        let code = f.agriculture(Zone::Board(A, Color::Yellow));
        if has_card {
            f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
        }
        f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]);
        let position = f.position();
        let mut game = Game::new(position.clone(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        if has_card {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options.len(), 2);
            assert_eq!(menu.options[1], Alternative::Decline);
            game.choose(1).unwrap();
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position(), &position);
        assert!(events.is_empty());
    }
}

#[test]
fn homers_tuck_cannot_satisfy_agricultures_if_you_do() {
    let mut f = Fixture::default();
    let code = f.agriculture(Zone::Board(A, Color::Yellow));
    let homer = f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let returned = f.figure(Figure::EmperorMeiji, Zone::Hand(A));
    let not_drawn = f.tech(Color::Red, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    assert_every_boundary(game.clone());
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        moved(&events[0]),
        &Movement {
            verb: Some(Verb::Tuck),
            card: returned,
            source: Zone::Hand(A),
            destination: Zone::Board(A, Color::Purple),
            executor: A,
            inside_karma: true,
        }
    );
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Purple)),
        &[returned, homer]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(8))),
        &[not_drawn]
    );
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn homer_replaces_both_return_and_junk_with_an_unfulfilled_original_result() {
    for (verb, destination) in [
        (Verb::Return, Zone::Deck(Deck::figures(4))),
        (Verb::Junk, Zone::Junk),
    ] {
        let mut f = Fixture::default();
        f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
        let card = f.figure(Figure::Michelangelo, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Move {
                verb,
                target: target(card, Zone::Hand(A)),
                destination,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Replaced,
                card: None,
                value: None,
                short: false,
            }))
        );
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).verb, Some(Verb::Tuck));
        assert!(moved(&events[0]).inside_karma);
        assert_eq!(game.position().top(A, Color::Yellow), Some(card));
        assert!(game.position().cards(destination).is_empty());
        assert!(game.position().check_conservation());
    }
}

#[test]
fn homer_requires_the_owners_operation_on_a_figure_in_their_hand() {
    for (figure, source, executor, verb, destination) in [
        (
            false,
            Zone::Hand(A),
            A,
            Verb::Return,
            Zone::Deck(Deck::base(4)),
        ),
        (
            true,
            Zone::Hand(B),
            A,
            Verb::Return,
            Zone::Deck(Deck::figures(4)),
        ),
        (
            true,
            Zone::Hand(A),
            B,
            Verb::Return,
            Zone::Deck(Deck::figures(4)),
        ),
        (
            true,
            Zone::Score(A),
            A,
            Verb::Return,
            Zone::Deck(Deck::figures(4)),
        ),
        (true, Zone::Hand(A), A, Verb::Score, Zone::Score(A)),
    ] {
        let mut f = Fixture::default();
        f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
        let card = if figure {
            f.figure(Figure::Michelangelo, source)
        } else {
            f.tech(Color::Yellow, 4, source, vec![])
        };
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Move {
                verb,
                target: target(card, source),
                destination,
            },
            Context::new(executor),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Performed,
                card: Some(card),
                value: Some(4),
                short: false,
            }))
        );
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).verb, Some(verb));
        assert!(!moved(&events[0]).inside_karma);
        assert_eq!(game.position().cards(destination), &[card]);
    }
}

#[test]
fn trebek_cannot_substitute_its_child_transfer_for_agricultures_draw_result() {
    let mut f = Fixture::default();
    // Self-execution can use an off-board source. Trebek and Agriculture are
    // both yellow; do not invent simultaneous top cards of the same color.
    let code = f.agriculture(Zone::Junk);
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let searched = f.figure(Figure::Kalashnikov, Zone::Deck(Deck::figures(9)));
    let drawn = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    assert_every_boundary(game.clone());
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 6);
    assert_eq!(moved(&events[0]).card, returned);
    assert!(matches!(events[1], Event::FigureSearch { found: Some(card), .. } if card == searched));
    assert_eq!(moved(&events[2]).card, searched);
    assert!(moved(&events[2]).inside_karma);
    assert!(matches!(events[3], Event::Shuffled { deck, .. } if deck == Deck::figures(9)));
    assert_eq!(moved(&events[4]).card, drawn);
    assert_eq!(moved(&events[4]).verb, None);
    assert_eq!(moved(&events[5]).card, drawn);
    assert_eq!(moved(&events[5]).verb, Some(Verb::Score));
    assert!(!moved(&events[5]).inside_karma);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[searched]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[drawn]);
}

#[test]
fn agriculture_in_foreign_self_execution_keeps_executor_and_suppression() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let code = f.agriculture(Zone::Junk);
    let instructions = f.cards[usize::from(code.0)].effects[0].instructions.clone();
    // A synthetic red caller uses the real B004 program for this control probe.
    f.tech(Color::Red, 1, Zone::Board(B, Color::Red), instructions);
    let returned = f.figure(Figure::Michelangelo, Zone::Hand(A));
    let drawn = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, A);
    game.choose(1).unwrap(); // Foreign self-execution instead of scoring the top.
    assert_eq!(decision(&mut game, &mut events).player, A);
    game.choose(0).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 4);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, drawn, drawn, incoming]
    );
    assert_eq!(moved(&events[0]).verb, Some(Verb::Return));
    assert!(
        events[..3]
            .iter()
            .all(|e| moved(e).inside_karma && moved(e).executor == A)
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(4))),
        &[returned]
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[drawn]);
    assert!(!moved(&events[3]).inside_karma);
    assert!(game.position().check_conservation());
}

#[test]
fn result_consumers_do_not_substitute_a_card_or_reuse_an_older_result() {
    let mut f = Fixture::default();
    let code = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(Verb::Return, build::HAND).to(Destination::Deck),
            // The returned identity is no longer in hand. Another held card must
            // not be scored instead, and this inapplicable result clears the value.
            build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
            build::draw_age(Expr::Add(&build::LAST_VALUE, 1)),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let returned = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let retained = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, returned);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[retained]);
}

#[test]
fn an_empty_or_declined_optional_move_clears_the_previous_result() {
    for decline in [false, true] {
        let mut f = Fixture::default();
        let code = f.tech(
            Color::Purple,
            1,
            Zone::Board(A, Color::Purple),
            vec![
                build::draw(1),
                build::return_(if decline { build::HAND } else { build::SCORE }).optional(),
                build::draw_age(Expr::Add(&build::LAST_VALUE, 1)),
                build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
                build::if_(build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        if decline {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options[1], Alternative::Decline);
            game.choose(1).unwrap();
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), 1);
        assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
        assert!(game.position().cards(Zone::Score(A)).is_empty());
    }
}

#[test]
fn last_results_are_local_to_one_effect_not_the_whole_execution() {
    let mut f = Fixture::default();
    let code = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![build::draw(1)],
    );
    f.cards[usize::from(code.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![
            build::draw_age(Expr::Add(&build::LAST_VALUE, 1)),
            build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    });
    let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
}

#[test]
fn optional_menus_use_controller_visibility_not_private_continuation_identity() {
    let make = |reverse| {
        let mut f = Fixture::default();
        let code = f.agriculture(Zone::Board(A, Color::Yellow));
        let first = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
        let second = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        if reverse {
            f.placements
                .swap(usize::from(first.0), usize::from(second.0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        game.self_execute(code, context).unwrap();
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let mut events = Vec::new();
    let menu = decision(&mut first, &mut events);
    assert_eq!(menu, decision(&mut second, &mut events));
    assert_eq!(menu.player, B);
    assert_eq!(
        menu.options,
        vec![
            Alternative::BlindMove {
                verb: Verb::Return,
                source: Zone::Hand(A),
                back: Deck::base(2),
            },
            Alternative::Decline,
        ]
    );
    assert!(events.is_empty());
}

#[test]
fn computed_age_overflow_faults_but_exhaustion_ends_without_repeating_the_return() {
    for increase in [1, 255] {
        let mut f = Fixture::default();
        let code = f.agriculture(Zone::Board(A, Color::Yellow));
        f.cards[usize::from(code.0)].effects[0].instructions[2] =
            build::draw_age(Expr::Add(&build::LAST_VALUE, increase));
        let card = f.tech(Color::Blue, 11, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        decision(&mut game, &mut events);
        game.choose(0).unwrap();
        let expected = if increase == 1 {
            Ok(Status::Draw(DrawReason::Exhaustion))
        } else {
            Err(Error::Unsupported("computed draw value overflow"))
        };
        assert_eq!(game.advance(100, &mut events), expected);
        assert_eq!(events.len(), 1);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(11))), &[card]);
        let before = game.clone();
        assert_eq!(game.advance(100, &mut events), expected);
        assert_eq!(game, before);
        assert_eq!(events.len(), 1);
        assert!(game.position().check_conservation());
    }
}
