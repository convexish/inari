//! Safe selection, verbs and admission: secrets are ordered public slots,
//! hidden from their owner, with arrival order and capacity preserved.

use inno_engine::*;

mod common;
use common::*;

fn program(f: &mut Fixture, instructions: Vec<Instruction>) -> CardId {
    f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        instructions,
    )
}

/// A two-card red pile splayed up gives both limited zones capacity 2.
fn cap_two(f: &mut Fixture) {
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
}

fn hidden_slots(decision: &Decision, source: Zone) -> Vec<(usize, Deck)> {
    decision
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target:
                    TargetDescriptor::Hidden {
                        source: s,
                        slot,
                        back,
                    },
                ..
            }
            | Alternative::Select {
                target:
                    TargetDescriptor::Hidden {
                        source: s,
                        slot,
                        back,
                    },
            } if *s == source => Some((*slot, *back)),
            _ => None,
        })
        .collect()
}

/// Safeguarding keeps the chooser's arrival order, and a full Safe refuses
/// the rest: a mandatory batch attempts (and leaves) the refused card, an
/// "any number" batch ends as if its player stopped.
#[test]
fn safeguard_batches_keep_arrival_order_and_respect_capacity() {
    for any_number in [false, true] {
        let mut f = Fixture::default();
        cap_two(&mut f);
        let red = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
        let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let green = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
        let step = build::safeguard(build::HAND);
        let code = program(
            &mut f,
            vec![if any_number {
                step.up_to(None)
            } else {
                step.all()
            }],
        );
        let position = f.position();
        let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
        assert_eq!(game.position().safe_limit(A), 5);
        let game_with_splay = game
            .position()
            .clone()
            .with_splay(A, Color::Red, Splay::Up)
            .unwrap();
        game = Game::new(game_with_splay, Expansions::UNSEEN).unwrap();
        assert_eq!(game.position().safe_limit(A), 2);
        game.self_execute(code, Context::new(A)).unwrap();
        let mut events = Vec::new();
        // Every hand card is a visible, separately ordered alternative.
        choose_card(&mut game, &mut events, blue);
        choose_card(&mut game, &mut events, red);
        let status = settle(&mut game, &mut events);
        assert!(matches!(status, Status::Complete(_)), "{status:?}");
        let position = game.position();
        assert_eq!(position.cards(Zone::Safe(A)), &[blue, red]);
        assert_eq!(position.cards(Zone::Hand(A)), &[green]);
        let moved: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Moved(m) => Some((m.verb, m.card, m.source, m.destination)),
                _ => None,
            })
            .collect();
        assert_eq!(
            moved,
            [blue, red]
                .map(|card| (Some(Verb::Safeguard), card, Zone::Hand(A), Zone::Safe(A)))
                .to_vec()
        );
        assert!(position.check_conservation());
    }
}

/// A transfer into a full Safe is refused in place.
#[test]
fn transfers_into_a_full_safe_are_refused_in_place() {
    let mut f = Fixture::default();
    cap_two(&mut f);
    let first = f.tech(Color::Yellow, 4, Zone::Safe(A), vec![]);
    let second = f.tech(Color::Yellow, 5, Zone::Safe(A), vec![]);
    let theirs = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    let code = program(
        &mut f,
        vec![build::transfer(
            Selector::Hand(Who::Opponent),
            Destination::Safe(Who::Executor),
        )],
    );
    let position = f.position().with_splay(A, Color::Red, Splay::Up).unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let status = settle(&mut game, &mut events);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    assert!(!events.iter().any(|e| matches!(e, Event::Moved(_))));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[first, second]);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[theirs]);
}

/// Returning a secret offers one public slot per secret in arrival order,
/// oldest first, with equal backs kept distinct; the chosen slot's card moves.
#[test]
fn secret_choices_are_ordered_public_slots() {
    let mut f = Fixture::default();
    let old = f.tech(Color::Red, 3, Zone::Safe(A), vec![]);
    let blue = f.tech(Color::Blue, 1, Zone::Safe(A), vec![]);
    let green = f.tech(Color::Green, 1, Zone::Safe(A), vec![]);
    let code = program(&mut f, vec![build::return_(Selector::Safe(Who::Executor))]);
    let position = f.position();
    let mut game = Game::new(position, Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let first = decision(&mut game, &mut events);
    assert_eq!(first.player, A);
    assert_eq!(
        hidden_slots(&first, Zone::Safe(A)),
        vec![(0, Deck::base(3)), (1, Deck::base(1)), (2, Deck::base(1)),]
    );
    assert_eq!(first.options.len(), 3);
    // Both observations and the chooser's sampled world accept the slots.
    for viewer in Player::ALL {
        game.observe(viewer).unwrap();
    }
    game.determinize(A, 3).unwrap();
    let index = first
        .options
        .iter()
        .position(|option| {
            matches!(
                option,
                Alternative::Move {
                    target: TargetDescriptor::Hidden { slot: 2, .. },
                    ..
                }
            )
        })
        .unwrap();
    game.choose(index).unwrap();
    let status = settle(&mut game, &mut events);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    assert_eq!(game.position().cards(Zone::Safe(A)), &[old, blue]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), &[green]);
    // Removal shifts the later arrivals forward.
    game.self_execute(code, Context::new(A)).unwrap();
    let second = decision(&mut game, &mut events);
    assert_eq!(
        hidden_slots(&second, Zone::Safe(A)),
        vec![(0, Deck::base(3)), (1, Deck::base(1))]
    );
}

/// The owner remembers a face they saw enter the Safe; the opponent does not
/// learn it, and sampled worlds keep each same-back arrival in its own slot.
#[test]
fn safe_arrivals_are_remembered_by_whoever_saw_the_face() {
    let mut f = Fixture::default();
    let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let yellow = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let older = f.tech(Color::Green, 1, Zone::Safe(A), vec![]);
    let code = program(&mut f, vec![build::safeguard(build::HAND)]);
    let position = f.position();
    let mut game = Game::new(position, Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.self_execute(code, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, blue);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[older, blue]);
    // A's worlds pin the card A watched arrive, and never shuffle it.
    for seed in 0..8 {
        let world = game.determinize(A, seed).unwrap();
        assert_eq!(world.position().cards(Zone::Safe(A)), &[older, blue]);
        assert_eq!(world.position().cards(Zone::Hand(A)), &[yellow]);
    }
    // B's worlds vary the newest secret between A's two hand candidates
    // while the older secret, which B never saw, stays in its slot.
    let mut newest = std::collections::BTreeSet::new();
    for seed in 0..32 {
        let world = game.determinize(B, seed).unwrap();
        let safe = world.position().cards(Zone::Safe(A));
        assert_eq!(safe.len(), 2);
        assert_eq!(world.position().card(safe[0]).origin_deck(), Deck::base(1));
        newest.insert(safe[1]);
        assert!(world.position().check_conservation());
    }
    assert!(
        newest.contains(&blue) && newest.contains(&yellow),
        "{newest:?}"
    );
}
