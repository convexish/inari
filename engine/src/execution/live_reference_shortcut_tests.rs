use super::common::*;
use super::*;

fn construction(ages: &[u8], figure_in_hand: bool) -> (Fixture, CardId) {
    let mut f = Fixture::default();
    let source = f.add(base_card("B016").unwrap(), Zone::Board(A, Color::Red));
    for &age in ages {
        f.tech(Color::Blue, age, Zone::Hand(B), vec![]);
    }
    if figure_in_hand {
        f.figure(Figure::Shennong, Zone::Hand(B));
    }
    f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    (f, source)
}
fn anatomy() -> (Fixture, CardId) {
    let mut f = Fixture::default();
    let source = f.add(base_card("B038").unwrap(), Zone::Board(A, Color::Yellow));
    f.tech(Color::Blue, 2, Zone::Score(B), vec![]);
    f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    for color in [Color::Purple, Color::Green, Color::Blue] {
        f.tech(color, 4, Zone::Deck(Deck::base(4)), vec![]);
    }
    (f, source)
}
fn start(game: &mut Game, source: CardId, controller: Player) {
    let mut context = Context::new(B);
    context.demander = A;
    context.controller = controller;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}
fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            let Alternative::Move { verb, target } = option else {
                return None;
            };
            let (card, slot) = match target {
                TargetDescriptor::Known { card, .. } => (*card, 0),
                TargetDescriptor::Hidden { source, slot, .. } => {
                    (game.position.cards(*source)[*slot], *slot)
                }
            };
            if *verb == Verb::Junk {
                Some(((0, 0, usize::MAX - slot), index))
            } else {
                let deck = game.position.card(card).origin_deck();
                Some(((deck.age, deck.set as u8, usize::from(card.0)), index))
            }
        })
        .min_by_key(|&(key, _)| key)
        .unwrap()
        .1
}
fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            result => return result,
        }
    }
    panic!("shortcut oracle did not finish");
}
// Hands, score piles and junk are unordered: only their contents matter.
fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}
fn leaves(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 10);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut clone = game.clone();
                clone.choose(index).unwrap();
                leaves(clone, depth + 1)
            })
            .collect(),
        Status::Complete(ReturnValue::Unit) => {
            assert!(game.position.check_conservation());
            vec![game]
        }
        other => panic!("unexpected shortcut leaf {other:?}"),
    }
}
fn classes<const F: bool>(f: Fixture, source: CardId, reduced_count: usize, manual_count: usize) {
    let mut reduced = Game::seeded(f.position(), Expansions::with_figures(F), 412).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, B);
    start(&mut manual, source, B);
    let reduced = leaves(reduced, 0);
    let manual = leaves(manual, 0);
    assert_eq!(reduced.len(), reduced_count);
    assert_eq!(manual.len(), manual_count);
    for game in &manual {
        assert!(
            reduced.iter().any(
                |other| normalized(other.position.clone()) == normalized(game.position.clone())
            )
        );
        assert_eq!(game.rng, reduced[0].rng);
    }
    for game in &reduced {
        assert!(
            manual.iter().any(
                |other| normalized(other.position.clone()) == normalized(game.position.clone())
            )
        );
    }
}

#[test]
fn construction_preserves_subsets_but_not_orders_into_the_demanders_hand() {
    fn exercise<const F: bool>() {
        for (ages, reduced, manual) in [
            (&[2, 1][..], 1, 2),
            (&[1, 1][..], 1, 2),
            (&[1, 2, 3][..], 6, 6),
        ] {
            let (f, source) = construction(ages, false);
            classes::<F>(f, source, reduced, manual);
        }
    }
    exercise::<false>();
    exercise::<true>();
    let (f, source) = construction(&[1], true);
    classes::<true>(f, source, 1, 2);
}

#[test]
fn anatomy_mandatory_deck_sweep_has_one_reduced_path_and_all_six_unrestricted_orders() {
    let (f, source) = anatomy();
    classes::<false>(f, source, 1, 6);
    let (f, source) = anatomy();
    classes::<true>(f, source, 1, 6);
}

#[test]
fn canonical_events_rng_and_clones_match_at_every_work_boundary_for_both_phases() {
    for (f, source) in [construction(&[2, 1], false), anatomy()] {
        let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 412).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, B);
        start(&mut manual, source, B);
        let mut events = Vec::new();
        let mut manual_events = Vec::new();
        let expected = finish(&mut manual, &mut manual_events);
        for _ in 0..200 {
            let mut fork = reduced.clone();
            let mut suffix = Vec::new();
            assert_eq!(finish(&mut fork, &mut suffix), expected);
            manual.shortcuts_disabled = false;
            assert_eq!(fork, manual);
            manual.shortcuts_disabled = true;
            let status = reduced.advance(1, &mut events).unwrap();
            match status {
                Status::WorkLimit => {}
                Status::Decision(menu) => reduced.choose(canonical(&reduced, &menu)).unwrap(),
                other => {
                    assert_eq!(other, expected);
                    break;
                }
            }
        }
        assert_eq!(events, manual_events);
        manual.shortcuts_disabled = false;
        assert_eq!(reduced, manual);
    }
}

#[test]
fn native_transfer_hazards_fall_back_but_unrelated_draw_and_dogma_hooks_do_not() {
    for (code, reduced) in [("F071", 2), ("F012", 1), ("F008", 1)] {
        let (mut f, source) = construction(&[2, 1], false);
        let figure = figure_card(code).unwrap();
        let color = figure.color;
        f.add(figure, Zone::Board(A, color));
        classes::<true>(f, source, reduced, 2);
    }
}

#[test]
fn foreign_controllers_keep_unrestricted_identical_hidden_back_menus() {
    let mut menus = Vec::new();
    // Two identical backs: the controller's full menu must not depend on
    // the hidden fronts. (Different backs may still be grouped by back.)
    for swap_fronts in [false, true] {
        let (mut f, source) = construction(&[1, 1, 2], false);
        if swap_fronts {
            f.cards[1].color = Color::Yellow;
            f.cards[2].symbols[0] = Symbol::Icon(Icon::Health);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, source, A);
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.options.len(), 2);
        assert!(menu.options.iter().all(|option| matches!(
            option,
            Alternative::BlindMove {
                verb: Verb::Transfer,
                ..
            }
        )));
        menus.push(menu);
    }
    assert_eq!(menus[0], menus[1]);
}

#[test]
fn suppressed_relevant_transfer_hooks_restore_grouping_but_live_each_membership_does_not() {
    for (code, expected) in [("F071", 1), ("F074", 2)] {
        let (mut f, source) = construction(&[2, 1], false);
        let figure = figure_card(code).unwrap();
        let color = figure.color;
        f.add(
            figure,
            Zone::Board(if code == "F074" { B } else { A }, color),
        );
        let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if code == "F074" {
            reduced.begin_turn(B).unwrap();
        }
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        let mut context = Context::new(B);
        context.demander = A;
        context.set_suppress_karma(true);
        for game in [&mut reduced, &mut manual] {
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::default(),
                },
                context,
            )
            .unwrap();
        }
        assert_eq!(leaves(reduced, 0).len(), expected);
        assert_eq!(leaves(manual, 0).len(), 2);
    }
}

#[test]
fn anatomy_deck_junk_ignores_hand_and_score_only_hazards() {
    for code in ["F015", "F069"] {
        let (mut f, source) = anatomy();
        let figure = figure_card(code).unwrap();
        let color = figure.color;
        f.add(figure, Zone::Board(A, color));
        classes::<true>(f, source, 1, 6);
    }
}
