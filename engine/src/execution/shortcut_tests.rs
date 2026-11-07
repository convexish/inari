use super::*;

use super::common::*;

fn prosperity(f: &mut Fixture, age: u8, zone: Zone) -> CardId {
    let card = f.tech(Color::Blue, age, zone, vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
    card
}

fn demand_context() -> Context {
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    context
}

fn action(game: &mut Game, oars: CardId) {
    game.take_action(
        Action::Dogma {
            card: oars,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
}

/// Canonical manual sequence, not an assertion that every Oars order is equal.
fn manual_choice(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            if let Alternative::Move {
                verb: Verb::Transfer,
                target: TargetDescriptor::Known { card, .. },
            } = option
            {
                Some(((game.position.card(*card).value, *card), index))
            } else {
                None
            }
        })
        .min_by_key(|(key, _)| *key)
        .map_or(0, |(_, index)| index)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut decisions = 0;
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                decisions += 1;
                game.choose(manual_choice(game, &menu)).unwrap();
            }
            status => return (status, decisions),
        }
    }
    panic!("Oars did not complete");
}

fn basic_fixture() -> (Fixture, CardId, Vec<CardId>, Vec<CardId>) {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    // Deliberately noncanonical hand order; drawn qualifying cards join the loop.
    let hand = vec![
        prosperity(&mut f, 7, Zone::Hand(B)),
        prosperity(&mut f, 2, Zone::Hand(B)),
        prosperity(&mut f, 4, Zone::Hand(B)),
    ];
    let mut draws = Vec::new();
    for _ in 0..3 {
        draws.push(f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]));
    }
    for _ in 0..2 {
        draws.push(prosperity(&mut f, 1, Zone::Deck(Deck::base(1))));
    }
    (f, oars, hand, draws)
}

fn assert_oracle<const F: bool>(position: Position, oars: CardId) -> (Game, Vec<Event>, usize) {
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    action(&mut reduced, oars);
    action(&mut manual, oars);
    let mut reduced_events = Vec::new();
    let mut manual_events = Vec::new();
    let (result, decisions) = finish(&mut reduced, &mut reduced_events);
    let (manual_result, manual_decisions) = finish(&mut manual, &mut manual_events);
    assert_eq!(result, manual_result);
    assert_eq!(reduced_events, manual_events);
    // Compare everything including RNG, continuation/completion and ordinals.
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual);
    assert!(reduced.position.check_conservation());
    assert!(decisions <= manual_decisions);
    (reduced, reduced_events, decisions)
}

#[test]
fn ordinary_oars_is_one_activation_with_no_internal_decisions_and_all_events_retained() {
    let (f, oars, hand, draws) = basic_fixture();
    let (game, events, decisions) = assert_oracle::<false>(f.position(), oars);
    assert_eq!(decisions, 0);
    assert_eq!(
        events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) => Some(m.card),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            hand[1], draws[4], draws[4], draws[3], draws[3], draws[2], hand[2], draws[1], hand[0],
            draws[0]
        ]
    );
    assert_eq!(
        game.position.cards(Zone::Score(A)),
        &[hand[1], draws[4], draws[3], hand[2], hand[0]]
    );
    assert_eq!(
        game.position.cards(Zone::Hand(B)),
        &[draws[2], draws[1], draws[0]]
    );
    assert_eq!(
        events
            .iter()
            .filter_map(|e| match e {
                Event::Revealed {
                    card,
                    source: Zone::Hand(B),
                    ..
                } => Some(*card),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [draws[2], draws[1], draws[0]]
    );
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position.regular_actions_started(), 1);
    assert_eq!(game.position.turn_owner(), A);
}

#[test]
fn the_manual_oracle_really_exposes_transfer_choices() {
    let (f, oars, ..) = basic_fixture();
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.shortcuts_disabled = true;
    action(&mut game, oars);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B);
    assert_eq!(menu.options.len(), 3);
    assert!(events.is_empty());
    assert!(finish(&mut game, &mut events).1 >= 3);
}

#[test]
fn dormant_figure_decks_do_not_disable_shortcutting() {
    let (mut f, oars, ..) = basic_fixture();
    f.figure(Figure::AlexTrebek, Zone::Deck(Deck::figures(10)));
    let (_, events, decisions) = assert_oracle::<true>(f.position(), oars);
    assert_eq!(decisions, 0);
    assert_eq!(events.len(), 13);
}

#[test]
fn unrelated_figures_do_not_disable_the_loop_and_scoped_boxes_still_apply() {
    for figure in [
        Figure::Homer,
        Figure::Archimedes,
        Figure::ShenKuo,
        Figure::JamesClerkMaxwell,
        Figure::HedyLamarr,
    ] {
        let mut f = Fixture::default();
        let oars = f.oars(Zone::Board(A, Color::Red));
        let color = match figure {
            Figure::Homer => Color::Purple,
            Figure::ShenKuo | Figure::HedyLamarr => Color::Green,
            _ => Color::Blue,
        };
        f.figure(figure, Zone::Board(B, color));
        prosperity(&mut f, 5, Zone::Hand(B));
        prosperity(&mut f, 3, Zone::Hand(B));
        let age = if figure == Figure::Archimedes { 2 } else { 1 };
        for _ in 0..2 {
            f.tech(Color::Yellow, age, Zone::Deck(Deck::base(age)), vec![]);
        }
        let (_, events, decisions) = assert_oracle::<true>(f.position(), oars);
        assert_eq!(decisions, 0, "{figure:?}");
        assert_eq!(events.len(), 6);
        assert!(
            events
                .iter()
                .filter_map(|e| match e {
                    Event::Moved(m) if m.verb.is_none() => Some(m.source),
                    _ => None,
                })
                .all(|source| source == Zone::Deck(Deck::base(age)))
        );
    }
}

#[test]
fn each_transfer_or_draw_hazard_restores_the_ordinary_menu() {
    for (figure, color) in [
        (Figure::YiSunSin, Color::Red),
        (Figure::AlfredNobel, Color::Green),
        (Figure::Nightingale, Color::Yellow),
        (Figure::AlexTrebek, Color::Yellow),
        (Figure::JackieChan, Color::Red),
    ] {
        let mut f = Fixture::default();
        let oars = f.oars(Zone::Board(A, Color::Red));
        f.figure(figure, Zone::Board(B, color));
        prosperity(&mut f, 4, Zone::Hand(B));
        prosperity(&mut f, 2, Zone::Hand(B));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        action(&mut game, oars);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B, "{figure:?}");
        assert_eq!(menu.options.len(), 2, "{figure:?}");
        assert!(menu.options.iter().all(|o| matches!(
            o,
            Alternative::Move {
                verb: Verb::Transfer,
                ..
            }
        )));
        assert!(events.is_empty());
    }
}

#[test]
fn trebek_blocks_even_when_a_figure_currently_in_hand_suppresses_his_first_draw_trigger() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    f.figure(Figure::AlexTrebek, Zone::Board(B, Color::Yellow));
    f.figure(Figure::AlfredNobel, Zone::Hand(B)); // Prosperity; may leave before a later draw.
    prosperity(&mut f, 2, Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    action(&mut game, oars);
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options.len(), 2);
    assert!(events.is_empty());
}

#[test]
fn buried_and_hand_figures_are_not_active_shortcut_hazards() {
    let (mut f, oars, ..) = basic_fixture();
    f.figure(Figure::AlfredNobel, Zone::Board(B, Color::Green));
    f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    f.figure(Figure::JackieChan, Zone::Hand(B));
    let (_, _, decisions) = assert_oracle::<true>(f.position(), oars);
    assert_eq!(decisions, 0);
}

#[test]
fn guards_requery_board_state_instead_of_latching_at_action_start() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    let noble = f.figure(Figure::AlfredNobel, Zone::Board(B, Color::Green));
    prosperity(&mut f, 4, Zone::Hand(B));
    prosperity(&mut f, 2, Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let context = demand_context();
    let original = game
        .select_bound(
            build::filtered(&build::HAND, &[Filter::Has(Test::Icon(Icon::Prosperity))]),
            context,
            None,
            SelectionParameter::None,
        )
        .unwrap();
    let mut targets = original.clone();
    game.reduce_moves(Shortcut::LowestFirst, (oars, 0, 0), context, &mut targets);
    assert_eq!(targets.len(), 2);
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(noble, Zone::Board(B, Color::Green)),
            destination: Zone::Score(B),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    game.reduce_moves(Shortcut::LowestFirst, (oars, 0, 0), context, &mut targets);
    assert_eq!(targets.len(), 1);
    // A semantic meld reactivates the same hazard without changing the hand.
    game.start(
        meld(noble, Zone::Score(B), B, Color::Green),
        Context::new(B),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    targets = original;
    game.reduce_moves(Shortcut::LowestFirst, (oars, 0, 0), context, &mut targets);
    assert_eq!(targets.len(), 2);
}

#[test]
fn the_oars_exception_preserves_lowest_first_even_when_other_orders_change_the_winner() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    let high = prosperity(&mut f, 7, Zone::Hand(B));
    let low = prosperity(&mut f, 2, Zone::Hand(B));
    f.tech(Color::Yellow, 4, Zone::Score(B), vec![]);
    let position = f.position();
    let (mut game, events, decisions) = assert_oracle::<false>(position.clone(), oars);
    assert_eq!(decisions, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, low);
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(B));
    let mut other = Game::new(position, Expansions::BASE).unwrap();
    other.shortcuts_disabled = true;
    action(&mut other, oars);
    let mut events = Vec::new();
    choose_card(&mut other, &mut events, high);
    assert_eq!(settle(&mut other, &mut events), Status::Winner(A));
}

#[test]
fn no_original_transfer_means_the_followup_draw_and_no_extra_choice() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    let hidden = f.tech(Color::Yellow, 7, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let (game, events, decisions) = assert_oracle::<false>(f.position(), oars);
    assert_eq!(decisions, 0);
    assert_eq!(events.len(), 2);
    assert!(
        matches!(events[0], Event::Revealed { card, source: Zone::Hand(B), .. } if card == hidden)
    );
    assert_eq!(moved(&events[1]).card, drawn);
    assert_eq!(game.position.cards(Zone::Hand(A)), &[drawn]);
    assert!(game.position.cards(Zone::Score(A)).is_empty());
}

#[test]
fn replaced_transfer_stops_the_repeat_and_cannot_satisfy_the_cross_demand_condition() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let low = prosperity(&mut f, 2, Zone::Hand(B));
    let high = prosperity(&mut f, 5, Zone::Hand(B));
    let reaction = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let followup = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let (game, events, decisions) = assert_oracle::<true>(f.position(), oars);
    assert_eq!(decisions, 1);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [low, reaction, reaction, oars, followup]
    );
    // Yi scores the now-foreign Oars, but its retained program still reaches
    // the follow-up and must not count Yi's child activity as a transfer.
    assert!(events[..4].iter().all(|e| moved(e).inside_karma));
    assert!(!moved(&events[4]).inside_karma);
    assert_eq!(game.position.cards(Zone::Score(B)), &[oars]);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[high]);
    assert_eq!(game.position.cards(Zone::Hand(A)), &[followup]);
    assert!(game.position.cards(Zone::Score(A)).is_empty());
}

#[test]
fn a_trebek_transfer_in_shared_fallback_cannot_cancel_the_other_players_fallback() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    let support = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    f.cards[usize::from(support.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    f.figure(Figure::AlexTrebek, Zone::Board(B, Color::Yellow));
    let homer = f.figure(Figure::Homer, Zone::Deck(Deck::figures(1)));
    let draws: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]))
        .collect();
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    action(&mut game, oars);
    let mut events = Vec::new();
    let decks = decision(&mut game, &mut events);
    game.choose(
        decks
            .options
            .iter()
            .position(|o| *o == Alternative::Deck(Deck::figures(1)))
            .unwrap(),
    )
    .unwrap();
    let names = decision(&mut game, &mut events);
    game.choose(
        names
            .options
            .iter()
            .position(|o| matches!(o, Alternative::NameFigure(name) if name.code == "F015"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    let transfers: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb == Some(Verb::Transfer) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers[0].card, homer);
    assert!(transfers[0].inside_karma);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[homer, draws[2]]);
    assert_eq!(game.position.cards(Zone::Hand(A)), &[draws[1], draws[0]]); // fallback + sharing reward
    assert!(game.position.cards(Zone::Score(A)).is_empty());
    assert!(game.position.check_conservation());
}

#[test]
fn keplers_replacement_counts_its_own_suppressed_demand_and_keeps_the_shortcut() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    prosperity(&mut f, 7, Zone::Hand(B));
    prosperity(&mut f, 2, Zone::Hand(B));
    for _ in 0..2 {
        f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    }
    let (game, events, decisions) = assert_oracle::<true>(f.position(), oars);
    assert_eq!(decisions, 0);
    assert_eq!(events.len(), 6);
    assert!(events.iter().all(|e| match e {
        Event::Moved(m) => m.inside_karma,
        Event::Revealed { inside_karma, .. } => *inside_karma,
        _ => false,
    }));
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position.cards(Zone::Score(A)).len(), 2);
    assert_eq!(game.position.regular_actions_started(), 1);
}

#[test]
fn lowest_first_remains_the_policy_with_michelangelos_physical_hand_score_alias() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let high = prosperity(&mut f, 7, Zone::Hand(B));
    let low = prosperity(&mut f, 2, Zone::Hand(B));
    let (mut game, events, decisions) = assert_oracle::<true>(f.position(), oars);
    assert_eq!(decisions, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, low);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[high]);
    assert_eq!(game.position.score(B), 7);
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(B));
}

#[test]
fn meiji_borrowing_and_foreign_control_do_not_inherit_the_ordinary_demand_proof() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    prosperity(&mut f, 4, Zone::Hand(A));
    prosperity(&mut f, 2, Zone::Hand(B));
    let game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    let mut targets = game
        .select_bound(
            build::filtered(&build::HAND, &[Filter::Has(Test::Icon(Icon::Prosperity))]),
            context,
            None,
            SelectionParameter::None,
        )
        .unwrap();
    assert_eq!(targets.len(), 2);
    assert!(!game.position.dependencies_clear(
        &[
            ShortcutDependency::Move(Verb::Transfer),
            ShortcutDependency::Draw,
            ShortcutDependency::Win,
            ShortcutDependency::HandMembership,
        ],
        context,
        false
    ));
    game.reduce_moves(Shortcut::LowestFirst, (oars, 0, 0), context, &mut targets);
    assert_eq!(targets.len(), 2);
}

#[test]
fn self_execution_skips_demands_and_has_its_own_empty_cross_demand_result() {
    let mut f = Fixture::default();
    let oars = f.oars(Zone::Board(A, Color::Red));
    let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    // A suspended outer invocation of this SAME card has already transferred
    // cards. The nested self-execution must not inherit that summary.
    game.frames.push(Frame::Dogma(Dogma {
        card: oars,
        player: A,
        source_owner: A,
        effect: 1,
        participant: 2,
        endorsed: false,
        sharing: false,
        vulnerable: true,
        eligibility: false,
        shared: false,
        executing_shared: false,
        demand_transfers: 3,
        demand_returns: 0,
        suppress_karma: true,
        boxed_action: BoxedValues::default(),
        extra: None,
    }));
    let mut context = Context::new(B);
    context.demander = A;
    context.set_suppress_karma(true);
    game.push_copied(oars, ExecutionMode::SelfExecute, context);
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Dogma { shared: false })
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, drawn);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[drawn]);
}

#[test]
fn demand_summary_resets_between_actions() {
    let (f, oars, ..) = basic_fixture();
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    action(&mut game, oars);
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    action(&mut game, oars);
    // With no eligible cards and no supply, the second action's fallback must
    // attempt a draw and end the game, not reuse the first demand's count.
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert_eq!(events.len(), 16);
}

#[test]
fn filtered_hidden_hand_selection_fails_closed_instead_of_leaking_eligibility() {
    let (f, oars, ..) = basic_fixture();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = demand_context();
    context.controller = A;
    game.start(
        Operation::ExecuteEffect {
            card: oars,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(100, &mut events),
        Err(Error::Unsupported("private filtered card selection"))
    );
    assert!(events.is_empty());
}

#[test]
fn clones_resume_the_same_shortcut_and_demand_summary_at_every_work_boundary() {
    let (f, oars, ..) = basic_fixture();
    let mut game = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    action(&mut game, oars);
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = finish(&mut expected, &mut all);
    assert_eq!(result.1, 0);
    let mut prefix = Vec::new();
    for _ in 0..300 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => panic!("unexpected reduced decision {menu:?}"),
            status => {
                assert_eq!(status, result.0);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position.check_conservation());
    }
    panic!("Oars did not complete");
}

#[test]
fn narrated_games_announce_each_copied_execution_with_its_cause() {
    use crate::program::build::{BOUND, bind, draw, execute, meld};
    let mut f = Fixture::default();
    // Computers' shape: draw and meld a 2, then self-execute it.
    let outer = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![
            draw(2),
            bind(Selector::Reg(Reg::Last)),
            meld(BOUND),
            execute(ExecutionMode::SelfExecute, Who::Executor),
        ],
    );
    let inner = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![draw(3)]);
    let drawn = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    // The Chain award for a self-execution inside a card effect.
    f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE)
        .unwrap()
        .with_narration();
    game.self_execute(outer, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let status = settle(&mut game, &mut events);
    assert_eq!(status, Status::Complete(ReturnValue::Unit), "{events:#?}");
    let narration: Vec<_> = events
        .iter()
        .filter(|event| !matches!(event, Event::Moved(_)))
        .collect();
    assert_eq!(
        narration,
        [
            &Event::ExecutionStarted {
                executor: A,
                card: outer,
                mode: ExecutionMode::SelfExecute,
                source: None,
                inside_karma: false,
            },
            &Event::ExecutionStarted {
                executor: A,
                card: inner,
                mode: ExecutionMode::SelfExecute,
                source: Some(outer),
                inside_karma: false,
            },
            &Event::ChainAwarded {
                executor: A,
                card: inner,
            },
            &Event::ExecutionEnded {
                executor: A,
                card: inner,
            },
            &Event::ExecutionEnded {
                executor: A,
                card: outer,
            },
        ],
        "{events:#?}"
    );
    // The inner card's draw sits between its start and its end, after the
    // Chain award it announces.
    let at = |wanted: &Event| events.iter().position(|e| e == wanted).unwrap();
    let inner_draw = events
        .iter()
        .position(|e| matches!(e, Event::Moved(m) if m.card == drawn))
        .unwrap();
    assert!(at(narration[2]) < inner_draw && inner_draw < at(narration[3]));
    // Unnarrated games (search, training) report none of this.
    let mut f = Fixture::default();
    let card = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![draw(2)]);
    f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert!(events.iter().all(|event| matches!(event, Event::Moved(_))));
}

#[test]
fn a_karma_execution_names_its_figure_as_the_cause() {
    let mut f = Fixture::default();
    let figure = f.figure(Figure::Avicenna, Zone::Board(A, Color::Yellow));
    let card = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_narration();
    game.super_execute(card, Context::reaction(A, figure))
        .unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert_eq!(
        events,
        [
            Event::ExecutionStarted {
                executor: A,
                card,
                mode: ExecutionMode::SuperExecute,
                source: Some(figure),
                inside_karma: true,
            },
            Event::ExecutionEnded { executor: A, card },
        ]
    );
}
