use inno_engine::*;

mod common;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn invoke(game: &mut Game, card: CardId, effect: u16) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn decline(game: &mut Game, events: &mut Vec<Event>) {
    let menu = decision(game, events);
    game.choose(
        menu.options
            .iter()
            .position(|option| *option == Alternative::Decline)
            .unwrap(),
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                game.choose(0).unwrap();
            }
            status => return status,
        }
    }
    panic!("aggregate program failed to settle");
}

fn rewards<const F: bool>() {
    for (code, age, score, factor) in [
        ("B020", 2, true, 1),
        ("B074", 7, true, 1),
        ("B078", 8, false, 2),
    ] {
        for selected in 0..=3 {
            let mut f = Fixture::default();
            let source = base(&mut f, code, Zone::Junk);
            // Two distinct printed values, but three successful originals.
            let payments = ["B001", "B002", "B026"].map(|code| base(&mut f, code, Zone::Hand(A)));
            let supply: &[&str] = match age {
                2 => &["B016", "B018"],
                7 => &["B070", "B071"],
                _ => &["B077", "B079", "B080", "B081"],
            };
            let draws: Vec<_> = supply
                .iter()
                .map(|code| base(&mut f, code, Zone::Deck(Deck::base(age))))
                .collect();
            let mut game = Game::seeded(f.position(), Expansions::with_figures(F), 91).unwrap();
            invoke(&mut game, source, 0);
            let mut events = Vec::new();
            for card in payments.into_iter().take(selected) {
                choose_card(&mut game, &mut events, card);
            }
            if selected < 3 {
                decline(&mut game, &mut events);
            }
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            let expected = if selected == 0 {
                0
            } else if selected == 3 {
                2 * factor
            } else {
                factor
            };
            let destination = if score { Zone::Score(A) } else { Zone::Hand(A) };
            assert_eq!(
                draws
                    .iter()
                    .filter(|card| game.position().cards(destination).contains(card))
                    .count(),
                expected
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, Event::Moved(m) if m.verb.is_none()))
                    .count(),
                expected
            );
            assert!(game.position().check_conservation());
        }
    }
}

#[test]
fn distinct_original_values_drive_optional_reward_count_and_score_in_both_kernels() {
    rewards::<false>();
    rewards::<true>();
}

#[test]
fn replaced_return_and_karma_tuck_supply_no_distinct_value_reward() {
    for code in ["B020", "B078", "B064"] {
        let mut f = Fixture::default();
        let source = base(&mut f, code, Zone::Junk);
        f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
        let replacement = f.figure(Figure::Archimedes, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, replacement);
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        assert!(events.iter().any(|event| matches!(event, Event::Moved(m) if m.card == replacement && m.verb == Some(Verb::Tuck) && m.inside_karma)));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Moved(m) if m.verb.is_none()))
        );
    }
}

fn alchemy<const F: bool>() {
    for red in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B032", Zone::Board(A, Color::Blue));
        base(&mut f, "B001", Zone::Board(A, Color::Red));
        let existing = base(&mut f, "B004", Zone::Hand(A));
        let first = base(&mut f, "B038", Zone::Deck(Deck::base(4)));
        let second = base(
            &mut f,
            if red { "B037" } else { "B042" },
            Zone::Deck(Deck::base(4)),
        );
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        let reveals: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(reveals, [second, first]);
        let draw_reveal: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Moved(m) if m.verb.is_none() => Some((false, m.card)),
                Event::Revealed { card, .. } => Some((true, *card)),
                _ => None,
            })
            .collect();
        assert_eq!(
            draw_reveal,
            [
                (false, second),
                (true, second),
                (false, first),
                (true, first)
            ]
        );
        for card in [existing, first, second] {
            assert_eq!(game.position().cards(Zone::Hand(A)).contains(&card), !red);
        }
    }
}

#[test]
fn alchemy_snapshots_authority_colors_and_returns_the_entire_hand_after_red() {
    alchemy::<false>();
    alchemy::<true>();
}

fn alchemy_second<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B032", Zone::Junk);
    let melded = base(&mut f, "B001", Zone::Hand(A));
    let scored = base(&mut f, "B004", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 1);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(!menu.options.contains(&Alternative::Decline));
    choose_card(&mut game, &mut events, melded);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().top(A, Color::Red), Some(melded));
    assert_eq!(game.position().cards(Zone::Score(A)), [scored]);
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).verb)
            .collect::<Vec<_>>(),
        [Some(Verb::Meld), Some(Verb::Score)]
    );
}

#[test]
fn alchemy_second_effect_is_separate_mandatory_meld_then_score() {
    alchemy_second::<false>();
    alchemy_second::<true>();
}

fn physics<const F: bool>() {
    for duplicate in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B053", Zone::Junk);
        let existing = base(&mut f, "B004", Zone::Hand(A));
        let codes = ["B056", if duplicate { "B057" } else { "B058" }, "B062"];
        let originals = codes.map(|code| base(&mut f, code, Zone::Deck(Deck::base(6))));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Revealed { .. }))
                .count(),
            3
        );
        for card in originals.into_iter().chain([existing]) {
            assert_eq!(
                game.position().cards(Zone::Hand(A)).contains(&card),
                !duplicate
            );
        }
    }
}

#[test]
fn physics_compares_the_three_original_colors_not_the_prior_hand() {
    physics::<false>();
    physics::<true>();
}

fn exhaustion<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B053", Zone::Junk);
    let first = base(&mut f, "B056", Zone::Deck(Deck::base(6)));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(events.len(), 2);
    assert_eq!(game.position().cards(Zone::Hand(A)), [first]);
    assert!(matches!(events[1], Event::Revealed { card, .. } if card == first));
}

#[test]
fn compound_reveals_happen_before_later_draw_exhaustion() {
    exhaustion::<false>();
    exhaustion::<true>();
}

fn democracy_shared<const F: bool>() {
    for own in [0, 1, 2] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B064", Zone::Board(A, Color::Purple));
        base(&mut f, "B012", Zone::Board(B, Color::Blue));
        let opponent = base(&mut f, "B004", Zone::Hand(B));
        let payments = [
            base(&mut f, "B001", Zone::Hand(A)),
            base(&mut f, "B002", Zone::Hand(A)),
        ];
        let reward_one = base(&mut f, "B077", Zone::Deck(Deck::base(8)));
        let reward_two = base(&mut f, "B079", Zone::Deck(Deck::base(8)));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        game.start(
            Operation::Dogma {
                card: source,
                icon: Icon::Concept,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(decision(&mut game, &mut events).player, B);
        choose_card(&mut game, &mut events, opponent);
        for card in payments.into_iter().take(own) {
            choose_card(&mut game, &mut events, card);
        }
        if own < 2 {
            decline(&mut game, &mut events);
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Dogma { shared: true })
        );
        assert_eq!(game.position().cards(Zone::Score(B)), [reward_two]);
        assert_eq!(
            game.position().cards(Zone::Score(A)).contains(&reward_one),
            own == 2
        );
        // A new root invocation has a fresh action, not the previous tally.
        invoke(&mut game, source, 0);
        if own < 2 {
            decline(&mut game, &mut events);
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().cards(Zone::Score(A)).len(),
            usize::from(own == 2)
        );
    }
}

#[test]
fn democracy_compares_prior_shared_executor_returns_strictly_and_resets() {
    democracy_shared::<false>();
    democracy_shared::<true>();
}

#[test]
fn reaction_draws_are_neither_revealed_nor_members_of_original_color_cohorts() {
    for code in ["B032", "B053"] {
        let mut f = Fixture::default();
        let source = base(&mut f, code, Zone::Board(A, Color::Blue));
        base(&mut f, "B001", Zone::Board(A, Color::Red));
        f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
        let payment = base(&mut f, "B046", Zone::Hand(A));
        let originals: Vec<_> = if code == "B032" {
            ["B038", "B042"]
                .into_iter()
                .map(|code| base(&mut f, code, Zone::Deck(Deck::base(4))))
                .collect()
        } else {
            ["B056", "B058", "B062"]
                .into_iter()
                .map(|code| base(&mut f, code, Zone::Deck(Deck::base(6))))
                .collect()
        };
        let child = base(&mut f, "B057", Zone::Deck(Deck::base(6)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, payment);
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(game.position().cards(Zone::Hand(A)).contains(&child));
        for card in &originals {
            assert!(game.position().cards(Zone::Hand(A)).contains(card));
        }
        let reveals: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(reveals, originals.into_iter().rev().collect::<Vec<_>>());
        assert!(!reveals.contains(&child));
    }
}

#[test]
fn alchemy_keeps_a_returned_original_and_its_draw_keeps_the_resolved_age() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B032", Zone::Board(A, Color::Blue));
    base(&mut f, "B001", Zone::Board(A, Color::Red));
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    let original = base(&mut f, "B046", Zone::Deck(Deck::base(5)));
    let child = base(&mut f, "B062", Zone::Deck(Deck::base(6)));
    let seven = base(&mut f, "B068", Zone::Deck(Deck::base(7)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    // The first draw skips empty age 4. The second resolves to age 6; its karma returns
    // that age-5 original and draws the age-6 child. The original draw then continues
    // from age 6, not back down to the refilled age 5, so it takes the 7.
    choose_card(&mut game, &mut events, original);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    let reveals: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(reveals, [original, seven]);
    // The returned red original still counts as drawn, so the hand returns.
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(5))), [original]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(6))), [child]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), [seven]);
}

fn alchemy_zero<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B032", Zone::Junk);
    let untouched = base(&mut f, "B001", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), [untouched]);
}

#[test]
fn zero_authority_colors_do_not_draw_or_treat_existing_red_cards_as_drawn() {
    alchemy_zero::<false>();
    alchemy_zero::<true>();
}

fn reward_exhaustion<const F: bool>() {
    for (code, supply, age, score) in [
        ("B020", "B016", 2, true),
        ("B074", "B070", 7, true),
        ("B078", "B077", 8, false),
    ] {
        let mut f = Fixture::default();
        let source = base(&mut f, code, Zone::Junk);
        let first = base(&mut f, "B001", Zone::Hand(A));
        let second = if score {
            Some(base(&mut f, "B026", Zone::Hand(A)))
        } else {
            None
        };
        // Currency can redraw its own payment at age 3; that is the one
        // available reward, not an extra pre-existing supply card.
        let drawn = if code == "B020" {
            second.unwrap()
        } else {
            base(&mut f, supply, Zone::Deck(Deck::base(age)))
        };
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, first);
        if let Some(second) = second {
            choose_card(&mut game, &mut events, second);
        }
        let status = settle(&mut game, &mut events);
        assert_eq!(
            status,
            if score {
                Status::Winner(A)
            } else {
                Status::Draw(DrawReason::Exhaustion)
            }
        );
        assert_eq!(
            game.position()
                .cards(if score { Zone::Score(A) } else { Zone::Hand(A) }),
            [drawn]
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Moved(m) if m.verb.is_none()))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Score)))
                .count(),
            usize::from(score)
        );
    }
}

#[test]
fn each_distinct_value_reward_finishes_its_score_before_later_exhaustion() {
    reward_exhaustion::<false>();
    reward_exhaustion::<true>();
}

#[test]
fn equal_printed_values_from_different_origin_sets_earn_only_one_currency_reward() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B020", Zone::Junk);
    let base_payment = base(&mut f, "B016", Zone::Hand(A));
    let figure_payment = f.figure(Figure::Archimedes, Zone::Hand(A));
    let reward = base(&mut f, "B018", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, base_payment);
    choose_card(&mut game, &mut events, figure_payment);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), [reward]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        [base_payment]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(2))),
        [figure_payment]
    );
}
