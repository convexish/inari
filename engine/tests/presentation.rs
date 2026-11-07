//! Presentation must follow the running continuation, never the last action or
//! the source's current zone. It must not change choices or expose private text.
use inno_engine::*;
mod common;
use common::*;

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

#[test]
fn every_catalog_branch_has_explicit_wording() {
    let mut count = 0;
    let mut inspect = |program: &[Instruction]| {
        for instruction in program {
            if let Instruction::Choose {
                choice: Choice::Branch(branches) | Choice::SetBranch { branches, .. },
                ..
            } = instruction
            {
                count += 1;
                assert!(branches.len() >= 2);
                for (_, label) in *branches {
                    assert!(!label.trim().is_empty());
                    assert!(!label.starts_with("Option "));
                }
            }
        }
    };
    for metadata in BASE_METADATA {
        for effect in base_card(metadata.code).unwrap().effects {
            inspect(&effect.instructions);
        }
    }
    for figure in FIGURE_NAMES {
        let card = figure_card(figure.code).unwrap();
        for effect in card.effects {
            inspect(&effect.instructions);
        }
        for karma in card.figure.unwrap().def().karma {
            if let Body::Program(program) = karma.body {
                inspect(program);
            }
        }
    }
    assert_eq!(count, 5);
}

#[test]
fn base_branch_wording_preserves_indices_and_optional_decline() {
    for (code, labels) in [
        (
            "B018",
            [
                "Exchange all highest cards in your hand and score pile",
                "Junk all cards in the [3] deck",
            ],
        ),
        (
            "B072",
            [
                "Draw and score an [8], then return a card from your score pile",
                "Draw one value higher than your highest score card",
            ],
        ),
    ] {
        let mut f = Fixture::default();
        let card = base_card(code).unwrap();
        let zone = Zone::Board(A, card.color);
        let source = f.add(card, zone);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        invoke(&mut game, source);
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.source_card(A), Some(source));
        assert_eq!(menu.source_card(B), Some(source));
        assert_eq!(
            menu.options,
            [
                Alternative::Branch(0),
                Alternative::Branch(1),
                Alternative::Decline
            ]
        );
        assert_eq!(menu.option_label(0, A), Some(labels[0]));
        assert_eq!(menu.option_label(1, B), Some(labels[1]));
        assert_eq!(menu.option_label(2, A), None);
        assert_eq!(menu.option_label(99, A), None);
        game.choose(2).unwrap();
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
    }
}

#[test]
fn newtons_native_draw_action_reports_karma_source_and_destination_wording() {
    for branch in 0..2 {
        let mut f = Fixture::default();
        let card = figure_card(Figure::IsaacNewton.code()).unwrap();
        let color = card.color;
        let newton = f.add(card, Zone::Board(A, color));
        let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(Action::Draw, ActionKind::Regular).unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.source_card(A), Some(newton));
        assert_eq!(menu.source_card(B), Some(newton));
        assert_eq!(
            menu.option_label(0, A),
            Some("Transfer the drawn card to your board")
        );
        assert_eq!(
            menu.option_label(1, A),
            Some("Transfer the drawn card to your opponent's board")
        );
        let mut clone = game.clone();
        assert_eq!(decision(&mut clone, &mut Vec::new()), menu);
        game.choose(branch).unwrap();
        clone.choose(branch).unwrap();
        let mut suffix = Vec::new();
        assert_eq!(
            settle(&mut game, &mut suffix),
            settle(&mut clone, &mut Vec::new())
        );
        assert_eq!(game, clone);
        assert_eq!(
            game.position()
                .top(if branch == 0 { A } else { B }, Color::Yellow),
            Some(drawn)
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn executing_a_hidden_card_discloses_it_then_resumes_the_parent_source() {
    let mut f = Fixture::default();
    let parent = f.tech(
        Color::Blue,
        3,
        Zone::Board(A, Color::Blue),
        vec![
            Instruction::Bind(Binding::Card(build::HAND)),
            build::execute(ExecutionMode::SelfExecute, Who::Executor),
            build::choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
        ],
    );
    let child = f.tech(
        Color::Red,
        2,
        Zone::Hand(A),
        vec![
            build::return_(Selector::Reg(Reg::Source)),
            build::choose(
                Who::Executor,
                Choice::Branch(&[
                    (0, "Finish this private effect"),
                    (0, "Also finish this private effect"),
                ]),
            ),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    invoke(&mut game, parent);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[child]);
    // Executing a card exposes its text at the table (as for Mystery Box),
    // so even a hand card's source and wording are public once it runs.
    assert_eq!(menu.source_card(A), Some(child));
    assert_eq!(menu.source_card(B), Some(child));
    assert_eq!(menu.option_label(0, A), Some("Finish this private effect"));
    assert_eq!(menu.option_label(0, B), Some("Finish this private effect"));
    let returned = game.tracker(B).unwrap().belief(Zone::Deck(Deck::base(2)));
    assert_eq!(returned[0].known(), Some(child));
    let mut clone = game.clone();
    assert_eq!(decision(&mut clone, &mut Vec::new()), menu);
    game.choose(0).unwrap();
    let parent_menu = decision(&mut game, &mut events);
    assert_eq!(parent_menu.source_card(A), Some(parent));
    assert_eq!(parent_menu.source_card(B), Some(parent));
    assert_eq!(
        parent_menu.options,
        [Alternative::Value(1), Alternative::Value(2)]
    );
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
    }
}

#[test]
fn demand_choices_name_the_effect_source_not_the_victim_or_enclosing_karma() {
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Blue,
        3,
        Zone::Board(A, Color::Blue),
        vec![build::choose(
            Who::Executor,
            Choice::Value(ValueChoices::Boxed(&[1, 2])),
        )],
    );
    f.cards[usize::from(source.0)].effects[0].kind = EffectKind::Demand;
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.super_execute(source, Context::new(A)).unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, B);
    assert_eq!(menu.source_card(A), Some(source));
    assert_eq!(menu.source_card(B), Some(source));
}

#[test]
fn effect_victory_names_its_executed_source_after_it_returns() {
    for public in [false, true] {
        let mut f = Fixture::default();
        let source = f.tech(
            Color::Red,
            2,
            if public {
                Zone::Board(A, Color::Red)
            } else {
                Zone::Hand(A)
            },
            vec![
                build::return_(Selector::Reg(Reg::Source)),
                Instruction::Win(WinRule::Executor),
            ],
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        assert_eq!(game.victory(), None);
        assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(A));
        let victory = game.victory().unwrap();
        assert_eq!(victory.reason, WinReason::Effect);
        assert_eq!(victory.source_card(A), Some(source));
        // Execution disclosed even a hand card's identity.
        assert_eq!(victory.source_card(B), Some(source));
        assert_eq!(game.clone().victory(), Some(victory));
    }
}

#[test]
fn opening_and_action_end_fading_do_not_inherit_a_card_source() {
    let mut opening = Game::new_figures(4).unwrap();
    let menu = decision(&mut opening, &mut Vec::new());
    assert_eq!(menu.source_card(A), None);
    assert_eq!(menu.source_card(B), None);

    let mut f = Fixture::default();
    for figure in [Figure::IsaacNewton, Figure::Gilgamesh] {
        let card = figure_card(figure.code()).unwrap();
        let color = card.color;
        f.add(card, Zone::Board(A, color));
    }
    let source = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.source_card(A), None);
    assert!(menu.options.iter().all(|option| matches!(
        option,
        Alternative::Move {
            verb: Verb::Score,
            ..
        }
    )));
}
