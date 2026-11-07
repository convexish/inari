//! Group moves under `ByOrigin`: per-value return batches (Toilet, Barcode)
//! as one card menu over every eligible score card, lowest value group
//! first, keeping which cards go and same-deck order; and Calculator's
//! two-color score. Reduced play must reach exactly the unrestricted
//! oracle's outcomes.

use super::common::*;
use super::*;

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

/// B returns for A's demand.
fn demand_context() -> Context {
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    context
}

fn start(position: Position, expansions: Expansions, card: CardId) -> Game {
    let mut game = Game::seeded(position, expansions.union(Expansions::ECHOES), 37).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        demand_context(),
    )
    .unwrap();
    game
}

/// Reduced and unrestricted play: their path counts and distinct outcomes,
/// after checking they reach the same outcomes.
fn compare(position: Position, expansions: Expansions, card: CardId) -> (usize, usize, usize) {
    let reduced = start(position, expansions, card);
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
    let mut distinct: Vec<&Game> = Vec::new();
    for game in &manual {
        if !distinct.contains(&game) {
            distinct.push(game);
        }
    }
    (reduced.len(), manual.len(), distinct.len())
}

fn bonus(f: &mut Fixture, player: Player, color: Color, value: u8) {
    let card = f.tech(color, 1, Zone::Board(player, color), vec![]);
    f.cards[usize::from(card.0)].symbols[3] = Symbol::Bonus(value);
}

/// B's score pile: two Base [2]s (one deck), an Echoes [2], a Base and an
/// Echoes [3], and a Base [4] that matches nothing.
fn score_pile(f: &mut Fixture) {
    for (value, set) in [
        (2, CardSet::Base),
        (2, CardSet::Base),
        (2, CardSet::Echoes),
        (3, CardSet::Base),
        (3, CardSet::Echoes),
        (4, CardSet::Base),
    ] {
        let card = f.tech(Color::Purple, value, Zone::Score(B), vec![]);
        f.cards[usize::from(card.0)].set = set;
    }
}

fn returned(game: &Game) -> Vec<u8> {
    let mut values: Vec<_> = (2..=4)
        .flat_map(|age| {
            [CardSet::Base, CardSet::Echoes]
                .map(|set| game.position.cards(Zone::Deck(Deck { set, age })).len())
                .into_iter()
                .flat_map(move |count| std::iter::repeat_n(age, count))
        })
        .collect();
    values.sort();
    values
}

#[test]
fn barcode_returns_each_bonus_value_group_in_one_order() {
    // B shows 2, 2 and 3: two [2]s and one [3] go back.
    let mut f = Fixture::default();
    bonus(&mut f, B, Color::Red, 2);
    bonus(&mut f, B, Color::Blue, 2);
    bonus(&mut f, B, Color::Green, 3);
    score_pile(&mut f);
    let barcode = f.add(echoes_card("E101").unwrap(), Zone::Hand(A));
    let position = f.position();
    let (reduced, manual, distinct) = compare(position.clone(), Expansions::ECHOES, barcode);
    assert!(reduced < manual, "{reduced} {manual}");
    // Which two [2]s (and in which order onto one deck), and which [3].
    assert_eq!(reduced, distinct);
    let mut game = start(position, Expansions::ECHOES, barcode);
    while let Status::Decision(_) = settle(&mut game, &mut Vec::new()) {
        game.choose(0).unwrap();
    }
    assert_eq!(returned(&game), [2, 2, 3]);
}

#[test]
fn toilet_returns_one_card_of_each_demander_bonus_value_in_one_order() {
    // A shows 2, 2 and 3: one [2] and one [3] go back.
    let mut f = Fixture::default();
    bonus(&mut f, A, Color::Red, 2);
    bonus(&mut f, A, Color::Blue, 2);
    bonus(&mut f, A, Color::Green, 3);
    score_pile(&mut f);
    let toilet = f.add(echoes_card("E045").unwrap(), Zone::Hand(A));
    let position = f.position();
    let (reduced, manual, distinct) = compare(position.clone(), Expansions::ECHOES, toilet);
    assert!(reduced < manual, "{reduced} {manual}");
    assert_eq!(reduced, distinct);
    let mut game = start(position, Expansions::ECHOES, toilet);
    while let Status::Decision(_) = settle(&mut game, &mut Vec::new()) {
        game.choose(0).unwrap();
    }
    assert_eq!(returned(&game), [2, 3]);
}

#[test]
fn a_figure_watching_returns_keeps_every_order() {
    // B shows 2 and 3 off Maxwell's blue: one [2] and one [3] go back.
    // Without him one order is enough; Maxwell draws an [8] before each
    // return below 8, on either board, so every order stays.
    for owner in [None, Some(A), Some(B)] {
        let mut f = Fixture::default();
        bonus(&mut f, B, Color::Red, 2);
        bonus(&mut f, B, Color::Green, 3);
        score_pile(&mut f);
        for _ in 0..4 {
            f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
        }
        if let Some(owner) = owner {
            let card = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
            let color = card.color;
            f.add(card, Zone::Board(owner, color));
        }
        let barcode = f.add(echoes_card("E101").unwrap(), Zone::Hand(A));
        let (reduced, manual, _) = compare(f.position(), Expansions::FIGURES, barcode);
        if owner.is_some() {
            assert_eq!(reduced, manual, "Maxwell on {owner:?}");
        } else {
            assert!(reduced < manual, "{reduced} {manual}");
        }
    }
}

/// Run `card`'s demand on B to the end, answering with `pick`; its events.
fn run_demand(
    position: Position,
    card: CardId,
    mut pick: impl FnMut(&Decision) -> usize,
) -> (Game, Vec<Event>) {
    let mut game = start(position, Expansions::FIGURES, card);
    let mut events = Vec::new();
    while let Status::Decision(decision) = settle(&mut game, &mut events) {
        let choice = pick(&decision);
        game.choose(choice).unwrap();
    }
    (game, events)
}

#[test]
fn toilet_also_returns_a_card_a_karma_scores_mid_demand() {
    // Peter the Great scores B's green [2] before B returns the [3]; the
    // [2] cap is still open, so it goes back too.
    let mut f = Fixture::default();
    bonus(&mut f, A, Color::Red, 2);
    bonus(&mut f, A, Color::Blue, 3);
    f.figure(Figure::PeterTheGreat, Zone::Board(B, Color::Red));
    let green = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    let three = f.tech(Color::Purple, 3, Zone::Score(B), vec![]);
    let toilet = f.add(echoes_card("E045").unwrap(), Zone::Hand(A));
    let (game, _) = run_demand(f.position(), toilet, |_| 0);
    assert_eq!(game.position.locate(three), Some(Zone::Deck(Deck::base(3))));
    assert_eq!(game.position.locate(green), Some(Zone::Deck(Deck::base(2))));
    assert!(game.position.cards(Zone::Score(B)).is_empty());
}

#[test]
fn barcode_attempts_a_card_again_for_each_bonus() {
    // B shows two 6s (one Napoleon's own) and has one [6] scored. Napoleon
    // melds it instead of each return and B scores it back (a top 6), so
    // the second bonus attempts the same card again.
    let mut f = Fixture::default();
    bonus(&mut f, B, Color::Green, 6);
    f.figure(Figure::NapoleonBonaparte, Zone::Board(B, Color::Red));
    let six = f.tech(Color::Purple, 6, Zone::Score(B), vec![]);
    let barcode = f.add(echoes_card("E101").unwrap(), Zone::Hand(A));
    let (_, events) = run_demand(f.position(), barcode, |decision| {
        decision
            .options
            .iter()
            .position(|option| {
                matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == six)
            })
            .unwrap_or(0)
    });
    let melds = events
        .iter()
        .filter(|event| {
            matches!(event, Event::Moved(movement)
                if movement.card == six && movement.source == Zone::Score(B))
        })
        .count();
    assert_eq!(melds, 2);
}

#[test]
fn calculator_orders_its_two_scores_only_when_both_must_go() {
    // With two non-blue colors both are scored, in one order; with three,
    // which two is a real choice. Totals of 12 or more draw nothing.
    // A red [1] under a [4] and a green [2] under a [5]: the first round
    // totals 3, draws it and repeats over the exposed 4 and 5 (total 9).
    let repeat: &[(Color, &[u8])] = &[(Color::Red, &[1, 4]), (Color::Green, &[2, 5])];
    for (piles, expected) in [
        (&[(Color::Red, &[6][..]), (Color::Green, &[7])][..], (1, 2)),
        (
            &[
                (Color::Red, &[6][..]),
                (Color::Green, &[7]),
                (Color::Yellow, &[8]),
            ],
            (6, 6),
        ),
        (repeat, (1, 4)),
    ] {
        let mut f = Fixture::default();
        for &(color, values) in piles {
            for &value in values {
                f.tech(color, value, Zone::Board(A, color), vec![]);
            }
        }
        for age in [3, 9] {
            f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
        }
        f.tech(Color::Blue, 9, Zone::Board(A, Color::Blue), vec![]);
        let calculator = f.add(echoes_card("E092").unwrap(), Zone::Hand(A));
        let mut reduced = Game::seeded(f.position(), Expansions::ECHOES, 41).unwrap();
        reduced
            .start(
                Operation::ExecuteEffect {
                    card: calculator,
                    effect: 0,
                    boxed: BoxedValues::default(),
                },
                Context::new(A),
            )
            .unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        // Score piles are unordered: only their contents matter.
        let normalized = |games: Vec<Game>| -> Vec<Position> {
            games
                .into_iter()
                .map(|game| {
                    let mut position = game.position;
                    position.sort_unordered_piles();
                    position
                })
                .collect()
        };
        let reduced = normalized(outcomes(reduced, 0));
        let manual = normalized(outcomes(manual, 0));
        assert!(
            manual.iter().all(|o| reduced.contains(o)),
            "missing outcome"
        );
        assert!(
            reduced.iter().all(|o| manual.contains(o)),
            "invented outcome"
        );
        assert_eq!((reduced.len(), manual.len()), expected, "{piles:?}");
    }
}
