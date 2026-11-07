//! Unseen's five special achievements and the own-secret Achieve action.

use inno_engine::*;

mod common;
use common::*;

const UNSEEN_FIGURES: Expansions = Expansions::UNSEEN.union(Expansions::FIGURES);

fn tops(f: &mut Fixture, player: Player, values: &[u8]) {
    for (color, &value) in Color::ALL.into_iter().zip(values) {
        f.tech(color, value, Zone::Board(player, color), vec![]);
    }
}

fn secrets(f: &mut Fixture, player: Player, count: usize) {
    for i in 0..count {
        f.tech(Color::ALL[i % 5], 1, Zone::Safe(player), vec![]);
    }
}

fn eligible(f: Fixture, expansions: Expansions, special: Special) -> bool {
    f.position()
        .with_expansions(expansions)
        .eligible_for_special(A, special)
}

#[test]
fn unseen_specials_are_enabled_only_with_unseen() {
    let f = Fixture::default();
    let position = f.position();
    assert_eq!(
        position
            .clone()
            .with_expansions(Expansions::UNSEEN)
            .specials()
            .collect::<Vec<_>>(),
        Special::ALL
            .into_iter()
            .chain(Special::UNSEEN)
            .collect::<Vec<_>>()
    );
    assert!(
        position
            .with_expansions(Expansions::ARTIFACTS)
            .specials()
            .all(|special| !Special::UNSEEN.contains(&special))
    );
}

#[test]
fn confidence_needs_a_top_five_and_four_secrets() {
    for (top, count, expected) in [(5, 4, true), (4, 4, false), (5, 3, false), (9, 6, true)] {
        let mut f = Fixture::default();
        tops(&mut f, A, &[top, 1]);
        secrets(&mut f, A, count);
        assert_eq!(
            eligible(f, Expansions::UNSEEN, Special::Confidence),
            expected,
            "top {top}, {count} secrets"
        );
    }
}

#[test]
fn zen_needs_a_top_six_and_no_odd_top() {
    for (values, expected) in [
        (vec![6, 2, 4], true),
        (vec![6, 3], false),
        (vec![4, 2], false),
        (vec![8], true),
    ] {
        let mut f = Fixture::default();
        tops(&mut f, A, &values);
        assert_eq!(
            eligible(f, Expansions::UNSEEN, Special::Zen),
            expected,
            "{values:?}"
        );
    }
}

#[test]
fn anonymity_needs_a_top_seven_and_no_standard_achievements() {
    for (top, achievements, expected) in [(7, 0, true), (6, 0, false), (7, 1, false)] {
        let mut f = Fixture::default();
        tops(&mut f, A, &[top]);
        for _ in 0..achievements {
            f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
        }
        // The opponent's achievements and owned specials do not matter.
        f.tech(Color::Red, 2, Zone::Achievements(B), vec![]);
        assert_eq!(
            eligible(f, Expansions::UNSEEN, Special::Anonymity),
            expected,
            "top {top}, {achievements} achievements"
        );
    }
}

#[test]
fn folklore_needs_a_top_eight_and_no_visible_industry() {
    for (top, industry, expected) in [(8, false, true), (7, false, false), (8, true, false)] {
        let mut f = Fixture::default();
        let card = f.tech(Color::Red, top, Zone::Board(A, Color::Red), vec![]);
        f.cards[usize::from(card.0)].symbols[1] = Symbol::Icon(if industry {
            Icon::Industry
        } else {
            Icon::Health
        });
        // A covered Industry is not on the board.
        let buried = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        f.cards[usize::from(buried.0)].symbols[1] = Symbol::Icon(Icon::Industry);
        f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        assert_eq!(
            eligible(f, Expansions::UNSEEN, Special::Folklore),
            expected,
            "top {top}, industry {industry}"
        );
    }
}

#[test]
fn mystery_needs_a_top_nine_and_fewer_than_five_colors() {
    for (values, expected) in [
        (vec![9, 1, 1, 1], true),
        (vec![9, 1, 1, 1, 1], false),
        (vec![8, 1], false),
    ] {
        let mut f = Fixture::default();
        tops(&mut f, A, &values);
        assert_eq!(
            eligible(f, Expansions::UNSEEN, Special::Mystery),
            expected,
            "{values:?}"
        );
    }
}

/// Daedalus adds each nonempty unsplayed color to the claim value, and the
/// specials' "top card of value N or higher" is a claim requirement.
#[test]
fn daedalus_raises_the_top_value_for_unseen_specials() {
    for (daedalus, expected) in [(false, false), (true, true)] {
        let mut f = Fixture::default();
        // Daedalus (1) and two other unsplayed colors: top 3 + 3 = 6.
        if daedalus {
            f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
        } else {
            f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        }
        f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Purple, 2, Zone::Board(A, Color::Purple), vec![]);
        secrets(&mut f, A, 4);
        let position = f.position().with_expansions(UNSEEN_FIGURES);
        assert_eq!(
            position.highest_claim_value(A),
            if daedalus { 6 } else { 3 }
        );
        assert_eq!(
            position.eligible_for_special(A, Special::Confidence),
            expected
        );
        // Zen keeps its printed odd-value test on the real tops: the
        // allowance reaches 6 but the top red 3 is odd.
        assert!(!position.eligible_for_special(A, Special::Zen));
    }
}

/// Hedy lowers each number by one: the top value, Confidence's secrets and Mystery's
/// color bound, which makes Mystery harder (publisher). The "no..." conditions have no
/// number.
#[test]
fn hedy_lowers_the_numbers_of_unseen_specials() {
    for hedy in [false, true] {
        let mut f = Fixture::default();
        if hedy {
            f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
        } else {
            f.tech(Color::Green, 9, Zone::Board(A, Color::Green), vec![]);
        }
        // A 9 on top with three other colors: four colors in all.
        f.tech(Color::Red, 8, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
        secrets(&mut f, A, 3);
        f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
        let position = f.position().with_expansions(UNSEEN_FIGURES);
        // Four colors are fewer than five, but not fewer than four.
        assert_eq!(position.eligible_for_special(A, Special::Mystery), !hedy);
        // Three secrets suffice only under Hedy.
        assert_eq!(position.eligible_for_special(A, Special::Confidence), hedy);
        // One standard achievement still blocks Anonymity.
        assert!(!position.eligible_for_special(A, Special::Anonymity));
    }
}

/// Confidence is claimed by the end-of-action check and counts toward the
/// raised victory target.
#[test]
fn unseen_specials_are_claimed_at_the_end_of_an_action() {
    let mut f = Fixture::default();
    tops(&mut f, A, &[5]);
    secrets(&mut f, A, 4);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    game.finish_action().unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        game.position().special_location(Special::Confidence),
        SpecialLocation::Owned(A)
    );
    assert_eq!(game.position().achievement_count(A), 1);
}

/// A secret is claimed as an action under the normal score, top-value and
/// duplicate-value costs; the request names its public slot.
#[test]
fn own_secrets_are_achievable_by_their_public_slot() {
    let mut f = Fixture::default();
    tops(&mut f, A, &[3]);
    let three = f.tech(Color::Red, 3, Zone::Safe(A), vec![]);
    let four = f.tech(Color::Blue, 4, Zone::Safe(A), vec![]);
    let theirs = f.tech(Color::Green, 1, Zone::Safe(B), vec![]);
    let scored = f.tech(Color::Green, 8, Zone::Score(A), vec![]);
    let also = f.tech(Color::Green, 7, Zone::Score(A), vec![]);
    let supply = f.tech(Color::Yellow, 1, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    let _ = (scored, also, theirs);
    // Score 15, top 3: the 3 is affordable, the 4 exceeds the top value.
    let claims: Vec<_> = game
        .legal_actions()
        .into_iter()
        .filter(|action| matches!(action, Action::Achieve { .. }))
        .collect();
    assert_eq!(
        claims,
        vec![
            Action::Achieve { card: supply },
            Action::Achieve { card: three }
        ]
    );
    assert_eq!(
        game.take_action(Action::Achieve { card: four }, ActionKind::Regular),
        Err(Error::InvalidAction)
    );
    for viewer in Player::ALL {
        game.observe(viewer).unwrap();
    }
    game.take_action(Action::Achieve { card: three }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    let moved = events
        .iter()
        .find_map(|event| match event {
            Event::Moved(m) => Some(m),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (moved.verb, moved.card, moved.source, moved.destination),
        (
            Some(Verb::Achieve),
            three,
            Zone::Safe(A),
            Zone::Achievements(A)
        )
    );
    assert_eq!(game.position().cards(Zone::Safe(A)), &[four]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[three]);
    // A second 3 would cost 30; the remaining secret is still a 4.
    assert!(
        !game
            .legal_actions()
            .iter()
            .any(|action| matches!(action, Action::Achieve { card } if *card == four))
    );
    assert!(game.position().check_conservation());
}
