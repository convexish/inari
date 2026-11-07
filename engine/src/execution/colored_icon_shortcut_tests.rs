use super::common::*;
use super::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn start(g: &mut Game, source: CardId, effect: u16) {
    g.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}
// Hands, score piles and junk are unordered: only their contents matter.
fn normalized(mut p: Position) -> Position {
    p.sort_unordered_piles();
    p
}
fn leaves(mut g: Game, depth: usize) -> Vec<(Position, crate::rng::Rng)> {
    assert!(depth < 12);
    match settle(&mut g, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut next = g.clone();
                next.choose(choice).unwrap();
                leaves(next, depth + 1)
            })
            .collect(),
        status => {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert!(g.position.check_conservation());
            vec![(normalized(g.position), g.rng)]
        }
    }
}
fn fixture(code: &str, mixed: bool) -> (Position, CardId, u16) {
    let mut f = Fixture::default();
    match code {
        "B019" => {
            let source = base(&mut f, code, Zone::Board(A, Color::Yellow));
            for code in ["B016", "B018", "B021"] {
                base(&mut f, code, Zone::Deck(Deck::base(2)));
            }
            (f.position(), source, 1)
        }
        "B039" => {
            let source = base(&mut f, code, Zone::Board(A, Color::Yellow));
            base(&mut f, "B012", Zone::Board(A, Color::Blue));
            for code in if mixed {
                ["B001", "B002", "B021", "B026"]
            } else {
                ["B001", "B002", "B004", "B008"]
            } {
                base(&mut f, code, Zone::Hand(A));
            }
            (f.position(), source, 0)
        }
        "B083" => {
            let source = base(&mut f, code, Zone::Board(A, Color::Blue));
            base(&mut f, "B075", Zone::Board(A, Color::Purple));
            for code in if mixed {
                ["B001", "B021", "B026"]
            } else {
                ["B001", "B002", "B021"]
            } {
                base(&mut f, code, Zone::Score(B));
            }
            (f.position(), source, 0)
        }
        _ => unreachable!(),
    }
}
fn all_paths<const F: bool>() {
    for code in ["B019", "B039", "B083"] {
        for mixed in [false, true] {
            let (p, source, effect) = fixture(code, mixed);
            let mut reduced = Game::seeded(p, Expansions::with_figures(F), 37).unwrap();
            let mut manual = reduced.clone();
            manual.shortcuts_disabled = true;
            start(&mut reduced, source, effect);
            start(&mut manual, source, effect);
            let reduced = leaves(reduced, 0);
            let manual = leaves(manual, 0);
            assert!(
                manual.iter().all(|leaf| reduced.contains(leaf)),
                "missing manual outcome {code}"
            );
            assert!(
                reduced.iter().all(|leaf| manual.contains(leaf)),
                "invented outcome {code}"
            );
            if mixed || code == "B019" {
                assert!(reduced.len() < manual.len(), "missing reduction {code}");
            }
        }
    }
}
#[test]
fn all_quota_subsets_origin_orders_and_native_rng_outcomes_survive() {
    all_paths::<false>();
    all_paths::<true>();
}

fn canonical(g: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            let key = match option {
                Alternative::Move {
                    target: TargetDescriptor::Known { card, .. },
                    ..
                } => {
                    let deck = g.position.card(*card).origin_deck();
                    (deck.age, deck.set as u8, usize::from(card.0))
                }
                Alternative::Move {
                    target:
                        TargetDescriptor::Hidden {
                            back: Deck { age: value, set },
                            slot,
                            ..
                        },
                    ..
                } => (*value, *set as u8, usize::MAX - slot),
                Alternative::BlindMove {
                    back: Deck { age: value, set },
                    ..
                } => (*value, *set as u8, 0),
                _ => return None,
            };
            Some((key, index))
        })
        .min_by_key(|&(key, _)| key)
        .unwrap()
        .1
}
fn finish(g: &mut Game, events: &mut Vec<Event>) {
    loop {
        match settle(g, events) {
            Status::Decision(menu) => {
                let choice = canonical(g, &menu);
                g.choose(choice).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                return;
            }
        }
    }
}
fn canonical_paths<const F: bool>() {
    for code in ["B019", "B039", "B083"] {
        let (p, source, effect) = fixture(code, true);
        let mut reduced = Game::seeded(p, Expansions::with_figures(F), 37).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, effect);
        start(&mut manual, source, effect);
        let mut events = Vec::new();
        loop {
            let mut clone = reduced.clone();
            let mut a = Vec::new();
            let mut b = Vec::new();
            let status = reduced.advance(1, &mut a).unwrap();
            assert_eq!(clone.advance(1, &mut b).unwrap(), status);
            assert_eq!(a, b);
            assert_eq!(clone, reduced);
            events.extend(a);
            match status {
                Status::WorkLimit => {}
                Status::Decision(menu) => {
                    let choice = canonical(&reduced, &menu);
                    reduced.choose(choice).unwrap();
                }
                status => {
                    assert_eq!(status, Status::Complete(ReturnValue::Unit));
                    break;
                }
            }
        }
        let mut expected = Vec::new();
        finish(&mut manual, &mut expected);
        assert_eq!(events, expected);
        manual.shortcuts_disabled = false;
        assert_eq!(reduced, manual);
    }
}
#[test]
fn canonical_events_rng_and_every_work_boundary_clones_match() {
    canonical_paths::<false>();
    canonical_paths::<true>();
}

#[test]
fn private_score_menus_do_not_expose_identity_or_slot_and_non_singleton_groups_stay_unreduced() {
    let (p, source, effect) = fixture("B083", false);
    let mut reduced = Game::seeded(p.clone(), Expansions::BASE, 91).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, effect);
    start(&mut manual, source, effect);
    let menu = decision(&mut reduced, &mut Vec::new());
    assert_eq!(menu, decision(&mut manual, &mut Vec::new()));
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::BlindMove {
            source: Zone::Score(B),
            ..
        }
    )));
    let mut permuted = p;
    permuted.players[B.index()].score.reverse();
    let mut other = Game::seeded(permuted, Expansions::BASE, 91).unwrap();
    start(&mut other, source, effect);
    assert_eq!(menu, decision(&mut other, &mut Vec::new()));
    let before = reduced.rng.clone();
    let age_one = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::BlindMove {
                    back: Deck { age: 1, .. },
                    ..
                }
            )
        })
        .unwrap();
    reduced.choose(age_one).unwrap();
    let mut expected = before;
    expected.next_u64();
    assert_eq!(reduced.rng, expected);
}

#[test]
/// Each additions are not on the board, so they give no color an icon: Gilgamesh adds
/// Industry to totals only.
fn colors_count_board_symbols_only_and_either_sums_boards() {
    let mut f = Fixture::default();
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    base(&mut f, "B011", Zone::Board(A, Color::Blue));
    base(&mut f, "B002", Zone::Board(B, Color::Red));
    let g = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    assert_eq!(
        g.query_count(
            Expr::Count(&Selector::Filtered(
                &Selector::Tops(Who::Executor),
                &[Filter::Has(Test::StackShows(Icon::Industry))]
            )),
            context
        ),
        0
    );
    assert_eq!(
        g.query_count(
            Expr::Count(&Selector::Filtered(
                &Selector::Tops(Who::Demander),
                &[Filter::Has(Test::StackShows(Icon::Authority))]
            )),
            context
        ),
        1
    );
    assert_eq!(
        g.query_count(
            Expr::Count(&Selector::Filtered(
                &Selector::Tops(Who::Either),
                &[Filter::Has(Test::StackShows(Icon::Authority))]
            )),
            context
        ),
        3
    );
    assert_eq!(
        g.query_count(
            Expr::Count(&Selector::Filtered(
                &Selector::Tops(Who::Opponent),
                &[Filter::Has(Test::StackShows(Icon::Industry))]
            )),
            context
        ),
        0
    );
}

#[test]
fn live_return_hooks_restore_all_order_choices_but_unrelated_each_does_not() {
    for hazard in [false, true] {
        let mut f = Fixture::default();
        let source = base(
            &mut f,
            "B083",
            if hazard {
                Zone::Junk
            } else {
                Zone::Board(A, Color::Blue)
            },
        );
        base(&mut f, "B075", Zone::Board(A, Color::Purple));
        if hazard {
            f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
            base(&mut f, "B080", Zone::Deck(Deck::base(8)));
        } else {
            f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
        }
        for code in ["B001", "B021", "B026"] {
            base(&mut f, code, Zone::Score(B));
        }
        let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut g, source, 0);
        let menu = decision(&mut g, &mut Vec::new());
        let can_start_high = menu.options.iter().any(|o| {
            matches!(
                o,
                Alternative::BlindMove {
                    back: Deck { age: 3, .. },
                    ..
                }
            )
        });
        assert_eq!(can_start_high, hazard);
        if hazard {
            let high = menu
                .options
                .iter()
                .position(|o| {
                    matches!(
                        o,
                        Alternative::BlindMove {
                            back: Deck { age: 3, .. },
                            ..
                        }
                    )
                })
                .unwrap();
            g.choose(high).unwrap();
            let next = decision(&mut g, &mut Vec::new());
            assert_eq!(next.options.len(), 2);
            assert_eq!(g.position.cards(Zone::Hand(A)).len(), 1);
        }
    }
}

#[test]
fn exact_base_deck_junk_ignores_homers_irrelevant_figure_hand_hook() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B019", Zone::Board(B, Color::Yellow));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    for code in ["B016", "B018", "B021"] {
        base(&mut f, code, Zone::Deck(Deck::base(2)));
    }
    let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, 1);
    start(&mut manual, source, 1);
    let reduced = leaves(reduced, 0);
    let manual = leaves(manual, 0);
    assert_eq!(reduced.len(), 1);
    assert_eq!(manual.len(), 6);
    assert!(manual.iter().all(|outcome| reduced.contains(outcome)));
}
