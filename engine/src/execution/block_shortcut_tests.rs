//! Any-order blocks (Touchscreen's per-color splays): one printed order
//! unless a live Figure could observe a splay, or a City draw after one
//! could exhaust. Reduced play must reach exactly the unrestricted oracle's
//! outcomes.

use super::common::*;
use super::*;

fn outcomes(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 8);
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

fn seeded(position: Position, expansions: Expansions, screen: CardId) -> Game {
    let mut game = Game::seeded(position, expansions.union(Expansions::ECHOES), 31).unwrap();
    invoke(&mut game, screen);
    game
}

/// Reduced and unrestricted play: their path counts, after checking that
/// they reach the same outcomes.
fn compare(position: Position, expansions: Expansions, screen: CardId) -> (usize, usize) {
    let reduced = seeded(position, expansions, screen);
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

/// Touchscreen in A's hand and A's piles of these sizes, with a covered
/// bonus in each so splays change visible symbols.
fn touchscreen_with(piles: &[(Color, usize)]) -> (Fixture, CardId) {
    let mut f = Fixture::default();
    for &(color, size) in piles {
        for depth in 0..size {
            let card = f.tech(color, 1, Zone::Board(A, color), vec![]);
            if depth == 0 {
                f.cards[usize::from(card.0)].symbols[3] = Symbol::Bonus(2);
            }
        }
    }
    let screen = f.add(echoes_card("E113").unwrap(), Zone::Hand(A));
    (f, screen)
}

/// Three piles Touchscreen splays (2: aslant, 4: up, 8: left).
fn touchscreen() -> (Fixture, CardId) {
    touchscreen_with(&[(Color::Red, 2), (Color::Blue, 4), (Color::Green, 8)])
}

fn figure_on(f: &mut Fixture, player: Player, figure: Figure) {
    let color = figure_card(figure.code()).unwrap().color;
    f.figure(figure, Zone::Board(player, color));
}

/// Unrestricted play's different outcomes.
fn distinct_outcomes(position: Position, expansions: Expansions, source: CardId) -> Vec<Game> {
    let mut manual = seeded(position, expansions, source);
    manual.shortcuts_disabled = true;
    let mut distinct: Vec<Game> = Vec::new();
    for game in outcomes(manual, 0) {
        if !distinct.contains(&game) {
            distinct.push(game);
        }
    }
    distinct
}

#[test]
fn touchscreen_takes_one_color_order_without_a_decision() {
    let (f, screen) = touchscreen();
    let position = f.position();
    let mut game = seeded(position.clone(), Expansions::ECHOES, screen);
    assert!(!matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Decision(_)
    ));
    for (color, splay) in [
        (Color::Red, Splay::Aslant),
        (Color::Blue, Splay::Up),
        (Color::Green, Splay::Left),
    ] {
        assert_eq!(game.position.splay(A, color), splay);
    }
    // Every one of the six orders reaches that outcome.
    assert_eq!(compare(position, Expansions::ECHOES, screen), (1, 6));
}

#[test]
fn a_figure_watching_splays_keeps_the_order_choice() {
    // Machiavelli's First clause watches every splay, on either board;
    // Archimedes only changes effect values.
    let figures = Expansions::FIGURES;
    let (mut f, screen) = touchscreen();
    figure_on(&mut f, B, Figure::NiccoloMachiavelli);
    assert_eq!(compare(f.position(), figures, screen), (6, 6));
    let (mut f, screen) = touchscreen();
    figure_on(&mut f, B, Figure::Archimedes);
    assert_eq!(compare(f.position(), figures, screen), (1, 6));
    // On A's board Machiavelli reacts: each splay of A's first unsplays that
    // color on B's board. His own one-card purple pile joins the colors.
    let (mut f, screen) = touchscreen();
    figure_on(&mut f, A, Figure::NiccoloMachiavelli);
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    }
    let position = f.position().with_splay(B, Color::Red, Splay::Left).unwrap();
    let ended = distinct_outcomes(position.clone(), figures, screen);
    assert!(
        ended
            .iter()
            .all(|game| game.position.splay(B, Color::Red) == Splay::None)
    );
    assert_eq!(compare(position, figures, screen), (24, 24));
}

#[test]
fn with_cities_the_order_is_one_only_when_city_draws_cannot_exhaust() {
    // The first splay draws the City, so A then holds one and later splays
    // draw nothing; Base [1]s for every pass prove no draw can exhaust.
    let cities = Expansions::CITIES;
    let (mut f, screen) = touchscreen();
    f.add(city_card("C007").unwrap(), Zone::Deck(Deck::cities(1)));
    for _ in 0..3 {
        f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    assert_eq!(compare(f.position(), cities, screen), (1, 6));
    // With no supply the first splay's draw ends the game, so which pile
    // splays first matters and every order stays a choice.
    let (f, screen) = touchscreen();
    let position = f.position();
    assert_eq!(distinct_outcomes(position.clone(), cities, screen).len(), 3);
    let (reduced, manual) = compare(position, cities, screen);
    assert_eq!(reduced, manual);
}

#[test]
fn a_figure_watching_city_draws_keeps_the_order_choice() {
    // Hawking on B's board watches only B's draws, but the guard is
    // conservative.
    let both = Expansions::CITIES.union(Expansions::FIGURES);
    let (mut f, screen) = touchscreen();
    for _ in 0..9 {
        f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    figure_on(&mut f, B, Figure::StephenHawking);
    let (reduced, manual) = compare(f.position(), both, screen);
    assert_eq!(reduced, manual);
    // On A's board (atop blue) he reacts to each splay's City draw by first
    // drawing and tucking a red [10], so the red pile's size, and its
    // splay, depend on how many splays came before it.
    let (mut f, screen) = touchscreen_with(&[(Color::Red, 3), (Color::Blue, 4), (Color::Green, 8)]);
    figure_on(&mut f, A, Figure::StephenHawking);
    for _ in 0..9 {
        f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]);
    }
    let position = f.position();
    assert!(distinct_outcomes(position.clone(), both, screen).len() > 1);
    let (reduced, manual) = compare(position, both, screen);
    assert_eq!(reduced, manual);
}

#[test]
fn city_draws_are_budgeted_per_splay_a_pass_can_reach() {
    // Each pass splays its color left, then right: two City draws a pass
    // while A holds no City. Two Base [1]s let the second color's first
    // draw exhaust, so the order matters; four cover every draw.
    const ITEM: Slot = Slot::effect(0);
    let program = vec![
        build::for_each(
            Domain::Colors(ColorSet::Only(&[Color::Red, Color::Blue])),
            Order::Chosen(Who::Executor),
            Some(ITEM),
            2,
        )
        .shortcut(Shortcut::AnyOrder),
        build::splay_set(ColorSet::Slot(ITEM), Splay::Left),
        build::splay_set(ColorSet::Slot(ITEM), Splay::Right),
    ];
    for (supply, expected) in [(2, (2, 2)), (4, (1, 2))] {
        let mut f = Fixture::default();
        for color in [Color::Red, Color::Blue] {
            for _ in 0..2 {
                f.tech(color, 1, Zone::Board(A, color), vec![]);
            }
        }
        for _ in 0..supply {
            f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        let source = f.tech(Color::Purple, 1, Zone::Hand(A), program.clone());
        let position = f.position();
        let distinct = distinct_outcomes(position.clone(), Expansions::CITIES, source);
        assert_eq!(distinct.len(), if supply == 2 { 2 } else { 1 });
        assert_eq!(compare(position, Expansions::CITIES, source), expected);
    }
}

#[test]
fn clones_resume_an_any_order_block_at_every_work_boundary() {
    let (f, screen) = touchscreen();
    let mut game = seeded(f.position(), Expansions::ECHOES, screen);
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = settle(&mut expected, &mut all);
    let mut prefix = Vec::new();
    for _ in 0..300 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(settle(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => panic!("unexpected reduced decision {menu:?}"),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
    }
    panic!("Touchscreen did not complete");
}

#[test]
fn a_block_shortcut_outside_its_proof_is_refused() {
    // A printed-order block, and a chosen one whose body draws.
    for (order, body) in [
        (Order::Printed, build::splay(&[Color::Red], Splay::Left)),
        (Order::Chosen(Who::Executor), build::draw(1)),
    ] {
        let mut f = Fixture::default();
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        let block = build::for_each(Domain::Colors(ColorSet::Only(&Color::ALL)), order, None, 1)
            .shortcut(Shortcut::AnyOrder);
        let source = f.tech(Color::Blue, 1, Zone::Hand(A), vec![block, body]);
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        invoke(&mut game, source);
        assert_eq!(
            game.advance(1000, &mut Vec::new()),
            Err(Error::Unsupported(
                "a block shortcut needs a frozen chosen order over splays"
            ))
        );
    }
}
