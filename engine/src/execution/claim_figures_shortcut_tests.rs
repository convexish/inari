use super::common::*;
use super::*;

fn fixture(colors: usize) -> (Game, CardId, CardId) {
    let mut f = Fixture::default();
    for color in Color::ALL.into_iter().take(colors) {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
        if color != Color::Yellow {
            f.tech(color, 2, Zone::Board(A, color), vec![]);
        }
    }
    let cai = f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    let scored = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    (
        Game::seeded(f.position(), Expansions::FIGURES, 37).unwrap(),
        cai,
        scored,
    )
}

fn start(game: &mut Game, scored: CardId) {
    let mut context = Context::new(B);
    context.controller = A;
    context.demander = A;
    context.source_owner = A;
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(scored, Zone::Hand(B)),
            destination: Zone::Score(B),
        },
        context,
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut decisions = 0;
    loop {
        match settle(game, events) {
            Status::Decision(menu) => {
                assert_eq!(menu.player, A);
                assert!(menu.options.iter().all(|o| matches!(
                    o,
                    Alternative::Splay {
                        direction: Splay::Left,
                        ..
                    }
                )));
                decisions += 1;
                game.choose(0).unwrap();
            }
            status => return (status, decisions),
        }
    }
}

fn all_paths(
    mut game: Game,
    mut events: Vec<Event>,
    depth: usize,
) -> Vec<(Game, Status, Vec<Event>)> {
    assert!(depth <= 5);
    match settle(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                all_paths(fork, events.clone(), depth + 1)
            })
            .collect(),
        status => vec![(game, status, events)],
    }
}

fn canonical_events(mut events: Vec<Event>) -> Vec<Event> {
    let splays = events.len().saturating_sub(1);
    assert!(
        events[..splays]
            .iter()
            .all(|event| matches!(event, Event::Splayed { .. }))
    );
    assert!(matches!(events.last(), Some(Event::Moved(_))));
    events[..splays].sort_by_key(|event| match event {
        Event::Splayed { color, .. } => *color as u8,
        _ => unreachable!(),
    });
    events
}

#[test]
fn cai_lun_canonical_oracle_preserves_exact_events_rng_game_and_common_frame_budget() {
    let (mut reduced, _, scored) = fixture(5);
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let initial_rng = reduced.rng.clone();
    start(&mut reduced, scored);
    start(&mut manual, scored);
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    let (status, choices) = finish(&mut reduced, &mut actual);
    let (manual_status, manual_choices) = finish(&mut manual, &mut expected);
    assert_eq!(choices, 0);
    assert_eq!(manual_choices, 4);
    assert_eq!(status, manual_status);
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 6);
    assert_eq!(moved(&actual[5]).card, scored);
    assert_eq!(reduced.rng, initial_rng);
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual);
    assert!(std::mem::size_of::<Frame>() <= 56);
}

#[test]
fn every_unrestricted_color_permutation_has_the_same_state_and_semantic_events() {
    for count in 0..=5 {
        let (mut reduced, _, scored) = fixture(count);
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, scored);
        start(&mut manual, scored);
        let mut events = Vec::new();
        let (status, choices) = finish(&mut reduced, &mut events);
        assert_eq!(choices, 0);
        let paths = all_paths(manual, Vec::new(), 0);
        assert_eq!(paths.len(), (1..=count).product::<usize>());
        for (mut end, result, path_events) in paths {
            end.shortcuts_disabled = false;
            assert_eq!(end, reduced);
            assert_eq!(result, status);
            assert_eq!(canonical_events(path_events), events);
            assert!(end.position.check_conservation());
        }
    }
}

#[test]
fn clone_at_every_reduced_and_unrestricted_boundary_keeps_continuations_and_context() {
    for disabled in [false, true] {
        let (mut game, cai, scored) = fixture(5);
        game.shortcuts_disabled = disabled;
        start(&mut game, scored);
        let mut expected = game.clone();
        let mut expected_events = Vec::new();
        let expected_status = finish(&mut expected, &mut expected_events).0;
        let mut events = Vec::new();
        let mut complete = false;
        for _ in 0..200 {
            let mut fork = game.clone();
            let mut suffix = events.clone();
            assert_eq!(finish(&mut fork, &mut suffix).0, expected_status);
            assert_eq!(fork, expected);
            assert_eq!(suffix, expected_events);
            for frame in &game.frames {
                if let Frame::SplayAll(batch) = frame {
                    assert_eq!(batch.context.executor, A);
                    assert_eq!(batch.context.source_card, Some(cai));
                    assert!(batch.context.suppress_karma());
                }
            }
            match game.advance(1, &mut events).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(_) => game.choose(0).unwrap(),
                status => {
                    assert_eq!(status, expected_status);
                    complete = true;
                    break;
                }
            }
        }
        assert!(complete);
    }
}

#[test]
fn splay_batch_requeries_legal_colors_and_offers_order_only_when_observable() {
    let (mut game, _, scored) = fixture(5);
    // Existing left splays are already done; an empty/singleton cannot splay.
    game.position.players[A.index()].splays[Color::Red as usize] = Splay::Left;
    let removed = game
        .position
        .zone_mut(Zone::Board(A, Color::Blue))
        .pop()
        .unwrap();
    game.position.zone_mut(Zone::Hand(A)).push(removed);
    game.position.refresh_boards();
    start(&mut game, scored);
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events).1, 0);
    let splays: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Splayed {
                color,
                inside_karma: true,
                ..
            } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(splays, [Color::Green, Color::Yellow, Color::Purple]);
    // Outside karma, a live Shen Kuo could see each splay: a real order.
    let mut f = Fixture::default();
    for color in [Color::Red, Color::Blue] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
        f.tech(color, 2, Zone::Board(A, color), vec![]);
    }
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    let source = f.tech(
        Color::Purple,
        1,
        Zone::Junk,
        vec![Instruction::Splay(SplayStep {
            who: Who::Executor,
            choices: SplayChoices::Fixed {
                colors: ColorSet::Only(&Color::ALL),
                direction: Splay::Up,
                from: None,
            },
            targets: SplayTargets::Changes,
            optional: false,
            all: true,
        })],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options.len(), 2);
}

#[test]
fn leeuwenhoek_stale_original_return_never_earns_the_child_draw() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    let payment = f.tech(Color::Blue, 5, Zone::Hand(A), vec![]);
    let reward = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
    let original = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let payment_index = menu
        .options
        .iter()
        .position(|option| matches!(option, Alternative::Move { .. }))
        .unwrap();
    game.choose(payment_index).unwrap();
    // Invalidate the exact selected physical target before its ordinary return.
    assert_eq!(game.position.zone_mut(Zone::Hand(A)).pop(), Some(payment));
    game.position.zone_mut(Zone::Hand(B)).push(payment);
    assert!(
        matches!(settle(&mut game, &mut events), Status::Complete(ReturnValue::Operation(OperationOutcome { disposition: Disposition::Performed, card: Some(card), .. })) if card == original)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(6))), &[reward]);
    assert!(game.position.check_conservation());
}

#[test]
fn leeuwenhoek_optional_payment_is_identical_with_shortcuts_disabled_and_clone_safe() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    for color in [Color::Red, Color::Blue] {
        f.tech(color, 5, Zone::Hand(A), vec![]);
    }
    f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
    f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 37).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    for game in [&mut reduced, &mut manual] {
        game.start(
            Operation::Draw {
                value: 1,
                set: None,
            },
            Context::new(A),
        )
        .unwrap();
    }
    let menu = decision(&mut reduced, &mut Vec::new());
    assert_eq!(menu, decision(&mut manual, &mut Vec::new()));
    assert_eq!(menu.options.len(), 3);
    for choice in 0..menu.options.len() {
        let mut actual = reduced.clone();
        let mut expected = manual.clone();
        actual.choose(choice).unwrap();
        expected.choose(choice).unwrap();
        let mut events = Vec::new();
        let mut oracle_events = Vec::new();
        assert_eq!(
            settle(&mut actual, &mut events),
            settle(&mut expected, &mut oracle_events)
        );
        assert_eq!(events, oracle_events);
        expected.shortcuts_disabled = false;
        assert_eq!(actual, expected);
    }
}

#[test]
fn unrestricted_splays_still_suppress_real_reactions_despite_changing_each_queries() {
    let mut f = Fixture::default();
    for color in [Color::Green, Color::Yellow, Color::Purple] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
        for value in [1, 2] {
            f.tech(color, value, Zone::Board(B, color), vec![]);
        }
    }
    f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.figure(Figure::NiccoloMachiavelli, Zone::Board(A, Color::Purple));
    let card = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let mut position = f.position();
    for color in [Color::Green, Color::Yellow, Color::Purple] {
        position.players[B.index()].splays[color as usize] = Splay::Right;
    }
    let mut reduced = Game::seeded(position, Expansions::FIGURES, 37).unwrap();
    let before_score = reduced.position.score(A);
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, card);
    start(&mut manual, card);
    let mut events = Vec::new();
    let (status, _) = finish(&mut reduced, &mut events);
    assert!(reduced.position.score(A) > before_score);
    assert_eq!(events.len(), 4);
    let paths = all_paths(manual, Vec::new(), 0);
    assert_eq!(paths.len(), 6);
    for (mut game, result, path_events) in paths {
        game.shortcuts_disabled = false;
        assert_eq!(game, reduced);
        assert_eq!(result, status);
        assert_eq!(canonical_events(path_events), events);
        for color in [Color::Green, Color::Yellow, Color::Purple] {
            assert_eq!(game.position.splay(A, color), Splay::Left);
            assert_eq!(game.position.splay(B, color), Splay::Right);
        }
    }
}

#[test]
fn new_figures_block_only_reviewed_shortcut_dependencies_and_only_while_active() {
    for figure in [Figure::CaiLun, Figure::AntonieVanLeeuwenhoek] {
        for covered in [false, true] {
            let mut f = Fixture::default();
            f.figure(figure, Zone::Board(A, Color::Yellow));
            if covered {
                f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
            }
            let position = f.position().with_expansions(Expansions::FIGURES);
            let stem_cells = Game::move_dependencies(
                Verb::Score,
                build::HAND,
                Destination::Score(Who::Executor),
            );
            let mut oars = Game::move_dependencies(
                Verb::Transfer,
                build::HAND,
                Destination::Score(Who::Demander),
            );
            oars.extend([ShortcutDependency::Draw, ShortcutDependency::Win]);
            let affected = if figure == Figure::CaiLun {
                stem_cells
            } else {
                oars
            };
            assert_eq!(
                position.dependencies_clear(&affected, Context::new(A), false),
                covered
            );
            let railroad = Game::move_dependencies(Verb::Return, build::HAND, Destination::Deck);
            assert!(position.dependencies_clear(&railroad, Context::new(A), false));
            assert!(position.dependencies_clear(
                &[ShortcutDependency::Move(Verb::Junk)],
                Context::new(A),
                true
            ));
            assert!(position.dependencies_clear(
                &[ShortcutDependency::JunkSpecial],
                Context::new(A),
                false
            ));
        }
    }
}
