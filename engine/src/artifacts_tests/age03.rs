//! Age 3 Artifacts (A026-A035): expected outcomes on explicit positions.
use super::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);

fn base_deck(age: u8) -> Zone {
    Zone::Deck(Deck::base(age))
}

fn board(player: Player, color: Color) -> Zone {
    Zone::Board(player, color)
}

fn sharing_icons(code: &str) -> [Icon; 3] {
    [artifacts_card(code).unwrap().featured.unwrap(); 3]
}

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
    let mut l = Layout::default();
    let excalibur = l.artifact("A026", board(A, Color::Red));
    l.tech(Color::Blue, 2, board(A, Color::Blue), &[]);
    let higher = l.tech(
        Color::Blue,
        5,
        board(B, Color::Blue),
        &sharing_icons("A026"),
    );
    l.tech(Color::Green, 1, board(B, Color::Green), &[]); // A has no green: 1 > 0.
    let mut game = l.game();
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
    let mut l = Layout::default();
    let amulet = l.artifact("A027", board(A, Color::Red));
    let under = l.tech(Color::Green, 1, board(B, Color::Green), &[]);
    let top = l.tech(
        Color::Green,
        4,
        board(B, Color::Green),
        &sharing_icons("A027"),
    );
    let other = l.tech(Color::Blue, 2, board(B, Color::Blue), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let shrine = l.artifact("A028", board(A, Color::Yellow));
    l.tech(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A028"),
    );
    let red = l.tech(Color::Red, 3, HAND_B, &[]);
    let green = l.tech(Color::Green, 5, HAND_B, &[]);
    let three = l.tech(Color::Purple, 3, base_deck(3), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let river = l.artifact("A029", board(A, Color::Yellow));
    let purple = l.tech(Color::Purple, 4, base_deck(4), &[]);
    let red = l.tech(Color::Red, 4, base_deck(4), &[]);
    let mut game = l.game();
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

    let mut l = Layout::default();
    let river = l.artifact("A029", board(A, Color::Yellow));
    let yellow = l.tech(Color::Yellow, 4, base_deck(4), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let stone = l.artifact("A030", board(A, Color::Green));
    let two = l.tech(Color::Red, 2, HAND_A, &[]);
    let three = l.tech(Color::Blue, 3, HAND_A, &[]);
    let four = l.tech(Color::Yellow, 4, HAND_A, &[]);
    let seven = l.tech(Color::Purple, 7, base_deck(7), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let head = l.artifact("A031", board(A, Color::Green));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let ach2 = l.tech(Color::Blue, 2, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let head = l.artifact("A031", board(A, Color::Green));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let two = l.tech(Color::Blue, 2, base_deck(2), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let chart = l.artifact("A032", board(A, Color::Blue));
    let one = l.tech(Color::Red, 1, HAND_A, &[]);
    let four = l.tech(Color::Green, 4, HAND_A, &[]);
    let two = l.tech(Color::Yellow, 2, base_deck(2), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let mirror = l.artifact("A033", board(A, Color::Blue));
    let shown = l.tech(Color::Red, 2, HAND_A, &[]);
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let b_red = l.add(
        with_effect(base_card("B001").unwrap(), Color::Red, vec![build::draw(1)]),
        board(B, Color::Red),
    );
    let one = l.tech(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = l.game();
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
        let mut l = Layout::default();
        let book = l.artifact("A034", board(A, Color::Purple));
        let hand = l.tech(Color::Red, 1, HAND_A, &[]);
        let scored = l.tech(Color::Red, 2, SCORE_A, &[]);
        l.tech(Color::Green, 1, board(A, Color::Green), &[]);
        l.tech(Color::Green, 2, board(A, Color::Green), &[]);
        let drawn = l.tech(color, 3, base_deck(3), &[]);
        let nine = l.tech(Color::Red, 9, base_deck(9), &[]);
        let mut game = l.game();
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
        let mut l = Layout::default();
        let shroud = l.artifact("A035", board(A, Color::Purple));
        let hand = l.tech(Color::Red, 1, HAND_A, &[]);
        let top = l.tech(Color::Red, 2, board(A, Color::Red), &[]);
        let scored = l.tech(scored_color, 3, SCORE_A, &[]);
        let ach = l.tech(Color::Green, 9, Zone::AvailableAchievements, &[]);
        let mut game = l.game();
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
