//! Age 3 Artifacts (A026-A035): expected outcomes on explicit positions.
use inno_engine::*;

mod common;
use common::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);

fn with_effect(mut card: Card, color: Color, instructions: Vec<Instruction>) -> Card {
    card.color = color;
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    card
}

#[test]
fn excalibur_takes_a_top_card_higher_than_the_demanders_top_of_that_color() {
    let mut f = Fixture::default();
    let excalibur = f.card("A026", board(A, Color::Red));
    f.plain(Color::Blue, 2, board(A, Color::Blue), &[]);
    let higher = f.plain(
        Color::Blue,
        5,
        board(B, Color::Blue),
        &sharing_icons("A026"),
    );
    f.plain(Color::Green, 1, board(B, Color::Green), &[]); // A has no green: 1 > 0.
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, excalibur);
    let (status, events) = play(&mut game, |decision| option_moving(decision, higher));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Transfer),
            higher,
            board(B, Color::Blue),
            board(A, Color::Blue)
        )]
    );
}

#[test]
fn mjolnir_amulet_moves_the_chosen_tops_whole_stack_to_the_demanders_score() {
    let mut f = Fixture::default();
    let amulet = f.card("A027", board(A, Color::Red));
    let under = f.plain(Color::Green, 1, board(B, Color::Green), &[]);
    let top = f.plain(
        Color::Green,
        4,
        board(B, Color::Green),
        &sharing_icons("A027"),
    );
    let other = f.plain(Color::Blue, 2, board(B, Color::Blue), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, amulet);
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, B);
        decision.options.iter().position(|o| matches!(o, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == top))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 2);
    assert!(
        moved
            .iter()
            .all(|m| m.0 == Some(Verb::Transfer) && m.3 == SCORE_A)
    );
    assert!(moved.iter().any(|m| m.1 == under) && moved.iter().any(|m| m.1 == top));
    assert_eq!(game.position().top(B, Color::Blue), Some(other));
}

#[test]
fn moylough_belt_shrine_lets_the_demander_pick_from_the_revealed_hand() {
    let mut f = Fixture::default();
    let shrine = f.card("A028", board(A, Color::Yellow));
    f.plain(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A028"),
    );
    let red = f.plain(Color::Red, 3, HAND_B, &[]);
    let green = f.plain(Color::Green, 5, HAND_B, &[]);
    let three = f.plain(Color::Purple, 3, base_deck(3), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, shrine);
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, A);
        decision.options.iter().position(|o| matches!(o, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == red))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Revealed { .. }))
            .count(),
        2
    );
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Transfer), red, HAND_B, board(A, Color::Red)),
            (Some(Verb::Junk), three, base_deck(3), Zone::Junk),
        ]
    );
    assert_eq!(game.position().cards(HAND_B), &[green]);
}

#[test]
fn along_the_river_tucks_yellow_scores_purple_and_repeats_on_other_colors() {
    let mut f = Fixture::default();
    let river = f.card("A029", board(A, Color::Yellow));
    let purple = f.plain(Color::Purple, 4, base_deck(4), &[]);
    let red = f.plain(Color::Red, 4, base_deck(4), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, river);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, red, base_deck(4), HAND_A),
            (None, purple, base_deck(4), HAND_A),
            (Some(Verb::Score), purple, HAND_A, SCORE_A),
        ]
    );

    let mut f = Fixture::default();
    let river = f.card("A029", board(A, Color::Yellow));
    let yellow = f.plain(Color::Yellow, 4, base_deck(4), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, river);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events)[1],
        (Some(Verb::Tuck), yellow, HAND_A, board(A, Color::Yellow))
    );
    assert_eq!(
        game.position().cards(board(A, Color::Yellow)),
        &[yellow, river]
    );
}

#[test]
fn philosophers_stone_scores_as_many_as_the_returned_value_and_junks_their_total() {
    let mut f = Fixture::default();
    let stone = f.card("A030", board(A, Color::Green));
    let two = f.plain(Color::Red, 2, HAND_A, &[]);
    let three = f.plain(Color::Blue, 3, HAND_A, &[]);
    let four = f.plain(Color::Yellow, 4, HAND_A, &[]);
    let seven = f.plain(Color::Purple, 7, base_deck(7), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, stone);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, two)
            .or_else(|| option_moving(decision, three))
            .or_else(|| option_moving(decision, four))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved[0], (Some(Verb::Return), two, HAND_A, base_deck(2)));
    assert_eq!(moved[1].0, Some(Verb::Score));
    assert_eq!(moved[2].0, Some(Verb::Score));
    assert_eq!(
        moved[3],
        (Some(Verb::Junk), seven, base_deck(7), Zone::Junk)
    );
}

#[test]
fn ife_head_splays_an_unsplayed_color_right_then_junks_or_draws_its_count() {
    let mut f = Fixture::default();
    let head = f.card("A031", board(A, Color::Green));
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let ach2 = f.plain(Color::Blue, 2, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, head);
    let (status, events) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Junk),
            ach2,
            Zone::AvailableAchievements,
            Zone::Junk
        )]
    );

    // No matching achievement: draw a card of that value instead.
    let mut f = Fixture::default();
    let head = f.card("A031", board(A, Color::Green));
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let two = f.plain(Color::Blue, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, head);
    let (_, events) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
    });
    assert_eq!(moves(&events), [(None, two, base_deck(2), HAND_A)]);
}

#[test]
fn dunhuang_star_chart_returns_the_hand_and_draws_the_count() {
    let mut f = Fixture::default();
    let chart = f.card("A032", board(A, Color::Blue));
    let one = f.plain(Color::Red, 1, HAND_A, &[]);
    let four = f.plain(Color::Green, 4, HAND_A, &[]);
    let two = f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, chart);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 3);
    assert!(moved[..2].iter().any(|m| m.1 == one) && moved[..2].iter().any(|m| m.1 == four));
    assert_eq!(moved[2], (None, two, base_deck(2), HAND_A));
}

#[test]
fn yata_no_kagami_splays_the_revealed_color_and_self_executes_a_matching_top_anywhere() {
    let mut f = Fixture::default();
    let mirror = f.card("A033", board(A, Color::Blue));
    let shown = f.plain(Color::Red, 2, HAND_A, &[]);
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let b_red = f.add(
        with_effect(
            Card::from_code("B001").unwrap(),
            Color::Red,
            vec![build::draw(1)],
        ),
        board(B, Color::Red),
    );
    let one = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, mirror);
    let (status, events) = play(&mut game, |decision| {
        decision.options.iter().position(|o| matches!(o, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == b_red))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == shown))
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
    assert_eq!(moves(&events), [(None, one, base_deck(1), HAND_A)]);
}

#[test]
fn necronomicon_branches_on_the_revealed_color() {
    let cases = [
        (Color::Yellow, "hand returned"),
        (Color::Green, "unsplayed"),
        (Color::Red, "score returned"),
        (Color::Blue, "drew a nine"),
        (Color::Purple, "nothing"),
    ];
    for (color, what) in cases {
        let mut f = Fixture::default();
        let book = f.card("A034", board(A, Color::Purple));
        let hand = f.plain(Color::Red, 1, HAND_A, &[]);
        let scored = f.plain(Color::Red, 2, SCORE_A, &[]);
        f.plain(Color::Green, 1, board(A, Color::Green), &[]);
        f.plain(Color::Green, 2, board(A, Color::Green), &[]);
        let drawn = f.plain(color, 3, base_deck(3), &[]);
        let nine = f.plain(Color::Red, 9, base_deck(9), &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        let mut position = game.position().clone();
        position = position.with_splay(A, Color::Green, Splay::Left).unwrap();
        game = Game::new(position, Expansions::ARTIFACTS).unwrap();
        dogma(&mut game, book);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit), "{what}");
        let moved = moves(&events);
        assert_eq!(moved[0], (None, drawn, base_deck(3), HAND_A));
        match color {
            Color::Yellow => {
                assert_eq!(moved.len(), 3, "{what}");
                assert!(moved[1..].iter().all(|m| m.0 == Some(Verb::Return)));
                assert!(moved[1..].iter().any(|m| m.1 == hand));
            }
            Color::Green => {
                assert_eq!(moved.len(), 1, "{what}");
                assert_eq!(game.position().splay(A, Color::Green), Splay::None);
            }
            Color::Red => assert_eq!(
                moved[1],
                (Some(Verb::Return), scored, SCORE_A, base_deck(2))
            ),
            Color::Blue => assert_eq!(moved[1], (None, nine, base_deck(9), HAND_A)),
            Color::Purple => assert_eq!(moved.len(), 1, "{what}"),
        }
    }
}

#[test]
fn shroud_of_turin_claims_only_after_all_three_returns() {
    for scored_color in [Color::Red, Color::Blue] {
        let mut f = Fixture::default();
        let shroud = f.card("A035", board(A, Color::Purple));
        let hand = f.plain(Color::Red, 1, HAND_A, &[]);
        let top = f.plain(Color::Red, 2, board(A, Color::Red), &[]);
        let scored = f.plain(scored_color, 3, SCORE_A, &[]);
        let ach = f.plain(Color::Green, 9, Zone::AvailableAchievements, &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, shroud);
        // The claim may take any available achievement, a special included.
        let (status, events) = play(&mut game, |decision| {
            decision.options.iter().position(|o| {
                matches!(
                    o,
                    Alternative::BlindMove {
                        verb: Verb::Achieve,
                        ..
                    }
                )
            })
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0], (Some(Verb::Return), hand, HAND_A, base_deck(1)));
        assert_eq!(
            moved[1],
            (Some(Verb::Return), top, board(A, Color::Red), base_deck(2))
        );
        if scored_color == Color::Red {
            assert_eq!(
                moved[2],
                (Some(Verb::Return), scored, SCORE_A, base_deck(3))
            );
            assert_eq!(
                moved[3],
                (
                    Some(Verb::Achieve),
                    ach,
                    Zone::AvailableAchievements,
                    Zone::Achievements(A)
                )
            );
        } else {
            assert_eq!(moved.len(), 2);
        }
    }
}
