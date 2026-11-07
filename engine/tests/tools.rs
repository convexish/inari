use inno_engine::*;
mod common;
use common::*;

fn second(game: &mut Game, tools: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card: tools,
            effect: 1,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

#[test]
fn complete_tools_performs_both_printed_payments_and_rewards() {
    let mut f = Fixture::default();
    let tools = f.tools(Zone::Board(A, Color::Blue));
    let hand: Vec<_> = [1, 2, 4, 3]
        .into_iter()
        .map(|age| f.tech(Color::Red, age, Zone::Hand(A), vec![]))
        .collect();
    let reward = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let ones: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]))
        .collect();
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    let action = Action::Dogma {
        card: tools,
        auspice: false,
    };
    base.take_action(action, ActionKind::Regular).unwrap();
    figures.take_action(action, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    let mut other = Vec::new();
    for &card in &hand {
        choose_card(&mut base, &mut events, card);
        choose_card(&mut figures, &mut other, card);
    }
    assert_eq!(
        settle(&mut base, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        settle(&mut figures, &mut other),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(base.position(), figures.position());
    assert_eq!(events, other);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [
            hand[0], hand[1], hand[2], reward, reward, hand[3], ones[2], ones[1], ones[0]
        ]
    );
    assert_eq!(base.position().top(A, Color::Green), Some(reward));
    assert_eq!(
        base.position().cards(Zone::Hand(A)),
        &[ones[2], ones[1], ones[0]]
    );
    assert_eq!(base.position().regular_actions_started(), 1);
    assert!(base.position().check_conservation());
}

#[test]
fn archimedes_modifies_both_the_boxed_comparison_and_draw_age_but_not_three_draws() {
    let mut f = Fixture::default();
    let tools = f.tools(Zone::Board(A, Color::Blue));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let three = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let four = f.tech(Color::Green, 4, Zone::Hand(A), vec![]);
    let twos: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    second(&mut game, tools);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(
        matches!(menu.options[0], Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if card == four)
    );
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [four, twos[2], twos[1], twos[0]]
    );
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[three, twos[2], twos[1], twos[0]]
    );
}

#[test]
fn absent_and_declined_matching_cards_do_not_draw_or_reuse_an_old_result() {
    for has_three in [false, true] {
        let mut f = Fixture::default();
        let tools = f.tools(Zone::Board(A, Color::Blue));
        let card = f.tech(
            Color::Red,
            if has_three { 3 } else { 4 },
            Zone::Hand(A),
            vec![],
        );
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        second(&mut game, tools);
        let mut events = Vec::new();
        if has_three {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options.len(), 2);
            assert_eq!(menu.options[1], Alternative::Decline);
            game.choose(1).unwrap();
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(events.is_empty());
        assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
    }
}

#[test]
fn homer_replacing_the_matching_return_does_not_earn_the_three_draws() {
    let mut f = Fixture::default();
    let tools = f.tools(Zone::Board(A, Color::Blue));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let shen = f.figure(Figure::ShenKuo, Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    second(&mut game, tools);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, shen);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Tuck));
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().top(A, Color::Green), Some(shen));
}

#[test]
fn maxwells_first_result_is_separate_from_the_matching_return_and_three_draws() {
    let mut f = Fixture::default();
    let tools = f.tools(Zone::Board(A, Color::Blue));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let paid = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let eight = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
    let ones: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    second(&mut game, tools);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, paid);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [eight, paid, ones[2], ones[1], ones[0]]
    );
    assert!(moved(&events[0]).inside_karma);
    assert!(events[1..].iter().all(|e| !moved(e).inside_karma));
}

#[test]
fn a_boxed_comparison_above_eleven_matches_nothing_instead_of_causing_exhaustion() {
    let mut f = Fixture::default();
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let eleven = f.tech(Color::Red, 11, Zone::Hand(A), vec![]);
    let source = f.tech(
        Color::Green,
        1,
        Zone::Board(A, Color::Green),
        vec![
            build::verb(
                Verb::Return,
                build::filtered(&build::HAND, &[Filter::Has(Test::BoxedValues(&[11]))]),
            )
            .to(Destination::Deck)
            .optional(),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[eleven]);
}

#[test]
fn clones_preserve_the_comparison_scope_and_each_sequential_draw() {
    let mut f = Fixture::default();
    let tools = f.tools(Zone::Board(A, Color::Blue));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    f.tech(Color::Red, 4, Zone::Hand(A), vec![]);
    for _ in 0..3 {
        f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    }
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    second(&mut game, tools);
    let finish = |game: &mut Game, events: &mut Vec<Event>| {
        for _ in 0..20 {
            match settle(game, events) {
                Status::Decision(_) => game.choose(0).unwrap(),
                status => return status,
            }
        }
        panic!("Tools did not finish");
    };
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = finish(&mut expected, &mut all);
    let mut prefix = Vec::new();
    for _ in 0..150 {
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
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("Tools did not finish");
}

#[test]
fn boxed_value_filter_uses_public_backs_without_exposing_private_fronts() {
    let make = |reverse| {
        let mut f = Fixture::default();
        let tools = f.tools(Zone::Board(A, Color::Blue));
        let first = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
        let second = f.tech(Color::Green, 3, Zone::Hand(A), vec![]);
        f.tech(Color::Yellow, 4, Zone::Hand(A), vec![]);
        if reverse {
            f.placements
                .swap(usize::from(first.0), usize::from(second.0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        game.start(
            Operation::ExecuteEffect {
                card: tools,
                effect: 1,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let mut events = Vec::new();
    let menu = decision(&mut first, &mut events);
    assert_eq!(menu, decision(&mut second, &mut events));
    // Both [3] backs are one blind option; neither front orders the menu.
    assert_eq!(
        menu.options,
        [
            Alternative::BlindMove {
                verb: Verb::Return,
                source: Zone::Hand(A),
                back: Deck::base(3),
            },
            Alternative::Decline,
        ]
    );
    assert!(events.is_empty());
}
