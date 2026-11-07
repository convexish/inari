use super::*;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn start(game: &mut Game, source: CardId, effect: u16, boxed: BoxedValues, suppressed: bool) {
    let mut context = Context::new(A);
    context.demander = B;
    context.set_suppress_karma(suppressed);
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed,
        },
        context,
    )
    .unwrap();
}
fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        let status = settle(game, events);
        if let Status::Decision(_) = status {
            game.choose(0).unwrap();
        } else {
            assert!(game.position.check_conservation());
            return status;
        }
    }
    panic!("exchange did not finish");
}
fn branch(game: &mut Game, events: &mut Vec<Event>, option: Alternative) {
    let menu = decision(game, events);
    game.choose(menu.options.iter().position(|o| *o == option).unwrap())
        .unwrap();
}
fn grouped(events: &[Event]) -> Vec<&[ExchangeMovement]> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Exchanged { movements, .. } => Some(movements.as_slice()),
            _ => None,
        })
        .collect()
}
fn ordinary_cases<const F: bool>() {
    for code in ["B028", "B029", "B069"] {
        for (left_count, right_count) in [(0, 0), (0, 1), (1, 0), (1, 1), (3, 2)] {
            let mut f = Fixture::default();
            let source = base(&mut f, code, Zone::Junk);
            let zones = if code == "B029" {
                [Zone::Score(A), Zone::Score(B)]
            } else {
                [Zone::Hand(A), Zone::Hand(B)]
            };
            for code in ["B032", "B033", "B034"].into_iter().take(left_count) {
                base(&mut f, code, zones[0]);
            }
            for code in ["B001", "B004"].into_iter().take(right_count) {
                base(&mut f, code, zones[1]);
            }
            let mut game = Game::seeded(f.position(), Expansions::with_figures(F), 37).unwrap();
            start(&mut game, source, 0, BoxedValues::default(), false);
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            let left_moved = if code == "B028" {
                left_count
            } else {
                left_count.min(if code == "B069" { 2 } else { 1 })
            };
            let right_moved = if code == "B028" {
                right_count
            } else {
                right_count.min(1)
            };
            assert_eq!(
                game.position.cards(zones[0]).len(),
                left_count - left_moved + right_moved
            );
            assert_eq!(
                game.position.cards(zones[1]).len(),
                right_count - right_moved + left_moved
            );
            assert_eq!(
                grouped(&events).len(),
                usize::from(left_count + right_count != 0)
            );
            assert!(!events.iter().any(|e| matches!(e, Event::Moved(_))));
        }
    }
}
#[test]
fn both_kernels_empty_sides_and_partial_quotas_are_one_grouped_exchange() {
    ordinary_cases::<false>();
    ordinary_cases::<true>();
}

#[test]
fn machinery_uses_each_sides_original_extrema_and_has_separate_followups() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B028", Zone::Junk);
    let low = base(&mut f, "B001", Zone::Hand(B));
    let high = base(&mut f, "B032", Zone::Hand(B));
    let own = base(&mut f, "B077", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    start(&mut game, source, 0, BoxedValues::default(), false);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Hand(A)), &[high]);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[low, own]);
    let card = base_card("B028").unwrap();
    assert_eq!(card.effects.len(), 3);
    assert_eq!(
        card.effects[1].instructions,
        [build::verb(
            Verb::Score,
            build::filtered(&build::HAND, &[Filter::Has(Test::Icon(Icon::Authority))])
        )
        .to(Destination::Score(Who::Executor))
        .verify()
        .reveal_if_empty(&build::HAND)]
    );
}

#[test]
fn sanitation_takes_next_highest_after_unique_highest() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B069", Zone::Junk);
    let low = base(&mut f, "B001", Zone::Hand(A));
    let middle = base(&mut f, "B032", Zone::Hand(A));
    let high = base(&mut f, "B077", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    start(&mut game, source, 0, BoxedValues::default(), false);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Hand(A)), &[low]);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[middle, high]);
}

#[test]
fn private_exchange_groups_only_public_backs_and_samples_native_rng() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B029", Zone::Junk);
    base(&mut f, "B001", Zone::Score(B));
    base(&mut f, "B004", Zone::Score(B));
    f.figure(Figure::Gilgamesh, Zone::Score(B));
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 71).unwrap();
    start(&mut game, source, 0, BoxedValues::default(), false);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.iter().all(|option| matches!(
        option,
        Alternative::BlindSelect {
            source: Zone::Score(B),
            back: Deck { age: 1, .. },
            ..
        }
    )));
    let mut expected_rng = game.rng.clone();
    expected_rng.next_u64();
    let mut cloned = game.clone();
    let choice = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::BlindSelect {
                    back: Deck {
                        set: CardSet::Base,
                        ..
                    },
                    ..
                }
            )
        })
        .unwrap();
    game.choose(choice).unwrap();
    cloned.choose(choice).unwrap();
    let mut clone_events = Vec::new();
    finish(&mut game, &mut events);
    finish(&mut cloned, &mut clone_events);
    assert_eq!(game, cloned);
    assert_eq!(events, clone_events);
    assert_eq!(game.rng, expected_rng);
}

#[test]
fn private_singleton_is_automatic_and_consumes_no_rng() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B029", Zone::Junk);
    base(&mut f, "B001", Zone::Score(B));
    let mut game = Game::seeded(f.position(), Expansions::BASE, 91).unwrap();
    let rng = game.rng.clone();
    start(&mut game, source, 0, BoxedValues::default(), false);
    assert_eq!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.rng, rng);
}

fn branches<const F: bool>() {
    for code in ["B018", "B072"] {
        let mut f = Fixture::default();
        let source = base(&mut f, code, Zone::Junk);
        base(&mut f, "B001", Zone::Hand(A));
        base(&mut f, "B032", Zone::Deck(Deck::base(3)));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        let before = game.position.clone();
        let rng = game.rng.clone();
        start(&mut game, source, 0, BoxedValues::default(), false);
        let mut events = Vec::new();
        branch(&mut game, &mut events, Alternative::Decline);
        finish(&mut game, &mut events);
        assert_eq!(game.position, before);
        assert_eq!(game.rng, rng);
        assert!(events.is_empty());
    }
    for increase in [0, 1, 9] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B018", Zone::Junk);
        let three = base(&mut f, "B032", Zone::Deck(Deck::base(3)));
        let four = base(&mut f, "B036", Zone::Deck(Deck::base(4)));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        let mut boxed = BoxedValues::default();
        boxed.increase(increase).unwrap();
        start(&mut game, source, 0, boxed, false);
        let mut events = Vec::new();
        branch(&mut game, &mut events, Alternative::Branch(1));
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position
                .cards(Zone::Deck(Deck::base(3)))
                .contains(&three),
            increase != 0
        );
        assert_eq!(
            game.position
                .cards(Zone::Deck(Deck::base(4)))
                .contains(&four),
            increase != 1
        );
    }
}
#[test]
fn whole_branch_decline_and_exact_boxed_deck_boundaries() {
    branches::<false>();
    branches::<true>();
}

#[test]
fn evolution_score_then_mandatory_return_and_unboxed_highest_plus_one() {
    for branch_index in [0, 1] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B072", Zone::Junk);
        let low = base(&mut f, "B001", Zone::Score(A));
        let computed = base(&mut f, "B018", Zone::Deck(Deck::base(2)));
        let drawn = base(&mut f, "B088", Zone::Deck(Deck::base(9)));
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        let mut boxed = BoxedValues::default();
        boxed.increase(1).unwrap();
        start(&mut game, source, 0, boxed, false);
        let mut events = Vec::new();
        branch(&mut game, &mut events, Alternative::Branch(branch_index));
        finish(&mut game, &mut events);
        if branch_index == 0 {
            assert_eq!(game.position.cards(Zone::Score(A)), &[drawn]);
            assert_eq!(game.position.cards(Zone::Deck(Deck::base(1))), &[low]);
        } else {
            assert_eq!(game.position.cards(Zone::Score(A)), &[low]);
            assert_eq!(game.position.cards(Zone::Hand(A)), &[computed]);
        }
    }
}

#[test]
fn medicine_followup_is_mandatory_standard_three_or_four_not_boxed() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B029", Zone::Junk);
    let two = base(&mut f, "B018", Zone::AvailableAchievements);
    let three = base(&mut f, "B032", Zone::AvailableAchievements);
    let five = base(&mut f, "B046", Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut boxed = BoxedValues::default();
    boxed.increase(2).unwrap();
    start(&mut game, source, 1, boxed, false);
    assert_eq!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        game.position.cards(Zone::AvailableAchievements),
        &[two, five]
    );
    assert!(game.position.cards(Zone::Junk).contains(&three));
}

#[test]
fn overlap_is_frozen_and_zero_movers_do_not_invoke_yi() {
    for overlap_only in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B018", Zone::Junk);
        f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
        let hand = base(&mut f, "B032", Zone::Hand(A));
        let score = if overlap_only {
            None
        } else {
            Some(base(&mut f, "B033", Zone::Score(A)))
        };
        if overlap_only {
            f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, source, 0, BoxedValues::default(), false);
        let mut events = Vec::new();
        branch(&mut game, &mut events, Alternative::Branch(0));
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(game.position.cards(Zone::Hand(A)).contains(&hand));
        if let Some(score) = score {
            assert!(game.position.cards(Zone::Hand(A)).contains(&score));
        }
        assert_eq!(grouped(&events).len(), usize::from(!overlap_only));
    }
}

#[test]
fn yi_replaces_the_entire_selected_exchange_once_unless_suppressed() {
    for suppressed in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B069", Zone::Junk);
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        base(&mut f, "B032", Zone::Hand(A));
        base(&mut f, "B033", Zone::Hand(A));
        base(&mut f, "B001", Zone::Hand(B));
        let draw = base(&mut f, "B036", Zone::Deck(Deck::base(4)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, source, 0, BoxedValues::default(), suppressed);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(grouped(&events).len(), usize::from(suppressed));
        assert_eq!(
            game.position
                .cards(Zone::Deck(Deck::base(4)))
                .contains(&draw),
            suppressed
        );
        if !suppressed {
            assert!(
                events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Moved(m) => Some(m),
                        _ => None,
                    })
                    .all(|m| m.inside_karma)
            );
        }
    }
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng, Status)> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            game.position.junk.sort_unstable();
            vec![(game.position, game.rng, status)]
        }
    }
}
fn oracle<const F: bool>() {
    for code in ["B018", "B069"] {
        let mut f = Fixture::default();
        let source = base(&mut f, code, Zone::Junk);
        for card in ["B032", "B033", "B034"] {
            base(
                &mut f,
                card,
                if code == "B018" {
                    Zone::Deck(Deck::base(3))
                } else {
                    Zone::Hand(A)
                },
            );
        }
        let mut reduced = Game::seeded(f.position(), Expansions::with_figures(F), 71).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, 0, BoxedValues::default(), false);
        start(&mut manual, source, 0, BoxedValues::default(), false);
        if code == "B018" {
            branch(&mut reduced, &mut Vec::new(), Alternative::Branch(1));
            branch(&mut manual, &mut Vec::new(), Alternative::Branch(1));
        }
        let reduced_paths = outcomes(reduced.clone(), 0);
        let manual_paths = outcomes(manual.clone(), 0);
        assert!(reduced_paths.len() < manual_paths.len());
        assert!(manual_paths.iter().all(|p| reduced_paths.contains(p)));
        assert!(reduced_paths.iter().all(|p| manual_paths.contains(p)));
        let mut events = Vec::new();
        for _ in 0..200 {
            let mut cloned = reduced.clone();
            let mut step = Vec::new();
            let mut clone_events = Vec::new();
            let status = reduced.advance(1, &mut step).unwrap();
            assert_eq!(cloned.advance(1, &mut clone_events).unwrap(), status);
            assert_eq!(cloned, reduced);
            assert_eq!(step, clone_events);
            events.extend(step);
            match status {
                Status::Decision(menu) => {
                    let index = if code == "B018" {
                        menu.options.len() - 1
                    } else {
                        0
                    };
                    reduced.choose(index).unwrap();
                }
                Status::WorkLimit => {}
                _ => break,
            }
        }
        let mut expected = Vec::new();
        while let Status::Decision(menu) = settle(&mut manual, &mut expected) {
            manual
                .choose(if code == "B018" {
                    menu.options.len() - 1
                } else {
                    0
                })
                .unwrap();
        }
        assert_eq!(events, expected);
        manual.shortcuts_disabled = false;
        assert_eq!(reduced, manual);
    }
}
#[test]
fn all_real_subsets_and_canonical_events_rng_clone_boundaries_match_unrestricted() {
    oracle::<false>();
    oracle::<true>();
}
