//! Age 9 Artifacts (A086-A095): expected outcomes on explicit positions.
use inno_engine::*;

mod common;
use common::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

fn with_effect(mut card: Card, color: Color, value: u8, instructions: Vec<Instruction>) -> Card {
    card.color = color;
    card.value = value;
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    card
}

#[test]
fn crusader_rabbit_takes_the_two_bottoms_of_each_demand_color_and_scores_a_demand_top() {
    let mut f = Fixture::default();
    let rabbit = f.card("A086", board(A, Color::Red));
    f.cards[usize::from(rabbit.0)].effects.truncate(1);
    let b1 = f.plain(Color::Blue, 1, board(B, Color::Blue), &[]);
    let b2 = f.plain(Color::Blue, 2, board(B, Color::Blue), &[]);
    let b3 = f.plain(Color::Blue, 3, board(B, Color::Blue), &[]);
    let mut oars = Card::from_code("B003").unwrap();
    oars.color = Color::Blue;
    oars.symbols = [Symbol::Icon(Card::from_code("A086").unwrap().featured.unwrap()); SLOTS];
    let demand_top = f.add(oars, board(B, Color::Blue));
    f.plain(Color::Green, 1, board(B, Color::Green), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, rabbit);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 2, "{moved:?}");
    assert!(
        moved
            .iter()
            .all(|m| m.0 == Some(Verb::Transfer) && m.3 == SCORE_A)
    );
    assert!(moved.iter().any(|m| m.1 == b1) && moved.iter().any(|m| m.1 == b2));
    assert_eq!(
        game.position().cards(board(B, Color::Blue)),
        &[b3, demand_top]
    );

    let mut f = Fixture::default();
    let rabbit = f.card("A086", board(A, Color::Red));
    f.cards[usize::from(rabbit.0)].effects.remove(0);
    let own_demand = f.add(Card::from_code("B003").unwrap(), board(A, Color::Red));
    let _ = (rabbit, HAND_B, own_demand);
    let mut f = Fixture::default();
    f.add(Card::from_code("B003").unwrap(), board(A, Color::Red));
    let rabbit = f.card("A086", board(A, Color::Red));
    f.cards[usize::from(rabbit.0)].effects.remove(0);
    let own_demand = f.add(Card::from_code("B003").unwrap(), board(A, Color::Blue));
    f.cards[usize::from(own_demand.0)].color = Color::Blue;
    let ten = f.plain(Color::Blue, 10, base_deck(10), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, rabbit);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (
                Some(Verb::Score),
                own_demand,
                board(A, Color::Blue),
                SCORE_A
            ),
            (None, ten, base_deck(10), HAND_A),
        ]
    );
}

#[test]
fn velcro_shoes_takes_a_nine_from_hand_then_score_and_wins_when_neither_exists() {
    for (hand_nine, score_nine, expect) in [
        (true, true, "hand"),
        (false, true, "score"),
        (false, false, "win"),
    ] {
        let mut f = Fixture::default();
        let velcro = f.card("A087", board(A, Color::Red));
        f.cards[usize::from(velcro.0)].effects.truncate(1);
        f.plain(
            Color::Blue,
            1,
            board(B, Color::Blue),
            &sharing_icons("A087"),
        );
        let h9 = f.plain(Color::Green, if hand_nine { 9 } else { 8 }, HAND_B, &[]);
        let s9 = f.plain(Color::Yellow, if score_nine { 9 } else { 8 }, SCORE_B, &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, velcro);
        let (status, events) = play(&mut game, |_| Some(0));
        let moved = moves(&events);
        match expect {
            "hand" => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                assert_eq!(moved, [(Some(Verb::Transfer), h9, HAND_B, HAND_A)]);
            }
            "score" => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                assert_eq!(moved, [(Some(Verb::Transfer), s9, SCORE_B, SCORE_A)]);
            }
            _ => {
                assert_eq!(status, Status::Winner(A));
                assert!(moved.is_empty());
            }
        }
    }
}

#[test]
fn rock_around_the_clock_scores_a_nine_per_efficiency_top_and_junks_by_score_size() {
    let mut f = Fixture::default();
    let rock = f.card("A088", board(A, Color::Yellow));
    f.plain(
        Color::Red,
        1,
        board(A, Color::Red),
        &[Icon::Efficiency, Icon::Efficiency],
    );
    f.plain(Color::Blue, 1, board(A, Color::Blue), &[Icon::Efficiency]);
    f.plain(Color::Green, 1, board(A, Color::Green), &[Icon::Authority]);
    let nines: Vec<_> = (0..3)
        .map(|_| f.plain(Color::Purple, 9, base_deck(9), &[]))
        .collect();
    // Two Efficiency tops (Rock itself counts if it prints one): that many
    // scores, then the deck of that value is junked.
    let efficiency_tops = 2 + usize::from(
        Card::from_code("A088")
            .unwrap()
            .printed_icon_count(Icon::Efficiency)
            > 0,
    );
    let junked = f.plain(
        Color::Purple,
        efficiency_tops as u8,
        base_deck(efficiency_tops as u8),
        &[],
    );
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, rock);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    let scores = moved.iter().filter(|m| m.0 == Some(Verb::Score)).count();
    assert_eq!(scores, efficiency_tops);
    assert_eq!(
        moved.last().unwrap(),
        &(
            Some(Verb::Junk),
            junked,
            base_deck(efficiency_tops as u8),
            Zone::Junk
        )
    );
    let _ = nines;
}

#[test]
fn magnavox_odyssey_wins_when_both_melded_tens_share_a_color() {
    for (second_color, wins) in [(Color::Red, true), (Color::Blue, false)] {
        let mut f = Fixture::default();
        let odyssey = f.card("A089", board(A, Color::Yellow));
        let first = f.plain(second_color, 10, base_deck(10), &[]);
        let second = f.plain(Color::Red, 10, base_deck(10), &[]);
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, odyssey);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(
            moves(&events)
                .iter()
                .filter(|m| m.0 == Some(Verb::Meld))
                .count(),
            2
        );
        assert_eq!(
            status,
            if wins {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
        let _ = (first, second);
    }
}

#[test]
fn philips_compact_cassette_unsplays_the_victim_and_splays_two_colors_up() {
    // Any two colors on the board, a lone card included.
    for picked in [[Color::Red, Color::Blue], [Color::Red, Color::Green]] {
        let mut f = Fixture::default();
        let cassette = f.card("A090", board(A, Color::Green));
        f.plain(Color::Red, 1, board(A, Color::Red), &[]);
        f.plain(Color::Red, 2, board(A, Color::Red), &[]);
        f.plain(Color::Blue, 1, board(A, Color::Blue), &[]);
        f.plain(Color::Blue, 2, board(A, Color::Blue), &[]);
        f.plain(
            Color::Yellow,
            1,
            board(B, Color::Yellow),
            &sharing_icons("A090"),
        );
        f.plain(
            Color::Yellow,
            2,
            board(B, Color::Yellow),
            &sharing_icons("A090"),
        );
        f.plain(Color::Purple, 9, base_deck(9), &[]); // A's sharing reward
        let mut game = f.game(Expansions::ARTIFACTS);
        let position = game
            .position()
            .clone()
            .with_splay(B, Color::Yellow, Splay::Right)
            .unwrap();
        game = Game::new(position, Expansions::ARTIFACTS).unwrap();
        dogma(&mut game, cassette);
        let mask = picked.iter().fold(0, |m, &c| m | 1 << c as u8);
        let (status, _) = play(&mut game, |decision| {
            assert_eq!(decision.player, A, "B's one color needs no choice");
            // Two of A's green, red and blue.
            assert_eq!(decision.options.len(), 3);
            decision
                .options
                .iter()
                .position(|o| *o == Alternative::Colors(mask))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        // The compel unsplayed B's yellow; sharing the second effect splayed
        // it up again.
        assert_eq!(game.position().splay(B, Color::Yellow), Splay::Up);
        assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
        let blue = if picked.contains(&Color::Blue) {
            Splay::Up
        } else {
            Splay::None
        };
        assert_eq!(game.position().splay(A, Color::Blue), blue);
        assert_eq!(game.position().splay(A, Color::Green), Splay::None);
    }
}

#[test]
fn syncom_3_returns_the_hand_and_wins_on_five_revealed_colors() {
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
        let mut f = Fixture::default();
        let syncom = f.card("A091", board(A, Color::Green));
        let held = f.plain(Color::Red, 1, HAND_A, &[]);
        for color in colors {
            f.plain(color, 9, base_deck(9), &[]);
        }
        let mut game = f.game(Expansions::ARTIFACTS);
        dogma(&mut game, syncom);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(
            moves(&events)[0],
            (Some(Verb::Return), held, HAND_A, base_deck(1))
        );
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
fn yeagers_bell_x1a_melds_executes_and_repeats_on_efficiency() {
    let mut f = Fixture::default();
    let yeager = f.card("A092", board(A, Color::Blue));
    let mut plain = with_effect(
        Card::from_code("B001").unwrap(),
        Color::Green,
        9,
        vec![build::draw(1)],
    );
    plain.symbols = [Symbol::Blank; SLOTS];
    let plain = f.add(plain, base_deck(9));
    let mut efficient = with_effect(
        Card::from_code("B001").unwrap(),
        Color::Red,
        9,
        vec![build::draw(1)],
    );
    efficient.symbols = [Symbol::Icon(Icon::Efficiency); SLOTS];
    let efficient = f.add(efficient, base_deck(9));
    let one_a = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let one_b = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, yeager);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, efficient, base_deck(9), HAND_A),
            (Some(Verb::Meld), efficient, HAND_A, board(A, Color::Red)),
            (None, one_b, base_deck(1), HAND_A),
            (None, plain, base_deck(9), HAND_A),
            (Some(Verb::Meld), plain, HAND_A, board(A, Color::Green)),
            (None, one_a, base_deck(1), HAND_A),
        ]
    );
}

#[test]
fn luna_3_returns_the_score_pile_and_scores_the_count_then_junks_a_chosen_deck() {
    let mut f = Fixture::default();
    let luna = f.card("A093", board(A, Color::Blue));
    f.plain(Color::Red, 3, SCORE_A, &[]);
    f.plain(Color::Green, 5, SCORE_A, &[]);
    let two = f.plain(Color::Yellow, 2, base_deck(2), &[]);
    let four = f.plain(Color::Yellow, 4, base_deck(4), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, luna);
    let (status, events) = play(&mut game, |decision| pick_value(decision, 4).or(Some(0)));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[..2]
            .iter()
            .filter(|m| m.0 == Some(Verb::Return))
            .count(),
        2
    );
    assert_eq!(moved[2], (None, two, base_deck(2), HAND_A));
    assert_eq!(moved[3], (Some(Verb::Score), two, HAND_A, SCORE_A));
    assert_eq!(moved[4], (Some(Verb::Junk), four, base_deck(4), Zone::Junk));
}

#[test]
fn the_big_bang_repeats_while_the_blue_top_changes_something() {
    // The blue top draws a card: a change, so draw and junk a [10] and
    // repeat; the second execution draws again, then the [10] deck is empty.
    let mut f = Fixture::default();
    let big_bang = f.card("A094", board(A, Color::Purple));
    let blue = f.add(
        with_effect(
            Card::from_code("B001").unwrap(),
            Color::Blue,
            3,
            vec![build::draw(1)],
        ),
        board(A, Color::Blue),
    );
    let one_a = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let one_b = f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let ten = f.plain(Color::Yellow, 10, base_deck(10), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, big_bang);
    let (status, events) = no_decisions(&mut game);
    let moved = moves(&events);
    assert_eq!(moved[0], (None, one_b, base_deck(1), HAND_A));
    assert_eq!(moved[1], (None, ten, base_deck(10), HAND_A));
    assert_eq!(moved[2], (Some(Verb::Junk), ten, HAND_A, Zone::Junk));
    assert_eq!(moved[3], (None, one_a, base_deck(1), HAND_A));
    // The second [10] draw exhausts: the game ends.
    assert!(
        matches!(status, Status::Winner(_) | Status::Draw(_)),
        "{status:?}"
    );
    let _ = blue;

    // An effectless blue top changes nothing: no draw, no repeat.
    let mut f = Fixture::default();
    let big_bang = f.card("A094", board(A, Color::Purple));
    f.plain(Color::Blue, 3, board(A, Color::Blue), &[]);
    f.plain(Color::Yellow, 10, base_deck(10), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, big_bang);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(moves(&events).is_empty());
}

#[test]
fn marilyn_diptych_may_score_and_retrieve_then_wins_at_exactly_25_points() {
    let mut f = Fixture::default();
    let marilyn = f.card("A095", board(A, Color::Purple));
    f.cards[usize::from(marilyn.0)].effects.truncate(1);
    let ten = f.plain(Color::Red, 10, HAND_A, &[]);
    f.plain(Color::Blue, 9, SCORE_A, &[]);
    f.plain(Color::Green, 6, SCORE_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, marilyn);
    // Score the 10 (25 points), decline the transfer.
    let mut step = 0;
    let (status, events) = play(&mut game, |decision| {
        step += 1;
        if step == 1 {
            option_moving(decision, ten)
        } else {
            decision
                .options
                .iter()
                .position(|o| *o == Alternative::Decline)
        }
    });
    assert_eq!(moves(&events), [(Some(Verb::Score), ten, HAND_A, SCORE_A)]);
    assert_eq!(status, Status::Winner(A));
    let _ = SCORE_B;
}
