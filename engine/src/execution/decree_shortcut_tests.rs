//! Decree choice reductions: War's no-op value alias, War/Rivalry cross-deck
//! top-return ordering, and Rivalry's distinct-back score-return ordering.

use super::common::*;
use super::*;

fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng)> {
    assert!(depth < 14);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        Status::Complete(_) | Status::Draw(_) | Status::Winner(_) => {
            assert!(game.position.check_conservation());
            vec![(normalized(game.position), game.rng)]
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

fn issue(game: &mut Game, decree: Decree) {
    game.start(Operation::IssueDecree { decree }, Context::new(A))
        .unwrap();
}

#[test]
fn war_collapses_noop_values_and_commutes_cross_deck_returns_while_keeping_same_deck_orders() {
    // Empty opposing board: all 11 values are no-ops -> 0 decisions.
    let mut game = Game::seeded(Fixture::default().position(), Expansions::FIGURES, 7).unwrap();
    issue(&mut game, Decree::War);
    assert_eq!(compare(game), (1, 11));

    // Opponent has a Base 3 and a Figure 3 (distinct decks) plus two Base 4s (same deck):
    // Reduced: no-op (1) + value 3 (1 order) + value 4 (2 deck orders) = 4 paths.
    // Manual: 9 no-ops + value 3 (2 orders) + value 4 (2 orders) = 13 paths.
    let mut f = Fixture::default();
    f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green)); // age 3 Figure
    f.tech(Color::Blue, 4, Zone::Board(B, Color::Blue), vec![]);
    f.tech(Color::Yellow, 4, Zone::Board(B, Color::Yellow), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 11).unwrap();
    issue(&mut game, Decree::War);
    assert_eq!(compare(game), (4, 13));
}

#[test]
fn rivalry_commutes_distinct_back_score_returns_and_distinct_age_figure_returns() {
    // 4 distinct-back score cards (choose 3 -> 4 subsets instead of 24) plus
    // 2 distinct-age top Figures (1 order instead of 2): 4 vs 48 paths.
    let mut f = Fixture::default();
    for age in 1..=4 {
        f.tech(Color::Red, age, Zone::Score(B), vec![]);
    }
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green)); // age 3
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red)); // age 4
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 19).unwrap();
    issue(&mut game, Decree::Rivalry);
    assert_eq!(compare(game), (4, 48));

    // A return-watching Figure (James Clerk Maxwell) blocks both stages.
    let mut f = Fixture::default();
    for age in 1..=3 {
        f.tech(Color::Red, age, Zone::Score(B), vec![]);
    }
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(B, Color::Blue));
    for _ in 0..5 {
        f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    }
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 23).unwrap();
    issue(&mut game, Decree::Rivalry);
    let (reduced, manual) = compare(game);
    assert_eq!(reduced, manual);
    assert!(manual >= 12);
}

#[test]
fn rivalry_covered_return_figure_blocks_top_return_reduction() {
    let mut f = Fixture::default();
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(B, Color::Blue));
    f.figure(Figure::Alhazen, Zone::Board(B, Color::Blue)); // age 3, covers Maxwell
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red)); // age 4
    f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 29).unwrap();
    issue(&mut game, Decree::Rivalry);
    assert_eq!(compare(game), (2, 2));
}

#[test]
fn war_uses_effective_board_value_for_battleship_yamato() {
    // Battleship Yamato (A077) has printed age 8 but effective board value 11.
    // War must offer 11 (plus one canonical no-op, 1) and return Yamato on 11.
    let mut f = Fixture::default();
    let yamato_color = Card::from_code("A077").unwrap().color;
    let yamato = f.card("A077", Zone::Board(B, yamato_color));
    let mut game = Game::seeded(f.position(), Expansions::from_bits(9).unwrap(), 13).unwrap();
    issue(&mut game, Decree::War);
    let mut probe = game.clone();
    let menu = decision(&mut probe, &mut Vec::new());
    assert_eq!(
        menu.options,
        vec![Alternative::Value(1), Alternative::Value(11)]
    );
    probe.choose(1).unwrap();
    assert!(matches!(
        settle(&mut probe, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert!(probe.position.top(B, yamato_color).is_none());
    assert!(
        probe
            .position
            .cards(Zone::Deck(Deck {
                set: CardSet::Artifacts,
                age: 8,
            }))
            .contains(&yamato)
    );
    assert_eq!(compare(game), (2, 11));
}
