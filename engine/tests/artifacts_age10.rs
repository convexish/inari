//! Age 10 Artifacts (A096-A105): expected outcomes on explicit positions.
use inno_engine::*;

mod common;
use common::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

fn artifact_deck(age: u8) -> Zone {
    Zone::Deck(Deck {
        set: CardSet::Artifacts,
        age,
    })
}

fn any(game: &mut Game) -> (Status, Vec<Event>) {
    play(game, |_| Some(0))
}

/// The fixture's game with these piles already splayed.
fn splayed(f: Fixture, splays: &[(Player, Color, Splay)]) -> Game {
    let mut position = f.position();
    for &(player, color, direction) in splays {
        position = position.with_splay(player, color, direction).unwrap();
    }
    Game::new(position, Expansions::ARTIFACTS).unwrap()
}

#[test]
fn tanker_exxon_valdez_eliminates_the_victim() {
    let mut f = Fixture::default();
    let tanker = f.card("A096", board(A, Color::Red));
    f.plain(
        Color::Blue,
        3,
        board(B, Color::Blue),
        &sharing_icons("A096"),
    );
    f.plain(Color::Green, 2, HAND_B, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, tanker);
    let (status, _) = any(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(
        game.victory().map(|v| v.reason),
        Some(WinReason::Elimination { loser: B })
    );
}

#[test]
fn maldives_keeps_two_of_each_for_the_victim_and_four_scored_for_the_owner() {
    let mut f = Fixture::default();
    let maldives = f.card("A097", board(A, Color::Red));
    f.plain(
        Color::Blue,
        3,
        board(B, Color::Blue),
        &sharing_icons("A097"),
    );
    for value in 1..=4 {
        f.plain(Color::Green, value, HAND_B, &[]);
    }
    for value in 1..=3 {
        f.plain(Color::Yellow, value, SCORE_B, &[]);
    }
    for value in 1..=6 {
        f.plain(Color::Purple, value, SCORE_A, &[]);
    }
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, maldives);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let returned = moves(&events);
    assert!(returned.iter().all(|m| m.0 == Some(Verb::Return)));
    assert_eq!(returned.iter().filter(|m| m.2 == HAND_B).count(), 2);
    assert_eq!(returned.iter().filter(|m| m.2 == SCORE_B).count(), 1);
    assert_eq!(returned.iter().filter(|m| m.2 == SCORE_A).count(), 2);
    assert_eq!(game.position().cards(HAND_B).len(), 2);
    assert_eq!(game.position().cards(SCORE_B).len(), 2);
    assert_eq!(game.position().cards(SCORE_A).len(), 4);
}

#[test]
fn dolly_the_sheep_wins_on_domestication_or_melds_the_highest_then_draws_an_eleven() {
    // Domestication at the bottom of yellow: declining both options still wins.
    let mut f = Fixture::default();
    let dolly = f.card("A098", board(A, Color::Yellow));
    f.cards[usize::from(dolly.0)].effects.truncate(1);
    let domestication = f.card("B005", board(A, Color::Yellow));
    f.placements.swap(0, 1);
    f.plain(Color::Blue, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    assert_eq!(
        game.position().cards(board(A, Color::Yellow))[0],
        domestication
    );
    dogma(&mut game, dolly);
    let (status, _) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Decline)
    });
    assert_eq!(status, Status::Winner(A));

    // Otherwise the highest hand card melds and an 11 is drawn.
    let mut f = Fixture::default();
    let dolly = f.card("A098", board(A, Color::Yellow));
    f.cards[usize::from(dolly.0)].effects.truncate(1);
    let low = f.plain(Color::Blue, 3, HAND_A, &[]);
    let high = f.plain(Color::Green, 5, HAND_A, &[]);
    let eleven = f.plain(Color::Red, 11, base_deck(11), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, dolly);
    let (status, events) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Decline)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (Some(Verb::Meld), high, HAND_A, board(A, Color::Green)),
            (None, eleven, base_deck(11), HAND_A),
        ]
    );
    assert_eq!(game.position().cards(HAND_A), &[low, eleven]);

    // The second effect junks every available achievement.
    let mut f = Fixture::default();
    let dolly = f.card("A098", board(A, Color::Yellow));
    f.cards[usize::from(dolly.0)].effects.remove(0);
    let one = f.plain(Color::Blue, 1, Zone::AvailableAchievements, &[]);
    let two = f.plain(Color::Blue, 2, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, dolly);
    let (status, _) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(
        game.position()
            .cards(Zone::AvailableAchievements)
            .is_empty()
    );
    assert!(game.position().cards(Zone::Junk).contains(&one));
    assert!(game.position().cards(Zone::Junk).contains(&two));
}

#[test]
fn wheres_waldo_wins() {
    let mut f = Fixture::default();
    let waldo = f.card("A099", board(A, Color::Yellow));
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, waldo);
    let (status, _) = any(&mut game);
    assert_eq!(status, Status::Winner(A));
}

#[test]
fn musks_tesla_roadster_wins_on_most_scored_cards_or_scores_the_drawn_color() {
    // Tied most qualifies.
    let mut f = Fixture::default();
    let roadster = f.card("A100", board(A, Color::Green));
    f.plain(Color::Blue, 1, SCORE_A, &[]);
    f.plain(Color::Blue, 1, SCORE_B, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, roadster);
    assert_eq!(any(&mut game).0, Status::Winner(A));

    // Fewer scored cards: the drawn color's board cards score, the draw returns.
    let mut f = Fixture::default();
    let roadster = f.card("A100", board(A, Color::Green));
    f.plain(Color::Blue, 1, SCORE_B, &[]);
    let red_low = f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let red_top = f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let drawn = f.plain(Color::Red, 11, base_deck(11), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, roadster);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved[0], (None, drawn, base_deck(11), HAND_A));
    assert!(moved[1..3].iter().all(|m| m.0 == Some(Verb::Score)));
    assert!(
        moved[1..3].iter().any(|m| m.1 == red_low) && moved[1..3].iter().any(|m| m.1 == red_top)
    );
    assert_eq!(moved[3], (Some(Verb::Return), drawn, HAND_A, base_deck(11)));

    // Nothing of that color on the board: the draw melds instead.
    let mut f = Fixture::default();
    let roadster = f.card("A100", board(A, Color::Green));
    f.plain(Color::Blue, 1, SCORE_B, &[]);
    let drawn = f.plain(Color::Purple, 11, base_deck(11), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, roadster);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events)[1],
        (Some(Verb::Meld), drawn, HAND_A, board(A, Color::Purple))
    );
}

#[test]
fn seikan_tunnel_wins_with_the_most_visible_cards_in_a_color_including_ties() {
    // A's splayed-up red shows two; B's unsplayed stack shows one.
    let mut f = Fixture::default();
    let tunnel = f.card("A101", board(A, Color::Green));
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    f.plain(Color::Blue, 1, board(B, Color::Blue), &[]);
    f.plain(Color::Blue, 2, board(B, Color::Blue), &[]);
    f.plain(Color::Blue, 3, board(B, Color::Blue), &[]);
    let mut game = splayed(f, &[(A, Color::Red, Splay::Up)]);
    dogma(&mut game, tunnel);
    assert_eq!(any(&mut game).0, Status::Winner(A));

    // B's splayed-up stack shows three: no win.
    let mut f = Fixture::default();
    let tunnel = f.card("A101", board(A, Color::Green));
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    f.plain(Color::Blue, 1, board(B, Color::Blue), &[]);
    f.plain(Color::Blue, 2, board(B, Color::Blue), &[]);
    f.plain(Color::Blue, 3, board(B, Color::Blue), &[]);
    let mut game = splayed(
        f,
        &[(A, Color::Red, Splay::Up), (B, Color::Blue, Splay::Up)],
    );
    dogma(&mut game, tunnel);
    assert_eq!(any(&mut game).0, Status::Complete(ReturnValue::Unit));
}

#[test]
fn rover_curiosity_draws_melds_and_self_executes_an_artifact_ten() {
    let mut f = Fixture::default();
    let rover = f.card("A102", board(A, Color::Blue));
    let waldo = f.card("A099", artifact_deck(10));
    // The age comes from the Base decks, as for any draw.
    let base_ten = f.plain(Color::Red, 10, base_deck(10), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, rover);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(
        moves(&events),
        [
            (None, waldo, artifact_deck(10), HAND_A),
            (Some(Verb::Meld), waldo, HAND_A, board(A, Color::Yellow)),
        ]
    );
    assert_eq!(game.position().cards(base_deck(10)), &[base_ten]);
}

#[test]
fn higgs_boson_scores_the_whole_board() {
    let mut f = Fixture::default();
    let higgs = f.card("A103", board(A, Color::Blue));
    let red_low = f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let red_top = f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let green = f.plain(Color::Green, 3, board(A, Color::Green), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, higgs);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let scored: Vec<_> = moves(&events)
        .into_iter()
        .filter(|m| m.0 == Some(Verb::Score))
        .map(|m| m.1)
        .collect();
    assert_eq!(scored.len(), 4);
    for card in [higgs, red_low, red_top, green] {
        assert!(scored.contains(&card));
    }
    assert!(game.position().cards(SCORE_A).len() == 4);
}

#[test]
fn delorean_junks_one_top_per_pile_and_every_hand_only_while_it_is_a_top() {
    let mut f = Fixture::default();
    let delorean = f.card("A104", board(A, Color::Purple));
    let a_red_low = f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let a_red_top = f.plain(Color::Red, 2, board(A, Color::Red), &[]);
    let b_blue = f.plain(Color::Blue, 3, board(B, Color::Blue), &[]);
    let a_hand = f.plain(Color::Green, 4, HAND_A, &[]);
    let b_hand = f.plain(Color::Yellow, 5, HAND_B, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, delorean);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let junked: Vec<_> = moves(&events)
        .into_iter()
        .filter(|m| m.0 == Some(Verb::Junk))
        .map(|m| m.1)
        .collect();
    assert_eq!(junked.len(), 5, "{junked:?}");
    for card in [delorean, a_red_top, b_blue, a_hand, b_hand] {
        assert!(junked.contains(&card));
    }
    assert_eq!(game.position().top(A, Color::Red), Some(a_red_low));

    // Covered: nothing happens.
    let mut f = Fixture::default();
    let delorean = f.card("A104", board(A, Color::Purple));
    f.plain(Color::Purple, 9, board(A, Color::Purple), &[]);
    f.plain(Color::Green, 4, HAND_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    game.self_execute(delorean, Context::new(A)).unwrap();
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(moves(&events).is_empty());
}

#[test]
fn dragons_lair_loses_on_a_single_meld_and_junks_deck_tops_otherwise() {
    // A unique-valued 5 melds, a green hand card follows: two melds, so
    // the top of every deck is junked.
    let mut f = Fixture::default();
    let lair = f.card("A105", board(A, Color::Purple));
    f.plain(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A105"),
    );
    f.plain(Color::Red, 3, SCORE_B, &[]);
    f.plain(Color::Yellow, 3, SCORE_B, &[]);
    let five = f.plain(Color::Green, 5, SCORE_B, &[]);
    let green = f.plain(Color::Green, 2, HAND_B, &[]);
    let deck_one = f.plain(Color::Blue, 1, base_deck(1), &[]);
    let deck_one_under = f.plain(Color::Blue, 1, base_deck(1), &[]);
    f.placements.swap(6, 7);
    let deck_seven = f.plain(Color::Blue, 7, base_deck(7), &[]);
    let artifact_two = f.card("A016", artifact_deck(2));
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, lair);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(
        moved[0],
        (Some(Verb::Meld), five, SCORE_B, board(B, Color::Green))
    );
    assert_eq!(
        moved[1],
        (Some(Verb::Meld), green, HAND_B, board(B, Color::Green))
    );
    let junked: Vec<_> = moved[2..].iter().map(|m| m.1).collect();
    assert_eq!(junked.len(), 3, "{junked:?}");
    for card in [deck_one, deck_seven, artifact_two] {
        assert!(junked.contains(&card));
    }
    assert_eq!(game.position().cards(base_deck(1)), &[deck_one_under]);

    // No matching hand card: one meld total, the victim loses.
    let mut f = Fixture::default();
    let lair = f.card("A105", board(A, Color::Purple));
    f.plain(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A105"),
    );
    f.plain(Color::Green, 5, SCORE_B, &[]);
    f.plain(Color::Red, 2, HAND_B, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, lair);
    let (status, _) = any(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(
        game.victory().map(|v| v.reason),
        Some(WinReason::Elimination { loser: B })
    );
}

#[test]
fn dragons_lair_deck_top_snapshot_samples_by_deck_position() {
    // Rod Serling watches junking from anywhere, so the deck tops stay an
    // ordered menu (Nightingale guards only score piles, which a deck-top
    // sweep never selects). Every card here is a [4] or more, so he never
    // fires. The frozen originals are hidden deck tops: membership is their
    // place, so each observer's world junks its own sampled tops.
    let mut f = Fixture::default();
    let lair = f.card("A105", board(A, Color::Purple));
    f.plain(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A105"),
    );
    f.top_figure(Figure::RodSerling, B);
    f.plain(Color::Red, 3, SCORE_B, &[]);
    f.plain(Color::Yellow, 3, SCORE_B, &[]);
    f.plain(Color::Green, 5, SCORE_B, &[]);
    f.plain(Color::Green, 4, HAND_B, &[]);
    for color in [Color::Red, Color::Green, Color::Purple] {
        f.plain(color, 4, base_deck(4), &[]);
        f.plain(color, 5, base_deck(5), &[]);
        f.plain(color, 7, base_deck(7), &[]);
    }
    let mut game = f
        .game(Expansions::ARTIFACTS.union(Expansions::FIGURES))
        .with_tracking()
        .unwrap();
    dogma(&mut game, lair);
    let menu = loop {
        match game.advance(1000, &mut Vec::new()).unwrap() {
            Status::Decision(menu)
                if matches!(
                    menu.options[0],
                    Alternative::BlindMove {
                        verb: Verb::Junk,
                        ..
                    }
                ) =>
            {
                break menu;
            }
            Status::Decision(_) => game.choose(0).unwrap(),
            status => panic!("no deck-top menu: {status:?}"),
        }
    };
    let decks: Vec<_> = menu
        .options
        .iter()
        .map(|option| match option {
            Alternative::BlindMove { source, .. }
            | Alternative::Move {
                target: TargetDescriptor::Known { source, .. },
                ..
            } => *source,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(decks.len(), 3);
    let top = |game: &Game| -> Vec<CardId> {
        let mut tops: Vec<_> = decks
            .iter()
            .map(|&deck| *game.position().cards(deck).last().unwrap())
            .collect();
        tops.sort();
        tops
    };
    let viewer = menu.player;
    let mut resampled = false;
    for seed in 0..16 {
        let mut world = game.determinize(viewer, seed).unwrap();
        assert_eq!(
            world.observe(viewer).unwrap(),
            game.observe(viewer).unwrap()
        );
        assert!(world.position().check_conservation());
        assert!(world.tracker(viewer).unwrap().check_truth(world.position()));
        let tops = top(&world);
        resampled |= tops != top(&game);
        let (status, events) = play(&mut world, |_| Some(0));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let mut junked: Vec<_> = moves(&events).iter().map(|m| m.1).collect();
        junked.sort();
        assert_eq!(junked, tops, "seed {seed}");
    }
    assert!(resampled);
}
