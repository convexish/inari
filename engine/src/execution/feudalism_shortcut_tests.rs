use super::common::*;
use super::*;

fn fixture() -> (Position, CardId) {
    let mut f = Fixture::default();
    let source = f.add(base_card("B035").unwrap(), Zone::Board(A, Color::Purple));
    f.add(base_card("B001").unwrap(), Zone::Hand(B));
    (f.position(), source)
}

fn context() -> Context {
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    context
}

fn start(game: &mut Game, source: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context(),
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut decisions = 0;
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                assert!(
                    menu.options
                        .iter()
                        .all(|o| matches!(o, Alternative::JunkSpecial(_)))
                );
                decisions += 1;
                game.choose(0).unwrap();
            }
            status => return (status, decisions),
        }
    }
    panic!("special sweep did not finish");
}

fn leaves(mut game: Game, depth: usize) -> Vec<(Game, Status)> {
    assert!(depth < 6);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut child = game.clone();
                child.choose(index).unwrap();
                leaves(child, depth + 1)
            })
            .collect(),
        status => {
            game.shortcuts_disabled = false;
            assert!(game.position.check_conservation());
            vec![(game, status)]
        }
    }
}

fn all_orders<const F: bool>(available: usize) {
    let (mut position, source) = fixture();
    for &special in &Special::ALL[available..] {
        position.specials[special as usize] = SpecialLocation::Owned(A);
    }
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 47).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source);
    start(&mut manual, source);
    let reduced = leaves(reduced, 0);
    let manual = leaves(manual, 0);
    assert_eq!(reduced.len(), 1);
    assert_eq!(manual.len(), (1..=available).product::<usize>());
    // Unlike card junk piles, special locations have no physical ordering to
    // quotient out: every complete native machine/RNG state is exactly equal.
    assert!(manual.iter().all(|state| reduced.contains(state)));
}

#[test]
fn every_special_order_has_one_exact_outcome_with_base_and_figures() {
    for available in 0..=5 {
        all_orders::<false>(available);
        all_orders::<true>(available);
    }
}

#[test]
fn canonical_events_rng_and_every_work_boundary_clone_match_the_manual_sequence() {
    let (position, source) = fixture();
    let mut reduced = Game::seeded(position, Expansions::FIGURES, 193).unwrap();
    let initial_rng = reduced.rng.clone();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source);
    start(&mut manual, source);
    let mut expected = reduced.clone();
    let mut expected_events = Vec::new();
    assert_eq!(
        finish(&mut expected, &mut expected_events),
        (Status::Complete(ReturnValue::Unit), 0)
    );
    let mut manual_events = Vec::new();
    assert_eq!(
        finish(&mut manual, &mut manual_events),
        (Status::Complete(ReturnValue::Unit), 4)
    );
    assert_eq!(expected_events, manual_events);
    manual.shortcuts_disabled = false;
    assert_eq!(manual, expected);
    assert_eq!(expected.rng, initial_rng);
    assert_eq!(
        expected_events
            .iter()
            .filter_map(|event| match event {
                Event::SpecialMoved {
                    special,
                    source: SpecialLocation::Available,
                    destination: SpecialLocation::Junk,
                    executor: B,
                    inside_karma: false,
                } => Some(*special),
                _ => None,
            })
            .collect::<Vec<_>>(),
        Special::ALL
    );

    let mut prefix = Vec::new();
    for _ in 0..200 {
        let mut fork = reduced.clone();
        let mut suffix = Vec::new();
        assert_eq!(
            finish(&mut fork, &mut suffix).0,
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(fork, expected);
        let mut joined = prefix.clone();
        joined.extend(suffix);
        assert_eq!(joined, expected_events);
        match reduced.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Complete(ReturnValue::Unit) => {
                assert_eq!(reduced, expected);
                return;
            }
            other => panic!("unexpected reduced status: {other:?}"),
        }
    }
    panic!("special sweep exceeded work boundary bound");
}

#[test]
fn live_supply_is_requeried_and_public_token_proof_is_independent_of_controller() {
    let (position, source) = fixture();
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    start(&mut game, source);
    let mut events = Vec::new();
    for _ in 0..100 {
        if matches!(game.frames.last(), Some(Frame::JunkAvailableSpecials { considered, .. }) if *considered == 1)
            && game.position.special_location(Special::Monument) == SpecialLocation::Junk
        {
            // Isolated transition witness, not an invented Figure hook.
            // Foreign control cannot distinguish orders of these public tokens.
            let Some(Frame::JunkAvailableSpecials { context, .. }) = game.frames.last_mut() else {
                unreachable!()
            };
            context.controller = A;
            game.position.specials[Special::World as usize] = SpecialLocation::Owned(A);
            assert_eq!(
                finish(&mut game, &mut events),
                (Status::Complete(ReturnValue::Unit), 0)
            );
            assert!(!events.iter().any(|e| matches!(
                e,
                Event::SpecialMoved {
                    special: Special::World,
                    destination: SpecialLocation::Junk,
                    ..
                }
            )));
            assert_eq!(
                game.position.special_location(Special::World),
                SpecialLocation::Owned(A)
            );
            return;
        }
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    panic!("missing special-operation boundary");
}

#[test]
fn unrelated_native_junk_and_claim_figures_do_not_block_supply_token_reduction() {
    let mut f = Fixture::default();
    let source = f.add(base_card("B035").unwrap(), Zone::Board(A, Color::Purple));
    f.add(base_card("B001").unwrap(), Zone::Hand(B));
    f.figure(Figure::Homer, Zone::Board(B, Color::Purple));
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, source);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        (Status::Complete(ReturnValue::Unit), 0)
    );
    assert_eq!(events.len(), 6);
    assert!(
        Special::ALL
            .into_iter()
            .all(|s| game.position.special_location(s) == SpecialLocation::Junk)
    );
}
