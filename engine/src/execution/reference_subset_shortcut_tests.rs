use super::common::*;
use super::*;

fn fixture(code: &str, values: &[u8]) -> (Fixture, CardId) {
    let mut f = Fixture::default();
    let source = base_card(code).unwrap();
    let color = source.color;
    let source = f.add(source, Zone::Board(A, color));
    let zone = if matches!(code, "B047" | "B100") {
        Zone::Score(B)
    } else {
        Zone::Hand(B)
    };
    for &value in values {
        f.tech(Color::Blue, value, zone, vec![]);
    }
    if code == "B100" {
        f.tech(Color::Red, 2, Zone::Achievements(B), vec![]);
    }
    if code == "B067" {
        f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
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
                let mut branch = game.clone();
                branch.choose(index).unwrap();
                leaves(branch, depth + 1)
            })
            .collect(),
        Status::Complete(ReturnValue::Unit) => {
            assert!(game.position.check_conservation());
            vec![game]
        }
        status => panic!("unexpected subset oracle terminal {status:?}"),
    }
}

fn compare_classes<const F: bool>(
    f: Fixture,
    source: CardId,
    reduced_count: usize,
    manual_count: usize,
) {
    let mut reduced = Game::seeded(f.position(), Expansions::with_figures(F), 271).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, B);
    start(&mut manual, source, B);
    let reduced = leaves(reduced, 0);
    let manual = leaves(manual, 0);
    assert_eq!(reduced.len(), reduced_count);
    assert_eq!(manual.len(), manual_count);
    for (left, right) in [(&reduced, &manual), (&manual, &reduced)] {
        for game in left {
            assert!(right.iter().any(
                |other| normalized(game.position.clone()) == normalized(other.position.clone())
            ));
            assert_eq!(game.rng, right[0].rng);
        }
    }
}

#[test]
fn every_quota_subset_survives_refrigeration_databases_and_piracy() {
    fn exercise<const F: bool>() {
        for code in ["B068", "B100", "B047"] {
            // Same-deck return orders stay real; a score pile is unordered.
            let same_back = if code == "B047" { 3 } else { 4 };
            for (values, reduced, manual) in
                [(&[1, 2, 3][..], 3, 6), (&[1, 1, 2][..], same_back, 6)]
            {
                let (f, source) = fixture(code, values);
                compare_classes::<F>(f, source, reduced, manual);
            }
        }
    }
    exercise::<false>();
    exercise::<true>();
}

#[test]
fn explosives_keeps_one_order_into_the_demanders_hand() {
    fn exercise<const F: bool>() {
        let (f, source) = fixture("B067", &[1, 2, 3, 4]);
        compare_classes::<F>(f, source, 1, 1);
        let (f, source) = fixture("B067", &[1, 1, 1]);
        compare_classes::<F>(f, source, 1, 6);
    }
    exercise::<false>();
    exercise::<true>();
    let (mut f, source) = fixture("B067", &[1, 1]);
    f.add(figure_card("F005").unwrap(), Zone::Hand(B));
    compare_classes::<true>(f, source, 1, 6);
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => {
                let metadata = game.position.card(*card);
                Some(((metadata.value, metadata.figure.is_some(), card.0), index))
            }
            _ => None,
        })
        .min_by_key(|&(key, _)| key)
        .unwrap()
        .1
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            status => return status,
        }
    }
    panic!("subset canonical path did not finish");
}

#[test]
fn canonical_semantic_operations_rng_and_every_boundary_clone_match_the_manual_sequence() {
    for code in ["B068", "B100", "B047", "B067"] {
        let (f, source) = fixture(code, &[1, 2, 3]);
        let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 271).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, B);
        start(&mut manual, source, B);
        let mut events = Vec::new();
        let mut manual_events = Vec::new();
        let expected = finish(&mut manual, &mut manual_events);
        for _ in 0..300 {
            let mut fork = reduced.clone();
            assert_eq!(finish(&mut fork, &mut Vec::new()), expected);
            manual.shortcuts_disabled = false;
            assert_eq!(fork, manual);
            manual.shortcuts_disabled = true;
            match reduced.advance(1, &mut events).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(menu) => reduced.choose(canonical(&reduced, &menu)).unwrap(),
                status => {
                    assert_eq!(status, expected);
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
fn relevant_live_hooks_restore_choices_but_suppression_does_not_disable_each_membership() {
    for suppressed in [false, true] {
        let (mut f, source) = fixture("B100", &[1, 2, 3]);
        let figure = figure_card("F069").unwrap();
        let color = figure.color;
        f.add(figure, Zone::Board(B, color));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(B);
        context.demander = A;
        context.set_suppress_karma(suppressed);
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.options.len(), if suppressed { 2 } else { 3 });
    }
    let (mut f, source) = fixture("B068", &[1, 2, 3]);
    let figure = figure_card("F074").unwrap();
    let color = figure.color;
    f.add(figure, Zone::Board(B, color));
    f.tech(Color::Green, 4, Zone::Hand(A), vec![]);
    let mut position = f.position();
    position.begin_turn(B);
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    let mut context = Context::new(B);
    context.demander = A;
    context.set_suppress_karma(true);
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options.len(), 4);
}

#[test]
fn foreign_controller_cannot_distinguish_hidden_front_permutations() {
    let mut menus = Vec::new();
    for swap in [false, true] {
        let (mut f, source) = fixture("B068", &[1, 2, 3]);
        if swap {
            f.cards[1].color = Color::Yellow;
            f.cards[2].symbols[0] = Symbol::Icon(Icon::Health);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, source, A);
        let menu = decision(&mut game, &mut Vec::new());
        assert!(menu.options.iter().all(|option| matches!(
            option,
            Alternative::BlindMove {
                verb: Verb::Return,
                source: Zone::Hand(B),
                ..
            }
        )));
        menus.push(menu);
    }
    assert_eq!(menus[0], menus[1]);
}
