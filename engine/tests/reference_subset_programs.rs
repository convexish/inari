use inno_engine::*;
mod common;
use common::*;

fn native(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}

fn figure(f: &mut Fixture, kind: Figure, owner: Player) -> CardId {
    let card = figure_card(kind.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}

fn start(game: &mut Game, card: CardId, effect: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = menu
                    .options
                    .iter()
                    .position(|option| *option == Alternative::Decline)
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("reference program did not finish");
}

fn select_reveal(game: &mut Game, events: &mut Vec<Event>, card: CardId) {
    let menu = decision(game, events);
    let index = menu.options.iter().position(|option| matches!(option,
        Alternative::Reveal { target: TargetDescriptor::Known { card: selected, .. } } if *selected == card
    )).unwrap();
    game.choose(index).unwrap();
}

fn movements(events: &[Event], verb: Verb) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(movement) if movement.verb == Some(verb) => Some(movement.card),
            _ => None,
        })
        .collect()
}

#[test]
fn colonialism_checks_the_original_draw_then_repeats_only_its_own_effect() {
    fn exercise<const F: bool>() {
        let mut f = Fixture::default();
        let source = native(&mut f, "B036", A);
        let last = f.add(base_card("B026").unwrap(), Zone::Deck(Deck::base(3)));
        let first = f.add(base_card("B030").unwrap(), Zone::Deck(Deck::base(3)));
        let bottom = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
        let top = f.tech(Color::Blue, 5, Zone::Deck(Deck::base(5)), vec![]);
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        start(&mut game, source, 0, A, A);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(movements(&events, Verb::Tuck), [first, last]);
        assert_eq!(movements(&events, Verb::Junk), [top, bottom]);
        assert_eq!(game.position().cards(Zone::Board(A, Color::Green)), [first]);
        assert_eq!(
            game.position().cards(Zone::Board(A, Color::Red)),
            [last, source]
        );
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn pirate_code_counts_original_demand_transfers_and_keeps_lowest_prosperity_ties() {
    for protected in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B047", A);
        if protected {
            figure(&mut f, Figure::Nightingale, B);
        }
        let one = f.tech(Color::Blue, 1, Zone::Score(B), vec![]);
        let four = f.tech(Color::Green, 4, Zone::Score(B), vec![]);
        let five = f.tech(Color::Yellow, 5, Zone::Score(B), vec![]);
        let blue = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        let green = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        for card in [blue, green] {
            f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        if !protected {
            let menu = decision(&mut game, &mut events);
            let mut eligible: Vec<_> = menu
                .options
                .iter()
                .filter_map(|option| match option {
                    Alternative::Move {
                        verb: Verb::Score,
                        target: TargetDescriptor::Known { card, .. },
                    } => Some(*card),
                    _ => None,
                })
                .collect();
            eligible.sort_unstable();
            let mut expected = vec![blue, green];
            expected.sort_unstable();
            assert_eq!(eligible, expected);
            let index = menu.options.iter().position(|option| matches!(option,
                Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == blue
            )).unwrap();
            game.choose(index).unwrap();
        }
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if protected {
            assert!(movements(&events, Verb::Transfer).is_empty());
            assert!(game.position().cards(Zone::Score(A)).is_empty());
            assert_eq!(game.position().top(A, Color::Blue), Some(blue));
        } else {
            assert_eq!(movements(&events, Verb::Transfer), [one, four]);
            assert_eq!(game.position().cards(Zone::Score(B)), [five]);
            assert_eq!(game.position().score(A), 7);
            assert_eq!(movements(&events, Verb::Score), [blue]);
        }
        assert_eq!(game.position().top(A, Color::Green), Some(green));
    }
}

#[test]
fn explosives_attempts_three_highest_and_draws_only_after_a_real_transfer_empties_hand() {
    fn exercise<const F: bool>() {
        for count in 0..=4 {
            let mut f = Fixture::default();
            let source = native(&mut f, "B067", A);
            let hand: Vec<_> = (1..=count)
                .map(|age| f.tech(Color::Blue, age, Zone::Hand(B), vec![]))
                .collect();
            let reward = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
            let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            start(&mut game, source, 0, B, A);
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            let expected: Vec<_> = hand.iter().rev().take(3).copied().collect();
            assert_eq!(movements(&events, Verb::Transfer), expected);
            assert_eq!(
                game.position().cards(Zone::Hand(B)).contains(&reward),
                count > 0 && count <= 3
            );
            if count == 4 {
                assert_eq!(game.position().cards(Zone::Hand(B)), [hand[0]]);
            }
            if count == 0 {
                assert!(game.position().cards(Zone::Hand(B)).is_empty());
            }
        }
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn explosives_does_not_let_an_attempted_borrowed_highest_hide_the_next_eligible_card() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B067", A);
    figure(&mut f, Figure::EmperorMeiji, B);
    let borrowed = f.add(base_card("B024").unwrap(), Zone::Hand(A));
    let owned = f.add(base_card("B012").unwrap(), Zone::Hand(B));
    let mut position = f.position();
    position.begin_turn(B);
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    start(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(movements(&events, Verb::Transfer), [owned]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), [borrowed, owned]);
}

#[test]
fn refrigeration_preserves_the_choice_of_survivor_and_can_score_it_in_the_next_effect() {
    fn exercise<const F: bool>() {
        for count in 0..=4 {
            let mut f = Fixture::default();
            let source = native(&mut f, "B068", A);
            let hand: Vec<_> = (1..=count)
                .map(|age| f.tech(Color::Blue, age, Zone::Hand(B), vec![]))
                .collect();
            let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            start(&mut game, source, 0, B, A);
            let mut events = Vec::new();
            if count >= 3 {
                choose_card(&mut game, &mut events, hand[1]);
            }
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            let expected = if count >= 3 {
                &hand[..1]
            } else {
                &hand[hand.len().saturating_sub(1)..]
            };
            assert_eq!(game.position().cards(Zone::Hand(B)), expected);
            assert_eq!(
                movements(&events, Verb::Return).len(),
                hand.len().saturating_sub(1)
            );
            if let Some(&survivor) = expected.first() {
                start(&mut game, source, 1, B, B);
                choose_card(&mut game, &mut events, survivor);
                assert_eq!(
                    finish(&mut game, &mut events),
                    Status::Complete(ReturnValue::Unit)
                );
                assert_eq!(game.position().cards(Zone::Score(B)), [survivor]);
            }
        }
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn specialization_transfers_only_one_current_top_and_retains_the_revealed_card() {
    fn exercise<const F: bool>() {
        let mut f = Fixture::default();
        let source = native(&mut f, "B095", A);
        let revealed = f.add(base_card("B012").unwrap(), Zone::Hand(A));
        let covered = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
        let top = f.tech(Color::Blue, 4, Zone::Board(B, Color::Blue), vec![]);
        let other = f.tech(Color::Green, 5, Zone::Board(B, Color::Green), vec![]);
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(movements(&events, Verb::Transfer), [top]);
        assert_eq!(game.position().cards(Zone::Hand(A)), [revealed, top]);
        assert_eq!(game.position().top(B, Color::Blue), Some(covered));
        assert_eq!(game.position().top(B, Color::Green), Some(other));
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn databases_uses_highest_physical_achievement_value_not_achievement_count() {
    fn exercise<const F: bool>() {
        for value in [0, 2, 4] {
            let mut f = Fixture::default();
            let source = native(&mut f, "B100", A);
            if value > 0 {
                f.tech(Color::Red, value, Zone::Achievements(B), vec![]);
            }
            let score: Vec<_> = (1..=3)
                .map(|age| f.tech(Color::Blue, age, Zone::Score(B), vec![]))
                .collect();
            let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            start(&mut game, source, 0, B, A);
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                movements(&events, Verb::Return),
                score[..usize::from(value.min(3))]
            );
            assert_eq!(
                game.position().cards(Zone::Score(B)),
                &score[usize::from(value.min(3))..]
            );
        }
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn escapism_returns_the_retained_value_draws_three_and_executes_the_junked_native_card() {
    fn exercise<const F: bool>() {
        let mut f = Fixture::default();
        let source = native(&mut f, "B114", A);
        let writing = f.add(base_card("B012").unwrap(), Zone::Hand(A));
        let returned = f.add(base_card("B015").unwrap(), Zone::Hand(A));
        let deck: Vec<_> = ["B002", "B004", "B005"]
            .into_iter()
            .map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))))
            .collect();
        let reward = f.add(base_card("B022").unwrap(), Zone::Deck(Deck::base(2)));
        let chain = f.add(base_card("B115").unwrap(), Zone::Deck(Deck::base(11)));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        select_reveal(&mut game, &mut events, writing);
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().cards(Zone::Junk), [writing]);
        assert_eq!(movements(&events, Verb::Return), [returned]);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), [returned]);
        assert_eq!(game.position().cards(Zone::Achievements(A)), [chain]);
        let mut expected: Vec<_> = deck.into_iter().rev().collect();
        expected.push(reward);
        assert_eq!(game.position().cards(Zone::Hand(A)), expected);
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn escapism_does_not_borrow_homers_replacement_for_the_junked_card_reference() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B114", B);
    figure(&mut f, Figure::Homer, A);
    let chosen = f.add(figure_card("F005").unwrap(), Zone::Hand(A));
    let retained = f.add(base_card("B012").unwrap(), Zone::Hand(A));
    let supply = f.add(base_card("B015").unwrap(), Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, source, 0, A, A);
    let mut events = Vec::new();
    select_reveal(&mut game, &mut events, chosen);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(movements(&events, Verb::Tuck), [chosen]);
    assert!(movements(&events, Verb::Junk).is_empty());
    assert!(movements(&events, Verb::Return).is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), [retained]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), [supply]);
}

#[test]
fn a_transfer_to_the_same_physical_area_is_inapplicable_before_would_reactions() {
    let mut f = Fixture::default();
    figure(&mut f, Figure::EmperorMeiji, B);
    figure(&mut f, Figure::YiSunSin, A);
    let card = f.add(base_card("B024").unwrap(), Zone::Hand(A));
    let mut position = f.position();
    position.begin_turn(B);
    let before = position.clone();
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: Target {
                card,
                physical: Zone::Hand(A),
                logical: LogicalZone::Hand(B),
            },
            destination: Zone::Hand(A),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert_eq!(game.position(), &before);
    assert!(events.is_empty());
}
