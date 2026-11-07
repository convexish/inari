//! Unseen Age 3: one expected-outcome scenario per card.

use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
}

fn icons(f: &mut Fixture, card: CardId, icons: &[Icon]) {
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
    }
}

fn effect(game: &mut Game, card: CardId, index: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
            | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
            if *c == card
        )
    })
}

fn run(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = preferred
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("did not terminate");
}

fn complete(status: Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

#[test]
fn knights_templar_unsplays_a_color_and_takes_its_top() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U026", Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let top = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
    f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    let position = f
        .position()
        .with_splay(B, Color::Blue, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(B, Color::Blue), Splay::None);
    assert_eq!(position.cards(Zone::Score(A)), &[top]);
}

#[test]
fn red_envelope_moves_a_value_held_two_or_three_times_and_scores_two_or_three() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U027", Zone::Board(A, Color::Red));
    let two_a = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let two_b = f.tech(Color::Green, 2, Zone::Score(A), vec![]);
    let one = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    for color in Color::ALL.iter().take(4) {
        f.tech(*color, 5, Zone::Score(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    // Only 2 qualifies (1 once, 5 four times), so no value is asked.
    complete(run(&mut game, &mut events, &[]));
    let mut theirs = game.position().cards(Zone::Score(B)).to_vec();
    theirs.sort();
    assert_eq!(theirs, vec![two_a, two_b]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[one]);

    // Second effect: with one hand card nothing is offered; with three the
    // branch and the scores follow.
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[one]);
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U027", Zone::Board(A, Color::Red));
    for color in Color::ALL.iter().take(3) {
        f.tech(*color, 1, Zone::Hand(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 3);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)).len(), 3);

    // With exactly two hand cards, three is not offered: score two or decline.
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U027", Zone::Board(A, Color::Red));
    for color in Color::ALL.iter().take(2) {
        f.tech(*color, 1, Zone::Hand(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let branch = decision(&mut game, &mut events);
    assert_eq!(
        branch.options,
        [Alternative::Branch(0), Alternative::Decline]
    );
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)).len(), 2);
}

#[test]
fn freemasons_tucks_per_color_and_draws_for_yellow_or_expansion_tucks() {
    for yellow in [false, true] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U028", Zone::Board(A, Color::Yellow));
        let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let other = f.tech(
            if yellow { Color::Yellow } else { Color::Blue },
            1,
            Zone::Hand(A),
            vec![],
        );
        let threes = [
            f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]),
            f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]),
        ];
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[red, other]));
        let position = game.position();
        assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red]);
        let mut hand = position.cards(Zone::Hand(A)).to_vec();
        hand.sort();
        if yellow {
            assert_eq!(hand, threes.to_vec());
        } else {
            assert!(hand.is_empty());
        }
    }
}

/// At most one card per color, in any order, stopping at any time.
#[test]
fn freemasons_tucks_at_most_one_card_of_each_color_and_may_stop() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U028", Zone::Board(A, Color::Yellow));
    let reds = [
        f.tech(Color::Red, 1, Zone::Hand(A), vec![]),
        f.tech(Color::Red, 2, Zone::Hand(A), vec![]),
    ];
    let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let first = decision(&mut game, &mut events);
    assert!(first.options.contains(&Alternative::Decline));
    game.choose(pick_known(&first, reds[0]).unwrap()).unwrap();
    let second = decision(&mut game, &mut events);
    // The other red is no longer offered; stopping is.
    assert!(pick_known(&second, reds[1]).is_none());
    assert!(pick_known(&second, blue).is_some());
    let stop = second
        .options
        .iter()
        .position(|o| *o == Alternative::Decline)
        .unwrap();
    game.choose(stop).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
    hand.sort();
    assert_eq!(hand, vec![reds[1], blue]);
}

#[test]
fn secret_police_tucks_and_returns_until_it_cannot_then_draws() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U029", Zone::Board(A, Color::Yellow));
    let red_top = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let red = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let blue = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let three = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    // Tuck red (returns the red top), repeat: tuck blue (alone, so it is
    // its own top and returns), repeat: no hand card, draw a [3].
    complete(run(&mut game, &mut events, &[red, blue]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), &[red]);
    assert!(position.cards(Zone::Deck(Deck::base(2))).contains(&red_top));
    assert!(position.cards(Zone::Board(B, Color::Blue)).is_empty());
    assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&blue));
    assert_eq!(position.cards(Zone::Hand(B)), &[three]);
}

#[test]
fn cliffhanger_dispatches_on_the_revealed_secret_or_safeguards_the_deck_top() {
    for color in [Color::Green, Color::Red, Color::Blue, Color::Purple] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U030", Zone::Board(A, Color::Green));
        let secret = f.tech(color, 4, Zone::Safe(A), vec![]);
        let other = f.tech(Color::Yellow, 3, Zone::Safe(A), vec![]);
        let five = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Revealed { card, .. } if *card == secret))
        );
        match color {
            // Tucked beneath Cliffhanger itself; melded on top.
            Color::Green => assert_eq!(position.cards(Zone::Board(A, color))[0], secret),
            Color::Purple => assert_eq!(position.top(A, color), Some(secret)),
            Color::Red => assert_eq!(position.cards(Zone::Achievements(A)), &[secret]),
            _ => {
                assert_eq!(position.cards(Zone::Safe(A)), &[secret, other]);
                assert_eq!(position.cards(Zone::Hand(A)), &[five]);
            }
        }
    }
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U030", Zone::Board(A, Color::Green));
    f.tech(Color::Yellow, 3, Zone::Safe(A), vec![]);
    let four = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Safe(A)).len(), 2);
    assert!(game.position().cards(Zone::Safe(A)).contains(&four));
    // A direct deck top is not a draw.
    assert!(!game.position().first_drawn(A));
}

#[test]
fn smuggling_takes_two_score_cards_by_both_top_yellows() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U031", Zone::Board(A, Color::Green));
    f.tech(Color::Yellow, 3, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Yellow, 2, Zone::Board(B, Color::Yellow), vec![]);
    let two = f.tech(Color::Red, 2, Zone::Score(B), vec![]);
    let three = f.tech(Color::Blue, 3, Zone::Score(B), vec![]);
    let four = f.tech(Color::Green, 4, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let mut mine = game.position().cards(Zone::Score(A)).to_vec();
    mine.sort();
    assert_eq!(mine, vec![two, three]);
    assert_eq!(game.position().cards(Zone::Score(B)), &[four]);
}

#[test]
fn brethren_melds_one_higher_than_its_last_meld_and_repeats_over_concept() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U032", Zone::Board(A, Color::Blue));
    let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    icons(&mut f, red, &[Icon::Concept]);
    let three = f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]);
    let four = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let five = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    // The 3 covers the Concept card, so a 4 follows; it covers nothing. With
    // the [3] deck empty, a [3] and one higher both draw the 4: no choice.
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red, three]);
    assert_eq!(position.cards(Zone::Board(A, Color::Green)), &[four]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(5))), &[five]);
}

#[test]
fn brethrens_first_meld_chooses_a_three_or_a_one() {
    for one in [false, true] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U032", Zone::Board(A, Color::Blue));
        let low = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
        let three = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let Status::Decision(menu) = settle(&mut game, &mut events) else {
            panic!("expected the [3] or [1] choice");
        };
        assert_eq!(
            menu.option_label(1, A),
            Some("Draw and meld one higher than the last Brethren meld (a [1] if none)")
        );
        game.choose(usize::from(one)).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        let (melded, left, age) = if one {
            (low, three, 3)
        } else {
            (three, low, 1)
        };
        assert_eq!(position.top(A, position.card(melded).color), Some(melded));
        assert_eq!(position.cards(Zone::Deck(Deck::base(age))), [left]);
    }
}

#[test]
fn secretum_returns_five_draws_by_color_count_then_melds_one_and_scores_one() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U033", Zone::Board(A, Color::Blue));
    let hand: Vec<_> = [Color::Red, Color::Red, Color::Green]
        .into_iter()
        .map(|color| f.tech(color, 1, Zone::Hand(A), vec![]))
        .collect();
    let scored: Vec<_> = [Color::Blue, Color::Blue]
        .into_iter()
        .map(|color| f.tech(color, 2, Zone::Score(A), vec![]))
        .collect();
    let drawn = [
        f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]),
        f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    // Three colors returned: two [3]s, one melded and one scored.
    let _ = (hand, scored);
    assert!(position.cards(Zone::Hand(A)).is_empty());
    let melded = drawn
        .iter()
        .filter(|&&d| position.top(A, position.card(d).color) == Some(d))
        .count();
    let scored = drawn
        .iter()
        .filter(|d| position.cards(Zone::Score(A)).contains(d))
        .count();
    assert_eq!((melded, scored), (1, 1));
}

#[test]
fn masquerade_safeguards_by_hand_size_returns_that_value_and_may_claim_anonymity() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U034", Zone::Board(A, Color::Purple));
    let four_a = f.tech(Color::Red, 4, Zone::Hand(A), vec![]);
    let four_b = f.tech(Color::Blue, 4, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
    let supply = f.tech(Color::Purple, 4, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[supply]);
    assert_eq!(position.cards(Zone::Hand(A)).len(), 2);
    assert!(position.cards(Zone::Deck(Deck::base(4))).contains(&four_a));
    assert!(position.cards(Zone::Deck(Deck::base(4))).contains(&four_b));
    assert_eq!(
        position.special_location(Special::Anonymity),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn taqiyya_takes_a_color_into_hand_and_scores_a_bottom_meld_with_its_color() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U035", Zone::Board(A, Color::Purple));
    let red_a = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let red_b = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let colors = decision(&mut game, &mut events);
    let red = colors
        .options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << Color::Red as u8))
        .unwrap();
    game.choose(red).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
    hand.sort();
    assert_eq!(hand, vec![red_a, red_b]);
    assert!(game.position().cards(Zone::Board(A, Color::Red)).is_empty());

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U035", Zone::Board(A, Color::Purple));
    let green_hand = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let three = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[green_hand]));
    let position = game.position();
    let mut scored = position.cards(Zone::Score(A)).to_vec();
    scored.sort();
    assert_eq!(scored, vec![green_hand, three]);
    assert_eq!(position.cards(Zone::Hand(A)).len(), 1);
}
