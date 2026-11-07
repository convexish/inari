//! Every-path age-seven order proofs retain RNG, information, and live reactions.
use super::*;
use age03_mechanics_tests::oracle;
use shortcuts::common::*;
const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);
fn card(f: &mut Fixture, code: &str) -> CardId {
    let c = echoes_card(code).unwrap();
    let color = c.color;
    f.add(c, Zone::Board(A, color))
}
fn tech(f: &mut Fixture, age: u8, color: Color, zone: Zone) -> CardId {
    f.tech(color, age, zone, vec![])
}
fn figure(f: &mut Fixture, which: Figure, owner: Player) {
    let c = figure_card(which.code()).unwrap();
    let color = c.color;
    f.add(c, Zone::Board(owner, color));
}
fn run(f: Fixture, source: CardId, index: u16) -> Game {
    let mut g = Game::seeded(f.position(), BOTH, 19)
        .unwrap()
        .with_tracking()
        .unwrap();
    g.start(
        Operation::ExecuteEffect {
            card: source,
            effect: index,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    g
}

#[test]
fn elevator_back_batches_keep_rng_and_cai_lun_first_reactions_over_every_path() {
    for hands in [false, true] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E068");
            tech(&mut f, 3, Color::Red, Zone::Score(A));
            let origin = if hands { Zone::Hand(B) } else { Zone::Score(B) };
            let pair = [
                tech(&mut f, 3, Color::Red, origin),
                tech(&mut f, 3, Color::Blue, origin),
            ];
            if reacting {
                for age in [1, 2] {
                    tech(&mut f, age, Color::Red, Zone::Board(B, Color::Red));
                }
                figure(&mut f, Figure::CaiLun, B);
            }
            tech(&mut f, 3, Color::Yellow, Zone::Deck(Deck::base(3)));
            let (reduced, manual) = oracle(run(f, source, 0));
            assert_eq!(reduced.len(), 2);
            assert_eq!(manual.len(), 2);
            for end in &reduced {
                let scored = pair
                    .iter()
                    .filter(|id| end.game.position.locate(**id) == Some(Zone::Score(A)))
                    .count();
                assert!([0, 2].contains(&scored));
                if reacting && scored == 2 {
                    assert!(end.events.iter().any(|e| matches!(
                        e,
                        Event::Splayed {
                            inside_karma: true,
                            ..
                        }
                    )));
                }
            }
        }
    }
}

#[test]
fn fertilizer_transfer_cohorts_commute_unless_nobel_observes_them() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E069");
        tech(&mut f, 2, Color::Red, Zone::Hand(A));
        let own = tech(&mut f, 2, Color::Yellow, Zone::Score(A));
        let pair = [
            tech(&mut f, 2, Color::Red, Zone::Score(B)),
            tech(&mut f, 2, Color::Blue, Zone::Score(B)),
        ];
        let seven = tech(&mut f, 7, Color::Green, Zone::Deck(Deck::base(7)));
        if reacting {
            figure(&mut f, Figure::AlfredNobel, B);
        }
        let (reduced, manual) = oracle(run(f, source, 0));
        if reacting {
            assert_eq!(reduced.len(), manual.len());
        } else {
            assert!(reduced.len() < manual.len());
        }
        assert!(
            reduced
                .iter()
                .any(|end| end.game.position.locate(own) == Some(Zone::Hand(A)))
        );
        for end in reduced {
            let moved = end.game.position.locate(own) == Some(Zone::Hand(A));
            for id in pair {
                assert_eq!(
                    end.game.position.locate(id),
                    Some(if moved { Zone::Hand(A) } else { Zone::Score(B) })
                );
            }
            if reacting && moved {
                assert_eq!(end.game.position.locate(seven), Some(Zone::Junk));
                assert!(
                    end.events
                        .iter()
                        .any(|e| matches!(e,Event::Moved(m) if m.card==seven&&m.inside_karma))
                );
            }
        }
    }
}

#[test]
fn typewriter_return_order_keeps_same_deck_orders_and_maxwell_arrivals_live() {
    for same in [false, true] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E073");
            tech(&mut f, 1, Color::Red, Zone::Hand(A));
            tech(&mut f, if same { 1 } else { 2 }, Color::Blue, Zone::Hand(A));
            if reacting {
                figure(&mut f, Figure::JamesClerkMaxwell, B);
            }
            for age in [6, 7, 8, 8, 9] {
                tech(&mut f, age, Color::Green, Zone::Deck(Deck::base(age)));
            }
            let (reduced, manual) = oracle(run(f, source, 0));
            assert_eq!(manual.len(), 2);
            assert_eq!(reduced.len(), if same || reacting { 2 } else { 1 });
            for end in reduced {
                assert_eq!(end.game.position.cards(Zone::Hand(A)).len(), 3);
                if reacting {
                    assert_eq!(end.game.position.cards(Zone::Hand(B)).len(), 2);
                    assert!(
                        end.events
                            .iter()
                            .any(|e| matches!(e,Event::Moved(m) if m.inside_karma))
                    );
                }
            }
        }
    }
}

#[test]
fn telegraph_foreseen_splay_order_matches_all_paths_with_machiavelli() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let mut source = echoes_card("E071").unwrap();
        source.effects = vec![source.effects[2].clone()];
        let source = f.add(source, Zone::Forecast(A));
        let driver = tech(&mut f, 7, Color::Yellow, Zone::Hand(A));
        for owner in Player::ALL {
            for color in [Color::Red, Color::Blue] {
                for age in [1, 2] {
                    tech(&mut f, age, color, Zone::Board(owner, color));
                }
            }
        }
        if reacting {
            figure(&mut f, Figure::NiccoloMachiavelli, A);
        }
        let mut game = Game::seeded(f.position(), BOTH, 19)
            .unwrap()
            .with_tracking()
            .unwrap();
        for owner in Player::ALL {
            for color in [Color::Red, Color::Blue] {
                game.position.players[owner.index()].splays[color as usize] = Splay::Left;
            }
        }
        game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
            .unwrap();
        let (reduced, manual) = oracle(game);
        assert_eq!(manual.len(), 2);
        assert_eq!(reduced.len(), if reacting { 2 } else { 1 });
        for end in reduced {
            assert_eq!(
                end.game.position.locate(source),
                Some(Zone::Board(A, Color::Green))
            );
            for color in [Color::Red, Color::Blue] {
                assert_eq!(end.game.position.splay(A, color), Splay::Up);
                assert_eq!(
                    end.game.position.splay(B, color),
                    if reacting { Splay::None } else { Splay::Left }
                );
            }
            if reacting {
                assert!(end.events.iter().any(|e| matches!(
                    e,
                    Event::Splayed {
                        inside_karma: true,
                        ..
                    }
                )));
            }
        }
    }
}

#[test]
fn jeans_deck_sweep_oracle_uses_archimedes_shifted_age_and_preserves_other_sets() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E070");
        if reacting {
            figure(&mut f, Figure::Archimedes, A);
        }
        let mut cards = Vec::new();
        for age in [7, 8, 9] {
            for color in [Color::Red, Color::Blue] {
                cards.push(tech(&mut f, age, color, Zone::Deck(Deck::base(age))));
            }
        }
        let echo = tech(
            &mut f,
            8,
            Color::Green,
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 8,
            }),
        );
        f.cards[usize::from(echo.0)].set = CardSet::Echoes;
        let (reduced, manual) = oracle(run(f, source, 1));
        assert_eq!(reduced.len(), 2);
        assert_eq!(manual.len(), 4);
        for end in reduced {
            assert_eq!(
                end.game.position.locate(echo),
                Some(Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 8
                }))
            );
            let junk: Vec<_> = cards
                .iter()
                .filter(|id| end.game.position.locate(**id) == Some(Zone::Junk))
                .collect();
            assert_eq!(junk.len(), 2);
            let value = end.game.position.card(*junk[0]).value;
            assert!((if reacting { [8, 9] } else { [7, 8] }).contains(&value));
        }
    }
}

fn paths(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<age03_mechanics_tests::End> {
    assert!(depth < 12);
    let status = game.advance(10000, &mut events).unwrap();
    if let Status::Decision(menu) = status {
        (0..menu.options.len())
            .flat_map(|index| {
                let mut fork = game.clone();
                fork.choose(index).unwrap();
                paths(fork, events.clone(), depth + 1)
            })
            .collect()
    } else {
        assert!(game.position.check_conservation());
        vec![age03_mechanics_tests::End {
            game,
            status,
            events,
            decisions: depth,
        }]
    }
}
fn law_key(
    end: &age03_mechanics_tests::End,
) -> (Position, Status, [Vec<Vec<crate::tracking::BeliefSlot>>; 3]) {
    let mut position = end.game.position.clone();
    position.sort_unordered_piles();
    let beliefs = std::array::from_fn(|observer| {
        crate::tracking::zones()
            .into_iter()
            .map(|zone| end.game.knowledge.as_ref().unwrap().trackers[observer].belief(zone))
            .collect()
    });
    (position, end.status.clone(), beliefs)
}
#[test]
fn ice_cream_sweep_takes_the_deck_top_for_any_cohort_and_reacting_boxed_values() {
    for n in [2, 3] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E074");
            if reacting {
                figure(&mut f, Figure::Archimedes, A);
            }
            tech(&mut f, 10, Color::Red, Zone::Board(A, Color::Red));
            for _ in 0..4 {
                tech(&mut f, 10, Color::Blue, Zone::Score(A));
            }
            let age = if reacting { 7 } else { 6 };
            for color in [Color::Red, Color::Blue, Color::Green].into_iter().take(n) {
                tech(&mut f, age, color, Zone::Deck(Deck::base(age)));
            }
            let mut game = run(f, source, 1);
            let menu = decision(&mut game, &mut vec![]);
            assert_eq!(menu.options[0], Alternative::Value(age));
            game.choose(0).unwrap();
            if n == 3 {
                // A three-card cohort is not a power of two, so the junk order
                // decides which hidden identity carries the native remainder
                // weight; the sweep takes the deck top anyway (shortcut policy).
                let mut manual = game.clone();
                manual.shortcuts_disabled = true;
                let reduced = paths(game, vec![], 0);
                let manual = paths(manual, vec![], 0);
                assert_eq!(reduced.len(), 2);
                assert_eq!(manual.len(), 7);
                assert!(reduced.iter().all(|end| manual.iter().any(|other| {
                    other.game.position == end.game.position && other.game.rng == end.game.rng
                })));
                assert!(
                    reduced
                        .iter()
                        .any(|end| end.game.position.cards(Zone::Achievements(A)).len() == 1)
                );
                continue;
            }
            // Every modulo-2 class has exactly half of the native 64-bit mass.
            // Compare each deliberate junk order separately, never average player
            // choices. Exact RNG advancement remains an independent assertion.
            let seeds: [u64; 2] = std::array::from_fn(|class| {
                (0..100)
                    .find(|&seed| Rng::new(seed).next_u64() % 2 == class as u64)
                    .unwrap()
            });
            let mut reduced = Vec::new();
            let mut manual = Vec::new();
            for seed in seeds {
                for disabled in [false, true] {
                    let mut root = game.clone();
                    root.shortcuts_disabled = disabled;
                    root.rng = Rng::new(seed);
                    let leaves = paths(root, vec![], 0);
                    assert_eq!(leaves.len(), if disabled { 3 } else { 2 });
                    for leaf in &leaves {
                        let mut expected = Rng::new(seed);
                        if !leaf.game.position.cards(Zone::Achievements(A)).is_empty() {
                            expected.next_u64();
                        }
                        assert_eq!(leaf.game.rng, expected);
                    }
                    if disabled {
                        manual.extend(leaves);
                    } else {
                        reduced.extend(leaves);
                    }
                }
            }
            let order = |end: &age03_mechanics_tests::End| {
                end.events
                    .iter()
                    .filter_map(|e| {
                        if let Event::Moved(m) = e {
                            (m.verb == Some(Verb::Junk)).then_some(m.card)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            };
            for accepted in [false, true] {
                let reduced: Vec<_> = reduced
                    .iter()
                    .filter(|end| {
                        end.game.position.cards(Zone::Achievements(A)).is_empty() != accepted
                    })
                    .collect();
                let full: Vec<_> = manual
                    .iter()
                    .filter(|end| {
                        end.game.position.cards(Zone::Achievements(A)).is_empty() != accepted
                    })
                    .collect();
                let mut orders = Vec::new();
                for end in &full {
                    if !orders.contains(&order(end)) {
                        orders.push(order(end));
                    }
                }
                for sequence in orders {
                    let same: Vec<_> = full.iter().filter(|end| order(end) == sequence).collect();
                    for end in &same {
                        let key = law_key(end);
                        assert_eq!(
                            same.iter().filter(|other| law_key(other) == key).count()
                                * reduced.len(),
                            reduced.iter().filter(|other| law_key(other) == key).count()
                                * same.len()
                        );
                    }
                }
                for leaf in &reduced {
                    assert!(
                        full.iter()
                            .any(|end| law_key(end) == law_key(leaf) && end.events == leaf.events)
                    );
                }
            }
        }
    }
}
