use inno_engine::*;

mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("Pottery did not finish");
}

fn decline(game: &mut Game, events: &mut Vec<Event>) {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Decline)
        .unwrap();
    game.choose(index).unwrap();
}

fn first_effect(game: &mut Game, card: CardId, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

#[test]
fn printed_pottery_returns_zero_to_three_and_scores_the_exact_count_draw_in_both_modes() {
    for count in 0..=3 {
        let mut f = Fixture::default();
        let pottery = f.pottery(Zone::Board(A, Color::Blue));
        let hand: Vec<_> = (0..4)
            .map(|_| f.tech(Color::Red, 7, Zone::Hand(A), vec![]))
            .collect();
        let final_draw = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        let reward = (count > 0).then(|| {
            f.tech(
                Color::Green,
                count as u8,
                Zone::Deck(Deck::base(count as u8)),
                vec![],
            )
        });
        let position = f.position();
        let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
        let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
        base.self_execute(pottery, Context::new(A)).unwrap();
        figures.self_execute(pottery, Context::new(A)).unwrap();
        let mut base_events = Vec::new();
        let mut figure_events = Vec::new();
        for &card in &hand[..count] {
            choose_card(&mut base, &mut base_events, card);
            choose_card(&mut figures, &mut figure_events, card);
        }
        if count < 3 {
            decline(&mut base, &mut base_events);
            decline(&mut figures, &mut figure_events);
        }
        assert_eq!(
            settle(&mut base, &mut base_events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            settle(&mut figures, &mut figure_events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(base.position(), figures.position());
        assert_eq!(base_events, figure_events);
        assert_eq!(base_events.len(), count + usize::from(count > 0) * 2 + 1);
        assert_eq!(base.position().cards(Zone::Score(A)), reward.as_slice());
        assert_eq!(
            base.position().cards(Zone::Hand(A)),
            [hand[count..].to_vec(), vec![final_draw]].concat()
        );
        assert!(base.position().check_conservation());
    }
}

#[test]
fn an_empty_hand_needs_no_return_decision_and_still_runs_the_second_effect() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    let drawn = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(pottery, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn a_singleton_return_remains_optional_and_stopping_retains_prior_returns() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    let card = f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    let drawn = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    first_effect(&mut game, pottery, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options.len(), 2);
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 3);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), &[card]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[drawn]);
}

#[test]
fn homer_replacements_consume_selections_but_only_actual_returns_determine_reward_age() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let figure = f.figure(Figure::Archimedes, Zone::Hand(A));
    let returned: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 7, Zone::Hand(A), vec![]))
        .collect();
    let reward = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let final_draw = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(pottery, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [figure, returned[0], returned[1], reward, reward, final_draw]
    );
    assert_eq!(moved(&events[0]).verb, Some(Verb::Tuck));
    assert!(moved(&events[0]).inside_karma);
    assert!(events[1..].iter().all(|e| !moved(e).inside_karma));
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[returned[2], final_draw]
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[reward]);
    assert!(game.position().check_conservation());
}

#[test]
fn all_replaced_returns_earn_nothing_but_do_not_cancel_the_next_effect() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let figure = f.figure(Figure::Archimedes, Zone::Hand(A));
    let draw = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(pottery, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [figure, draw]
    );
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[draw]);
}

#[test]
fn maxwells_first_draws_do_not_inflate_the_return_count_or_execute_ahead_of_returns() {
    let mut f = Fixture::default();
    // Pottery is selected before covering it with Maxwell; foreign execution
    // keeps the selected program even when it is no longer a top card.
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let returned: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Red, 7, Zone::Hand(A), vec![]))
        .collect();
    let draws: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]))
        .collect();
    let reward = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    first_effect(&mut game, pottery, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [
            draws[2],
            returned[0],
            draws[1],
            returned[1],
            draws[0],
            returned[2],
            reward,
            reward
        ]
    );
    assert_eq!(events.iter().filter(|e| moved(e).inside_karma).count(), 3);
    assert_eq!(game.position().cards(Zone::Score(A)), &[reward]);
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        draws.into_iter().rev().collect::<Vec<_>>()
    );
    assert!(game.position().check_conservation());
}

fn mixed_return_program(condition: Instruction) -> (Game, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    f.figure(Figure::Archimedes, Zone::Hand(A));
    f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    let code = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![
            build::verb(Verb::Return, build::HAND)
                .to(Destination::Deck)
                .all(),
            condition,
            Instruction::Win(WinRule::Executor),
        ],
    );
    (Game::new(f.position(), Expansions::FIGURES).unwrap(), code)
}

#[test]
fn partial_all_is_not_if_you_do_but_is_if_any_and_exactly_one() {
    for (condition, wins) in [
        (build::if_(build::DID, 1), false),
        (build::if_(build::DID_ANY, 1), true),
        (Instruction::If(&Cond::Did(Outcome::Exactly(1)), 1), true),
        (Instruction::If(&Cond::Did(Outcome::Exactly(2)), 1), false),
        (Instruction::If(&Cond::Did(Outcome::Exactly(0)), 1), false),
    ] {
        let (mut game, code) = mixed_return_program(condition);
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            if wins {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
        assert_eq!(events.len(), 2);
    }
}

#[test]
fn an_empty_or_zero_limit_batch_clears_an_earlier_card_result() {
    for zero_limit in [false, true] {
        let mut f = Fixture::default();
        let returned = f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
        let code = f.tech(
            Color::Blue,
            1,
            Zone::Board(A, Color::Blue),
            vec![
                build::verb(Verb::Return, build::HAND).to(Destination::Deck),
                if zero_limit {
                    build::verb(Verb::Return, build::HAND)
                        .to(Destination::Deck)
                        .up_to(Some(Expr::Const(0)))
                } else {
                    build::verb(Verb::Return, build::HAND)
                        .to(Destination::Deck)
                        .all()
                },
                build::draw_age(Expr::NonZero(&Expr::Done)),
                build::if_(build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).card, returned);
    }
}

#[test]
fn a_bulk_result_has_no_single_card_for_move_last_and_does_not_leak_between_effects() {
    let mut f = Fixture::default();
    let returned = f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    let code = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![
            build::verb(Verb::Return, build::HAND)
                .to(Destination::Deck)
                .all(),
            build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    f.cards[usize::from(code.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![
            build::draw_age(Expr::NonZero(&Expr::Done)),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    });
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, returned);
}

#[test]
fn separate_effects_do_not_inherit_the_preceding_bulk_quantity() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    let code = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![
            build::verb(Verb::Return, build::HAND)
                .to(Destination::Deck)
                .all(),
        ],
    );
    f.cards[usize::from(code.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![build::draw_age(Expr::NonZero(&Expr::Done))],
    });
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn meiji_borrowed_hand_returns_count_physical_cards_once() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let own = f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    let borrowed = f.tech(Color::Green, 6, Zone::Hand(B), vec![]);
    let reward = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    first_effect(&mut game, pottery, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    // Returns run in display order: the borrowed [6] before the own [7].
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [borrowed, own, reward, reward]
    );
    assert_eq!(moved(&events[0]).source, Zone::Hand(B));
    assert_eq!(moved(&events[1]).executor, A);
    assert_eq!(game.position().cards(Zone::Score(A)), &[reward]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn shared_pottery_keeps_separate_player_counts_and_earns_one_full_draw_reward() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    let opposing = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    f.cards[usize::from(opposing.0)].symbols = four_slots([Symbol::Icon(Icon::Health); 4]);
    let opponent_return = f.tech(Color::Red, 7, Zone::Hand(B), vec![]);
    let own: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Green, 7, Zone::Hand(A), vec![]))
        .collect();
    let ones: Vec<_> = (0..4)
        .map(|_| f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]))
        .collect();
    let three = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    let action = Action::Dogma {
        card: pottery,
        auspice: false,
    };
    base.take_action(action, ActionKind::Regular).unwrap();
    figures.take_action(action, ActionKind::Regular).unwrap();
    let mut base_events = Vec::new();
    let mut figure_events = Vec::new();
    assert_eq!(
        finish(&mut base, &mut base_events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        finish(&mut figures, &mut figure_events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(base.position(), figures.position());
    assert_eq!(base_events, figure_events);
    assert_eq!(
        base_events
            .iter()
            .map(|e| moved(e).card)
            .collect::<Vec<_>>(),
        [
            opponent_return,
            ones[3],
            ones[3],
            own[0],
            own[1],
            own[2],
            three,
            three,
            ones[2],
            ones[1],
            ones[0]
        ]
    );
    assert_eq!(base.position().cards(Zone::Score(A)), &[three]);
    assert_eq!(base.position().cards(Zone::Score(B)), &[ones[3]]);
    assert_eq!(base.position().cards(Zone::Hand(A)), &[ones[1], ones[0]]);
    assert_eq!(base.position().cards(Zone::Hand(B)), &[ones[2]]);
    assert_eq!(base.position().regular_actions_started(), 1);
    assert!(base.position().check_conservation());
}

#[test]
fn archimedes_changes_the_printed_second_age_not_potterys_limit_or_computed_age() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let hand: Vec<_> = (0..4)
        .map(|_| f.tech(Color::Red, 7, Zone::Hand(A), vec![]))
        .collect();
    let reward = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let final_draw = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(pottery, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 6);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[hand[3], final_draw]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[reward]);
}

#[test]
fn hidden_return_menus_keep_fronts_private_and_invalid_choices_are_atomic() {
    let make = |reverse| {
        let mut f = Fixture::default();
        let pottery = f.pottery(Zone::Board(A, Color::Blue));
        let first = f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
        let second = f.tech(Color::Green, 7, Zone::Hand(A), vec![]);
        if reverse {
            f.placements
                .swap(usize::from(first.0), usize::from(second.0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        first_effect(&mut game, pottery, context);
        game
    };
    let mut game = make(false);
    let mut other = make(true);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu, decision(&mut other, &mut events));
    assert_eq!(menu.player, B);
    assert!(menu.options.contains(&Alternative::Decline));
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Return,
            source: Zone::Hand(A),
            back: Deck {
                set: CardSet::Base,
                age: 7
            }
        } | Alternative::Decline
    )));
    let snapshot = game.clone();
    assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
    assert_eq!(game, snapshot);
    assert!(events.is_empty());
}

#[test]
fn a_terminal_first_abandons_the_batch_and_its_reward() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let pending = f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 11, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    first_effect(&mut game, pottery, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[pending]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn clones_preserve_partial_counts_and_reaction_progress_at_every_work_boundary() {
    let mut f = Fixture::default();
    let pottery = f.pottery(Zone::Board(A, Color::Blue));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    f.figure(Figure::Archimedes, Zone::Hand(A));
    for _ in 0..2 {
        f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    }
    f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.self_execute(pottery, Context::new(A)).unwrap();
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = finish(&mut expected, &mut all);
    let mut prefix = Vec::new();
    for _ in 0..300 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("batch did not terminate");
}
