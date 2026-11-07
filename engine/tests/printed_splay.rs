use inno_engine::*;

mod common;
use common::*;

fn pile(f: &mut Fixture, player: Player, color: Color, count: usize) {
    for _ in 0..count {
        f.tech(color, 1, Zone::Board(player, color), vec![]);
    }
}

fn effect(game: &mut Game, card: CardId, index: u16, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn set_splay(game: &mut Game, player: Player, color: Color, direction: Splay) {
    game.start(
        Operation::Splay {
            player,
            color,
            direction,
        },
        Context::new(player),
    )
    .unwrap();
    assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
}

fn choose(game: &mut Game, events: &mut Vec<Event>, option: Alternative) {
    let menu = decision(game, events);
    game.choose(menu.options.iter().position(|o| *o == option).unwrap())
        .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("printed program did not complete");
}

#[test]
fn philosophy_offers_only_nonempty_nonsingleton_non_noop_colors_then_separate_scoring() {
    let mut f = Fixture::default();
    let card = f.add(base_card("B025").unwrap(), Zone::Board(A, Color::Purple));
    pile(&mut f, A, Color::Red, 2);
    pile(&mut f, A, Color::Blue, 2);
    pile(&mut f, A, Color::Green, 1);
    let hand = f.tech(Color::Yellow, 3, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    set_splay(&mut game, A, Color::Blue, Splay::Left);
    game.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        decision(&mut game, &mut events).options,
        [
            Alternative::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Left
            },
            Alternative::Decline
        ]
    );
    choose(&mut game, &mut events, Alternative::Decline);
    let mut declined = game.clone();
    choose(&mut declined, &mut Vec::new(), Alternative::Decline);
    assert_eq!(
        settle(&mut declined, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    choose_card(&mut game, &mut events, hand);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[hand]);
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(events.len(), 1);
}

#[test]
fn metric_and_flight_check_before_their_later_splay_not_retroactively() {
    for (code, key, direction) in [
        ("B061", Color::Green, Splay::Right),
        ("B076", Color::Red, Splay::Up),
    ] {
        let mut f = Fixture::default();
        pile(&mut f, A, key, 1);
        let card = f.add(base_card(code).unwrap(), Zone::Board(A, key));
        pile(&mut f, A, Color::Blue, 2);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(card, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            decision(&mut game, &mut events).options,
            [
                Alternative::Splay {
                    player: A,
                    color: key,
                    direction
                },
                Alternative::Decline
            ]
        );
        game.choose(0).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
        game.self_execute(card, Context::new(A)).unwrap();
        assert_eq!(
            decision(&mut game, &mut events).options,
            [
                Alternative::Splay {
                    player: A,
                    color: Color::Blue,
                    direction
                },
                Alternative::Decline
            ]
        );
        game.choose(0).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().splay(A, Color::Blue), direction);
    }
}

#[test]
fn railroad_current_direction_filter_is_right_only_and_optional() {
    let mut f = Fixture::default();
    let card = f.add(base_card("B075").unwrap(), Zone::Board(A, Color::Purple));
    for color in [Color::Red, Color::Blue, Color::Green, Color::Yellow] {
        pile(&mut f, A, color, 2);
    }
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    set_splay(&mut game, A, Color::Red, Splay::Right);
    set_splay(&mut game, A, Color::Blue, Splay::Left);
    set_splay(&mut game, A, Color::Green, Splay::Up);
    effect(&mut game, card, 2, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(
        decision(&mut game, &mut events).options,
        [
            Alternative::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Up
            },
            Alternative::Decline
        ]
    );
    let mut declined = game.clone();
    choose(&mut declined, &mut Vec::new(), Alternative::Decline);
    assert_eq!(
        settle(&mut declined, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(declined.position().splay(A, Color::Red), Splay::Right);
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
}

#[test]
fn atomic_theory_draw_and_meld_remain_separate_and_match_in_both_kernels() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Blue, 1);
    let card = f.add(base_card("B062").unwrap(), Zone::Board(A, Color::Blue));
    let drawn = f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    base.self_execute(card, Context::new(A)).unwrap();
    figures.self_execute(card, Context::new(A)).unwrap();
    let mut a = Vec::new();
    let mut b = Vec::new();
    assert_eq!(
        finish(&mut base, &mut a),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        finish(&mut figures, &mut b),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(a, b);
    assert_eq!(base.position(), figures.position());
    assert_eq!(base.position().splay(A, Color::Blue), Splay::Right);
    assert_eq!(moved(&a[1]).card, drawn);
    assert_eq!(moved(&a[1]).destination, Zone::Hand(A));
    assert_eq!(moved(&a[2]).verb, Some(Verb::Meld));
    assert_eq!(base.position().top(A, Color::Red), Some(drawn));
}

#[test]
fn emancipation_demand_transfers_to_demander_then_draws_only_on_success() {
    for has_hand in [false, true] {
        let mut f = Fixture::default();
        let card = f.add(base_card("B065").unwrap(), Zone::Board(A, Color::Purple));
        let hand = has_hand.then(|| f.tech(Color::Blue, 3, Zone::Hand(B), vec![]));
        let drawn = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        let mut context = Context::new(B);
        context.demander = A;
        context.source_owner = A;
        effect(&mut game, card, 0, context);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().cards(Zone::Score(A)), hand.as_slice());
        assert_eq!(events.len(), if has_hand { 2 } else { 0 });
        if has_hand {
            assert_eq!(game.position().cards(Zone::Hand(B)), &[drawn]);
        }
    }
}

#[test]
fn emancipation_red_or_purple_is_one_color_not_both() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Purple, 1);
    let card = f.add(base_card("B065").unwrap(), Zone::Board(A, Color::Purple));
    pile(&mut f, A, Color::Red, 2);
    pile(&mut f, A, Color::Blue, 2);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, card, 1, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(
        decision(&mut game, &mut events).options,
        [
            Alternative::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Right
            },
            Alternative::Splay {
                player: A,
                color: Color::Purple,
                direction: Splay::Right
            },
            Alternative::Decline,
        ]
    );
    game.choose(1).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(game.position().splay(A, Color::Purple), Splay::Right);
}

#[test]
fn railroad_has_separate_return_draw_and_splay_effects_and_three_is_not_boxed() {
    for boosted in [false, true] {
        let mut f = Fixture::default();
        let card = f.add(base_card("B075").unwrap(), Zone::Board(A, Color::Purple));
        if boosted {
            f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
        }
        let hand: Vec<_> = (1..=4)
            .map(|age| f.tech(Color::Yellow, age, Zone::Hand(A), vec![]))
            .collect();
        let sixes: Vec<_> = (0..3)
            .map(|_| f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]))
            .collect();
        let sevens: Vec<_> = (0..3)
            .map(|_| f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]))
            .collect();
        pile(&mut f, A, Color::Red, 2);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        set_splay(&mut game, A, Color::Red, Splay::Right);
        game.self_execute(card, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), 8);
        assert_eq!(
            events[..4]
                .iter()
                .map(|e| moved(e).card)
                .collect::<Vec<_>>(),
            hand
        );
        let drawn = if boosted { &sevens } else { &sixes };
        assert_eq!(
            game.position().cards(Zone::Hand(A)),
            drawn.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
    }
}

#[test]
fn internet_scores_one_then_draws_and_melds_each_of_two_sequentially() {
    let mut f = Fixture::default();
    let card = f.add(base_card("B105").unwrap(), Zone::Board(A, Color::Purple));
    pile(&mut f, A, Color::Green, 2);
    let draws: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    base.self_execute(card, Context::new(A)).unwrap();
    figures.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let mut other = Vec::new();
    assert_eq!(
        finish(&mut base, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        finish(&mut figures, &mut other),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events, other);
    assert_eq!(base.position(), figures.position());
    assert_eq!(base.position().cards(Zone::Score(A)), &[draws[2]]);
    assert_eq!(
        base.position().cards(Zone::Board(A, Color::Red)),
        &[draws[1], draws[0]]
    );
    assert_eq!(
        events[1..]
            .iter()
            .map(|e| (moved(e).card, moved(e).verb))
            .collect::<Vec<_>>(),
        [
            (draws[2], None),
            (draws[2], Some(Verb::Score)),
            (draws[1], None),
            (draws[1], Some(Verb::Meld)),
            (draws[0], None),
            (draws[0], Some(Verb::Meld)),
        ]
    );
}

#[test]
fn shared_philosophy_splays_each_executors_board_and_scores_their_own_hand() {
    let mut f = Fixture::default();
    let card = f.add(base_card("B025").unwrap(), Zone::Board(A, Color::Purple));
    pile(&mut f, A, Color::Red, 2);
    pile(&mut f, B, Color::Blue, 1);
    let opposing = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    f.cards[usize::from(opposing.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
    let own = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let foreign = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let reward = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, B);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().splay(B, Color::Blue), Splay::Left);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
    assert_eq!(game.position().cards(Zone::Score(A)), &[own]);
    assert_eq!(game.position().cards(Zone::Score(B)), &[foreign]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
}
