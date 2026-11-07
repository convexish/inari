use super::common::*;
use super::*;

fn native_board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

fn purple_stack(f: &mut Fixture, player: Player) {
    for code in ["B014", "B015"] {
        native_board(f, code, player);
    }
}

fn fixture(ages: &[u8]) -> (Fixture, CardId, Vec<CardId>) {
    let mut f = Fixture::default();
    purple_stack(&mut f, A);
    let source = native_board(&mut f, "B091", A);
    let hand = ages
        .iter()
        .enumerate()
        .map(|(index, age)| {
            let cards: Vec<_> = BASE_METADATA
                .iter()
                .filter(|m| m.value == *age)
                .filter_map(|m| base_card(m.code).ok())
                .collect();
            f.add(cards[index % cards.len()].clone(), Zone::Hand(A))
        })
        .collect();
    (f, source, hand)
}

fn start(game: &mut Game, card: CardId, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn splay_options() -> Vec<Alternative> {
    vec![
        Alternative::Splay {
            player: A,
            color: Color::Purple,
            direction: Splay::Up,
        },
        Alternative::Decline,
    ]
}

fn canonical(game: &Game, menu: &Decision, splay: bool) -> usize {
    if menu.options.contains(&Alternative::Decline) {
        assert_eq!(menu.options, splay_options());
        assert!(game.position.cards(Zone::Hand(A)).is_empty());
        return usize::from(!splay);
    }
    menu.options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let Alternative::Move {
                verb: Verb::Return,
                target: TargetDescriptor::Known { card, .. },
            } = option
            else {
                panic!("unexpected return choice: {option:?}");
            };
            let deck = game.position.card(*card).origin_deck();
            ((deck.age, deck.set as u8, *card), index)
        })
        .min_by_key(|(key, _)| *key)
        .unwrap()
        .1
}

fn finish(game: &mut Game, events: &mut Vec<Event>, splay: bool) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu, splay)).unwrap(),
            status => return status,
        }
    }
    panic!("Satellites effect did not finish");
}

fn oracle<const F: bool>(position: Position, card: CardId, splay: bool) -> (Game, Vec<Event>) {
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, card, Context::new(A));
    start(&mut manual, card, Context::new(A));
    let mut events = Vec::new();
    let mut expected = Vec::new();
    assert_eq!(
        finish(&mut reduced, &mut events, splay),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        finish(&mut manual, &mut expected, splay),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events, expected);
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual); // Physical order, suspended work, and RNG.
    assert!(reduced.position.check_conservation());
    (reduced, events)
}

fn outcomes(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => {
            if menu.options.contains(&Alternative::Decline) {
                assert_eq!(menu.options, splay_options());
                assert!(game.position.cards(Zone::Hand(A)).is_empty());
            } else {
                assert!(menu.options.iter().all(|o| matches!(
                    o,
                    Alternative::Move {
                        verb: Verb::Return,
                        ..
                    }
                )));
            }
            (0..menu.options.len())
                .flat_map(|choice| {
                    let mut fork = game.clone();
                    fork.choose(choice).unwrap();
                    outcomes(fork, depth + 1)
                })
                .collect()
        }
        status => {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert!(game.position.check_conservation());
            game.shortcuts_disabled = false;
            vec![game]
        }
    }
}

fn compare_paths<const F: bool>(
    position: Position,
    card: CardId,
    manual_paths: usize,
    reduced_paths: usize,
) {
    let mut reduced = Game::seeded(position.clone(), Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, card, Context::new(A));
    start(&mut manual, card, Context::new(A));
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert_eq!(manual.len(), manual_paths);
    assert_eq!(reduced.len(), reduced_paths);
    assert!(manual.iter().all(|outcome| reduced.contains(outcome)));
    assert!(reduced.iter().all(|outcome| manual.contains(outcome)));
    for splay in [false, true] {
        let (game, events) = oracle::<F>(position.clone(), card, splay);
        assert_eq!(
            game.position.splay(A, Color::Purple),
            if splay { Splay::Up } else { Splay::None }
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Splayed { .. }))
                .count(),
            usize::from(splay)
        );
    }
}

#[test]
fn only_cross_origin_interleavings_disappear_and_both_later_splay_outcomes_survive() {
    for (ages, manual, reduced) in [
        (vec![], 2, 2),
        (vec![1], 2, 2),
        (vec![1, 2, 4], 12, 2),
        (vec![1, 1, 2], 12, 4),
        (vec![1, 1, 1], 12, 12),
        (vec![1, 2, 1, 2], 48, 8),
    ] {
        let (f, card, _) = fixture(&ages);
        compare_paths::<false>(f.position(), card, manual, reduced);
    }
    let (mut f, card, _) = fixture(&[1, 1, 2]);
    f.figure(Figure::Gilgamesh, Zone::Hand(A));
    compare_paths::<true>(f.position(), card, 48, 4);
}

#[test]
fn whole_hand_has_no_quota_and_canonical_events_are_exact() {
    let (mut f, card, hand) = fixture(&[1, 1, 2, 3, 4]);
    let figure = f.figure(Figure::Gilgamesh, Zone::Hand(A));
    let (game, events) = oracle::<true>(f.position(), card, true);
    let order = [hand[0], hand[1], figure, hand[2], hand[3], hand[4]];
    let mut expected: Vec<_> = order
        .into_iter()
        .map(|card| {
            Event::Moved(Movement {
                verb: Some(Verb::Return),
                card,
                source: Zone::Hand(A),
                destination: Zone::Deck(game.position.card(card).origin_deck()),
                executor: A,
                inside_karma: false,
            })
        })
        .collect();
    expected.push(Event::Splayed {
        player: A,
        color: Color::Purple,
        from: Splay::None,
        to: Splay::Up,
        executor: A,
        inside_karma: false,
    });
    assert_eq!(events, expected);
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(
        game.position.cards(Zone::Deck(Deck::base(1))),
        &[hand[1], hand[0]]
    );
    assert_eq!(game.position.cards(Zone::Deck(Deck::figures(1))), &[figure]);
}

#[test]
fn pending_clones_resume_identically_across_returns_and_the_optional_splay() {
    for splay in [false, true] {
        let (f, card, _) = fixture(&[1, 1, 2, 4]);
        let mut game = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
        start(&mut game, card, Context::new(A));
        let mut expected = game.clone();
        let mut all = Vec::new();
        let status = finish(&mut expected, &mut all, splay);
        let mut prefix = Vec::new();
        let mut completed = false;
        for _ in 0..300 {
            let mut fork = game.clone();
            let mut suffix = Vec::new();
            assert_eq!(finish(&mut fork, &mut suffix, splay), status);
            assert_eq!(fork, expected);
            assert_eq!(
                prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
                all
            );
            assert!(game.position.check_conservation());
            match game.advance(1, &mut prefix).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(menu) => game.choose(canonical(&game, &menu, splay)).unwrap(),
                result => {
                    assert_eq!(result, status);
                    assert_eq!(game, expected);
                    completed = true;
                    break;
                }
            }
        }
        assert!(completed);
    }
}

fn assert_returns(menu: &Decision, count: usize) {
    assert_eq!(menu.options.len(), count);
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::Move {
            verb: Verb::Return,
            ..
        }
    )));
}

#[test]
fn foreign_controller_cannot_distinguish_hidden_front_permutations() {
    let make = |reverse| {
        let (mut f, card, hand) = fixture(&[1, 1, 2]);
        if reverse {
            f.placements
                .swap(usize::from(hand[0].0), usize::from(hand[1].0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        start(&mut game, card, context);
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let menu = decision(&mut first, &mut Vec::new());
    assert_eq!(menu, decision(&mut second, &mut Vec::new()));
    assert_eq!(menu.player, B);
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Return,
            source: Zone::Hand(A),
            ..
        }
    )));
}

#[test]
fn relevant_return_and_hand_membership_hooks_fall_back_but_independent_hooks_do_not() {
    for (figure, blocked) in [
        (Figure::Homer, true),
        (Figure::JamesClerkMaxwell, true),
        (Figure::Nightingale, true),
        (Figure::AlexTrebek, false),
        (Figure::GalileoGalilei, false),
        (Figure::JohnVonNeumann, false),
        (Figure::NiccoloMachiavelli, false),
        (Figure::CatherineTheGreat, false),
        (Figure::SergeyBrin, false),
        (Figure::NelsonMandela, false),
    ] {
        let (mut f, card, hand) = fixture(&[1, 2, 4]);
        let native = figure_card(figure.code()).unwrap();
        let color = native.color;
        f.add(native, Zone::Board(A, color));
        let position = f.position();
        let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
        start(&mut game, card, Context::new(A));
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        if blocked {
            assert_returns(&menu, 3);
            assert!(events.is_empty());
        } else {
            assert_eq!(menu.options, splay_options(), "{figure:?}");
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                hand
            );
            assert!(game.position.cards(Zone::Hand(A)).is_empty());
            game.choose(0).unwrap();
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(game.position.splay(A, Color::Purple), Splay::Up);
            assert!(game.position.check_conservation());
            oracle::<true>(position, card, true);
        }
    }
}

#[test]
fn machiavelli_runs_after_the_reduced_returns_and_before_the_real_splay() {
    let (mut f, card, hand) = fixture(&[1, 2, 4]);
    let machiavelli = figure_card(Figure::NiccoloMachiavelli.code()).unwrap();
    let color = machiavelli.color;
    f.add(machiavelli, Zone::Board(A, color));
    purple_stack(&mut f, B);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: B,
            color: Color::Purple,
            direction: Splay::Right,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: None,
            value: None,
            short: false,
        }))
    );
    start(&mut game, card, Context::new(A));
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options, splay_options());
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        hand
    );
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        &events[hand.len()..],
        &[
            Event::Splayed {
                player: B,
                color: Color::Purple,
                from: Splay::Right,
                to: Splay::None,
                executor: A,
                inside_karma: true
            },
            Event::Splayed {
                player: A,
                color: Color::Purple,
                from: Splay::None,
                to: Splay::Up,
                executor: A,
                inside_karma: false
            },
        ]
    );
    let mut expected = Vec::new();
    finish(&mut manual, &mut expected, true);
    assert_eq!(events, expected);
    manual.shortcuts_disabled = false;
    assert_eq!(game, manual);
    assert!(game.position.check_conservation());
}

#[test]
fn meiji_timing_and_newly_exposed_hazards_are_rechecked_at_resumption() {
    for regular_actions in [0, 2] {
        let (mut f, card, hand) = fixture(&[2, 4]);
        let native = figure_card(Figure::EmperorMeiji.code()).unwrap();
        let color = native.color;
        f.add(native, Zone::Board(A, color));
        let borrowed = f.add(base_card("B001").unwrap(), Zone::Hand(B));
        let mut position = f.position();
        position.regular_actions_started = regular_actions;
        let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
        start(&mut game, card, Context::new(A));
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        if regular_actions == 0 {
            assert_returns(&menu, 3);
            assert!(events.is_empty());
            assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == borrowed)));
        } else {
            assert_eq!(menu.options, splay_options());
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                hand
            );
        }
        let (finished, _) = oracle::<true>(position, card, true);
        assert_eq!(
            finished.position.cards(Zone::Hand(B)),
            if regular_actions == 0 {
                &[][..]
            } else {
                std::slice::from_ref(&borrowed)
            }
        );
    }
    for figure in [Figure::Homer, Figure::EmperorMeiji] {
        let (mut f, card, hand) = fixture(&[1, 1, 2, 4]);
        let native = figure_card(figure.code()).unwrap();
        let color = native.color;
        f.add(native, Zone::Board(A, color));
        // Synthetic cover is an explicit guard-boundary witness, not a rule program.
        let cover = f.tech(color, 8, Zone::Board(A, color), vec![]);
        let borrowed = f.add(base_card("B001").unwrap(), Zone::Hand(B));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, card, Context::new(A));
        let mut events = Vec::new();
        assert_returns(&decision(&mut game, &mut events), 2);
        choose_card(&mut game, &mut events, hand[0]);
        assert_eq!(
            game.position.zone_mut(Zone::Board(A, color)).pop(),
            Some(cover)
        );
        game.position.zone_mut(Zone::Junk).push(cover);
        game.position.refresh_boards();
        let menu = decision(&mut game, &mut events);
        assert_returns(&menu, if figure == Figure::EmperorMeiji { 4 } else { 3 });
        if figure == Figure::EmperorMeiji {
            assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == borrowed)));
        }
        assert!(game.position.check_conservation());
    }
}

#[test]
fn live_regular_action_timing_can_enable_or_disable_membership_between_returns() {
    for initially_live in [false, true] {
        let (mut f, card, hand) = fixture(&[1, 1, 2, 4]);
        let native = figure_card(Figure::EmperorMeiji.code()).unwrap();
        let color = native.color;
        f.add(native, Zone::Board(A, color));
        let borrowed = f.add(base_card("B012").unwrap(), Zone::Hand(B));
        let mut position = f.position();
        position.regular_actions_started = if initially_live { 0 } else { 2 };
        let mut game = Game::new(position, Expansions::FIGURES).unwrap();
        start(&mut game, card, Context::new(A));
        let mut events = Vec::new();
        assert_returns(
            &decision(&mut game, &mut events),
            if initially_live { 5 } else { 2 },
        );
        choose_card(&mut game, &mut events, hand[0]);
        game.position.regular_actions_started = if initially_live { 2 } else { 0 };
        let menu = decision(&mut game, &mut events);
        if initially_live {
            assert_eq!(menu.options, splay_options());
            assert_eq!(game.position.cards(Zone::Hand(B)), &[borrowed]);
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                hand
            );
        } else {
            assert_returns(&menu, 4);
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                [hand[0]]
            );
            assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == borrowed)));
        }
        assert!(game.position.check_conservation());
    }
}

#[test]
fn homer_replaces_a_return_without_skipping_remaining_returns_or_later_splay() {
    let (mut f, card, hand) = fixture(&[2, 4]);
    let homer = figure_card(Figure::Homer.code()).unwrap();
    let color = homer.color;
    f.add(homer, Zone::Board(A, color));
    let returned = f.figure(Figure::Archimedes, Zone::Hand(A));
    let (game, events) = oracle::<true>(f.position(), card, true);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Return)))
            .count(),
        hand.len()
    );
    assert!(events.iter().any(|e| matches!(e, Event::Moved(m) if m.card == returned && m.verb == Some(Verb::Tuck) && m.inside_karma)));
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(
        events.last(),
        Some(&Event::Splayed {
            player: A,
            color: Color::Purple,
            from: Splay::None,
            to: Splay::Up,
            executor: A,
            inside_karma: false,
        })
    );
}
