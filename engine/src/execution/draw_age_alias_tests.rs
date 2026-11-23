//! Public `Expr::DrawAge` aliases: values that skip empty Base decks to the
//! same resolved draw age (or exhaustion) merge into one option, preserving
//! set routing, draw karmas and decline options.

use super::common::*;
use super::*;

fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng, Status)> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status @ (Status::Complete(_) | Status::Draw(_) | Status::Winner(_)) => {
            assert!(game.position.check_conservation());
            vec![(normalized(game.position), game.rng, status)]
        }
        Status::WorkLimit => panic!("unexpected oracle work limit"),
    }
}

fn compare(reduced: Game) -> (usize, usize) {
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert!(
        manual.iter().all(|o| reduced.contains(o)),
        "missing outcome"
    );
    assert!(
        reduced.iter().all(|o| manual.contains(o)),
        "invented outcome"
    );
    (reduced.len(), manual.len())
}

fn start_effect(game: &mut Game, source: CardId, effect: u16) {
    let mut context = Context::new(A);
    context.demander = B;
    context.source_owner = A;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

#[test]
fn atm_merges_empty_base_decks_and_exhaustion_across_echoes_and_figures() {
    for expansions in [
        Expansions::ECHOES,
        Expansions::ECHOES.union(Expansions::FIGURES),
    ] {
        let mut f = Fixture::default();
        let source = f.card("E088", Zone::Junk);
        // Only Base 4 and Base 8 exist: 1..=4 -> 4, 5..=8 -> 8, 9..=11 -> exhaustion (3 classes).
        f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
        // Echoes card at age 4 with unique highest top card tests Echoes routing too.
        f.card(
            "E036",
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 4,
            }),
        );
        f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
        if expansions.figures() {
            // A draw-watching Figure still fires identically on the resolved draw age.
            f.figure(Figure::CharlesDarwin, Zone::Board(A, Color::Blue));
        }
        let mut game = Game::seeded(f.position(), expansions, 31).unwrap();
        start_effect(&mut game, source, Card::ECHO);
        let mut probe = game.clone();
        let menu = decision(&mut probe, &mut Vec::new());
        assert_eq!(
            menu.options,
            [
                Alternative::Value(4),
                Alternative::Value(8),
                Alternative::Value(11),
            ]
        );
        assert_eq!(compare(game), (3, 11));
    }
}

#[test]
fn horseshoes_and_charitable_trust_merge_when_lower_base_deck_is_empty() {
    // Horseshoes (E017) echo: optional [2] or [3].
    for base_two_present in [false, true] {
        let mut f = Fixture::default();
        let source = f.card("E017", Zone::Junk);
        if base_two_present {
            f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
        }
        f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::seeded(f.position(), Expansions::ECHOES, 37).unwrap();
        start_effect(&mut game, source, Card::ECHO);
        // Optional decline stays: (2, 3) when Base 2 is empty, (3, 3) when present.
        let expected = if base_two_present { (3, 3) } else { (2, 3) };
        assert_eq!(compare(game), expected);
    }

    // Charitable Trust (E030) echo: mandatory [3] or [4].
    let mut f = Fixture::default();
    let source = f.card("E030", Zone::Junk);
    f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 41).unwrap();
    start_effect(&mut game, source, Card::ECHO);
    assert_eq!(compare(game), (1, 2));
}

#[test]
fn barometer_and_attic_merge_values_sharing_resolved_draw_age() {
    // Barometer (E038): bonuses 2, 5, 9, 10 -> draw 4, 7, 11, 12.
    // Only Base 7 exists: 2 (+2=4) and 5 (+2=7) both resolve to 7; 9 (+2=11) and 10 (+2=12) exhaust.
    let mut f = Fixture::default();
    let source = f.card("E038", Zone::Junk);
    f.card("E017", Zone::Board(A, Color::Red)); // bonus 2
    f.card("E054", Zone::Board(A, Color::Purple)); // bonus 5
    f.card("E088", Zone::Board(A, Color::Yellow)); // bonus 9
    f.card("E103", Zone::Board(A, Color::Blue)); // bonus 10
    f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 43).unwrap();
    start_effect(&mut game, source, 0);
    assert_eq!(compare(game), (2, 4));

    // Attic (U058) effect 2: draw a card of value in score pile.
    let mut f = Fixture::default();
    let source = f.card("U058", Zone::Junk);
    f.tech(Color::Red, 1, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    f.tech(Color::Green, 3, Zone::Score(A), vec![]);
    f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::UNSEEN, 47).unwrap();
    start_effect(&mut game, source, 2);
    assert_eq!(compare(game), (1, 3));
}
