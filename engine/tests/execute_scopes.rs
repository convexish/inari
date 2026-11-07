//! Execute's scoped modifiers: must, a controller override
//! with inspection, one effect by index and the activity result.

use inno_engine::*;

mod common;
use common::*;

fn run(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("execution did not terminate");
}

fn has_decline(decision: &Decision) -> bool {
    decision.options.contains(&Alternative::Decline)
}

/// A program on A's purple top that executes A's top card of `color`.
fn caller(f: &mut Fixture, color: Color, execute: Instruction) -> CardId {
    f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![build::bind(Selector::TopOf(Who::Executor, color)), execute],
    )
}

const SELF: ExecutionMode = ExecutionMode::SelfExecute;

/// Each self-execution keyword earns a Chain draw of an [11].
fn elevens(f: &mut Fixture, count: usize) {
    for i in 0..count {
        f.tech(Color::ALL[i % 5], 11, Zone::Deck(Deck::base(11)), vec![]);
    }
}

/// Blackmail's must: Tools' commitment cannot be declined, Pottery's "may return up to
/// three" must return at least one, and Road Building's "one or two", with no "may",
/// still stops after one.
#[test]
fn must_removes_may_but_keeps_quantity_stops() {
    for must in [false, true] {
        let modified = |i: Instruction| if must { i.must() } else { i };
        // Tools: "you may return three cards from your hand".
        let mut f = Fixture::default();
        elevens(&mut f, 1);
        f.add(base_card("B011").unwrap(), Zone::Board(A, Color::Blue));
        for color in [Color::Red, Color::Green, Color::Yellow] {
            f.tech(color, 1, Zone::Hand(A), vec![]);
        }
        let code = caller(
            &mut f,
            Color::Blue,
            modified(build::execute(SELF, Who::Executor)),
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let first = decision(&mut game, &mut events);
        assert_eq!(has_decline(&first), !must, "Tools, must={must}");

        // Pottery: "you may return up to three cards": at least one, then a
        // quantity stop.
        let mut f = Fixture::default();
        elevens(&mut f, 1);
        f.add(base_card("B010").unwrap(), Zone::Board(A, Color::Blue));
        f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
        let code = caller(
            &mut f,
            Color::Blue,
            modified(build::execute(SELF, Who::Executor)),
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let first = decision(&mut game, &mut events);
        assert_eq!(has_decline(&first), !must, "Pottery, must={must}");
        if must {
            game.choose(0).unwrap();
            let second = decision(&mut game, &mut events);
            assert!(has_decline(&second), "Pottery stops after one");
        }

        // Road Building: "meld one or two" keeps its second-meld stop.
        let mut f = Fixture::default();
        elevens(&mut f, 1);
        f.add(base_card("B017").unwrap(), Zone::Board(A, Color::Red));
        let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let green = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
        let code = caller(
            &mut f,
            Color::Red,
            modified(build::execute(SELF, Who::Executor)),
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let first = decision(&mut game, &mut events);
        assert!(!has_decline(&first));
        game.choose(0).unwrap();
        let status = settle(&mut game, &mut events);
        let Status::Decision(second) = status else {
            panic!("expected the second meld's stop, must={must}");
        };
        assert!(has_decline(&second));
        game.choose(second.options.len() - 1).unwrap();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(game.position().cards(Zone::Hand(A)).len(), 1);
        assert!(
            game.position().top(A, Color::Blue) == Some(blue)
                || game.position().top(A, Color::Green) == Some(green)
        );
    }
}

/// Hitchhiking's controller decides inside the execution and in nested
/// executions; with inspection they see the executor's hand, learn it for
/// good, and lose the permission when the execution ends.
#[test]
fn a_controller_override_decides_nested_choices_and_may_inspect() {
    for inspect in [false, true] {
        let mut f = Fixture::default();
        elevens(&mut f, 2);
        // Blue top: meld a hand card. Green top: self-execute the blue top.
        f.tech(
            Color::Blue,
            1,
            Zone::Board(A, Color::Blue),
            vec![build::meld(build::HAND)],
        );
        f.tech(
            Color::Green,
            1,
            Zone::Board(A, Color::Green),
            vec![
                build::bind(Selector::TopOf(Who::Executor, Color::Blue)),
                build::execute(SELF, Who::Executor),
            ],
        );
        // Two backs: a blind chooser still has two groups to pick from. A
        // face-down [2] in the deck keeps the leftover [2] uncertain unless
        // the controller inspected the hand.
        let red = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
        f.tech(Color::Green, 3, Zone::Hand(A), vec![]);
        f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
        let execute = build::execute(SELF, Who::Executor).controlled_by(Who::Opponent);
        let execute = if inspect {
            execute.inspecting()
        } else {
            execute
        };
        let code = caller(&mut f, Color::Green, execute);
        let mut game = Game::new(f.position(), Expansions::BASE)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let choice = decision(&mut game, &mut events);
        assert_eq!(choice.player, B, "inspect={inspect}");
        let known = choice
            .options
            .iter()
            .filter(|option| {
                matches!(
                    option,
                    Alternative::Move {
                        target: TargetDescriptor::Known { .. },
                        ..
                    }
                )
            })
            .count();
        let blind = choice
            .options
            .iter()
            .filter(|option| matches!(option, Alternative::BlindMove { .. }))
            .count();
        assert_eq!((known, blind), if inspect { (3, 0) } else { (0, 2) });
        let sees = |game: &Game, card| {
            matches!(
                game.position().descriptor(target(card, Zone::Hand(A)), B),
                TargetDescriptor::Known { .. }
            )
        };
        assert_eq!(sees(&game, red), inspect);
        game.choose(0).unwrap();
        assert!(matches!(run(&mut game, &mut events), Status::Complete(_)));
        // Permission ends with the execution; knowledge does not.
        let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
        hand.sort();
        assert_eq!(hand.len(), 2);
        assert!(!sees(&game, hand[0]));
        let mut pinned = true;
        for seed in 0..16 {
            let world = game.determinize(B, seed).unwrap();
            let mut sampled = world.position().cards(Zone::Hand(A)).to_vec();
            sampled.sort();
            pinned &= sampled == hand;
        }
        assert_eq!(pinned, inspect, "inspect={inspect}");
    }
}

/// One non-demand effect by index runs alone, an absent index runs nothing,
/// and reveals count as activity only when asked.
#[test]
fn one_effect_by_index_reports_its_activity() {
    for (wanted, ignore_reveals, drawn, active) in [
        (0, false, Some(1), true),
        (1, false, Some(2), true),
        (2, false, None, true),
        (2, true, None, false),
        (3, false, None, false),
    ] {
        let mut f = Fixture::default();
        let one = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
        let two = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
        f.tech(Color::Yellow, 3, Zone::Hand(A), vec![]);
        let card = f.tech(
            Color::Green,
            1,
            Zone::Board(A, Color::Green),
            vec![build::draw(1)],
        );
        let effects = &mut f.cards[usize::from(card.0)].effects;
        effects.push(Effect {
            kind: EffectKind::Demand,
            instructions: vec![build::draw(3)],
        });
        effects.push(Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![build::draw(2)],
        });
        effects.push(Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![build::reveal(build::HAND)],
        });
        let execute = build::execute(ExecutionMode::Effect(wanted), Who::Executor);
        let execute = if ignore_reveals {
            execute.ignore_reveals()
        } else {
            execute
        };
        let code = f.tech(
            Color::Purple,
            1,
            Zone::Board(A, Color::Purple),
            vec![
                build::bind(Selector::TopOf(Who::Executor, Color::Green)),
                execute,
                build::if_(build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let status = run(&mut game, &mut events);
        let label = format!("effect {wanted}, ignore_reveals={ignore_reveals}");
        assert_eq!(matches!(status, Status::Winner(A)), active, "{label}");
        let expected = match drawn {
            Some(1) => Some(one),
            Some(2) => Some(two),
            _ => None,
        };
        let hand = game.position().cards(Zone::Hand(A));
        assert_eq!(hand.len(), 1 + usize::from(expected.is_some()), "{label}");
        if let Some(card) = expected {
            assert!(hand.contains(&card), "{label}");
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Revealed { .. }))
                .count(),
            usize::from(wanted == 2),
            "{label}"
        );
    }
}

/// Hitchhiking's donor performs and chooses the transfer from their own
/// hand to the executor's board; "you" still means the executor.
#[test]
fn a_performer_runs_and_chooses_a_single_move_for_the_executor() {
    let mut f = Fixture::default();
    let red = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let code = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::transfer(
                Selector::Hand(Who::Opponent),
                Destination::Board(Who::Executor),
            )
            .by(Who::Opponent),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let choice = decision(&mut game, &mut events);
    assert_eq!(choice.player, B);
    let known: Vec<_> = choice
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                verb: Verb::Transfer,
                target: TargetDescriptor::Known { card, .. },
            } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(known, vec![red, blue]);
    game.choose(1).unwrap();
    assert!(matches!(run(&mut game, &mut events), Status::Complete(_)));
    let moved = moved(&events[0]);
    assert_eq!(
        (moved.executor, moved.card, moved.source, moved.destination),
        (B, blue, Zone::Hand(B), Zone::Board(A, Color::Blue))
    );
    assert_eq!(game.position().top(A, Color::Blue), Some(blue));
}

/// A Figure reaction inside a controlled execution has its choice made by the
/// controller too.
#[test]
fn a_reaction_inside_a_controlled_execution_is_decided_by_the_controller() {
    let mut f = Fixture::default();
    elevens(&mut f, 1);
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 3, Zone::AvailableAchievements, vec![]);
    f.tech(Color::Yellow, 4, Zone::AvailableAchievements, vec![]);
    f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![build::meld(build::HAND)],
    );
    f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let code = caller(
        &mut f,
        Color::Blue,
        build::execute(SELF, Who::Executor).controlled_by(Who::Opponent),
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    // The single hand card melds without a question; Galileo's First
    // reaction then asks which available achievement to junk.
    let choice = decision(&mut game, &mut events);
    assert_eq!(choice.player, B);
    assert!(choice.options.iter().all(|option| matches!(
        option,
        Alternative::BlindMove {
            verb: Verb::Junk,
            source: Zone::AvailableAchievements,
            ..
        }
    )));
    assert_eq!(choice.options.len(), 2);
    game.choose(0).unwrap();
    assert!(matches!(run(&mut game, &mut events), Status::Complete(_)));
    assert_eq!(game.position().cards(Zone::Junk).len(), 1);
}
