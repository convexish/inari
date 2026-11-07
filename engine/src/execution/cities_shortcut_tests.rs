//! Cities consequences inside reduced phases: a splay batch may draw a City
//! after each pile, and melds can claim Tradition/Repute/Fame. Reduced play
//! must reach exactly the unrestricted oracle's outcomes.

use super::common::*;
use super::*;

fn outcomes(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 12);
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

fn invoke(game: &mut Game, card: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

/// Reduced and unrestricted play from one position: their outcome sets and
/// how many distinct outcomes each path count reached.
fn compare(position: Position, expansions: Expansions, source: CardId) -> (usize, usize) {
    let mut reduced = Game::seeded(position, expansions, 29).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    invoke(&mut reduced, source);
    invoke(&mut manual, source);
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

const SPLAY_ALL_LEFT: Instruction = Instruction::Splay(SplayStep {
    who: Who::Executor,
    choices: SplayChoices::Fixed {
        colors: ColorSet::Only(&Color::ALL),
        direction: Splay::Left,
        from: None,
    },
    targets: SplayTargets::Changes,
    optional: false,
    all: true,
});

/// Three two-card piles whose covered cards show different bonuses under
/// Left; `base_twos` Base cards at the request age, plus one City there.
fn splay_batch(base_twos: usize, hawking: bool) -> (Position, CardId) {
    let mut f = Fixture::default();
    for (color, bonus) in [(Color::Red, 5), (Color::Blue, 3), (Color::Green, 4)] {
        let covered = f.tech(color, 1, Zone::Board(A, color), vec![]);
        f.cards[usize::from(covered.0)].symbols[3] = Symbol::Bonus(bonus);
        f.tech(color, 2, Zone::Board(A, color), vec![]);
    }
    if hawking {
        let card = figure_card(Figure::StephenHawking.code()).unwrap();
        f.add(card, Zone::Board(B, Color::Blue));
    }
    for _ in 0..base_twos {
        f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    }
    f.add(city_card("C016").unwrap(), Zone::Deck(Deck::cities(2)));
    f.tech(Color::Yellow, 4, Zone::Score(B), vec![]);
    let source = f.tech(Color::Purple, 1, Zone::Hand(A), vec![SPLAY_ALL_LEFT]);
    (f.position(), source)
}

#[test]
fn a_cities_splay_batch_takes_one_order_only_when_its_draws_cannot_exhaust() {
    // Ample Base supply: one canonical order reaches the only outcome every
    // order reaches (the first splay draws the City, the rest skip).
    let (position, source) = splay_batch(3, false);
    assert_eq!(compare(position, Expansions::CITIES, source), (1, 6));
    // An empty supply: the first splay's draw ends the game, so the pile
    // splayed first matters and every order stays a choice.
    let (position, source) = splay_batch(0, false);
    let (reduced, manual) = compare(position, Expansions::CITIES, source);
    assert_eq!(reduced, manual);
    // Hawking watches draws: with Cities every splay draws, so the guard keeps
    // the choice even with an ample supply.
    let both = Expansions::FIGURES.union(Expansions::CITIES);
    let (position, source) = splay_batch(9, true);
    let (reduced, manual) = compare(position, both, source);
    assert_eq!(reduced, manual);
}

#[test]
fn cities_meld_claims_keep_masonry_color_reductions_equivalent() {
    // Kaifeng and Constantinople both bear Left and Authority; each pile is
    // already splayed Left, so either meld claims Tradition first.
    let mut f = Fixture::default();
    for color in [Color::Yellow, Color::Purple] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
        f.tech(color, 1, Zone::Board(A, color), vec![]);
    }
    f.add(city_card("C029").unwrap(), Zone::Hand(A));
    f.add(city_card("C034").unwrap(), Zone::Hand(A));
    let masonry = f.add(base_card("B006").unwrap(), Zone::Junk);
    let position = f
        .position()
        .with_splay(A, Color::Yellow, Splay::Left)
        .unwrap()
        .with_splay(A, Color::Purple, Splay::Left)
        .unwrap();
    let (reduced, manual) = compare(position, Expansions::CITIES, masonry);
    assert!(reduced < manual, "{reduced} {manual}");
}
