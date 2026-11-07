//! Copied splays and complete Forecast batches: explicit choices and all-path oracles.
use super::*;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn pile(f: &mut Fixture, owner: Player, color: Color) {
    for value in [1, 2] {
        f.tech(color, value, Zone::Board(owner, color), Vec::new());
    }
}
fn effect(game: &mut Game, source: CardId, index: u16) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: index,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
}

#[derive(Clone)]
pub(super) struct End {
    pub(super) game: Game,
    pub(super) status: Status,
    pub(super) events: Vec<Event>,
    pub(super) decisions: usize,
}
fn paths(mut game: Game, mut events: Vec<Event>, decisions: usize) -> Vec<End> {
    assert!(decisions < 12, "bounded oracle");
    let status = game.advance(10000, &mut events).unwrap();
    if let Status::Decision(menu) = status {
        (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                paths(fork, events.clone(), decisions + 1)
            })
            .collect()
    } else {
        assert!(
            matches!(
                status,
                Status::Complete(_) | Status::Winner(_) | Status::Draw(_)
            ),
            "{status:?}"
        );
        assert!(game.position.check_conservation());
        for viewer in Player::ALL {
            if let Some(tracker) = game.tracker(viewer) {
                assert!(tracker.check_truth(&game.position));
            }
        }
        if let Some(tracker) = game.public_tracker() {
            assert!(tracker.check_truth(&game.position));
        }
        vec![End {
            game,
            status,
            events,
            decisions,
        }]
    }
}
#[derive(PartialEq, Eq)]
struct Outcome {
    position: Position,
    rng: Rng,
    status: Status,
    beliefs: [Vec<Vec<crate::tracking::BeliefSlot>>; 3],
    views: [Vec<u32>; 2],
}
fn outcomes(ends: &[End]) -> Vec<Outcome> {
    let mut result = Vec::new();
    for end in ends {
        // Generate the position key and complete encoded views from the same
        // canonical piles; otherwise Hand arrival order only permutes the
        // next turn's Meld action rows. Trackers already canonicalize beliefs.
        let mut normalized = end.game.clone();
        normalized.position.sort_unordered_piles();
        let position = normalized.position.clone();
        let trackers = &end
            .game
            .knowledge
            .as_ref()
            .expect("tracked oracle")
            .trackers;
        let beliefs = std::array::from_fn(|observer| {
            crate::tracking::zones()
                .into_iter()
                .map(|zone| trackers[observer].belief(zone))
                .collect()
        });
        let item = Outcome {
            position,
            rng: end.game.rng.clone(),
            status: end.status.clone(),
            beliefs,
            views: Player::ALL.map(|p| {
                normalized
                    .observe(p)
                    .unwrap()
                    .row()
                    .into_iter()
                    .map(f32::to_bits)
                    .collect()
            }),
        };
        if !result.contains(&item) {
            result.push(item);
        }
    }
    result
}
pub(super) fn oracle(game: Game) -> (Vec<End>, Vec<End>) {
    assert!(
        game.knowledge.is_some(),
        "attach tracking before the oracle invocation"
    );
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let reduced = paths(game, Vec::new(), 0);
    let unrestricted = paths(manual, Vec::new(), 0);
    let a = outcomes(&reduced);
    let b = outcomes(&unrestricted);
    assert_eq!(a.len(), b.len());
    assert!(a.iter().all(|item| b.contains(item)));
    let mut traces = Vec::new();
    for end in &unrestricted {
        if !traces.contains(&end.events.as_slice()) {
            traces.push(end.events.as_slice());
        }
    }
    assert!(
        reduced
            .iter()
            .all(|end| traces.contains(&end.events.as_slice())),
        "every canonical execution preserves an actual unrestricted event trace"
    );
    (reduced, unrestricted)
}

#[test]
fn sunglasses_source_alias_oracle_keeps_destinations_and_reacting_shen_kuo() {
    for purple in [Splay::None, Splay::Right] {
        let mut f = Fixture::default();
        pile(&mut f, A, Color::Purple);
        let source = f.add(echoes_card("E035").unwrap(), Zone::Board(A, Color::Purple));
        for color in [Color::Green, Color::Yellow, Color::Blue] {
            pile(&mut f, A, color);
        }
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.position.players[A.index()].splays[Color::Purple as usize] = purple;
        game.position.players[A.index()].splays[Color::Green as usize] = Splay::Left;
        game.position.players[A.index()].splays[Color::Yellow as usize] = Splay::Left;
        game.position.players[A.index()].splays[Color::Blue as usize] = Splay::Right;
        effect(&mut game, source, 0);
        let mut peek = game.clone();
        let menu = decision(&mut peek, &mut Vec::new());
        assert_eq!(
            menu.options.len(),
            if purple == Splay::None { 3 } else { 4 }
        );
        assert_eq!(
            menu.options
                .iter()
                .filter(|option| matches!(
                    option,
                    Alternative::Splay {
                        player: A,
                        color: Color::Purple,
                        direction: Splay::Left
                    }
                ))
                .count(),
            1
        );
        if purple == Splay::Right {
            for color in [Color::Green, Color::Yellow] {
                assert!(menu.options.contains(&Alternative::Splay {
                    player: A,
                    color,
                    direction: Splay::Right
                }));
            }
            assert!(!menu.options.iter().any(|option| matches!(
                option,
                Alternative::Splay {
                    direction: Splay::None,
                    ..
                }
            )));
        }
        let (reduced, manual) = oracle(game);
        assert!(reduced.len() < manual.len());
        if purple == Splay::None {
            assert!(
                reduced
                    .iter()
                    .any(|end| end.events.iter().any(|event| matches!(
                        event,
                        Event::Splayed {
                            player: A,
                            color: Color::Purple,
                            to: Splay::Right,
                            inside_karma: true,
                            ..
                        }
                    )))
            );
        } else {
            assert!(
                reduced
                    .iter()
                    .all(|end| end.game.position.splay(A, Color::Purple) == Splay::Right),
                "Shen replaces every attempted left splay with the unchanged right splay"
            );
        }
    }
}

#[test]
fn copied_matching_splay_freezes_before_machiavelli_changes_its_source() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Blue);
    pile(&mut f, B, Color::Blue);
    f.figure(Figure::NiccoloMachiavelli, Zone::Board(A, Color::Purple));
    let source = f.tech(
        Color::Purple,
        1,
        Zone::Junk,
        vec![Instruction::Splay(SplayStep {
            who: Who::Executor,
            choices: SplayChoices::Options(&[SplayOption {
                colors: ColorSet::Only(&[Color::Blue]),
                direction: SplayDirection::Matching(Who::Opponent),
                from: None,
            }]),
            targets: SplayTargets::AnyColor,
            optional: false,
            all: false,
        })],
    );
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.position.players[B.index()].splays[Color::Blue as usize] = Splay::Right;
    game.self_execute(source, Context::new(A)).unwrap();
    let mut clone = game.clone();
    let mut events = Vec::new();
    for _ in 0..100 {
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(status, clone.advance(1, &mut expected).unwrap());
        assert_eq!(actual, expected);
        assert_eq!(game, clone);
        events.extend(actual);
        if matches!(status, Status::Complete(_)) {
            break;
        }
        assert_eq!(status, Status::WorkLimit);
    }
    assert_eq!(game.position.splay(B, Color::Blue), Splay::None);
    assert_eq!(game.position.splay(A, Color::Blue), Splay::Right);
    assert!(matches!(
        events.as_slice(),
        [
            Event::Splayed {
                player: B,
                to: Splay::None,
                inside_karma: true,
                ..
            },
            Event::Splayed {
                player: A,
                to: Splay::Right,
                inside_karma: false,
                ..
            }
        ]
    ));
}

#[test]
fn splay_targets_keep_board_owners_and_noops_without_firing_karma() {
    for policy in [
        SplayTargets::Changes,
        SplayTargets::Nonempty,
        SplayTargets::AnyColor,
    ] {
        let mut f = Fixture::default();
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        pile(&mut f, B, Color::Blue);
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        let source = f.tech(
            Color::Purple,
            1,
            Zone::Junk,
            vec![Instruction::Splay(SplayStep {
                who: Who::Either,
                choices: SplayChoices::Fixed {
                    colors: ColorSet::Only(&[Color::Blue, Color::Yellow]),
                    direction: Splay::Left,
                    from: None,
                },
                targets: policy,
                optional: false,
                all: false,
            })],
        );
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        // A's lone blue card cannot splay; B's blue pile can. A menu whose
        // every pile would stay as it is would not be asked at all.
        game.position.players[B.index()].splays[Color::Blue as usize] = Splay::Right;
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let status = game.advance(1000, &mut events).unwrap();
        if policy == SplayTargets::Changes {
            assert!(matches!(status, Status::Complete(_)));
            continue;
        }
        let Status::Decision(menu) = status else {
            panic!("{status:?}")
        };
        assert_eq!(
            menu.options.len(),
            if policy == SplayTargets::Nonempty {
                2
            } else {
                4
            }
        );
        for player in Player::ALL {
            assert!(menu.options.contains(&Alternative::Splay {
                player,
                color: Color::Blue,
                direction: Splay::Left
            }));
        }
        for viewer in Player::ALL {
            let mut viewed = game.clone();
            // Keep the same both-board alternatives and give either seat the
            // decision, so the option owner must be encoded relative to it.
            let Some(Frame::Choose { player, .. }) = viewed.frames.last_mut() else {
                panic!("missing decision")
            };
            *player = viewer;
            let observation = viewed.observe(viewer).unwrap();
            for (option, row) in menu.options.iter().zip(&observation.options) {
                let Alternative::Splay { player, .. } = option else {
                    unreachable!()
                };
                assert_eq!(
                    row[observation::field::OWNER],
                    if *player == viewer { 1.0 } else { 2.0 }
                );
            }
        }
        for choice in 0..menu.options.len() {
            let mut fork = game.clone();
            fork.choose(choice).unwrap();
            let ends = paths(fork, events.clone(), 0);
            assert_eq!(ends.len(), 1);
            let real = matches!(
                menu.options[choice],
                Alternative::Splay {
                    player: B,
                    color: Color::Blue,
                    ..
                }
            );
            if real {
                assert!(
                    ends[0]
                        .events
                        .iter()
                        .all(|event| matches!(event, Event::Splayed { .. }))
                );
            } else {
                assert!(
                    ends[0].events.is_empty(),
                    "impossible splay has no Would karma"
                );
            }
        }
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

fn forecast_game(opponent: bool, existing: usize, hedy: bool) -> (Game, Vec<CardId>) {
    let mut f = Fixture::default();
    let (code, index) = if opponent { ("E032", 1) } else { ("E029", 2) };
    let mut source = echoes_card(code).unwrap();
    source.echo = None;
    source.effects = vec![source.effects[index].clone()];
    f.add(source, Zone::Forecast(A));
    for _ in 0..existing {
        f.tech(Color::Red, 4, Zone::Forecast(A), vec![]);
    }
    let origin = if opponent {
        Zone::Forecast(B)
    } else {
        Zone::Hand(A)
    };
    let incoming = [4, 4, 5]
        .map(|age| f.tech(Color::Green, age, origin, vec![]))
        .to_vec();
    if hedy {
        f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
        pile(&mut f, A, Color::Purple);
    }
    let driver = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let mut game = Game::seeded(
        f.position(),
        if hedy { BOTH } else { Expansions::ECHOES },
        27,
    )
    .unwrap()
    .with_tracking()
    .unwrap();
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    (game, incoming)
}

#[test]
fn forecast_batches_match_every_path_only_when_all_fit_with_real_hedy_claim() {
    for opponent in [false, true] {
        for existing in [2, 3, 4] {
            let (game, incoming) = forecast_game(opponent, existing, existing == 2);
            let (reduced, manual) = oracle(game);
            if existing == 2 {
                assert!(
                    reduced.iter().map(|end| end.decisions).min()
                        < manual.iter().map(|end| end.decisions).min()
                );
            } else if existing == 3 {
                assert_eq!(reduced.len(), manual.len(), "proper subset choices remain");
            } else {
                assert!(
                    reduced.len() < manual.len(),
                    "full Forecast suffixes have no order decision"
                );
            }
            for end in reduced.iter().chain(&manual) {
                assert_eq!(
                    incoming
                        .iter()
                        .filter(|card| end.game.position.locate(**card) == Some(Zone::Forecast(A)))
                        .count(),
                    5 - existing
                );
                assert_eq!(
                    end.game.position.special_location(Special::Destiny),
                    SpecialLocation::Owned(A)
                );
            }
            // Hedy's splay may name a color it cannot change (a no-op choice
            // is allowed), so some paths splay and some do not.
            if existing == 2 {
                assert!(
                    reduced
                        .iter()
                        .chain(&manual)
                        .any(|end| end.events.iter().any(|event| matches!(
                            event,
                            Event::Splayed {
                                player: A,
                                to: Splay::Up,
                                inside_karma: true,
                                ..
                            }
                        ))),
                    "Hedy reacts to Destiny after the batch"
                );
            }
        }
    }
}

#[test]
fn opposing_forecast_batches_offer_only_backs_and_preserve_native_sampling() {
    let (mut game, incoming) = forecast_game(true, 3, false);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(
        menu.options,
        [4, 5].map(|age| Alternative::BlindMove {
            verb: Verb::Foreshadow,
            source: Zone::Forecast(B),
            back: Deck::base(age)
        })
    );
    let mut rng = game.rng.clone();
    let expected = incoming[(rng.next_u64() % 2) as usize];
    for seed in 0..24 {
        let world = game.determinize(A, seed).unwrap();
        assert_eq!(game.observe(A).unwrap(), world.observe(A).unwrap());
        let PromptView::Decision(sampled) = world.prompt_view() else {
            panic!("sample lost decision")
        };
        assert_eq!(sampled, menu);
    }
    game.choose(0).unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(game.position.locate(expected), Some(Zone::Forecast(A)));
    assert_eq!(game.rng, rng);
    assert_eq!(menu.options.len(), 2);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Revealed { .. }))
    );
}

#[test]
fn copied_splay_front_dependencies_include_both_source_and_destination() {
    const BOUND: ColorSet = ColorSet::Of(Reg::Bound);
    const BLUE: ColorSet = ColorSet::Only(&[Color::Blue]);
    let cases = [
        (
            SplayChoices::Fixed {
                colors: BOUND,
                direction: Splay::Left,
                from: None,
            },
            true,
        ),
        (
            SplayChoices::Options(&[SplayOption {
                colors: BOUND,
                direction: SplayDirection::Matching(Who::Opponent),
                from: None,
            }]),
            true,
        ),
        (
            SplayChoices::Options(&[SplayOption {
                colors: BLUE,
                direction: SplayDirection::Copy {
                    who: Who::Executor,
                    colors: BOUND,
                    splayed_only: true,
                },
                from: None,
            }]),
            true,
        ),
        (
            SplayChoices::Options(&[SplayOption {
                colors: BLUE,
                direction: SplayDirection::Copy {
                    who: Who::Opponent,
                    colors: BLUE,
                    splayed_only: false,
                },
                from: None,
            }]),
            false,
        ),
        (
            SplayChoices::Options(&[SplayOption {
                colors: BLUE,
                direction: SplayDirection::Fixed(Splay::Right),
                from: None,
            }]),
            false,
        ),
    ];
    for (choices, expected) in cases {
        let body = Box::leak(Box::new([Instruction::Splay(SplayStep {
            who: Who::Executor,
            choices,
            targets: SplayTargets::AnyColor,
            optional: false,
            all: false,
        })]));
        let karma = Karma {
            body: Body::Program(body),
            ..Figure::ShenKuo.def().karma[0]
        };
        assert_eq!(karma.body_reads_card_front(), expected, "{choices:?}");
    }
}
