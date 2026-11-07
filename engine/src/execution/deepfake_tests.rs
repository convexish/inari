//! Original-board memory and Deepfake's nested super-execution boundaries.
use super::shortcuts::common::*;
use super::*;
use crate::program::build::{self, filtered};

const E: Expansions = Expansions::ECHOES;
const EF: Expansions = E.union(Expansions::FIGURES);

fn start(position: Position, expansions: Expansions, card: CardId, own_turn: bool) -> Game {
    let mut game = Game::seeded(position, expansions, 83)
        .unwrap()
        .with_tracking()
        .unwrap();
    if !own_turn {
        game.begin_turn(B).unwrap();
    }
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    game
}

fn deepfake(f: &mut Fixture) -> CardId {
    f.add(echoes_card("E114").unwrap(), Zone::Board(A, Color::Purple))
}

fn selection(menu: &Decision, wanted: CardId) -> usize {
    menu.options.iter().position(|option| matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. }
        | Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == wanted
    )).unwrap_or_else(|| panic!("missing {wanted:?} in {menu:?}"))
}

fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut pick: impl FnMut(&Decision) -> usize,
) -> Status {
    loop {
        let status = game.advance(1000, events).unwrap();
        assert!(game.position.check_conservation());
        for player in Player::ALL {
            assert!(game.tracker(player).unwrap().check_truth(&game.position));
        }
        match status {
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            status => return status,
        }
    }
}

fn transfers(events: &[Event], wanted: CardId) -> Vec<(Zone, Zone)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(movement)
                if movement.card == wanted
                    && movement.verb == Some(Verb::Transfer)
                    && !movement.inside_karma =>
            {
                Some((movement.source, movement.destination))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn deepfake_runs_only_on_its_turn_and_restores_either_original_board() {
    for own_turn in [false, true] {
        for owner in Player::ALL {
            let mut f = Fixture::default();
            let source = deepfake(&mut f);
            let victim = f.tech(Color::Red, 1, Zone::Board(owner, Color::Red), vec![]);
            let copier = f.tech(
                Color::Green,
                2,
                Zone::Board(A, Color::Green),
                vec![build::draw(1)],
            );
            let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
            let mut game = start(f.position(), E, source, own_turn);
            let mut events = Vec::new();
            let mut menus = 0;
            let status = finish(&mut game, &mut events, |menu| {
                assert_eq!(menu.player, A);
                let wanted = if menus == 0 {
                    assert!(menu.options.iter().any(|option| matches!(option, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == source)));
                    victim
                } else {
                    assert!(!menu.options.iter().any(|option| matches!(option, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == source)));
                    copier
                };
                menus += 1;
                selection(menu, wanted)
            });
            assert!(matches!(status, Status::Complete(_)));
            assert_eq!(menus, if own_turn { 2 } else { 0 });
            assert_eq!(
                game.position.locate(victim),
                Some(Zone::Board(owner, Color::Red))
            );
            assert_eq!(
                game.position.locate(drawn),
                Some(if own_turn {
                    Zone::Hand(A)
                } else {
                    Zone::Deck(Deck::base(1))
                })
            );
            assert_eq!(
                transfers(&events, victim),
                if own_turn && owner == B {
                    vec![
                        (Zone::Board(owner, Color::Red), Zone::Board(A, Color::Red)),
                        (Zone::Board(A, Color::Red), Zone::Board(owner, Color::Red)),
                    ]
                } else {
                    vec![]
                }
            );
        }
    }
}

/// A card chosen from either board returns there while it is still a top card; one
/// chosen from Deepfake's own board counts as transferred too.
#[test]
fn deepfake_checks_top_on_any_board_but_does_not_recover_covered_or_scored_cards() {
    for outcome in 0..3 {
        for owner in Player::ALL {
            let mut f = Fixture::default();
            let source = deepfake(&mut f);
            let victim = f.tech(Color::Red, 1, Zone::Board(owner, Color::Red), vec![]);
            let program = match outcome {
                0 => vec![build::transfer(
                    Selector::TopOf(Who::Executor, Color::Red),
                    Destination::Board(Who::Opponent),
                )],
                1 => vec![build::meld(build::HAND)],
                _ => vec![build::score(Selector::TopOf(Who::Executor, Color::Red))],
            };
            let copier = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), program);
            if outcome == 1 {
                f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
            }
            let mut game = start(f.position(), E, source, true);
            let mut events = Vec::new();
            let mut menus = 0;
            finish(&mut game, &mut events, |menu| {
                let wanted = if menus == 0 { victim } else { copier };
                menus += 1;
                selection(menu, wanted)
            });
            let expected = match outcome {
                0 => Zone::Board(owner, Color::Red),
                1 => Zone::Board(A, Color::Red),
                _ => Zone::Score(A),
            };
            assert_eq!(game.position.locate(victim), Some(expected));
            assert_eq!(
                transfers(&events, victim).len(),
                usize::from(owner == B)
                    + usize::from(outcome == 0)
                    + usize::from(outcome == 0 && owner == A)
            );
            if outcome == 1 {
                assert_ne!(game.position.top(A, Color::Red), Some(victim));
            }
        }
    }
}

#[test]
fn deepfake_replaced_transfer_keeps_no_origin_but_still_super_executes() {
    let mut f = Fixture::default();
    let source = deepfake(&mut f);
    let victim = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let copier = f.tech(
        Color::Green,
        2,
        Zone::Board(A, Color::Green),
        vec![build::draw(1)],
    );
    let yi = figure_card(Figure::YiSunSin.code()).unwrap();
    f.add(yi.clone(), Zone::Board(B, yi.color));
    let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let tucked = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = start(f.position(), EF, source, true);
    let mut events = Vec::new();
    let mut menus = 0;
    finish(&mut game, &mut events, |menu| {
        let wanted = if menus == 0 { victim } else { copier };
        menus += 1;
        selection(menu, wanted)
    });
    assert_eq!(
        game.position.locate(victim),
        Some(Zone::Board(B, Color::Blue))
    );
    assert_eq!(game.position.locate(drawn), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(tucked),
        Some(Zone::Board(B, Color::Yellow))
    );
    assert!(transfers(&events, victim).is_empty());
    assert_eq!(events.iter().filter(|event| matches!(event, Event::Moved(m) if m.card == victim && m.inside_karma && m.verb == Some(Verb::Tuck))).count(), 1);
}

#[test]
fn deepfake_nested_copies_keep_separate_records_and_clone_resume_every_boundary() {
    let mut f = Fixture::default();
    let source = deepfake(&mut f);
    let red = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let blue = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
    let copier = f.tech(
        Color::Green,
        3,
        Zone::Board(A, Color::Green),
        vec![
            build::bind(Selector::TopOf(Who::Executor, Color::Purple)),
            crate::build::execute(ExecutionMode::SelfExecute, Who::Executor),
        ],
    );
    for _ in 0..2 {
        f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    }
    let mut game = start(f.position(), E, source, true);
    let targets = [red, copier, blue, blue];
    let mut menus = 0;
    let mut events = Vec::new();
    loop {
        let mut clone = game.clone();
        let mut clone_events = Vec::new();
        let mut step_events = Vec::new();
        let status = game.advance(1, &mut step_events).unwrap();
        assert_eq!(clone.advance(1, &mut clone_events).unwrap(), status);
        assert_eq!(step_events, clone_events);
        assert_eq!(game, clone);
        events.extend(step_events);
        match status {
            Status::WorkLimit => {}
            Status::Decision(menu) => {
                // Sampling during either nested choice preserves public
                // references and the origin column, without pinning new facts.
                let mut world = game.determinize(menu.player, 61).unwrap();
                let mut next = menus;
                let mut sampled_events = Vec::new();
                let end = finish(&mut world, &mut sampled_events, |choice| {
                    let target = targets[next];
                    next += 1;
                    selection(choice, target)
                });
                assert_eq!(
                    world.position.locate(red),
                    Some(Zone::Board(B, Color::Red)),
                    "menu {menus}, status {end:?}, events {sampled_events:?}"
                );
                assert_eq!(
                    world.position.locate(blue),
                    Some(Zone::Board(B, Color::Blue))
                );
                game.choose(selection(&menu, targets[menus])).unwrap();
                menus += 1;
            }
            Status::Complete(_) => break,
            status => panic!("unexpected {status:?}"),
        }
    }
    assert_eq!(menus, 4);
    assert_eq!(game.position.locate(red), Some(Zone::Board(B, Color::Red)));
    assert_eq!(
        game.position.locate(blue),
        Some(Zone::Board(B, Color::Blue))
    );
    assert_eq!(
        transfers(&events, red),
        vec![
            (Zone::Board(B, Color::Red), Zone::Board(A, Color::Red)),
            (Zone::Board(A, Color::Red), Zone::Board(B, Color::Red))
        ]
    );
    assert_eq!(
        transfers(&events, blue),
        vec![
            (Zone::Board(B, Color::Blue), Zone::Board(A, Color::Blue)),
            (Zone::Board(A, Color::Blue), Zone::Board(B, Color::Blue))
        ]
    );
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn deepfake_super_executes_demands_and_terminal_unwind_prevents_the_return() {
    for wins in [false, true] {
        let mut f = Fixture::default();
        let source = deepfake(&mut f);
        let victim = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
        let copier = f.tech(
            Color::Green,
            2,
            Zone::Board(A, Color::Green),
            vec![if wins {
                Instruction::Win(WinRule::Executor)
            } else {
                build::draw(1)
            }],
        );
        f.cards[usize::from(copier.0)].effects[0].kind = EffectKind::Demand;
        let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = start(f.position(), E, source, true);
        let mut events = Vec::new();
        let mut menus = 0;
        let status = finish(&mut game, &mut events, |menu| {
            let wanted = if menus == 0 { victim } else { copier };
            menus += 1;
            selection(menu, wanted)
        });
        if wins {
            assert_eq!(status, Status::Winner(B));
            assert_eq!(
                game.position.locate(victim),
                Some(Zone::Board(A, Color::Red))
            );
            assert_eq!(transfers(&events, victim).len(), 1);
            assert!(game.frames.is_empty());
        } else {
            assert!(matches!(status, Status::Complete(_)));
            assert_eq!(game.position.locate(drawn), Some(Zone::Hand(B)));
            assert_eq!(
                game.position.locate(victim),
                Some(Zone::Board(B, Color::Red))
            );
        }
    }
}

#[test]
fn origin_records_append_latest_and_replace_empty() {
    const S: Slot = Slot::effect(0);
    for append in [false, true] {
        let mut f = Fixture::default();
        let keep = |instruction: Instruction| {
            if append {
                instruction.add(S)
            } else {
                instruction.keep(S)
            }
        };
        let source = f.tech(
            Color::Purple,
            11,
            Zone::Board(A, Color::Purple),
            vec![
                build::transfer(
                    Selector::TopOf(Who::Opponent, Color::Red),
                    Destination::Hand(Who::Executor),
                )
                .keep(S),
                keep(build::transfer(
                    Selector::TopOf(Who::Executor, Color::Blue),
                    Destination::Hand(Who::Executor),
                )),
                build::transfer(
                    filtered(&build::HAND, &[Filter::Has(Test::Is(Reg::Slot(S)))]),
                    Destination::Origin(S),
                ),
            ],
        );
        let first = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
        let last = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        let mut game = start(f.position(), E, source, true);
        finish(&mut game, &mut Vec::new(), |_| panic!("forced transfers"));
        assert_eq!(game.position.locate(first), Some(Zone::Hand(A)));
        assert_eq!(
            game.position.locate(last),
            Some(Zone::Board(A, Color::Blue))
        );
    }
    // An empty keep must erase the previous origin as well as its card.
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Purple,
        11,
        Zone::Board(A, Color::Purple),
        vec![
            build::transfer(
                Selector::TopOf(Who::Opponent, Color::Red),
                Destination::Hand(Who::Executor),
            )
            .keep(S),
            build::transfer(
                Selector::TopOf(Who::Opponent, Color::Blue),
                Destination::Hand(Who::Executor),
            )
            .keep(S),
            build::transfer(build::HAND, Destination::Origin(S)),
        ],
    );
    let card = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let mut game = start(f.position(), E, source, true);
    finish(&mut game, &mut Vec::new(), |_| {
        panic!("empty origin is inapplicable")
    });
    assert_eq!(game.position.locate(card), Some(Zone::Hand(A)));

    // An appended non-board operation is the latest record; it must not
    // inherit an earlier card's origin.
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Purple,
        11,
        Zone::Board(A, Color::Purple),
        vec![
            build::transfer(
                Selector::TopOf(Who::Opponent, Color::Red),
                Destination::Hand(Who::Executor),
            )
            .keep(S),
            build::score(filtered(
                &build::HAND,
                &[Filter::Has(Test::Color(Color::Blue))],
            ))
            .add(S),
            build::transfer(Selector::Reg(Reg::Slot(S)), Destination::Origin(S)),
        ],
    );
    f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let last = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = start(f.position(), E, source, true);
    finish(&mut game, &mut Vec::new(), |_| panic!("forced score"));
    assert_eq!(game.position.locate(last), Some(Zone::Score(A)));
}
