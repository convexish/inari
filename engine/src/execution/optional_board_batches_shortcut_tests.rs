use super::common::*;
use super::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn invoke(game: &mut Game, source: CardId, effect: usize, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: effect.try_into().unwrap(),
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn fixture(code: &str) -> (Position, CardId, [CardId; 3]) {
    let mut f = Fixture::default();
    let source = base(&mut f, code, Zone::Junk);
    let origin = if code == "B033" {
        Zone::Score(A)
    } else {
        Zone::Hand(A)
    };
    let cards = ["B001", "B002", "B009"].map(|code| base(&mut f, code, origin));
    if code == "B085" {
        base(&mut f, "B003", Zone::Board(A, Color::Red));
    }
    if code == "B089" {
        for draw in ["B004", "B006", "B009"] {
            base(&mut f, draw, Zone::Deck(Deck::base(1)));
        }
    }
    (f.position(), source, cards)
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Status)> {
    assert!(depth < 8);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert!(game.position.check_conservation());
            // Junk is the only unordered zone; every hand, score, deck and
            // board stack is compared in its exact physical order.
            game.position.junk.sort_unstable();
            vec![(game.position, status)]
        }
    }
}

fn subset_oracle<const F: bool>() {
    for code in ["B006", "B089"] {
        let (position, source, cards) = fixture(code);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
        let mut unrestricted = reduced.clone();
        unrestricted.shortcuts_disabled = true;
        invoke(&mut reduced, source, 0, Context::new(A));
        invoke(&mut unrestricted, source, 0, Context::new(A));
        let reduced = outcomes(reduced, 0);
        let unrestricted = outcomes(unrestricted, 0);
        assert_eq!(unrestricted.len(), 16);
        assert_eq!(reduced.len(), 10);
        assert!(unrestricted.iter().all(|result| reduced.contains(result)));
        assert!(reduced.iter().all(|result| unrestricted.contains(result)));
        // Enumerate all eight subsets, including empty, and both orders of
        // the same-color pair. No minimum-color-first subset is forced.
        for mask in 0..8 {
            assert!(
                reduced
                    .iter()
                    .any(|(p, _)| cards.iter().enumerate().all(|(index, card)| {
                        p.cards(Zone::Hand(A)).contains(card) == (mask & (1 << index) == 0)
                    }))
            );
        }
        for order in [[cards[0], cards[1]], [cards[1], cards[0]]] {
            assert!(
                reduced
                    .iter()
                    .any(|(p, _)| p.cards(Zone::Board(A, Color::Red)) == order)
            );
        }
    }
}

#[test]
fn optional_grouping_preserves_all_subsets_stops_and_same_color_orders_in_both_kernels() {
    subset_oracle::<false>();
    subset_oracle::<true>();
}

fn committed_oracle<const F: bool>() {
    for code in ["B033", "B085"] {
        let (position, source, _) = fixture(code);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
        let mut unrestricted = reduced.clone();
        unrestricted.shortcuts_disabled = true;
        invoke(&mut reduced, source, 0, Context::new(A));
        invoke(&mut unrestricted, source, 0, Context::new(A));
        let reduced = outcomes(reduced, 0);
        let unrestricted = outcomes(unrestricted, 0);
        assert_eq!(unrestricted.len(), 7);
        assert_eq!(reduced.len(), 3);
        assert!(unrestricted.iter().all(|result| reduced.contains(result)));
        assert!(reduced.iter().all(|result| unrestricted.contains(result)));
    }
}

#[test]
fn whole_translation_and_socialism_commitments_preserve_every_outcome() {
    committed_oracle::<false>();
    committed_oracle::<true>();
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(((game.position.card(*card).color as u8, card.0), index)),
            Alternative::Move {
                target: TargetDescriptor::Hidden { slot, .. },
                ..
            } => Some(((0, u16::MAX - *slot as u16), index)),
            _ => None,
        })
        .min_by_key(|&(key, _)| key)
        .unwrap()
        .1
}

fn finish(game: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                return;
            }
        }
    }
    panic!("canonical phase did not complete");
}

fn canonical_oracle<const F: bool>() {
    for code in ["B006", "B033", "B085", "B089"] {
        let (position, source, _) = fixture(code);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 73).unwrap();
        let mut unrestricted = reduced.clone();
        unrestricted.shortcuts_disabled = true;
        invoke(&mut reduced, source, 0, Context::new(A));
        invoke(&mut unrestricted, source, 0, Context::new(A));
        let mut clone = reduced.clone();
        let mut events = Vec::new();
        let mut expected_events = Vec::new();
        let mut clone_events = Vec::new();
        finish(&mut reduced, &mut events);
        finish(&mut unrestricted, &mut expected_events);
        finish(&mut clone, &mut clone_events);
        assert_eq!(events, expected_events);
        assert_eq!(events, clone_events);
        unrestricted.shortcuts_disabled = false;
        assert_eq!(reduced, unrestricted); // Includes RNG and all continuations.
        assert_eq!(reduced, clone);
    }
}

#[test]
fn canonical_paths_have_exact_events_rng_and_clone_parity() {
    canonical_oracle::<false>();
    canonical_oracle::<true>();
}

#[test]
fn subset_initial_menu_contains_all_colors_and_floor_preserves_same_color_choices() {
    let (position, source, cards) = fixture("B006");
    let mut game = Game::new(position, Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    for card in cards {
        assert!(menu.options.iter().any(|o| matches!(o,
            Alternative::Move { target: TargetDescriptor::Known { card: candidate, .. }, .. } if *candidate == card)));
    }
    choose_card(&mut game, &mut events, cards[0]);
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.contains(&Alternative::Decline));
    assert!(menu.options.iter().any(|o| matches!(o,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == cards[1])));
}

#[test]
fn membership_guards_are_live_even_when_would_is_suppressed() {
    for (code, figure) in [
        ("B033", Figure::Michelangelo),
        ("B089", Figure::EmperorMeiji),
    ] {
        for suppressed in [false, true] {
            let mut f = Fixture::default();
            let source = base(&mut f, code, Zone::Junk);
            let native = figure_card(figure.code()).unwrap();
            let color = native.color;
            f.add(native, Zone::Board(A, color));
            let own = if code == "B033" {
                Zone::Score(A)
            } else {
                Zone::Hand(A)
            };
            let cards = [base(&mut f, "B001", own), base(&mut f, "B008", own)];
            let borrowed = base(
                &mut f,
                "B002",
                if code == "B033" {
                    Zone::Hand(A)
                } else {
                    Zone::Hand(B)
                },
            );
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let mut context = Context::new(A);
            context.set_suppress_karma(suppressed);
            invoke(&mut game, source, 0, context);
            let menu = decision(&mut game, &mut Vec::new());
            for card in [cards[0], cards[1], borrowed] {
                assert!(menu.options.iter().any(|o| matches!(o,
                    Alternative::Move { target: TargetDescriptor::Known { card: candidate, .. }, .. } if *candidate == card)));
            }
            let dependencies = Game::move_dependencies(
                Verb::Meld,
                if code == "B033" {
                    build::SCORE
                } else {
                    build::filtered(&build::HAND, &[Filter::Has(Test::Icon(Icon::Authority))])
                },
                Destination::Board(Who::Executor),
            );
            assert!(
                !game
                    .position
                    .dependencies_clear(&dependencies, context, false)
            );
        }
    }
}

#[test]
fn movement_hooks_and_candidate_figures_disable_the_entire_remaining_phase() {
    for candidate in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B033", Zone::Junk);
        let first = base(&mut f, "B001", Zone::Score(A));
        let second = base(&mut f, "B008", Zone::Score(A));
        let zone = if candidate {
            Zone::Score(A)
        } else {
            Zone::Board(A, Color::Red)
        };
        let figure = f.figure(Figure::Kalashnikov, zone);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut game, source, 0, Context::new(A));
        let menu = decision(&mut game, &mut Vec::new());
        for card in [first, second] {
            assert!(menu.options.iter().any(|o| matches!(o,
                Alternative::Move { target: TargetDescriptor::Known { card: candidate, .. }, .. } if *candidate == card)));
        }
        if candidate {
            assert!(menu.options.iter().any(|o| matches!(o,
                Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == figure)));
        }
    }
}

#[test]
fn suppressed_movement_hooks_do_not_block_proven_translation_grouping() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B033", Zone::Junk);
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    let cards = [
        base(&mut f, "B001", Zone::Score(A)),
        base(&mut f, "B008", Zone::Score(A)),
    ];
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.set_suppress_karma(true);
    invoke(&mut game, source, 0, context);
    let menu = decision(&mut game, &mut Vec::new());
    let expected = *cards
        .iter()
        .min_by_key(|&&card| game.position.card(card).color as u8)
        .unwrap();
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.contains(&Alternative::Decline));
    assert!(menu.options.iter().any(|o| matches!(o,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == expected)));
}

#[test]
fn foreign_control_wrong_destinations_and_quotas_fail_closed() {
    let (position, _, _) = fixture("B033");
    let game = Game::new(position, Expansions::FIGURES).unwrap();
    let context = Context::new(A);
    let original = game
        .select_bound(build::SCORE, context, None, SelectionParameter::None)
        .unwrap();
    for variant in 0..3 {
        let mut batch = MoveAll::new(
            Verb::Meld,
            build::SCORE,
            Destination::Board(Who::Executor),
            context,
        );
        batch.stop = Stop::Initially;
        match variant {
            // A foreign controller cannot see the score pile.
            0 => batch.context.controller = B,
            // Scoring is not a board placement.
            1 => batch.to = Destination::Score(Who::Executor),
            // A quota keeps every pair reachable by one colour order: the
            // lone green (highest colour) waits until a red is taken.
            _ => batch.limit = Some(2),
        }
        let mut targets = original.clone();
        {
            batch.shortcut = Some(Shortcut::ByColor);
            game.reduce_batch_moves(&mut batch, &mut targets)
        };
        if variant < 2 {
            assert_eq!(targets, original, "variant {variant}");
        } else {
            let reds: Vec<Target> = original
                .iter()
                .copied()
                .filter(|target| game.position.card(target.card).color == Color::Red)
                .collect();
            assert_eq!(reds.len(), 2);
            assert_eq!(targets, reds);
        }
    }
}

fn deck_oracle<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B089", Zone::Junk);
    let cards = ["B088", "B089", "B091"].map(|code| base(&mut f, code, Zone::Deck(Deck::base(9))));
    let mut reduced = Game::seeded(f.position(), Expansions::with_figures(F), 89).unwrap();
    let mut unrestricted = reduced.clone();
    unrestricted.shortcuts_disabled = true;
    invoke(&mut reduced, source, 1, Context::new(A));
    invoke(&mut unrestricted, source, 1, Context::new(A));
    let reduced_outcomes = outcomes(reduced.clone(), 0);
    let all = outcomes(unrestricted.clone(), 0);
    assert_eq!(reduced_outcomes.len(), 2);
    assert_eq!(all.len(), 7);
    assert!(all.iter().all(|result| reduced_outcomes.contains(result)));
    assert!(reduced_outcomes.iter().all(|result| all.contains(result)));
    let mut clone = reduced.clone();
    let mut events = Vec::new();
    let mut expected = Vec::new();
    let mut cloned = Vec::new();
    finish(&mut reduced, &mut events);
    finish(&mut unrestricted, &mut expected);
    finish(&mut clone, &mut cloned);
    assert_eq!(events, expected);
    assert_eq!(events, cloned);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        cards.into_iter().rev().collect::<Vec<_>>()
    );
    unrestricted.shortcuts_disabled = false;
    assert_eq!(reduced, unrestricted);
    assert_eq!(reduced, clone);
}

#[test]
fn suburbia_deck_proof_preserves_all_outcomes_and_exact_canonical_trace() {
    deck_oracle::<false>();
    deck_oracle::<true>();
}

#[test]
fn foreign_controller_gets_unrestricted_hidden_score_backs_not_color_information() {
    let (position, source, _) = fixture("B033");
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    invoke(&mut game, source, 0, context);
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options.len(), 4);
    assert_eq!(menu.player, B);
    assert!(menu.options.iter().all(|option| matches!(
        option,
        Alternative::Decline
            | Alternative::Move {
                target: TargetDescriptor::Hidden {
                    source: Zone::Score(A),
                    ..
                },
                ..
            }
    )));
}

#[test]
fn a_newly_proved_subset_suffix_does_not_inherit_an_unrestricted_color_floor() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B089", Zone::Junk);
    let figure = f.figure(Figure::WilliamShakespeare, Zone::Hand(A));
    let cards = ["B001", "B002"].map(|code| base(&mut f, code, Zone::Hand(A)));
    for code in ["B004", "B008", "B009"] {
        base(&mut f, code, Zone::Deck(Deck::base(1)));
    }
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    assert!(reduced.position.card(figure).color as u8 > Color::Red as u8);
    let mut unrestricted = reduced.clone();
    unrestricted.shortcuts_disabled = true;
    invoke(&mut reduced, source, 0, Context::new(A));
    invoke(&mut unrestricted, source, 0, Context::new(A));
    let mut events = Vec::new();
    let mut expected = Vec::new();
    choose_card(&mut reduced, &mut events, figure);
    choose_card(&mut unrestricted, &mut expected, figure);
    let menu = decision(&mut reduced, &mut events);
    assert_eq!(menu, decision(&mut unrestricted, &mut expected));
    assert!(menu.options.contains(&Alternative::Decline));
    for card in cards {
        assert!(menu.options.iter().any(|o| matches!(o,
            Alternative::Move { target: TargetDescriptor::Known { card: candidate, .. }, .. } if *candidate == card)));
    }
    finish(&mut reduced, &mut events);
    finish(&mut unrestricted, &mut expected);
    assert_eq!(events, expected);
    unrestricted.shortcuts_disabled = false;
    assert_eq!(reduced, unrestricted);
}
