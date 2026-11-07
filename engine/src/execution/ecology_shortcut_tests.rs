use super::common::*;
use super::*;

fn start(game: &mut Game, ecology: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card: ecology,
            effect: 1,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn fixture() -> (Fixture, CardId, Vec<CardId>) {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let cards: Vec<_> = [Color::Red, Color::Blue, Color::Green]
        .into_iter()
        .map(|color| f.tech(color, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    // Physical order deliberately differs from both ascending and descending IDs.
    f.placements
        .swap(usize::from(cards[0].0), usize::from(cards[1].0));
    f.tech(Color::Purple, 9, Zone::Deck(Deck::base(9)), vec![]);
    f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
    f.tech(Color::Red, 4, Zone::Board(B, Color::Red), vec![]);
    f.tech(Color::Red, 1, Zone::Junk, vec![]);
    (f, ecology, cards)
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    let deck = game.position.cards(Zone::Deck(Deck::base(10)));
    let top = *deck.last().unwrap();
    menu.options
        .iter()
        .position(|option| match option {
            Alternative::Move {
                verb: Verb::Junk,
                target: TargetDescriptor::Hidden { source, slot, .. },
            } => *source == Zone::Deck(Deck::base(10)) && deck[*slot] == top,
            Alternative::Move {
                verb: Verb::Junk,
                target: TargetDescriptor::Known { card, .. },
            } => *card == top,
            _ => false,
        })
        .expect("physical deck top is a legal junk choice")
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut decisions = 0;
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                decisions += 1;
                game.choose(canonical(game, &menu)).unwrap();
            }
            status => return (status, decisions),
        }
    }
    panic!("Ecology did not finish");
}

fn oracle<const F: bool>(position: Position, ecology: CardId) -> (Game, Vec<Event>) {
    let expected_order: Vec<_> = position
        .cards(Zone::Deck(Deck::base(10)))
        .iter()
        .rev()
        .copied()
        .collect();
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, ecology);
    start(&mut manual, ecology);
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    let (status, choices) = finish(&mut reduced, &mut events);
    let (manual_status, manual_choices) = finish(&mut manual, &mut manual_events);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(status, manual_status);
    assert_eq!(choices, usize::from(!expected_order.is_empty()));
    assert_eq!(
        manual_choices,
        expected_order
            .len()
            .saturating_sub(1)
            .max(usize::from(!expected_order.is_empty()))
    );
    assert_eq!(events, manual_events);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        expected_order
    );
    assert!(events.iter().all(|e| {
        let movement = moved(e);
        movement.verb == Some(Verb::Junk)
            && movement.source == Zone::Deck(Deck::base(10))
            && !movement.inside_karma
    }));
    manual.shortcuts_disabled = false;
    // Exact native game equality includes RNG, continuations, and every pile's
    // physical order. Only all-path outcome classification below ignores junk order.
    assert_eq!(reduced, manual);
    assert!(reduced.position.check_conservation());
    (reduced, events)
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Status)> {
    assert!(depth < 8);
    let mut events = Vec::new();
    match settle(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            // Supported-core limit: junk is an uninspectable unordered pile.
            // No equivalence is claimed if future rules expose its order. Every
            // deck, hand, board, score and all other state remain EXACTLY ordered.
            game.position.junk.sort_unstable();
            vec![(game.position, status)]
        }
    }
}

#[test]
fn canonical_junk_is_physical_top_first_with_exact_events_rng_and_game_parity() {
    let (f, ecology, cards) = fixture();
    let (game, events) = oracle::<false>(f.position(), ecology);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [cards[2], cards[0], cards[1]]
    );
    assert!(game.position.cards(Zone::Deck(Deck::base(10))).is_empty());
}

#[test]
fn three_card_oracle_has_six_committed_paths_and_decline_reduction_has_one_and_decline() {
    let (f, ecology, _) = fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, ecology);
    start(&mut manual, ecology);
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert_eq!(manual.len(), 7);
    assert_eq!(reduced.len(), 2);
    for paths in [&manual, &reduced] {
        assert_eq!(
            paths
                .iter()
                .filter(|(p, _)| !p.cards(Zone::Deck(Deck::base(10))).is_empty())
                .count(),
            1
        );
        assert!(
            paths
                .iter()
                .all(|(_, status)| *status == Status::Complete(ReturnValue::Unit))
        );
    }
    assert!(manual.iter().all(|result| reduced.contains(result)));
    assert!(reduced.iter().all(|result| manual.contains(result)));
}

#[test]
fn initial_decline_is_real_and_commitment_has_no_later_stop() {
    let (f, ecology, _) = fixture();
    let initial = f.position();
    for disabled in [false, true] {
        let mut game = Game::new(initial.clone(), Expansions::BASE).unwrap();
        game.shortcuts_disabled = disabled;
        start(&mut game, ecology);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), if disabled { 4 } else { 2 });
        assert!(events.is_empty());
        let mut declined = game.clone();
        declined
            .choose(
                menu.options
                    .iter()
                    .position(|o| *o == Alternative::Decline)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            settle(&mut declined, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(declined.position, initial);
        assert!(events.is_empty());
        game.choose(canonical(&game, &menu)).unwrap();
        if disabled {
            let next = decision(&mut game, &mut events);
            assert_eq!(next.options.len(), 2);
            assert!(!next.options.contains(&Alternative::Decline));
        }
        assert_eq!(
            finish(&mut game, &mut events).0,
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), 3);
    }
}

#[test]
fn hidden_front_permutations_do_not_change_reduced_or_unrestricted_back_menus() {
    for disabled in [false, true] {
        let (f, ecology, _) = fixture();
        let first_position = f.position();
        let mut second_position = first_position.clone();
        second_position
            .zone_mut(Zone::Deck(Deck::base(10)))
            .reverse();
        let mut first = Game::new(first_position, Expansions::BASE).unwrap();
        let mut second = Game::new(second_position, Expansions::BASE).unwrap();
        first.shortcuts_disabled = disabled;
        second.shortcuts_disabled = disabled;
        start(&mut first, ecology);
        start(&mut second, ecology);
        let mut events = Vec::new();
        let menu = decision(&mut first, &mut events);
        assert_eq!(menu, decision(&mut second, &mut events));
        assert_eq!(menu.options.len(), if disabled { 4 } else { 2 });
        assert!(menu.options.iter().all(|option| matches!(
            option,
            Alternative::Decline
                | Alternative::Move {
                    verb: Verb::Junk,
                    target: TargetDescriptor::Hidden {
                        source: Zone::Deck(_),
                        back: Deck {
                            set: CardSet::Base,
                            age: 10
                        },
                        ..
                    },
                }
        )));
        assert!(events.is_empty());
    }
}

#[test]
fn active_unrelated_and_hand_score_junk_figures_do_not_disable_physical_deck_reduction() {
    for (figure, color) in [
        (Figure::AlexTrebek, Color::Yellow),
        (Figure::Homer, Color::Purple),
        (Figure::Nightingale, Color::Yellow),
        (Figure::HedyLamarr, Color::Green),
    ] {
        let (mut f, ecology, _) = fixture();
        f.figure(figure, Zone::Board(A, color));
        oracle::<true>(f.position(), ecology);
    }
}

#[test]
fn native_clones_resume_with_identical_progress_at_every_single_work_boundary() {
    let (f, ecology, _) = fixture();
    let mut game = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    start(&mut game, ecology);
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = finish(&mut expected, &mut all).0;
    let mut prefix = Vec::new();
    for _ in 0..200 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix).0, result);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => game.choose(canonical(&game, &menu)).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position.check_conservation());
    }
    panic!("Ecology did not complete");
}

#[test]
fn activating_homer_mid_batch_is_only_a_guard_witness_not_a_legal_gameplay_sequence() {
    let (mut f, ecology, _) = fixture();
    let homer = f.figure(Figure::Homer, Zone::Hand(B));
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    start(&mut game, ecology);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    game.choose(canonical(&game, &menu)).unwrap();
    for _ in 0..100 {
        let status = game.advance(1, &mut events).unwrap();
        if !events.is_empty() {
            assert_eq!(events.len(), 1);
            // Deliberate internal mutation to witness the next boundary's guard;
            // this is NOT presented as an action permitted during a real effect.
            game.position
                .zone_mut(Zone::Hand(B))
                .retain(|&c| c != homer);
            game.position
                .zone_mut(Zone::Board(B, Color::Purple))
                .push(homer);
            game.position.refresh_boards();
            assert!(game.position.check_conservation());
            let mut manual = game.clone();
            manual.shortcuts_disabled = true;
            let mut rest = Vec::new();
            let mut manual_rest = Vec::new();
            let (result, decisions) = finish(&mut game, &mut rest);
            let (manual_result, manual_decisions) = finish(&mut manual, &mut manual_rest);
            assert_eq!(decisions, 0);
            assert_eq!(manual_decisions, 1);
            assert_eq!(result, manual_result);
            assert_eq!(rest, manual_rest);
            manual.shortcuts_disabled = false;
            assert_eq!(game, manual);
            assert!(game.position.check_conservation());
            return;
        }
        assert_eq!(status, Status::WorkLimit);
    }
    panic!("first Ecology junk did not occur");
}

#[test]
fn every_canonical_movement_boundary_preserves_position_and_rng() {
    fn next_movement(game: &mut Game) -> Event {
        let mut events = Vec::new();
        for _ in 0..100 {
            let status = game.advance(1, &mut events).unwrap();
            if !events.is_empty() {
                assert_eq!(events.len(), 1);
                assert!(game.position.check_conservation());
                return events.pop().unwrap();
            }
            match status {
                Status::WorkLimit => {}
                Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
                status => panic!("completed before the next junk: {status:?}"),
            }
        }
        panic!("next Ecology junk did not occur");
    }

    let (f, ecology, _) = fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, ecology);
    start(&mut manual, ecology);
    for remaining in (0..3).rev() {
        assert_eq!(next_movement(&mut reduced), next_movement(&mut manual));
        assert_eq!(reduced.position, manual.position);
        assert_eq!(reduced.rng, manual.rng);
        assert_eq!(
            reduced.position.cards(Zone::Deck(Deck::base(10))).len(),
            remaining
        );
    }
    let mut reduced_events = Vec::new();
    let mut manual_events = Vec::new();
    assert_eq!(
        finish(&mut reduced, &mut reduced_events).0,
        finish(&mut manual, &mut manual_events).0
    );
    assert!(reduced_events.is_empty());
    assert!(manual_events.is_empty());
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual);
}
