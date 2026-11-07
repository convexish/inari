//! Age 4 Artifacts (A036-A045): expected outcomes on explicit positions.
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

fn pick_value(decision: &Decision, value: u8) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Value(value))
}

#[test]
fn east_india_company_charter_returns_the_chosen_value_everywhere_and_rewards_each_pile() {
    let mut l = Layout::default();
    let charter = l.artifact("A036", board(A, Color::Red));
    let a_three = l.tech(Color::Blue, 3, SCORE_A, &[]);
    let b_three = l.tech(Color::Green, 3, SCORE_B, &[]);
    let b_four = l.tech(Color::Green, 4, SCORE_B, &[]);
    let five_a = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let five_b = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let three_deck = l.tech(Color::Purple, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, charter);
    let mut menu = 0;
    let (status, events) = play(&mut game, |decision| {
        if let Some(index) = pick_value(decision, 3) {
            menu = decision.options.len();
            assert!(pick_value(decision, 5).is_none(), "no [5]");
            return Some(index);
        }
        Some(0)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // [3] and [4] are scored somewhere; every other value (never [5]) is the
    // same empty choice, offered once.
    assert_eq!(menu, 3);
    let moved = moves(&events);
    assert_eq!(moved.len(), 2 + 4 + 3, "{moved:?}");
    assert!(moved[..2].iter().all(|m| m.0 == Some(Verb::Return)));
    assert!(moved[..2].iter().any(|m| m.1 == a_three) && moved[..2].iter().any(|m| m.1 == b_three));
    assert_eq!(moved[2].1, five_b);
    assert_eq!(moved[3], (Some(Verb::Score), five_b, HAND_A, SCORE_A));
    assert_eq!(moved[4].1, five_a);
    assert_eq!(moved[5], (Some(Verb::Score), five_a, HAND_A, SCORE_A));
    // Only 5s were scored: the [3] deck, returned cards included, is junked.
    assert!(
        moved[6..]
            .iter()
            .all(|m| m.0 == Some(Verb::Junk) && m.2 == base_deck(3))
    );
    assert!(moved[6..].iter().any(|m| m.1 == three_deck));
    assert_eq!(game.position().cards(SCORE_B), &[b_four]);
}

#[test]
fn galleon_atocha_takes_all_scored_cards_of_the_chosen_value_then_a_matching_top() {
    let mut l = Layout::default();
    let galleon = l.artifact("A037", board(A, Color::Red));
    let four_top = l.tech(
        Color::Blue,
        4,
        board(B, Color::Blue),
        &sharing_icons("A037"),
    );
    let s4a = l.tech(Color::Green, 4, SCORE_B, &[]);
    let s4b = l.tech(Color::Yellow, 4, SCORE_B, &[]);
    let s6 = l.tech(Color::Purple, 6, SCORE_B, &[]);
    let mut game = l.game();
    dogma(&mut game, galleon);
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, A);
        pick_value(decision, 4)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(
        moved[..2]
            .iter()
            .all(|m| m.0 == Some(Verb::Transfer) && m.3 == SCORE_A)
    );
    assert!(moved[..2].iter().any(|m| m.1 == s4a) && moved[..2].iter().any(|m| m.1 == s4b));
    assert_eq!(
        moved[2],
        (
            Some(Verb::Transfer),
            four_top,
            board(B, Color::Blue),
            board(A, Color::Blue)
        )
    );
    assert_eq!(game.position().cards(SCORE_B), &[s6]);
}

#[test]
fn moses_takes_one_prosperity_top_per_color_and_scores_one_of_its_owners() {
    let mut l = Layout::default();
    let moses = l.artifact("A038", board(A, Color::Yellow));
    let a_top = l.tech(Color::Red, 2, board(A, Color::Red), &[Icon::Prosperity]);
    let b_red = l.tech(
        Color::Red,
        3,
        board(B, Color::Red),
        &[Icon::Prosperity, Icon::Prosperity, Icon::Prosperity],
    );
    let b_blue = l.tech(Color::Blue, 1, board(B, Color::Blue), &[Icon::Prosperity]);
    let b_green = l.tech(
        Color::Green,
        1,
        board(B, Color::Green),
        &sharing_icons("A038"),
    );
    let mut game = l.game();
    dogma(&mut game, moses);
    let (status, events) = play(&mut game, |decision| option_moving(decision, a_top));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(
        moved[..2]
            .iter()
            .all(|m| m.0 == Some(Verb::Transfer) && m.3 == SCORE_A)
    );
    assert!(moved[..2].iter().any(|m| m.1 == b_red) && moved[..2].iter().any(|m| m.1 == b_blue));
    assert_eq!(
        moved[2],
        (Some(Verb::Score), a_top, board(A, Color::Red), SCORE_A)
    );
    assert_eq!(game.position().top(B, Color::Green), Some(b_green));
}

#[test]
fn mona_lisa_scores_and_splays_the_color_when_the_committed_count_is_exact() {
    for (k, succeed) in [(1, true), (0, false)] {
        let mut l = Layout::default();
        let lisa = l.artifact("A039", board(A, Color::Yellow));
        l.tech(Color::Red, 1, board(A, Color::Red), &[]);
        l.tech(Color::Red, 2, board(A, Color::Red), &[]);
        let held = l.tech(Color::Red, 3, HAND_A, &[]);
        let others: Vec<_> = (0..4)
            .map(|_| l.tech(Color::Blue, 4, base_deck(4), &[]))
            .collect();
        let red_draw = l.tech(Color::Red, 4, base_deck(4), &[]);
        let mut game = l.game();
        dogma(&mut game, lisa);
        let (status, events) = play(&mut game, |decision| {
            decision
                .options
                .iter()
                .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
                .or_else(|| pick_value(decision, k))
                .or(Some(0))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit), "{k}");
        let moved = moves(&events);
        assert_eq!(
            moved.len(),
            5 + if succeed { 2 } else { 6 },
            "{k}: {moved:?}"
        );
        if succeed {
            assert!(moved[5..].iter().all(|m| m.0 == Some(Verb::Score)));
            assert!(
                moved[5..].iter().any(|m| m.1 == held)
                    && moved[5..].iter().any(|m| m.1 == red_draw)
            );
            assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
            assert_eq!(game.position().cards(HAND_A).len(), 4);
        } else {
            assert!(moved[5..].iter().all(|m| m.0 == Some(Verb::Return)));
            assert!(game.position().cards(HAND_A).is_empty());
        }
        let _ = others;
    }
}

#[test]
fn molasses_reef_caravel_returns_the_hand_then_draws_melds_green_and_junks_its_top_green_value() {
    let mut l = Layout::default();
    let caravel = l.artifact("A040", board(A, Color::Green));
    let held = l.tech(Color::Blue, 2, HAND_A, &[]);
    let green = l.tech(Color::Green, 4, base_deck(4), &[]);
    let blue = l.tech(Color::Blue, 4, base_deck(4), &[]);
    let red = l.tech(Color::Red, 4, base_deck(4), &[]);
    let four_left = l.tech(Color::Yellow, 4, base_deck(4), &[]);
    let mut game = l.game();
    dogma(&mut game, caravel);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // No green was drawn, so nothing melds; the top green is Caravel (4)
    // and the rest of the [4] deck is junked.
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Return), held, HAND_A, base_deck(2)),
            (None, four_left, base_deck(4), HAND_A),
            (None, red, base_deck(4), HAND_A),
            (None, blue, base_deck(4), HAND_A),
            (Some(Verb::Junk), green, base_deck(4), Zone::Junk),
        ]
    );
}

#[test]
fn hunt_lenox_globe_returns_non_green_tops_with_a_small_hand_then_melds() {
    let mut l = Layout::default();
    let globe = l.artifact("A041", board(A, Color::Green));
    let red = l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    let blue = l.tech(Color::Blue, 3, board(A, Color::Blue), &[]);
    let held = l.tech(Color::Purple, 1, HAND_A, &[]);
    let five_a = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let five_b = l.tech(Color::Yellow, 5, base_deck(5), &[]);
    let mut game = l.game();
    dogma(&mut game, globe);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, five_b).or_else(|| option_moving(decision, held))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 5, "{moved:?}");
    assert!(moved[..2].iter().any(|m| m.1 == red) && moved[..2].iter().any(|m| m.1 == blue));
    assert_eq!(moved[2].1, five_b);
    assert_eq!(moved[3].1, five_a);
    assert_eq!(
        moved[4],
        (Some(Verb::Meld), five_b, HAND_A, board(A, Color::Yellow))
    );
}

#[test]
fn petition_of_right_takes_one_scored_card_per_authority_color_and_junks_by_authority_count() {
    let mut l = Layout::default();
    let petition = l.artifact("A042", board(A, Color::Blue));
    l.tech(
        Color::Red,
        1,
        board(B, Color::Red),
        &[Icon::Authority, Icon::Authority, Icon::Authority],
    );
    l.tech(Color::Green, 1, board(B, Color::Green), &[Icon::Authority]);
    l.tech(
        Color::Yellow,
        1,
        board(B, Color::Yellow),
        &sharing_icons("A042"),
    );
    let s1 = l.tech(Color::Purple, 1, SCORE_B, &[]);
    let s2 = l.tech(Color::Purple, 2, SCORE_B, &[]);
    let s3 = l.tech(Color::Purple, 3, SCORE_B, &[]);
    // A's Authority count (a red top plus Petition's own) picks the junk.
    l.tech(
        Color::Red,
        1,
        board(A, Color::Red),
        &[Icon::Authority, Icon::Authority],
    );
    let authority = 2 + artifacts_card("A042")
        .unwrap()
        .printed_icon_count(Icon::Authority) as u8;
    let ach = l.tech(Color::Green, authority, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
    dogma(&mut game, petition);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, s1).or_else(|| option_moving(decision, s2))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(
        moved[..2]
            .iter()
            .all(|m| m.0 == Some(Verb::Transfer) && m.3 == SCORE_A)
    );
    assert_eq!(
        moved[2],
        (
            Some(Verb::Junk),
            ach,
            Zone::AvailableAchievements,
            Zone::Junk
        )
    );
    assert_eq!(game.position().cards(SCORE_B), &[s3]);
}

#[test]
fn delft_pocket_telescope_reveals_a_drawn_card_sharing_an_icon_or_returns_both_and_repeats() {
    let mut l = Layout::default();
    let telescope = l.artifact("A043", board(A, Color::Blue));
    let returned = l.tech(Color::Red, 2, SCORE_A, &[Icon::Authority]);
    // First pair: no shared icon; second pair: the [6] shares Authority.
    let six_hit = l.tech(Color::Green, 6, base_deck(6), &[Icon::Authority]);
    let five_second = l.tech(Color::Green, 5, base_deck(5), &[Icon::Health]);
    let six_miss = l.tech(Color::Yellow, 6, base_deck(6), &[Icon::Concept]);
    let five_miss = l.tech(Color::Yellow, 5, base_deck(5), &[Icon::Health]);
    let mut game = l.game();
    dogma(&mut game, telescope);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (Some(Verb::Return), returned, SCORE_A, base_deck(2))
    );
    assert_eq!(moved[1].1, five_miss);
    assert_eq!(moved[2].1, six_miss);
    assert!(moved[3..5].iter().all(|m| m.0 == Some(Verb::Return)));
    // The repeat returns another score card first: none left, so it stops.
    assert_eq!(moved.len(), 5, "{moved:?}");
    let _ = (six_hit, five_second);

    // A hit on the first pair: reveal it and stop.
    let mut l = Layout::default();
    let telescope = l.artifact("A043", board(A, Color::Blue));
    l.tech(Color::Red, 2, SCORE_A, &[Icon::Authority]);
    let six_hit = l.tech(Color::Green, 6, base_deck(6), &[Icon::Authority]);
    l.tech(Color::Yellow, 5, base_deck(5), &[Icon::Health]);
    let mut game = l.game();
    dogma(&mut game, telescope);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(moves(&events).len(), 3);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == six_hit))
    );
    assert_eq!(game.position().cards(HAND_A).len(), 2);
}

#[test]
fn cross_of_coronado_wins_with_exactly_five_cards_of_five_colors() {
    for (colors, wins) in [
        (
            [
                Color::Red,
                Color::Blue,
                Color::Green,
                Color::Yellow,
                Color::Purple,
            ],
            true,
        ),
        (
            [
                Color::Red,
                Color::Red,
                Color::Green,
                Color::Yellow,
                Color::Purple,
            ],
            false,
        ),
    ] {
        let mut l = Layout::default();
        let cross = l.artifact("A044", board(A, Color::Purple));
        for color in colors {
            l.tech(color, 1, HAND_A, &[]);
        }
        let mut game = l.game();
        dogma(&mut game, cross);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Revealed { .. }))
                .count(),
            5
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
fn abell_gallery_harpsichord_draws_and_scores_each_unique_top_value_ascending() {
    let mut l = Layout::default();
    let harpsichord = l.artifact("A045", board(A, Color::Purple));
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Blue, 3, board(A, Color::Blue), &[]);
    l.tech(Color::Green, 3, board(A, Color::Green), &[]);
    l.tech(Color::Yellow, 5, board(A, Color::Yellow), &[]);
    let one = l.tech(Color::Red, 1, base_deck(1), &[]);
    let three = l.tech(Color::Red, 3, base_deck(3), &[]);
    let four = l.tech(Color::Red, 4, base_deck(4), &[]);
    let five = l.tech(Color::Red, 5, base_deck(5), &[]);
    let mut game = l.game();
    dogma(&mut game, harpsichord);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // Harpsichord's own 4 is unique too: 1, 4 and 5 ascending; the 3 is twice.
    assert_eq!(
        moves(&events),
        [
            (None, one, base_deck(1), HAND_A),
            (Some(Verb::Score), one, HAND_A, SCORE_A),
            (None, four, base_deck(4), HAND_A),
            (Some(Verb::Score), four, HAND_A, SCORE_A),
            (None, five, base_deck(5), HAND_A),
            (Some(Verb::Score), five, HAND_A, SCORE_A),
        ]
    );
    let _ = three;
}

fn pick_count(decision: &Decision, count: u16) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Count(count))
}

fn pick_red(decision: &Decision) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
}

#[test]
fn mona_lisa_offers_the_literal_final_count_when_a_figure_can_change_the_hand() {
    // Marie Curie's extra 9 after each escalating draw makes ten draws of
    // which seven are red: only the literal count 7 can succeed.
    let mut l = Layout::default();
    let lisa = l.artifact("A039", Zone::Junk);
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    l.tech(Color::Red, 2, board(A, Color::Red), &[]);
    l.figure(Figure::MarieCurie, A);
    for age in 4..=8 {
        l.tech(Color::Red, age, base_deck(age), &[]);
    }
    for color in [
        Color::Red,
        Color::Red,
        Color::Blue,
        Color::Blue,
        Color::Green,
    ] {
        l.tech(color, 9, base_deck(9), &[]);
    }
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    game.self_execute(lisa, Context::new(A)).unwrap();
    let mut offered = Vec::new();
    let (status, events) = play(&mut game, |decision| {
        if let Some(index) = pick_red(decision) {
            return Some(index);
        }
        if decision.options.contains(&Alternative::Count(0)) {
            offered = decision.options.clone();
            return pick_count(decision, 7);
        }
        // Figures in play: the player orders the scores.
        Some(0)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // The enabled catalog's red cards (49 in a real Base + Figures game;
    // this fixture's roster here), plus the impossible count above them.
    let reds = game
        .position()
        .catalog
        .iter()
        .filter(|card| card.color == Color::Red)
        .count();
    assert_eq!(
        offered,
        (0..=reds as u16 + 1)
            .map(Alternative::Count)
            .collect::<Vec<_>>()
    );
    let moved = moves(&events);
    assert_eq!(moved.iter().filter(|m| m.0.is_none()).count(), 10);
    let scored: Vec<_> = moved.iter().filter(|m| m.0 == Some(Verb::Score)).collect();
    assert_eq!(scored.len(), 7);
    assert!(
        scored
            .iter()
            .all(|m| game.position().card(m.1).color == Color::Red)
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);

    // An unrelated Figure keeps the drawn-count menu.
    let mut l = Layout::default();
    let lisa = l.artifact("A039", Zone::Junk);
    l.figure(Figure::Michelangelo, A);
    for _ in 0..5 {
        l.tech(Color::Red, 4, base_deck(4), &[]);
    }
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    game.self_execute(lisa, Context::new(A)).unwrap();
    let mut offered = Vec::new();
    let (status, events) = play(&mut game, |decision| {
        if let Some(index) = pick_red(decision) {
            return Some(index);
        }
        if decision.options.contains(&Alternative::Value(6)) {
            offered = decision.options.clone();
            return pick_value(decision, 5);
        }
        Some(0)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(offered, (0..=6).map(Alternative::Value).collect::<Vec<_>>());
    assert_eq!(
        moves(&events)
            .iter()
            .filter(|m| m.0 == Some(Verb::Score))
            .count(),
        5
    );

    // Meiji's borrowed hand on the executor's own board: literal menu.
    let mut l = Layout::default();
    let lisa = l.artifact("A039", board(A, Color::Yellow));
    l.figure(Figure::EmperorMeiji, A);
    for _ in 0..5 {
        l.tech(Color::Red, 4, base_deck(4), &[]);
    }
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    dogma(&mut game, lisa);
    let mut literal = false;
    let (status, _) = play(&mut game, |decision| {
        if let Some(index) = pick_red(decision) {
            return Some(index);
        }
        literal |= decision.options.contains(&Alternative::Count(0));
        pick_count(decision, 5).or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(literal);
    assert_eq!(game.position().cards(SCORE_A).len(), 5);
}
