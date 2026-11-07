use inno_engine::*;
mod common;
use common::*;

fn native(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = base_card(code).unwrap();
    let zone = Zone::Board(owner, card.color);
    f.add(card, zone)
}

fn native_figure(f: &mut Fixture, figure: Figure, owner: Player) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let zone = Zone::Board(owner, card.color);
    f.add(card, zone)
}

fn pick(menu: &Decision, preferred: Option<CardId>) -> usize {
    menu.options
        .iter()
        .position(|option| match option {
            Alternative::Select {
                target: TargetDescriptor::Known { card, .. },
            }
            | Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(*card) == preferred,
            Alternative::NameFigure(name) => name.code == Figure::Gilgamesh.code(),
            _ => false,
        })
        .unwrap_or(0)
}

fn finish(game: &mut Game, events: &mut Vec<Event>, preferred: Option<CardId>) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(pick(&menu, preferred)).unwrap(),
            status => return status,
        }
    }
    panic!("copied execution failed to terminate");
}

fn keyword(mode: ExecutionMode) -> Vec<Instruction> {
    vec![
        Instruction::Bind(Binding::Card(build::HAND)),
        build::execute(mode, Who::Executor),
    ]
}

#[test]
fn native_computers_robotics_internet_awards_only_the_nested_chain_before_child_effects() {
    let mut f = Fixture::default();
    let computers = native(&mut f, "B092", A);
    let second = f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let first = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
    let scored = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let chain = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let internet = f.add(base_card("B105").unwrap(), Zone::Deck(Deck::base(10)));
    let robotics = f.add(base_card("B097").unwrap(), Zone::Deck(Deck::base(10)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card: computers,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).verb, moved(e).card))
            .collect::<Vec<_>>(),
        [
            (None, robotics),
            (Some(Verb::Meld), robotics),
            (None, internet),
            (Some(Verb::Meld), internet),
            (None, chain),
            (Some(Verb::Achieve), chain),
            (None, scored),
            (Some(Verb::Score), scored),
            (None, first),
            (Some(Verb::Meld), first),
            (None, second),
            (Some(Verb::Meld), second),
        ]
    );
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[chain]);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        &[computers, first]
    );
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        &[robotics, second]
    );
    assert_eq!(game.position().top(A, Color::Purple), Some(internet));
    assert!(game.position().check_conservation());
}

#[test]
fn super_execution_ignores_sharing_icons_and_owns_its_preceding_demand_summary() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B037", A);
    let payment = native(&mut f, "B001", B);
    native(&mut f, "B024", B);
    native(&mut f, "B052", B);
    let reward = f.add(base_card("B021").unwrap(), Zone::Deck(Deck::base(2)));
    let position = f.position();
    assert!(
        position
            .clone()
            .with_expansions(Expansions::BASE)
            .icon_count(B, Icon::Industry)
            >= position
                .clone()
                .with_expansions(Expansions::BASE)
                .icon_count(A, Icon::Industry)
    );
    let mut self_only = Game::new(position.clone(), Expansions::BASE).unwrap();
    self_only.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut self_only, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(self_only.position(), &position);
    let mut game = Game::new(position, Expansions::BASE).unwrap();
    game.super_execute(source, Context::new(A)).unwrap();
    assert_eq!(
        finish(&mut game, &mut events, Some(payment)),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[payment, reward]);
    assert_eq!(
        events.iter().map(|e| moved(e).executor).collect::<Vec<_>>(),
        [B, A, A]
    );
    assert_eq!(game.position().regular_actions_started(), 0);
    assert_eq!(game.position().turn_owner(), A);
}

#[test]
fn native_foreign_super_execution_continues_after_its_source_is_transferred_away() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B024", B);
    let own = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let demanded = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut context = Context::new(A);
    context.source_owner = B;
    game.super_execute(source, context).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[source]);
    assert_eq!(game.position().top(B, Color::Purple), None);
    assert_eq!(game.position().top(B, Color::Blue), Some(demanded));
    assert_eq!(game.position().top(A, Color::Red), Some(own));
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).card, moved(e).executor))
            .collect::<Vec<_>>(),
        [
            (source, B),
            (demanded, B),
            (demanded, B),
            (own, A),
            (own, A)
        ]
    );
}

#[test]
fn copied_demand_keyword_does_not_inherit_chain_eligibility_from_outer_non_demand() {
    let mut f = Fixture::default();
    let root = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        keyword(ExecutionMode::SuperExecute),
    );
    let middle = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    f.cards[usize::from(middle.0)].effects = vec![
        Effect {
            kind: EffectKind::Demand,
            instructions: keyword(ExecutionMode::SelfExecute),
        },
        Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![build::draw(1)],
        },
    ];
    let child = f.tech(Color::Red, 1, Zone::Hand(B), vec![build::draw(2)]);
    let unclaimed = f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let claimed = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let own = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let opposing = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(root, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[claimed]);
    assert!(game.position().cards(Zone::Achievements(B)).is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(11))),
        &[unclaimed]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[middle, own]);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[child, opposing]);
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).card, moved(e).executor))
            .collect::<Vec<_>>(),
        [(claimed, A), (claimed, A), (opposing, B), (own, A)]
    );
}

#[test]
fn chain_draw_and_claim_are_real_operations_before_the_pending_keyword() {
    let mut f = Fixture::default();
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let root = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        keyword(ExecutionMode::SelfExecute),
    );
    native_figure(&mut f, Figure::HedyLamarr, A);
    let child = f.tech(Color::Red, 1, Zone::Hand(A), vec![build::draw(1)]);
    let award = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let drawn = f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(game.position().score(A) < 55);
    game.self_execute(root, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, Some(child)),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(moved(&events[0]).card, award);
    assert!(matches!(
        events[1],
        Event::Splayed {
            color: Color::Blue,
            to: Splay::Up,
            inside_karma: true,
            ..
        }
    ));
    assert_eq!(moved(&events[2]).verb, Some(Verb::Achieve));
    assert_eq!(moved(&events[2]).card, award);
    assert_eq!(moved(&events[3]).card, drawn);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[award]);
}

#[test]
fn recursive_keywords_award_each_chain_without_box_increase_and_exhaust_without_fake_work_draw() {
    let mut f = Fixture::default();
    native_figure(&mut f, Figure::Archimedes, A);
    let source = f.tech(
        Color::Red,
        1,
        Zone::Hand(A),
        keyword(ExecutionMode::SelfExecute),
    );
    let second = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let first = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    for _ in 0..300 {
        let mut clone = game.clone();
        let mut actual = Vec::new();
        let mut copied = Vec::new();
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(clone.advance(1, &mut copied).unwrap(), status);
        assert_eq!(actual, copied);
        assert_eq!(game, clone);
        events.extend(actual);
        match status {
            Status::WorkLimit => {}
            Status::Decision(menu) => game.choose(pick(&menu, Some(source))).unwrap(),
            Status::Winner(winner) => {
                assert_eq!(winner, A);
                break;
            }
            other => panic!("unexpected recursive result {other:?}"),
        }
    }
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(A));
    assert_eq!(
        game.position().cards(Zone::Achievements(A)),
        &[first, second]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[source]);
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).verb, moved(e).card))
            .collect::<Vec<_>>(),
        [
            (None, first),
            (Some(Verb::Achieve), first),
            (None, second),
            (Some(Verb::Achieve), second)
        ]
    );
}

#[test]
fn retained_drawn_card_is_executed_after_von_neumann_replaces_the_meld() {
    for copied_root in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B092", A);
        native_figure(&mut f, Figure::JohnVonNeumann, A);
        let child = f.tech(
            Color::Red,
            10,
            Zone::Deck(Deck::base(10)),
            vec![build::draw(1)],
        );
        let later = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let early = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        let second_award = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
        let first_award = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if copied_root {
            game.self_execute(source, Context::new(A)).unwrap();
        } else {
            game.take_action(
                Action::Dogma {
                    card: source,
                    auspice: false,
                },
                ActionKind::Regular,
            )
            .unwrap();
        }
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events, None),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().top(A, Color::Red), Some(child));
        assert_eq!(game.position().cards(Zone::Hand(A)), &[early, later]);
        let child_draws: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Moved(m) if m.verb.is_none() && [early, later].contains(&m.card) => {
                    Some((m.card, m.inside_karma))
                }
                _ => None,
            })
            .collect();
        assert_eq!(child_draws, [(early, true), (later, false)]);
        let expected: &[CardId] = if copied_root {
            &[first_award, second_award]
        } else {
            &[]
        };
        assert_eq!(game.position().cards(Zone::Achievements(A)), expected);
        if copied_root {
            let awards: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Moved(m) if m.verb == Some(Verb::Achieve) => {
                        Some((m.card, m.inside_karma))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(awards, [(first_award, true), (second_award, false)]);
        }
    }
}

#[test]
fn a_selected_figure_executes_nothing_and_earns_no_chain() {
    let mut f = Fixture::default();
    let root = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        keyword(ExecutionMode::SelfExecute),
    );
    let figure = f.figure(Figure::Tigernmas, Zone::Hand(A));
    let award = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(figure, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    game.self_execute(root, Context::new(A)).unwrap();
    assert_eq!(
        finish(&mut game, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(11))), &[award]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[figure]);
    assert!(events.is_empty());
}

#[test]
fn effect_and_nested_call_boundaries_do_not_inherit_or_overwrite_retained_identities() {
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![
            Instruction::Bind(Binding::Card(build::HAND)),
            build::execute(ExecutionMode::SelfExecute, Who::Executor),
            build::execute(ExecutionMode::SelfExecute, Who::Executor),
        ],
    );
    f.cards[usize::from(source.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    f.cards[usize::from(source.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![build::execute(ExecutionMode::SelfExecute, Who::Executor)],
    });
    let child = f.tech(
        Color::Red,
        1,
        Zone::Hand(A),
        vec![
            build::draw(1),
            Instruction::Bind(Binding::Card(Selector::Reg(Reg::Last))),
        ],
    );
    let second = f.tech(
        Color::Yellow,
        1,
        Zone::Deck(Deck::base(1)),
        vec![Instruction::Win(WinRule::Executor)],
    );
    let first = f.tech(
        Color::Purple,
        1,
        Zone::Deck(Deck::base(1)),
        vec![Instruction::Win(WinRule::Executor)],
    );
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, Some(child)),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[child, first, second]
    );
}

#[test]
fn native_satellites_returns_then_splays_then_draws_then_melds_and_executes_the_selected_card() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B091", A);
    for _ in 0..2 {
        f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    }
    let returned = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let high_return = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let third = f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]);
    let second = f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let selected = f.tech(
        Color::Red,
        8,
        Zone::Deck(Deck::base(8)),
        vec![build::draw(1)],
    );
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        decision(&mut game, &mut events).options,
        [
            Alternative::Splay {
                player: A,
                color: Color::Purple,
                direction: Splay::Up
            },
            Alternative::Decline
        ]
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, high_return]
    );
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events, Some(selected)),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().splay(A, Color::Purple), Splay::Up);
    assert_eq!(game.position().top(A, Color::Red), Some(selected));
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[second, third, returned]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(3))),
        &[high_return]
    );
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
}

#[test]
fn robotics_uses_the_drawn_cards_printed_industry_or_efficiency_not_board_modifiers() {
    for icon in [Icon::Industry, Icon::Efficiency, Icon::Health] {
        let mut f = Fixture::default();
        let source = native(&mut f, "B097", A);
        native_figure(&mut f, Figure::CatherineTheGreat, A);
        let child = f.tech(
            Color::Red,
            10,
            Zone::Deck(Deck::base(10)),
            vec![build::draw(1)],
        );
        f.cards[usize::from(child.0)].symbols[0] = Symbol::Icon(icon);
        let reward = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events, None),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().top(A, Color::Red), Some(child));
        assert!(game.position().icon_count(A, Icon::Efficiency) > 0);
        let expected: &[CardId] = if icon == Icon::Health {
            &[source]
        } else {
            &[source, reward]
        };
        assert_eq!(game.position().cards(Zone::Hand(A)), expected);
        assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    }
}

#[test]
fn native_computers_binds_its_original_draw_not_trebeks_first_transfer() {
    let mut f = Fixture::default();
    let source = native(&mut f, "B092", A);
    native_figure(&mut f, Figure::AlexTrebek, A);
    let found = f.figure(Figure::Gilgamesh, Zone::Deck(Deck::figures(1)));
    let child = f.tech(
        Color::Red,
        10,
        Zone::Deck(Deck::base(10)),
        vec![build::draw(1)],
    );
    let award = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let reward = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, None),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[found, reward]);
    assert_eq!(game.position().top(A, Color::Red), Some(child));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[award]);
    let movements: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.verb, m.card, m.inside_karma)),
            _ => None,
        })
        .collect();
    assert_eq!(
        movements,
        [
            (Some(Verb::Transfer), found, true),
            (None, child, false),
            (Some(Verb::Meld), child, false),
            (None, award, false),
            (Some(Verb::Achieve), award, false),
            (None, reward, false),
        ]
    );
}

#[test]
fn chain_claims_its_original_draw_not_the_first_reactions_transferred_card() {
    let mut f = Fixture::default();
    let root = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        keyword(ExecutionMode::SelfExecute),
    );
    native_figure(&mut f, Figure::AlexTrebek, A);
    let child = f.tech(Color::Red, 1, Zone::Hand(A), vec![build::draw(1)]);
    let found = f.figure(Figure::Gilgamesh, Zone::Deck(Deck::figures(1)));
    let award = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let drawn = f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(root, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, Some(child)),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[child, found, drawn]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[award]);
    let movements: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.verb, m.card, m.inside_karma)),
            _ => None,
        })
        .collect();
    assert_eq!(
        movements,
        [
            (Some(Verb::Transfer), found, true),
            (None, award, false),
            (Some(Verb::Achieve), award, false),
            (None, drawn, false)
        ]
    );
}

#[test]
fn absent_selected_identity_never_performs_either_keyword_or_awards_chain() {
    for mode in [ExecutionMode::SelfExecute, ExecutionMode::SuperExecute] {
        let mut f = Fixture::default();
        let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), keyword(mode));
        f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
        let position = f.position();
        let mut game = Game::new(position.clone(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events, None),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position(), &position);
        assert!(events.is_empty());
    }
}
