//! Age 7 Artifacts (A066-A075): expected outcomes on explicit positions.
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

fn with_effect(mut card: Card, color: Color, value: u8, instructions: Vec<Instruction>) -> Card {
    card.color = color;
    card.value = value;
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    card
}

fn pick_value(decision: &Decision, value: u8) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Value(value))
}

fn pick_known(decision: &Decision, card: CardId) -> Option<usize> {
    decision.options.iter().position(|o| {
        matches!(o,
        Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
        | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
        | Alternative::Reveal { target: TargetDescriptor::Known { card: c, .. } } if *c == card)
    })
}

#[test]
fn submarine_hunley_returns_the_melded_color_when_its_bottom_is_even() {
    for (bottom_value, returns) in [(2, true), (3, false)] {
        let mut l = Layout::default();
        let hunley = l.artifact("A066", board(A, Color::Red));
        let bottom = l.tech(
            Color::Green,
            bottom_value,
            board(B, Color::Green),
            &sharing_icons("A066"),
        );
        let seven = l.tech(Color::Green, 7, base_deck(7), &[]);
        let mut game = l.game();
        dogma(&mut game, hunley);
        let (status, events) = play(&mut game, |_| Some(0));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0], (None, seven, base_deck(7), HAND_B));
        assert_eq!(
            moved[1],
            (Some(Verb::Meld), seven, HAND_B, board(B, Color::Green))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Revealed { card, .. } if *card == bottom))
        );
        assert_eq!(moved.len(), if returns { 4 } else { 2 }, "{bottom_value}");
        if returns {
            assert!(game.position().cards(board(B, Color::Green)).is_empty());
        }
    }
}

#[test]
fn jedlik_scores_an_eight_then_melds_one_and_junks_a_chosen_deck_if_it_was_an_eight() {
    let mut l = Layout::default();
    let jedlik = l.artifact("A067", board(A, Color::Red));
    let eight_a = l.tech(Color::Blue, 8, base_deck(8), &[]);
    let eight_b = l.tech(Color::Green, 8, base_deck(8), &[]);
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, jedlik);
    let (status, events) = play(&mut game, |decision| pick_value(decision, 3));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, eight_b, base_deck(8), HAND_A),
            (Some(Verb::Score), eight_b, HAND_A, SCORE_A),
            (None, eight_a, base_deck(8), HAND_A),
            (Some(Verb::Meld), eight_a, HAND_A, board(A, Color::Blue)),
            (Some(Verb::Junk), three, base_deck(3), Zone::Junk),
        ]
    );

    // The [8] deck is empty: the meld is a 9, so no junk.
    let mut l = Layout::default();
    let jedlik = l.artifact("A067", board(A, Color::Red));
    l.tech(Color::Blue, 8, base_deck(8), &[]);
    let nine = l.tech(Color::Green, 9, base_deck(9), &[]);
    l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, jedlik);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events)[3],
        (Some(Verb::Meld), nine, HAND_A, board(A, Color::Green))
    );
    assert_eq!(moves(&events).len(), 4);
}

#[test]
fn colt_paterson_revolver_returns_hand_and_score_when_the_draw_matches_a_held_color() {
    for (drawn_color, returns) in [(Color::Red, true), (Color::Green, false)] {
        let mut l = Layout::default();
        let colt = l.artifact("A068", board(A, Color::Yellow));
        l.tech(
            Color::Blue,
            1,
            board(B, Color::Blue),
            &sharing_icons("A068"),
        );
        let held = l.tech(Color::Red, 2, HAND_B, &[]);
        let scored = l.tech(Color::Purple, 3, SCORE_B, &[]);
        let seven = l.tech(drawn_color, 7, base_deck(7), &[]);
        let mut game = l.game();
        dogma(&mut game, colt);
        let (status, events) = play(&mut game, |_| Some(0));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0], (None, seven, base_deck(7), HAND_B));
        if returns {
            assert_eq!(moved.len(), 4, "{moved:?}");
            assert!(moved[1..].iter().all(|m| m.0 == Some(Verb::Return)));
            assert!(
                moved[1..].iter().any(|m| m.1 == held) && moved[1..].iter().any(|m| m.1 == scored)
            );
        } else {
            assert_eq!(moved.len(), 1);
        }
    }
}

#[test]
fn singer_model_27_tucks_splays_up_tucks_matching_scores_and_junks_a_standard() {
    let mut l = Layout::default();
    let singer = l.artifact("A069", board(A, Color::Yellow));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    let tucked = l.tech(Color::Red, 2, HAND_A, &[]);
    let scored_red = l.tech(Color::Red, 3, SCORE_A, &[]);
    l.tech(Color::Blue, 3, SCORE_A, &[]);
    let ach = l.tech(Color::Green, 9, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
    dogma(&mut game, singer);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Tuck), tucked, HAND_A, board(A, Color::Red)),
            (Some(Verb::Tuck), scored_red, SCORE_A, board(A, Color::Red)),
            (
                Some(Verb::Junk),
                ach,
                Zone::AvailableAchievements,
                Zone::Junk
            ),
        ]
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
}

#[test]
fn singer_model_27_junks_nothing_when_its_color_was_already_splayed_up() {
    // "If you do" needs the splay to change the pile (doc/rules/IF_YOU_DO_AUDIT.md).
    let mut l = Layout::default();
    let singer = l.artifact("A069", board(A, Color::Yellow));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    let tucked = l.tech(Color::Red, 2, HAND_A, &[]);
    let scored_red = l.tech(Color::Red, 3, SCORE_A, &[]);
    l.tech(Color::Green, 9, Zone::AvailableAchievements, &[]);
    let mut position = Position::new(l.cards, &l.placements, A).unwrap();
    position.players[A.index()].splays[Color::Red as usize] = Splay::Up;
    position.refresh_boards();
    let mut game = Game::new(position, Expansions::ARTIFACTS).unwrap();
    dogma(&mut game, singer);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Tuck), tucked, HAND_A, board(A, Color::Red)),
            (Some(Verb::Tuck), scored_red, SCORE_A, board(A, Color::Red)),
        ]
    );
}

#[test]
fn metre_bar_wins_when_each_drawn_color_has_the_chosen_number_of_cards_else_returns_them() {
    for (extra, wins) in [(true, true), (false, false)] {
        let mut l = Layout::default();
        let metre = l.artifact("A070", board(A, Color::Green));
        l.tech(Color::Red, 1, board(A, Color::Red), &[]);
        l.tech(Color::Red, 2, board(A, Color::Red), &[]);
        l.tech(Color::Blue, 1, board(A, Color::Blue), &[]);
        if extra {
            l.tech(Color::Blue, 2, board(A, Color::Blue), &[]);
        }
        for color in [Color::Red, Color::Red, Color::Blue] {
            l.tech(color, 2, base_deck(2), &[]);
        }
        let mut game = l.game();
        dogma(&mut game, metre);
        let (status, events) = play(&mut game, |decision| pick_value(decision, 2).or(Some(0)));
        if wins {
            assert_eq!(status, Status::Winner(A));
            assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
            assert_eq!(game.position().splay(A, Color::Blue), Splay::Up);
        } else {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(
                moves(&events)
                    .iter()
                    .filter(|m| m.0 == Some(Verb::Return))
                    .count(),
                3
            );
        }
    }
}

#[test]
fn hansen_writing_ball_compels_four_draws_a_blue_meld_and_a_hand_transfer() {
    let mut l = Layout::default();
    let hansen = l.artifact("A071", board(A, Color::Green));
    l.cards[usize::from(hansen.0)].effects.truncate(1);
    l.tech(
        Color::Yellow,
        1,
        board(B, Color::Yellow),
        &sharing_icons("A071"),
    );
    let held = l.tech(Color::Green, 2, HAND_B, &[]);
    for _ in 0..3 {
        l.tech(Color::Red, 7, base_deck(7), &[]);
    }
    let blue = l.tech(Color::Blue, 7, base_deck(7), &[]);
    let mut game = l.game();
    dogma(&mut game, hansen);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved[..4].iter().filter(|m| m.3 == HAND_B).count(), 4);
    assert_eq!(
        moved[4],
        (Some(Verb::Meld), blue, HAND_B, board(B, Color::Blue))
    );
    assert_eq!(
        moved[5..]
            .iter()
            .filter(|m| m.0 == Some(Verb::Transfer) && m.3 == HAND_A)
            .count(),
        4
    );
    assert!(game.position().cards(HAND_B).is_empty());
    assert!(game.position().cards(HAND_A).contains(&held));
}

#[test]
fn hansen_writing_ball_tucks_and_repeats_until_a_drawn_seven_has_efficiency() {
    let mut l = Layout::default();
    let hansen = l.artifact("A071", board(A, Color::Green));
    let with_efficiency = l.tech(Color::Purple, 7, base_deck(7), &[Icon::Efficiency]);
    let plain = l.tech(Color::Purple, 7, base_deck(7), &[]);
    let mut game = l.game();
    dogma(&mut game, hansen);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, plain, base_deck(7), HAND_A),
            (Some(Verb::Tuck), plain, HAND_A, board(A, Color::Purple)),
            (None, with_efficiency, base_deck(7), HAND_A),
        ]
    );
}

#[test]
fn periodic_table_draws_one_higher_and_repeats_when_melding_over_a_chosen_card() {
    let mut l = Layout::default();
    let table = l.artifact("A072", board(A, Color::Blue));
    let red = l.tech(Color::Red, 3, board(A, Color::Red), &[]);
    let green = l.tech(Color::Green, 3, board(A, Color::Green), &[]);
    l.tech(Color::Yellow, 5, board(A, Color::Yellow), &[]);
    let four_purple = l.tech(Color::Purple, 4, base_deck(4), &[]);
    let four_red = l.tech(Color::Red, 4, base_deck(4), &[]);
    let mut game = l.game();
    dogma(&mut game, table);
    let (status, events) = play(&mut game, |decision| {
        pick_known(decision, red).or_else(|| pick_known(decision, green))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // Red 4 covers the chosen red 3: repeat; then no equal-value pair (3 and
    // 4 differ): the choice has no candidates and the effect ends.
    assert_eq!(
        moves(&events),
        [
            (None, four_red, base_deck(4), HAND_A),
            (Some(Verb::Meld), four_red, HAND_A, board(A, Color::Red)),
        ]
    );
    let _ = four_purple;
}

#[test]
fn corvette_challenger_tucks_splays_up_and_uses_the_stack_count() {
    let mut l = Layout::default();
    let corvette = l.artifact("A073", board(A, Color::Blue));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    let eight = l.tech(Color::Red, 8, base_deck(8), &[]);
    let two = l.tech(Color::Yellow, 2, base_deck(2), &[]);
    let two_left = l.tech(Color::Yellow, 2, base_deck(2), &[]);
    let mut game = l.game();
    dogma(&mut game, corvette);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, eight, base_deck(8), HAND_A),
            (Some(Verb::Tuck), eight, HAND_A, board(A, Color::Red)),
            (None, two_left, base_deck(2), HAND_A),
            (Some(Verb::Score), two_left, HAND_A, SCORE_A),
            (Some(Verb::Junk), two, base_deck(2), Zone::Junk),
        ]
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
}

#[test]
fn roundhay_garden_scene_melds_the_highest_scored_card_scores_two_of_its_value_and_executes_it() {
    let mut l = Layout::default();
    let roundhay = l.artifact("A074", board(A, Color::Purple));
    let high = l.add(
        with_effect(
            base_card("B001").unwrap(),
            Color::Green,
            3,
            vec![build::draw(1)],
        ),
        SCORE_A,
    );
    l.tech(Color::Red, 2, SCORE_A, &[]);
    let three_a = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let three_b = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let one = l.tech(Color::Blue, 1, base_deck(1), &[]);
    let mut game = l.game();
    dogma(&mut game, roundhay);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Meld), high, SCORE_A, board(A, Color::Green)),
            (None, three_b, base_deck(3), HAND_A),
            (Some(Verb::Score), three_b, HAND_A, SCORE_A),
            (None, three_a, base_deck(3), HAND_A),
            (Some(Verb::Score), three_a, HAND_A, SCORE_A),
            (None, one, base_deck(1), HAND_A),
        ]
    );
}

#[test]
fn communist_manifesto_gives_one_drawn_seven_to_the_opponent_and_executes_the_other() {
    let mut l = Layout::default();
    let manifesto = l.artifact("A075", board(A, Color::Purple));
    let given = l.add(
        with_effect(
            base_card("B001").unwrap(),
            Color::Red,
            7,
            vec![build::draw(1)],
        ),
        base_deck(7),
    );
    let kept = l.add(
        with_effect(
            base_card("B001").unwrap(),
            Color::Green,
            7,
            vec![build::draw(2)],
        ),
        base_deck(7),
    );
    let two = l.tech(Color::Blue, 2, base_deck(2), &[]);
    l.tech(Color::Blue, 1, base_deck(1), &[]);
    let mut game = l.game();
    dogma(&mut game, manifesto);
    let (status, events) = play(&mut game, |decision| pick_known(decision, given));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, kept, base_deck(7), HAND_A),
            (None, given, base_deck(7), HAND_A),
            (Some(Verb::Transfer), given, HAND_A, board(B, Color::Red)),
            (Some(Verb::Meld), kept, HAND_A, board(A, Color::Green)),
            (None, two, base_deck(2), HAND_A),
        ]
    );
}
