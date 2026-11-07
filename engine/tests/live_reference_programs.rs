use inno_engine::*;
mod common;
use common::*;

fn native(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn figure(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = figure_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn run(game: &mut Game, card: CardId, effect: u16, executor: Player, demander: Player) {
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
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let choice = menu
                    .options
                    .iter()
                    .position(|option| *option == Alternative::Decline)
                    .unwrap_or(0);
                game.choose(choice).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("live-reference program did not finish");
}
fn moved_cards(events: &[Event], verb: Verb) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb == Some(verb) => Some(m.card),
            _ => None,
        })
        .collect()
}

#[test]
fn construction_commits_two_transfers_as_possible_and_draws_even_with_no_payment() {
    fn exercise<const F: bool>() {
        for count in 0..=3 {
            let mut f = Fixture::default();
            let source = native(&mut f, "B016", A);
            let hand: Vec<_> = (0..count)
                .map(|i| f.tech(Color::Blue, i + 1, Zone::Hand(B), vec![]))
                .collect();
            let drawn = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
            let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            run(&mut game, source, 0, B, A);
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                moved_cards(&events, Verb::Transfer),
                hand[..hand.len().min(2)]
            );
            assert_eq!(
                game.position().cards(Zone::Hand(A)),
                &hand[..hand.len().min(2)]
            );
            assert!(game.position().cards(Zone::Hand(B)).contains(&drawn));
            assert_eq!(
                game.position().cards(Zone::Hand(B)).len(),
                usize::from(count.saturating_sub(2)) + 1
            );
            assert_eq!(moved(events.last().unwrap()).card, drawn);
        }
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn construction_requires_five_physical_tops_and_no_other_five_top_board() {
    for (own_count, other_count, claims) in
        [(4, 0, false), (5, 0, true), (5, 4, true), (5, 5, false)]
    {
        let mut f = Fixture::default();
        let source = native(&mut f, "B016", A);
        for &color in Color::ALL.iter().take(own_count).skip(1) {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
        }
        for &color in Color::ALL.iter().take(other_count) {
            f.tech(color, 1, Zone::Board(B, color), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        run(&mut game, source, 1, A, A);
        finish(&mut game, &mut Vec::new());
        assert_eq!(
            game.position().special_location(Special::Empire),
            if claims {
                SpecialLocation::Owned(A)
            } else {
                SpecialLocation::Available
            }
        );
    }
}

#[test]
fn optics_retains_the_original_draws_printed_icon_after_a_replaced_meld() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B027", A);
    figure(&mut f, "F033", A);
    let original = f.add(base_card("B031").unwrap(), Zone::Deck(Deck::base(3)));
    let reward = f.add(base_card("B040").unwrap(), Zone::Deck(Deck::base(4)));
    let replacement = f.add(base_card("B037").unwrap(), Zone::Deck(Deck::base(4)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().top(A, Color::Red), Some(replacement));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), [original]);
    assert_eq!(game.position().cards(Zone::Score(A)), [reward]);
    assert!(moved_cards(&events, Verb::Transfer).is_empty());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Moved(m) if m.card == replacement && m.inside_karma))
    );
}

#[test]
fn optics_otherwise_uses_live_effective_scores_after_melding_and_excludes_ties() {
    for supplies_authority in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B027", A);
        figure(&mut f, "F002", B);
        let own = f.tech(Color::Yellow, 3, Zone::Score(A), vec![]);
        let other = f.tech(Color::Green, 1, Zone::Score(B), vec![]);
        let child = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
        if supplies_authority {
            f.cards[usize::from(child.0)].symbols[0] = Symbol::Icon(Icon::Authority);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(game.position().score(A), 3);
        assert_eq!(game.position().score(B), 2);
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().top(A, Color::Blue), Some(child));
        if supplies_authority {
            assert_eq!(game.position().score(A), 2);
            assert_eq!(game.position().score(B), 2);
            assert_eq!(game.position().cards(Zone::Score(A)), [own]);
            assert!(moved_cards(&events, Verb::Transfer).is_empty());
        } else {
            assert_eq!(game.position().cards(Zone::Score(B)), [other, own]);
            assert_eq!(moved_cards(&events, Verb::Transfer), [own]);
        }
    }
}

#[test]
fn compass_requeries_demanders_tops_and_does_not_gate_melding_on_a_transfer() {
    for can_transfer in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B030", A);
        let buried = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        let selected = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        let target = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
        f.cards[usize::from(target.0)].symbols[0] = Symbol::Icon(if can_transfer {
            Icon::Health
        } else {
            Icon::Authority
        });
        let green = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
        f.cards[usize::from(green.0)].symbols[0] = Symbol::Icon(Icon::Health);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        run(&mut game, source, 0, B, A);
        let mut events = Vec::new();
        if !can_transfer {
            choose_card(&mut game, &mut events, selected);
        }
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().top(B, Color::Blue), Some(selected));
        assert_eq!(game.position().top(B, Color::Green), Some(green));
        assert_eq!(
            game.position().top(A, Color::Red),
            Some(if can_transfer { target } else { buried })
        );
        assert_eq!(
            moved_cards(&events, Verb::Transfer),
            if can_transfer { vec![target] } else { vec![] }
        );
        assert_eq!(moved_cards(&events, Verb::Meld), [selected]);
        assert_eq!(
            moved(events.last().unwrap()).source,
            Zone::Board(A, Color::Blue)
        );
    }
}

#[test]
fn compass_uses_explicit_demander_even_when_it_is_not_the_executors_opponent() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B030", A);
    let own = native(&mut f, "B012", A);
    let foreign = native(&mut f, "B015", B);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    run(&mut game, source, 0, A, A);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(moved_cards(&events, Verb::Meld), [own]);
    assert_eq!(game.position().top(B, Color::Purple), Some(foreign));
}

#[test]
fn anatomy_requires_both_original_returns_and_junks_newly_returned_fours_too() {
    fn exercise<const F: bool>() {
        for (payment, matching) in [(false, false), (true, false), (true, true)] {
            let mut f = Fixture::default();
            let source = native(&mut f, "B038", A);
            let score = payment.then(|| f.tech(Color::Purple, 4, Zone::Score(B), vec![]));
            let top = f.tech(
                Color::Red,
                if matching { 4 } else { 3 },
                Zone::Board(B, Color::Red),
                vec![],
            );
            let deck = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
            let untouched = f.tech(Color::Blue, 5, Zone::Deck(Deck::base(5)), vec![]);
            let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            run(&mut game, source, 0, B, A);
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            if matching {
                assert_eq!(moved_cards(&events, Verb::Return), [score.unwrap(), top]);
                let junk = game.position().cards(Zone::Junk);
                assert_eq!(junk.len(), 3);
                for id in [score.unwrap(), top, deck] {
                    assert!(junk.contains(&id));
                }
            } else {
                assert!(moved_cards(&events, Verb::Junk).is_empty());
                assert_eq!(game.position().top(B, Color::Red), Some(top));
                assert!(
                    game.position()
                        .cards(Zone::Deck(Deck::base(4)))
                        .contains(&deck)
                );
            }
            assert_eq!(
                game.position().cards(Zone::Deck(Deck::base(5))),
                [untouched]
            );
        }
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn anatomy_replaced_score_payment_does_not_return_a_top_or_junk_the_deck() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B038", A);
    figure(&mut f, "F069", B);
    let score = f.tech(Color::Blue, 2, Zone::Score(B), vec![]);
    let top = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let deck = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    run(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(B)), [score]);
    assert_eq!(game.position().top(B, Color::Red), Some(top));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), [deck]);
    assert!(events.is_empty());
}

#[test]
fn anatomy_original_value_is_unboxed_but_the_junked_deck_is_boxed() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B038", A);
    figure(&mut f, "F022", A);
    let score = f.tech(Color::Yellow, 2, Zone::Score(B), vec![]);
    let matching = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let wrong = f.tech(Color::Green, 3, Zone::Board(B, Color::Green), vec![]);
    let four = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let five = f.tech(Color::Purple, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    run(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(moved_cards(&events, Verb::Return), [score, matching]);
    assert_eq!(moved_cards(&events, Verb::Junk), [five]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), [four]);
    assert_eq!(game.position().top(B, Color::Green), Some(wrong));
}

#[test]
fn societies_filters_strict_same_color_values_and_preserves_every_eligible_choice() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B055", A);
    for (color, value) in [(Color::Red, 2), (Color::Yellow, 5), (Color::Blue, 3)] {
        f.tech(color, value, Zone::Board(A, color), vec![]);
    }
    let mut candidates = Vec::new();
    for (color, value) in [
        (Color::Red, 2),
        (Color::Yellow, 4),
        (Color::Green, 1),
        (Color::Blue, 4),
        (Color::Purple, 6),
    ] {
        let id = f.tech(color, value, Zone::Board(B, color), vec![]);
        if color != Color::Purple {
            f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        }
        candidates.push(id);
    }
    let reward = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    run(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let mut eligible: Vec<_> = menu
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(*card),
            _ => None,
        })
        .collect();
    eligible.sort_unstable();
    assert_eq!(eligible, [candidates[2], candidates[3]]);
    let selected=menu.options.iter().position(|option|matches!(option,Alternative::Move {target:TargetDescriptor::Known {card,..},..} if *card==candidates[3])).unwrap();
    game.choose(selected).unwrap();
    finish(&mut game, &mut events);
    assert_eq!(moved_cards(&events, Verb::Transfer), [candidates[3]]);
    assert_eq!(game.position().top(A, Color::Blue), Some(candidates[3]));
    assert_eq!(game.position().cards(Zone::Hand(B)), [reward]);
}

#[test]
fn societies_replaced_transfer_never_borrows_yis_child_success_for_its_draw() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B055", A);
    figure(&mut f, "F037", A);
    let target = f.tech(Color::Blue, 4, Zone::Board(B, Color::Blue), vec![]);
    f.cards[usize::from(target.0)].symbols[0] = Symbol::Icon(Icon::Concept);
    f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let reward = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    run(&mut game, source, 0, B, A);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert!(moved_cards(&events, Verb::Transfer).is_empty());
    assert!(moved_cards(&events, Verb::Tuck).contains(&target));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(5))), [reward]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
}

#[test]
fn industrialization_counts_live_effective_efficiency_after_all_three_tucks_and_rejects_ties() {
    for mode in 0..3 {
        let mut f = Fixture::default();
        let source = native(&mut f, "B056", A);
        if mode == 2 {
            figure(&mut f, "F064", A);
        }
        let mut rewards = Vec::new();
        for color in [Color::Red, Color::Green, Color::Blue] {
            let id = f.tech(color, 6, Zone::Deck(Deck::base(6)), vec![]);
            if mode == 1 && color == Color::Blue {
                f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Efficiency);
            }
            rewards.push(id);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(game.position().basic_icon_count(A, Icon::Efficiency), 0);
        if mode == 2 {
            assert_eq!(game.position().icon_count(A, Icon::Efficiency), 2);
        }
        run(&mut game, source, 0, A, A);
        let mut events = Vec::new();
        finish(&mut game, &mut events);
        rewards.reverse();
        assert_eq!(moved_cards(&events, Verb::Tuck), rewards);
        assert_eq!(
            moved_cards(&events, Verb::Return),
            if mode == 0 { vec![] } else { vec![source] }
        );
        assert_eq!(
            game.position().top(A, Color::Red),
            Some(if mode == 0 { source } else { rewards[2] })
        );
    }
}

#[test]
fn new_front_dependent_filters_fail_closed_for_uninspectable_hands() {
    const FILTERS: [&[Filter]; 3] = [
        &[Filter::Lacks(Test::Icon(Icon::Health))],
        &[Filter::Has(Test::AboveTop(Who::Demander))],
        &[Filter::Has(Test::Code("synthetic-base-program"))],
    ];
    for filters in FILTERS {
        for has_health in [false, true] {
            let mut f = Fixture::default();
            let source = f.tech(
                Color::Red,
                1,
                Zone::Board(A, Color::Red),
                vec![build::transfer(
                    Selector::Filtered(&Selector::Hand(Who::Opponent), filters),
                    Destination::Hand(Who::Executor),
                )],
            );
            let hidden = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
            if has_health {
                f.cards[usize::from(hidden.0)].symbols[0] = Symbol::Icon(Icon::Health);
            }
            let before = f.position();
            let mut game = Game::new(before.clone(), Expansions::BASE).unwrap();
            run(&mut game, source, 0, A, A);
            let mut events = Vec::new();
            assert_eq!(
                game.advance(100, &mut events),
                Err(Error::Unsupported("private filtered card selection"))
            );
            assert_eq!(game.position(), &before);
            assert!(events.is_empty());
        }
    }
}
