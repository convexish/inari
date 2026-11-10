//! Age 2 Artifacts (A016-A025): expected outcomes on explicit positions.
use inno_engine::*;

mod common;
use common::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);

fn artifact_deck(age: u8) -> Zone {
    Zone::Deck(Deck {
        set: CardSet::Artifacts,
        age,
    })
}

fn with_effect(mut card: Card, instructions: Vec<Instruction>) -> Card {
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    card
}

#[test]
fn chronicle_of_zuo_draws_when_tied_or_behind_on_icons() {
    let mut f = Fixture::default();
    let chronicle = f.card("A016", board(A, Color::Red));
    // With Chronicle's own icons A has 1 Authority, 1 Prosperity and 4
    // Concept; B has 2 Authority and 4 Concept: least Authority, tied Concept.
    f.plain(
        Color::Blue,
        1,
        board(A, Color::Blue),
        &[Icon::Authority, Icon::Concept, Icon::Concept],
    );
    f.plain(
        Color::Green,
        1,
        board(B, Color::Green),
        &[
            Icon::Authority,
            Icon::Authority,
            Icon::Concept,
            Icon::Concept,
        ],
    );
    f.plain(
        Color::Purple,
        1,
        board(B, Color::Purple),
        &[Icon::Concept, Icon::Concept],
    );
    for _ in 0..2 {
        f.plain(Color::Yellow, 3, base_deck(3), &[]);
        f.plain(Color::Yellow, 4, base_deck(4), &[]);
    }
    let reward = f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, chronicle);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // B shares (tied Concept): least Prosperity draws a 3 and tied Concept a
    // 4; A draws both too (least Authority, tied Concept), then a reward.
    let moved = moves(&events);
    assert_eq!(moved.len(), 5, "{moved:?}");
    assert_eq!(moved[0].3, HAND_B);
    assert_eq!(moved[1].3, HAND_A);
    assert_eq!(moved[2].3, HAND_B);
    assert_eq!(moved[3].3, HAND_A);
    assert_eq!(moved[4], (None, reward, base_deck(2), HAND_A));
}

#[test]
fn babylonian_chronicles_takes_a_non_red_authority_top_then_scores_a_three() {
    let mut f = Fixture::default();
    let chronicles = f.card("A017", board(A, Color::Red));
    let red = f.plain(Color::Red, 2, board(B, Color::Red), &sharing_icons("A017"));
    let blue = f.plain(Color::Blue, 2, board(B, Color::Blue), &[Icon::Authority]);
    let three_a = f.plain(Color::Yellow, 3, base_deck(3), &[]);
    let three_b = f.plain(Color::Yellow, 3, base_deck(3), &[]);
    f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, chronicles);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (
            Some(Verb::Transfer),
            blue,
            board(B, Color::Blue),
            board(A, Color::Blue)
        )
    );
    // B shares the draw-and-score, then A, then A draws for sharing.
    assert_eq!(moved[1], (None, three_b, base_deck(3), HAND_B));
    assert_eq!(
        moved[2],
        (Some(Verb::Score), three_b, HAND_B, Zone::Score(B))
    );
    assert_eq!(moved[3], (None, three_a, base_deck(3), HAND_A));
    assert_eq!(moved[4], (Some(Verb::Score), three_a, HAND_A, SCORE_A));
    assert_eq!(game.position().top(B, Color::Red), Some(red));
}

#[test]
fn holy_grail_claims_the_returned_value_ignoring_eligibility() {
    let mut f = Fixture::default();
    let grail = f.card("A018", board(A, Color::Yellow));
    let returned = f.plain(Color::Red, 4, HAND_A, &[]);
    let ach4 = f.plain(Color::Green, 4, Zone::AvailableAchievements, &[]);
    f.plain(Color::Green, 3, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, grail);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Return), returned, HAND_A, base_deck(4)),
            (
                Some(Verb::Achieve),
                ach4,
                Zone::AvailableAchievements,
                Zone::Achievements(A)
            ),
        ]
    );
}

#[test]
fn terracotta_army_junks_the_scored_value_or_tucks_itself() {
    // Score a 2 without Authority: junk the [2] deck.
    let mut f = Fixture::default();
    let army = f.card("A019", board(A, Color::Yellow));
    let plain = f.plain(Color::Red, 2, HAND_A, &[]);
    f.plain(Color::Blue, 2, HAND_A, &[Icon::Authority]);
    let two = f.plain(Color::Green, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, army);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Score), plain, HAND_A, SCORE_A),
            (Some(Verb::Junk), two, base_deck(2), Zone::Junk),
        ]
    );

    // Only Authority cards in hand: Terracotta Army tucks under its pile.
    let mut f = Fixture::default();
    let under = f.plain(Color::Yellow, 1, board(A, Color::Yellow), &[]);
    let army = f.card("A019", board(A, Color::Yellow));
    f.plain(Color::Blue, 2, HAND_A, &[Icon::Authority]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, army);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Tuck),
            army,
            board(A, Color::Yellow),
            board(A, Color::Yellow)
        )]
    );
    assert_eq!(
        game.position().cards(board(A, Color::Yellow)),
        &[army, under]
    );

    // The compel returns a top without Authority.
    let mut f = Fixture::default();
    let army = f.card("A019", board(A, Color::Yellow));
    let b_top = f.plain(Color::Red, 3, board(B, Color::Red), &sharing_icons("A019"));
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, army);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events)[0],
        (
            Some(Verb::Return),
            b_top,
            board(B, Color::Red),
            base_deck(3)
        )
    );
}

#[test]
fn holy_lance_takes_a_top_artifact_and_wins_with_holy_grail_on_top() {
    let mut f = Fixture::default();
    let lance = f.card("A020", board(A, Color::Green));
    let b_artifact = f.card("A018", board(B, Color::Yellow));
    f.plain(Color::Red, 1, board(B, Color::Red), &sharing_icons("A020"));
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, lance);
    let (status, events) = no_decisions(&mut game);
    // Holy Grail arrives on A's board as a top card: A wins.
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Transfer),
            b_artifact,
            board(B, Color::Yellow),
            board(A, Color::Yellow)
        )]
    );
    assert_eq!(status, Status::Winner(A));
}

#[test]
fn baghdad_battery_junks_both_decks_only_for_different_values() {
    for (score_value, junked) in [(5, 2), (3, 0)] {
        let mut f = Fixture::default();
        let battery = f.card("A021", board(A, Color::Green));
        let melded = f.plain(Color::Red, 3, HAND_A, &[]);
        let scored = f.plain(Color::Blue, score_value, HAND_A, &[]);
        f.plain(Color::Yellow, 3, base_deck(3), &[]);
        f.plain(Color::Yellow, 5, base_deck(5), &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, battery);
        let (status, events) = play(&mut game, |decision| {
            option_moving(decision, melded).or_else(|| option_moving(decision, scored))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        assert_eq!(moved[0].0, Some(Verb::Meld));
        assert_eq!(moved[1].0, Some(Verb::Score));
        assert_eq!(
            moved[2..]
                .iter()
                .filter(|m| m.0 == Some(Verb::Junk))
                .count(),
            junked
        );
    }
}

#[test]
fn seikilos_epitaph_melds_the_bottom_of_the_drawn_color_and_self_executes_it() {
    let mut f = Fixture::default();
    let epitaph = f.card("A022", board(A, Color::Blue));
    let bottom = f.add(
        with_effect(Card::from_code("B001").unwrap(), vec![build::draw(1)]),
        board(A, Color::Red),
    );
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let drawn = f.plain(Color::Red, 3, base_deck(3), &[]);
    let one = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, epitaph);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, drawn, base_deck(3), HAND_A),
            (Some(Verb::Meld), drawn, HAND_A, board(A, Color::Red)),
            (
                Some(Verb::Meld),
                bottom,
                board(A, Color::Red),
                board(A, Color::Red)
            ),
            (None, one, base_deck(1), HAND_A),
        ]
    );
    assert_eq!(game.position().top(A, Color::Red), Some(bottom));
}

#[test]
fn rosetta_stone_draws_two_from_the_chosen_set_melds_one_and_gives_the_other() {
    let mut f = Fixture::default();
    let stone = f.card("A023", board(A, Color::Blue));
    let first = f.plain(Color::Red, 2, base_deck(2), &[]);
    let second = f.plain(Color::Green, 2, base_deck(2), &[]);
    f.card("A016", artifact_deck(2));
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, stone);
    let mut set_choices = 0;
    let (status, events) = play(&mut game, |decision| {
        if let Some(index) = decision.options.iter().position(|o| {
            matches!(
                o,
                Alternative::Draw {
                    set: CardSet::Base,
                    ..
                }
            )
        }) {
            set_choices += 1;
            return Some(index);
        }
        option_moving(decision, first)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(set_choices, 1, "one set choice for both draws");
    assert_eq!(
        moves(&events),
        [
            (None, second, base_deck(2), HAND_A),
            (None, first, base_deck(2), HAND_A),
            (Some(Verb::Meld), first, HAND_A, board(A, Color::Red)),
            (Some(Verb::Transfer), second, HAND_A, board(B, Color::Green)),
        ]
    );
}

#[test]
fn dead_sea_scrolls_draws_an_artifact_at_the_highest_top_then_junks_that_artifact_deck() {
    let mut f = Fixture::default();
    let scrolls = f.card("A024", board(A, Color::Purple));
    // Highest top is Dead Sea Scrolls itself: a 2.
    let left = f.card("A016", artifact_deck(2));
    let drawn = f.card("A017", artifact_deck(2));
    let _ = left;
    f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let ach4 = f.plain(Color::Green, 2, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, scrolls);
    // B's empty board stands for value 0: choose 0 or the highest top value.
    let (status, events) = play(&mut game, |decision| {
        assert!(matches!(
            decision.options[..],
            [Alternative::Branch(0), Alternative::Branch(1)]
        ));
        Some(1)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, drawn, artifact_deck(2), HAND_A),
            (Some(Verb::Junk), left, artifact_deck(2), Zone::Junk),
            (
                Some(Verb::Junk),
                ach4,
                Zone::AvailableAchievements,
                Zone::Junk
            ),
        ]
    );
}

#[test]
fn dead_sea_scrolls_junks_a_whole_artifact_deck_without_asking_its_order() {
    let mut f = Fixture::default();
    let scrolls = f.card("A024", board(A, Color::Purple));
    let bottom = f.card("A016", artifact_deck(2));
    let top = f.card("A018", artifact_deck(2));
    let drawn = f.card("A017", artifact_deck(2));
    f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let ach = f.plain(Color::Green, 2, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, scrolls);
    // The only question is B's empty board: every junk order is the same.
    let (status, events) = play(&mut game, |decision| {
        assert!(matches!(
            decision.options[..],
            [Alternative::Branch(0), Alternative::Branch(1)]
        ));
        Some(1)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, drawn, artifact_deck(2), HAND_A),
            (Some(Verb::Junk), top, artifact_deck(2), Zone::Junk),
            (Some(Verb::Junk), bottom, artifact_deck(2), Zone::Junk),
            (
                Some(Verb::Junk),
                ach,
                Zone::AvailableAchievements,
                Zone::Junk
            ),
        ]
    );
}

#[test]
fn dead_sea_scrolls_still_chooses_the_set_among_same_value_achievements() {
    // Equal highest tops skip the value, not which achievement of it.
    let mut f = Fixture::default();
    let scrolls = f.card("A024", board(A, Color::Purple));
    f.card("A016", artifact_deck(2));
    f.card("A017", artifact_deck(2));
    for age in 2..=4 {
        f.plain(Color::Yellow, age, base_deck(age), &[]);
    }
    f.plain(Color::Red, 2, board(B, Color::Red), &[]);
    f.plain(Color::Green, 2, Zone::AvailableAchievements, &[]);
    let echo = f.add(
        Card::from_code("E016").unwrap(),
        Zone::AvailableAchievements,
    );
    let mut game = f.game(Expansions::ARTIFACTS.union(Expansions::ECHOES));
    dogma(&mut game, scrolls);
    let mut asked = Vec::new();
    let (status, events) = play(&mut game, |decision| {
        asked.push(decision.options.clone());
        decision.options.iter().position(|o| {
            matches!(
                o,
                Alternative::BlindMove {
                    back: Deck {
                        set: CardSet::Echoes,
                        ..
                    },
                    ..
                }
            )
        })
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].len(), 2, "{asked:?}");
    assert!(
        moves(&events)
            .iter()
            .any(|m| m.1 == echo && m.3 == Zone::Junk)
    );
}

#[test]
fn dead_sea_scrolls_chooses_between_the_two_highest_top_values() {
    let mut f = Fixture::default();
    let scrolls = f.card("A024", board(A, Color::Purple));
    f.card("A016", artifact_deck(2));
    f.card("A017", artifact_deck(2));
    for age in 2..=4 {
        f.plain(Color::Yellow, age, base_deck(age), &[]);
    }
    f.plain(Color::Red, 3, board(B, Color::Red), &[]);
    f.plain(Color::Green, 2, Zone::AvailableAchievements, &[]);
    let three = f.plain(Color::Green, 3, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, scrolls);
    let mut offered = Vec::new();
    let (status, events) = play(&mut game, |decision| {
        offered = decision.options.clone();
        decision
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Value(3)))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(offered, [Alternative::Value(2), Alternative::Value(3)]);
    let junked: Vec<_> = moves(&events)
        .into_iter()
        .filter(|m| m.3 == Zone::Junk && m.2 == Zone::AvailableAchievements)
        .map(|m| m.1)
        .collect();
    assert_eq!(junked, [three]);
}

#[test]
fn dead_sea_scrolls_asks_for_a_player_only_when_the_answer_matters() {
    // Equal highest tops, or no achievement of either value: both answers
    // junk the same card or none, so nothing is asked.
    for (b_top, achievement) in [(2, 2), (3, 5)] {
        let mut f = Fixture::default();
        let scrolls = f.card("A024", board(A, Color::Purple));
        f.card("A016", artifact_deck(2));
        f.card("A017", artifact_deck(2));
        for age in 2..=4 {
            f.plain(Color::Yellow, age, base_deck(age), &[]);
        }
        f.plain(Color::Red, b_top, board(B, Color::Red), &[]);
        let ach = f.plain(Color::Green, achievement, Zone::AvailableAchievements, &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, scrolls);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let junked = moves(&events).iter().any(|m| m.1 == ach);
        assert_eq!(junked, achievement == 2, "B top {b_top}");
    }
}

#[test]
fn cyrus_cylinder_splays_any_board_left_and_self_executes_another_purple_top() {
    let mut f = Fixture::default();
    let cylinder = f.card("A025", board(A, Color::Purple));
    f.plain(Color::Red, 1, board(B, Color::Red), &[]);
    f.plain(Color::Red, 2, board(B, Color::Red), &[]);
    let mut purple = with_effect(Card::from_code("B001").unwrap(), vec![build::draw(1)]);
    purple.color = Color::Purple;
    let b_purple = f.add(purple, board(B, Color::Purple));
    let one = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, cylinder);
    let (status, events) = play(&mut game, |decision| {
        decision.options.iter().position(|o| {
            matches!(
                o,
                Alternative::Splay {
                    player: B,
                    color: Color::Red,
                    ..
                }
            )
        })
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().splay(B, Color::Red), Splay::Left);
    // The copied purple draw is A's.
    assert_eq!(moves(&events), [(None, one, base_deck(1), HAND_A)]);
    let _ = b_purple;
}

fn choose_base_draw(decision: &Decision) -> Option<usize> {
    decision.options.iter().position(|o| {
        matches!(
            o,
            Alternative::Draw {
                set: CardSet::Base,
                ..
            }
        )
    })
}

#[test]
fn choosing_base_from_any_set_is_never_redirected_to_echoes() {
    // Rosetta Stone: A has a unique highest top, which redirects ordinary
    // draws to Echoes, but the explicit Base choice draws from Base.
    let mut f = Fixture::default();
    let stone = f.card("A023", board(A, Color::Blue));
    let first = f.plain(Color::Red, 2, base_deck(2), &[]);
    let second = f.plain(Color::Green, 2, base_deck(2), &[]);
    let echo = f.in_deck("E016");
    let echo_deck = Zone::Deck(Card::from_code("E016").unwrap().origin_deck());
    let mut game = f.game(Expansions::ARTIFACTS.union(Expansions::ECHOES));
    dogma(&mut game, stone);
    let (status, events) = play(&mut game, |decision| {
        choose_base_draw(decision).or_else(|| option_moving(decision, first))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events)[..2],
        [
            (None, second, base_deck(2), HAND_A),
            (None, first, base_deck(2), HAND_A),
        ]
    );
    assert_eq!(game.position().cards(echo_deck), &[echo]);

    // Papyrus of Ani's single any-set draw likewise.
    let mut f = Fixture::default();
    let papyrus = f.card("A002", board(A, Color::Red));
    let returned = f.plain(Color::Purple, 1, HAND_A, &[]);
    let base = f.plain(Color::Green, 3, base_deck(3), &[]);
    let echo = f.in_deck("E026");
    let echo_deck = Zone::Deck(Card::from_code("E026").unwrap().origin_deck());
    let mut game = f.game(Expansions::ARTIFACTS.union(Expansions::ECHOES));
    dogma(&mut game, papyrus);
    let (status, events) = play(&mut game, |decision| {
        choose_base_draw(decision).or_else(|| option_moving(decision, returned))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(moves(&events).contains(&(None, base, base_deck(3), HAND_A)));
    assert_eq!(game.position().cards(echo_deck), &[echo]);
}
