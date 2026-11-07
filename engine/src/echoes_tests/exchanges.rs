//! Telescope, its atomic Figure replacement, and private exchange admission.
use super::*;
use common::*;

fn echo_deck(age: u8) -> Deck {
    Deck {
        set: CardSet::Echoes,
        age,
    }
}

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn card(f: &mut Fixture, set: CardSet, value: u8, color: Color, zone: Zone) -> CardId {
    let id = f.tech(color, value, zone, Vec::new());
    f.cards[usize::from(id.0)].set = set;
    id
}

fn telescope(f: &mut Fixture) -> CardId {
    f.add(echoes_card("E043").unwrap(), Zone::Board(A, Color::Blue))
}

/// Telescope exchanging with every available achievement, Figure board
/// aliases included. Printed Telescope says "standard"; this variant keeps
/// the alias exchange machinery covered.
fn alias_telescope(f: &mut Fixture) -> CardId {
    let mut card = echoes_card("E043").unwrap();
    for effect in &mut card.effects {
        for instruction in &mut effect.instructions {
            if let Instruction::Exchange(step) = instruction {
                step.right = Selector::AvailableAchievements;
            }
        }
    }
    f.add(card, Zone::Board(A, Color::Blue))
}

fn start(game: &mut Game, source: CardId, effect: u16) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        let mut cloned = game.clone();
        let mut next = Vec::new();
        let status = game.advance(1000, &mut next).unwrap();
        let mut cloned_events = Vec::new();
        assert_eq!(cloned.advance(1000, &mut cloned_events).unwrap(), status);
        assert_eq!(cloned_events, next);
        assert_eq!(cloned, *game);
        events.extend(next);
        if let Status::Decision(_) = status {
            game.choose(0).unwrap();
        } else {
            assert!(game.position.check_conservation());
            for player in Player::ALL {
                if let Some(tracker) = game.tracker(player) {
                    assert!(tracker.check_truth(&game.position));
                }
            }
            return status;
        }
    }
    panic!("exchange failed to finish");
}

fn place(game: &mut Game, events: &mut Vec<Event>, placed: CardId) {
    let menu = decision(game, events);
    let index = menu.options.iter().position(|option| matches!(option,
        Alternative::Move { verb: Verb::Place, target: TargetDescriptor::Known { card, .. }, .. } if *card == placed)).unwrap();
    game.choose(index).unwrap();
}

#[test]
fn telescope_optional_placement_is_top_not_return_and_gates_all_followups() {
    for decline in [false, true] {
        let mut f = Fixture::default();
        let source = telescope(&mut f);
        let placed = card(&mut f, CardSet::Echoes, 1, Color::Red, Zone::Forecast(A));
        let bottom = card(
            &mut f,
            CardSet::Echoes,
            1,
            Color::Green,
            Zone::Deck(echo_deck(1)),
        );
        let junked = card(
            &mut f,
            CardSet::Base,
            5,
            Color::Yellow,
            Zone::Deck(Deck::base(5)),
        );
        let untouched = card(
            &mut f,
            CardSet::Echoes,
            5,
            Color::Yellow,
            Zone::Deck(echo_deck(5)),
        );
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        let before = game.position.clone();
        start(&mut game, source, 0);
        let mut events = Vec::new();
        if decline {
            let menu = decision(&mut game, &mut events);
            game.choose(
                menu.options
                    .iter()
                    .position(|o| *o == Alternative::Decline)
                    .unwrap(),
            )
            .unwrap();
        } else {
            place(&mut game, &mut events, placed);
        }
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if decline {
            assert_eq!(game.position, before);
            assert!(events.is_empty());
        } else {
            assert_eq!(
                game.position.cards(Zone::Deck(echo_deck(1))),
                [bottom, placed]
            );
            assert!(game.position.cards(Zone::Junk).contains(&junked));
            assert_eq!(game.position.cards(Zone::Deck(echo_deck(5))), [untouched]);
            assert!(events.iter().any(|event| matches!(event, Event::Moved(m) if m.card == placed && m.verb == Some(Verb::Place))));
            assert!(!events.iter().any(|event| matches!(
                event,
                Event::Exchanged { .. }
                    | Event::Moved(Movement {
                        verb: Some(Verb::Return | Verb::Transfer),
                        ..
                    })
            )));
        }
    }
}

#[test]
fn telescope_echo_resolves_base_age_before_explicit_set_and_keeps_refused_draw_in_hand() {
    for full in [false, true] {
        let mut f = Fixture::default();
        let source = telescope(&mut f);
        // Tie Telescope's top value, so an ordinary draw would stay Base.
        card(
            &mut f,
            CardSet::Base,
            4,
            Color::Red,
            Zone::Board(A, Color::Red),
        );
        if full {
            for _ in 0..5 {
                card(&mut f, CardSet::Base, 1, Color::Red, Zone::Forecast(A));
            }
        }
        let skipped = card(
            &mut f,
            CardSet::Echoes,
            5,
            Color::Green,
            Zone::Deck(echo_deck(5)),
        );
        card(
            &mut f,
            CardSet::Base,
            6,
            Color::Green,
            Zone::Deck(Deck::base(6)),
        );
        let drawn = card(
            &mut f,
            CardSet::Echoes,
            6,
            Color::Yellow,
            Zone::Deck(echo_deck(6)),
        );
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        start(&mut game, source, Card::ECHO);
        finish(&mut game, &mut Vec::new());
        assert_eq!(
            game.position.locate(drawn),
            Some(if full {
                Zone::Hand(A)
            } else {
                Zone::Forecast(A)
            })
        );
        assert_eq!(game.position.cards(Zone::Deck(echo_deck(5))), [skipped]);
    }
}

#[test]
fn telescope_exchanges_all_outgoing_even_when_supply_or_capacity_is_short() {
    for (outgoing_count, supply_count, limited) in [
        (0, 3, false),
        (3, 0, false),
        (3, 1, false),
        (3, 4, false),
        (3, 4, true),
    ] {
        let mut f = Fixture::default();
        let source = telescope(&mut f);
        let placed = card(&mut f, CardSet::Base, 1, Color::Blue, Zone::Forecast(A));
        let outgoing: Vec<_> = (0..outgoing_count)
            .map(|i| {
                card(
                    &mut f,
                    CardSet::Echoes,
                    2 + i,
                    Color::Green,
                    Zone::Forecast(A),
                )
            })
            .collect();
        let supply: Vec<_> = (0..supply_count)
            .map(|i| {
                card(
                    &mut f,
                    CardSet::Base,
                    6 + i,
                    Color::Yellow,
                    Zone::AvailableAchievements,
                )
            })
            .collect();
        let junked = card(
            &mut f,
            CardSet::Base,
            5,
            Color::Blue,
            Zone::Deck(Deck::base(5)),
        );
        if limited {
            for _ in 0..2 {
                card(
                    &mut f,
                    CardSet::Base,
                    1,
                    Color::Red,
                    Zone::Board(A, Color::Red),
                );
            }
        }
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        if limited {
            game.position.players[A.index()].splays[Color::Red as usize] = Splay::Up;
        }
        start(&mut game, source, 0);
        let mut events = Vec::new();
        place(&mut game, &mut events, placed);
        finish(&mut game, &mut events);
        let incoming = outgoing_count
            .min(supply_count)
            .min(if limited { 2 } else { 5 });
        assert_eq!(
            game.position.cards(Zone::Forecast(A)).len(),
            usize::from(incoming)
        );
        assert!(
            outgoing
                .iter()
                .all(|card| game.position.locate(*card) == Some(Zone::AvailableAchievements))
        );
        assert_eq!(
            supply
                .iter()
                .filter(|card| game.position.locate(**card) == Some(Zone::Forecast(A)))
                .count(),
            usize::from(incoming)
        );
        assert!(game.position.cards(Zone::Junk).contains(&junked));
        let exchanges: Vec<_> = events
            .iter()
            .filter_map(|event| {
                if let Event::Exchanged { movements, .. } = event {
                    Some(movements)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(exchanges.len(), usize::from(outgoing_count != 0));
        if let Some(moves) = exchanges.first() {
            assert_eq!(moves.len(), usize::from(outgoing_count + incoming));
        }
    }
}

#[test]
fn telescope_incoming_is_blind_through_atomic_yi_replacement() {
    let mut f = Fixture::default();
    let source = telescope(&mut f);
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let placed = card(&mut f, CardSet::Echoes, 2, Color::Blue, Zone::Forecast(A));
    let outgoing = card(&mut f, CardSet::Echoes, 3, Color::Green, Zone::Forecast(A));
    let choices: Vec<_> = (0..2)
        .map(|_| {
            card(
                &mut f,
                CardSet::Base,
                1,
                Color::Yellow,
                Zone::AvailableAchievements,
            )
        })
        .collect();
    card(
        &mut f,
        CardSet::Base,
        1,
        Color::Purple,
        Zone::Deck(Deck::base(1)),
    );
    let other_set = card(
        &mut f,
        CardSet::Echoes,
        1,
        Color::Yellow,
        Zone::AvailableAchievements,
    );
    let drawn = card(
        &mut f,
        CardSet::Base,
        4,
        Color::Purple,
        Zone::Deck(Deck::base(4)),
    );
    let junked = card(
        &mut f,
        CardSet::Base,
        5,
        Color::Blue,
        Zone::Deck(Deck::base(5)),
    );
    let mut game = Game::seeded(f.position(), BOTH, 71)
        .unwrap()
        .with_tracking()
        .unwrap();
    let before = game.tracker(A).unwrap().belief(Zone::AvailableAchievements);
    let mut expected_rng = game.rng.clone();
    let chosen = choices[(expected_rng.next_u64() % 2) as usize];
    start(&mut game, source, 0);
    let mut events = Vec::new();
    place(&mut game, &mut events, placed);
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options.as_slice(),
        [
            Alternative::BlindSelect {
                source: Zone::AvailableAchievements,
                back: Deck::base(1)
            },
            Alternative::BlindSelect {
                source: Zone::AvailableAchievements,
                back: echo_deck(1)
            },
        ]
    );
    game.choose(0).unwrap();
    // Advancing one work unit at a time observes the suspended exchange
    // before Yi executes. Selection alone cannot alter anyone's beliefs.
    let mut reached = false;
    for _ in 0..30 {
        game.advance(1, &mut events).unwrap();
        if game
            .frames
            .iter()
            .any(|frame| matches!(frame, Frame::Exchange { .. }))
        {
            reached = true;
            assert_eq!(
                game.tracker(A).unwrap().belief(Zone::AvailableAchievements),
                before
            );
            assert!(
                choices
                    .iter()
                    .all(|card| game.position.locate(*card) == Some(Zone::AvailableAchievements))
            );
            let sampled = game.determinize(A, 112).unwrap();
            assert!(sampled.tracker(A).unwrap().check_truth(sampled.position()));
            break;
        }
    }
    assert!(reached);
    finish(&mut game, &mut events);
    assert_eq!(game.rng, expected_rng);
    assert_eq!(game.position.locate(placed), Some(Zone::Deck(echo_deck(2))));
    assert_eq!(
        game.position.locate(outgoing),
        Some(Zone::Board(B, Color::Green))
    );
    assert_eq!(
        game.position.locate(chosen),
        Some(Zone::Board(B, Color::Yellow))
    );
    assert_eq!(
        game.position.locate(drawn),
        Some(Zone::Board(B, Color::Purple))
    );
    assert_eq!(
        game.position.locate(other_set),
        Some(Zone::AvailableAchievements)
    );
    assert!(game.position.cards(Zone::Junk).contains(&junked));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Exchanged { .. }))
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Moved(m) if m.card == drawn && m.verb.is_none()))
            .count(),
        1
    );
}

#[test]
fn telescope_exchange_commit_readmits_by_back_and_leaves_rejected_cards_hidden_at_source() {
    for spare in [0, 1] {
        let mut f = Fixture::default();
        let outgoing = card(&mut f, CardSet::Echoes, 3, Color::Green, Zone::Forecast(A));
        if spare == 0 {
            card(&mut f, CardSet::Echoes, 2, Color::Green, Zone::Forecast(A));
        }
        for _ in 0..2 {
            card(
                &mut f,
                CardSet::Base,
                4,
                Color::Red,
                Zone::Board(A, Color::Red),
            );
        }
        let incoming: Vec<_> = [1, 1, 2]
            .into_iter()
            .map(|age| {
                card(
                    &mut f,
                    CardSet::Base,
                    age,
                    Color::Yellow,
                    Zone::AvailableAchievements,
                )
            })
            .collect();
        card(
            &mut f,
            CardSet::Base,
            1,
            Color::Blue,
            Zone::Deck(Deck::base(1)),
        );
        let mut game = Game::seeded(f.position(), Expansions::ECHOES, 71)
            .unwrap()
            .with_tracking()
            .unwrap();
        // Exercise the commit continuation after a hypothetical First has
        // changed capacity. No current Figure has First on Exchange.
        game.position.players[A.index()].splays[Color::Red as usize] = Splay::Aslant;
        let before = game.tracker(A).unwrap().belief(Zone::AvailableAchievements);
        game.frames.push(Frame::Exchange {
            plan: Box::new(ExchangePlan {
                left: vec![target(outgoing, Zone::Forecast(A))],
                right: incoming
                    .iter()
                    .map(|card| target(*card, Zone::AvailableAchievements))
                    .collect(),
                left_destination: Zone::AvailableAchievements,
                right_destination: Zone::Forecast(A),
                context: Context::new(A),
            }),
            phase: Phase::Commit,
        });
        let mut events = Vec::new();
        if spare != 0 {
            let menu = decision(&mut game, &mut events);
            assert_eq!(
                menu.options.as_slice(),
                [
                    Alternative::BlindSelect {
                        source: Zone::AvailableAchievements,
                        back: Deck::base(1)
                    },
                    Alternative::BlindSelect {
                        source: Zone::AvailableAchievements,
                        back: Deck::base(2)
                    },
                ]
            );
            assert!(events.is_empty());
            assert_eq!(
                game.tracker(A).unwrap().belief(Zone::AvailableAchievements),
                before
            );
            assert_eq!(game.position.locate(outgoing), Some(Zone::Forecast(A)));
            game.choose(0).unwrap();
        }
        finish(&mut game, &mut events);
        assert_eq!(game.position.cards(Zone::Forecast(A)).len(), 1);
        assert_eq!(
            game.position.locate(outgoing),
            Some(Zone::AvailableAchievements)
        );
        assert_eq!(
            incoming
                .iter()
                .filter(|card| game.position.locate(**card) == Some(Zone::Forecast(A)))
                .count(),
            spare
        );
        assert_eq!(
            incoming
                .iter()
                .filter(|card| game.position.locate(**card) == Some(Zone::AvailableAchievements))
                .count(),
            3 - spare
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Exchanged { .. }))
                .count(),
            1
        );
        // Unchosen age-1 fronts remain ambiguous, including to the chooser.
        assert!(
            game.tracker(A)
                .unwrap()
                .belief(Zone::AvailableAchievements)
                .iter()
                .filter(|slot| slot.back == Deck::base(1))
                .all(|slot| slot.known().is_none())
        );
    }
}

#[test]
fn telescope_forecast_overlap_has_no_physical_movers_or_yi_reaction() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let held = card(&mut f, CardSet::Echoes, 1, Color::Green, Zone::Forecast(A));
    let draw = card(
        &mut f,
        CardSet::Base,
        4,
        Color::Yellow,
        Zone::Deck(Deck::base(4)),
    );
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Hand(A),
        vec![Instruction::Exchange(ExchangeStep {
            left: Selector::Forecast(Who::Executor),
            right: Selector::Forecast(Who::Executor),
            counts: &[None, None],
            optional: false,
        })],
    );
    let mut game = Game::new(f.position(), BOTH).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(game.position.locate(held), Some(Zone::Forecast(A)));
    assert_eq!(game.position.locate(draw), Some(Zone::Deck(Deck::base(4))));
    assert!(events.is_empty());
}

#[derive(Debug, PartialEq, Eq)]
struct End {
    position: Position,
    rng: Rng,
    status: Status,
    events: Vec<Event>,
}

fn paths(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<End> {
    assert!(depth < 20);
    match game.advance(1000, &mut events).unwrap() {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut next = game.clone();
                next.choose(choice).unwrap();
                paths(next, events.clone(), depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            game.position.sort_unordered_piles();
            // Forecasts and available achievements are unordered as well.
            for player in Player::ALL {
                game.position
                    .zone_mut(Zone::Forecast(player))
                    .sort_unstable();
            }
            game.position
                .zone_mut(Zone::AvailableAchievements)
                .sort_unstable();
            vec![End {
                position: game.position,
                rng: game.rng,
                status,
                events,
            }]
        }
    }
}

#[test]
fn telescope_supply_shortcuts_match_every_unrestricted_path_with_reacting_yi() {
    assert!(std::mem::size_of::<Instruction>() <= 96);
    for yi in [false, true] {
        for whole in [false, true] {
            let mut f = Fixture::default();
            let source = telescope(&mut f);
            if yi {
                f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
            }
            let placed = card(&mut f, CardSet::Echoes, 2, Color::Blue, Zone::Forecast(A));
            for age in 6..if whole { 9 } else { 8 } {
                card(
                    &mut f,
                    CardSet::Echoes,
                    age,
                    Color::Green,
                    Zone::Forecast(A),
                );
            }
            for age in if whole { [1, 1, 2] } else { [1, 2, 3] } {
                card(
                    &mut f,
                    CardSet::Base,
                    age,
                    Color::Yellow,
                    Zone::AvailableAchievements,
                );
            }
            card(
                &mut f,
                CardSet::Base,
                4,
                Color::Purple,
                Zone::Deck(Deck::base(4)),
            );
            let mut reduced = Game::seeded(f.position(), BOTH, 71).unwrap();
            let mut manual = reduced.clone();
            manual.shortcuts_disabled = true;
            for game in [&mut reduced, &mut manual] {
                start(game, source, 0);
                place(game, &mut Vec::new(), placed);
            }
            let reduced = paths(reduced, Vec::new(), 0);
            let manual = paths(manual, Vec::new(), 0);
            assert!(reduced.len() < manual.len(), "Yi {yi}; whole {whole}");
            // Events list moves in the order chosen, which is what Yi's
            // reduced tucks remove: compare the ends themselves.
            let same = |a: &End, b: &End| {
                a.position == b.position && a.rng == b.rng && a.status == b.status
            };
            assert!(
                reduced
                    .iter()
                    .all(|end| manual.iter().any(|other| same(end, other))),
                "reduced outcome missing: Yi {yi}; whole {whole}"
            );
            assert!(
                manual
                    .iter()
                    .all(|end| reduced.iter().any(|other| same(end, other))),
                "manual outcome missing: Yi {yi}; whole {whole}"
            );
            if yi {
                assert!(reduced.iter().all(|end| end.events.iter().any(|event| matches!(event, Event::Moved(m) if m.inside_karma && m.verb == Some(Verb::Tuck)))));
            }
        }
    }
}

#[test]
fn telescope_available_board_alias_moves_atomically_and_clears_its_lost_splay() {
    let mut f = Fixture::default();
    let source = alias_telescope(&mut f);
    f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    let placed = card(&mut f, CardSet::Echoes, 2, Color::Blue, Zone::Forecast(A));
    let outgoing = card(&mut f, CardSet::Echoes, 3, Color::Green, Zone::Forecast(A));
    let bottom = card(
        &mut f,
        CardSet::Base,
        1,
        Color::Red,
        Zone::Board(A, Color::Red),
    );
    let incoming = card(
        &mut f,
        CardSet::Base,
        2,
        Color::Red,
        Zone::Board(A, Color::Red),
    );
    f.cards[usize::from(incoming.0)].symbols[0] = Symbol::Icon(Icon::Industry);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.position.players[A.index()].splays[Color::Red as usize] = Splay::Left;
    game.position.refresh_board(A);
    start(&mut game, source, 0);
    let mut events = Vec::new();
    place(&mut game, &mut events, placed);
    let menu = decision(&mut game, &mut events);
    game.choose(
        menu.options
            .iter()
            .position(|option| {
                matches!(option,
        Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == incoming)
            })
            .unwrap(),
    )
    .unwrap();
    finish(&mut game, &mut events);
    assert_eq!(game.position.cards(Zone::Board(A, Color::Red)), [bottom]);
    assert_eq!(game.position.splay(A, Color::Red), Splay::None);
    assert_eq!(game.position.cards(Zone::Forecast(A)), [incoming]);
    assert_eq!(
        game.position.locate(outgoing),
        Some(Zone::AvailableAchievements)
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Exchanged { .. }))
            .count(),
        1
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Splayed { .. }))
    );
}

#[test]
fn telescope_selecting_a_board_alias_never_becomes_a_supply_ordering_floor() {
    let mut f = Fixture::default();
    let source = alias_telescope(&mut f);
    let mcadam = f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    let placed = card(&mut f, CardSet::Echoes, 2, Color::Blue, Zone::Forecast(A));
    for age in [6, 7] {
        card(
            &mut f,
            CardSet::Echoes,
            age,
            Color::Green,
            Zone::Forecast(A),
        );
    }
    let supply: Vec<_> = [1, 2, 3]
        .into_iter()
        .map(|age| {
            card(
                &mut f,
                CardSet::Base,
                age,
                Color::Purple,
                Zone::AvailableAchievements,
            )
        })
        .collect();
    let mut reduced = Game::seeded(f.position(), BOTH, 71).unwrap();
    start(&mut reduced, source, 0);
    place(&mut reduced, &mut Vec::new(), placed);
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let reduced_paths = paths(reduced.clone(), Vec::new(), 0);
    let manual_paths = paths(manual, Vec::new(), 0);
    assert!(reduced_paths.iter().all(|end| manual_paths.contains(end)));
    assert!(manual_paths.iter().all(|end| reduced_paths.contains(end)));
    let mut events = Vec::new();
    let menu = decision(&mut reduced, &mut events);
    reduced
        .choose(
            menu.options
                .iter()
                .position(|option| {
                    matches!(option,
        Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == mcadam)
                })
                .unwrap(),
        )
        .unwrap();
    let menu = decision(&mut reduced, &mut events);
    assert_eq!(
        menu.options.as_slice(),
        [1, 2, 3].map(|age| Alternative::BlindSelect {
            source: Zone::AvailableAchievements,
            back: Deck::base(age)
        })
    );
    assert!(reduced.position.cards(Zone::Forecast(A)).len() == 2);
    reduced.choose(0).unwrap();
    finish(&mut reduced, &mut events);
    assert_eq!(reduced.position.cards(Zone::Forecast(A)).len(), 2);
    assert!(reduced.position.cards(Zone::Forecast(A)).contains(&mcadam));
    assert!(
        reduced
            .position
            .cards(Zone::Forecast(A))
            .contains(&supply[0])
    );
}

#[test]
fn telescope_yi_tucks_every_frozen_alias_after_its_granting_figure_moves() {
    let mut f = Fixture::default();
    let source = alias_telescope(&mut f);
    let mcadam = f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    let alias = card(
        &mut f,
        CardSet::Base,
        3,
        Color::Purple,
        Zone::Board(A, Color::Purple),
    );
    f.cards[usize::from(alias.0)].symbols[0] = Symbol::Icon(Icon::Industry);
    let placed = card(&mut f, CardSet::Echoes, 2, Color::Blue, Zone::Forecast(A));
    for age in [6, 7] {
        card(
            &mut f,
            CardSet::Echoes,
            age,
            Color::Green,
            Zone::Forecast(A),
        );
    }
    card(
        &mut f,
        CardSet::Base,
        4,
        Color::Blue,
        Zone::Deck(Deck::base(4)),
    );
    let mut game = Game::seeded(f.position(), BOTH, 71)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    place(&mut game, &mut events, placed);
    // Both board aliases must be selected before Yi's one replacement.
    let menu = decision(&mut game, &mut events);
    game.choose(
        menu.options
            .iter()
            .position(|option| {
                matches!(option,
        Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == mcadam)
            })
            .unwrap(),
    )
    .unwrap();
    // The remaining incoming selection and the sole hook resolve
    // automatically. Yi's tucks run one colour at a time, lowest first, so
    // McAdam's yellow leaves before the purple alias he granted, which is
    // still tucked after its granting Figure moved.
    finish(&mut game, &mut events);
    assert!(!game.position.has_figure(A, Figure::JohnLoudonMcAdam));
    let tucks: Vec<CardId> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Tuck) => Some(m.card),
            _ => None,
        })
        .collect();
    let tucked = |card| tucks.iter().position(|&c| c == card).unwrap();
    assert!(tucked(mcadam) < tucked(alias));
    assert_eq!(
        game.position.locate(alias),
        Some(Zone::Board(B, Color::Purple))
    );
    assert_eq!(
        game.position.locate(mcadam),
        Some(Zone::Board(B, Color::Yellow))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Exchanged { .. }))
    );
    assert_eq!(events.iter().filter(|event| matches!(event, Event::Moved(m) if m.card == alias && m.verb == Some(Verb::Tuck))).count(), 1);
}

#[test]
fn telescope_exchanges_only_standard_achievements_never_figure_aliases() {
    let mut f = Fixture::default();
    let source = telescope(&mut f);
    let mcadam = f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    let placed = card(&mut f, CardSet::Echoes, 2, Color::Blue, Zone::Forecast(A));
    card(&mut f, CardSet::Echoes, 6, Color::Green, Zone::Forecast(A));
    for age in [1, 2] {
        card(
            &mut f,
            CardSet::Base,
            age,
            Color::Purple,
            Zone::AvailableAchievements,
        );
    }
    let mut game = Game::seeded(f.position(), BOTH, 71).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    place(&mut game, &mut events, placed);
    finish(&mut game, &mut events);
    assert_eq!(
        game.position.locate(mcadam),
        Some(Zone::Board(A, Color::Yellow))
    );
    assert_eq!(game.position.cards(Zone::Forecast(A)).len(), 1);
}
