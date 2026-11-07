//! Ownership cleanup reaches displays, stored Artifacts with their Museums
//! and Safes; a civilization's claimed Museums leave the game with it;
//! Tasmanian Tiger proves an impossible exchange.

use inno_engine::*;

mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("cleanup did not terminate");
}

/// Fission's red branch junks each player's non-achievement cards: displayed
/// Artifacts, stored Artifacts with the Museum cards holding them, and
/// secrets. Achievements stay.
#[test]
fn fission_junks_displayed_and_stored_artifacts_their_museums_and_secrets() {
    let mut f = Fixture::default();
    let fission = f.add(base_card("B087").unwrap(), Zone::Board(A, Color::Red));
    let display_a = f.add(artifacts_card("A001").unwrap(), Zone::Display(A));
    let display_b = f.add(artifacts_card("A002").unwrap(), Zone::Display(B));
    let museum = f.add(artifacts_card("A003").unwrap(), Zone::Museum(B));
    let secret = f.tech(Color::Blue, 2, Zone::Safe(A), vec![]);
    let achievement = f.tech(Color::Green, 1, Zone::Achievements(B), vec![]);
    f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]);
    let expansions = Expansions::ARTIFACTS.union(Expansions::UNSEEN);
    let mut game = Game::new(f.position(), expansions).unwrap();
    assert_eq!(game.position().available_museums(), 4);
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    game.start(
        Operation::ExecuteEffect {
            card: fission,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    let position = game.position();
    for card in [fission, display_a, display_b, museum, secret] {
        assert!(position.cards(Zone::Junk).contains(&card), "{card:?}");
    }
    assert_eq!(position.cards(Zone::Achievements(B)), &[achievement]);
    assert!(position.cards(Zone::Display(A)).is_empty());
    assert!(position.cards(Zone::Museum(B)).is_empty());
    // The Museum card that held A003 is junked with it, not returned.
    assert_eq!(position.available_museums(), 4);
    assert_eq!(position.junked_museums(), 1);
    assert!(position.check_conservation());
}

/// Elimination junks the civilization's claimed Museums too; the opponent's
/// pieces and the supply are untouched.
#[test]
fn elimination_junks_claimed_museums() {
    let mut f = Fixture::default();
    let stored = f.add(artifacts_card("A001").unwrap(), Zone::Museum(A));
    let theirs = f.add(artifacts_card("A002").unwrap(), Zone::Museum(B));
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    let mut events = Vec::new();
    // A Museum Check's award counts as claimed from the award on.
    game.start(Operation::ClaimMuseum { player: A }, Context::new(A))
        .unwrap();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().claimed_museums(A), 1);
    // The awarded Museum is the occupied one until its returns run.
    assert_eq!(game.position().available_museums(), 3);
    game.start(Operation::Lose { player: A }, Context::new(A))
        .unwrap();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    let position = game.position();
    assert!(position.cards(Zone::Junk).contains(&stored));
    assert_eq!(position.cards(Zone::Museum(B)), &[theirs]);
    assert_eq!(position.claimed_museums(A), 0);
    // The claimed Museum and the one that held A001 are both junked.
    assert_eq!(position.junked_museums(), 2);
    assert_eq!(position.available_museums(), 2);
}

/// With no top card of the chosen score card's color, Tasmanian Tiger reveals
/// that card before offering its optional returns.
#[test]
fn tiger_reveals_the_card_that_prevents_exchange() {
    let mut f = Fixture::default();
    let tiger = f.add(
        artifacts_card("A109").unwrap(),
        Zone::Board(A, Color::Yellow),
    );
    let red = f.tech(Color::Red, 2, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.self_execute(tiger, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let choice = decision(&mut game, &mut events);
    let index = choice
        .options
        .iter()
        .position(|o| {
            matches!(o, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == red)
        })
        .unwrap();
    game.choose(index).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Decision(_)
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == red)),
        "{events:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, Event::Moved(_))));
    // The opponent's worlds now pin the revealed card.
    for seed in 0..4 {
        let world = game.determinize(A, seed).unwrap();
        assert!(world.position().cards(Zone::Score(A)).contains(&red));
    }
}

/// With Wernher von Braun the junk counts as Tasmanian Tiger's score pile: a
/// chosen junk card trades places with the top card, which arrives in the
/// executor's score pile.
#[test]
fn tiger_exchanges_a_junk_card_counted_as_score_under_von_braun() {
    let mut f = Fixture::default();
    let tiger = f.add(
        artifacts_card("A109").unwrap(),
        Zone::Board(A, Color::Yellow),
    );
    f.figure(Figure::WernherVonBraun, Zone::Board(A, Color::Blue));
    let junked = f.tech(Color::Red, 2, Zone::Junk, vec![]);
    let top = f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    let expansions = Expansions::ARTIFACTS.union(Expansions::FIGURES);
    let mut game = Game::new(f.position(), expansions).unwrap();
    game.self_execute(tiger, Context::new(A)).unwrap();
    let mut events = Vec::new();
    while let Status::Decision(choice) = settle(&mut game, &mut events) {
        let index = choice.options.iter().position(|o| {
            matches!(o, Alternative::Select { target: TargetDescriptor::Known { card, .. } }
                if *card == junked || *card == top)
        });
        game.choose(index.expect("a Tiger choice")).unwrap();
    }
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), &[junked]);
    assert_eq!(position.cards(Zone::Score(A)), &[top]);
    assert!(position.check_conservation());
}
