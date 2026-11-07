//! Expected outcomes for the last six Base programs: Road Building,
//! Vaccination, Classification, Composites, Collaboration, Miniaturization.
use inno_engine::*;

mod common;
use common::*;

fn printed(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

fn invoke(game: &mut Game, source: CardId, effect: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn select(game: &mut Game, events: &mut Vec<Event>, card: CardId) {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|option| {
            matches!(option, Alternative::Move { target, .. } | Alternative::Select { target }
                | Alternative::Reveal { target }
            if match target {
                TargetDescriptor::Known { card: known, .. } => *known == card,
                TargetDescriptor::Hidden { source, slot, .. } =>
                    game.position().cards(*source)[*slot] == card,
            })
        })
        .expect("offered card");
    game.choose(index).unwrap();
}

fn decline(game: &mut Game, events: &mut Vec<Event>) {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|option| *option == Alternative::Decline)
        .expect("optional");
    game.choose(index).unwrap();
}

#[test]
fn road_building_melds_two_then_trades_red_for_the_opponents_green() {
    let mut f = Fixture::default();
    let road = printed(&mut f, "B017", A);
    let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let green = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let theirs = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, road, 0, A, A);
    let mut events = Vec::new();
    select(&mut game, &mut events, blue);
    select(&mut game, &mut events, green);
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.contains(&Alternative::Decline));
    game.choose(0).unwrap(); // Transfer the top red card.
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), [road]);
    assert_eq!(
        position.cards(Zone::Board(A, Color::Green)),
        [green, theirs]
    );
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), [blue]);
    assert!(position.cards(Zone::Board(B, Color::Green)).is_empty());
}

#[test]
fn road_building_with_one_meld_offers_no_transfer() {
    let mut f = Fixture::default();
    let road = printed(&mut f, "B017", A);
    let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, road, 0, A, A);
    let mut events = Vec::new();
    select(&mut game, &mut events, blue);
    decline(&mut game, &mut events);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), [road]);
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 1);
}

#[test]
fn vaccination_returns_every_card_of_a_present_value_and_rewards_both_effects() {
    let mut f = Fixture::default();
    let vaccination = printed(&mut f, "B059", A);
    let threes = [
        f.tech(Color::Blue, 3, Zone::Score(B), vec![]),
        f.tech(Color::Green, 3, Zone::Score(B), vec![]),
    ];
    let five = f.tech(Color::Blue, 5, Zone::Score(B), vec![]);
    let six = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
    let seven = f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card: vaccination,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B);
    assert_eq!(menu.options, [Alternative::Value(3), Alternative::Value(5)]);
    game.choose(0).unwrap();
    // Two cards return to one deck: their order is a real choice.
    select(&mut game, &mut events, threes[1]);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(B)), [five]);
    assert_eq!(
        position.cards(Zone::Deck(Deck::base(3))),
        [threes[0], threes[1]]
    );
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), [six]);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), [seven]);
}

#[test]
fn classification_reveals_both_hands_before_the_cards_they_affect_move() {
    let mut f = Fixture::default();
    let classification = printed(&mut f, "B060", A);
    let mine = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let kept = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let first = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let red = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let second = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    assert!(
        game.tracker(A)
            .unwrap()
            .belief(Zone::Hand(B))
            .iter()
            .any(|slot| slot.candidates.contains(&red) && slot.known().is_none())
    );
    invoke(&mut game, classification, 0, A, A);
    let mut events = Vec::new();
    select(&mut game, &mut events, mine);
    // The sweep has no menu; the next choice is the meld order.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3);
    assert!(menu.options.iter().all(|option| matches!(
        option,
        Alternative::Move {
            verb: Verb::Meld,
            target: TargetDescriptor::Known { .. }
        }
    )));
    let transfers: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Transfer) => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(transfers, [first, second]);
    assert_eq!(
        game.tracker(A).unwrap().belief(Zone::Hand(B))[0].known(),
        Some(red)
    );
    // The whole hand is revealed before the first transfer, and only once.
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed {
                card,
                source: Zone::Hand(B),
                ..
            } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, [first, red, second]);
    let reveal_at = events
        .iter()
        .rposition(|event| {
            matches!(
                event,
                Event::Revealed {
                    source: Zone::Hand(B),
                    ..
                }
            )
        })
        .unwrap();
    let transfer_at = events
        .iter()
        .position(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Transfer)))
        .unwrap();
    assert!(reveal_at < transfer_at);
    // The executor's own hand is shown before its melds too: after the card
    // revealed first, only the rest, not the transferred cards again.
    let own: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed {
                card,
                source: Zone::Hand(A),
                ..
            } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(own, [mine, kept]);
    assert!(
        game.tracker(B)
            .unwrap()
            .belief(Zone::Hand(A))
            .iter()
            .all(|slot| slot.known().is_some())
    );
    select(&mut game, &mut events, second);
    select(&mut game, &mut events, mine);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(B)), [red]);
    assert_eq!(
        position.cards(Zone::Board(A, Color::Blue)),
        [second, mine, first]
    );
    assert_eq!(position.cards(Zone::Hand(A)), [kept]);
    assert!(position.check_conservation());
    assert!(matches!(events.last(), Some(Event::Moved(m)) if m.verb == Some(Verb::Meld)));
    for viewer in Player::ALL {
        let tracker = game.tracker(viewer).unwrap();
        assert!(tracker.check_truth(position));
        assert_eq!(tracker.belief(Zone::Hand(A))[0].known(), Some(kept));
        assert_eq!(tracker.belief(Zone::Hand(B))[0].known(), Some(red));
        let sampled = game.determinize(viewer, 7).unwrap();
        assert_eq!(sampled.position().cards(Zone::Hand(A)), [kept]);
        assert_eq!(sampled.position().cards(Zone::Hand(B)), [red]);
    }
}

#[test]
fn classification_under_yi_sun_sin_lets_the_executor_order_the_revealed_matches() {
    let mut f = Fixture::default();
    let classification = printed(&mut f, "B060", A);
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let mine = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let first = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let red = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let second = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let fours = [
        f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]),
        f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    invoke(&mut game, classification, 0, A, A);
    let mut events = Vec::new();
    // Each transfer becomes a tuck, a draw and a score, so the order is a
    // real choice, made among the cards the reveal just showed.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    let offered: Vec<_> = menu
        .options
        .iter()
        .map(|option| match option {
            Alternative::Move {
                verb: Verb::Transfer,
                target: TargetDescriptor::Known { card, .. },
            } => *card,
            other => panic!("expected a known transfer, got {other:?}"),
        })
        .collect();
    assert_eq!(offered, [first, second]);
    assert!(events.iter().all(|event| !matches!(event, Event::Moved(_))));
    select(&mut game, &mut events, second);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    let tucked: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Tuck) => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(tucked, [second, fours[1], first, fours[0]]);
    let position = game.position();
    assert_eq!(
        position.cards(Zone::Board(A, Color::Blue)),
        [first, second, mine]
    );
    assert_eq!(position.cards(Zone::Hand(B)), [red]);
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert!(position.check_conservation());
    for viewer in Player::ALL {
        let tracker = game.tracker(viewer).unwrap();
        assert!(tracker.check_truth(position));
        assert_eq!(tracker.belief(Zone::Hand(B))[0].known(), Some(red));
    }
}

#[test]
fn classification_verifies_no_matches_but_does_not_reveal_without_a_chosen_color() {
    for has_card in [false, true] {
        for expansions in [
            Expansions::BASE,
            Expansions::FIGURES,
            Expansions::CITIES,
            Expansions::FIGURES.union(Expansions::CITIES),
        ] {
            let mut f = Fixture::default();
            let classification = printed(&mut f, "B060", A);
            if has_card {
                f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
            }
            let others = [
                f.tech(Color::Red, 2, Zone::Hand(B), vec![]),
                f.tech(Color::Green, 2, Zone::Hand(B), vec![]),
            ];
            f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
            let mut game = Game::new(f.position(), expansions)
                .unwrap()
                .with_tracking()
                .unwrap();
            let before = game.tracker(A).unwrap().belief(Zone::Hand(B));
            assert!(before.iter().all(|slot| slot.known().is_none()));
            invoke(&mut game, classification, 0, A, A);
            let mut events = Vec::new();
            assert!(matches!(
                settle(&mut game, &mut events),
                Status::Complete(_)
            ));
            let after = game.tracker(A).unwrap().belief(Zone::Hand(B));
            if has_card {
                let revealed: Vec<_> = events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Revealed {
                            card,
                            source: Zone::Hand(B),
                            ..
                        } => Some(*card),
                        _ => None,
                    })
                    .collect();
                assert_eq!(revealed, others);
                assert_eq!(
                    after
                        .iter()
                        .filter_map(BeliefSlot::known)
                        .collect::<Vec<_>>(),
                    others
                );
            } else {
                assert!(events.is_empty());
                assert_eq!(after, before);
            }
            assert_eq!(game.position().cards(Zone::Hand(B)), others);
            assert!(game.position().check_conservation());
            for viewer in Player::ALL {
                assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
            }
        }
    }
}

#[test]
fn composites_keeps_one_card_and_takes_the_highest_score_card() {
    let mut f = Fixture::default();
    let composites = printed(&mut f, "B086", A);
    let hand = [1, 2, 3].map(|age| f.tech(Color::Blue, age, Zone::Hand(B), vec![]));
    f.tech(Color::Blue, 4, Zone::Score(B), vec![]);
    let six = f.tech(Color::Green, 6, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, composites, 0, B, A);
    let mut events = Vec::new();
    // Distinct backs: only which card stays is a choice, not the order.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    select(&mut game, &mut events, hand[0]);
    select(&mut game, &mut events, hand[2]);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(B)), [hand[1]]);
    assert_eq!(position.cards(Zone::Hand(A)), [hand[0], hand[2]]);
    assert_eq!(position.cards(Zone::Score(A)), [six]);
}

#[test]
fn collaboration_lets_the_demander_pick_and_the_executor_melds_the_other() {
    let mut f = Fixture::default();
    let collaboration = printed(&mut f, "B090", A);
    let blue = f.tech(Color::Blue, 9, Zone::Deck(Deck::base(9)), vec![]);
    let red = f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, collaboration, 0, B, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    select(&mut game, &mut events, blue);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), [blue]);
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), [red]);
    let reveals = events
        .iter()
        .filter(|event| matches!(event, Event::Revealed { .. }))
        .count();
    assert_eq!(reveals, 2);
}

#[test]
fn collaboration_wins_with_ten_green_cards() {
    for greens in [9, 10] {
        let mut f = Fixture::default();
        let collaboration = printed(&mut f, "B090", A);
        for _ in 1..greens {
            f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        invoke(&mut game, collaboration, 1, A, A);
        let status = settle(&mut game, &mut Vec::new());
        if greens == 10 {
            assert_eq!(status, Status::Winner(A));
        } else {
            assert!(matches!(status, Status::Complete(_)));
        }
    }
}

#[test]
fn miniaturization_rewards_a_returned_ten_and_junks_the_eleven_deck() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B096", A);
    let ten = f.tech(Color::Blue, 10, Zone::Hand(A), vec![]);
    for age in [1, 1, 4] {
        f.tech(Color::Blue, age, Zone::Score(A), vec![]);
    }
    let drawn = [
        f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]),
        f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, A, A);
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    // Two different score values: two draws, leaving the returned ten.
    assert_eq!(game.position().cards(Zone::Hand(A)), [drawn[1], drawn[0]]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), [ten]);

    let mut f = Fixture::default();
    let source = printed(&mut f, "B096", A);
    let eleven = f.tech(Color::Blue, 11, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, A, A);
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    let position = game.position();
    assert!(position.cards(Zone::Deck(Deck::base(11))).is_empty());
    assert_eq!(position.cards(Zone::Junk).len(), 2);
    assert!(position.cards(Zone::Junk).contains(&eleven));
}
