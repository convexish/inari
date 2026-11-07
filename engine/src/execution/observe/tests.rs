//! Structured continuation privacy at actual scheduler pauses.
use super::super::shortcuts::common::{A, B, Fixture, decision};
use super::*;
use crate::observation::context::{Field as F, RecordKind as K};
use crate::program::build::*;

const EFFECT: Slot = Slot::effect(0);
const EXECUTION: Slot = Slot::execution(0);
const ACTION: Slot = Slot::action(0);
const PRIVATE: Selector = filtered(&HAND, &[Filter::Has(Test::Color(Color::Red))]);
const IN_PRIVATE_COLORS: Selector = filtered(
    &HAND,
    &[Filter::Has(Test::ColorIn(ColorSet::Slot(EXECUTION)))],
);
const STOP: Instruction = choose(Who::Opponent, Choice::Value(ValueChoices::Boxed(&[1, 2])));

fn fixture(instructions: Vec<Instruction>, swapped: bool) -> (Game, CardId, [CardId; 2]) {
    fixture_effects(
        vec![Effect {
            kind: EffectKind::NonDemand,
            instructions,
        }],
        swapped,
    )
}
fn fixture_effects(effects: Vec<Effect>, swapped: bool) -> (Game, CardId, [CardId; 2]) {
    let mut f = Fixture::default();
    let mut source = base_card("B012").unwrap();
    source.effects = effects;
    let source = f.add(source, Zone::Board(A, Color::Blue));
    let hidden =
        ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
    f.placements
        .iter_mut()
        .find(|(card, _)| *card == hidden[usize::from(swapped)])
        .unwrap()
        .1 = Zone::Hand(A);
    // Prevent sharing: only the turn player's invocation may reach the pause.
    f.cards[source.0 as usize].symbols = [Symbol::Icon(Icon::Concept); SLOTS];
    let mut game = Game::seeded(f.position(), Expansions::BASE, 19)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    (game, source, hidden)
}
fn rows(game: &Game, viewer: Player) -> Vec<context::ContextRecord> {
    game.context_records(viewer)
}
fn field(row: &context::ContextRecord, name: F) -> Option<u64> {
    row.fields
        .iter()
        .find_map(|&(f, value)| (f == name).then_some(value))
}
fn slots(game: &Game, viewer: Player, life: Life) -> Vec<context::ContextRecord> {
    rows(game, viewer)
        .into_iter()
        .filter(|row| {
            row.kind == K::Slot
                && field(row, F::Life) == Some(life as u64)
                && field(row, F::Slot) == Some(0)
        })
        .collect()
}
fn pause(game: &mut Game) {
    let d = decision(game, &mut Vec::new());
    assert_eq!(
        d.options,
        vec![Alternative::Value(1), Alternative::Value(2)]
    );
}

#[test]
fn private_scalar_empty_and_present_values_are_hidden_in_each_lifetime() {
    for slot in [EFFECT, EXECUTION, ACTION] {
        let make = |swapped| {
            fixture(
                vec![
                    Instruction::Bind(Binding::Value(slot, Expr::Max(&PRIVATE))),
                    STOP,
                ],
                swapped,
            )
            .0
        };
        let mut a = make(false);
        let mut b = make(true);
        pause(&mut a);
        pause(&mut b);
        let real_a = slots(&a, A, slot.life());
        let real_b = slots(&b, A, slot.life());
        assert_eq!(real_a.len(), 1);
        assert_eq!(real_b.len(), 1);
        assert_ne!(
            real_a, real_b,
            "private empty and Value(1) must be distinct for the owner"
        );
        let hidden_a = rows(&a, B);
        let hidden_b = rows(&b, B);
        assert_eq!(hidden_a, hidden_b);
        assert_eq!(hidden_a.len(), 1);
        assert_eq!(hidden_a[0].kind, K::Opaque);
        assert_eq!(a.observe(B).unwrap(), b.observe(B).unwrap());
        for seed in 0..4 {
            for game in [&a, &b] {
                assert!(matches!(
                    game.determinize(B, seed),
                    Err(Error::Unsupported(
                        "a retained scalar depends on private information"
                    ))
                ));
                assert!(matches!(
                    game.sample_world(B, seed),
                    Err(Error::Unsupported(
                        "a retained scalar depends on private information"
                    ))
                ));
            }
        }
        assert_eq!(a.clone().observe(B).unwrap(), a.observe(B).unwrap());
    }
}

#[test]
fn private_colors_propagate_to_bound_slots_and_frozen_block_items() {
    let instructions = vec![
        Instruction::Bind(Binding::Colors(EFFECT, ColorSet::OfCards(&HAND))),
        Instruction::Bind(Binding::Colors(EXECUTION, ColorSet::Slot(EFFECT))),
        Instruction::Bind(Binding::Value(ACTION, Expr::Count(&IN_PRIVATE_COLORS))),
        Instruction::Block(Block {
            len: 1,
            domain: Domain::Colors(ColorSet::Slot(EFFECT)),
            order: Order::Printed,
            item: Some(Slot::effect(1)),
            shortcut: None,
        }),
        STOP,
    ];
    let mut games = [
        fixture(instructions.clone(), false).0,
        fixture(instructions, true).0,
    ];
    for game in &mut games {
        pause(game);
        assert!(matches!(
            game.determinize(B, 3),
            Err(Error::Unsupported(
                "a retained scalar depends on private information"
            ))
        ));
    }
    assert_ne!(games[0].observe(A).unwrap(), games[1].observe(A).unwrap());
    assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    let public = rows(&games[0], B);
    assert_eq!(public.len(), 1);
    assert_eq!(public[0].kind, K::Opaque);
    assert_eq!(field(&public[0], F::Known), Some(0));
    assert!(rows(&games[0], A).iter().any(|row| row.kind == K::Block));
}

#[test]
fn public_replacement_and_repeat_clear_scalar_provenance_without_touching_kept_slots() {
    for slot in [EFFECT, EXECUTION, ACTION] {
        let mut game = fixture(
            vec![
                Instruction::Bind(Binding::Colors(slot, ColorSet::OfCards(&HAND))),
                STOP,
            ],
            false,
        )
        .0;
        pause(&mut game);
        let choice = game.frames.pop().unwrap();
        let program = game.frames.pop().unwrap();
        let Frame::Program { context, .. } = program else {
            panic!("program")
        };
        let regs = Regs {
            locals: slot.life() == Life::Effect,
            ..Regs::EMPTY
        };
        let keep = Keep {
            slot,
            append: false,
        };
        game.store(keep, Some(slots::Held::Value(7)), context)
            .unwrap();
        assert_eq!(game.slot_scalar_known(slot, context, regs), 3);
        game.store_scalar(keep, None, context, 1).unwrap();
        game.store_scalar(
            Keep {
                append: true,
                ..keep
            },
            None,
            context,
            3,
        )
        .unwrap();
        assert_eq!(
            game.slot_scalar_known(slot, context, regs),
            1,
            "failed add preserves provenance in {slot:?}"
        );
        if slot.life() == Life::Effect {
            game.own_locals().scalar_known[1] = 1;
            game.restart_locals(EFFECT.bit());
            assert_eq!(game.own_locals().scalar_known[..2], [1, 3]);
        }
        game.store(keep, None, context).unwrap();
        assert_eq!(
            game.slot_scalar_known(slot, context, regs),
            3,
            "public keep clears private undefined value in {slot:?}"
        );
        game.frames.push(program);
        game.frames.push(choice);
        assert!(game.determinize(B, 2).is_ok());
    }
}

#[test]
fn private_count_stays_hidden_through_an_opposing_first_program() {
    fn make(swapped: bool) -> Game {
        let mut f = Fixture::default();
        let mut source = base_card("B012").unwrap();
        source.effects = vec![Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![
                draws(Expr::Add(&Expr::Count(&PRIVATE), 1), 1, Then::Keep).set(CardSet::Figures),
            ],
        }];
        source.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
        let source = f.add(source, Zone::Board(A, Color::Blue));
        let hidden =
            ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
        f.placements
            .iter_mut()
            .find(|(card, _)| *card == hidden[usize::from(swapped)])
            .unwrap()
            .1 = Zone::Hand(A);
        let figure = figure_card(Figure::WernherVonBraun.code()).unwrap();
        f.add(figure.clone(), Zone::Board(B, figure.color));
        for code in ["F001", "F002", "F003"] {
            f.add(figure_card(code).unwrap(), Zone::Deck(Deck::figures(1)));
        }
        for (code, age) in [("B086", 9), ("B102", 10)] {
            f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(age)));
        }
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 19)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        game
    }
    let mut games = [make(false), make(true)];
    for game in &mut games {
        let menu = decision(game, &mut Vec::new());
        assert_eq!(menu.player, B);
        assert_eq!(menu.options.len(), 2);
        assert!(
            menu.options
                .iter()
                .all(|option| matches!(option, Alternative::Value(9 | 10)))
        );
        assert!(matches!(
            game.sample_world(B, 7),
            Err(Error::Unsupported(
                "a retained scalar depends on private information"
            ))
        ));
        assert_eq!(
            game.frames
                .iter()
                .filter(|frame| matches!(frame,Frame::Locals(locals) if locals.progress_known==1))
                .count(),
            1
        );
        let records = rows(game, B);
        assert!(
            records
                .iter()
                .any(|row| row.kind == K::Opaque && field(row, F::Known) == Some(0))
        );
    }
    assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    for game in &mut games {
        game.choose(0).unwrap();
        let mut events = Vec::new();
        loop {
            match game.advance(1, &mut events).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(menu) => {
                    assert_eq!(menu.player, B);
                    assert!(matches!(
                        game.determinize(B, 11),
                        Err(Error::Unsupported(
                            "a retained scalar depends on private information"
                        ))
                    ));
                    game.choose(0).unwrap();
                }
                Status::Complete(_) => break,
                status => panic!("unexpected {status:?}"),
            }
        }
        assert!(events.iter().any(
            |event| matches!(event,Event::Moved(m) if m.inside_karma && m.verb==Some(Verb::Junk))
        ));
        assert!(game.frames.is_empty());
        assert!(game.determinize(A, 2).is_ok());
        assert!(game.determinize(B, 2).is_ok());
    }
}

#[test]
fn pending_keep_and_add_preserve_committed_order_origins_and_scope() {
    for append in [false, true] {
        let mut f = Fixture::default();
        let mut source = base_card("B012").unwrap();
        // A public target leaves its original board, then waits at a native
        // choice before committing a new packet into the same slot.
        let move_one = transfer(
            Selector::TopOf(Who::Opponent, Color::Red),
            Destination::Hand(Who::Executor),
        )
        .keep(ACTION);
        let instruction = if append {
            choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))).add(ACTION)
        } else {
            choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))).keep(ACTION)
        };
        source.effects = vec![Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![move_one, instruction],
        }];
        source.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
        let source = f.add(source, Zone::Board(A, Color::Blue));
        let moved = f.add(base_card("B002").unwrap(), Zone::Board(B, Color::Red));
        let mut game = Game::seeded(f.position(), Expansions::BASE, 5)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.player, A);
        assert_eq!(game.position.locate(moved), Some(Zone::Hand(A)));
        let observed = rows(&game, B);
        let committed = observed
            .iter()
            .position(|row| row.kind == K::Slot && field(row, F::Life) == Some(Life::Action as u64))
            .unwrap()
            + 1;
        let record = observed
            .iter()
            .find(|row| row.kind == K::Record && row.parent == committed)
            .unwrap();
        assert_eq!(record.sight, Some(Sight::Card(moved)));
        assert_eq!(field(record, F::Ordinal), Some(0));
        assert_eq!(
            field(record, F::Origin),
            Some(1),
            "original opponent board now belongs to viewer B"
        );
        let pending = observed.iter().find(|row| row.kind == K::Pending).unwrap();
        assert_eq!(field(pending, F::Append), Some(u64::from(append)));
        assert_eq!(field(pending, F::Choice), Some(1));
        assert_eq!(field(pending, F::Life), Some(Life::Action as u64));
        assert_eq!(
            game.determinize(A, 8).unwrap().observe(A).unwrap(),
            game.observe(A).unwrap()
        );
        // The in-flight update has not replaced the committed card yet.
        game.choose(1).unwrap();
        assert!(matches!(
            super::super::shortcuts::common::settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
    }
}

#[test]
fn private_control_uses_one_opaque_record_for_different_branch_and_child_shapes() {
    const NO_RED: Cond = Cond::Compare(Expr::Count(&PRIVATE), Cmp::Equal, Expr::Const(0));
    for instructions in [
        vec![if_(&NO_RED, 1), STOP, STOP],
        vec![
            Instruction::Block(Block {
                len: 1,
                domain: Domain::Times(Expr::Count(&PRIVATE)),
                order: Order::Printed,
                item: None,
                shortcut: None,
            }),
            STOP,
            STOP,
        ],
        vec![bind(PRIVATE), STOP],
        vec![
            Instruction::Bind(Binding::Value(EFFECT, Expr::Max(&PRIVATE))),
            if_(
                &Cond::Card(
                    Reg::Source,
                    Filter::Has(Test::Value(Cmp::Equal, Expr::Slot(EFFECT))),
                ),
                1,
            ),
            STOP,
            STOP,
        ],
    ] {
        let mut games = [
            fixture(instructions.clone(), false).0,
            fixture(instructions, true).0,
        ];
        for game in &mut games {
            pause(game);
            let projection = rows(game, B);
            assert_eq!(projection.len(), 1);
            assert_eq!(projection[0].kind, K::Opaque);
            assert_eq!(field(&projection[0], F::Known), Some(0));
            assert!(matches!(
                game.sample_world(B, 9),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
        }
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
        for game in &mut games {
            loop {
                game.choose(0).unwrap();
                match super::super::shortcuts::common::settle(game, &mut Vec::new()) {
                    Status::Decision(_) => {}
                    Status::Complete(_) => break,
                    status => panic!("{status:?}"),
                }
            }
            assert!(rows(game, B).is_empty());
            assert!(game.determinize(B, 1).is_ok());
        }
    }
    let mut public = fixture(
        vec![
            if_(
                &Cond::Compare(Expr::Count(&HAND), Cmp::Equal, Expr::Const(1)),
                1,
            ),
            STOP,
            STOP,
        ],
        false,
    )
    .0;
    pause(&mut public);
    assert!(rows(&public, B).iter().any(|row| row.kind == K::Effect));
    assert!(public.determinize(B, 3).is_ok());
}

#[test]
fn private_skipped_memory_producers_stay_opaque_after_the_effect_retires() {
    const NO_RED: Cond = Cond::Compare(Expr::Count(&PRIVATE), Cmp::Equal, Expr::Const(0));
    for slot in [EXECUTION, ACTION] {
        let effects = vec![
            Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![
                    if_(&NO_RED, 1),
                    Instruction::Bind(Binding::Value(slot, Expr::Const(7))),
                ],
            },
            Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![STOP],
            },
        ];
        let mut games = [
            fixture_effects(effects.clone(), false).0,
            fixture_effects(effects, true).0,
        ];
        for game in &mut games {
            pause(game);
            assert!(
                !game
                    .frames
                    .iter()
                    .any(|frame| matches!(frame,Frame::Locals(locals) if locals.flow_known!=3))
            );
            let projection = rows(game, B);
            assert_eq!(projection.len(), 1);
            assert_eq!(projection[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 6),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
        }
        assert_ne!(
            slots(&games[0], A, slot.life()),
            slots(&games[1], A, slot.life())
        );
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn private_branches_restrict_only_memories_their_program_can_change() {
    const NO_RED: Cond = Cond::Compare(Expr::Count(&PRIVATE), Cmp::Equal, Expr::Const(0));
    let effects = |branch: Instruction| {
        vec![
            Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![if_(&NO_RED, 1), branch],
            },
            Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![STOP],
            },
        ]
    };
    // An effect slot retires with its effect, so the branch leaves nothing
    // private behind (Cell Phone's private tuck loop, shared by both players).
    let effect_slot = Instruction::Bind(Binding::Value(EFFECT, Expr::Const(7)));
    let mut games = [false, true].map(|swapped| fixture_effects(effects(effect_slot), swapped).0);
    for game in &mut games {
        pause(game);
        assert!(game.sample_world(B, 6).is_ok());
    }
    assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    // A nested execution could record into either memory.
    let execute = crate::build::execute(ExecutionMode::SelfExecute, Who::Executor);
    for swapped in [false, true] {
        let mut game = fixture_effects(effects(execute), swapped).0;
        pause(&mut game);
        assert!(matches!(
            game.sample_world(B, 6),
            Err(Error::Unsupported(
                "a retained scalar depends on private information"
            ))
        ));
    }
}

#[test]
fn forecast_beliefs_are_nonvacuous_and_promotion_search_uses_native_choices() {
    use crate::observation::{Group, scalar};
    use crate::search::SearchConfig;
    use crate::session::Step;
    let mut f = Fixture::default();
    let soap = f.add(echoes_card("E006").unwrap(), Zone::Forecast(A));
    let comb = f.add(echoes_card("E007").unwrap(), Zone::Forecast(A));
    let foreign = f.add(echoes_card("E008").unwrap(), Zone::Forecast(B));
    f.add(
        echoes_card("E009").unwrap(),
        Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 1,
        }),
    );
    let action = f.add(base_card("B011").unwrap(), Zone::Hand(A));
    for code in ["B001", "B002", "B004"] {
        f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1)));
    }
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 51)
        .unwrap()
        .with_tracking()
        .unwrap();
    let view = game.observe(A).unwrap();
    let mine: Vec<_> = view
        .tokens
        .iter()
        .filter(|t| t.group == Group::MyForecast)
        .collect();
    let theirs: Vec<_> = view
        .tokens
        .iter()
        .filter(|t| t.group == Group::TheirForecast)
        .collect();
    assert_eq!(mine.len(), 2);
    assert!(mine.iter().all(|t| t.weight == 1.0));
    assert_eq!(theirs.len(), 2);
    assert!(theirs.iter().all(|t| t.weight == 0.5));
    let base = scalar::PLAYERS;
    assert_eq!(view.scalars[base + scalar::FORECAST_LIMIT], 0.5);
    assert_eq!(view.scalars[base + scalar::FORECAST_ROOM], 0.3);
    let mut different = false;
    for seed in 0..12 {
        let world = game.determinize(A, seed).unwrap();
        assert_eq!(world.observe(A).unwrap(), view);
        different |= world.position.locate(foreign) != Some(Zone::Forecast(B));
    }
    assert!(different, "sampling really changes the foreign Forecast");
    game.take_action(Action::Meld { card: action }, ActionKind::Regular)
        .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.iter().all(|option| matches!(
        option,
        Alternative::Move {
            verb: Verb::Meld,
            target: TargetDescriptor::Known {
                source: Zone::Forecast(A),
                ..
            }
        }
    )));
    let before = game.clone();
    let result = game
        .search(SearchConfig {
            iterations: 12,
            rollout_actions: 0,
            seed: 4,
            ..SearchConfig::default()
        })
        .unwrap();
    assert_eq!(game, before);
    assert_eq!(result.moves.len(), 2);
    assert_ne!(
        result.moves[0].step, result.moves[1].step,
        "native alternatives retain distinct search keys"
    );
    assert!(
        result
            .moves
            .iter()
            .all(|m| m.visits > 0 && matches!(m.step, Step::Choice(0 | 1)))
    );
    assert_eq!(result.moves.iter().map(|m| m.visits).sum::<u32>(), 12);
    let promotion = rows(&game, A)
        .into_iter()
        .find(|row| row.kind == K::Promotion)
        .unwrap();
    assert_eq!(promotion.sight, Some(Sight::Card(action)));
    let selected=menu.options.iter().position(|option|matches!(option,Alternative::Move{target:TargetDescriptor::Known{card,..},..} if *card==soap)).unwrap();
    game.choose(selected).unwrap();
    let colors = decision(&mut game, &mut Vec::new());
    assert_eq!(colors.options.len(), 5);
    let continuation = rows(&game, A);
    assert!(
        continuation
            .iter()
            .any(|row| row.kind == K::Effect && field(row, F::Foreseen) == Some(1))
    );
    assert!(
        continuation
            .iter()
            .any(|row| row.kind == K::Action && field(row, F::Promoted) == Some(1))
    );
    assert_eq!(game.position.locate(comb), Some(Zone::Forecast(A)));
    for seed in 0..8 {
        assert_eq!(
            game.determinize(A, seed).unwrap().observe(A).unwrap(),
            game.observe(A).unwrap()
        );
    }
}

#[test]
fn robocar_search_keeps_opposing_hand_and_forecast_fronts_private() {
    use crate::search::SearchConfig;
    use crate::session::Step;
    let mut f = Fixture::default();
    let source = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
    for code in ["B001", "B002", "B004"] {
        f.add(base_card(code).unwrap(), Zone::Hand(A));
    }
    f.add(base_card("B017").unwrap(), Zone::Hand(A));
    let forecast = f.add(echoes_card("E001").unwrap(), Zone::Forecast(A));
    f.add(
        echoes_card("E002").unwrap(),
        Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 1,
        }),
    );
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 25)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.begin_turn(B).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, B);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.iter().all(|option| matches!(
        option,
        Alternative::BlindSelect {
            source: Zone::Hand(A),
            ..
        }
    )));
    let before = game.clone();
    let view = game.observe(B).unwrap();
    let mut moved = false;
    for seed in 0..8 {
        let world = game.determinize(B, seed).unwrap();
        assert_eq!(world.observe(B).unwrap(), view);
        moved |= world.position.locate(forecast) != Some(Zone::Forecast(A));
    }
    assert!(moved);
    let result = game
        .search(SearchConfig {
            iterations: 10,
            rollout_actions: 0,
            seed: 41,
            ..SearchConfig::default()
        })
        .unwrap();
    assert_eq!(game, before);
    assert_eq!(result.moves.len(), 2);
    assert_ne!(
        result.moves[0].step, result.moves[1].step,
        "native alternatives retain distinct search keys"
    );
    assert!(
        result
            .moves
            .iter()
            .all(|m| m.visits > 0 && matches!(m.step, Step::Choice(0 | 1)))
    );
}

#[test]
fn checked_claim_operand_stays_anonymous_in_context_through_real_first_reactions() {
    let mut f = Fixture::default();
    let heart = f.add(echoes_card("E102").unwrap(), Zone::Board(A, Color::Blue));
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    f.figure(Figure::ZhangHeng, Zone::Board(A, Color::Blue));
    for code in ["B087", "B092", "B101"] {
        f.add(base_card(code).unwrap(), Zone::Score(A));
    }
    let first = f.add(base_card("B001").unwrap(), Zone::AvailableAchievements);
    f.add(base_card("B017").unwrap(), Zone::AvailableAchievements);
    f.add(base_card("B004").unwrap(), Zone::Deck(Deck::base(1)));
    f.add(base_card("B018").unwrap(), Zone::Deck(Deck::base(2)));
    let mut game = Game::seeded(
        f.position(),
        Expansions::ECHOES.union(Expansions::FIGURES),
        7,
    )
    .unwrap()
    .with_tracking()
    .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: heart,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    let pick=menu.options.iter().position(|option|matches!(option,Alternative::BlindMove{verb:Verb::Achieve,back,..} if *back==Deck::base(1)) || matches!(option,Alternative::Move{verb:Verb::Achieve,target:TargetDescriptor::Hidden{back,..}} if *back==Deck::base(1))).unwrap();
    game.choose(pick).unwrap();
    let first_menu = decision(&mut game, &mut Vec::new());
    assert_eq!(first_menu.player, A);
    assert_eq!(first_menu.options.len(), 2);
    assert!(
        first_menu
            .options
            .iter()
            .all(|option| matches!(option, Alternative::Karma { .. }))
    );
    let observation = game.observe(A).unwrap();
    let operand = rows(&game, A)
        .into_iter()
        .find(|row| {
            row.kind == K::Operation && field(row, F::Verb) == Some(Verb::Achieve as u64 + 1)
        })
        .unwrap();
    assert_eq!(operand.sight, Some(Sight::Back(Deck::base(1))));
    let mut changed = false;
    for seed in 0..12 {
        let world = game.determinize(A, seed).unwrap();
        assert_eq!(world.observe(A).unwrap(), observation);
        changed |= world.position.locate(first) != Some(Zone::AvailableAchievements);
    }
    assert!(changed);
}

#[test]
fn telescope_projects_exchange_counts_and_selected_backs_without_incoming_faces() {
    let mut f = Fixture::default();
    let source = f.add(echoes_card("E043").unwrap(), Zone::Board(A, Color::Blue));
    let outgoing =
        ["E001", "E002", "E003"].map(|code| f.add(echoes_card(code).unwrap(), Zone::Forecast(A)));
    for code in ["B001", "B017", "B026"] {
        f.add(base_card(code).unwrap(), Zone::AvailableAchievements);
    }
    for (code, age) in [("B004", 1), ("B018", 2), ("B027", 3)] {
        f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(age)));
    }
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 3)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    let place=menu.options.iter().position(|option|matches!(option,Alternative::Move{verb:Verb::Place,target:TargetDescriptor::Known{card,..}} if *card==outgoing[0])).unwrap();
    game.choose(place).unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options.len(), 2);
    game.choose(0).unwrap();
    let next = decision(&mut game, &mut Vec::new());
    assert_eq!(next.options.len(), 2);
    let projection = rows(&game, A);
    let exchange = projection
        .iter()
        .position(|row| row.kind == K::Exchange)
        .unwrap()
        + 1;
    let sides: Vec<_> = projection
        .iter()
        .enumerate()
        .filter(|(_, row)| row.kind == K::Group && row.parent == exchange)
        .collect();
    assert_eq!(sides.len(), 2);
    assert_eq!(field(sides[1].1, F::Limit), Some(3));
    let incoming: Vec<_> = projection
        .iter()
        .filter(|row| row.parent == sides[1].0 + 1)
        .collect();
    assert_eq!(incoming.len(), 1);
    assert!(matches!(incoming[0].sight, Some(Sight::Back(_))));
    let before = game.observe(A).unwrap();
    for seed in 0..12 {
        assert_eq!(
            game.determinize(A, seed).unwrap().observe(A).unwrap(),
            before
        );
    }
    let foreign = rows(&game, B);
    assert!(
        foreign
            .iter()
            .filter(|row| row.kind == K::Record && field(row, F::SourceZone).is_some())
            .all(|row| matches!(row.sight, Some(Sight::Back(_))))
    );
}

#[test]
fn structured_tokens_preserve_repeated_records_and_full_width_progress() {
    use crate::observation::{Group, context as wire};
    let mut game = fixture(
        vec![reveal(HAND).keep(EFFECT), reveal(HAND).add(EFFECT), STOP],
        false,
    )
    .0;
    pause(&mut game);
    for frame in &mut game.frames {
        if let Frame::ActionReturns { democracy, .. } = frame {
            *democracy = [u32::MAX, u32::MAX - 1];
        }
    }
    let records = rows(&game, B);
    let slot_id = records
        .iter()
        .position(|r| {
            r.kind == K::Slot
                && field(r, F::Life) == Some(Life::Effect as u64)
                && field(r, F::Slot) == Some(0)
        })
        .unwrap()
        + 1;
    let cards: Vec<_> = records
        .iter()
        .filter(|row| row.kind == K::Record && row.parent == slot_id)
        .collect();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].sight, cards[1].sight);
    assert_eq!(field(cards[0], F::Ordinal), Some(0));
    assert_eq!(field(cards[1], F::Ordinal), Some(1));
    let observation = game.observe(B).unwrap();
    let tokens: Vec<_> = observation
        .tokens
        .iter()
        .filter(|t| t.group == Group::Context)
        .collect();
    let mut index = 0;
    for record in records {
        let header = tokens[index];
        index += 1;
        assert_eq!(header.code, wire::HEADER + record.kind as u32);
        let mut fields = std::collections::BTreeMap::<u32, u64>::new();
        while index < tokens.len() && tokens[index].code >= wire::FIELD {
            let token = tokens[index];
            index += 1;
            let code = token.code - wire::FIELD;
            let limb = code % 4;
            let key = code / 4;
            *fields.entry(key).or_default() |= (token.value as u64) << (16 * limb);
        }
        assert_eq!(fields[&(F::Parent as u32)], record.parent as u64);
        for (name, value) in record.fields {
            assert_eq!(fields[&(name as u32)], value);
        }
    }
    assert_eq!(index, tokens.len());
    assert_eq!(observation.row()[1], crate::observation::REVISION as f32);
}

#[test]
fn private_requested_age_retains_exactness_but_performed_count_is_public() {
    const REQUEST: Expr = Expr::Add(&Expr::Count(&PRIVATE), 1);
    for branch in [false, true] {
        let mut games = Vec::new();
        for swapped in [false, true] {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let mut instructions = vec![
                draw_age(REQUEST),
                Instruction::Bind(Binding::Value(EFFECT, Expr::Done)),
            ];
            if branch {
                instructions.extend([if_(DID, 1), STOP]);
            }
            instructions.extend([STOP, reveal(LAST), STOP]);
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions,
            }];
            source.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            // No Base age-one supply: both private requested ages resolve to
            // exactly the same public age-two draw, but only one is exact.
            let hidden =
                ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Forecast(A)));
            f.placements
                .iter_mut()
                .find(|(card, _)| *card == hidden[usize::from(swapped)])
                .unwrap()
                .1 = Zone::Hand(A);
            let drawn = f.add(base_card("B017").unwrap(), Zone::Deck(Deck::base(2)));
            let mut game = Game::seeded(f.position(), Expansions::ECHOES, 31)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.take_action(
                Action::Dogma {
                    card: source,
                    auspice: false,
                },
                ActionKind::Regular,
            )
            .unwrap();
            pause(&mut game);
            assert_eq!(game.position.locate(drawn), Some(Zone::Hand(A)));
            let locals = game
                .frames
                .iter()
                .find_map(|frame| {
                    if let Frame::Locals(locals) = frame {
                        Some(locals)
                    } else {
                        None
                    }
                })
                .unwrap();
            assert_eq!(
                locals.scalar_known[0], 3,
                "Done is a public performed count"
            );
            assert_eq!(locals.last_exact_known, 1);
            assert!(matches!(locals.slots[0], Some(slots::Held::Value(1))));
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 3),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
        if !branch {
            for game in &mut games {
                game.choose(0).unwrap();
                pause(game);
                assert!(rows(game, B).iter().any(|row| row.kind == K::Effect));
                assert!(game.determinize(B, 3).is_ok());
            }
        }
    }
}

#[test]
fn finite_and_per_value_quotas_stay_private_at_real_transfer_reactions() {
    const QUOTA: PerValue = PerValue {
        source: ValueSource::Cards(&PRIVATE),
        multiplicity: Multiplicity::Every,
    };
    for per_value in [false, true] {
        let mut games = Vec::new();
        for swapped in [false, true] {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let movement = transfer(Selector::Reg(Reg::Source), Destination::Hand(Who::Opponent));
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![if per_value {
                    movement.per_value(&QUOTA)
                } else {
                    movement.exactly(Expr::Count(&PRIVATE))
                }],
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            f.add(base_card("B001").unwrap(), Zone::Hand(A));
            let hidden = ["B002", "B004"]
                .map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
            f.placements
                .iter_mut()
                .find(|(card, _)| *card == hidden[usize::from(swapped)])
                .unwrap()
                .1 = Zone::Hand(A);
            f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
            f.figure(Figure::AlfredNobel, Zone::Board(B, Color::Green));
            let mut game = Game::seeded(f.position(), Expansions::FIGURES, 8)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.begin_turn(B).unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let menu = decision(&mut game, &mut Vec::new());
            assert_eq!(menu.player, B);
            assert_eq!(menu.options.len(), 2);
            assert!(
                menu.options
                    .iter()
                    .all(|option| matches!(option, Alternative::Karma { .. }))
            );
            assert!(
                rows(&game, B)
                    .iter()
                    .any(|row| row.kind == K::Opaque && field(row, F::Known) == Some(0))
            );
            assert!(matches!(
                game.sample_world(B, 2),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn pending_special_and_exchange_counts_hide_foreign_private_queries() {
    for exchange in [false, true] {
        let mut games = Vec::new();
        for swapped in [false, true] {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let instruction = if exchange {
                Instruction::Exchange(ExchangeStep {
                    left: HAND,
                    right: Selector::AvailableAchievements,
                    counts: &[None, Some(Expr::Add(&Expr::Count(&PRIVATE), 1))],
                    optional: false,
                })
            } else {
                Instruction::Special(SpecialStep::JunkAvailable {
                    count: &Expr::Add(&Expr::Count(&PRIVATE), 1),
                    shortcut: None,
                })
            };
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![instruction],
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            let hidden = ["B002", "B004"]
                .map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
            f.placements
                .iter_mut()
                .find(|(card, _)| *card == hidden[usize::from(swapped)])
                .unwrap()
                .1 = Zone::Hand(A);
            for code in ["B017", "B026", "B038"] {
                f.add(base_card(code).unwrap(), Zone::AvailableAchievements);
            }
            let mut game = Game::seeded(f.position(), Expansions::BASE, 8)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let menu = decision(&mut game, &mut Vec::new());
            assert_eq!(menu.player, A);
            assert!(
                rows(&game, B)
                    .iter()
                    .any(|row| row.kind == K::Opaque && field(row, F::Known) == Some(0))
            );
            games.push(game);
        }
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn refused_private_sweep_hides_failed_count_without_revealing_any_card() {
    let mut games = Vec::new();
    for swapped in [false, true] {
        let mut f = Fixture::default();
        let mut source = base_card("B012").unwrap();
        source.effects = vec![Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![foreshadow(PRIVATE).all().verify(), STOP],
        }];
        source.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
        let source = f.add(source, Zone::Board(A, Color::Blue));
        let hidden =
            ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
        f.placements
            .iter_mut()
            .find(|(card, _)| *card == hidden[usize::from(swapped)])
            .unwrap()
            .1 = Zone::Hand(A);
        for code in ["E001", "E002", "E003", "E004", "E005"] {
            f.add(echoes_card(code).unwrap(), Zone::Forecast(A));
        }
        let mut game = Game::seeded(f.position(), Expansions::ECHOES, 0)
            .unwrap()
            .with_tracking()
            .unwrap();
        assert_eq!(
            game.position.cards(Zone::Forecast(A)).len(),
            game.position.forecast_limit(A)
        );
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Revealed { .. } | Event::Moved(_)))
        );
        assert_eq!(rows(&game, B).len(), 1);
        assert_eq!(rows(&game, B)[0].kind, K::Opaque);
        assert!(matches!(
            game.determinize(B, 4),
            Err(Error::Unsupported(
                "a retained scalar depends on private information"
            ))
        ));
        games.push(game);
    }
    assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    assert_ne!(
        rows(&games[0], A),
        rows(&games[1], A),
        "executor knows whether a matching card was refused"
    );
}

#[test]
fn shared_barometer_public_witness_allows_search_and_keeps_other_returns_private() {
    let mut games = Vec::new();
    for swapped in [false, true] {
        let mut f = Fixture::default();
        let mut source = echoes_card("E038").unwrap();
        // Isolate the printed second dogma while retaining the ordinary shared
        // scheduler. A public blue witness suffices; the other return is private.
        source.effects = vec![source.effects[1].clone()];
        source.echo = None;
        source.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
        source.featured = Some(Icon::Concept);
        let color = source.color;
        let source = f.add(source, Zone::Board(A, color));
        let mut share = base_card("B012").unwrap();
        share.symbols = [Symbol::Icon(Icon::Concept); SLOTS];
        f.add(share, Zone::Board(B, Color::Blue));
        let blue = f.add(base_card("B011").unwrap(), Zone::Forecast(B));
        let hidden =
            ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
        f.placements
            .iter_mut()
            .find(|(card, _)| *card == hidden[usize::from(swapped)])
            .unwrap()
            .1 = Zone::Forecast(B);
        for code in ["E001", "E002"] {
            f.add(echoes_card(code).unwrap(), Zone::Forecast(A));
        }
        let mut game = Game::seeded(f.position(), Expansions::ECHOES, 3)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        let blue_choice=menu.options.iter().position(|option|matches!(option,Alternative::Move{target:TargetDescriptor::Known{card,..},..} if *card==blue)).unwrap();
        game.choose(blue_choice).unwrap();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, A);
        assert_eq!(menu.options.len(), 3);
        assert_eq!(
            events
                .iter()
                .filter_map(|event| if let Event::Revealed { card, .. } = event {
                    Some(*card)
                } else {
                    None
                })
                .collect::<Vec<_>>(),
            [blue]
        );
        assert!(rows(&game, A).iter().all(|row| row.kind != K::Opaque));
        let before = game.clone();
        for seed in 0..8 {
            let world = game.determinize(A, seed).unwrap();
            assert_eq!(world.observe(A).unwrap(), game.observe(A).unwrap());
        }
        assert_eq!(
            game.search(SearchConfig {
                iterations: 4,
                rollout_actions: 0,
                seed: 7,
                ..SearchConfig::default()
            })
            .unwrap()
            .iterations,
            4,
        );
        assert_eq!(
            game, before,
            "search does not consume game RNG or choose an action"
        );
        games.push(game);
    }
    assert_eq!(games[0].observe(A).unwrap(), games[1].observe(A).unwrap());
}

#[test]
fn hidden_direct_effect_source_remains_private_after_its_action_record_producer_retires() {
    let mut games = Vec::new();
    for swapped in [false, true] {
        let mut f = Fixture::default();
        let cards = ["B011", "B012"].map(|code| {
            let mut card = base_card(code).unwrap();
            card.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![
                    Instruction::Bind(Binding::Value(ACTION, Expr::Const(7))),
                    STOP,
                ],
            }];
            f.add(card, Zone::Forecast(A))
        });
        let mut public = base_card("B015").unwrap();
        public.effects = vec![Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![STOP],
        }];
        let color = public.color;
        let public = f.add(public, Zone::Board(A, color));
        let source = cards[usize::from(swapped)];
        f.placements
            .iter_mut()
            .find(|(card, _)| *card == source)
            .unwrap()
            .1 = Zone::Hand(A);
        let mut game = Game::seeded(f.position(), Expansions::ECHOES, 1)
            .unwrap()
            .with_tracking()
            .unwrap();
        // Self/super keywords disclose their source; the public direct-effect
        // probe API can deliberately execute an inspected private card.
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        pause(&mut game);
        assert_eq!(rows(&game, B).len(), 1);
        assert_eq!(rows(&game, B)[0].kind, K::Opaque);
        assert!(
            rows(&game, A)
                .iter()
                .any(|row| row.kind == K::Slot && row.sight == Some(Sight::Card(source)))
        );
        game.choose(0).unwrap();
        for _ in 0..10 {
            if matches!(
                game.frames.as_slice(),
                [Frame::ActionReturns {
                    memory: Some(_),
                    ..
                }]
            ) {
                break;
            }
            assert!(matches!(
                game.advance(1, &mut Vec::new()).unwrap(),
                Status::WorkLimit
            ));
        }
        assert!(matches!(
            game.frames.as_slice(),
            [Frame::ActionReturns {
                memory: Some(_),
                ..
            }]
        ));
        assert_eq!(rows(&game, B).len(), 1);
        assert_eq!(rows(&game, B)[0].kind, K::Opaque);
        game.frames.push(Frame::operation(
            Operation::ExecuteEffect {
                card: public,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        ));
        pause(&mut game);
        let sampled = game.sample_world(B, 2);
        assert!(
            matches!(
                sampled,
                Err(Error::Unsupported(tracking::remap::HIDDEN_SOURCE))
            ),
            "{:?}",
            sampled.as_ref().err()
        );

        games.push(game);
    }
    assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
}

#[test]
fn hidden_per_color_and_group_progress_stays_opaque_before_reveal() {
    for group in [false, true] {
        let mut games = Vec::new();
        for selected in [0, 1] {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let instruction = if group {
                return_(HAND).group(2, GroupConstraint::SameValueDifferentColors)
            } else {
                meld(HAND).per_color(None)
            };
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![instruction],
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            let hand =
                ["B002", "B011", "B009"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let figure = figure_card("F087").unwrap();
            let color = figure.color;
            f.add(figure, Zone::Board(B, color));
            let victim = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
            f.cards[victim.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
            let mut game = Game::seeded(f.position(), Expansions::FIGURES, 1)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, A);
            let index=menu.options.iter().position(|option|matches!(option,Alternative::Move{target:TargetDescriptor::Known{card,..},..}|Alternative::Select{target:TargetDescriptor::Known{card,..}} if *card==hand[selected])).unwrap();
            game.choose(index).unwrap();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, if group { A } else { B });
            if !group {
                assert_eq!(game.pending_melds(B)[0].card, CardView::Back(Deck::base(1)));
                assert_eq!(menu.options.len(), 2);
            }
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Event::Revealed { .. } | Event::Moved(_)))
            );
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            let sampled = game.sample_world(B, 4);
            if group {
                assert!(matches!(
                    sampled,
                    Err(Error::Unsupported(
                        "sample a decision for its acting observer"
                    ))
                ));
                assert_eq!(
                    game.determinize(A, 4).unwrap().observe(A).unwrap(),
                    game.observe(A).unwrap()
                );
            } else {
                assert!(
                    matches!(
                        sampled,
                        Err(Error::Unsupported(
                            "a retained scalar depends on private information"
                        ))
                    ),
                    "{:?}",
                    sampled.as_ref().err()
                );
            }
            games.push(game);
        }
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn anonymous_retained_refs_do_not_gain_front_or_alias_proof_from_current_locations() {
    const RED_RECORD: Selector = filtered(
        &Selector::Slot(EFFECT),
        &[Filter::Has(Test::Color(Color::Red))],
    );
    const UNION: Selector = Selector::Union(&[Selector::Slot(EFFECT), TOPS]);
    for case in 0..3 {
        let mut games = Vec::new();
        for original in 0..2 {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let mut instructions = vec![choose(Who::Executor, Choice::Card(&HAND)).keep(EFFECT)];
            if case == 2 {
                instructions.push(choose(Who::Executor, Choice::Card(&HAND)).add(EFFECT));
            } else {
                instructions.push(meld(HAND));
            }
            let selector = match case {
                0 => &RED_RECORD,
                1 => &UNION,
                _ => &Selector::Slot(EFFECT),
            };
            instructions.extend([
                Instruction::Bind(Binding::Value(ACTION, Expr::Count(selector))),
                STOP,
            ]);
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions,
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            let hand = ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let mut game = Game::seeded(f.position(), Expansions::BASE, 2)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            for card in [hand[original], hand[0]] {
                let menu = decision(&mut game, &mut events);
                assert_eq!(menu.player, A);
                let selected=menu.options.iter().position(|option|matches!(option,Alternative::Select{target:TargetDescriptor::Known{card:known,..}}|Alternative::Move{target:TargetDescriptor::Known{card:known,..},..} if *known==card)).unwrap();
                game.choose(selected).unwrap();
            }
            pause(&mut game);
            let slot = slots(&game, A, Life::Action);
            assert_eq!(slot.len(), 1);
            assert_eq!(
                field(&slot[0], F::Scalar),
                Some(match case {
                    0 => u64::from(original == 0),
                    1 => 2 + original as u64,
                    _ => 1 + original as u64,
                })
            );
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 7),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].position, games[1].position);
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn location_queries_on_anonymous_retained_refs_need_their_naming() {
    // Each query reads where the retained card is now. Whether the anonymous
    // pick is the card that was independently melded is private to A.
    const INSPECTABLE: Selector = filtered(
        &Selector::Slot(EFFECT),
        &[Filter::Has(Test::Inspectable(Who::Opponent))],
    );
    const SHOWS: Selector = filtered(
        &Selector::Slot(EFFECT),
        &[Filter::Has(Test::StackShows(Icon::Authority))],
    );
    const BENEATH: Selector = Selector::Board(
        Who::Executor,
        ColorSet::Only(&Color::ALL),
        BoardRegion::BeneathBound,
    );
    const ON_BOARD: Cond = Cond::Card(
        Reg::Slot(EFFECT),
        Filter::Has(Test::Inspectable(Who::Opponent)),
    );
    for case in 0..4 {
        let mut games = Vec::new();
        for original in 0..2 {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let mut instructions = vec![
                choose(Who::Executor, Choice::Card(&HAND)).keep(EFFECT),
                meld(HAND),
            ];
            match case {
                0 => instructions.push(Instruction::Bind(Binding::Value(
                    ACTION,
                    Expr::Count(&INSPECTABLE),
                ))),
                1 => instructions.push(Instruction::Bind(Binding::Value(
                    ACTION,
                    Expr::Count(&SHOWS),
                ))),
                2 => instructions.extend([
                    bind(Selector::Slot(EFFECT)),
                    Instruction::Bind(Binding::Value(ACTION, Expr::Count(&BENEATH))),
                ]),
                _ => instructions.extend([if_(&ON_BOARD, 1), STOP]),
            }
            instructions.push(STOP);
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions,
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            // A red card for the melded red card to cover.
            f.add(base_card("B001").unwrap(), Zone::Board(A, Color::Red));
            let hand = ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let mut game = Game::seeded(f.position(), Expansions::BASE, 2)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            // Keep either card, then meld the red B002 in both worlds.
            for card in [hand[original], hand[0]] {
                let menu = decision(&mut game, &mut events);
                assert_eq!(menu.player, A);
                let selected=menu.options.iter().position(|option|matches!(option,Alternative::Select{target:TargetDescriptor::Known{card:known,..}}|Alternative::Move{target:TargetDescriptor::Known{card:known,..},..} if *known==card)).unwrap();
                game.choose(selected).unwrap();
            }
            pause(&mut game);
            if case < 3 {
                let slot = slots(&game, A, Life::Action);
                assert_eq!(slot.len(), 1);
                // Nonvacuous: the real count differs between the worlds.
                assert_eq!(field(&slot[0], F::Scalar), Some(u64::from(original == 0)));
            }
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 7),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].position, games[1].position);
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn identity_narrowed_and_union_counts_need_known_membership() {
    // The narrowed selection and the union's deduplication depend on where
    // the anonymous kept card is. After A privately returns or junks one of
    // two same-back hand cards, B cannot tell whether that was the kept one.
    const KEPT_IN_HAND: Selector = filtered(&HAND, &[Filter::Has(Test::InSlot(EFFECT))]);
    const KEPT_OR_JUNK: Selector = Selector::Union(&[Selector::Slot(EFFECT), Selector::Junk]);
    for union in [false, true] {
        let mut games = Vec::new();
        for original in 0..2 {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![
                    choose(Who::Executor, Choice::Card(&HAND)).keep(EFFECT),
                    if union { junk(HAND) } else { return_(HAND) },
                    Instruction::Bind(Binding::Value(
                        ACTION,
                        Expr::Count(if union { &KEPT_OR_JUNK } else { &KEPT_IN_HAND }),
                    )),
                    STOP,
                ],
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            let hand = ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let mut game = Game::seeded(f.position(), Expansions::BASE, 2)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            // Keep either card, then move B002 away in both worlds.
            for card in [hand[original], hand[0]] {
                let menu = decision(&mut game, &mut events);
                assert_eq!(menu.player, A);
                let selected=menu.options.iter().position(|option|matches!(option,Alternative::Select{target:TargetDescriptor::Known{card:known,..}}|Alternative::Move{target:TargetDescriptor::Known{card:known,..},..} if *known==card)).unwrap();
                game.choose(selected).unwrap();
            }
            pause(&mut game);
            let slot = slots(&game, A, Life::Action);
            assert_eq!(slot.len(), 1);
            // Nonvacuous: the real count differs between the worlds.
            assert_eq!(
                field(&slot[0], F::Scalar),
                Some(u64::from(original == 1) + u64::from(union))
            );
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 7),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].position, games[1].position);
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn narrowed_verdicts_need_named_fronts_places_and_whole_aliases() {
    // B knows where the anonymous kept card is, but not: its color (case 0);
    // which hand holds it after A privately passes one of two cards (case 1);
    // or whether it is the top that Cai Lun makes an achievement, after both
    // red cards are melded (case 2).
    const RED_KEPT: Selector = filtered(
        &HAND,
        &[
            Filter::Has(Test::InSlot(EFFECT)),
            Filter::Has(Test::Color(Color::Red)),
        ],
    );
    const KEPT_INSPECTABLE: Selector = filtered(
        &Selector::Hand(Who::Either),
        &[
            Filter::Has(Test::InSlot(EFFECT)),
            Filter::Has(Test::Inspectable(Who::Opponent)),
        ],
    );
    const KEPT_AVAILABLE: Selector = filtered(
        &Selector::AvailableAchievements,
        &[Filter::Has(Test::InSlot(EFFECT))],
    );
    for case in 0..3 {
        let mut games = Vec::new();
        for original in 0..2 {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let mut instructions = vec![choose(Who::Executor, Choice::Card(&HAND)).keep(EFFECT)];
            let counted = match case {
                0 => &RED_KEPT,
                1 => {
                    instructions.push(transfer(HAND, Destination::Hand(Who::Opponent)));
                    &KEPT_INSPECTABLE
                }
                _ => {
                    instructions.extend([meld(HAND), meld(HAND)]);
                    &KEPT_AVAILABLE
                }
            };
            instructions.extend([
                Instruction::Bind(Binding::Value(ACTION, Expr::Count(counted))),
                STOP,
            ]);
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions,
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            let codes = if case == 2 {
                f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
                ["B001", "B002"]
            } else {
                ["B002", "B004"]
            };
            let hand = codes.map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let expansions = if case == 2 {
                Expansions::FIGURES
            } else {
                Expansions::BASE
            };
            let mut game = Game::seeded(f.position(), expansions, 2)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            let choices: &[CardId] = if case == 0 {
                &[hand[original]]
            } else {
                &[hand[original], hand[0]]
            };
            for &card in choices {
                let menu = decision(&mut game, &mut events);
                assert_eq!(menu.player, A);
                let selected=menu.options.iter().position(|option|matches!(option,Alternative::Select{target:TargetDescriptor::Known{card:known,..}}|Alternative::Move{target:TargetDescriptor::Known{card:known,..},..} if *known==card)).unwrap();
                game.choose(selected).unwrap();
            }
            pause(&mut game);
            let slot = slots(&game, A, Life::Action);
            assert_eq!(slot.len(), 1);
            // Nonvacuous: the real count differs between the worlds.
            assert_eq!(
                field(&slot[0], F::Scalar),
                Some(u64::from((original == 0) != (case == 2)))
            );
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 7),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].position, games[1].position);
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn identity_conditions_require_the_tested_anonymous_reference_to_be_named() {
    for in_slot in [false, true] {
        let mut games = Vec::new();
        for selected in 0..2 {
            let mut f = Fixture::default();
            let mut source = base_card("B012").unwrap();
            let condition = Box::leak(Box::new(Cond::Card(
                Reg::Bound,
                Filter::Has(if in_slot {
                    Test::InSlot(EFFECT)
                } else {
                    Test::Is(Reg::Slot(EFFECT))
                }),
            )));
            source.effects = vec![Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![
                    reveal(HAND).keep(EFFECT),
                    bind(HAND),
                    if_(condition, 1),
                    STOP,
                    STOP,
                ],
            }];
            let source = f.add(source, Zone::Board(A, Color::Blue));
            let hand = ["B002", "B004"].map(|code| f.add(base_card(code).unwrap(), Zone::Hand(A)));
            let mut game = Game::seeded(f.position(), Expansions::BASE, 8)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            for card in [hand[0], hand[selected]] {
                let menu = decision(&mut game, &mut events);
                assert_eq!(menu.player, A);
                let choice=menu.options.iter().position(|option|matches!(option,Alternative::Reveal{target:TargetDescriptor::Known{card:known,..}}|Alternative::Select{target:TargetDescriptor::Known{card:known,..}} if *known==card)).unwrap();
                game.choose(choice).unwrap();
            }
            pause(&mut game);
            assert_eq!(rows(&game, B).len(), 1);
            assert_eq!(rows(&game, B)[0].kind, K::Opaque);
            assert!(matches!(
                game.sample_world(B, 9),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
            games.push(game);
        }
        assert_eq!(games[0].observe(B).unwrap(), games[1].observe(B).unwrap());
    }
}

#[test]
fn tracker_named_retired_memory_source_samples_without_explicit_evidence() {
    let mut f = Fixture::default();
    let mut public = base_card("B015").unwrap();
    public.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![choose(
            Who::Executor,
            Choice::Value(ValueChoices::Boxed(&[1, 2])),
        )],
    }];
    let color = public.color;
    let public = f.add(public, Zone::Board(A, color));
    let hidden = ["B011", "B012"].map(|code| f.add(base_card(code).unwrap(), Zone::Score(B)));
    let mut game = Game::seeded(f.position(), Expansions::BASE, 5)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: public,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    // A's own named exclusion narrows the anonymous pick to one identity.
    // Only A's tracker names it; the retained evidence stays anonymous.
    let anonymous = game.capture(hidden[0]);
    let pick = anonymous.evidence.pick().unwrap();
    let mut other = game.capture(hidden[1]);
    other.evidence |= Evidence::known_to(A);
    game.constrain_anonymous_choice(anonymous, &[other], A);
    assert_eq!(
        game.tracker(A).unwrap().pick_identity(pick),
        Some(hidden[0])
    );
    assert!(!anonymous.evidence.known(A));
    let mut source = Context::new(A);
    source.source_card = Some(hidden[0]);
    source.source_evidence = anonymous.evidence;
    game.store(
        Keep {
            slot: ACTION,
            append: false,
        },
        Some(slots::Held::Value(7)),
        source,
    )
    .unwrap();
    let mut permuted = false;
    for seed in 0..32 {
        let sampled = game.sample_world(A, seed).unwrap();
        permuted |=
            sampled.world.position.cards(Zone::Score(B)) != game.position.cards(Zone::Score(B));
        assert_eq!(
            game.determinize(A, seed).unwrap().observe(A).unwrap(),
            game.observe(A).unwrap()
        );
    }
    assert!(
        permuted,
        "physical matching really permutes the ambiguous pile"
    );
}

#[test]
fn named_retired_memory_source_pins_older_anonymous_alias_through_sampling() {
    let mut f = Fixture::default();
    let mut public = base_card("B015").unwrap();
    public.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![choose(
            Who::Executor,
            Choice::Value(ValueChoices::Boxed(&[1, 2])),
        )],
    }];
    let color = public.color;
    let public = f.add(public, Zone::Board(A, color));
    let hidden = ["B011", "B012"].map(|code| f.add(base_card(code).unwrap(), Zone::Score(B)));
    let mut game = Game::seeded(f.position(), Expansions::BASE, 5)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: public,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    // A retained source was named earlier; an older alias shares its pick.
    // Subsequent anonymous departures left its physical support ambiguous.
    let anonymous = game.capture(hidden[0]);
    let pick = anonymous.evidence.pick().unwrap();
    assert!(game.tracker(A).unwrap().pick_identity(pick).is_none());
    let mut source = Context::new(A);
    source.source_card = Some(hidden[0]);
    source.source_evidence = anonymous.evidence | Evidence::known_to(A);
    game.store(
        Keep {
            slot: ACTION,
            append: false,
        },
        Some(slots::Held::Value(7)),
        source,
    )
    .unwrap();
    let mut source = Context::new(A);
    source.source_card = Some(public);
    source.source_evidence = Evidence::PUBLIC;
    let alias_slot = Slot::action(1);
    game.store(
        Keep {
            slot: alias_slot,
            append: false,
        },
        Some(slots::Held::Cards(vec![anonymous].into())),
        source,
    )
    .unwrap();
    let mut permuted = false;
    for seed in 0..32 {
        let sampled = game.sample_world(A, seed).unwrap();
        assert_eq!(
            sampled.world.slot_cards(alias_slot, source, Regs::EMPTY)[0].inner,
            hidden[0]
        );
        permuted |=
            sampled.world.position.cards(Zone::Score(B)) != game.position.cards(Zone::Score(B));
        assert_eq!(
            game.determinize(A, seed).unwrap().observe(A).unwrap(),
            game.observe(A).unwrap()
        );
    }
    assert!(
        permuted,
        "physical matching really permutes the ambiguous pile"
    );
}
