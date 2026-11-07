//! Unseen draw routing: each player's first draw of a turn comes from the
//! Unseen deck, after Echoes' priority, with Base fallback and age skipping.

use inno_engine::*;

mod common;
use common::*;

fn unseen(age: u8) -> Deck {
    Deck {
        set: CardSet::Unseen,
        age,
    }
}

fn echoes(age: u8) -> Deck {
    Deck {
        set: CardSet::Echoes,
        age,
    }
}

fn draw(game: &mut Game, set: Option<CardSet>, value: u8, player: Player) -> CardId {
    game.start(Operation::Draw { value, set }, Context::new(player))
        .unwrap();
    let mut events = Vec::new();
    let Status::Complete(ReturnValue::Operation(outcome)) = settle(game, &mut events) else {
        panic!("draw did not complete");
    };
    outcome.card.expect("a drawn card")
}

/// Each player's first draw of a turn is Unseen, on either player's turn;
/// later draws are Base until the next turn begins.
#[test]
fn the_first_draw_of_each_player_each_turn_is_unseen() {
    let mut f = Fixture::default();
    let u1 = [
        f.tech(Color::Red, 1, Zone::Deck(unseen(1)), vec![]),
        f.tech(Color::Blue, 1, Zone::Deck(unseen(1)), vec![]),
        f.tech(Color::Green, 1, Zone::Deck(unseen(1)), vec![]),
    ];
    let b1 = [
        f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]),
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]),
        f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]),
    ];
    for card in u1.iter().chain(&b1) {
        f.cards[usize::from(card.0)].set = if u1.contains(card) {
            CardSet::Unseen
        } else {
            CardSet::Base
        };
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    assert!(!game.position().first_drawn(A));
    // A's turn: A draws Unseen, then Base; B's draw during it is B's first.
    assert_eq!(draw(&mut game, None, 1, A), u1[2]);
    assert!(game.position().first_drawn(A));
    assert_eq!(draw(&mut game, None, 1, A), b1[2]);
    assert_eq!(draw(&mut game, None, 1, B), u1[1]);
    assert_eq!(draw(&mut game, None, 1, B), b1[1]);
    // A new turn resets both players.
    game.begin_turn(B).unwrap();
    assert!(!game.position().first_drawn(A) && !game.position().first_drawn(B));
    assert_eq!(draw(&mut game, None, 1, B), u1[0]);
    assert_eq!(draw(&mut game, None, 1, A), b1[0]);
    assert!(game.position().check_conservation());
}

/// Echoes' unique-highest-top replacement has priority, and that draw is
/// still the player's first; an empty Unseen deck falls back to Base, and an
/// empty Base age is skipped for Unseen draws as for any set.
#[test]
fn echoes_has_priority_and_fallbacks_do_not_retry_replacement() {
    let mut f = Fixture::default();
    f.tech(Color::Blue, 5, Zone::Board(A, Color::Blue), vec![]);
    let e2 = f.add(echoes_card("E016").unwrap(), Zone::Deck(echoes(2)));
    let u2 = f.tech(Color::Red, 2, Zone::Deck(unseen(2)), vec![]);
    f.cards[usize::from(u2.0)].set = CardSet::Unseen;
    let b2 = [
        f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]),
        f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]),
    ];
    let b3 = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let expansions = Expansions::UNSEEN.union(Expansions::ECHOES);
    let mut game = Game::new(f.position(), expansions).unwrap();
    // Unique highest top: Echoes wins and consumes the first draw.
    assert_eq!(draw(&mut game, None, 2, A), e2);
    assert!(game.position().first_drawn(A));
    assert_eq!(draw(&mut game, None, 2, A), b2[1]);
    // B's first draw: no Echoes (B has no top), Unseen.
    assert_eq!(draw(&mut game, None, 2, B), u2);
    assert_eq!(draw(&mut game, None, 2, A), b2[0]);
    // B's turn: Unseen 2 and Base 2 are empty, so the age skips to 3;
    // Unseen 3 is empty and falls back to Base 3 without retrying.
    game.begin_turn(B).unwrap();
    assert_eq!(draw(&mut game, None, 2, B), b3);
    assert!(game.position().check_conservation());
}

/// An explicit set draw consumes the first draw too; a direct deck top
/// (Teleprompter-style) is not a draw and leaves it available.
#[test]
fn explicit_set_draws_consume_the_first_draw_but_deck_tops_do_not() {
    let mut f = Fixture::default();
    let u1 = f.tech(Color::Red, 1, Zone::Deck(unseen(1)), vec![]);
    f.cards[usize::from(u1.0)].set = CardSet::Unseen;
    let b1 = [
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]),
        f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]),
        f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    game.start(
        Operation::DrawTop {
            deck: Deck::base(1),
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[b1[2]]);
    assert!(!game.position().first_drawn(A));
    assert_eq!(draw(&mut game, Some(CardSet::Base), 1, A), b1[1]);
    assert!(game.position().first_drawn(A));
    assert_eq!(draw(&mut game, None, 1, A), b1[0]);
    assert_eq!(game.position().cards(Zone::Deck(unseen(1))), &[u1]);
}

/// A First draw karma's own draw takes the replacement; the resumed original draw is no
/// longer the turn's first.
#[test]
fn a_first_karma_draw_takes_the_replacement_before_the_original() {
    let mut f = Fixture::default();
    f.figure(Figure::FuXi, Zone::Board(A, Color::Green));
    let u1 = f.tech(Color::Blue, 1, Zone::Deck(unseen(1)), vec![]);
    let u2 = f.tech(Color::Green, 2, Zone::Deck(unseen(2)), vec![]);
    for card in [u1, u2] {
        f.cards[usize::from(card.0)].set = CardSet::Unseen;
    }
    let b1 = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let b2 = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let expansions = Expansions::UNSEEN.union(Expansions::FIGURES);
    let mut game = Game::new(f.position(), expansions).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    for _ in 0..100 {
        match settle(&mut game, &mut events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => break,
            status => panic!("{status:?}"),
        }
    }
    // Fu Xi's [2] came from Unseen and was scored; the original [1] is Base.
    assert_eq!(game.position().cards(Zone::Score(A)), &[u2]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[b1]);
    assert_eq!(game.position().cards(Zone::Deck(unseen(1))), &[u1]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[b2]);
}
