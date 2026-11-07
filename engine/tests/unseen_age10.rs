//! Unseen Age 10: one expected-outcome scenario per card (3D Printing lives
//! in `unseen_hard_cases.rs`).

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
fn fight_club_takes_a_secret_as_an_achievement_and_may_splay_yellow() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U096", Zone::Board(A, Color::Red));
    let secret = f.tech(Color::Blue, 3, Zone::Safe(B), vec![]);
    f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Yellow, 2, Zone::Board(A, Color::Yellow), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Achievements(A)), &[secret]);
    assert!(position.cards(Zone::Safe(B)).is_empty());
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::Up);
}

#[test]
fn secret_santa_makes_the_victim_meld_from_the_demanders_score_and_scores_three_tens() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U097", Zone::Board(A, Color::Red));
    let scored = f.tech(Color::Green, 2, Zone::Score(A), vec![]);
    let tens: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().top(B, Color::Green), Some(scored));
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    let mut score = game.position().cards(Zone::Score(A)).to_vec();
    score.sort();
    assert_eq!(score, tens);
}

#[test]
fn ride_hailing_melds_a_foreign_efficient_top_and_executes_it_or_draws_an_eleven() {
    for eligible in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U098", Zone::Board(A, Color::Yellow));
        let red = f.tech(
            Color::Red,
            1,
            Zone::Board(B, Color::Red),
            vec![build::draw(1)],
        );
        if eligible {
            icons(&mut f, red, &[Icon::Efficiency]);
        }
        let yellow = f.tech(Color::Yellow, 2, Zone::Board(B, Color::Yellow), vec![]);
        icons(&mut f, yellow, &[Icon::Efficiency]);
        let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let eleven = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 1, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if eligible {
            assert_eq!(position.top(A, Color::Red), Some(red));
            assert_eq!(position.cards(Zone::Hand(A)), &[one]);
        } else {
            assert_eq!(position.top(B, Color::Red), Some(red));
            assert_eq!(position.cards(Zone::Hand(A)), &[eleven]);
        }
        assert_eq!(position.top(B, Color::Yellow), Some(yellow));
    }
}

#[test]
fn whatchamacallit_draws_and_scores_each_value_missing_from_tops_and_score() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U099", Zone::Board(A, Color::Yellow));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let three = f.tech(Color::Green, 3, Zone::Score(A), vec![]);
    let mut expected = vec![three];
    for value in 1..=11u8 {
        let drawn = f.tech(Color::Blue, value, Zone::Deck(Deck::base(value)), vec![]);
        if value != 1 && value != 3 && value != 10 {
            expected.push(drawn);
        }
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(A, Color::Yellow), Some(card));
    let mut score = position.cards(Zone::Score(A)).to_vec();
    score.sort();
    // Yellow 10 is a top card, so the 10 stays in its deck.
    assert_eq!(score, expected);
    assert_eq!(position.cards(Zone::Deck(Deck::base(10))).len(), 1);
}

#[test]
fn cryptocurrency_returns_the_score_pile_and_scores_a_ten_per_distinct_value() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U100", Zone::Board(A, Color::Green));
    f.tech(Color::Red, 2, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    f.tech(Color::Green, 5, Zone::Score(A), vec![]);
    let tens: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Deck(Deck::base(2))).len(), 2);
    let score = position.cards(Zone::Score(A));
    assert_eq!(score.len(), 2);
    assert!(score.iter().all(|c| tens.contains(c)));
}

#[test]
fn denver_airport_may_achieve_a_secret_regardless_of_eligibility() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U101", Zone::Board(A, Color::Green));
    let secret = f.tech(Color::Blue, 9, Zone::Safe(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2, "{:?}", menu.options);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[secret]);
}

#[test]
fn hacking_moves_the_two_highest_secrets_and_scores_and_melds_the_two_lowest() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U102", Zone::Board(A, Color::Blue));
    let secrets = [
        f.tech(Color::Red, 1, Zone::Safe(B), vec![]),
        f.tech(Color::Red, 2, Zone::Safe(B), vec![]),
        f.tech(Color::Red, 3, Zone::Safe(B), vec![]),
    ];
    let scored = [
        f.tech(Color::Green, 1, Zone::Score(B), vec![]),
        f.tech(Color::Purple, 4, Zone::Score(B), vec![]),
        f.tech(Color::Yellow, 5, Zone::Score(B), vec![]),
        f.tech(Color::Blue, 6, Zone::Score(B), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(B)), &[secrets[0]]);
    let mut taken = position.cards(Zone::Safe(A)).to_vec();
    taken.sort();
    assert_eq!(taken, &secrets[1..]);
    let mut score = position.cards(Zone::Score(A)).to_vec();
    score.sort();
    assert_eq!(score, &scored[2..]);
    assert!(position.cards(Zone::Score(B)).is_empty());
    assert_eq!(position.top(B, Color::Green), Some(scored[0]));
    assert_eq!(position.top(B, Color::Purple), Some(scored[1]));
}

#[test]
fn inhomogeneous_cosmology_repeats_while_a_top_is_placed_or_a_card_melded() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U103", Zone::Board(A, Color::Blue));
    let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let blue = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let one = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    // Place the red top, meld the blue 2, then decline both on the repeat.
    let menu = decision(&mut game, &mut events);
    game.choose(pick_known(&menu, red).unwrap()).unwrap();
    let menu = decision(&mut game, &mut events);
    game.choose(pick_known(&menu, blue).unwrap()).unwrap();
    loop {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                let decline = menu.options.iter().position(|o| *o == Alternative::Decline);
                game.choose(decline.unwrap()).unwrap();
            }
            status => {
                complete(status);
                break;
            }
        }
    }
    let position = game.position();
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[one, red]);
    assert_eq!(position.top(A, Color::Blue), Some(blue));
    assert!(position.cards(Zone::Hand(A)).is_empty());

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U103", Zone::Board(A, Color::Blue));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    for _ in 0..4 {
        f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 3);
}

#[test]
fn order_of_the_occult_hand_loses_on_a_scored_three_wins_on_a_held_seven_and_reshapes() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U105", Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    assert_eq!(run(&mut game, &mut events, &[]), Status::Winner(B));

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U105", Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    assert_eq!(run(&mut game, &mut events, &[]), Status::Winner(A));

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U105", Zone::Board(A, Color::Purple));
    let blue_low = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let melds = [
        f.tech(Color::Blue, 4, Zone::Hand(A), vec![]),
        f.tech(Color::Red, 4, Zone::Hand(A), vec![]),
    ];
    let scores: Vec<_> = (0..4)
        .map(|_| f.tech(Color::Green, 2, Zone::Hand(A), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 2, A, A);
    // Melds of different colours run in colour order: red before blue.
    complete(run(&mut game, &mut events, &[melds[1], melds[0]]));
    let position = game.position();
    assert_eq!(
        position.cards(Zone::Board(A, Color::Blue)),
        &[blue_low, melds[0]]
    );
    assert_eq!(position.top(A, Color::Red), Some(melds[1]));
    assert_eq!(position.cards(Zone::Score(A)), scores);
    assert_eq!(position.splay(A, Color::Blue), Splay::Up);
}
