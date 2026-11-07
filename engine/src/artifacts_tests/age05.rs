//! Age 5 Artifacts (A046-A055): expected outcomes on explicit positions.
use super::*;

const HAND_A: Zone = Zone::Hand(A);
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

fn with_effect(mut card: Card, color: Color, instructions: Vec<Instruction>) -> Card {
    card.color = color;
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    card
}

fn pick_color(decision: &Decision, color: Color) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Colors(1 << color as u8))
}

#[test]
fn galley_whydah_counts_visible_cards_not_symbols() {
    // B's lone red City (Lyon) shows six symbol positions to Galley Whydah's four, but
    // each red pile shows one card, so red does not qualify.
    let mut l = Layout::default();
    let galley = l.artifact("A046", board(A, Color::Red));
    l.add(
        crate::catalog::city_card("C066").unwrap(),
        board(B, Color::Red),
    );
    l.tech(
        Color::Yellow,
        1,
        board(B, Color::Yellow),
        &sharing_icons("A046"),
    );
    let b_blue = l.tech(Color::Blue, 1, board(B, Color::Blue), &[]);
    l.tech(Color::Green, 1, board(B, Color::Green), &[]);
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::CITIES));
    dogma(&mut game, galley);
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, B);
        assert!(pick_color(decision, Color::Red).is_none());
        assert!(pick_color(decision, Color::Green).is_some());
        pick_color(decision, Color::Blue)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Transfer),
            b_blue,
            board(B, Color::Blue),
            board(A, Color::Blue)
        )]
    );
}

#[test]
fn galley_whydah_swaps_a_color_the_victim_shows_more_of_bottom_up() {
    let mut l = Layout::default();
    let galley = l.artifact("A046", board(A, Color::Red));
    let a_green = l.tech(Color::Green, 2, board(A, Color::Green), &[]);
    let b_bottom = l.tech(
        Color::Green,
        1,
        board(B, Color::Green),
        &sharing_icons("A046"),
    );
    let b_top = l.tech(
        Color::Green,
        3,
        board(B, Color::Green),
        &sharing_icons("A046"),
    );
    l.tech(Color::Blue, 1, board(B, Color::Blue), &[]);
    let mut game = l.game();
    // B's green shows one card (unsplayed), A's one: splay B's green to show two.
    let position = game
        .position()
        .clone()
        .with_splay(B, Color::Green, Splay::Left)
        .unwrap();
    game = Game::new(position, Expansions::ARTIFACTS).unwrap();
    dogma(&mut game, galley);
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, B);
        // Blue (1 vs 0) and green (2 vs 1) qualify; red does not.
        assert!(pick_color(decision, Color::Red).is_none());
        pick_color(decision, Color::Green)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (
                Some(Verb::Transfer),
                a_green,
                board(A, Color::Green),
                SCORE_A
            ),
            (
                Some(Verb::Transfer),
                b_bottom,
                board(B, Color::Green),
                board(A, Color::Green)
            ),
            (
                Some(Verb::Transfer),
                b_top,
                board(B, Color::Green),
                board(A, Color::Green)
            ),
        ]
    );
    assert_eq!(
        game.position().cards(board(A, Color::Green)),
        &[b_bottom, b_top]
    );
}

#[test]
fn ship_of_the_line_sussex_scores_a_color_with_an_empty_score_pile_or_returns_the_pile() {
    let mut l = Layout::default();
    let sussex = l.artifact("A047", board(A, Color::Red));
    let under = l.tech(Color::Blue, 1, board(A, Color::Blue), &[]);
    let top = l.tech(Color::Blue, 2, board(A, Color::Blue), &[]);
    let mut game = l.game();
    dogma(&mut game, sussex);
    let (status, events) = play(&mut game, |decision| pick_color(decision, Color::Blue));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 2);
    assert!(
        moved
            .iter()
            .all(|m| m.0 == Some(Verb::Score) && m.3 == SCORE_A)
    );
    assert!(moved.iter().any(|m| m.1 == under) && moved.iter().any(|m| m.1 == top));

    let mut l = Layout::default();
    let sussex = l.artifact("A047", board(A, Color::Red));
    let scored = l.tech(Color::Blue, 3, SCORE_A, &[]);
    let mut game = l.game();
    dogma(&mut game, sussex);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events),
        [(Some(Verb::Return), scored, SCORE_A, base_deck(3))]
    );
}

#[test]
fn gujin_tushu_jicheng_super_executes_an_opposing_top_on_your_turn() {
    let mut l = Layout::default();
    let gujin = l.artifact("A048", board(A, Color::Yellow));
    let mut oars = base_card("B003").unwrap();
    oars.color = Color::Red;
    l.add(oars, board(B, Color::Red));
    let prosperity = l.tech(Color::Green, 1, Zone::Hand(B), &[Icon::Prosperity]);
    l.tech(Color::Green, 1, base_deck(1), &[]);
    let mut game = l.game();
    dogma(&mut game, gujin);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // Oars' demand runs against B: its Prosperity card comes to A's score.
    assert_eq!(
        moves(&events)[0],
        (Some(Verb::Transfer), prosperity, Zone::Hand(B), SCORE_A)
    );
}

#[test]
fn the_daily_courant_returns_a_six_then_melds_and_executes_at_the_matching_top_value() {
    let mut l = Layout::default();
    let courant = l.artifact("A049", board(A, Color::Yellow));
    l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let six = l.tech(Color::Red, 6, base_deck(6), &[]);
    let mut two_card = with_effect(
        base_card("B001").unwrap(),
        Color::Green,
        vec![build::draw(1)],
    );
    two_card.value = 2;
    let two = l.add(two_card, base_deck(2));
    let one = l.tech(Color::Blue, 1, base_deck(1), &[]);
    let mut game = l.game();
    dogma(&mut game, courant);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, six, base_deck(6), HAND_A),
            (Some(Verb::Return), six, HAND_A, base_deck(6)),
            (None, two, base_deck(2), HAND_A),
            (Some(Verb::Meld), two, HAND_A, board(A, Color::Green)),
            (None, one, base_deck(1), HAND_A),
        ]
    );
}

#[test]
fn barque_longue_la_belle_melds_fives_until_a_green_one() {
    let mut l = Layout::default();
    let belle = l.artifact("A050", board(A, Color::Green));
    let green = l.tech(Color::Green, 5, base_deck(5), &[]);
    let red = l.tech(Color::Red, 5, base_deck(5), &[]);
    let mut game = l.game();
    dogma(&mut game, belle);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, red, base_deck(5), HAND_A),
            (Some(Verb::Meld), red, HAND_A, board(A, Color::Red)),
            (None, green, base_deck(5), HAND_A),
            (Some(Verb::Meld), green, HAND_A, board(A, Color::Green)),
        ]
    );
}

#[test]
fn hudsons_bay_scores_every_bottom_then_melds_from_score_splays_and_junks() {
    let mut l = Layout::default();
    let archives = l.artifact("A051", board(A, Color::Green));
    let red_bottom = l.tech(Color::Red, 3, board(A, Color::Red), &[]);
    let red_top = l.tech(Color::Red, 4, board(A, Color::Red), &[]);
    let blue = l.tech(Color::Blue, 2, board(A, Color::Blue), &[]);
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, archives);
    let (status, events) = play(&mut game, |decision| option_moving(decision, red_bottom));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    // Three bottoms score: red, blue and Archives itself.
    assert_eq!(
        moved[..3]
            .iter()
            .filter(|m| m.0 == Some(Verb::Score))
            .count(),
        3
    );
    assert!(moved[..3].iter().any(|m| m.1 == archives) && moved[..3].iter().any(|m| m.1 == blue));
    assert_eq!(
        moved[3],
        (Some(Verb::Meld), red_bottom, SCORE_A, board(A, Color::Red))
    );
    assert_eq!(
        moved[4],
        (Some(Verb::Junk), three, base_deck(3), Zone::Junk)
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
    assert_eq!(
        game.position().cards(board(A, Color::Red)),
        &[red_top, red_bottom]
    );
}

#[test]
fn boerhaave_microscope_returns_the_lowest_hand_and_top_cards_and_scores_their_sum() {
    let mut l = Layout::default();
    let microscope = l.artifact("A052", board(A, Color::Blue));
    let hand_low = l.tech(Color::Red, 2, HAND_A, &[]);
    l.tech(Color::Red, 4, HAND_A, &[]);
    let top_low = l.tech(Color::Green, 3, board(A, Color::Green), &[]);
    let five = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let mut game = l.game();
    dogma(&mut game, microscope);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Return), hand_low, HAND_A, base_deck(2)),
            (
                Some(Verb::Return),
                top_low,
                board(A, Color::Green),
                base_deck(3)
            ),
            (None, five, base_deck(5), HAND_A),
            (Some(Verb::Score), five, HAND_A, SCORE_A),
        ]
    );
}

#[test]
fn principia_returns_non_blue_tops_then_draws_and_melds_one_higher_ascending() {
    let mut l = Layout::default();
    let principia = l.artifact("A053", board(A, Color::Blue));
    let red = l.tech(Color::Red, 4, board(A, Color::Red), &[]);
    let green = l.tech(Color::Green, 2, board(A, Color::Green), &[]);
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let five = l.tech(Color::Purple, 5, base_deck(5), &[]);
    let mut game = l.game();
    dogma(&mut game, principia);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert!(moved[..2].iter().any(|m| m.1 == red) && moved[..2].iter().any(|m| m.1 == green));
    assert_eq!(
        moved[2..],
        [
            (None, three, base_deck(3), HAND_A),
            (Some(Verb::Meld), three, HAND_A, board(A, Color::Yellow)),
            (None, five, base_deck(5), HAND_A),
            (Some(Verb::Meld), five, HAND_A, board(A, Color::Purple)),
        ]
    );
}

#[test]
fn sandham_room_cricket_bat_claims_on_red_and_junks_otherwise() {
    for (color, claims) in [(Color::Red, true), (Color::Blue, false)] {
        let mut l = Layout::default();
        let bat = l.artifact("A054", board(A, Color::Purple));
        let six = l.tech(color, 6, base_deck(6), &[]);
        let ach = l.tech(Color::Green, 9, Zone::AvailableAchievements, &[]);
        let mut game = l.game();
        dogma(&mut game, bat);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0], (None, six, base_deck(6), HAND_A));
        let destination = if claims {
            Zone::Achievements(A)
        } else {
            Zone::Junk
        };
        assert_eq!(moved[1].1, ach);
        assert_eq!(moved[1].3, destination);
    }
}

#[test]
fn almira_claims_the_melded_value_or_junks_the_lowest_achievements_deck() {
    let mut l = Layout::default();
    let almira = l.artifact("A055", board(A, Color::Purple));
    let four = l.tech(Color::Red, 4, HAND_A, &[]);
    let ach4 = l.tech(Color::Green, 4, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
    dogma(&mut game, almira);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Meld), four, HAND_A, board(A, Color::Red)),
            (
                Some(Verb::Achieve),
                ach4,
                Zone::AvailableAchievements,
                Zone::Achievements(A)
            ),
        ]
    );

    let mut l = Layout::default();
    let almira = l.artifact("A055", board(A, Color::Purple));
    l.tech(Color::Green, 3, Zone::AvailableAchievements, &[]);
    l.tech(Color::Green, 6, Zone::AvailableAchievements, &[]);
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, almira);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events),
        [(Some(Verb::Junk), three, base_deck(3), Zone::Junk)]
    );
    let _ = SCORE_B;
}

#[test]
fn principia_uses_the_values_its_returns_showed_including_a_returned_yamato() {
    // Yamato's board value 11 is what Principia returned: it asks for a 12
    // and the game ends by exhaustion, instead of reading its back (8).
    let mut l = Layout::default();
    let principia = l.artifact("A053", board(A, Color::Blue));
    let yamato = l.artifact("A077", board(A, Color::Red));
    let nine = l.tech(Color::Green, 9, base_deck(9), &[]);
    let mut game = l.game();
    assert_eq!(game.position().value(yamato), 11);
    dogma(&mut game, principia);
    let (status, events) = no_decisions(&mut game);
    assert!(
        matches!(status, Status::Winner(_) | Status::Draw(_)),
        "{status:?}"
    );
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Return),
            yamato,
            board(A, Color::Red),
            Zone::Deck(Deck {
                set: CardSet::Artifacts,
                age: 8
            })
        )]
    );
    assert_eq!(game.position().cards(base_deck(9)), &[nine]);
    assert_eq!(game.position().value(yamato), 8);

    // Several returns keep their shown values in ascending order: the 2
    // draws and melds a 3 before Yamato's 12 exhausts the decks.
    let mut l = Layout::default();
    let principia = l.artifact("A053", board(A, Color::Blue));
    let yamato = l.artifact("A077", board(A, Color::Red));
    let green = l.tech(Color::Green, 2, board(A, Color::Green), &[]);
    let three = l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, principia);
    let (status, events) = no_decisions(&mut game);
    assert!(
        matches!(status, Status::Winner(_) | Status::Draw(_)),
        "{status:?}"
    );
    let moved = moves(&events);
    assert!(moved[..2].iter().any(|m| m.1 == green) && moved[..2].iter().any(|m| m.1 == yamato));
    assert_eq!(
        moved[2..],
        [
            (None, three, base_deck(3), HAND_A),
            (Some(Verb::Meld), three, HAND_A, board(A, Color::Yellow)),
        ]
    );
}
