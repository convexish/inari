//! Expected outcomes for the Age 0 programs, one scenario per printed
//! effect (and per "you lose" branch), on introduced Age 0 positions.
use inno_engine::*;

mod common;
use common::*;

/// A blank synthetic card with these printed icons in its first slots.
fn card(f: &mut Fixture, color: Color, value: u8, zone: Zone, icons: &[Icon]) -> CardId {
    let id = f.tech(color, value, zone, vec![]);
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(id.0)].symbols[slot] = Symbol::Icon(icon);
    }
    id
}

fn zero(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(age_zero_card(code).unwrap(), zone)
}

/// The printed card on top of its own color on that player's board.
fn top(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let color = age_zero_card(code).unwrap().color;
    zero(f, code, Zone::Board(player, color))
}

/// Execute one effect as `executor`, demanded by the other player.
fn effect(f: Fixture, card: CardId, effect: u16, executor: Player) -> Game {
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(executor);
    context.demander = executor.opponent();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    game
}

/// Resolve every decision with `pick`, defaulting to the first option.
fn run(game: &mut Game, mut pick: impl FnMut(&Decision) -> Option<usize>) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    for _ in 0..200 {
        match settle(game, &mut events) {
            Status::Decision(menu) => {
                let index = pick(&menu).unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return (status, events);
            }
        }
    }
    panic!("scenario did not finish");
}

fn first(game: &mut Game) -> (Status, Vec<Event>) {
    run(game, |_| None)
}

fn cards(game: &Game, zone: Zone) -> Vec<CardId> {
    game.position().cards(zone).to_vec()
}

fn revealed(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

#[test]
fn fresh_water_splays_an_unsplayed_color_left_or_its_executor_loses() {
    for splayable in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z001", A);
        if splayable {
            card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
            card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
        }
        let mut game = effect(f, source, 0, A);
        let (status, _) = first(&mut game);
        if splayable {
            assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
        } else {
            assert_eq!(status, Status::Winner(B));
        }
    }
}

#[test]
fn shelter_demands_a_score_and_exchanges_a_hand_for_two_score_cards() {
    for behind in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z002", B);
        let scored = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
        let theirs = if behind { 2 } else { 1 };
        for _ in 0..theirs {
            card(&mut f, Color::Blue, 1, Zone::Score(B), &[]);
        }
        let mut game = effect(f, source, 0, A);
        let (status, events) = first(&mut game);
        assert_eq!(moved(&events[0]).card, scored);
        // Elimination removes the loser's cards.
        assert_eq!(status == Status::Winner(B), behind);
    }

    let mut f = Fixture::default();
    let source = top(&mut f, "Z002", A);
    let hand = [1, 2].map(|v| card(&mut f, Color::Red, v, Zone::Hand(A), &[]));
    let score = [3, 4, 5].map(|v| card(&mut f, Color::Blue, v, Zone::Score(A), &[]));
    let mut game = effect(f, source, 1, A);
    first(&mut game);
    let now = cards(&game, Zone::Hand(A));
    assert_eq!(now.len(), 2);
    assert!(now.iter().all(|c| score.contains(c)));
    let pile = cards(&game, Zone::Score(A));
    assert_eq!(pile.len(), 3);
    assert!(hand.iter().all(|c| pile.contains(c)));
}

#[test]
fn skinning_scores_a_food_top_and_melds_one_card_per_food_icon() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z003", A);
    let food = card(
        &mut f,
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        &[Icon::Food, Icon::Food],
    );
    for color in [Color::Blue, Color::Green, Color::Purple] {
        card(&mut f, color, 1, Zone::Hand(A), &[]);
    }
    let mut game = effect(f, source, 0, A);
    run(&mut game, |menu| {
        menu.options.iter().position(|option| {
            matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == food)
        })
    });
    assert_eq!(cards(&game, Zone::Score(A)), [food]);
    assert_eq!(cards(&game, Zone::Hand(A)).len(), 1);
}

#[test]
fn fishing_takes_the_drawn_color_from_all_hands_and_rewards_fresh_water_and_triples() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z004", A);
    card(&mut f, Color::Blue, 2, Zone::Deck(Deck::base(2)), &[]);
    card(&mut f, Color::Blue, 1, Zone::Deck(Deck::base(1)), &[]);
    let water = zero(&mut f, "Z001", Zone::Deck(Deck::base(0)));
    let own = card(&mut f, Color::Yellow, 1, Zone::Hand(A), &[]);
    let taken = [1, 2].map(|_| card(&mut f, Color::Yellow, 1, Zone::Hand(B), &[]));
    let kept = card(&mut f, Color::Red, 1, Zone::Hand(B), &[]);
    let mut game = effect(f, source, 0, A);
    let (_, events) = first(&mut game);
    assert_eq!(game.position().top(A, Color::Yellow), Some(water));
    assert_eq!(cards(&game, Zone::Hand(B)), [kept]);
    let hand = cards(&game, Zone::Hand(A));
    assert!(hand.contains(&own) && taken.iter().all(|c| hand.contains(c)));
    // The Fresh Water [1] and the three-yellow [2].
    assert_eq!(hand.len(), 5);
    let shown = revealed(&events);
    assert!(shown.contains(&own) && shown.contains(&kept));
}

#[test]
fn fishing_under_yi_sun_sin_lets_the_executor_order_the_revealed_transfers() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z004", A);
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    card(&mut f, Color::Yellow, 0, Zone::Deck(Deck::base(0)), &[]);
    let taken = [1, 2].map(|value| card(&mut f, Color::Yellow, value, Zone::Hand(B), &[]));
    card(&mut f, Color::Red, 1, Zone::Hand(B), &[]);
    for _ in 0..2 {
        card(&mut f, Color::Green, 4, Zone::Deck(Deck::base(4)), &[]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.demander = B;
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
    // Each transfer becomes a tuck, a draw and a score: a real order.
    let menu = decision(&mut game, &mut events);
    let offered: Vec<_> = menu
        .options
        .iter()
        .map(|option| match option {
            Alternative::Move {
                verb: Verb::Transfer,
                target: TargetDescriptor::Known { card, .. },
            } => *card,
            other => panic!("expected a known transfer, got {other:?}"),
        })
        .collect();
    assert_eq!(offered, taken);
    choose_card(&mut game, &mut events, taken[1]);
    let (_, rest) = run(&mut game, |_| None);
    events.extend(rest);
    let tucked: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Tuck) && taken.contains(&m.card) => {
                Some(m.card)
            }
            _ => None,
        })
        .collect();
    assert_eq!(tucked, [taken[1], taken[0]]);
}

#[test]
fn foraging_may_score_a_drawn_card_without_food_and_draw_again() {
    for food in [false, true] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z005", A);
        let second = card(&mut f, Color::Red, 0, Zone::Deck(Deck::base(0)), &[]);
        let icons: &[Icon] = if food { &[Icon::Food] } else { &[] };
        let drawn = card(&mut f, Color::Blue, 0, Zone::Deck(Deck::base(0)), icons);
        let mut game = effect(f, source, 0, A);
        first(&mut game);
        if food {
            assert_eq!(cards(&game, Zone::Hand(A)), [drawn]);
        } else {
            assert_eq!(cards(&game, Zone::Score(A)), [drawn]);
            assert_eq!(cards(&game, Zone::Hand(A)), [second]);
        }
    }
}

#[test]
fn hunting_transfers_the_demanders_choice_of_two_drawn_cards() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z006", B);
        let hidden = zero(&mut f, "Z002", Zone::Hand(A));
        zero(&mut f, "Z003", Zone::Deck(Deck::base(0)));
        let later = zero(&mut f, "Z001", Zone::Deck(Deck::base(0)));
        let earlier = zero(&mut f, "Z005", Zone::Deck(Deck::base(0)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        let mut context = Context::new(A);
        context.demander = B;
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        let expected = [earlier, later].map(|card| Alternative::Select {
            target: TargetDescriptor::Known {
                card,
                source: Zone::Hand(A),
            },
        });
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        assert_eq!(menu.options, expected);
        assert_eq!(revealed(&events), [earlier, later]);

        let mut worlds = vec![game.clone(), game.clone()];
        if tracked {
            assert!(
                !game
                    .tracker(B)
                    .unwrap()
                    .belief(Zone::Hand(A))
                    .iter()
                    .any(|slot| slot.known() == Some(hidden))
            );
            let observation = game.observe(B).unwrap();
            let mut permuted = false;
            for seed in 0..16 {
                let world = game.determinize(B, seed).unwrap();
                permuted |= cards(&world, Zone::Hand(A)) != cards(&game, Zone::Hand(A));
                assert_eq!(world.observe(B).unwrap(), observation);
                worlds.push(world);
            }
            assert!(permuted, "samples must vary the same-back hand");
        }
        for (index, mut world) in worlds.into_iter().enumerate() {
            let menu = decision(&mut world, &mut Vec::new());
            assert_eq!(menu.player, B);
            assert_eq!(menu.options, expected);
            let choice = index % 2;
            let chosen = [earlier, later][choice];
            let other = [later, earlier][choice];
            world.choose(choice).unwrap();
            assert!(matches!(first(&mut world).0, Status::Complete(_)));
            let color = world.position().card(chosen).color;
            assert_eq!(world.position().top(B, color), Some(chosen));
            assert!(cards(&world, Zone::Hand(A)).contains(&other));
            assert_eq!(cards(&world, Zone::Hand(A)).len(), 2);
            if tracked {
                for player in Player::ALL {
                    assert!(world.tracker(player).unwrap().check_truth(world.position()));
                }
            }
        }
    }
}

#[test]
fn fire_demands_a_qualifying_reveal_and_requires_a_matching_meld() {
    for (value, loses) in [(0, true), (1, false)] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z007", B);
        let shown = card(&mut f, Color::Red, value, Zone::Hand(A), &[]);
        if value == 0 {
            // Red is on the demander's board, so a red 0 does not qualify.
            card(&mut f, Color::Red, 1, Zone::Board(B, Color::Red), &[]);
        }
        let mut game = effect(f, source, 0, A);
        let (status, events) = first(&mut game);
        assert_eq!(status == Status::Winner(B), loses);
        assert_eq!(revealed(&events).contains(&shown), !loses);
    }
    for matching in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z007", A);
        let color = if matching { Color::Red } else { Color::Blue };
        let melded = card(&mut f, color, 1, Zone::Hand(A), &[]);
        let mut game = effect(f, source, 1, A);
        let (status, _) = first(&mut game);
        assert_eq!(status == Status::Winner(B), !matching);
        assert_eq!(game.position().top(A, Color::Red) == Some(melded), matching);
    }
}

#[test]
fn stone_knives_returns_a_drawn_zero_and_takes_that_colors_top_card() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z008", B);
    let drawn = card(&mut f, Color::Blue, 0, Zone::Deck(Deck::base(0)), &[]);
    let taken = card(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue), &[]);
    let mut game = effect(f, source, 0, A);
    let (_, events) = first(&mut game);
    assert_eq!(revealed(&events), [drawn]);
    assert_eq!(cards(&game, Zone::Deck(Deck::base(0))), [drawn]);
    assert_eq!(game.position().top(B, Color::Blue), Some(taken));

    for skinning in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z008", A);
        if skinning {
            top(&mut f, "Z003", A);
        }
        card(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)), &[]);
        let mut game = effect(f, source, 1, A);
        first(&mut game);
        assert_eq!(cards(&game, Zone::Hand(A)).len(), usize::from(skinning));
    }
}

#[test]
fn trapping_reveals_and_transfers_hand_cards_of_the_demanders_score_colors() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z009", B);
    let marker = card(&mut f, Color::Blue, 2, Zone::Score(B), &[]);
    let taken = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    let kept = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
    let mut game = effect(f, source, 0, A);
    let (_, events) = first(&mut game);
    let shown = revealed(&events);
    assert!([marker, taken, kept].iter().all(|c| shown.contains(c)));
    assert_eq!(cards(&game, Zone::Hand(B)), [taken]);
    assert_eq!(cards(&game, Zone::Hand(A)), [kept]);
}

#[test]
fn speak_with_bird_wins_with_five_top_zeros() {
    for zeros in [5, 4] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z010", A);
        for &color in &Color::ALL[..zeros] {
            if color != Color::Purple {
                card(&mut f, color, 0, Zone::Board(A, color), &[]);
            }
        }
        if zeros == 4 {
            card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
        }
        let mut game = effect(f, source, 0, A);
        let (status, _) = first(&mut game);
        assert_eq!(status == Status::Winner(A), zeros == 5);
    }
}

#[test]
fn tanning_scores_two_and_trades_a_score_card_for_the_opponents_bottom_card() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z011", A);
    for color in [Color::Red, Color::Blue, Color::Green] {
        card(&mut f, color, 1, Zone::Hand(A), &[]);
    }
    let mut game = effect(f, source, 0, A);
    first(&mut game);
    assert_eq!(cards(&game, Zone::Score(A)).len(), 2);
    assert_eq!(cards(&game, Zone::Hand(A)).len(), 1);

    let mut f = Fixture::default();
    let source = top(&mut f, "Z011", A);
    let given = card(&mut f, Color::Red, 2, Zone::Score(A), &[]);
    let bottom = card(&mut f, Color::Red, 1, Zone::Board(B, Color::Red), &[]);
    let middle = card(&mut f, Color::Red, 3, Zone::Board(B, Color::Red), &[]);
    let mut game = effect(f, source, 1, A);
    first(&mut game);
    assert_eq!(cards(&game, Zone::Board(B, Color::Red)), [middle, given]);
    assert_eq!(game.position().top(A, Color::Red), Some(bottom));
}

#[test]
fn tracking_draws_two_zeros_and_returns_one_of_them() {
    let mut f = Fixture::default();
    let source = top(&mut f, "Z012", A);
    let pair =
        [Color::Red, Color::Blue].map(|c| card(&mut f, c, 0, Zone::Deck(Deck::base(0)), &[]));
    let mut game = effect(f, source, 0, A);
    first(&mut game);
    let hand = cards(&game, Zone::Hand(A));
    let deck = cards(&game, Zone::Deck(Deck::base(0)));
    assert_eq!((hand.len(), deck.len()), (1, 1));
    assert!(pair.contains(&hand[0]) && pair.contains(&deck[0]));
}

#[test]
fn curing_scores_another_food_top_or_loses_and_may_exchange_for_the_highest() {
    for other in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z013", A);
        let food = other.then(|| {
            card(
                &mut f,
                Color::Red,
                1,
                Zone::Board(A, Color::Red),
                &[Icon::Food],
            )
        });
        let mut game = effect(f, source, 0, A);
        let (status, _) = first(&mut game);
        assert_eq!(status == Status::Winner(B), !other);
        // Curing itself shows [Food] but is not "another" top card.
        assert_eq!(
            cards(&game, Zone::Score(A)),
            food.into_iter().collect::<Vec<_>>()
        );
    }

    let mut f = Fixture::default();
    let source = top(&mut f, "Z013", A);
    let mine = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
    let highest = [5, 5].map(|v| card(&mut f, Color::Blue, v, Zone::Score(B), &[]));
    let low = card(&mut f, Color::Green, 2, Zone::Score(B), &[]);
    let mut game = effect(f, source, 1, A);
    first(&mut game);
    let hand = cards(&game, Zone::Hand(A));
    assert_eq!(hand.len(), 2);
    assert!(highest.iter().all(|c| hand.contains(c)));
    let theirs = cards(&game, Zone::Score(B));
    assert!(theirs.contains(&mine) && theirs.contains(&low));
}

#[test]
fn first_aid_melds_the_lone_card_of_the_returned_color_or_loses() {
    for matching in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z014", A);
        let returned = card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
        let color = if matching { Color::Red } else { Color::Green };
        let drawn = card(&mut f, color, 0, Zone::Deck(Deck::base(0)), &[]);
        let mut game = effect(f, source, 0, A);
        let (status, events) = run(&mut game, |menu| {
            menu.options.iter().position(|option| {
                matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == returned)
            })
        });
        assert!(revealed(&events).contains(&drawn));
        assert_eq!(status == Status::Winner(B), !matching);
        if matching {
            assert_eq!(game.position().top(A, Color::Red), Some(drawn));
        }
    }
}

#[test]
fn knots_reveals_a_score_card_of_a_board_color_to_draw_a_one() {
    for on_board in [true, false] {
        let mut f = Fixture::default();
        let source = top(&mut f, "Z015", A);
        let color = if on_board { Color::Blue } else { Color::Red };
        card(&mut f, color, 3, Zone::Score(A), &[]);
        card(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)), &[]);
        let mut game = effect(f, source, 0, A);
        first(&mut game);
        assert_eq!(cards(&game, Zone::Hand(A)).len(), usize::from(on_board));
    }
}
