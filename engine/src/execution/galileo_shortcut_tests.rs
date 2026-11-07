use super::common::*;
use super::*;

fn start_draw(game: &mut Game) {
    game.start(
        Operation::Draw {
            value: 6,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
}

fn fixture() -> (Fixture, Vec<CardId>, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let cards: Vec<_> = [Color::Red, Color::Blue, Color::Yellow]
        .into_iter()
        .map(|color| f.tech(color, 4, Zone::Deck(Deck::base(4)), vec![]))
        .collect();
    // Make physical order different from either identity-sorted order.
    f.placements
        .swap(usize::from(cards[0].0), usize::from(cards[1].0));
    let reward = f.tech(Color::Purple, 6, Zone::Deck(Deck::base(6)), vec![]);
    f.figure(Figure::YiSunSin, Zone::Deck(Deck::figures(4)));
    f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Yellow, 1, Zone::Junk, vec![]);
    (f, cards, reward)
}

fn assert_decks(menu: &Decision, player: Player) {
    assert_eq!(menu.player, player);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.contains(&Alternative::Value(4)));
    assert!(menu.options.contains(&Alternative::Value(5)));
}

fn choose_deck(game: &mut Game, menu: &Decision, age: u8) {
    game.choose(
        menu.options
            .iter()
            .position(|option| *option == Alternative::Value(age))
            .unwrap(),
    )
    .unwrap();
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    if let Some(index) = menu
        .options
        .iter()
        .position(|option| *option == Alternative::Value(4))
    {
        return index;
    }
    menu.options
        .iter()
        .position(|option| match option {
            Alternative::Move {
                verb: Verb::Junk,
                target: TargetDescriptor::Hidden { source, slot, .. },
            } if matches!(source, Zone::Deck(_)) => *slot + 1 == game.position.cards(*source).len(),
            _ => false,
        })
        .expect("the physical top is a legal junk choice")
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut choices = 0;
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                choices += 1;
                game.choose(canonical(game, &menu)).unwrap();
            }
            status => return (status, choices),
        }
    }
    panic!("Galileo draw did not complete");
}

fn drawn(card: CardId) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(6),
        short: false,
    }))
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
            // Only hidden Junk is unordered in the supported core. Preserve
            // every deck, hand, board and other pile's exact physical order.
            game.position.junk.sort_unstable();
            vec![(game.position, status)]
        }
    }
}

#[test]
fn canonical_suppressed_sweep_preserves_events_rng_and_exact_final_game() {
    let (f, cards, reward) = fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let initial_rng = reduced.rng.clone();
    start_draw(&mut reduced);
    start_draw(&mut manual);
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    assert_eq!(finish(&mut reduced, &mut events), (drawn(reward), 1));
    // Deck commitment plus two sweep choices; the last card is automatic.
    assert_eq!(finish(&mut manual, &mut manual_events), (drawn(reward), 3));
    assert_eq!(events, manual_events);
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [cards[2], cards[0], cards[1], reward]
    );
    for event in &events[..3] {
        let movement = moved(event);
        assert_eq!(movement.verb, Some(Verb::Junk));
        assert_eq!(movement.source, Zone::Deck(Deck::base(4)));
        assert_eq!(movement.destination, Zone::Junk);
        assert_eq!(movement.executor, A);
        assert!(movement.inside_karma);
    }
    assert_eq!(moved(&events[3]).verb, None);
    assert!(!moved(&events[3]).inside_karma);
    assert_eq!(reduced.rng, initial_rng);
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual);
    assert!(reduced.position.check_conservation());
}

#[test]
fn three_card_oracle_has_six_committed_paths_and_empty_route_reduced_to_one_and_empty() {
    let (f, _, reward) = fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start_draw(&mut reduced);
    start_draw(&mut manual);
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert_eq!(manual.len(), 7);
    assert_eq!(reduced.len(), 2);
    for paths in [&manual, &reduced] {
        assert_eq!(
            paths
                .iter()
                .filter(|(position, _)| !position.cards(Zone::Deck(Deck::base(4))).is_empty())
                .count(),
            1
        );
        assert!(paths.iter().all(|(_, status)| *status == drawn(reward)));
    }
    assert!(manual.iter().all(|result| reduced.contains(result)));
    assert!(reduced.iter().all(|result| manual.contains(result)));
}

#[test]
fn mandatory_deck_menu_keeps_empty_deck_and_never_offers_decline_or_a_later_stop() {
    for disabled in [false, true] {
        let (f, _, reward) = fixture();
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.shortcuts_disabled = disabled;
        let initial_deck = game.position.cards(Zone::Deck(Deck::base(4))).to_vec();
        start_draw(&mut game);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_decks(&menu, A);
        assert!(events.is_empty());
        let mut empty = game.clone();
        choose_deck(&mut empty, &menu, 5);
        assert_eq!(settle(&mut empty, &mut events), drawn(reward));
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).card, reward);
        assert_eq!(
            empty.position.cards(Zone::Deck(Deck::base(4))),
            initial_deck
        );
        events.clear();
        choose_deck(&mut game, &menu, 4);
        if disabled {
            for size in [3, 2] {
                let sweep = decision(&mut game, &mut events);
                assert_eq!(sweep.options.len(), size);
                assert!(sweep.options.iter().all(|option| matches!(
                    option,
                    Alternative::Move {
                        verb: Verb::Junk,
                        target: TargetDescriptor::Hidden {
                            source: Zone::Deck(_),
                            back: Deck {
                                set: CardSet::Base,
                                age: 4
                            },
                            ..
                        },
                    }
                )));
                game.choose(canonical(&game, &sweep)).unwrap();
            }
        }
        assert_eq!(settle(&mut game, &mut events), drawn(reward));
        assert_eq!(events.len(), 4);
        assert!(game.position.cards(Zone::Deck(Deck::base(4))).is_empty());
    }
}

#[test]
fn hidden_front_permutations_preserve_deck_and_unrestricted_slot_menus() {
    for disabled in [false, true] {
        let (f, _, _) = fixture();
        let position = f.position();
        let mut permuted = position.clone();
        permuted.zone_mut(Zone::Deck(Deck::base(4))).reverse();
        let mut first = Game::new(position, Expansions::FIGURES).unwrap();
        let mut second = Game::new(permuted, Expansions::FIGURES).unwrap();
        first.shortcuts_disabled = disabled;
        second.shortcuts_disabled = disabled;
        start_draw(&mut first);
        start_draw(&mut second);
        let mut first_events = Vec::new();
        let mut second_events = Vec::new();
        let menu = decision(&mut first, &mut first_events);
        assert_eq!(menu, decision(&mut second, &mut second_events));
        assert_decks(&menu, A);
        assert!(first_events.is_empty() && second_events.is_empty());
        choose_deck(&mut first, &menu, 4);
        choose_deck(&mut second, &menu, 4);
        if disabled {
            for size in [3, 2] {
                let menu = decision(&mut first, &mut first_events);
                assert_eq!(menu, decision(&mut second, &mut second_events));
                assert_eq!(menu.options.len(), size);
                assert!(menu.options.iter().all(|option| matches!(
                    option,
                    Alternative::Move {
                        verb: Verb::Junk,
                        target: TargetDescriptor::Hidden {
                            back: Deck {
                                set: CardSet::Base,
                                age: 4
                            },
                            ..
                        },
                    }
                )));
                first.choose(0).unwrap();
                second.choose(0).unwrap();
            }
        }
        assert_eq!(
            settle(&mut first, &mut first_events),
            settle(&mut second, &mut second_events)
        );
        first.position.junk.sort_unstable();
        second.position.junk.sort_unstable();
        assert_eq!(first, second);
    }
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
                status => panic!("completed before next movement: {status:?}"),
            }
        }
        panic!("next movement did not occur");
    }

    let (f, _, reward) = fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let initial_rng = reduced.rng.clone();
    start_draw(&mut reduced);
    start_draw(&mut manual);
    for _ in 0..4 {
        assert_eq!(next_movement(&mut reduced), next_movement(&mut manual));
        assert_eq!(reduced.position, manual.position);
        assert_eq!(reduced.rng, manual.rng);
        assert_eq!(reduced.rng, initial_rng);
    }
    let mut events = Vec::new();
    assert_eq!(settle(&mut reduced, &mut events), drawn(reward));
    assert_eq!(settle(&mut manual, &mut events), drawn(reward));
    assert!(events.is_empty());
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual);
}

#[test]
fn native_clones_resume_exactly_at_every_single_work_boundary() {
    for disabled in [false, true] {
        let (f, _, reward) = fixture();
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
        game.shortcuts_disabled = disabled;
        start_draw(&mut game);
        let mut expected = game.clone();
        let mut all = Vec::new();
        assert_eq!(finish(&mut expected, &mut all).0, drawn(reward));
        let mut prefix = Vec::new();
        let mut complete = false;
        for _ in 0..300 {
            let mut fork = game.clone();
            let mut suffix = Vec::new();
            assert_eq!(finish(&mut fork, &mut suffix).0, drawn(reward));
            assert_eq!(fork, expected);
            assert_eq!(
                prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
                all
            );
            match game.advance(1, &mut prefix).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(menu) => game.choose(canonical(&game, &menu)).unwrap(),
                status => {
                    assert_eq!(status, drawn(reward));
                    assert_eq!(game, expected);
                    complete = true;
                    break;
                }
            }
            assert!(game.position.check_conservation());
        }
        assert!(complete, "Galileo did not complete");
    }
}

fn prosperity(f: &mut Fixture, value: u8, zone: Zone) -> CardId {
    let card = f.tech(Color::Blue, value, zone, vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
    card
}

#[test]
fn oars_restores_real_transfer_choices_for_active_galileo_but_not_inactive_sources() {
    for state in 0..5 {
        let mut f = Fixture::default();
        let oars = f.oars(Zone::Board(A, Color::Red));
        let zone = match state {
            0 | 1 => Zone::Board(B, Color::Green),
            2 => Zone::Score(B),
            3 => Zone::Junk,
            4 => Zone::Hand(A),
            _ => unreachable!(),
        };
        f.figure(Figure::GalileoGalilei, zone);
        if state == 1 {
            f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
        }
        let high = prosperity(&mut f, 7, Zone::Hand(B));
        let low = prosperity(&mut f, 2, Zone::Hand(B));
        for _ in 0..2 {
            f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        // A nonempty 4 keeps Galileo's deck choice real.
        f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: oars,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        if state == 0 {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, B);
            assert_eq!(menu.options.len(), 2);
            assert!(events.is_empty());
            assert!(menu.options.iter().all(|option| matches!(
                option,
                Alternative::Move {
                    verb: Verb::Transfer,
                    ..
                }
            )));
            choose_card(&mut game, &mut events, high);
            // The higher card is genuinely selectable, not merely displayed.
            for expected_transfers in [1, 2] {
                let deck_menu = decision(&mut game, &mut events);
                assert_decks(&deck_menu, B);
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| moved(event).verb == Some(Verb::Transfer))
                        .count(),
                    expected_transfers
                );
                choose_deck(&mut game, &deck_menu, 5);
            }
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(game.position.cards(Zone::Score(A)), &[high, low]);
        } else {
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(game.position.cards(Zone::Score(A)), &[low, high]);
        }
        assert_eq!(events.len(), 6);
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed {
                        card,
                        source: Zone::Hand(B),
                        ..
                    } => Some(*card),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            game.position.cards(Zone::Hand(B)),
        );
        assert!(game.position.check_conservation());
    }
}

#[test]
fn tools_still_has_one_return_commitment_then_real_draw_and_meld_first_reactions() {
    let mut f = Fixture::default();
    let tools = f.tools_first(Zone::Board(A, Color::Blue));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let hand: Vec<_> = [4, 1, 2]
        .into_iter()
        .map(|value| f.tech(Color::Red, value, Zone::Hand(A), vec![]))
        .collect();
    let reward = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let achievement = f.tech(Color::Yellow, 3, Zone::AvailableAchievements, vec![]);
    let other = f.tech(Color::Blue, 5, Zone::AvailableAchievements, vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: tools,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let commitment = decision(&mut game, &mut events);
    assert_eq!(commitment.options.len(), 2);
    assert!(commitment.options.contains(&Alternative::Decline));
    assert!(events.is_empty());
    choose_card(&mut game, &mut events, hand[1]);
    // No further return decision: all three independent returns precede Draw First.
    let decks = decision(&mut game, &mut events);
    assert_decks(&decks, A);
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [hand[1], hand[2], hand[0]]
    );
    assert!(
        events
            .iter()
            .all(|event| moved(event).verb == Some(Verb::Return) && !moved(event).inside_karma)
    );
    choose_deck(&mut game, &decks, 4);
    let achievements = decision(&mut game, &mut events);
    assert_eq!(events.len(), 5);
    assert_eq!(moved(&events[3]).card, hand[0]);
    assert_eq!(moved(&events[3]).verb, Some(Verb::Junk));
    assert!(moved(&events[3]).inside_karma);
    assert_eq!(moved(&events[4]).card, reward);
    assert_eq!(moved(&events[4]).verb, None);
    assert!(!moved(&events[4]).inside_karma);
    assert_eq!(game.position.cards(Zone::Hand(A)), &[reward]);
    assert_eq!(game.position.top(A, Color::Purple), None);
    assert_eq!(achievements.options.len(), 2);
    assert!(achievements.options.iter().all(|option| matches!(
        option,
        Alternative::BlindMove {
            verb: Verb::Junk,
            source: Zone::AvailableAchievements,
            ..
        }
    )));
    game.choose(
        achievements
            .options
            .iter()
            .position(|option| {
                matches!(
                    option,
                    Alternative::BlindMove {
                        back: Deck { age: 3, .. },
                        ..
                    }
                )
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 7);
    assert_eq!(moved(&events[5]).card, achievement);
    assert_eq!(moved(&events[5]).verb, Some(Verb::Junk));
    assert!(moved(&events[5]).inside_karma);
    assert_eq!(moved(&events[6]).card, reward);
    assert_eq!(moved(&events[6]).verb, Some(Verb::Meld));
    assert!(!moved(&events[6]).inside_karma);
    assert_eq!(game.position.top(A, Color::Purple), Some(reward));
    assert_eq!(game.position.cards(Zone::AvailableAchievements), &[other]);
    assert!(game.position.check_conservation());
}

#[test]
fn active_galileo_does_not_disable_ecology_deck_reduction() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let cards: Vec<_> = [Color::Blue, Color::Red, Color::Purple]
        .into_iter()
        .map(|color| f.tech(color, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: ecology,
            effect: 1,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.contains(&Alternative::Decline));
    assert!(events.is_empty());
    game.choose(canonical(&game, &menu)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        cards.into_iter().rev().collect::<Vec<_>>()
    );
    assert!(
        events
            .iter()
            .all(|event| moved(event).verb == Some(Verb::Junk) && !moved(event).inside_karma)
    );
    assert!(game.position.check_conservation());
}
