//! Age 11 Artifacts (A106-A115): expected outcomes on explicit positions.
use super::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

fn base_deck(age: u8) -> Zone {
    Zone::Deck(Deck::base(age))
}

fn board(player: Player, color: Color) -> Zone {
    Zone::Board(player, color)
}

fn sharing_icons(code: &str) -> [Icon; 3] {
    [artifacts_card(code).unwrap().featured.unwrap(); 3]
}

/// A synthetic Base card with one non-demand effect.
fn with_effect(mut card: Card, instructions: Vec<Instruction>) -> Card {
    card.set = CardSet::Base;
    card.code = "synthetic-base-program";
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    card
}

fn any(game: &mut Game) -> (Status, Vec<Event>) {
    play(game, |_| Some(0))
}

/// The option selecting this card, by its known identity.
fn option_selecting(decision: &Decision, card: CardId) -> Option<usize> {
    decision.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } } if *c == card
        )
    })
}

fn pick_value(decision: &Decision, value: u8) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Value(value))
}

#[test]
fn sixteen_psyche_offers_values_absent_from_the_tops_and_scores_that_deck_and_junk() {
    let mut l = Layout::default();
    let psyche = l.artifact("A106", board(A, Color::Red));
    l.tech(Color::Blue, 3, board(A, Color::Blue), &[]);
    let deck_low = l.tech(Color::Green, 5, base_deck(5), &[]);
    let deck_top = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let junked = l.tech(Color::Purple, 5, Zone::Junk, &[]);
    l.tech(Color::Purple, 6, Zone::Junk, &[]);
    let mut game = l.game();
    dogma(&mut game, psyche);
    let mut offered = Vec::new();
    let (status, events) = play(&mut game, |decision| {
        if decision.options.contains(&Alternative::Value(5)) {
            offered = decision.options.clone();
            return pick_value(decision, 5);
        }
        Some(0)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // Red (Psyche's 11) and blue 3 are tops: 11 and 3 are not offered. The
    // [5] deck and the junked [5] and [6] can be scored; every other value
    // has nothing to score and is offered once.
    assert_eq!(offered, [1, 5, 6].map(Alternative::Value).to_vec());
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Score), deck_top, base_deck(5), SCORE_A),
            (Some(Verb::Score), deck_low, base_deck(5), SCORE_A),
            (Some(Verb::Score), junked, Zone::Junk, SCORE_A),
        ]
    );
}

#[test]
fn pizza_pacaya_junks_the_victims_board_then_draws_and_melds_every_value() {
    let mut l = Layout::default();
    let pizza = l.artifact("A107", board(A, Color::Red));
    let b_top = l.tech(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A107"),
    );
    let colors = [
        Color::Blue,
        Color::Green,
        Color::Yellow,
        Color::Purple,
        Color::Red,
    ];
    let drawn: Vec<_> = (1..=11)
        .map(|value| l.tech(colors[usize::from(value) % 5], value, base_deck(value), &[]))
        .collect();
    let mut game = l.game();
    dogma(&mut game, pizza);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (Some(Verb::Junk), b_top, board(B, Color::Blue), Zone::Junk)
    );
    for (index, &card) in drawn.iter().enumerate() {
        let value = index as u8 + 1;
        assert_eq!(moved[1 + 2 * index], (None, card, base_deck(value), HAND_B));
        assert_eq!(moved[2 + 2 * index].0, Some(Verb::Meld));
        assert_eq!(moved[2 + 2 * index].1, card);
    }
    assert_eq!(moved.len(), 23);
}

#[test]
fn what_does_the_fox_say_melds_both_elevens_and_executes_the_second_on_its_turn() {
    let mut l = Layout::default();
    let fox = l.artifact("A108", board(A, Color::Yellow));
    let second = l.add(
        with_effect(blank(Color::Blue, 11), vec![build::draw(1)]),
        base_deck(11),
    );
    let first = l.add(
        with_effect(blank(Color::Green, 11), vec![build::draw(2)]),
        base_deck(11),
    );
    let one = l.tech(Color::Red, 1, base_deck(1), &[]);
    let two = l.tech(Color::Red, 2, base_deck(2), &[]);
    let mut game = l.game();
    dogma(&mut game, fox);
    let (status, events) = play(&mut game, |decision| option_moving(decision, first));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, first, base_deck(11), HAND_A),
            (None, second, base_deck(11), HAND_A),
            (Some(Verb::Meld), first, HAND_A, board(A, Color::Green)),
            (Some(Verb::Meld), second, HAND_A, board(A, Color::Blue)),
            (None, one, base_deck(1), HAND_A),
        ]
    );
    assert_eq!(game.position().cards(base_deck(2)), &[two]);
}

#[test]
fn tasmanian_tiger_exchanges_a_scored_card_with_a_matching_top_and_repeats_on_two_returns() {
    let mut l = Layout::default();
    let tiger = l.artifact("A109", board(A, Color::Yellow));
    let scored = l.tech(Color::Red, 3, SCORE_A, &[]);
    let b_low = l.tech(Color::Red, 1, board(B, Color::Red), &[]);
    let b_top = l.tech(Color::Red, 7, board(B, Color::Red), &[]);
    let hand_one = l.tech(Color::Blue, 2, HAND_A, &[]);
    let hand_two = l.tech(Color::Green, 4, HAND_A, &[]);
    let mut game = l.game();
    dogma(&mut game, tiger);
    let mut exchanges = 0;
    let (status, events) = play(&mut game, |decision| {
        if decision.options.contains(&Alternative::Decline) && exchanges >= 1 {
            // After the repeat the hand is empty, so only declining remains.
            return decision
                .options
                .iter()
                .position(|o| *o == Alternative::Decline);
        }
        if decision.options.iter().any(|o| {
            matches!(
                o,
                Alternative::Move {
                    verb: Verb::Return,
                    ..
                }
            )
        }) {
            exchanges += 1;
        }
        Some(0)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let exchanged: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Exchanged { movements, .. } => Some(movements.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(exchanged.len(), 2, "{exchanged:?}");
    assert_eq!(exchanged[0][0].card, scored);
    assert_eq!(exchanged[0][0].destination, board(B, Color::Red));
    assert_eq!(exchanged[0][1].card, b_top);
    assert_eq!(exchanged[0][1].destination, SCORE_A);
    // The second pass swaps them back.
    assert_eq!(exchanged[1][0].card, b_top);
    assert_eq!(exchanged[1][1].card, scored);
    let returned: Vec<_> = moves(&events)
        .into_iter()
        .filter(|m| m.0 == Some(Verb::Return))
        .map(|m| m.1)
        .collect();
    assert_eq!(returned.len(), 2);
    assert!(returned.contains(&hand_one) && returned.contains(&hand_two));
    assert_eq!(game.position().cards(SCORE_A), &[scored]);
    assert_eq!(game.position().cards(board(B, Color::Red)), &[b_low, b_top]);
    assert!(game.position().cards(HAND_A).is_empty());
}

#[test]
fn greenland_returns_qualifying_tops_until_none_remain() {
    let mut l = Layout::default();
    let greenland = l.artifact("A110", board(A, Color::Green));
    let b_under = l.tech(Color::Blue, 1, board(B, Color::Blue), &[Icon::Efficiency]);
    let featured = artifacts_card("A110").unwrap().featured.unwrap();
    let b_top = l.tech(
        Color::Blue,
        2,
        board(B, Color::Blue),
        &[featured, featured, featured, Icon::Efficiency],
    );
    let b_red = l.tech(Color::Red, 3, board(B, Color::Red), &[Icon::Efficiency]);
    let a_under = l.tech(Color::Red, 1, board(A, Color::Red), &[Icon::Prosperity]);
    let a_top = l.tech(Color::Red, 2, board(A, Color::Red), &[Icon::Prosperity]);
    let mut game = l.game();
    dogma(&mut game, greenland);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let returned: Vec<_> = moves(&events)
        .into_iter()
        .filter(|m| m.0 == Some(Verb::Return))
        .map(|m| m.1)
        .collect();
    assert_eq!(returned.len(), 5, "{returned:?}");
    for card in [b_under, b_top, b_red, a_under, a_top] {
        assert!(returned.contains(&card));
    }
    assert!(game.position().cards(board(B, Color::Blue)).is_empty());
    assert!(game.position().cards(board(A, Color::Red)).is_empty());
}

#[test]
fn sanskrit_junks_score_piles_or_scores_every_other_color_top() {
    // Scored cards exist: they are junked and nothing else happens.
    let mut l = Layout::default();
    let sanskrit = l.artifact("A111", board(A, Color::Green));
    let a_scored = l.tech(Color::Red, 3, SCORE_A, &[]);
    let b_scored = l.tech(Color::Blue, 4, SCORE_B, &[]);
    l.tech(Color::Yellow, 2, board(A, Color::Yellow), &[]);
    let mut game = l.game();
    dogma(&mut game, sanskrit);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let junked: Vec<_> = moves(&events).into_iter().map(|m| m.1).collect();
    assert_eq!(junked.len(), 2);
    assert!(junked.contains(&a_scored) && junked.contains(&b_scored));

    // Empty score piles: each player's non-highest tops transfer to their
    // own score pile.
    let mut l = Layout::default();
    let sanskrit = l.artifact("A111", board(A, Color::Green));
    let a_low = l.tech(Color::Yellow, 2, board(A, Color::Yellow), &[]);
    let b_high = l.tech(Color::Blue, 4, board(B, Color::Blue), &[]);
    let b_low = l.tech(Color::Red, 1, board(B, Color::Red), &[]);
    let mut game = l.game();
    dogma(&mut game, sanskrit);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (
                Some(Verb::Transfer),
                a_low,
                board(A, Color::Yellow),
                SCORE_A
            ),
            (Some(Verb::Transfer), b_low, board(B, Color::Red), SCORE_B),
        ]
    );
    assert_eq!(game.position().top(A, Color::Green), Some(sanskrit));
    assert_eq!(game.position().top(B, Color::Blue), Some(b_high));
}

#[test]
fn great_barrier_reef_keeps_the_top_five_of_each_color_then_splays_aslant() {
    let mut l = Layout::default();
    let reef = l.artifact("A112", board(A, Color::Blue));
    let reds: Vec<_> = (1..=7)
        .map(|value| l.tech(Color::Red, value, board(A, Color::Red), &[]))
        .collect();
    let mut game = l.game();
    dogma(&mut game, reef);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let junked: Vec<_> = moves(&events).into_iter().map(|m| m.1).collect();
    assert_eq!(junked.len(), 2);
    assert!(junked.contains(&reds[0]) && junked.contains(&reds[1]));
    assert_eq!(game.position().cards(board(A, Color::Red)), &reds[2..]);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Aslant);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
}

#[test]
fn martian_internet_ends_the_game_in_a_draw() {
    let mut l = Layout::default();
    let martian = l.artifact("A113", board(A, Color::Blue));
    l.tech(Color::Red, 5, SCORE_A, &[]);
    let mut game = l.game();
    dogma(&mut game, martian);
    assert_eq!(any(&mut game).0, Status::Draw(DrawReason::Effect));
}

#[test]
fn jumbo_kingdom_junks_a_color_everywhere_and_scores_junk_values_under_eleven_points() {
    let mut l = Layout::default();
    let jumbo = l.artifact("A114", board(A, Color::Purple));
    l.cards[usize::from(jumbo.0)].effects.truncate(1);
    let a_red = l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let b_red_low = l.tech(Color::Red, 1, board(B, Color::Red), &[]);
    let b_red_top = l.tech(Color::Red, 3, board(B, Color::Red), &[]);
    l.tech(Color::Blue, 4, board(B, Color::Blue), &[]);
    let mut game = l.game();
    dogma(&mut game, jumbo);
    let (status, events) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
            .or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let junked: Vec<_> = moves(&events).into_iter().map(|m| m.1).collect();
    assert_eq!(junked.len(), 3);
    for card in [a_red, b_red_low, b_red_top] {
        assert!(junked.contains(&card));
    }

    // Two fives score ten points, so a second pass scores the three.
    let mut l = Layout::default();
    let jumbo = l.artifact("A114", board(A, Color::Purple));
    l.cards[usize::from(jumbo.0)].effects.remove(0);
    let five_a = l.tech(Color::Red, 5, Zone::Junk, &[]);
    let five_b = l.tech(Color::Blue, 5, Zone::Junk, &[]);
    let three = l.tech(Color::Green, 3, Zone::Junk, &[]);
    let mut game = l.game();
    dogma(&mut game, jumbo);
    // The junk is uninspectable: the choice names a back, here an age 5.
    let (status, events) = play(&mut game, |decision| {
        option_selecting(decision, five_a)
            .or_else(|| {
                decision.options.iter().position(
                    |o| matches!(o, Alternative::BlindSelect { back, .. } if back.age == 5),
                )
            })
            .or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let scored: Vec<_> = moves(&events).into_iter().map(|m| m.1).collect();
    assert_eq!(scored.len(), 3, "{scored:?}");
    assert!(scored[..2].contains(&five_a) && scored[..2].contains(&five_b));
    assert_eq!(scored[2], three);
    assert!(game.position().cards(Zone::Junk).is_empty());
}

#[test]
fn oculus_quest_moves_the_victims_whole_board_to_their_hand() {
    let mut l = Layout::default();
    let oculus = l.artifact("A115", board(A, Color::Purple));
    let b_top = l.tech(
        Color::Blue,
        2,
        board(B, Color::Blue),
        &sharing_icons("A115"),
    );
    let b_under = l.tech(Color::Blue, 1, board(B, Color::Blue), &[]);
    l.placements.swap(1, 2);
    let b_red = l.tech(Color::Red, 3, board(B, Color::Red), &[]);
    let mut game = l.game();
    dogma(&mut game, oculus);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let transferred: Vec<_> = moves(&events).into_iter().map(|m| m.1).collect();
    assert_eq!(transferred.len(), 3);
    for card in [b_top, b_under, b_red] {
        assert!(transferred.contains(&card));
        assert!(game.position().cards(HAND_B).contains(&card));
    }
    assert!(game.position().cards(board(B, Color::Blue)).is_empty());
    let _ = (HAND_A, SCORE_A, SCORE_B);
}
