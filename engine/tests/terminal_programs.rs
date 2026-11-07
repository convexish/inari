mod common;
use common::*;
use inno_engine::*;

fn add(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn native(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    let code = BASE_METADATA
        .iter()
        .find(|card| {
            card.color == color
                && card.value == value
                && !f.cards.iter().any(|used| used.code == card.code)
        })
        .unwrap()
        .code;
    add(f, code, zone)
}
fn invoke(game: &mut Game, card: CardId, effect: u16) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}
fn finish(game: &mut Game, events: &mut Vec<Event>, value: Option<u8>) -> Status {
    for _ in 0..10000 {
        // Every scheduler boundary remains clonable, including the rare boxed
        // sum/value state and the original operation's return continuation.
        let mut copy = game.clone();
        let mut copied_events = Vec::new();
        let begin = events.len();
        let status = game.advance(1, events).unwrap();
        assert_eq!(copy.advance(1, &mut copied_events).unwrap(), status);
        assert_eq!(copied_events, events[begin..]);
        assert_eq!(copy.position(), game.position());
        assert!(game.position().check_conservation());
        match status {
            Status::WorkLimit => {}
            Status::Decision(decision) => {
                let choice = value
                    .and_then(|v| {
                        decision
                            .options
                            .iter()
                            .position(|option| *option == Alternative::Value(v))
                    })
                    .unwrap_or(0);
                game.choose(choice).unwrap();
            }
            status => return status,
        }
    }
    panic!("test scenario did not terminate; WorkLimit is not a draw");
}

#[test]
fn fission_red_draw_sweeps_physical_ownership_and_keeps_turn_tail() {
    let mut f = Fixture::default();
    let fission = add(&mut f, "B087", Zone::Board(A, Color::Red));
    let buried = native(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
    let top = native(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue));
    let hand = native(&mut f, Color::Yellow, 1, Zone::Hand(A));
    let score = native(&mut f, Color::Green, 1, Zone::Score(B));
    let achievement = native(&mut f, Color::Purple, 1, Zone::Achievements(B));
    let supply = native(&mut f, Color::Purple, 2, Zone::AvailableAchievements);
    let spare = native(&mut f, Color::Green, 10, Zone::Deck(Deck::base(10)));
    let red = add(&mut f, "B097", Zone::Deck(Deck::base(10)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card: fission,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events, None),
        Status::Complete(_)
    ));
    for card in [fission, buried, top, hand, score, red] {
        assert!(game.position().cards(Zone::Junk).contains(&card));
    }
    assert!(
        game.position()
            .cards(Zone::Achievements(B))
            .contains(&achievement)
    );
    assert!(
        game.position()
            .cards(Zone::AvailableAchievements)
            .contains(&supply)
    );
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(10)))
            .contains(&spare)
    );
    assert_eq!(game.position().turn_owner(), A);
    assert_eq!(game.position().regular_actions_started(), 1);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events, None),
        Status::Complete(_)
    ));
    assert_eq!(game.position().turn_owner(), B);
    assert!(game.position().cards(Zone::Hand(A)).contains(&spare));
}

#[test]
fn fission_nested_in_a_meld_action_preserves_the_turn_tail() {
    for bits in 0..Expansions::COMBINATIONS as u8 {
        let expansions = Expansions::from_bits(bits).unwrap();
        if !expansions.figures() || !expansions.artifacts() {
            continue;
        }
        for (second_action, wins) in [(false, false), (true, false), (false, true)] {
            let mut f = Fixture::default();
            let sunshu = figure_card("F004").unwrap();
            let color = sunshu.color;
            let sunshu = f.add(sunshu, Zone::Board(A, color));
            let fission = add(&mut f, "B087", Zone::Board(A, Color::Red));
            let priest = f.add(artifacts_card("A007").unwrap(), Zone::Hand(A));
            let payment = native(&mut f, Color::Red, 1, Zone::Hand(A));
            let chain = native(&mut f, Color::Blue, 11, Zone::Deck(Deck::base(11)));
            let spare = native(&mut f, Color::Green, 10, Zone::Deck(Deck::base(10)));
            let red = add(&mut f, "B097", Zone::Deck(Deck::base(10)));
            let mut achievements = Vec::new();
            if wins {
                // The Chain award reaches the threshold. Fission must still run
                // the real action's victory check after junking the boards.
                for _ in 0..5 + expansions.count() {
                    let code = BASE_METADATA
                        .iter()
                        .find(|card| !f.cards.iter().any(|used| used.code == card.code))
                        .unwrap()
                        .code;
                    achievements.push(add(&mut f, code, Zone::Achievements(A)));
                }
            }
            achievements.push(chain);
            let mut game = Game::new(f.position(), expansions)
                .unwrap()
                .with_tracking()
                .unwrap();
            if second_action {
                game.begin_action(ActionKind::Regular).unwrap();
            }
            // Sunshu Ao replaces the Meld action with Priest-King's self-execution.
            // Scoring a red card then super-executes Fission, including its demand.
            game.take_action(Action::Meld { card: priest }, ActionKind::Regular)
                .unwrap();
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            let pick = menu
                .options
                .iter()
                .position(|option| {
                    matches!(option,
            Alternative::Move { verb: Verb::Score, target: TargetDescriptor::Known { card, .. } }
            if *card == payment)
                })
                .unwrap();
            game.choose(pick).unwrap();
            let status = finish(&mut game, &mut events, None);
            if wins {
                assert_eq!(status, Status::Winner(A));
                assert_eq!(game.victory().unwrap().reason, WinReason::Achievements);
            } else {
                assert!(
                    matches!(status, Status::Complete(_)),
                    "{status:?}\n{events:#?}"
                );
                assert_eq!(
                    game.position().turn_owner(),
                    if second_action { B } else { A }
                );
                assert!(!game.legal_actions().is_empty());
                assert_eq!(
                    game.position().regular_actions_started(),
                    u8::from(!second_action)
                );
            }
            for card in [sunshu, fission, priest, payment, red] {
                assert!(game.position().cards(Zone::Junk).contains(&card));
            }
            assert_eq!(game.position().cards(Zone::Achievements(A)), achievements);
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[spare]);
            for player in Player::ALL {
                assert!(game.tracker(player).unwrap().check_truth(game.position()));
            }
        }
    }
}

#[test]
fn fission_nondemand_excludes_native_fission_but_draws_without_return() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B087", Zone::Board(A, Color::Red));
    let drawn = native(&mut f, Color::Green, 10, Zone::Deck(Deck::base(10)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 1);
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events, None),
        Status::Complete(_)
    ));
    assert!(
        game.position()
            .cards(Zone::Board(A, Color::Red))
            .contains(&source)
    );
    assert!(game.position().cards(Zone::Hand(A)).contains(&drawn));
}

#[test]
fn fusion_uses_each_original_scored_value_not_initial_eleven() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B107", Zone::Board(A, Color::Red));
    let nine = native(&mut f, Color::Blue, 9, Zone::Board(A, Color::Blue));
    let seven = native(&mut f, Color::Green, 7, Zone::Board(A, Color::Green));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    let d = decision(&mut game, &mut events);
    assert_eq!(
        d.options,
        vec![Alternative::Value(10), Alternative::Value(9)]
    );
    game.choose(1).unwrap();
    let d = decision(&mut game, &mut events);
    assert_eq!(
        d.options,
        vec![Alternative::Value(8), Alternative::Value(7)]
    );
    game.choose(1).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events, None),
        Status::Complete(_)
    ));
    assert_eq!(
        game.position().cards(Zone::Score(A)),
        &[source, nine, seven]
    );
}

#[test]
fn reclamation_partial_sum_rounds_up_and_does_not_repeat() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B109", Zone::Board(A, Color::Yellow));
    let one = native(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let two = native(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    let drawn = native(&mut f, Color::Blue, 2, Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events, None),
        Status::Complete(_)
    ));
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(1)))
            .contains(&one)
    );
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(2)))
            .contains(&two)
    );
    assert!(
        game.position()
            .cards(Zone::Board(A, Color::Blue))
            .contains(&drawn)
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Return)))
            .count(),
        2
    );
}

#[test]
fn reclamation_repeats_original_drawn_color_and_recomputes_sum() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B109", Zone::Board(A, Color::Yellow));
    for code in ["B001", "B002", "B003"] {
        add(&mut f, code, Zone::Board(A, Color::Red));
    }
    let drawn = native(&mut f, Color::Blue, 2, Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events, None),
        Status::Complete(_)
    ));
    // Three red returns repeat blue. The original blue 2 is returned, then
    // half its value draws a returned red 1. One actual return ends the loop.
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(2)))
            .contains(&drawn)
    );
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)).len(), 1);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Return)))
            .count(),
        4
    );
}

#[test]
fn reclamation_empty_red_draws_age_zero_fallback_and_high_sum_exhausts() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B109", Zone::Board(A, Color::Yellow));
    let one = native(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    assert!(matches!(
        finish(&mut game, &mut Vec::new(), None),
        Status::Complete(_)
    ));
    assert!(
        game.position()
            .cards(Zone::Board(A, Color::Green))
            .contains(&one)
    );

    let mut f = Fixture::default();
    let source = add(&mut f, "B109", Zone::Board(A, Color::Yellow));
    for code in ["B087", "B097", "B106"] {
        add(&mut f, code, Zone::Board(A, Color::Red));
    }
    native(&mut f, Color::Purple, 1, Zone::Score(B));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    assert_eq!(finish(&mut game, &mut Vec::new(), None), Status::Winner(B));
}

#[test]
fn space_traffic_eliminates_ownership_without_jackie_victory_reaction() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B111", Zone::Board(A, Color::Green));
    let drawn = native(&mut f, Color::Green, 11, Zone::Deck(Deck::base(11)));
    let jackie = f.figure(
        Figure::JackieChan,
        Zone::Board(A, figure_card(Figure::JackieChan.code()).unwrap().color),
    );
    let hand = native(&mut f, Color::Red, 1, Zone::Hand(A));
    let achievement = native(&mut f, Color::Yellow, 1, Zone::Achievements(A));
    let opponent = native(&mut f, Color::Blue, 1, Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    // Jackie's genuine First-on-draw runs (there are no hand Figures to score),
    // but elimination must never dispatch his opponent-win reaction.
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events, None), Status::Winner(B));
    let victory = game.victory().unwrap();
    assert_eq!(victory.reason, WinReason::Elimination { loser: A });
    assert_eq!(victory.source_card(B), Some(source));
    for card in [source, drawn, jackie, hand, achievement] {
        assert!(game.position().cards(Zone::Junk).contains(&card));
    }
    assert!(game.position().cards(Zone::Hand(B)).contains(&opponent));
}

#[test]
fn space_traffic_tied_score_stops_and_scores_only_below_top_five() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B111", Zone::Board(A, Color::Green));
    let mut red = Vec::new();
    for age in 1..=5 {
        red.push(native(&mut f, Color::Red, age, Zone::Board(A, Color::Red)));
    }
    let drawn = native(&mut f, Color::Red, 11, Zone::Deck(Deck::base(11)));
    native(&mut f, Color::Blue, 11, Zone::Score(B));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    assert!(matches!(
        finish(&mut game, &mut Vec::new(), None),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Score(A)), &[drawn]);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), red);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Aslant);
}

#[test]
fn reclamation_lets_the_player_order_returns_that_share_a_deck() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B109", Zone::Board(A, Color::Yellow));
    let reds: Vec<_> = ["B001", "B002", "B003"]
        .into_iter()
        .map(|code| add(&mut f, code, Zone::Board(A, Color::Red)))
        .collect();
    let higher = native(&mut f, Color::Red, 4, Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    // All three bottom cards return to the Age 1 deck: each order is a
    // distinct deck-bottom order, so every one is offered.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3);
    choose_card(&mut game, &mut events, reds[2]);
    assert_eq!(decision(&mut game, &mut events).options.len(), 2);
    choose_card(&mut game, &mut events, reds[0]);
    finish(&mut game, &mut events, None);
    assert_eq!(
        &game.position().cards(Zone::Deck(Deck::base(1)))[..3],
        [reds[1], reds[0], reds[2]]
    );
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red))[0], higher);
}

#[test]
fn reclamation_does_not_order_returns_to_different_decks() {
    let mut f = Fixture::default();
    let source = add(&mut f, "B109", Zone::Board(A, Color::Yellow));
    let one = native(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let two = native(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    let three = native(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    native(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    // Different decks commute: no ordering decision before the draw.
    let first_returns: Vec<_> = events_until_decision(&mut game, &mut events);
    assert_eq!(first_returns, [one, two, three]);
}

fn events_until_decision(game: &mut Game, events: &mut Vec<Event>) -> Vec<CardId> {
    let _ = settle(game, events);
    events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb == Some(Verb::Return) => Some(m.card),
            _ => None,
        })
        .take(3)
        .collect()
}

/// Fission's sweep is an ordinary effect: a junk-reactive Figure it uncovers
/// (Rod Serling under a top card) reacts to the cards junked after him, so
/// the order stays a choice even though the sweep also empties a Safe.
#[test]
fn fission_keeps_its_sweep_order_while_a_buried_junk_karma_could_surface() {
    let mut f = Fixture::default();
    let fission = add(&mut f, "B087", Zone::Board(A, Color::Red));
    native(&mut f, Color::Yellow, 1, Zone::Hand(A));
    f.figure(Figure::RodSerling, Zone::Board(B, Color::Purple));
    f.tech(Color::Purple, 5, Zone::Board(B, Color::Purple), vec![]);
    f.tech(Color::Blue, 3, Zone::Safe(B), vec![]);
    add(&mut f, "B097", Zone::Deck(Deck::base(10)));
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::UNSEEN)).unwrap();
    game.take_action(
        Action::Dogma {
            card: fission,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B);
    assert!(menu.options.len() > 1);
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::Move {
            verb: Verb::Junk,
            ..
        }
    )));
}
