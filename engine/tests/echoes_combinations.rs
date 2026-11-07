//! Full authored setups cross the native action, promotion and Figure boundaries.
use inno_engine::*;

const A: Player = Player::One;
const B: Player = Player::Two;
const EF: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);
const MODES: [Expansions; 2] = [EF, EF.union(Expansions::CITIES)];

fn zones() -> Vec<Zone> {
    let mut zones = vec![
        Zone::AvailableAchievements,
        Zone::AgeZeroReserve,
        Zone::Junk,
    ];
    for player in Player::ALL {
        zones.extend([
            Zone::Hand(player),
            Zone::Score(player),
            Zone::Achievements(player),
            Zone::Forecast(player),
        ]);
        zones.extend(Color::ALL.map(|color| Zone::Board(player, color)));
    }
    for set in CardSet::ALL {
        zones.extend((0..=11).map(|age| Zone::Deck(Deck { set, age })));
    }
    zones
}
fn source(game: &Game, card: CardId) -> Zone {
    zones()
        .into_iter()
        .find(|&zone| game.position().cards(zone).contains(&card))
        .unwrap()
}
fn id(game: &Game, code: &str) -> CardId {
    zones()
        .into_iter()
        .flat_map(|zone| game.position().cards(zone))
        .copied()
        .find(|&card| game.position().card(card).code == code)
        .unwrap()
}
fn check(game: &Game) {
    assert!(game.position().check_conservation());
    for tracker in Player::ALL
        .into_iter()
        .filter_map(|p| game.tracker(p))
        .chain(game.public_tracker())
    {
        assert!(tracker.check_truth(game.position()));
    }
}
fn settle(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..4000 {
        let mut clone = game.clone();
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(clone.advance(1, &mut expected).unwrap(), status);
        assert_eq!(actual, expected);
        assert_eq!(*game, clone);
        check(game);
        events.extend(actual);
        if status != Status::WorkLimit {
            return status;
        }
    }
    panic!("native action failed to finish");
}
fn decline(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|option| matches!(option, Alternative::Decline))
        .unwrap_or_else(|| panic!("expected optional decision: {menu:?}"))
}
fn finish(game: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..30 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let choice = if let Some(index) = menu
                    .options
                    .iter()
                    .position(|option| *option == Alternative::Decline)
                {
                    index
                } else {
                    assert!(
                        menu.options.iter().all(|option| matches!(
                            option,
                            Alternative::Move {
                                verb: Verb::Return,
                                ..
                            } | Alternative::BlindMove {
                                verb: Verb::Return,
                                ..
                            }
                        )),
                        "{menu:?}"
                    );
                    0
                };
                game.choose(choice).unwrap();
            }
            Status::Complete(_) => return,
            status => panic!("unexpected {status:?}"),
        }
    }
    panic!("too many decisions");
}
fn operate(game: &mut Game, op: Operation) {
    game.start(op, Context::new(A)).unwrap();
    assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
}
fn place(game: &mut Game, code: &str, destination: Zone) -> CardId {
    let card = id(game, code);
    let physical = source(game, card);
    operate(
        game,
        Operation::Move {
            verb: Verb::Transfer,
            target: Target {
                card,
                physical,
                logical: LogicalZone::Physical(physical),
            },
            destination,
        },
    );
    assert_eq!(source(game, card), destination);
    card
}
fn board(game: &mut Game, code: &str) -> CardId {
    let color = game.position().card(id(game, code)).color;
    place(game, code, Zone::Board(A, color))
}
fn new(expansions: Expansions) -> Game {
    let mut game = Game::new_game(7, expansions)
        .unwrap()
        .with_tracking()
        .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Decision(_)
        ));
        game.choose(0).unwrap();
    }
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    // Retain the complete native setup and deck order. Only arrange the small
    // acceptance position through native transfers, retaining native setup knowledge.
    for player in Player::ALL {
        let cards: Vec<_> = [Zone::Hand(player)]
            .into_iter()
            .chain(Color::ALL.map(|c| Zone::Board(player, c)))
            .flat_map(|zone| game.position().cards(zone))
            .copied()
            .collect();
        for card in cards {
            let code = game.position().card(card).code;
            let origin = game.position().card(card).origin_deck();
            place(&mut game, code, Zone::Deck(origin));
        }
    }
    game
}
fn sampled_views(game: &Game, viewer: Player) {
    let before = game.clone();
    let row = game.observe(viewer).unwrap();
    let mut sampled_forecasts = Vec::new();
    for seed in 0..4 {
        let world = game.determinize(viewer, seed).unwrap();
        check(&world);
        assert_eq!(world.observe(viewer).unwrap(), row);
        sampled_forecasts.push(world.position().cards(Zone::Forecast(A)).to_vec());
    }
    sampled_forecasts.sort();
    sampled_forecasts.dedup();
    assert!(
        sampled_forecasts.len() > 1,
        "privacy probe changes actual hidden Forecast faces"
    );
    assert_eq!(*game, before, "sampling consumes no gameplay state");
}
fn sampled_continue(game: &Game, menu: &Decision, choice: usize, promoted: bool) {
    for seed in 0..3 {
        let mut world = game.determinize(menu.player, seed).unwrap();
        assert_eq!(
            world.observe(menu.player).unwrap(),
            game.observe(menu.player).unwrap()
        );
        let Status::Decision(sampled_menu) = settle(&mut world, &mut Vec::new()) else {
            panic!("retained decision");
        };
        assert_eq!(sampled_menu.options, menu.options);
        assert_eq!(
            sampled_menu.source_card(menu.player),
            menu.source_card(menu.player)
        );
        world.choose(choice).unwrap();
        let mut events = Vec::new();
        finish(&mut world, &mut events);
        assert_eq!(world.position().cards(Zone::Forecast(A)).len(), 2);
        assert_eq!(
            moves(&events)
                .filter(|m| m.verb == Some(Verb::Foreshadow))
                .count(),
            if promoted { 2 } else { 1 }
        );
        assert_eq!(world.position().regular_actions_started(), 1);
        if promoted {
            assert!(
                Player::ALL
                    .into_iter()
                    .all(|p| world.position().cards(Zone::Hand(p)).is_empty())
            );
        }
        check(&world);
    }
}
fn moves(events: &[Event]) -> impl Iterator<Item = &Movement> {
    events.iter().filter_map(|event| {
        if let Event::Moved(m) = event {
            Some(m)
        } else {
            None
        }
    })
}

#[test]
fn native_dogma_with_archimedes_admits_only_one_private_forecast_draw() {
    for expansions in MODES {
        let mut game = new(expansions);
        board(&mut game, "B001");
        board(&mut game, "E001");
        operate(
            &mut game,
            Operation::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Up,
            },
        );
        // The fixture splay may have drawn a City; remove that preparation card.
        let hand = game.position().cards(Zone::Hand(A)).to_vec();
        for card in hand {
            let code = game.position().card(card).code;
            let deck = game.position().card(card).origin_deck();
            place(&mut game, code, Zone::Deck(deck));
        }
        let bell = board(&mut game, "E013");
        board(&mut game, "F022"); // Actual Archimedes: boxed [1]/[2] become [2]/[3].
        let held = place(&mut game, "E051", Zone::Forecast(A));
        place(&mut game, "B020", Zone::Hand(A));
        game.begin_turn(A).unwrap();
        let mut game = game.with_tracking().unwrap();
        assert_eq!(game.position().forecast_limit(A), 2);
        sampled_views(&game, B);
        let action = Action::Dogma {
            card: bell,
            auspice: false,
        };
        assert!(game.legal_actions().contains(&action));
        game.take_action(action, ActionKind::Regular).unwrap();
        let mut events = Vec::new();
        let Status::Decision(menu) = settle(&mut game, &mut events) else {
            panic!("Bell's optional echo");
        };
        assert_eq!(menu.source_card(A), Some(bell));
        let choice = decline(&menu);
        sampled_continue(&game, &menu, choice, false);
        game.choose(choice).unwrap();
        finish(&mut game, &mut events);
        let drawn: Vec<_> = moves(&events).filter(|m| m.verb.is_none()).collect();
        assert_eq!(drawn.len(), 2);
        assert_eq!(
            drawn.iter().map(|m| m.source).collect::<Vec<_>>(),
            [
                Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 2
                }),
                Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 3
                })
            ]
        );
        assert_eq!(source(&game, drawn[0].card), Zone::Forecast(A));
        assert_eq!(source(&game, drawn[1].card), Zone::Hand(A));
        assert_eq!(
            game.position().cards(Zone::Forecast(A)),
            [held, drawn[0].card]
        );
        assert_eq!(
            moves(&events)
                .filter(|m| m.verb == Some(Verb::Foreshadow))
                .count(),
            1
        );
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
        assert_eq!(
            game.position().special_location(Special::Destiny),
            SpecialLocation::Available
        );
        for zone in [Zone::Hand(A), Zone::Forecast(A)] {
            assert!(
                game.tracker(B)
                    .unwrap()
                    .belief(zone)
                    .iter()
                    .all(|slot| slot.known().is_none())
            );
        }
        assert_eq!(game.position().regular_actions_started(), 1);
        sampled_views(&game, B);
    }
}

#[test]
fn native_meld_promotes_bell_after_city_draw_and_runs_visible_echo_before_foreseen_return() {
    for expansions in MODES {
        let mut game = new(expansions);
        board(&mut game, "E034"); // Novel's bottom-right echo remains visible left.
        board(&mut game, "B013");
        operate(
            &mut game,
            Operation::Splay {
                player: A,
                color: Color::Purple,
                direction: Splay::Left,
            },
        );
        let prep_hand = game.position().cards(Zone::Hand(A)).to_vec();
        for card in prep_hand {
            let code = game.position().card(card).code;
            let deck = game.position().card(card).origin_deck();
            place(&mut game, code, Zone::Deck(deck));
        }
        board(&mut game, "F022");
        let bell = place(&mut game, "E013", Zone::Forecast(A));
        let waiting = place(&mut game, "E001", Zone::Forecast(A));
        assert_eq!(game.position().card(waiting).value, 1);
        let currency = place(&mut game, "B020", Zone::Hand(A));
        let ours = place(&mut game, "B003", Zone::Hand(A));
        let theirs = place(&mut game, "B002", Zone::Hand(B));
        game.begin_turn(A).unwrap();
        let mut game = game.with_tracking().unwrap();
        sampled_views(&game, B);
        let action = Action::Meld { card: currency };
        assert!(game.legal_actions().contains(&action));
        game.take_action(action, ActionKind::Regular).unwrap();
        let mut events = Vec::new();
        let Status::Decision(promotion_menu) = settle(&mut game, &mut events) else {
            panic!("choose among both eligible Forecast cards");
        };
        assert_eq!(promotion_menu.options.len(), 2);
        let promote_bell = promotion_menu.options.iter().position(|option| matches!(option,
            Alternative::Move { verb: Verb::Meld, target: TargetDescriptor::Known { card, source: Zone::Forecast(A) } } if *card == bell
        )).unwrap();
        assert!(promotion_menu.options.iter().any(|option| matches!(option,
            Alternative::Move { verb: Verb::Meld, target: TargetDescriptor::Known { card, source: Zone::Forecast(A) } } if *card == waiting
        )));
        // The remaining age-one card would also qualify for a recursive Bell
        // promotion. The effect meld must proceed to its free Dogma instead.
        sampled_continue(&game, &promotion_menu, promote_bell, true);
        game.choose(promote_bell).unwrap();
        let Status::Decision(menu) = settle(&mut game, &mut events) else {
            panic!("promoted Bell's optional echo");
        };
        assert_eq!(menu.source_card(A), Some(bell));
        assert_eq!(game.position().top(A, Color::Purple), Some(bell));
        assert!(
            game.position().cards(Zone::Forecast(A)).is_empty(),
            "Novel ran before Bell's echo"
        );
        let choice = decline(&menu);
        sampled_continue(&game, &menu, choice, true);
        game.choose(choice).unwrap();
        finish(&mut game, &mut events);
        let moved: Vec<_> = moves(&events).collect();
        let promotion = moved
            .iter()
            .position(|m| m.card == bell && m.verb == Some(Verb::Meld))
            .unwrap();
        let novel = moved
            .iter()
            .position(|m| m.card == waiting && m.verb == Some(Verb::Return))
            .unwrap();
        assert!(promotion < novel);
        let city_draws: Vec<_> = moved
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.verb.is_none()
                    && matches!(
                        m.source,
                        Zone::Deck(Deck {
                            set: CardSet::Cities,
                            age: 2
                        })
                    )
            })
            .collect();
        assert_eq!(city_draws.len(), usize::from(expansions.cities()));
        if let Some((index, city)) = city_draws.first() {
            assert!(*index < promotion, "new-color City draw precedes promotion");
            assert!(
                moved
                    .iter()
                    .any(|m| m.card == city.card && m.verb == Some(Verb::Return)),
                "foreseen Bell also returns that native City draw"
            );
        }
        let drawn: Vec<_> = moved
            .iter()
            .filter(|m| m.verb.is_none() && game.position().card(m.card).set != CardSet::Cities)
            .collect();
        assert_eq!(drawn.len(), 2);
        assert_eq!(
            drawn.iter().map(|m| m.source).collect::<Vec<_>>(),
            [Zone::Deck(Deck::base(2)), Zone::Deck(Deck::base(3))]
        );
        for card in [ours, theirs] {
            assert!(
                moved
                    .iter()
                    .any(|m| m.card == card && m.verb == Some(Verb::Return))
            );
        }
        assert!(game.position().cards(Zone::Hand(A)).is_empty());
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(
            game.position().cards(Zone::Forecast(A)),
            drawn.iter().map(|m| m.card).collect::<Vec<_>>()
        );
        for tracker in [game.tracker(B).unwrap(), game.public_tracker().unwrap()] {
            let forecast = tracker.belief(Zone::Forecast(A));
            assert_eq!(forecast.len(), 2);
            assert!(
                forecast.iter().all(|slot| slot.known().is_none()),
                "both drawn Forecast faces stay private: {forecast:?}"
            );
        }
        assert_eq!(
            moved.iter().filter(|m| m.verb == Some(Verb::Meld)).count(),
            2,
            "promotion does not chain"
        );
        assert_eq!(game.position().turn_owner(), A);
        assert_eq!(
            game.position().regular_actions_started(),
            1,
            "free Dogma consumes no regular slot"
        );
        assert_eq!(
            game.position().special_location(Special::Destiny),
            SpecialLocation::Available
        );
        sampled_views(&game, B);
    }
}
