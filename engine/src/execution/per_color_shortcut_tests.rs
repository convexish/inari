use super::common::*;
use crate::*;

fn fixture(code: &str) -> (Fixture, CardId, Context, Vec<CardId>) {
    let mut f = Fixture::default();
    let source_card = base_card(code).unwrap();
    let source_color = source_card.color;
    let source = f.add(source_card, Zone::Board(A, source_color));
    let actor = if matches!(code, "B026" | "B115") {
        B
    } else {
        A
    };
    let mut context = Context::new(actor);
    context.demander = A;
    context.source_owner = A;
    let mut tops = Vec::new();
    for (color, age) in [(Color::Red, 1), (Color::Blue, 1), (Color::Purple, 2)] {
        f.tech(color, age, Zone::Board(actor, color), vec![]);
        let card = f.tech(color, age, Zone::Board(actor, color), vec![]);
        if code == "B026" {
            f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Authority);
        }
        if code == "B115" {
            f.cards[usize::from(card.0)].effects[0].kind = EffectKind::Demand;
        }
        tops.push(card);
    }
    if code == "B058" {
        f.tech(Color::Yellow, 6, Zone::Deck(Deck::base(6)), vec![]);
    }
    if code == "B071" {
        for color in [Color::Red, Color::Green, Color::Blue] {
            f.tech(color, 8, Zone::Deck(Deck::base(8)), vec![]);
        }
    }
    if code == "B115" {
        f.tech(Color::Yellow, 3, Zone::Score(A), vec![]);
        f.tech(Color::Green, 4, Zone::Score(B), vec![]);
    }
    (f, source, context, tops)
}
fn start(game: &mut Game, source: CardId, context: Context) {
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
fn canonical(game: &Game, menu: &Decision, code: &str) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => {
                let card_data = game.position().card(*card);
                let key = if code == "B115" {
                    (card_data.color as u8, 0, card.0)
                } else {
                    let deck = card_data.origin_deck();
                    (deck.age, deck.set as u8, card.0)
                };
                Some((key, index))
            }
            _ => None,
        })
        .min_by_key(|&(key, _)| key)
        .map_or(0, |(_, index)| index)
}
fn finish(game: &mut Game, events: &mut Vec<Event>, code: &str) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu, code)).unwrap(),
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("per-color phase did not finish");
}
fn leaves(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut next = game.clone();
                next.choose(index).unwrap();
                leaves(next, depth + 1)
            })
            .collect(),
        Status::Complete(ReturnValue::Unit) => {
            assert!(game.position().check_conservation());
            vec![game]
        }
        other => panic!("unexpected leaf {other:?}"),
    }
}
// Hands, score piles and junk are unordered: only their contents matter.
fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}
fn outcome_classes<const F: bool>() {
    for (code, manual_count, reduced_count) in [
        ("B026", 6, 1),
        ("B058", 7, 2),
        ("B071", 6, 2),
        ("B115", 6, 1),
    ] {
        let (f, source, context, _) = fixture(code);
        let mut reduced = Game::seeded(f.position(), Expansions::with_figures(F), 129).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, context);
        start(&mut manual, source, context);
        let reduced = leaves(reduced, 0);
        let manual = leaves(manual, 0);
        assert_eq!(reduced.len(), reduced_count, "{code}");
        assert_eq!(manual.len(), manual_count, "{code}");
        let classes: Vec<_> = reduced
            .iter()
            .map(|game| normalized(game.position().clone()))
            .collect();
        for game in &manual {
            assert!(
                classes.contains(&normalized(game.position().clone())),
                "{code}"
            );
            assert_eq!(game.rng, reduced[0].rng);
        }
        for game in reduced {
            assert!(
                manual
                    .iter()
                    .any(|candidate| normalized(candidate.position().clone())
                        == normalized(game.position().clone())),
                "{code}"
            );
        }
    }
}

#[test]
fn all_per_color_outcomes_survive_and_deck_back_orders_remain_real() {
    outcome_classes::<false>();
    outcome_classes::<true>();
}

#[test]
fn canonical_events_rng_and_every_work_boundary_clone_match_unrestricted_execution() {
    for code in ["B026", "B058", "B071", "B115"] {
        let (f, source, context, _) = fixture(code);
        let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 129).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, context);
        start(&mut manual, source, context);
        let mut events = Vec::new();
        loop {
            let mut clone = reduced.clone();
            let mut one = Vec::new();
            let mut two = Vec::new();
            let status = reduced.advance(1, &mut one).unwrap();
            assert_eq!(clone.advance(1, &mut two).unwrap(), status);
            assert_eq!(clone, reduced);
            assert_eq!(one, two);
            events.extend(one);
            match status {
                Status::WorkLimit => {}
                Status::Decision(menu) => reduced.choose(canonical(&reduced, &menu, code)).unwrap(),
                Status::Complete(ReturnValue::Unit) => break,
                other => panic!("unexpected status {other:?}"),
            }
        }
        let mut expected = Vec::new();
        assert_eq!(
            finish(&mut manual, &mut expected, code),
            Status::Complete(ReturnValue::Unit)
        );
        manual.shortcuts_disabled = false;
        assert_eq!(reduced, manual, "{code}");
        assert_eq!(events, expected, "{code}");
    }
}

fn exposing_maxwell(suppressed: bool) -> (Game, CardId, Context, Vec<CardId>) {
    let (mut f, source, mut context, tops) = fixture("B071");
    // This is a real future exposure, not a mutation injected between resumes.
    let old_bottom = f
        .placements
        .iter()
        .position(|(_, zone)| *zone == Zone::Board(A, Color::Blue))
        .unwrap();
    f.placements[old_bottom].1 = Zone::Hand(B);
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let top_index = f
        .placements
        .iter()
        .position(|(card, _)| *card == tops[1])
        .unwrap();
    let last = f.placements.len() - 1;
    f.placements.swap(top_index, last);
    for color in [Color::Yellow, Color::Purple] {
        f.tech(color, 8, Zone::Deck(Deck::base(8)), vec![]);
    }
    context.set_suppress_karma(suppressed);
    (
        Game::new(f.position(), Expansions::FIGURES).unwrap(),
        source,
        context,
        tops,
    )
}

#[test]
fn a_future_relevant_figure_exposure_blocks_before_any_order_is_discarded() {
    let (mut game, source, context, tops) = exposing_maxwell(false);
    start(&mut game, source, context);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.iter().any(|option| matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == tops[2])));
    let index = menu.options.iter().position(|option| matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == tops[1])).unwrap();
    game.choose(index).unwrap();
    assert_eq!(
        finish(&mut game, &mut events, "B071"),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Moved(m) if m.inside_karma && m.verb.is_none()))
            .count(),
        2
    );
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 5);
    assert!(game.position().has_figure(A, Figure::JamesClerkMaxwell));
}

#[test]
fn suppression_restores_the_proof_even_with_a_relevant_future_exposure() {
    let (mut reduced, source, context, _) = exposing_maxwell(true);
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, context);
    start(&mut manual, source, context);
    let reduced = leaves(reduced, 0);
    let manual = leaves(manual, 0);
    assert_eq!(reduced.len(), 2);
    assert_eq!(manual.len(), 6);
    for game in manual {
        assert!(
            reduced
                .iter()
                .any(|candidate| candidate.position() == game.position())
        );
        assert_eq!(game.position().cards(Zone::Hand(A)).len(), 3);
    }
}

#[test]
fn public_top_proofs_retain_outcomes_under_foreign_control() {
    for code in ["B026", "B071", "B115"] {
        let (f, source, mut context, _) = fixture(code);
        context.controller = context.executor.opponent();
        let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, context);
        start(&mut manual, source, context);
        let reduced = leaves(reduced, 0);
        let manual = leaves(manual, 0);
        assert_eq!(reduced.len(), if code == "B071" { 2 } else { 1 });
        assert_eq!(manual.len(), 6);
        for game in manual {
            assert!(
                reduced
                    .iter()
                    .any(|candidate| normalized(candidate.position().clone())
                        == normalized(game.position().clone()))
            );
        }
    }
}

#[test]
fn independent_draw_and_dogma_clauses_do_not_disable_public_top_reductions() {
    for (code, expected) in [("B026", 1), ("B058", 2), ("B071", 2), ("B115", 1)] {
        let (mut f, source, context, _) = fixture(code);
        let other = context.executor.opponent();
        f.figure(Figure::FuXi, Zone::Board(other, Color::Green));
        if code == "B071" {
            let buried = f
                .placements
                .iter()
                .find(|(_, zone)| *zone == Zone::Board(A, Color::Blue))
                .unwrap()
                .0;
            f.cards[usize::from(buried.0)] = figure_card("F012").unwrap();
        } else {
            f.figure(Figure::HuangDi, Zone::Board(other, Color::Blue));
        }
        let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, context);
        start(&mut manual, source, context);
        let reduced = leaves(reduced, 0);
        let manual = leaves(manual, 0);
        assert_eq!(reduced.len(), expected, "{code}");
        assert_eq!(manual.len(), if code == "B058" { 7 } else { 6 }, "{code}");
        for game in manual {
            assert!(
                reduced
                    .iter()
                    .any(|candidate| normalized(candidate.position().clone())
                        == normalized(game.position().clone()))
            );
        }
    }
}
