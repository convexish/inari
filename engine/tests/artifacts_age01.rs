//! Age 1 Artifacts (A001-A015): expected outcomes on explicit positions.
use inno_engine::*;

mod common;
use common::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

/// A printed Base card recolored, with these effects.
fn program(color: Color, value: u8, effects: Vec<Effect>) -> Card {
    Card {
        code: "synthetic-base-program",
        set: CardSet::Base,
        color,
        value,
        figure: None,
        echo: None,
        symbols: [Symbol::Blank; SLOTS],
        featured: Some(Icon::Authority),
        effects,
    }
}

fn non_demand(instructions: Vec<Instruction>) -> Effect {
    Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }
}

#[test]
fn holmegaard_bows_takes_the_highest_authority_top_or_junks_the_lowest_tops_deck() {
    // B has two Authority tops: the higher goes to A's hand; then A draws a 2.
    let mut f = Fixture::default();
    let bows = f.card("A001", board(A, Color::Red));
    let low = f.plain(
        Color::Blue,
        2,
        board(B, Color::Blue),
        &[Icon::Authority, Icon::Health],
    );
    let high = f.plain(
        Color::Green,
        4,
        board(B, Color::Green),
        &[Icon::Authority, Icon::Health],
    );
    let a_two = f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let b_two = f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let reward = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, bows);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(
        status,
        Status::Complete(ReturnValue::Unit),
        "{:?}",
        moves(&events)
    );
    // B shares the second effect too, then A draws for sharing.
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Transfer), high, board(B, Color::Green), HAND_A),
            (None, b_two, base_deck(2), HAND_B),
            (None, a_two, base_deck(2), HAND_A),
            (None, reward, base_deck(1), HAND_A),
        ]
    );
    assert_eq!(game.position().top(B, Color::Blue), Some(low));

    // No Authority top: junk the deck of B's lowest top value (3).
    let mut f = Fixture::default();
    let bows = f.card("A001", board(A, Color::Red));
    let b_top = f.plain(
        Color::Blue,
        3,
        board(B, Color::Blue),
        &[Icon::Health, Icon::Health],
    );
    f.plain(Color::Green, 5, board(B, Color::Green), &[]);
    let three = f.plain(Color::Yellow, 3, base_deck(3), &[]);
    f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, bows);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events)[0],
        (Some(Verb::Junk), three, base_deck(3), Zone::Junk)
    );
    assert_eq!(game.position().top(B, Color::Blue), Some(b_top));
}

#[test]
fn papyrus_of_ani_draws_from_a_chosen_set_and_self_executes_a_purple_draw() {
    let mut f = Fixture::default();
    let papyrus = f.card("A002", board(A, Color::Red));
    let returned = f.plain(Color::Purple, 1, HAND_A, &[]);
    // Base 3 top is purple with a draw effect; the Artifact 3 deck is empty
    // but the set is still offered.
    let purple = f.add(
        program(Color::Purple, 3, vec![non_demand(vec![build::draw(1)])]),
        base_deck(3),
    );
    let one = f.plain(Color::Blue, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, papyrus);
    let mut sets = Vec::new();
    let (status, events) = play(&mut game, |decision| {
        sets = decision.options.clone();
        decision.options.iter().position(|o| {
            matches!(
                o,
                Alternative::Draw {
                    set: CardSet::Base,
                    ..
                }
            )
        })
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        sets,
        [
            Alternative::Draw {
                value: 3,
                set: CardSet::Base
            },
            Alternative::Draw {
                value: 3,
                set: CardSet::Artifacts
            }
        ]
    );
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Return), returned, HAND_A, base_deck(1)),
            (None, purple, base_deck(3), HAND_A),
            (Some(Verb::Meld), purple, HAND_A, board(A, Color::Purple)),
            (None, one, base_deck(1), HAND_A),
        ]
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == purple))
    );
}

#[test]
fn pavlovian_tusk_draws_three_at_the_green_top_value_then_returns_and_scores_drawn_cards() {
    let mut f = Fixture::default();
    let tusk = f.card("A003", board(A, Color::Red));
    f.plain(Color::Green, 1, board(A, Color::Green), &[]);
    let drawn: Vec<_> = (0..3)
        .map(|_| f.plain(Color::Blue, 1, base_deck(1), &[]))
        .collect();
    let kept = f.plain(Color::Red, 5, HAND_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, tusk);
    let (status, events) = play(&mut game, |decision| {
        // The held 5 is never offered; return then score different drawn cards.
        assert!(option_moving(decision, kept).is_none());
        option_moving(decision, drawn[2]).or_else(|| option_moving(decision, drawn[1]))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 5);
    assert_eq!(moved[3].0, Some(Verb::Return));
    assert_eq!(moved[4].0, Some(Verb::Score));
    assert_ne!(moved[3].1, moved[4].1);
    assert_eq!(game.position().cards(HAND_A).len(), 2);
    assert_eq!(game.position().cards(SCORE_A).len(), 1);
}

#[test]
fn dancing_girl_wins_after_its_compel_put_it_on_the_opponents_board() {
    for b_shares in [true, false] {
        let mut f = Fixture::default();
        let girl = f.card("A004", board(A, Color::Yellow));
        let icons: &[Icon] = if b_shares {
            &[Icon::Concept, Icon::Concept, Icon::Concept]
        } else {
            &[]
        };
        f.plain(Color::Blue, 1, board(B, Color::Blue), icons);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, girl);
        let (status, events) = no_decisions(&mut game);
        if b_shares {
            assert_eq!(
                moves(&events),
                [(
                    Some(Verb::Transfer),
                    girl,
                    board(A, Color::Yellow),
                    board(B, Color::Yellow)
                )]
            );
            assert_eq!(status, Status::Winner(A));
        } else {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert!(moves(&events).is_empty());
        }
    }
}

/// The common case: A executes Dancing Girl where it lies, on B's board
/// (Gujin Tushu Jicheng super-executes it, skipping the compel), and wins.
#[test]
fn dancing_girl_executed_on_the_opponents_board_wins() {
    let mut f = Fixture::default();
    let gujin = Card::from_code("A048").unwrap();
    let color = gujin.color;
    let gujin = f.add(gujin, board(A, color));
    let girl = f.card("A004", board(B, Color::Yellow));
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, gujin);
    let (status, events) = play(&mut game, |decision| {
        decision.options.iter().position(|option| {
            matches!(
                option,
                Alternative::Select { target: TargetDescriptor::Known { card, .. } }
                if *card == girl
            )
        })
    });
    assert!(moves(&events).is_empty());
    assert_eq!(status, Status::Winner(A));
}

/// Dancing Girl counts the board it lay on when the action began, not only
/// its compel's transfer: A's Deepfake takes it from B's board and
/// super-executes it, skipping the compel, and A wins.
#[test]
fn dancing_girl_counts_the_board_it_lay_on_when_the_action_began() {
    let mut f = Fixture::default();
    let deepfake = f.add(Card::from_code("E114").unwrap(), board(A, Color::Purple));
    let girl = f.card("A004", board(B, Color::Yellow));
    f.plain(Color::Blue, 1, board(B, Color::Blue), &[]);
    let mut game = f.game(Expansions::ARTIFACTS.union(Expansions::ECHOES));
    dogma(&mut game, deepfake);
    let (status, events) = play(&mut game, |decision| {
        decision.options.iter().position(|option| {
            matches!(
                option,
                Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. }
                | Alternative::Select { target: TargetDescriptor::Known { card, .. } }
                if *card == girl
            )
        })
    });
    assert_eq!(
        moves(&events)[0],
        (
            Some(Verb::Transfer),
            girl,
            board(B, Color::Yellow),
            board(A, Color::Yellow)
        )
    );
    assert_eq!(status, Status::Winner(A));
}

#[test]
fn lurgan_canoe_scores_the_covered_cards_of_the_melded_color_and_repeats() {
    let mut f = Fixture::default();
    let canoe = f.card("A005", board(A, Color::Yellow));
    let under_a = f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let under_b = f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let red = f.plain(Color::Red, 3, HAND_A, &[]);
    let blue = f.plain(Color::Blue, 3, HAND_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, canoe);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, red).or_else(|| option_moving(decision, blue))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (Some(Verb::Meld), red, HAND_A, board(A, Color::Red))
    );
    assert_eq!(moved[1].0, Some(Verb::Score));
    assert_eq!(moved[2].0, Some(Verb::Score));
    // The repeat melds the blue card, which scores nothing: no further repeat.
    assert_eq!(
        moved[3],
        (Some(Verb::Meld), blue, HAND_A, board(A, Color::Blue))
    );
    assert_eq!(moved.len(), 4);
    assert_eq!(game.position().cards(board(A, Color::Red)), &[red]);
    let mut scored = game.position().cards(SCORE_A).to_vec();
    scored.sort();
    assert_eq!(scored, [under_a, under_b]);
}

#[test]
fn xianrendong_shards_reveals_three_scores_two_tucks_one_and_rewards_a_same_color_pair() {
    for same in [true, false] {
        let mut f = Fixture::default();
        let shards = f.card("A006", board(A, Color::Yellow));
        let r1 = f.plain(Color::Red, 2, HAND_A, &[]);
        let r2 = f.plain(if same { Color::Red } else { Color::Blue }, 3, HAND_A, &[]);
        let g = f.plain(Color::Green, 4, HAND_A, &[]);
        for _ in 0..3 {
            f.plain(Color::Purple, 1, base_deck(1), &[]);
        }
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, shards);
        let (status, events) = play(&mut game, |decision| {
            // Score r1 and r2 whenever offered; reveal in any order.
            option_moving(decision, r1)
                .or_else(|| option_moving(decision, r2))
                .or(Some(0))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit), "{same}");
        let reveals = events
            .iter()
            .filter(|e| matches!(e, Event::Revealed { .. }))
            .count();
        assert_eq!(reveals, 3);
        let moved = moves(&events);
        assert_eq!(moved[0].0, Some(Verb::Score));
        assert_eq!(moved[1].0, Some(Verb::Score));
        assert_eq!(
            moved[2],
            (Some(Verb::Tuck), g, HAND_A, board(A, Color::Green))
        );
        assert_eq!(moved.len(), if same { 6 } else { 3 }, "{same}");
        assert_eq!(
            game.position().cards(HAND_A).len(),
            if same { 3 } else { 0 }
        );
    }
}

#[test]
fn priest_king_super_executes_the_matching_top_on_your_turn() {
    // A's turn: the matching top's demand runs against B (super-execution).
    let mut f = Fixture::default();
    let priest = f.card("A007", board(A, Color::Green));
    let mut oars = Card::from_code("B003").unwrap();
    oars.color = Color::Blue;
    f.add(oars, board(A, Color::Blue));
    let scored = f.plain(Color::Blue, 2, HAND_A, &[]);
    let prosperity = f.plain(Color::Red, 1, HAND_B, &[Icon::Prosperity]);
    f.plain(Color::Yellow, 1, base_deck(1), &[]);
    f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, priest);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved[0], (Some(Verb::Score), scored, HAND_A, SCORE_A));
    assert_eq!(
        moved[1],
        (Some(Verb::Transfer), prosperity, HAND_B, SCORE_A)
    );
    assert!(moved.len() >= 3);
}

#[test]
fn electrum_stater_melds_and_repeats_until_a_drawn_color_is_already_on_the_board() {
    let mut f = Fixture::default();
    let stater = f.card("A008", board(A, Color::Green));
    f.plain(Color::Blue, 3, base_deck(3), &[]);
    f.plain(Color::Red, 3, base_deck(3), &[]);
    let green = f.plain(Color::Green, 3, base_deck(3), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, stater);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // Green is drawn first (deck top): a green top exists, so it stays in hand.
    assert_eq!(moves(&events), [(None, green, base_deck(3), HAND_A)]);

    let mut f = Fixture::default();
    let stater = f.card("A008", board(A, Color::Green));
    let green = f.plain(Color::Green, 3, base_deck(3), &[]);
    let red = f.plain(Color::Red, 3, base_deck(3), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, stater);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, red, base_deck(3), HAND_A),
            (Some(Verb::Meld), red, HAND_A, board(A, Color::Red)),
            (None, green, base_deck(3), HAND_A),
        ]
    );
}

#[test]
fn jiskairumoko_necklace_moves_a_matching_achievement_and_junks_that_deck() {
    let mut f = Fixture::default();
    let necklace = f.card("A009", board(A, Color::Green));
    f.plain(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &[Icon::Authority, Icon::Authority],
    );
    let scored = f.plain(Color::Red, 2, SCORE_B, &[]);
    let ach2 = f.plain(Color::Yellow, 2, Zone::Achievements(B), &[]);
    let ach3 = f.plain(Color::Yellow, 3, Zone::Achievements(B), &[]);
    let two = f.plain(Color::Purple, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, necklace);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // The returned card now sits in that deck, so the sweep junks it too.
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Return), scored, SCORE_B, base_deck(2)),
            (
                Some(Verb::Transfer),
                ach2,
                Zone::Achievements(B),
                Zone::Achievements(A)
            ),
            (Some(Verb::Junk), two, base_deck(2), Zone::Junk),
            (Some(Verb::Junk), scored, base_deck(2), Zone::Junk),
        ]
    );
    assert_eq!(game.position().cards(Zone::Achievements(B)), &[ach3]);
    assert!(game.position().cards(base_deck(2)).is_empty());
}

#[test]
fn treaty_of_kadesh_returns_one_demand_top_per_color_and_scores_a_non_blue_demand_top() {
    let mut f = Fixture::default();
    let treaty = f.card("A010", board(A, Color::Blue));
    let a_demand = f.add(Card::from_code("B003").unwrap(), board(A, Color::Red));
    let b_oars = f.add(Card::from_code("B003").unwrap(), board(B, Color::Red));
    let mut blue_demand = Card::from_code("B003").unwrap();
    blue_demand.color = Color::Blue;
    blue_demand.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
    let b_blue_demand = f.add(blue_demand, board(B, Color::Blue));
    let b_plain = f.plain(
        Color::Green,
        1,
        board(B, Color::Green),
        &[Icon::Authority; 4],
    );
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, treaty);
    // Both demand tops are 1s: B orders their returns into that deck.
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, B);
        Some(0)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 3, "{moved:?}");
    assert!(moved[..2].iter().all(|m| m.0 == Some(Verb::Return)));
    assert!(
        moved[..2].iter().any(|m| m.1 == b_oars) && moved[..2].iter().any(|m| m.1 == b_blue_demand)
    );
    assert_eq!(
        moved[2],
        (Some(Verb::Score), a_demand, board(A, Color::Red), SCORE_A)
    );
    assert_eq!(game.position().top(B, Color::Green), Some(b_plain));
}

#[test]
fn sibidu_needle_scores_and_repeats_while_the_drawn_card_matches_a_top() {
    let mut f = Fixture::default();
    let needle = f.card("A011", board(A, Color::Blue));
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let miss = f.plain(Color::Green, 1, base_deck(1), &[]);
    let hit = f.plain(Color::Red, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, needle);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, hit, base_deck(1), HAND_A),
            (Some(Verb::Score), hit, HAND_A, SCORE_A),
            (None, miss, base_deck(1), HAND_A),
        ]
    );
}

#[test]
fn basur_hoyuk_tokens_returns_the_draw_and_the_score_pile_when_an_earlier_title_tops_its_color() {
    let gunpowder = BASE_METADATA
        .iter()
        .find(|m| m.title == "GUNPOWDER")
        .unwrap()
        .code;
    let archery = BASE_METADATA
        .iter()
        .find(|m| m.title == "ARCHERY")
        .unwrap()
        .code;
    assert_eq!(Card::from_code(gunpowder).unwrap().color, Color::Red);
    let mut f = Fixture::default();
    let tokens = f.card("A012", board(A, Color::Blue));
    f.card(archery, board(A, Color::Red));
    let drawn = f.card(gunpowder, base_deck(4));
    let scored = f.plain(Color::Green, 2, SCORE_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, tokens);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, drawn, base_deck(4), HAND_A),
            (Some(Verb::Return), drawn, HAND_A, base_deck(4)),
            (Some(Verb::Return), scored, SCORE_A, base_deck(2)),
        ]
    );

    // "Writing" comes after "Gunpowder": nothing returns.
    let writing = BASE_METADATA
        .iter()
        .find(|m| m.title == "WRITING")
        .unwrap()
        .code;
    let mut f = Fixture::default();
    let tokens = f.card("A012", board(A, Color::Blue));
    let mut top = Card::from_code(writing).unwrap();
    top.color = Color::Red;
    f.add(top, board(A, Color::Red));
    let drawn = f.card(gunpowder, base_deck(4));
    f.plain(Color::Green, 2, SCORE_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, tokens);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(moves(&events), [(None, drawn, base_deck(4), HAND_A)]);
}

#[test]
fn mask_of_warka_returns_the_chosen_color_and_claims_matching_achievements_only_when_alone() {
    for b_has_red in [false, true] {
        let mut f = Fixture::default();
        let mask = f.card("A013", board(A, Color::Purple));
        f.plain(Color::Red, 2, HAND_A, &[]);
        f.plain(Color::Red, 5, HAND_A, &[]);
        let blue = f.plain(Color::Blue, 3, HAND_A, &[]);
        if b_has_red {
            f.plain(Color::Red, 1, HAND_B, &[]);
        }
        let ach2 = f.plain(Color::Green, 2, Zone::AvailableAchievements, &[]);
        let ach5 = f.plain(Color::Green, 5, Zone::AvailableAchievements, &[]);
        let ach6 = f.plain(Color::Green, 6, Zone::AvailableAchievements, &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, mask);
        let (status, events) = play(&mut game, |decision| {
            decision
                .options
                .iter()
                .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let moved = moves(&events);
        if b_has_red {
            assert!(moved.is_empty());
        } else {
            assert_eq!(moved.len(), 4, "{moved:?}");
            assert!(moved[..2].iter().all(|m| m.0 == Some(Verb::Return)));
            assert!(moved[2..].iter().all(|m| m.0 == Some(Verb::Achieve)));
            assert_eq!(game.position().cards(HAND_A), &[blue]);
            let mut claimed = game.position().cards(Zone::Achievements(A)).to_vec();
            claimed.sort();
            assert_eq!(claimed, [ach2, ach5]);
            assert_eq!(game.position().cards(Zone::AvailableAchievements), &[ach6]);
        }
    }
}

#[test]
fn ark_of_the_covenant_scores_the_color_on_boards_without_top_artifacts_and_returns_to_hand() {
    let mut f = Fixture::default();
    let ark = f.card("A014", board(A, Color::Purple));
    let returned = f.plain(Color::Red, 1, HAND_A, &[]);
    // A's board has a top Artifact (Ark): its reds stay. B's reds score.
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let b_under = f.plain(Color::Red, 1, board(B, Color::Red), &[]);
    let b_top = f.plain(Color::Red, 3, board(B, Color::Red), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, ark);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, b_top).or_else(|| option_moving(decision, b_under))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (Some(Verb::Return), returned, HAND_A, base_deck(1))
    );
    assert_eq!(moved.len(), 4, "{moved:?}");
    assert!(
        moved[1..3]
            .iter()
            .all(|m| m.0 == Some(Verb::Score) && m.3 == SCORE_A)
    );
    // Ark is still a top card: the second effect takes it to hand.
    assert_eq!(
        moved[3],
        (Some(Verb::Transfer), ark, board(A, Color::Purple), HAND_A)
    );
    assert!(game.position().cards(board(B, Color::Red)).is_empty());
}

#[test]
fn shipwrecked_sailor_melds_the_chosen_color_then_splays_left_and_junks_its_value() {
    let mut f = Fixture::default();
    let tale = f.card("A015", board(A, Color::Purple));
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let red = f.plain(Color::Red, 3, HAND_A, &[]);
    let drawn = f.plain(Color::Blue, 1, base_deck(1), &[]);
    let ach3 = f.plain(Color::Green, 3, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, tale);
    let (status, events) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, drawn, base_deck(1), HAND_A),
            (Some(Verb::Meld), red, HAND_A, board(A, Color::Red)),
            (
                Some(Verb::Junk),
                ach3,
                Zone::AvailableAchievements,
                Zone::Junk
            ),
        ]
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
}
