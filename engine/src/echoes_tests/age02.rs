//! Remaining age-two clauses, blind junk claims and exhaustive shortcut proofs.
use super::*;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn source(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = echoes_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn figure(f: &mut Fixture, figure: Figure, owner: Player) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, Vec::new())
}
fn deck(f: &mut Fixture, set: CardSet, age: u8) -> CardId {
    let id = tech(f, Color::Blue, age, Zone::Deck(Deck { set, age }));
    f.cards[usize::from(id.0)].set = set;
    id
}
fn start(game: &mut Game, card: CardId, effect: u16, context: Context, increase: u8) {
    let mut boxed = BoxedValues::ZERO;
    boxed.increase(increase).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed,
        },
        context,
    )
    .unwrap();
}
fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut choose: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..100 {
        let mut clone = game.clone();
        let mut next = Vec::new();
        let status = game.advance(1000, &mut next).unwrap();
        let mut expected = Vec::new();
        assert_eq!(clone.advance(1000, &mut expected).unwrap(), status);
        assert_eq!(clone, *game);
        assert_eq!(expected, next);
        events.extend(next);
        match status {
            Status::Decision(menu) => game.choose(choose(&menu)).unwrap(),
            status => {
                assert!(game.position.check_conservation());
                for viewer in Player::ALL {
                    if let Some(tracker) = game.tracker(viewer) {
                        assert!(tracker.check_truth(&game.position));
                    }
                }
                return status;
            }
        }
    }
    panic!("age-two effect did not finish")
}
fn known(menu: &Decision) -> Vec<CardId> {
    menu.options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(*card),
            _ => None,
        })
        .collect()
}
#[derive(Debug, PartialEq, Eq)]
struct Ending {
    position: Position,
    status: Status,
    rng: Rng,
    events: Vec<Event>,
    beliefs: Vec<Vec<crate::BeliefSlot>>,
    views: [Vec<u32>; 2],
}
type Outcomes = Vec<Ending>;

fn knowledge(game: &Game) -> (Vec<Vec<crate::BeliefSlot>>, [Vec<u32>; 2]) {
    let mut beliefs = Vec::new();
    for tracker in [game.tracker(A), game.tracker(B), game.public_tracker()] {
        let tracker = tracker.expect("oracle starts tracking before invocation");
        assert!(tracker.check_truth(&game.position));
        for zone in crate::tracking::zones() {
            beliefs.push(tracker.belief(zone));
        }
    }
    let views = Player::ALL.map(|viewer| {
        game.observe(viewer)
            .unwrap()
            .row()
            .into_iter()
            .map(f32::to_bits)
            .collect()
    });
    (beliefs, views)
}

fn leaves(mut game: Game, depth: usize, mut events: Vec<Event>, independent: bool) -> Outcomes {
    assert!(depth < 12);
    match game.advance(1000, &mut events).unwrap() {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                leaves(fork, depth + 1, events.clone(), independent)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            let (beliefs, views) = knowledge(&game);
            game.position.sort_unordered_piles();
            if independent {
                // These phases contain only the fixed original moves. Their
                // cross-color/deck ordering is exactly the equivalence proved;
                // every movement field and every ordered board/deck stay checked.
                events.sort_by_key(|event| match event {
                    Event::Moved(movement) => movement.card,
                    _ => panic!("independent phase had an observable interruption"),
                });
            }
            vec![Ending {
                position: game.position,
                status,
                rng: game.rng,
                events,
                beliefs,
                views,
            }]
        }
    }
}
fn oracle(game: Game, independent: bool) -> (Outcomes, Outcomes) {
    assert!(game.tracker(A).is_some());
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let reduced = leaves(game, 0, Vec::new(), independent);
    let expanded = leaves(manual, 0, Vec::new(), independent);
    for (left, right) in [(&reduced, &expanded), (&expanded, &reduced)] {
        assert!(left.iter().all(|outcome| right.contains(outcome)));
    }
    (reduced, expanded)
}

#[test]
fn horseshoes_echo_keeps_both_boxes_decline_and_capacity() {
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
    for tracked in [false, true] {
        for increase in [0, 1] {
            for choice in [0, 1, 2] {
                for full in [false, true] {
                    let mut f = Fixture::default();
                    let card = source(&mut f, "E017", A);
                    let cards = [
                        deck(&mut f, CardSet::Base, 2 + increase),
                        deck(&mut f, CardSet::Base, 3 + increase),
                    ];
                    if full {
                        for _ in 0..5 {
                            tech(&mut f, Color::Red, 1, Zone::Forecast(A));
                        }
                    }
                    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 37).unwrap();
                    if tracked {
                        game = game.with_tracking().unwrap();
                    }
                    start(&mut game, card, Card::ECHO, Context::new(A), increase);
                    let mut events = Vec::new();
                    let menu = decision(&mut game, &mut events);
                    assert_eq!(
                        menu.options.as_slice(),
                        [
                            Alternative::Value(2 + increase),
                            Alternative::Value(3 + increase),
                            Alternative::Decline
                        ]
                    );
                    game.choose(choice).unwrap();
                    finish(&mut game, &mut events, |_| 0);
                    for (index, drawn) in cards.into_iter().enumerate() {
                        assert_eq!(
                            game.position.locate(drawn),
                            Some(if choice == index {
                                if full {
                                    Zone::Hand(A)
                                } else {
                                    Zone::Forecast(A)
                                }
                            } else {
                                Zone::Deck(Deck::base(2 + increase + index as u8))
                            })
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn horseshoes_demand_excludes_either_icon_and_rewards_only_success() {
    for available in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E017", B);
        let mut allowed = Vec::new();
        for (index, color) in Color::ALL.into_iter().enumerate() {
            let target = tech(&mut f, color, 2, Zone::Board(A, color));
            if index < 3 || !available {
                f.cards[usize::from(target.0)].symbols[0] = Symbol::Icon(if index == 1 {
                    Icon::Industry
                } else {
                    Icon::Authority
                });
                if index == 2 {
                    f.cards[usize::from(target.0)].symbols[1] = Symbol::Icon(Icon::Industry);
                }
            } else {
                allowed.push(target);
            }
        }
        let reward = deck(&mut f, CardSet::Base, 2);
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        let mut context = Context::new(A);
        context.demander = B;
        start(&mut game, card, 0, context, 0);
        let mut events = Vec::new();
        if available {
            let menu = decision(&mut game, &mut events);
            assert_eq!(known(&menu), allowed);
            game.choose(1).unwrap();
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(reward),
            Some(if available {
                Zone::Board(A, Color::Blue)
            } else {
                Zone::Deck(Deck::base(2))
            })
        );
        if available {
            let chosen = allowed[1];
            assert_eq!(
                game.position.locate(chosen),
                Some(Zone::Board(B, game.position.card(chosen).color))
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Transfer)))
                    .count(),
                1
            );
        } else {
            assert!(events.is_empty());
        }
    }
}

#[test]
fn toothbrush_tucks_one_value_by_color_with_every_same_color_order_and_reacting_figure() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E018", A);
        let cards: Vec<_> = [Color::Red, Color::Red, Color::Blue]
            .into_iter()
            .map(|color| tech(&mut f, color, 2, Zone::Hand(A)))
            .collect();
        f.cards[usize::from(cards[1].0)].set = CardSet::Echoes;
        let untouched = tech(&mut f, Color::Green, 3, Zone::Hand(A));
        let reward = if reacting {
            figure(&mut f, Figure::SunshuAo, A);
            Some(deck(&mut f, CardSet::Base, 1))
        } else {
            None
        };
        let earlier: Vec<_> = if reacting {
            (0..5)
                .map(|_| tech(&mut f, Color::Purple, 4, Zone::Hand(A)))
                .collect()
        } else {
            Vec::new()
        };
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        for earlier in earlier {
            game.start(
                Operation::Move {
                    verb: Verb::Tuck,
                    target: target(earlier, Zone::Hand(A)),
                    destination: Zone::Board(A, Color::Purple),
                },
                Context::new(A),
            )
            .unwrap();
            finish(&mut game, &mut Vec::new(), |_| 0);
        }
        start(&mut game, card, Card::ECHO, Context::new(A), 1);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options.as_slice(),
            [Alternative::Value(2), Alternative::Value(3)]
        );
        game.choose(0).unwrap();
        let (reduced, expanded) = oracle(game, !reacting);
        assert_eq!(expanded.len(), 6);
        assert_eq!(reduced.len(), if reacting { 6 } else { 2 });
        for ending in reduced {
            let (position, status) = (ending.position, ending.status);
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(position.cards(Zone::Hand(A)), [untouched]);
            assert_eq!(position.cards(Zone::Board(A, Color::Red)).len(), 2);
            if let Some(reward) = reward {
                assert_eq!(position.locate(reward), Some(Zone::Achievements(A)));
            }
        }
    }
}

#[test]
fn toothbrush_splay_is_optional_and_rejects_singletons() {
    for decline in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E018", A);
        for _ in 0..2 {
            tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        }
        let mut position = f.position();
        position.players[A.index()].splays[Color::Red as usize] = Splay::Right;
        let mut game = Game::new(position, Expansions::ECHOES).unwrap();
        start(&mut game, card, 0, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options.as_slice(),
            [
                Alternative::Splay {
                    player: A,
                    color: Color::Red,
                    direction: Splay::Left
                },
                Alternative::Decline
            ]
        );
        game.choose(usize::from(decline)).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.splay(A, Color::Red),
            if decline { Splay::Right } else { Splay::Left }
        );
        assert_eq!(game.position.splay(A, Color::Yellow), Splay::None);
    }
}

#[test]
fn toothbrush_junks_only_base_box_then_checks_highest_before_eligibility() {
    for increase in [0, 1] {
        for mode in 0..4 {
            let mut f = Fixture::default();
            let card = source(&mut f, "E018", A);
            tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
            tech(&mut f, Color::Green, 11, Zone::Score(A));
            tech(&mut f, Color::Green, 2, Zone::Score(A));
            let highest = tech(
                &mut f,
                Color::Purple,
                if mode == 2 { 4 } else { 3 },
                Zone::Junk,
            );
            let lower = tech(&mut f, Color::Red, 1, Zone::Junk);
            let junked = (mode != 3).then(|| deck(&mut f, CardSet::Base, 2 + increase));
            let echo = deck(&mut f, CardSet::Echoes, 2 + increase);
            let mut game = Game::new(f.position(), Expansions::ECHOES)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, card, 1, Context::new(A), increase);
            let mut events = Vec::new();
            if mode != 3 {
                let menu = decision(&mut game, &mut events);
                assert!(menu.options.contains(&Alternative::Decline));
                game.choose(if mode == 1 { menu.options.len() - 1 } else { 0 })
                    .unwrap();
            }
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(
                game.position.cards(Zone::Achievements(A)).len(),
                usize::from(mode == 0)
            );
            if mode == 0 {
                assert!(
                    game.position
                        .cards(Zone::Achievements(A))
                        .contains(&highest)
                        || increase == 1
                );
            }
            assert_eq!(game.position.locate(lower), Some(Zone::Junk));
            assert_eq!(
                game.position.locate(echo),
                Some(Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 2 + increase
                }))
            );
            if let Some(junked) = junked {
                assert_eq!(
                    game.position.locate(junked) == Some(Zone::Deck(Deck::base(2 + increase))),
                    mode == 1
                );
            }
            assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
        }
    }
}

#[test]
fn age_two_deck_sweeps_match_every_disabled_path_including_rod_serling() {
    for code in ["E018", "E020"] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let card = source(&mut f, code, A);
            if reacting {
                f.figure(Figure::RodSerling, Zone::Board(B, Color::Purple));
            }
            for _ in 0..3 {
                deck(&mut f, CardSet::Base, 2);
            }
            // An older higher back leaves the eventual claim cohort unchanged;
            // the separate three-card case also takes the deck top now.
            if code == "E018" {
                tech(&mut f, Color::Green, 3, Zone::Junk);
            }
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, card, 1, Context::new(A), 0);
            if code == "E020" {
                let menu = decision(&mut game, &mut Vec::new());
                assert_eq!(
                    menu.options.as_slice(),
                    [Alternative::Value(2), Alternative::Value(3)]
                );
                game.choose(0).unwrap();
            }
            let (reduced, expanded) = oracle(game, !reacting);
            if !reacting {
                assert_eq!(reduced.len(), if code == "E018" { 2 } else { 1 });
                assert_eq!(expanded.len(), if code == "E018" { 7 } else { 6 });
            } else {
                assert_eq!(reduced.len(), expanded.len());
                assert!(
                    reduced
                        .iter()
                        .any(|ending| matches!(ending.status, Status::Winner(B)))
                );
            }
        }
    }
}

#[test]
fn glassblowing_scores_every_expansion_set_and_no_base_card() {
    for set in [CardSet::Echoes, CardSet::Figures, CardSet::Cities] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E020", A);
        let base = tech(&mut f, Color::Red, 1, Zone::Hand(A));
        let expansion = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
        f.cards[usize::from(expansion.0)].set = set;
        let mut game = Game::new(f.position(), BOTH.union(Expansions::CITIES))
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, card, Card::ECHO, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Hand(A)), [base]);
        assert_eq!(game.position.cards(Zone::Score(A)), [expansion]);
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
    }
}

#[test]
fn glassblowing_uses_lowest_non_green_top_or_zero_and_draws_when_forecast_full() {
    for other in [false, true] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let card = source(&mut f, "E020", A);
            tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
            if other {
                tech(&mut f, Color::Red, 7, Zone::Board(A, Color::Red));
                tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
            }
            if full {
                for _ in 0..5 {
                    tech(&mut f, Color::Purple, 1, Zone::Forecast(A));
                }
            }
            let drawn = deck(&mut f, CardSet::Base, if other { 6 } else { 3 });
            deck(&mut f, CardSet::Base, 9);
            let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
            start(&mut game, card, 0, Context::new(A), 1);
            finish(&mut game, &mut Vec::new(), |_| 0);
            assert_eq!(
                game.position.locate(drawn),
                Some(if full {
                    Zone::Hand(A)
                } else {
                    Zone::Forecast(A)
                })
            );
        }
    }
}

#[test]
fn glassblowing_keeps_nonempty_deck_choices_and_elides_only_two_empty_boxes() {
    for increase in [0, 1] {
        for empty_mask in 0..4 {
            let mut f = Fixture::default();
            let card = source(&mut f, "E020", A);
            let mut cards = Vec::new();
            for index in 0..2 {
                if empty_mask & (1 << index) == 0 {
                    cards.push(deck(&mut f, CardSet::Base, 2 + increase + index));
                }
            }
            let echo = deck(&mut f, CardSet::Echoes, 2 + increase);
            let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
            start(&mut game, card, 1, Context::new(A), increase);
            if empty_mask != 3 {
                let menu = decision(&mut game, &mut Vec::new());
                assert_eq!(
                    menu.options.as_slice(),
                    [
                        Alternative::Value(2 + increase),
                        Alternative::Value(3 + increase)
                    ]
                );
                game.choose(1).unwrap();
            }
            let mut events = Vec::new();
            let mut menus = 0;
            assert!(matches!(
                game.advance(1000, &mut events).unwrap(),
                Status::Complete(_)
            ));
            finish(&mut game, &mut events, |_| {
                menus += 1;
                0
            });
            assert_eq!(menus, 0);
            for target in cards {
                let age = game.position.card(target).value;
                assert_eq!(
                    game.position.locate(target),
                    Some(if age == 3 + increase {
                        Zone::Junk
                    } else {
                        Zone::Deck(Deck::base(age))
                    })
                );
            }
            assert_eq!(
                game.position.locate(echo),
                Some(Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 2 + increase
                }))
            );
        }
    }
}

#[test]
fn chaturanga_draws_melded_value_or_zero_using_current_board_even_at_capacity() {
    for empty in [false, true] {
        for zero in [false, true] {
            for full in [false, true] {
                let mut f = Fixture::default();
                let card = source(&mut f, "E025", A);
                let melded = (!empty).then(|| tech(&mut f, Color::Red, 2, Zone::Hand(A)));
                if full {
                    for _ in 0..5 {
                        tech(&mut f, Color::Green, 4, Zone::Forecast(A));
                    }
                }
                let drawn = deck(
                    &mut f,
                    CardSet::Base,
                    if empty { u8::from(!zero) } else { 2 },
                );
                let echo = if !empty {
                    deck(&mut f, CardSet::Base, 1); // zero must not alias the successful meld's age
                    Some(deck(&mut f, CardSet::Echoes, 2))
                } else {
                    None
                };
                let mut game = Game::new(f.position(), BOTH)
                    .unwrap()
                    .with_tracking()
                    .unwrap();
                start(&mut game, card, 0, Context::new(A), 1);
                finish(&mut game, &mut Vec::new(), |_| 0);
                assert_eq!(
                    game.position.locate(drawn),
                    Some(if full {
                        Zone::Hand(A)
                    } else {
                        Zone::Forecast(A)
                    })
                );
                if let Some(melded) = melded {
                    assert_eq!(
                        game.position.locate(melded),
                        Some(Zone::Board(A, Color::Red))
                    );
                }
                if let Some(echo) = echo {
                    assert_eq!(
                        game.position.locate(echo),
                        Some(Zone::Deck(Deck {
                            set: CardSet::Echoes,
                            age: 2
                        }))
                    );
                }
            }
        }
    }
}

#[test]
fn chaturanga_foreseen_counts_colors_after_its_meld_and_ordinary_effect_is_empty() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let card = f.add(
            echoes_card("E025").unwrap(),
            if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Board(A, Color::Purple)
            },
        );
        let promotion = tech(&mut f, Color::Red, 4, Zone::Hand(A));
        let melded = tech(&mut f, Color::Green, 3, Zone::Hand(A));
        tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        let cards = [
            deck(&mut f, CardSet::Base, 3),
            deck(&mut f, CardSet::Base, 3),
        ];
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        if foreseen {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, card, 1, Context::new(A), 0);
        }
        finish(&mut game, &mut Vec::new(), |_| 0);
        assert_eq!(
            game.position.cards(Zone::Forecast(A)).len(),
            if foreseen { 2 } else { 0 }
        );
        for drawn in cards {
            assert_eq!(
                game.position.locate(drawn),
                Some(if foreseen {
                    Zone::Forecast(A)
                } else {
                    Zone::Deck(Deck::base(3))
                })
            );
        }
        if foreseen {
            assert_eq!(
                game.position.locate(melded),
                Some(Zone::Board(A, Color::Green))
            );
        }
    }
}

#[test]
fn toothbrush_junk_claim_is_grouped_by_back_and_private_through_hedys_first() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E018", A);
        f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
        for color in [Color::Red, Color::Blue] {
            tech(&mut f, color, 2, Zone::Board(A, color));
            tech(&mut f, color, 3, Zone::Board(A, color));
        }
        tech(&mut f, Color::Purple, 11, Zone::Score(A));
        tech(&mut f, Color::Purple, 2, Zone::Score(A));
        deck(&mut f, CardSet::Base, 2);
        let base: Vec<_> = (0..2)
            .map(|_| tech(&mut f, Color::Purple, 3, Zone::Junk))
            .collect();
        let echo = tech(&mut f, Color::Purple, 3, Zone::Junk);
        f.cards[usize::from(echo.0)].set = CardSet::Echoes;
        let mut game = Game::seeded(f.position(), BOTH, 71).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        start(&mut game, card, 1, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.last(), Some(&Alternative::Decline));
        game.choose(0).unwrap();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options.as_slice(),
            [CardSet::Base, CardSet::Echoes].map(|set| Alternative::BlindMove {
                verb: Verb::Achieve,
                source: Zone::Junk,
                back: Deck { set, age: 3 },
            })
        );
        let mut rng = game.rng.clone();
        let selected = base[(rng.next_u64() % 2) as usize];
        game.choose(0).unwrap();
        let first = decision(&mut game, &mut events);
        assert!(
            first
                .options
                .iter()
                .all(|option| matches!(option, Alternative::Splay { .. }))
        );
        assert_eq!(game.rng, rng);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Revealed { .. }))
        );
        assert!(game.position.cards(Zone::Achievements(A)).is_empty());
        if tracked {
            for viewer in Player::ALL {
                assert!(
                    !game
                        .tracker(viewer)
                        .unwrap()
                        .knows(&game.position, selected, Zone::Junk)
                );
                if viewer != A {
                    continue;
                }
                for seed in 0..16 {
                    for mut sample in [
                        game.determinize(viewer, seed).unwrap(),
                        game.sample_world(viewer, seed).unwrap().world,
                    ] {
                        let mut sampled_events = Vec::new();
                        finish(&mut sample, &mut sampled_events, |_| 0);
                        let achieved = sample.position.cards(Zone::Achievements(A));
                        assert_eq!(achieved.len(), 1);
                        assert_eq!(
                            sample.position.card(achieved[0]).origin_deck(),
                            Deck::base(3)
                        );
                        assert!(
                            !sampled_events
                                .iter()
                                .any(|event| matches!(event, Event::Revealed { .. }))
                        );
                    }
                }
            }
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Achievements(A)), [selected]);
        assert_eq!(game.position.locate(echo), Some(Zone::Junk));
        assert_eq!(game.position.splay(A, Color::Red), Splay::Up);
    }
}

#[test]
fn chaturanga_replaced_meld_uses_zero_not_napoleons_child_movement() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E025", A);
    figure(&mut f, Figure::NapoleonBonaparte, A);
    let melded = tech(&mut f, Color::Green, 2, Zone::Hand(A));
    let zero = deck(&mut f, CardSet::Base, 1);
    let wrong = deck(&mut f, CardSet::Base, 2);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, card, 0, Context::new(A), 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.locate(melded),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(game.position.locate(zero), Some(Zone::Forecast(A)));
    assert_eq!(game.position.locate(wrong), Some(Zone::Deck(Deck::base(2))));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Moved(m) if m.card == melded && m.inside_karma))
    );
}

#[test]
fn glassblowing_empty_alias_matches_both_unreduced_choices_with_live_figure() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E020", A);
    figure(&mut f, Figure::Archimedes, A);
    let mut reduced = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    let original_catalog = reduced.position.catalog.clone();
    let mut expanded = reduced.clone();
    expanded.shortcuts_disabled = true;
    // Explicit reference program retains the two printed, empty-deck choices.
    std::sync::Arc::make_mut(&mut expanded.position.catalog)[usize::from(card.0)].effects[1]
        .instructions
        .remove(0);
    start(&mut reduced, card, 1, Context::new(A), 0);
    start(&mut expanded, card, 1, Context::new(A), 0);
    let mut events = Vec::new();
    assert_eq!(
        reduced.advance(1000, &mut events).unwrap(),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    let menu = decision(&mut expanded, &mut Vec::new());
    assert_eq!(
        menu.options,
        [Alternative::Value(3), Alternative::Value(4)],
        "Archimedes actually increases both boxes"
    );
    for choice in 0..2 {
        let mut branch = expanded.clone();
        branch.choose(choice).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut branch, &mut events, |_| 0),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(events.is_empty());
        branch.position.catalog = original_catalog.clone();
        branch.position.sort_unordered_piles();
        assert_eq!(branch.position, reduced.position);
        assert_eq!(branch.rng, reduced.rng);
        assert_eq!(knowledge(&branch), knowledge(&reduced));
    }
}

#[test]
fn toothbrush_fresh_three_card_sweep_takes_the_deck_top() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E018", A);
    tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
    tech(&mut f, Color::Red, 10, Zone::Score(A));
    for _ in 0..3 {
        deck(&mut f, CardSet::Base, 2);
    }
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, card, 1, Context::new(A), 0);
    let initial_rng = game.rng.clone();
    let (mut stepped_rng, mut clone) = (initial_rng.clone(), game.clone());
    stepped_rng.next_u64();
    let menu = decision(&mut clone, &mut Vec::new());
    assert_eq!(menu.options.len(), 2, "junk the deck top, or decline");
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let reduced = leaves(game, 0, Vec::new(), false);
    let expanded = leaves(manual, 0, Vec::new(), false);
    assert_eq!(reduced.len(), 2);
    assert_eq!(expanded.len(), 7);
    assert!(reduced.iter().all(|outcome| expanded.contains(outcome)));
    for ending in reduced.iter().chain(&expanded) {
        if ending.position.cards(Zone::Achievements(A)).is_empty() {
            assert_eq!(ending.rng, initial_rng);
        } else {
            assert_eq!(ending.position.cards(Zone::Achievements(A)).len(), 1);
            assert_eq!(ending.rng, stepped_rng);
            assert_eq!(ending.position.cards(Zone::Junk).len(), 2);
        }
    }
}
