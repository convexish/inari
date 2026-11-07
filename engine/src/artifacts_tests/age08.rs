//! Age 8 Artifacts (A076-A085): expected outcomes on explicit positions.
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

/// A color neither this Figure nor A's purple Ball occupies.
fn free_color(l: &Layout, figure: CardId) -> Color {
    let taken = l.cards[usize::from(figure.0)].color;
    [Color::Blue, Color::Green, Color::Yellow, Color::Red]
        .into_iter()
        .find(|&color| color != taken)
        .unwrap()
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

#[test]
fn battleship_bismarck_returns_the_victims_stack_of_the_drawn_color() {
    let mut l = Layout::default();
    let bismarck = l.artifact("A076", board(A, Color::Red));
    let g1 = l.tech(
        Color::Green,
        1,
        board(B, Color::Green),
        &sharing_icons("A076"),
    );
    let g2 = l.tech(
        Color::Green,
        2,
        board(B, Color::Green),
        &sharing_icons("A076"),
    );
    let eight = l.tech(Color::Green, 8, base_deck(8), &[]);
    let mut game = l.game();
    dogma(&mut game, bismarck);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved[0], (None, eight, base_deck(8), HAND_B));
    assert_eq!(moved.len(), 3);
    assert!(moved[1..].iter().any(|m| m.1 == g1) && moved[1..].iter().any(|m| m.1 == g2));
    assert!(game.position().cards(board(B, Color::Green)).is_empty());
}

#[test]
fn plusch_rod_bear_splays_up_the_chosen_values_colors_and_returns_that_value_from_all_scores() {
    let mut l = Layout::default();
    let bear = l.artifact("A078", board(A, Color::Yellow));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 3, board(A, Color::Red), &[]);
    l.tech(Color::Blue, 5, board(A, Color::Blue), &[]);
    let a3 = l.tech(Color::Green, 3, SCORE_A, &[]);
    let b3 = l.tech(Color::Purple, 3, SCORE_B, &[]);
    let b4 = l.tech(Color::Purple, 4, SCORE_B, &[]);
    let mut game = l.game();
    dogma(&mut game, bear);
    let (status, events) = play(&mut game, |decision| pick_value(decision, 3).or(Some(0)));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
    let moved = moves(&events);
    assert_eq!(moved.len(), 2);
    assert!(
        moved
            .iter()
            .all(|m| m.0 == Some(Verb::Return) && m.3 == base_deck(3))
    );
    assert!(moved.iter().any(|m| m.1 == a3) && moved.iter().any(|m| m.1 == b3));
    assert_eq!(game.position().cards(SCORE_B), &[b4]);
}

#[test]
fn action_comics_achieves_itself_on_green_or_strips_efficiency_tops_into_the_demanders_achievements()
 {
    // Green: an eligible victim achieves Action Comics, a top card on A's
    // board. The drawn card is revealed: its color picks the branch.
    let mut l = Layout::default();
    let comics = l.artifact("A079", board(A, Color::Yellow));
    l.tech(
        Color::Blue,
        8,
        board(B, Color::Blue),
        &sharing_icons("A079"),
    );
    for _ in 0..5 {
        l.tech(Color::Red, 8, SCORE_B, &[]);
    }
    let green = l.tech(Color::Green, 8, base_deck(8), &[]);
    let mut game = l.game();
    dogma(&mut game, comics);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Revealed { card, .. } if *card == green
    )));
    assert_eq!(
        moves(&events),
        [
            (None, green, base_deck(8), HAND_B),
            (
                Some(Verb::Achieve),
                comics,
                board(A, Color::Yellow),
                Zone::Achievements(B)
            ),
        ]
    );

    // Without ordinary eligibility the green branch achieves nothing.
    let mut l = Layout::default();
    let comics = l.artifact("A079", board(A, Color::Yellow));
    l.tech(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A079"),
    );
    let green = l.tech(Color::Green, 8, base_deck(8), &[]);
    let mut game = l.game();
    dogma(&mut game, comics);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(moves(&events), [(None, green, base_deck(8), HAND_B)]);

    // Efficiency: return it, give the matching Efficiency top to A, repeat.
    let mut l = Layout::default();
    let comics = l.artifact("A079", board(A, Color::Yellow));
    l.tech(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A079"),
    );
    let b_red = l.tech(Color::Red, 2, board(B, Color::Red), &[Icon::Efficiency]);
    let plain = l.tech(Color::Purple, 8, base_deck(8), &[]);
    let red_eight = l.tech(Color::Red, 8, base_deck(8), &[Icon::Efficiency]);
    let mut game = l.game();
    dogma(&mut game, comics);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, red_eight, base_deck(8), HAND_B),
            (Some(Verb::Return), red_eight, HAND_B, base_deck(8)),
            (
                Some(Verb::Transfer),
                b_red,
                board(B, Color::Red),
                Zone::Achievements(A)
            ),
            (None, plain, base_deck(8), HAND_B),
        ]
    );
}

#[test]
fn liner_titanic_scores_every_bottom_and_junks_the_lowest_deck_with_a_card() {
    let mut l = Layout::default();
    let titanic = l.artifact("A080", board(A, Color::Green));
    let red_bottom = l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    let red_top = l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let four = l.tech(Color::Blue, 4, base_deck(4), &[]);
    l.tech(Color::Blue, 6, base_deck(6), &[]);
    let mut game = l.game();
    dogma(&mut game, titanic);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(moved[..2].iter().all(|m| m.0 == Some(Verb::Score)));
    assert!(
        moved[..2].iter().any(|m| m.1 == red_bottom) && moved[..2].iter().any(|m| m.1 == titanic)
    );
    assert_eq!(moved[2], (Some(Verb::Junk), four, base_deck(4), Zone::Junk));
    assert_eq!(game.position().top(A, Color::Red), Some(red_top));
}

/// "The deck of lowest value with a card" means a deck in play: the Age 0 deck counts
/// only once a Figure has introduced it.
#[test]
fn liner_titanic_junks_the_age_zero_deck_only_once_it_is_in_play() {
    for introduced in [false, true] {
        let mut l = Layout::default();
        let titanic = l.artifact("A080", board(A, Color::Green));
        let dormant = if introduced {
            base_deck(0)
        } else {
            Zone::AgeZeroReserve
        };
        let zero = l.tech(Color::Red, 0, dormant, &[Icon::Food]);
        let four = l.tech(Color::Blue, 4, base_deck(4), &[]);
        let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
        dogma(&mut game, titanic);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let junked = if introduced {
            (zero, base_deck(0))
        } else {
            (four, base_deck(4))
        };
        assert_eq!(
            moves(&events).last(),
            Some(&(Some(Verb::Junk), junked.0, junked.1, Zone::Junk))
        );
    }
}

#[test]
fn meiji_mura_scores_three_of_the_returned_value_or_junks_the_highest_scored_decks() {
    for (threes, exact) in [(3, true), (2, false)] {
        let mut l = Layout::default();
        let meiji = l.artifact("A081", board(A, Color::Green));
        let returned = l.tech(Color::Red, 3, HAND_A, &[]);
        let four = l.tech(Color::Blue, 4, base_deck(4), &[]);
        let four_left = l.tech(Color::Blue, 4, base_deck(4), &[]);
        for _ in 0..threes {
            l.tech(Color::Yellow, 3, base_deck(3), &[]);
        }
        let mut game = l.game();
        dogma(&mut game, meiji);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(
            moved[0],
            (Some(Verb::Return), returned, HAND_A, base_deck(3))
        );
        // The returned 3 goes under the deck, so three 3s are drawn either way
        // when three were there; with two, the third draw is the returned 3.
        let scores = moved.iter().filter(|m| m.0 == Some(Verb::Score)).count();
        assert_eq!(scores, 3);
        let junks: Vec<_> = moved.iter().filter(|m| m.0 == Some(Verb::Junk)).collect();
        assert!(junks.is_empty(), "{exact}: {junks:?}");
        let _ = (four, four_left);
    }
    // An exhausted [3] deck skips to a 4: not exactly three 3s, so junk the
    // [4] deck (the highest scored card).
    let mut l = Layout::default();
    let meiji = l.artifact("A081", board(A, Color::Green));
    l.tech(Color::Red, 3, HAND_A, &[]);
    l.tech(Color::Yellow, 3, base_deck(3), &[]);
    let four_left = l.tech(Color::Blue, 4, base_deck(4), &[]);
    l.tech(Color::Blue, 4, base_deck(4), &[]);
    l.tech(Color::Blue, 4, base_deck(4), &[]);
    let mut game = l.game();
    dogma(&mut game, meiji);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved.last().unwrap(),
        &(Some(Verb::Junk), four_left, base_deck(4), Zone::Junk)
    );
}

#[test]
fn meiji_mura_with_an_empty_hand_scores_three_zeros_and_junks_only_a_zero_deck() {
    // Nothing returned: three 0s are drawn (as 1s without Age 0) and scored, and only
    // the 0 deck is junked, which is not in play without Figures; the [1] deck stays.
    let mut l = Layout::default();
    let meiji = l.artifact("A081", board(A, Color::Green));
    for _ in 0..4 {
        l.tech(Color::Yellow, 1, base_deck(1), &[]);
    }
    let mut game = l.game();
    dogma(&mut game, meiji);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    let verbs: Vec<_> = moved.iter().map(|m| (m.0, m.2, m.3)).collect();
    let draw = (None, base_deck(1), HAND_A);
    let score = (Some(Verb::Score), HAND_A, Zone::Score(A));
    assert_eq!(verbs, [draw, score, draw, score, draw, score]);
}

#[test]
fn meiji_mura_with_an_empty_hand_junks_the_zero_deck_when_it_exists() {
    let mut l = Layout::default();
    let meiji = l.artifact("A081", board(A, Color::Green));
    for _ in 0..4 {
        l.tech(Color::Yellow, 0, base_deck(0), &[]);
    }
    let one = l.tech(Color::Blue, 1, base_deck(1), &[]);
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    dogma(&mut game, meiji);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.iter().filter(|m| m.0 == Some(Verb::Score)).count(), 3);
    let junks: Vec<_> = moved.iter().filter(|m| m.0 == Some(Verb::Junk)).collect();
    assert_eq!(junks.len(), 1, "{junks:?}");
    assert_eq!((junks[0].2, junks[0].3), (base_deck(0), Zone::Junk));
    assert_eq!(game.position().cards(base_deck(1)), [one]);
}

#[test]
fn parnell_pitch_drop_wins_only_when_no_standard_is_left_to_junk_and_the_meld_has_three_efficiency()
{
    for (standard, wins) in [(false, true), (true, false)] {
        let mut l = Layout::default();
        let parnell = l.artifact("A082", board(A, Color::Blue));
        l.tech(Color::Red, 4, board(A, Color::Red), &[]);
        let nine = l.tech(
            Color::Green,
            9,
            base_deck(9),
            &[Icon::Efficiency, Icon::Efficiency, Icon::Efficiency],
        );
        if standard {
            l.tech(Color::Yellow, 2, Zone::AvailableAchievements, &[]);
        }
        let mut game = l.game();
        dogma(&mut game, parnell);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(
            moves(&events)[1],
            (Some(Verb::Meld), nine, HAND_A, board(A, Color::Green))
        );
        assert_eq!(
            status,
            if wins {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
    }
}

#[test]
fn earharts_electra_junks_one_top_per_value_descending_and_wins_with_all_eight_in_the_junk() {
    let mut l = Layout::default();
    let electra = l.artifact("A083", board(A, Color::Blue));
    let tops: Vec<_> = [(Color::Red, 3), (Color::Green, 5), (Color::Yellow, 5)]
        .into_iter()
        .map(|(color, value)| l.tech(color, value, board(A, color), &[]))
        .collect();
    for value in [1, 2, 4, 6, 7] {
        l.tech(Color::Purple, value, Zone::Junk, &[]);
    }
    let mut game = l.game();
    dogma(&mut game, electra);
    let (status, events) = play(&mut game, |decision| option_moving(decision, tops[1]));
    // Junked: 8 (Electra itself), 5 (one of two), 3; with 1, 2, 4, 6, 7 the
    // junk holds every value below 9.
    assert_eq!(status, Status::Winner(A));
    let moved = moves(&events);
    assert_eq!(
        moved.iter().map(|m| m.1).collect::<Vec<_>>(),
        [electra, tops[1], tops[0]]
    );
}

#[test]
fn garlands_ruby_slippers_wins_on_an_effectless_meld_and_executes_any_other() {
    let mut l = Layout::default();
    let slippers = l.artifact("A084", board(A, Color::Purple));
    let yamato = l.artifact("A077", HAND_A);
    let mut game = l.game();
    dogma(&mut game, slippers);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events),
        [(Some(Verb::Meld), yamato, HAND_A, board(A, Color::Red))]
    );
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.position().value(yamato), 11);

    let mut l = Layout::default();
    let slippers = l.artifact("A084", board(A, Color::Purple));
    let eight = l.add(
        with_effect(
            base_card("B001").unwrap(),
            Color::Red,
            8,
            vec![build::draw(1)],
        ),
        HAND_A,
    );
    let one = l.tech(Color::Blue, 1, base_deck(1), &[]);
    let mut game = l.game();
    dogma(&mut game, slippers);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Meld), eight, HAND_A, board(A, Color::Red)),
            (None, one, base_deck(1), HAND_A),
        ]
    );
}

#[test]
fn world_cup_final_ball_returns_a_claimed_standard_and_awards_the_unique_highest_top() {
    // The compel alone: B returns one of its claimed standard achievements.
    let mut l = Layout::default();
    let ball = l.artifact("A085", board(A, Color::Purple));
    l.cards[usize::from(ball.0)].effects.truncate(1);
    l.tech(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A085"),
    );
    let b_ach = l.tech(Color::Green, 2, Zone::Achievements(B), &[]);
    let mut game = l.game();
    dogma(&mut game, ball);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Return),
            b_ach,
            Zone::Achievements(B),
            base_deck(2)
        )]
    );

    // The draw alone, unshared: A's red 5 beats B's 3, so A achieves it and
    // repeats. Yellow is tied: nobody claims and the effect ends.
    let mut l = Layout::default();
    let ball = l.artifact("A085", board(A, Color::Purple));
    l.cards[usize::from(ball.0)].effects.remove(0);
    let a_red = l.tech(Color::Red, 5, board(A, Color::Red), &[]);
    let b_red = l.tech(Color::Red, 3, board(B, Color::Red), &[]);
    l.tech(Color::Yellow, 2, board(A, Color::Yellow), &[]);
    l.tech(Color::Yellow, 2, board(B, Color::Yellow), &[]);
    let yellow = l.tech(Color::Yellow, 8, base_deck(8), &[]);
    let red = l.tech(Color::Red, 8, base_deck(8), &[]);
    let mut game = l.game();
    dogma(&mut game, ball);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, red, base_deck(8), HAND_A),
            (
                Some(Verb::Achieve),
                a_red,
                board(A, Color::Red),
                Zone::Achievements(A)
            ),
            (None, yellow, base_deck(8), HAND_A),
        ]
    );
    assert_eq!(game.position().top(B, Color::Red), Some(b_red));

    // The opponent with the unique highest top takes it into their
    // achievements; the repeat's draw then exhausts the decks.
    let mut l = Layout::default();
    let ball = l.artifact("A085", board(A, Color::Purple));
    l.cards[usize::from(ball.0)].effects.remove(0);
    let b_red = l.tech(Color::Red, 3, board(B, Color::Red), &[]);
    l.tech(Color::Red, 8, base_deck(8), &[]);
    let mut game = l.game();
    dogma(&mut game, ball);
    let (status, events) = no_decisions(&mut game);
    // B now holds the only achievement: the exhaustion tiebreak is B's.
    assert_eq!(status, Status::Winner(B));
    assert_eq!(
        game.victory().map(|v| v.reason),
        Some(WinReason::Exhaustion)
    );
    assert_eq!(
        moves(&events)[1],
        (
            Some(Verb::Achieve),
            b_red,
            board(B, Color::Red),
            Zone::Achievements(B)
        )
    );
    let claim = events
        .iter()
        .find_map(|event| match event {
            Event::Moved(movement) if movement.card == b_red => Some(movement.executor),
            _ => None,
        })
        .unwrap();
    assert_eq!(claim, B, "the winning player achieves it themselves");
    let _ = (HAND_B, SCORE_B);
}

#[test]
fn world_cup_final_ball_opponent_claims_are_achievements_not_transfers() {
    // Yi Sun-sin replaces transfers, not achievements: B's blue 9 is
    // achieved by B, and the repeat's draw then exhausts the decks.
    let mut l = Layout::default();
    let ball = l.artifact("A085", board(A, Color::Purple));
    l.cards[usize::from(ball.0)].effects.remove(0);
    let yi = l.figure(Figure::YiSunSin, B);
    let color = free_color(&l, yi);
    let b_top = l.tech(color, 9, board(B, color), &[]);
    l.tech(Color::Yellow, 4, base_deck(4), &[]);
    let drawn = l.tech(color, 8, base_deck(8), &[]);
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    dogma(&mut game, ball);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Winner(B));
    assert_eq!(
        moves(&events),
        [
            (None, drawn, base_deck(8), HAND_A),
            (
                Some(Verb::Achieve),
                b_top,
                board(B, color),
                Zone::Achievements(B)
            ),
        ]
    );
    assert_eq!(game.position().cards(Zone::Achievements(B)), &[b_top]);

    // A claim karma on the claimant's side reacts to the achievement:
    // Darwin wins for B before the claim.
    let mut l = Layout::default();
    let ball = l.artifact("A085", board(A, Color::Purple));
    l.cards[usize::from(ball.0)].effects.remove(0);
    let darwin = l.figure(Figure::CharlesDarwin, B);
    let color = free_color(&l, darwin);
    l.tech(color, 9, board(B, color), &[]);
    l.tech(color, 8, base_deck(8), &[]);
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    dogma(&mut game, ball);
    let (status, _) = no_decisions(&mut game);
    assert_eq!(status, Status::Winner(B));
    assert_eq!(game.victory().map(|v| v.reason), Some(WinReason::Karma));
}
