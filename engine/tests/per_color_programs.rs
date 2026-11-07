use inno_engine::*;
mod common;
use common::*;

fn native(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}
fn card(f: &mut Fixture, player: Player, color: Color, age: u8, icon: Option<Icon>) -> CardId {
    let id = f.tech(color, age, Zone::Board(player, color), vec![]);
    if let Some(icon) = icon {
        f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(icon);
    }
    id
}
fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("per-color program did not finish");
}
fn moved_cards(events: &[Event], verb: Verb) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(verb) => Some(m.card),
            _ => None,
        })
        .collect()
}

#[test]
fn engineering_transfers_one_matching_top_per_color_without_sweeping_exposed_cards() {
    for figures in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B026", A);
        let mut tops = Vec::new();
        let mut buried = Vec::new();
        for (color, age) in [(Color::Red, 1), (Color::Green, 2), (Color::Blue, 1)] {
            buried.push(card(&mut f, B, color, 1, Some(Icon::Authority)));
            tops.push(card(&mut f, B, color, age, Some(Icon::Authority)));
        }
        let excluded = card(&mut f, B, Color::Yellow, 2, Some(Icon::Industry));
        fn exercise<const F: bool>(
            f: Fixture,
            source: CardId,
            mut tops: Vec<CardId>,
            buried: Vec<CardId>,
            excluded: CardId,
        ) {
            let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            game.super_execute(source, Context::new(A)).unwrap();
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            let mut transfers = moved_cards(&events, Verb::Transfer);
            transfers.sort();
            tops.sort();
            assert_eq!(transfers, tops);
            for id in buried {
                assert_eq!(
                    game.position().top(B, game.position().card(id).color),
                    Some(id)
                );
            }
            assert_eq!(game.position().top(B, Color::Yellow), Some(excluded));
            assert_eq!(game.position().cards(Zone::Score(A)).len(), 3);
        }
        if figures {
            exercise::<true>(f, source, tops, buried, excluded);
        } else {
            exercise::<false>(f, source, tops, buried, excluded);
        }
    }
}

#[test]
fn canning_optional_draw_gates_tuck_and_scoring_but_not_the_later_yellow_splay() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        card(&mut f, A, Color::Yellow, 1, Some(Icon::Industry));
        let source = native(&mut f, "B058", A);
        let under_red = card(&mut f, A, Color::Red, 1, None);
        let red = card(&mut f, A, Color::Red, 2, None);
        let under_green = card(&mut f, A, Color::Green, 3, None);
        let green = card(&mut f, A, Color::Green, 4, None);
        let blue = card(&mut f, A, Color::Blue, 1, Some(Icon::Industry));
        let drawn = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options,
            [
                Alternative::Draw {
                    value: 6,
                    set: CardSet::Base
                },
                Alternative::Decline
            ]
        );
        game.choose(usize::from(!accept)).unwrap();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().splay(A, Color::Yellow), Splay::Right);
        assert_eq!(game.position().top(A, Color::Blue), Some(blue));
        if accept {
            assert_eq!(moved_cards(&events, Verb::Tuck), [drawn]);
            assert_eq!(moved_cards(&events, Verb::Score), [red, green]);
            assert_eq!(
                game.position().cards(Zone::Board(A, Color::Red)),
                [drawn, under_red]
            );
            assert_eq!(game.position().top(A, Color::Green), Some(under_green));
        } else {
            assert!(moved_cards(&events, Verb::Tuck).is_empty());
            assert!(moved_cards(&events, Verb::Score).is_empty());
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(6))), [drawn]);
        }
    }
}

#[test]
fn canning_scores_nothing_when_napoleon_replaces_its_tuck() {
    // "If you tuck a card": Napoleon melds the drawn card instead, so the
    // tuck didn't happen and Canning scores no top card.
    let mut f = Fixture::default();
    let source = native(&mut f, "B058", A);
    f.figure(Figure::NapoleonBonaparte, Zone::Board(A, Color::Red));
    let blue = card(&mut f, A, Color::Blue, 2, None);
    let drawn = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(moved_cards(&events, Verb::Meld), [drawn]);
    assert!(moved_cards(&events, Verb::Tuck).is_empty());
    // Only Napoleon's own score of a top [5] or [6].
    assert_eq!(moved_cards(&events, Verb::Score).len(), 1);
    assert_eq!(game.position().top(A, Color::Blue), Some(blue));
}

#[test]
fn electricity_draws_once_per_original_return_not_per_first_reaction_draw() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B071", A);
    let red = card(&mut f, A, Color::Red, 1, None);
    let blue = card(&mut f, A, Color::Blue, 2, None);
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(B, Color::Blue));
    let second = f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let first = f.tech(Color::Red, 8, Zone::Deck(Deck::base(8)), vec![]);
    let second_child = f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]);
    let first_child = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, red);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(moved_cards(&events, Verb::Return), [red, blue]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [first, second]);
    assert_eq!(
        game.position().cards(Zone::Hand(B)),
        [first_child, second_child]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Moved(m) if m.inside_karma))
            .count(),
        2
    );
}

#[test]
fn electricity_with_no_eligible_top_does_not_attempt_a_reward_draw() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B071", A);
    let blue = card(&mut f, A, Color::Blue, 1, Some(Icon::Industry));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().top(A, Color::Blue), Some(blue));
}

#[test]
fn whataboutism_moves_only_top_demands_then_exchanges_scores_in_one_commit() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B115", A);
    let buried = native(&mut f, "B003", B);
    let red = native(&mut f, "B001", B);
    let green = native(&mut f, "B021", B);
    let excluded = native(&mut f, "B012", B);
    let own = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    let other = f.tech(Color::Green, 4, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.super_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(moved_cards(&events, Verb::Transfer), [red, green]);
    assert_eq!(game.position().top(A, Color::Red), Some(red));
    assert_eq!(game.position().top(B, Color::Red), Some(buried));
    assert_eq!(game.position().top(B, Color::Blue), Some(excluded));
    assert_eq!(game.position().cards(Zone::Score(A)), [other]);
    assert_eq!(game.position().cards(Zone::Score(B)), [own]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Exchanged { .. }))
            .count(),
        1
    );
}

#[test]
fn whataboutism_empty_demand_does_not_exchange_unrelated_score_piles() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B115", A);
    native(&mut f, "B012", B);
    let own = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    let other = f.tech(Color::Green, 4, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.super_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Score(A)), [own]);
    assert_eq!(game.position().cards(Zone::Score(B)), [other]);
}

#[test]
fn whataboutism_requeries_michelangelo_after_covering_him_before_exchange() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B115", A);
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let yellow = native(&mut f, "B098", B);
    let hand = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let own = f.tech(Color::Blue, 4, Zone::Score(A), vec![]);
    let other = f.tech(Color::Green, 5, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.super_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().top(A, Color::Yellow), Some(yellow));
    assert!(!game.position().has_figure(A, Figure::Michelangelo));
    assert_eq!(game.position().cards(Zone::Hand(A)), [hand]);
    assert_eq!(game.position().cards(Zone::Score(A)), [other]);
    assert_eq!(game.position().cards(Zone::Score(B)), [own]);
}

#[test]
fn whataboutism_exchange_includes_still_active_logical_score_hand_members() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B115", A);
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    native(&mut f, "B001", B);
    let hand = f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
    let own = f.tech(Color::Blue, 4, Zone::Score(A), vec![]);
    let other = f.tech(Color::Green, 5, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.super_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    let mut received = game.position().cards(Zone::Score(A)).to_vec();
    received.sort();
    let mut expected = vec![hand, other];
    expected.sort();
    assert_eq!(received, expected);
    assert_eq!(game.position().cards(Zone::Score(B)), [own]);
}

#[test]
fn replaced_per_color_transfers_consume_each_color_once_and_do_not_unlock_the_exchange() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B115", A);
    let yi = figure_card("F037").unwrap();
    let yi_color = yi.color;
    f.add(yi, Zone::Board(A, yi_color));
    let red = f.add(base_card("B037").unwrap(), Zone::Board(B, Color::Red));
    let purple = f.add(base_card("B065").unwrap(), Zone::Board(B, Color::Purple));
    // These native demands and the child draws have no Authority. Yi cannot
    // score a still-unconsidered demand before its own transfer is attempted.
    for color in [Color::Yellow, Color::Blue] {
        f.tech(color, 4, Zone::Deck(Deck::base(4)), vec![]);
    }
    let a_score = f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    let b_score = f.tech(Color::Yellow, 2, Zone::Score(B), vec![]);
    let mut context = Context::new(B);
    context.demander = A;
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(moved_cards(&events, Verb::Transfer).is_empty());
    for id in [red, purple] {
        assert_eq!(
            moved_cards(&events, Verb::Tuck)
                .iter()
                .filter(|&&card| card == id)
                .count(),
            1
        );
    }
    assert_eq!(game.position().cards(Zone::Score(A)), [a_score]);
    assert_eq!(game.position().cards(Zone::Score(B)), [b_score]);
}

#[test]
fn electricity_keeps_its_boxed_age_after_returning_archimedes_but_does_not_boost_the_count() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B071", A);
    let archimedes = figure_card("F022").unwrap();
    let color = archimedes.color;
    let figure = f.add(archimedes, Zone::Board(A, color));
    let red = card(&mut f, A, Color::Red, 1, None);
    let purple = card(&mut f, A, Color::Purple, 1, None);
    let mut rewards = Vec::new();
    for color in [Color::Red, Color::Green, Color::Blue] {
        rewards.push(f.tech(color, 9, Zone::Deck(Deck::base(9)), vec![]));
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    let returned = moved_cards(&events, Verb::Return);
    assert_eq!(returned.len(), 3);
    for card in [red, purple, figure] {
        assert!(returned.contains(&card));
    }
    rewards.reverse();
    assert_eq!(game.position().cards(Zone::Hand(A)), rewards);
    assert!(!game.position().has_figure(A, Figure::Archimedes));
}
