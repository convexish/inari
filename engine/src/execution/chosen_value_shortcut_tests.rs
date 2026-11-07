use super::common::*;
use super::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn fixture<const F: bool>(code: &str) -> (Position, CardId) {
    let mut f = Fixture::default();
    let source = base(&mut f, code, Zone::Junk);
    if code == "B069" {
        for code in ["B067", "B068", "B071"] {
            base(&mut f, code, Zone::Deck(Deck::base(7)));
        }
    } else {
        for code in ["B001", "B002", "B008"] {
            base(&mut f, code, Zone::Score(A));
        }
        if code == "B081" {
            base(&mut f, "B016", Zone::Hand(A));
            if F {
                f.figure(Figure::Gilgamesh, Zone::Score(B));
            } else {
                base(&mut f, "B004", Zone::Score(B));
            }
        } else if F && code != "B063" {
            f.figure(Figure::Gilgamesh, Zone::Score(A));
        }
    }
    (f.position(), source)
}
fn start(game: &mut Game, source: CardId, code: &str) {
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: if code == "B069" { 1 } else { 0 },
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}
fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            let key = match option {
                Alternative::Value(value) => (0, *value, 0, 0, 0),
                Alternative::Move {
                    target: TargetDescriptor::Known { card, .. },
                    verb,
                } => {
                    let definition = game.position.card(*card);
                    let deck = definition.origin_deck();
                    if *verb == Verb::Meld {
                        (1, definition.color as u8, 0, 0, card.0)
                    } else {
                        (1, deck.age, deck.set as u8, 1, card.0)
                    }
                }
                Alternative::Move {
                    target:
                        TargetDescriptor::Hidden {
                            slot,
                            back: Deck { age: value, set },
                            ..
                        },
                    ..
                } => (1, *value, *set as u8, 0, u16::MAX - *slot as u16),
                Alternative::BlindMove {
                    back: Deck { age: value, set },
                    ..
                } => (1, *value, *set as u8, 0, 0),
                Alternative::Karma { .. } => (2, 0, 0, 0, 0),
                _ => return None,
            };
            Some((key, index))
        })
        .min_by_key(|&(key, _)| key)
        .unwrap()
        .1
}
fn finish(game: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                return;
            }
        }
    }
    panic!("canonical chosen phase did not complete");
}
// Hands, score piles and junk are unordered: only their contents matter.
fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}
fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng, Status)> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut fork = game.clone();
                fork.choose(index).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert!(game.position.check_conservation());
            vec![(normalized(game.position), game.rng, status)]
        }
    }
}
fn all_paths<const F: bool>() {
    for code in ["B048", "B063", "B069", "B081", "B094", "B108"] {
        let (position, source) = fixture::<F>(code);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 97).unwrap();
        let mut unrestricted = reduced.clone();
        unrestricted.shortcuts_disabled = true;
        start(&mut reduced, source, code);
        start(&mut unrestricted, source, code);
        let reduced = outcomes(reduced, 0);
        let unrestricted = outcomes(unrestricted, 0);
        assert!(
            reduced.iter().all(|outcome| unrestricted.contains(outcome)),
            "{code}: added an outcome"
        );
        assert!(
            unrestricted.iter().all(|outcome| reduced.contains(outcome)),
            "{code}: lost an outcome"
        );
        if matches!(code, "B063" | "B069") || F {
            assert!(
                reduced.len() < unrestricted.len(),
                "{code}: proof never reduced"
            );
        }
    }
}
#[test]
fn all_value_choices_and_commuting_orders_preserve_every_outcome() {
    all_paths::<false>();
    all_paths::<true>();
}
fn canonical_oracle<const F: bool>() {
    for code in ["B048", "B063", "B069", "B081", "B094", "B108"] {
        let (position, source) = fixture::<F>(code);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 113).unwrap();
        let mut unrestricted = reduced.clone();
        unrestricted.shortcuts_disabled = true;
        start(&mut reduced, source, code);
        start(&mut unrestricted, source, code);
        let mut events = Vec::new();
        for _ in 0..1000 {
            let mut clone = reduced.clone();
            let mut step = Vec::new();
            let mut cloned = Vec::new();
            let status = reduced.advance(1, &mut step).unwrap();
            assert_eq!(clone.advance(1, &mut cloned).unwrap(), status);
            assert_eq!(step, cloned);
            assert_eq!(reduced, clone);
            events.extend(step);
            match status {
                Status::WorkLimit => {}
                Status::Decision(menu) => reduced.choose(canonical(&reduced, &menu)).unwrap(),
                Status::Complete(ReturnValue::Unit) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(reduced.frames.is_empty());
        let mut expected = Vec::new();
        finish(&mut unrestricted, &mut expected);
        assert_eq!(events, expected, "{code}");
        unrestricted.shortcuts_disabled = false;
        assert_eq!(reduced, unrestricted, "{code}");
    }
}
#[test]
fn canonical_events_rng_and_every_work_boundary_clone_match() {
    canonical_oracle::<false>();
    canonical_oracle::<true>();
}

#[test]
fn foreign_control_and_open_stops_keep_every_order() {
    let (position, _) = fixture::<false>("B063");
    let game = Game::new(position, Expansions::BASE).unwrap();
    let context = Context::new(A);
    let targets = game
        .select_bound(CHOSEN_SCORE, context, None, SelectionParameter::Value(1))
        .unwrap();
    for variant in 0..2 {
        let mut batch = MoveAll::new(
            Verb::Meld,
            CHOSEN_SCORE,
            Destination::Board(Who::Executor),
            context,
        );
        batch.stop = Stop::Initially;
        batch.regs.chosen = SelectionParameter::Value(1);
        match variant {
            0 => batch.context.controller = B,
            _ => batch.stop = Stop::AnyTime,
        }
        let mut actual = targets.clone();
        {
            batch.shortcut = Some(Shortcut::ByColor);
            game.reduce_batch_moves(&mut batch, &mut actual)
        };
        assert_eq!(actual, targets);
    }
}

#[test]
fn chosen_register_and_retained_card_are_independent_and_absence_is_not_zero() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B063", Zone::Junk);
    let retained = base(&mut f, "B016", Zone::Score(A));
    let chosen = base(&mut f, "B001", Zone::Score(A));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    start(&mut game, source, "B063");
    let menu = decision(&mut game, &mut Vec::new());
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(1))
            .unwrap(),
    )
    .unwrap();
    let Some(Frame::Program {
        retained: binding, ..
    }) = game.frames.last_mut()
    else {
        panic!("value register caller");
    };
    *binding = Some(Captured::new(retained, Evidence::PUBLIC));
    assert_eq!(
        game.query_value(
            Expr::Value(Reg::Bound),
            Context::new(A),
            Some(retained),
            SelectionParameter::Value(1)
        ),
        Some(2)
    );
    assert_eq!(
        game.query_value(
            Expr::Chosen,
            Context::new(A),
            Some(retained),
            SelectionParameter::Value(1)
        ),
        Some(1)
    );
    assert_eq!(
        game.query_value(
            Expr::Chosen,
            Context::new(A),
            Some(retained),
            SelectionParameter::None
        ),
        None
    );
    assert!(
        game.select_bound(
            CHOSEN_SCORE,
            Context::new(A),
            Some(retained),
            SelectionParameter::None
        )
        .unwrap()
        .is_empty()
    );
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.top(A, Color::Red), Some(chosen));
    assert_eq!(game.position.cards(Zone::Score(A)), [retained]);
}

#[test]
fn introduced_age_zero_extends_ordinary_domain_but_not_printed_boxes() {
    for introduced in [false, true] {
        let mut f = Fixture::default();
        let source = f.tech(
            Color::Red,
            1,
            Zone::Junk,
            vec![build::choose(
                Who::Executor,
                Choice::Value(ValueChoices::Any),
            )],
        );
        let mut position = f.position();
        position.age_zero = Some(Box::new(crate::model::AgeZero {
            reserve: Vec::new(),
            introduced,
        }));
        let mut game = Game::new(position, Expansions::FIGURES).unwrap();
        start(&mut game, source, "any value");
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.options.contains(&Alternative::Value(0)), introduced);
        for value in 1..=11 {
            assert!(menu.options.contains(&Alternative::Value(value)));
        }
    }
    // Encyclopedia: an absent value is the same as declining its meld.
    let mut f = Fixture::default();
    let source = base(&mut f, "B063", Zone::Junk);
    for code in ["B001", "B019", "B037", "B039"] {
        base(&mut f, code, Zone::Score(A));
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, source, "B063");
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options, [1, 2, 4].map(Alternative::Value).to_vec());
    // Sanitation boxed past 11: a nonempty 11 keeps the choice open.
    let mut f = Fixture::default();
    let source = base(&mut f, "B069", Zone::Junk);
    for code in ["B067", "B068", "B071"] {
        base(&mut f, code, Zone::Deck(Deck::base(7)));
    }
    let eleven = base(&mut f, "B106", Zone::Deck(Deck::base(11)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut boxed = BoxedValues::default();
    boxed.increase(4).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 1,
            boxed,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.contains(&Alternative::Value(11)));
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(12))
            .unwrap(),
    )
    .unwrap();
    finish(&mut game, &mut events);
    assert!(events.is_empty());
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(7))).len(), 3);
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(11))), [eleven]);
}

#[test]
fn live_membership_and_movement_dependencies_restore_ordinary_menus() {
    for figure in [Figure::Nightingale, Figure::Michelangelo] {
        let (position, source) = fixture::<true>("B063");
        let mut f = Fixture {
            cards: position.catalog.to_vec(),
            placements: Vec::new(),
        };
        for index in 0..f.cards.len() {
            let card = CardId(index as u16);
            f.placements.push((card, position.locate(card).unwrap()));
        }
        let definition = figure_card(figure.code()).unwrap();
        let color = definition.color;
        f.add(definition, Zone::Board(A, color));
        let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, "B063");
        start(&mut manual, source, "B063");
        for game in [&mut reduced, &mut manual] {
            // Michelangelo's hand cards widen the values present.
            let menu = decision(game, &mut Vec::new());
            if let Some(one) = menu
                .options
                .iter()
                .position(|o| *o == Alternative::Value(1))
            {
                game.choose(one).unwrap();
            }
        }
        assert_eq!(
            decision(&mut reduced, &mut Vec::new()),
            decision(&mut manual, &mut Vec::new())
        );
    }
}

#[test]
fn swapped_hidden_score_worlds_have_identical_back_only_return_menus() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B081", Zone::Junk);
    base(&mut f, "B016", Zone::Hand(A));
    let one = base(&mut f, "B001", Zone::Score(B));
    let two = base(&mut f, "B002", Zone::Score(B));
    let three = base(&mut f, "B008", Zone::Score(B));
    f.figure(Figure::Gilgamesh, Zone::Score(B));
    let position = f.position();
    let mut alternate = position.clone();
    alternate.players[B.index()].score.swap(0, 2);
    let mut original = Game::seeded(position, Expansions::FIGURES, 91).unwrap();
    let mut swapped = Game::seeded(alternate, Expansions::FIGURES, 91).unwrap();
    for game in [&mut original, &mut swapped] {
        start(game, source, "B081");
        let menu = decision(game, &mut Vec::new());
        game.choose(canonical(game, &menu)).unwrap();
        let menu = decision(game, &mut Vec::new());
        game.choose(
            menu.options
                .iter()
                .position(|o| *o == Alternative::Value(1))
                .unwrap(),
        )
        .unwrap();
    }
    let original_menu = decision(&mut original, &mut Vec::new());
    assert_eq!(original_menu, decision(&mut swapped, &mut Vec::new()));
    assert!(original_menu.options.iter().all(
        |o| matches!(o, Alternative::BlindMove { source: Zone::Score(player), .. } if *player == B)
    ));
    let mut manual = original.clone();
    manual.shortcuts_disabled = true;
    let reduced = outcomes(original, 0);
    let unrestricted = outcomes(manual, 0);
    assert!(reduced.iter().all(|outcome| unrestricted.contains(outcome)));
    assert!(unrestricted.iter().all(|outcome| reduced.contains(outcome)));
    for (position, _, _) in reduced {
        for card in [one, two, three] {
            assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&card));
        }
    }
}

#[test]
fn suppression_never_erases_live_score_membership_but_exact_decks_ignore_pile_hooks() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B063", Zone::Junk);
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let borrowed = base(&mut f, "B001", Zone::Hand(A));
    let physical = base(&mut f, "B008", Zone::Score(A));
    let game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.source_card = Some(source);
    context.set_suppress_karma(true);
    assert!(!game.position.dependencies_clear(
        &Game::move_dependencies(Verb::Meld, CHOSEN_SCORE, Destination::Board(Who::Executor)),
        context,
        false
    ));
    let targets = game
        .select_bound(CHOSEN_SCORE, context, None, SelectionParameter::Value(1))
        .unwrap();
    assert!(targets.iter().any(|target| target.card == borrowed));
    assert!(targets.iter().any(|target| target.card == physical));
    let mut batch = MoveAll::new(
        Verb::Meld,
        CHOSEN_SCORE,
        Destination::Board(Who::Executor),
        context,
    );
    batch.stop = Stop::Initially;
    batch.regs.chosen = SelectionParameter::Value(1);
    let mut reduced = targets.clone();
    {
        batch.shortcut = Some(Shortcut::ByColor);
        game.reduce_batch_moves(&mut batch, &mut reduced)
    };
    assert_eq!(reduced, targets);

    let mut f = Fixture::default();
    let source = base(&mut f, "B069", Zone::Junk);
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    f.figure(Figure::Nightingale, Zone::Board(B, Color::Yellow));
    for code in ["B067", "B068", "B071"] {
        base(&mut f, code, Zone::Deck(Deck::base(7)));
    }
    let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, "B069");
    start(&mut manual, source, "B069");
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert!(reduced.len() < manual.len());
    assert!(manual.iter().all(|outcome| reduced.contains(outcome)));
}

#[test]
fn sanitation_skips_its_deck_choice_only_when_both_boxed_decks_are_empty() {
    // Base cards of ages 7, 8 and 9.
    const AGED: [&str; 3] = ["B068", "B081", "B091"];
    for archimedes in [false, true] {
        let low = 7 + u8::from(archimedes);
        // The deck a literal 7-or-8 test would read instead of the boxes.
        let decoy_age = if archimedes { 7 } else { 9 };
        for (empty, decoy) in (0..4).flat_map(|empty| [(empty, false), (empty, true)]) {
            let mut f = Fixture::default();
            let source = base(&mut f, "B069", Zone::Junk);
            if archimedes {
                f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
            }
            for age in [low, low + 1] {
                if empty & (1 << (age - low)) == 0 {
                    base(
                        &mut f,
                        AGED[usize::from(age - 7)],
                        Zone::Deck(Deck::base(age)),
                    );
                }
            }
            if decoy {
                let code = AGED[usize::from(decoy_age - 7)];
                base(&mut f, code, Zone::Deck(Deck::base(decoy_age)));
            }
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let before = game.position.clone();
            start(&mut game, source, "B069");
            let mut events = Vec::new();
            if empty == 3 {
                assert_eq!(
                    settle(&mut game, &mut events),
                    Status::Complete(ReturnValue::Unit)
                );
                assert!(events.is_empty());
                assert_eq!(game.position, before);
                continue;
            }
            let menu = decision(&mut game, &mut events);
            assert_eq!(
                menu.options,
                [low, low + 1].map(Alternative::Value).to_vec(),
                "Archimedes {archimedes}, empty {empty:#b}, decoy {decoy}"
            );
            game.choose(1).unwrap();
            finish(&mut game, &mut events);
            for age in 7..=9 {
                let deck = Zone::Deck(Deck::base(age));
                let expected = if age == low + 1 {
                    0
                } else {
                    before.cards(deck).len()
                };
                assert_eq!(game.position.cards(deck).len(), expected);
            }
        }
    }
}

#[test]
fn sanitation_empty_deck_skip_matches_both_unreduced_choices_under_archimedes() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B069", Zone::Junk);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    // Only the unboxed 7 deck has cards; Archimedes names 8 and 9.
    base(&mut f, "B068", Zone::Deck(Deck::base(7)));
    let mut reduced = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    let original_catalog = reduced.position.catalog.clone();
    let mut expanded = reduced.clone();
    expanded.shortcuts_disabled = true;
    // Explicit reference program retains the two printed, empty-deck choices.
    std::sync::Arc::make_mut(&mut expanded.position.catalog)[usize::from(source.0)].effects[1]
        .instructions
        .remove(0);
    start(&mut reduced, source, "B069");
    start(&mut expanded, source, "B069");
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut reduced, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    let menu = decision(&mut expanded, &mut Vec::new());
    assert_eq!(menu.options, [Alternative::Value(8), Alternative::Value(9)]);
    for choice in 0..2 {
        let mut branch = expanded.clone();
        branch.choose(choice).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut branch, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(events.is_empty());
        branch.position.catalog = original_catalog.clone();
        branch.shortcuts_disabled = false;
        assert_eq!(branch, reduced);
    }
}
