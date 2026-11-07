use super::*;

use super::common::*;

fn start(game: &mut Game, tools: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card: tools,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn fixture(ages: &[u8]) -> (Fixture, CardId, Vec<CardId>, CardId) {
    let mut f = Fixture::default();
    let tools = f.tools_first(Zone::Board(A, Color::Blue));
    let hand = ages
        .iter()
        .map(|&age| f.tech(Color::Red, age, Zone::Hand(A), vec![]))
        .collect();
    let reward = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    (f, tools, hand, reward)
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            if let Alternative::Move {
                verb: Verb::Return,
                target: TargetDescriptor::Known { card, .. },
            } = option
            {
                let deck = game.position.card(*card).origin_deck();
                Some(((deck.age, deck.set as u8, *card), index))
            } else {
                None
            }
        })
        .min_by_key(|(key, _)| *key)
        .map_or(0, |(_, index)| index)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut count = 0;
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                count += 1;
                game.choose(canonical(game, &menu)).unwrap();
            }
            status => return (status, count),
        }
    }
    panic!("Tools did not finish");
}

fn oracle<const F: bool>(position: Position, tools: CardId) -> (Game, Vec<Event>, usize) {
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, tools);
    start(&mut manual, tools);
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    let (status, decisions) = finish(&mut reduced, &mut events);
    let (manual_status, manual_decisions) = finish(&mut manual, &mut manual_events);
    assert_eq!(status, manual_status);
    assert_eq!(events, manual_events);
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual); // Includes RNG and exact deck/stack order.
    assert!(decisions <= manual_decisions);
    assert!(reduced.position.check_conservation());
    (reduced, events, decisions)
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Status)> {
    assert!(depth < 12);
    let mut events = Vec::new();
    match settle(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => vec![(game.position.clone(), status)],
    }
}

#[test]
fn tools_preserves_decline_but_needs_no_further_decision_for_three_independent_decks() {
    let (f, tools, hand, reward) = fixture(&[4, 1, 2]);
    let (game, events, decisions) = oracle::<false>(f.position(), tools);
    assert_eq!(decisions, 1); // Commit versus decline, not three return decisions.
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [hand[1], hand[2], hand[0], reward, reward]
    );
    assert_eq!(game.position.top(A, Color::Green), Some(reward));
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
}

#[test]
fn all_manual_outcome_classes_survive_and_same_deck_orders_are_not_collapsed() {
    for (ages, manual_paths, reduced_paths) in [
        (vec![1, 2, 4], 7, 2),
        (vec![1, 1, 2], 7, 3),
        (vec![1, 1, 1], 7, 7),
        (vec![1, 2], 3, 2),
        (vec![], 1, 1),
        (vec![1, 2, 4, 5], 25, 25),
    ] {
        let (f, tools, ..) = fixture(&ages);
        let mut reduced = Game::new(f.position(), Expansions::BASE).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, tools);
        start(&mut manual, tools);
        let reduced = outcomes(reduced, 0);
        let manual = outcomes(manual, 0);
        assert_eq!(manual.len(), manual_paths, "{ages:?}");
        assert_eq!(reduced.len(), reduced_paths, "{ages:?}");
        assert!(
            manual.iter().all(|result| reduced.contains(result)),
            "{ages:?}"
        );
        assert!(
            reduced.iter().all(|result| manual.contains(result)),
            "{ages:?}"
        );
    }
}

#[test]
fn a_real_four_card_subset_is_not_reduced_to_lowest_first() {
    let (f, tools, hand, reward) = fixture(&[1, 2, 4, 5]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    start(&mut game, tools);
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options.len(), 5);
    choose_card(&mut game, &mut events, hand[3]);
    let next = decision(&mut game, &mut events);
    assert_eq!(next.options.len(), 3);
    assert!(!next.options.contains(&Alternative::Decline));
    choose_card(&mut game, &mut events, hand[2]);
    choose_card(&mut game, &mut events, hand[1]);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Hand(A)), &[hand[0]]);
    assert_eq!(game.position.top(A, Color::Green), Some(reward));
}

#[test]
fn optional_commitment_can_be_declined_but_not_abandoned_after_a_partial_return() {
    let (f, tools, hand, reward) = fixture(&[1, 1, 1]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    start(&mut game, tools);
    let mut declined = game.clone();
    let mut events = Vec::new();
    let menu = decision(&mut declined, &mut events);
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
    assert!(events.is_empty());
    assert_eq!(declined.position.cards(Zone::Hand(A)), &hand);
    choose_card(&mut game, &mut events, hand[2]);
    let next = decision(&mut game, &mut events);
    assert_eq!(next.options.len(), 2);
    assert!(!next.options.contains(&Alternative::Decline));
    assert_eq!(
        finish(&mut game, &mut events).0,
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.top(A, Color::Green), Some(reward));
}

#[test]
fn fewer_than_three_returns_do_as_much_as_possible_without_the_reward() {
    for ages in [vec![], vec![1], vec![1, 2]] {
        let (f, tools, hand, reward) = fixture(&ages);
        let (game, events, decisions) = oracle::<false>(f.position(), tools);
        assert_eq!(events.len(), hand.len());
        assert_eq!(decisions, usize::from(!hand.is_empty()));
        assert!(game.position.cards(Zone::Hand(A)).is_empty());
        assert_eq!(game.position.cards(Zone::Deck(Deck::base(3))), &[reward]);
        assert!(game.position.top(A, Color::Green).is_none());
    }
}

#[test]
fn figure_and_base_returns_use_distinct_deck_groups_even_at_the_same_age() {
    let mut f = Fixture::default();
    let tools = f.tools_first(Zone::Board(A, Color::Blue));
    let figure = f.figure(Figure::Gilgamesh, Zone::Hand(A));
    let first = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let second = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let reward = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, tools);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3); // two Base backs + decline, not the Figure
    assert!(!menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == figure)));
    choose_card(&mut game, &mut events, second);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [second, first, figure, reward, reward]
    );
    assert_eq!(
        game.position.cards(Zone::Deck(Deck::base(1))),
        &[first, second]
    );
    assert_eq!(game.position.cards(Zone::Deck(Deck::figures(1))), &[figure]);
}

#[test]
fn return_hazards_disable_grouping_without_erasing_the_optional_commitment() {
    for (figure, color) in [
        (Figure::Homer, Color::Purple),
        (Figure::JamesClerkMaxwell, Color::Blue),
        (Figure::Nightingale, Color::Yellow),
    ] {
        let (mut f, tools, ..) = fixture(&[1, 2, 4]);
        f.figure(figure, Zone::Board(A, color));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, tools);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 4, "{figure:?}");
        assert!(menu.options.contains(&Alternative::Decline));
        assert!(events.is_empty());
    }
}

#[test]
fn a_later_draw_hazard_does_not_disable_independent_returns_or_skip_the_draw_reaction() {
    let (mut f, tools, hand, _) = fixture(&[1, 2, 4]);
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, tools);
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options.len(), 2); // lowest deck + decline
    game.choose(0).unwrap();
    let decks = decision(&mut game, &mut events);
    assert!(
        decks
            .options
            .iter()
            .all(|o| matches!(o, Alternative::Deck(_)))
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        hand
    );
    assert!(game.position.top(A, Color::Green).is_none());
}

#[test]
fn archimedes_modifies_the_boxed_reward_not_the_ordinary_three_card_payment() {
    let (mut f, tools, hand, original_reward) = fixture(&[1, 2, 5]);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let boosted = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let (game, events, decisions) = oracle::<true>(f.position(), tools);
    assert_eq!(decisions, 1);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [hand[0], hand[1], hand[2], boosted, boosted]
    );
    assert_eq!(game.position.top(A, Color::Yellow), Some(boosted));
    assert_eq!(
        game.position.cards(Zone::Deck(Deck::base(3))),
        &[original_reward]
    );
}

#[test]
fn hedys_claim_reaction_does_not_block_the_independent_return_phase() {
    let (mut f, tools, hand, reward) = fixture(&[1, 2, 5]);
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    let (game, events, decisions) = oracle::<true>(f.position(), tools);
    assert_eq!(decisions, 1);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [hand[0], hand[1], hand[2], reward, reward]
    );
    assert_eq!(game.position.top(A, Color::Green), Some(reward));
}

#[test]
fn homer_replacement_consumes_a_payment_slot_but_does_not_complete_the_three_returns() {
    let (mut f, tools, _, reward) = fixture(&[2, 4]);
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let figure = f.figure(Figure::Archimedes, Zone::Hand(A));
    let (game, events, _) = oracle::<true>(f.position(), tools);
    assert_eq!(events.len(), 3);
    assert_eq!(
        events
            .iter()
            .filter(|e| moved(e).verb == Some(Verb::Return))
            .count(),
        2
    );
    assert!(events.iter().any(|e| moved(e).card == figure
        && moved(e).verb == Some(Verb::Tuck)
        && moved(e).inside_karma));
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(3))), &[reward]);
    assert!(game.position.top(A, Color::Green).is_none());
}

#[test]
fn maxwells_new_hand_cards_are_live_candidates_but_only_three_original_returns_are_attempted() {
    let (mut f, tools, hand, reward) = fixture(&[1, 2, 4]);
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let eights: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]))
        .collect();
    let (game, events, decisions) = oracle::<true>(f.position(), tools);
    assert_eq!(decisions, 3);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [
            eights[2], hand[0], eights[1], hand[1], eights[0], hand[2], reward, reward
        ]
    );
    assert_eq!(
        game.position.cards(Zone::Hand(A)),
        eights.into_iter().rev().collect::<Vec<_>>()
    );
    assert_eq!(game.position.top(A, Color::Green), Some(reward));
}

#[test]
fn terminal_reward_draw_happens_after_all_returns_in_the_canonical_manual_order() {
    let (mut f, tools, hand, reward) = fixture(&[1, 1, 2]);
    f.placements[usize::from(reward.0)].1 = Zone::Junk;
    let (mut game, events, _) = oracle::<false>(f.position(), tools);
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        hand
    );
    assert_eq!(
        settle(&mut game, &mut Vec::new()),
        Status::Draw(DrawReason::Exhaustion)
    );
}

#[test]
fn borrowed_hand_origins_keep_ordinary_choices_and_count_real_returns_once() {
    let (mut f, tools, hand, reward) = fixture(&[2, 4]);
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let borrowed = f.tech(Color::Yellow, 1, Zone::Hand(B), vec![]);
    let position = f.position();
    let mut probe = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    start(&mut probe, tools);
    assert_eq!(decision(&mut probe, &mut Vec::new()).options.len(), 4);
    let (game, events, _) = oracle::<true>(position, tools);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [borrowed, hand[0], hand[1], reward, reward]
    );
    assert_eq!(moved(&events[0]).source, Zone::Hand(B));
    assert!(game.position.cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position.top(A, Color::Green), Some(reward));
}

#[test]
fn foreign_control_preserves_privacy_and_the_initial_commitment_choice() {
    let make = |reverse| {
        let (mut f, tools, hand, _) = fixture(&[1, 1, 2]);
        if reverse {
            f.placements
                .swap(usize::from(hand[0].0), usize::from(hand[1].0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        game.start(
            Operation::ExecuteEffect {
                card: tools,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let mut events = Vec::new();
    let menu = decision(&mut first, &mut events);
    assert_eq!(menu, decision(&mut second, &mut events));
    assert_eq!(menu.player, B);
    assert!(menu.options.contains(&Alternative::Decline));
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Return,
            source: Zone::Hand(A),
            ..
        } | Alternative::Decline
    )));
    assert!(events.is_empty());
}

#[test]
fn clones_preserve_committed_quota_stop_permission_and_grouping_at_every_boundary() {
    let (f, tools, ..) = fixture(&[1, 1, 2]);
    let mut game = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    start(&mut game, tools);
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
    panic!("Tools did not complete");
}

#[test]
fn continuous_figures_preserve_tools_return_reduction_and_real_catherine_meld_first() {
    for figure in [
        Figure::NiccoloMachiavelli,
        Figure::CatherineTheGreat,
        Figure::SergeyBrin,
    ] {
        let mut f = Fixture::default();
        let tools = f.tools(Zone::Board(A, Color::Blue));
        let card = figure_card(figure.code()).unwrap();
        let color = card.color;
        let top = f.add(card, Zone::Board(A, color));
        let reward = f.add(base_card("B031").unwrap(), Zone::Deck(Deck::base(3)));
        let old_green = if color == Color::Green {
            top
        } else {
            f.sailing(Zone::Board(A, Color::Green))
        };
        for code in ["B001", "B021", "B042"] {
            f.add(base_card(code).unwrap(), Zone::Hand(A));
        }
        let position = f.position();
        let mut reduced = Game::seeded(position.clone(), Expansions::FIGURES, 42).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, tools);
        start(&mut manual, tools);
        let reduced_paths = outcomes(reduced, 0);
        let manual_paths = outcomes(manual, 0);
        assert_eq!(reduced_paths.len(), 2);
        assert_eq!(manual_paths.len(), 7);
        assert!(
            manual_paths
                .iter()
                .all(|outcome| reduced_paths.contains(outcome))
        );
        assert!(
            reduced_paths
                .iter()
                .all(|outcome| manual_paths.contains(outcome))
        );
        let (game, events, decisions) = oracle::<true>(position, tools);
        assert_eq!(decisions, 1);
        assert_eq!(game.position.top(A, Color::Green), Some(reward));
        if figure == Figure::CatherineTheGreat {
            assert_eq!(game.position.cards(Zone::Hand(A)), &[old_green]);
            assert!(events.iter().any(|event| matches!(
                event,
                Event::Moved(m) if m.card == old_green
                    && m.verb == Some(Verb::Transfer) && m.inside_karma
            )));
        }
    }
}
