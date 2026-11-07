use super::common::*;
use super::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn fixture(code: &str, extra: bool) -> (Position, CardId) {
    let mut f = Fixture::default();
    let source = base(&mut f, code, Zone::Junk);
    for payment in ["B001", "B004", "B026"] {
        base(&mut f, payment, Zone::Hand(A));
    }
    if extra {
        base(&mut f, "B008", Zone::Hand(A));
    }
    let supply: &[&str] = match code {
        "B020" => &["B016", "B018"],
        "B074" => &["B070", "B071"],
        "B078" => &["B077", "B079", "B080", "B081"],
        "B064" => &["B077"],
        "B032" => {
            // Two colors with Authority; source is not on board.
            base(&mut f, "B002", Zone::Board(A, Color::Red));
            &["B037"]
        }
        "B053" => &["B056", "B057", "B062"],
        _ => unreachable!(),
    };
    for draw in supply {
        let card = base_card(draw).unwrap();
        let deck = card.origin_deck();
        f.add(card, Zone::Deck(deck));
    }
    (f.position(), source)
}

fn invoke(game: &mut Game, card: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn outcomes(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 10);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        Status::Complete(_) | Status::Draw(_) | Status::Winner(_) => {
            assert!(game.position.check_conservation());
            game.shortcuts_disabled = false;
            vec![game]
        }
        Status::WorkLimit => panic!("unexpected oracle work limit"),
    }
}

fn all_paths<const F: bool>() {
    for code in ["B020", "B074", "B078", "B064", "B032", "B053"] {
        for extra in [false, true] {
            // Keep mandatory all-hand enumeration bounded, but include larger
            // than quota hands for both finite optional phases.
            if extra && !["B074", "B078"].contains(&code) {
                continue;
            }
            let (position, source) = fixture(code, extra);
            let mut reduced = Game::seeded(position, Expansions::with_figures(F), 183).unwrap();
            let mut manual = reduced.clone();
            manual.shortcuts_disabled = true;
            invoke(&mut reduced, source);
            invoke(&mut manual, source);
            let reduced = outcomes(reduced, 0);
            let manual = outcomes(manual, 0);
            assert!(reduced.len() < manual.len(), "{code}");
            assert!(
                manual.iter().all(|result| reduced.contains(result)),
                "missing {code} outcome"
            );
            assert!(
                reduced.iter().all(|result| manual.contains(result)),
                "invented {code} outcome"
            );
        }
    }
}

#[test]
fn all_six_phases_preserve_unrestricted_outcome_classes_and_every_ordered_zone() {
    all_paths::<false>();
    all_paths::<true>();
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    if let Some(index) = menu
        .options
        .iter()
        .position(|option| *option == Alternative::Deck(Deck::figures(2)))
    {
        return index;
    }
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                verb,
                target: TargetDescriptor::Known { card, .. },
            } => {
                let card_data = game.position.card(*card);
                let key = if *verb == Verb::Tuck {
                    (card_data.color as u8, 0, card.0)
                } else {
                    (card_data.value, card_data.origin_deck().set as u8, card.0)
                };
                Some((key, index))
            }
            _ => None,
        })
        .min_by_key(|&(key, _)| key)
        .map(|(_, index)| index)
        .unwrap_or_else(|| {
            menu.options
                .iter()
                .position(|option| matches!(option, Alternative::Pass | Alternative::Decline))
                .unwrap()
        })
}

fn run(game: &mut Game) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..2000 {
        // Every scheduler boundary is cloned, not just the visible choices.
        let mut fork = game.clone();
        let mut step_events = Vec::new();
        let status = game.advance(1, &mut step_events).unwrap();
        let mut fork_events = Vec::new();
        assert_eq!(fork.advance(1, &mut fork_events).unwrap(), status);
        assert_eq!(step_events, fork_events);
        assert_eq!(*game, fork);
        events.extend(step_events);
        match status {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            Status::WorkLimit => {}
            _ => return events,
        }
    }
    panic!("canonical aggregate did not settle");
}

fn canonical_oracles<const F: bool>() {
    for code in ["B020", "B074", "B078", "B064", "B032", "B053"] {
        let (position, source) = fixture(code, false);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 911).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        invoke(&mut reduced, source);
        invoke(&mut manual, source);
        assert_eq!(run(&mut reduced), run(&mut manual), "{code} events");
        manual.shortcuts_disabled = false;
        assert_eq!(reduced, manual, "{code} machine/RNG");
    }
}

#[test]
fn canonical_events_rng_terminal_state_and_every_boundary_clones_match() {
    canonical_oracles::<false>();
    canonical_oracles::<true>();
}

#[test]
fn return_masks_ignore_replacements_and_include_age_zero_without_frame_growth() {
    let mut outcome = BulkOutcome::default();
    for (disposition, value) in [
        (Disposition::Performed, 0),
        (Disposition::Performed, 11),
        (Disposition::Performed, 11),
        (Disposition::Replaced, 7),
        (Disposition::Inapplicable, 8),
    ] {
        outcome.record_original(OperationOutcome {
            disposition,
            card: Some(CardId(0)),
            value: Some(value),
            short: false,
        });
    }
    assert_eq!(outcome.distinct_values, 1 | (1 << 11));
    assert_eq!(ReturnValue::Bulk(outcome).distinct_value_count(), 2);
    assert_eq!(outcome.performed, 3);
    assert!(!outcome.fulfilled());
    assert!(std::mem::size_of::<Frame>() <= 56);
}

#[test]
fn foreign_controller_keeps_hidden_back_choices_and_does_not_inspect_grouping_fronts() {
    for code in ["B020", "B074", "B078", "B064"] {
        let (position, source) = fixture(code, false);
        let mut reduced = Game::new(position, Expansions::FIGURES).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        let mut context = Context::new(A);
        context.controller = B;
        for game in [&mut reduced, &mut manual] {
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::default(),
                },
                context,
            )
            .unwrap();
        }
        let reduced_menu = decision(&mut reduced, &mut Vec::new());
        let manual_menu = decision(&mut manual, &mut Vec::new());
        assert_eq!(reduced_menu, manual_menu);
        assert!(
            reduced_menu
                .options
                .iter()
                .filter_map(|option| match option {
                    Alternative::Move { target, .. } => Some(target),
                    _ => None,
                })
                .all(|target| matches!(target, TargetDescriptor::Hidden { .. }))
        );
    }
}

#[test]
fn active_return_hazards_and_future_tuck_figures_fail_closed() {
    for code in ["B020", "B078", "B064"] {
        for figure in [
            Figure::Homer,
            Figure::JamesClerkMaxwell,
            Figure::Nightingale,
        ] {
            let (position, source) = fixture(code, false);
            let mut cards = position.catalog.to_vec();
            let native = figure_card(figure.code()).unwrap();
            let color = native.color;
            let id = CardId(cards.len() as u16);
            cards.push(native);
            let mut placements: Vec<_> = (0..position.catalog.len())
                .map(|index| {
                    let card = CardId(index as u16);
                    (card, position.locate(card).unwrap())
                })
                .collect();
            placements.push((id, Zone::Board(A, color)));
            let mut reduced = Game::new(
                Position::new(cards, &placements, A).unwrap(),
                Expansions::FIGURES,
            )
            .unwrap();
            let mut manual = reduced.clone();
            manual.shortcuts_disabled = true;
            invoke(&mut reduced, source);
            invoke(&mut manual, source);
            let a = decision(&mut reduced, &mut Vec::new());
            let b = decision(&mut manual, &mut Vec::new());
            assert_eq!(a, b);
        }
    }
    let mut f = Fixture::default();
    let source = base(&mut f, "B074", Zone::Junk);
    base(&mut f, "B001", Zone::Hand(A));
    base(&mut f, "B004", Zone::Hand(A));
    f.figure(Figure::EmperorMeiji, Zone::Hand(A));
    let mut reduced = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    invoke(&mut reduced, source);
    invoke(&mut manual, source);
    assert_eq!(
        decision(&mut reduced, &mut Vec::new()),
        decision(&mut manual, &mut Vec::new())
    );
}

#[test]
fn democracy_copied_invocations_share_action_tally_but_not_the_next_action() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B064", Zone::Junk);
    let a_return = base(&mut f, "B001", Zone::Hand(A));
    let b_return = base(&mut f, "B004", Zone::Hand(B));
    let first = base(&mut f, "B077", Zone::Deck(Deck::base(8)));
    let second = base(&mut f, "B079", Zone::Deck(Deck::base(8)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    // An isolated scheduler witness for two copied printed invocations inside
    // one outer action: no synthetic card programs or alternate semantics.
    game.push_action_returns();
    game.push_copied(source, ExecutionMode::SelfExecute, Context::new(A));
    game.push_copied(source, ExecutionMode::SelfExecute, Context::new(B));
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, b_return);
    choose_card(&mut game, &mut events, a_return);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Score(B)), [second]);
    assert!(game.position.cards(Zone::Score(A)).is_empty());
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(8))), [first]);
    assert!(game.frames.is_empty());
    game.self_execute(source, Context::new(B)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Score(B)), [second]);
}

#[test]
fn democracy_terminal_unwind_discards_action_tallies_and_clone_resumes_exactly() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B064", Zone::Junk);
    let payment = base(&mut f, "B001", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source);
    choose_card(&mut game, &mut Vec::new(), payment);
    let events = run(&mut game);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Return)))
    );
    assert!(game.frames.is_empty());
    assert_eq!(game.terminal, Some(Terminal::Draw(DrawReason::Exhaustion)));
}

#[test]
fn democracy_later_copy_rewards_existing_action_lead_even_with_no_new_return() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B064", Zone::Junk);
    let a_returns = [
        base(&mut f, "B001", Zone::Hand(A)),
        base(&mut f, "B002", Zone::Hand(A)),
    ];
    let b_return = base(&mut f, "B004", Zone::Hand(B));
    let rewards =
        ["B077", "B079", "B080"].map(|code| base(&mut f, code, Zone::Deck(Deck::base(8))));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.push_action_returns();
    game.push_copied(source, ExecutionMode::SelfExecute, Context::new(A));
    game.push_copied(source, ExecutionMode::SelfExecute, Context::new(A));
    game.push_copied(source, ExecutionMode::SelfExecute, Context::new(B));
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, b_return);
    for card in a_returns {
        choose_card(&mut game, &mut events, card);
    }
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Score(B)), [rewards[2]]);
    assert_eq!(
        game.position.cards(Zone::Score(A)),
        [rewards[1], rewards[0]]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event,
        Event::Moved(m) if m.executor == A && m.verb == Some(Verb::Return)))
            .count(),
        2
    );
}

#[test]
fn lighting_starts_a_new_ordered_suffix_after_an_unrestricted_figure_tuck() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B074", Zone::Junk);
    // Tucked into an empty pile, the Figure becomes a live top.
    let figure = f.figure(Figure::Archimedes, Zone::Hand(A));
    let red = base(&mut f, "B001", Zone::Hand(A));
    let yellow = base(&mut f, "B004", Zone::Hand(A));
    for code in ["B070", "B071"] {
        base(&mut f, code, Zone::Deck(Deck::base(7)));
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, figure);
    let menu = decision(&mut game, &mut events);
    // Blue was chosen while the phase was unrestricted. It cannot install a
    // color floor that discards still-legal red or yellow subset extensions.
    for expected in [red, yellow] {
        assert!(menu.options.iter().any(|option| matches!(option,
            Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. }
                if *card == expected)));
    }
    choose_card(&mut game, &mut events, red);
    choose_card(&mut game, &mut events, yellow);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Score(A)).len(), 2);
}

#[test]
fn reward_draw_reactions_keep_native_shuffle_rng_and_canonical_event_oracle() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B020", Zone::Junk);
    for code in ["B001", "B004", "B026"] {
        base(&mut f, code, Zone::Hand(A));
    }
    for code in ["B016", "B018"] {
        base(&mut f, code, Zone::Deck(Deck::base(2)));
    }
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::Archimedes, Zone::Deck(Deck::figures(2)));
    f.figure(Figure::CaiLun, Zone::Deck(Deck::figures(2)));
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 831).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    invoke(&mut reduced, source);
    invoke(&mut manual, source);
    let events = run(&mut reduced);
    assert_eq!(events, run(&mut manual));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Shuffled { .. }))
            .count(),
        2
    );
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual);
}
