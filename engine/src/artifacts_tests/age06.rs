//! Age 6 Artifacts (A056-A065): expected outcomes on explicit positions.
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

fn pick_value(decision: &Decision, value: u8) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Value(value))
}

#[test]
fn frigate_constitution_returns_the_revealed_card_and_its_color_when_a_top_matches() {
    let mut l = Layout::default();
    let frigate = l.artifact("A056", board(A, Color::Red));
    l.tech(Color::Blue, 3, board(A, Color::Blue), &[]);
    let shown = l.tech(Color::Green, 3, HAND_B, &[]);
    let other = l.tech(Color::Yellow, 5, HAND_B, &[]);
    let g1 = l.tech(
        Color::Green,
        1,
        board(B, Color::Green),
        &sharing_icons("A056"),
    );
    let g2 = l.tech(
        Color::Green,
        2,
        board(B, Color::Green),
        &sharing_icons("A056"),
    );
    let mut game = l.game();
    dogma(&mut game, frigate);
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, B);
        decision.options.iter().position(|o| matches!(o, Alternative::Reveal { target: TargetDescriptor::Known { card, .. } } if *card == shown))
            .or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved[0], (Some(Verb::Return), shown, HAND_B, base_deck(3)));
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(moved[1..].iter().any(|m| m.1 == g1) && moved[1..].iter().any(|m| m.1 == g2));
    assert_eq!(game.position().cards(HAND_B), &[other]);
}

#[test]
fn us_declaration_takes_the_highest_hand_score_and_industry_cards() {
    let mut l = Layout::default();
    let declaration = l.artifact("A057", board(A, Color::Red));
    let hand_high = l.tech(Color::Blue, 7, HAND_B, &[]);
    l.tech(Color::Blue, 2, HAND_B, &[]);
    let score_high = l.tech(Color::Green, 6, SCORE_B, &[]);
    l.tech(Color::Green, 1, SCORE_B, &[]);
    let industry = l.tech(Color::Yellow, 4, board(B, Color::Yellow), &[Icon::Industry]);
    l.tech(
        Color::Purple,
        9,
        board(B, Color::Purple),
        &sharing_icons("A057"),
    );
    let mut game = l.game();
    dogma(&mut game, declaration);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Transfer), hand_high, HAND_B, HAND_A),
            (Some(Verb::Transfer), score_high, SCORE_B, SCORE_A),
            (
                Some(Verb::Transfer),
                industry,
                board(B, Color::Yellow),
                board(A, Color::Yellow)
            ),
        ]
    );
}

#[test]
fn qianlongs_dragon_robe_makes_four_independent_transfers() {
    let mut l = Layout::default();
    let robe = l.artifact("A058", board(A, Color::Yellow));
    let red = l.tech(Color::Red, 2, board(B, Color::Red), &sharing_icons("A058"));
    let green = l.tech(Color::Green, 3, board(B, Color::Green), &[]);
    let yellow = l.tech(Color::Yellow, 4, SCORE_B, &[]);
    let purple = l.tech(Color::Purple, 5, SCORE_B, &[]);
    let blue = l.tech(Color::Blue, 6, SCORE_B, &[]);
    let mut game = l.game();
    dogma(&mut game, robe);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Transfer), red, board(B, Color::Red), SCORE_A),
            (
                Some(Verb::Transfer),
                green,
                board(B, Color::Green),
                board(A, Color::Green)
            ),
            (Some(Verb::Transfer), yellow, SCORE_B, SCORE_A),
            (Some(Verb::Transfer), purple, SCORE_B, HAND_A),
        ]
    );
    assert_eq!(game.position().cards(SCORE_B), &[blue]);
}

#[test]
fn pride_and_prejudice_scores_and_repeats_while_the_melded_color_shows_the_fewest_cards() {
    let mut l = Layout::default();
    l.tech(Color::Yellow, 1, board(A, Color::Yellow), &[]);
    let pride = l.artifact("A059", board(A, Color::Yellow));
    l.tech(Color::Blue, 1, board(A, Color::Blue), &[]);
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let red = l.tech(Color::Red, 6, base_deck(6), &[]);
    let green = l.tech(Color::Green, 6, base_deck(6), &[]);
    let mut game = l.game();
    // Red shows two cards; yellow and blue show one each.
    let position = game
        .position()
        .clone()
        .with_splay(A, Color::Red, Splay::Left)
        .unwrap();
    game = Game::new(position, Expansions::ARTIFACTS).unwrap();
    dogma(&mut game, pride);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // Green melds alone (tied fewest): scored, repeat. Red then shows three.
    assert_eq!(
        moves(&events),
        [
            (None, green, base_deck(6), HAND_A),
            (Some(Verb::Meld), green, HAND_A, board(A, Color::Green)),
            (Some(Verb::Score), green, board(A, Color::Green), SCORE_A),
            (None, red, base_deck(6), HAND_A),
            (Some(Verb::Meld), red, HAND_A, board(A, Color::Red)),
        ]
    );
}

#[test]
fn the_wealth_of_nations_scores_a_one_then_the_rounded_up_fifth_of_its_score() {
    let mut l = Layout::default();
    let wealth = l.artifact("A060", board(A, Color::Green));
    l.tech(Color::Red, 8, SCORE_A, &[]);
    let one = l.tech(Color::Blue, 1, base_deck(1), &[]);
    let two_left = l.tech(Color::Purple, 2, base_deck(2), &[]);
    let two = l.tech(Color::Blue, 2, base_deck(2), &[]);
    let mut game = l.game();
    dogma(&mut game, wealth);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // 8 + 1 = 9, divided by five rounded up: 2.
    assert_eq!(
        moves(&events),
        [
            (None, one, base_deck(1), HAND_A),
            (Some(Verb::Score), one, HAND_A, SCORE_A),
            (None, two, base_deck(2), HAND_A),
            (Some(Verb::Score), two, HAND_A, SCORE_A),
            (Some(Verb::Junk), two_left, base_deck(2), Zone::Junk),
        ]
    );
}

#[test]
fn buttonwood_agreement_scores_a_chosen_color_or_punishes_another() {
    for (drawn_color, hit) in [(Color::Red, true), (Color::Purple, false)] {
        let mut l = Layout::default();
        let buttonwood = l.artifact("A061", board(A, Color::Green));
        l.tech(Color::Red, 1, board(A, Color::Red), &[]);
        l.tech(Color::Red, 2, board(A, Color::Red), &[]);
        l.tech(Color::Purple, 1, board(A, Color::Purple), &[]);
        l.tech(Color::Purple, 2, board(A, Color::Purple), &[]);
        let purple_scored = l.tech(Color::Purple, 3, SCORE_A, &[]);
        let eight = l.tech(drawn_color, 8, base_deck(8), &[]);
        let mut game = l.game();
        let position = game
            .position()
            .clone()
            .with_splay(A, Color::Purple, Splay::Left)
            .unwrap();
        game = Game::new(position, Expansions::ARTIFACTS).unwrap();
        dogma(&mut game, buttonwood);
        let colors = (1 << Color::Red as u8) | (1 << Color::Blue as u8) | (1 << Color::Green as u8);
        let (status, events) = play(&mut game, |decision| {
            decision
                .options
                .iter()
                .position(|o| *o == Alternative::Colors(colors))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0], (None, eight, base_deck(8), HAND_A));
        if hit {
            assert_eq!(moved[1], (Some(Verb::Score), eight, HAND_A, SCORE_A));
            assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
        } else {
            assert_eq!(
                moved[1],
                (Some(Verb::Return), purple_scored, SCORE_A, base_deck(3))
            );
            assert_eq!(game.position().splay(A, Color::Purple), Splay::None);
        }
    }
}

#[test]
fn kilogram_of_the_archives_scores_a_ten_for_a_sum_of_ten_or_junks_the_sum() {
    for (top_value, sums_to_ten) in [(6, true), (3, false)] {
        let mut l = Layout::default();
        let kilogram = l.artifact("A062", board(A, Color::Blue));
        let hand = l.tech(Color::Red, 4, HAND_A, &[]);
        let top = l.tech(Color::Green, top_value, board(A, Color::Green), &[]);
        let ten = l.tech(Color::Yellow, 10, base_deck(10), &[]);
        let seven = l.tech(Color::Yellow, 7, base_deck(7), &[]);
        let mut game = l.game();
        dogma(&mut game, kilogram);
        let (status, events) = play(&mut game, |decision| option_moving(decision, top));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0], (Some(Verb::Return), hand, HAND_A, base_deck(4)));
        assert_eq!(
            moved[1],
            (
                Some(Verb::Return),
                top,
                board(A, Color::Green),
                base_deck(top_value)
            )
        );
        if sums_to_ten {
            assert_eq!(moved[2], (None, ten, base_deck(10), HAND_A));
            assert_eq!(moved[3], (Some(Verb::Score), ten, HAND_A, SCORE_A));
        } else {
            assert_eq!(
                moved[2],
                (Some(Verb::Junk), seven, base_deck(7), Zone::Junk)
            );
        }
    }
}

#[test]
fn puffing_billy_tucks_splays_and_draws_the_most_common_icon_count() {
    let mut l = Layout::default();
    let billy = l.artifact("A063", board(A, Color::Blue));
    let tucked = l.tech(
        Color::Red,
        2,
        HAND_A,
        &[Icon::Industry, Icon::Industry, Icon::Industry, Icon::Health],
    );
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, billy);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // A single red card cannot splay; it shows three Industry.
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Tuck), tucked, HAND_A, board(A, Color::Red)),
            (None, three, base_deck(3), HAND_A),
        ]
    );
}

#[test]
fn puffing_billy_counts_bonuses_as_one_icon_type() {
    // Three visible bonuses of different values outnumber the one Industry: bonuses are
    // one icon type, as for Supremacy.
    let mut l = Layout::default();
    let billy = l.artifact("A063", board(A, Color::Blue));
    let mut symbols = [Symbol::Blank; SLOTS];
    symbols[..4].copy_from_slice(&[
        Symbol::Bonus(3),
        Symbol::Bonus(5),
        Symbol::Bonus(2),
        Symbol::Icon(Icon::Industry),
    ]);
    let tucked = l.add(
        Card {
            code: "synthetic-echoes-bonuses",
            set: CardSet::Echoes,
            color: Color::Red,
            value: 2,
            figure: None,
            echo: None,
            symbols,
            featured: Some(Icon::Industry),
            effects: Vec::new(),
        },
        HAND_A,
    );
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    l.tech(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::ECHOES));
    dogma(&mut game, billy);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Tuck), tucked, HAND_A, board(A, Color::Red)),
            (None, three, base_deck(3), HAND_A),
        ]
    );
}

#[test]
fn moonlight_sonata_melds_the_bottom_of_a_highest_color_then_claims_and_junks_standards() {
    let mut l = Layout::default();
    let sonata = l.artifact("A064", board(A, Color::Purple));
    let bottom = l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    let top = l.tech(Color::Red, 9, board(A, Color::Red), &[]);
    l.tech(Color::Blue, 3, board(A, Color::Blue), &[]);
    let ach_a = l.tech(Color::Green, 4, Zone::AvailableAchievements, &[]);
    let ach_b = l.tech(Color::Green, 5, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
    dogma(&mut game, sonata);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, ach_a).or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (
            Some(Verb::Meld),
            bottom,
            board(A, Color::Red),
            board(A, Color::Red)
        )
    );
    assert_eq!(game.position().cards(board(A, Color::Red)), &[top, bottom]);
    assert_eq!(
        moved[1],
        (
            Some(Verb::Achieve),
            ach_a,
            Zone::AvailableAchievements,
            Zone::Achievements(A)
        )
    );
    assert_eq!(
        moved[2],
        (
            Some(Verb::Junk),
            ach_b,
            Zone::AvailableAchievements,
            Zone::Junk
        )
    );
}

#[test]
fn marcha_real_rewards_a_returned_pair_sharing_a_value_or_a_color() {
    let mut l = Layout::default();
    let marcha = l.artifact("A065", board(A, Color::Purple));
    let r4 = l.tech(Color::Red, 4, HAND_A, &[]);
    let r4b = l.tech(Color::Red, 4, HAND_A, &[]);
    let five = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let ach = l.tech(Color::Green, 9, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
    dogma(&mut game, marcha);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert!(
        moved[..2]
            .iter()
            .all(|m| m.0 == Some(Verb::Return) && m.3 == base_deck(4))
    );
    assert_eq!(moved[2], (None, five, base_deck(5), HAND_A));
    assert_eq!(
        moved[3],
        (
            Some(Verb::Achieve),
            ach,
            Zone::AvailableAchievements,
            Zone::Achievements(A)
        )
    );
    let _ = (r4, r4b, pick_value);
}

#[test]
fn frigate_constitution_sees_a_revealed_hand_yamato_as_eleven() {
    let mut l = Layout::default();
    let frigate = l.artifact("A056", board(A, Color::Red));
    l.tech(Color::Blue, 11, board(A, Color::Blue), &[]);
    let b_red = l.tech(Color::Red, 2, board(B, Color::Red), &sharing_icons("A056"));
    let yamato = l.artifact("A077", HAND_B);
    let mut game = l.game();
    dogma(&mut game, frigate);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == yamato))
    );
    assert_eq!(
        moves(&events),
        [
            (
                Some(Verb::Return),
                yamato,
                HAND_B,
                Zone::Deck(Deck {
                    set: CardSet::Artifacts,
                    age: 8
                })
            ),
            (
                Some(Verb::Return),
                b_red,
                board(B, Color::Red),
                base_deck(2)
            ),
        ]
    );
    // Moving ended the reveal: the deck card is its back again.
    assert_eq!(game.position().value(yamato), 8);
    let _ = (HAND_A, SCORE_A, SCORE_B);
}
