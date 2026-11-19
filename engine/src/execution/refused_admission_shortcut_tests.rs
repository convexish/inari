//! A single move into a full Forecast or Safe (Rubber, Globe, Tuning Fork):
//! capacity refuses every pick before any karma sees it, so one pick stands
//! for all. Reduced play must reach exactly the unrestricted oracle's
//! outcomes.

use super::common::*;
use super::*;
use crate::program::build::*;

fn outcomes(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 10);
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
            game.shortcuts_disabled = false;
            vec![game]
        }
        Status::WorkLimit => panic!("unexpected oracle work limit"),
    }
}

/// A runs one synthetic effect with four tops, two cards in hand, two
/// available achievements, and a Forecast (Echoes) or Safe (Unseen) that is
/// full or not.
fn start(instruction: Instruction, expansions: Expansions, full: bool) -> Game {
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![instruction],
    );
    for color in [Color::Red, Color::Blue, Color::Green] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
    }
    for value in [2, 3] {
        f.tech(Color::Yellow, value, Zone::Hand(A), vec![]);
        f.tech(Color::Yellow, value, Zone::AvailableAchievements, vec![]);
    }
    if full {
        let zone = if expansions.echoes() {
            Zone::Forecast(A)
        } else {
            Zone::Safe(A)
        };
        for _ in 0..5 {
            f.tech(Color::Red, 4, zone, vec![]);
        }
    }
    let mut game = Game::seeded(f.position(), expansions, 37).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    game
}

/// Reduced and unrestricted path counts, after checking both reach the same
/// outcomes.
fn compare(instruction: Instruction, expansions: Expansions, full: bool) -> (usize, usize) {
    let reduced = start(instruction, expansions, full);
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

const TOPS: Selector = Selector::Tops(Who::Executor);
const HAND: Selector = Selector::Hand(Who::Executor);

#[test]
fn full_forecast_asks_no_top_or_hand_card_to_foreshadow() {
    // Rubber and Globe pick a top; Tuning Fork a card in hand.
    assert_eq!(compare(foreshadow(TOPS), Expansions::ECHOES, true), (1, 4));
    assert_eq!(compare(foreshadow(HAND), Expansions::ECHOES, true), (1, 2));
}

#[test]
fn forecast_with_room_keeps_every_pick() {
    assert_eq!(compare(foreshadow(TOPS), Expansions::ECHOES, false), (4, 4));
    assert_eq!(compare(foreshadow(HAND), Expansions::ECHOES, false), (2, 2));
}

#[test]
fn optional_pick_into_a_full_forecast_keeps_the_decline() {
    assert_eq!(
        compare(foreshadow(HAND).optional(), Expansions::ECHOES, true),
        (2, 3)
    );
}

#[test]
fn full_safe_asks_no_card_to_safeguard() {
    assert_eq!(compare(safeguard(HAND), Expansions::UNSEEN, true), (1, 2));
    assert_eq!(compare(safeguard(HAND), Expansions::UNSEEN, false), (2, 2));
}

#[test]
fn blind_pick_into_a_full_safe_keeps_its_menu() {
    // Achievement backs are hidden: the native sampler still runs.
    let blind = safeguard(Selector::StandardAchievements);
    let (reduced, manual) = compare(blind, Expansions::UNSEEN, true);
    assert_eq!(reduced, manual);
    assert!(manual > 1);
}
