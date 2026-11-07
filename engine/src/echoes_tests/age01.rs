//! Remaining age-one clauses, public-back operands, and bounded shortcut oracles.
use super::*;
use crate::program::build;
use shortcuts::common::*;

const E: Expansions = Expansions::ECHOES;
const EF: Expansions = E.union(Expansions::FIGURES);

fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, vec![])
}
fn draw_card(f: &mut Fixture, color: Color, age: u8) -> CardId {
    tech(f, color, age, Zone::Deck(Deck::base(age)))
}
fn source(f: &mut Fixture, code: &str) -> CardId {
    let card = echoes_card(code).unwrap();
    let color = card.color;
    let other = Color::ALL.into_iter().find(|c| *c != color).unwrap();
    tech(f, other, 1, Zone::Board(A, other));
    f.add(card, Zone::Board(A, color))
}
fn start(f: Fixture, card: CardId, effect: u16, expansions: Expansions) -> Game {
    let mut game = Game::seeded(f.position(), expansions, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}
fn advance(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..10000 {
        let mut clone = game.clone();
        let mut actual = vec![];
        let mut expected = vec![];
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(clone.advance(1, &mut expected).unwrap(), status);
        assert_eq!(actual, expected);
        assert_eq!(*game, clone);
        events.extend(actual);
        if status != Status::WorkLimit {
            return status;
        }
    }
    panic!("age-one program did not settle");
}
fn menu(game: &mut Game, events: &mut Vec<Event>) -> Decision {
    let status = advance(game, events);
    let Status::Decision(menu) = status else {
        panic!("expected decision, got {status:?}")
    };
    menu
}
fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut pick: impl FnMut(&Decision) -> usize,
) -> Status {
    loop {
        match advance(game, events) {
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            status => {
                assert!(game.position.check_conservation());
                for p in Player::ALL {
                    if let Some(tracker) = game.tracker(p) {
                        assert!(tracker.check_truth(&game.position));
                    }
                }
                return status;
            }
        }
    }
}
fn offered(menu: &Decision) -> Vec<CardId> {
    menu.options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            }
            | Alternative::Select {
                target: TargetDescriptor::Known { card, .. },
            } => Some(*card),
            _ => None,
        })
        .collect()
}
fn pick_card(game: &mut Game, menu: &Decision, card: CardId) {
    let index = menu
        .options
        .iter()
        .position(|option| {
            matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
        | Alternative::Select { target: TargetDescriptor::Known { card: c, .. } } if *c == card)
        })
        .unwrap_or_else(|| panic!("card {card:?} absent from {menu:?}"));
    game.choose(index).unwrap();
}
fn decline(game: &mut Game, menu: &Decision) {
    game.choose(
        menu.options
            .iter()
            .position(|a| *a == Alternative::Decline)
            .unwrap(),
    )
    .unwrap();
}
fn color(game: &mut Game, events: &mut Vec<Event>, color: Color) {
    let menu = menu(game, events);
    game.choose(
        menu.options
            .iter()
            .position(|a| *a == Alternative::Colors(1 << color as u8))
            .unwrap(),
    )
    .unwrap();
}
fn reveals(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

#[test]
fn age_one_catalog_has_every_printed_clause_and_retains_layout_budgets() {
    let expected = [
        (true, 2, false),
        (true, 1, true),
        (true, 1, false),
        (true, 1, false),
        (false, 2, false),
        (false, 1, false),
        (false, 1, false),
        (false, 2, false),
        (true, 1, false),
        (false, 2, false),
        (true, 1, true),
        (true, 1, false),
        (true, 2, false),
        (true, 2, true),
        (false, 1, false),
    ];
    for (i, &(echo, count, demand)) in expected.iter().enumerate() {
        let code = format!("E{:03}", i + 1);
        let card = echoes_card(&code).unwrap();
        assert_eq!(card.echo.is_some(), echo, "{code}");
        assert_eq!(card.effects.len(), count, "{code}");
        assert_eq!(card.effects[0].demand(), demand, "{code}");
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn candles_echo_uses_effective_points_and_allows_a_tie() {
    for opposing in [0, 1, 2] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E002");
        if opposing > 0 {
            tech(&mut f, Color::Blue, opposing, Zone::Score(B));
        }
        let drawn = draw_card(&mut f, Color::Blue, 3);
        let mut game = start(f, card, Card::ECHO, E);
        assert_eq!(game.position.score(A), 1, "Candles' own bonus");
        finish(&mut game, &mut vec![], |_| 0);
        assert_eq!(
            game.position.locate(drawn) == Some(Zone::Hand(A)),
            opposing >= 1
        );
    }
    let mut f = Fixture::default();
    let card = source(&mut f, "E002");
    let napoleon = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
    f.add(napoleon.clone(), Zone::Board(A, napoleon.color));
    tech(&mut f, Color::Blue, 1, Zone::Score(A));
    tech(&mut f, Color::Blue, 6, Zone::Score(B));
    let drawn = draw_card(&mut f, Color::Blue, 3);
    let mut game = start(f, card, Card::ECHO, EF);
    assert!(game.position.score(A) > 6);
    finish(&mut game, &mut vec![], |_| 0);
    assert_eq!(game.position.locate(drawn), Some(Zone::Deck(Deck::base(3))));
}

#[test]
fn candles_demand_offers_either_icon_and_reveals_only_necessary_evidence() {
    for icons in [
        vec![Icon::Authority],
        vec![Icon::Concept],
        vec![Icon::Authority, Icon::Concept],
        vec![],
    ] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E002");
        let chosen = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        for (index, icon) in icons.iter().enumerate() {
            f.cards[chosen.0 as usize].symbols[index] = Symbol::Icon(*icon);
        }
        let other = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
        let drawn = draw_card(&mut f, Color::Purple, 1);
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(chosen),
            Some(if icons.is_empty() {
                Zone::Hand(A)
            } else {
                Zone::Hand(B)
            })
        );
        assert_eq!(
            game.position.locate(drawn) == Some(Zone::Hand(A)),
            !icons.is_empty()
        );
        assert_eq!(game.position.locate(other), Some(Zone::Hand(A)));
        assert_eq!(
            reveals(&events),
            if icons.is_empty() {
                vec![chosen, other]
            } else {
                vec![chosen]
            }
        );
        if !icons.is_empty() {
            assert!(
                game.tracker(B)
                    .unwrap()
                    .belief(Zone::Hand(A))
                    .iter()
                    .any(|s| s.known().is_none())
            );
        }
    }
}

#[test]
fn plumbing_echo_scores_the_exact_blue_bottom_including_a_singleton() {
    for count in 0..=3 {
        let mut f = Fixture::default();
        let card = f.add(echoes_card("E003").unwrap(), Zone::Board(A, Color::Red));
        let blue: Vec<_> = (0..count)
            .map(|i| tech(&mut f, Color::Blue, i + 1, Zone::Board(A, Color::Blue)))
            .collect();
        let mut game = start(f, card, Card::ECHO, E);
        finish(&mut game, &mut vec![], |_| 0);
        assert_eq!(
            game.position.cards(Zone::Score(A)),
            &blue[..usize::from(count > 0)]
        );
        assert_eq!(
            game.position.cards(Zone::Board(A, Color::Blue)),
            &blue[usize::from(count > 0)..]
        );
        assert_eq!(game.position.locate(card), Some(Zone::Board(A, Color::Red)));
    }
}

#[test]
fn chopsticks_echo_can_decline_and_draws_even_when_forecast_is_full() {
    for (full, accept) in [(false, false), (false, true), (true, true)] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E004");
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Purple, 2, Zone::Forecast(A));
            }
        }
        let drawn = draw_card(&mut f, Color::Blue, 1);
        let mut game = start(f, card, Card::ECHO, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        if accept {
            game.choose(0).unwrap();
        } else {
            decline(&mut game, &m);
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(drawn),
            Some(if !accept {
                Zone::Deck(Deck::base(1))
            } else if full {
                Zone::Hand(A)
            } else {
                Zone::Forecast(A)
            })
        );
    }
}

#[test]
fn chopsticks_requires_real_junking_then_checks_the_highest_before_eligibility() {
    for (deck, accept, highest) in [
        (false, true, 2),
        (true, false, 2),
        (true, true, 2),
        (true, true, 4),
    ] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E004");
        tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
        tech(&mut f, Color::Red, 10, Zone::Score(A));
        let low = tech(&mut f, Color::Green, 1, Zone::Junk);
        let high = tech(&mut f, Color::Red, highest, Zone::Junk);
        let swept = deck.then(|| draw_card(&mut f, Color::Purple, 1));
        let echo_deck = f.add(
            echoes_card("E005").unwrap(),
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 1,
            }),
        );
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        if deck {
            let m = menu(&mut game, &mut events);
            if accept {
                game.choose(0).unwrap()
            } else {
                decline(&mut game, &m)
            };
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(high) == Some(Zone::Achievements(A)),
            deck && accept && highest == 2
        );
        assert_eq!(game.position.locate(low), Some(Zone::Junk));
        if let Some(swept) = swept {
            assert_eq!(
                game.position.locate(swept),
                Some(if accept {
                    Zone::Junk
                } else {
                    Zone::Deck(Deck::base(1))
                })
            );
        }
        assert_eq!(
            game.position.locate(echo_deck),
            Some(Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 1
            }))
        );
        assert!(reveals(&events).is_empty());
    }
}

#[test]
fn noodles_count_is_strict_and_counts_backs_across_sets() {
    for (ours, theirs) in [(0, 0), (1, 1), (2, 1)] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E005");
        for i in 0..ours {
            let c = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
            if i == 1 {
                f.cards[c.0 as usize].set = CardSet::Echoes;
            }
        }
        for _ in 0..theirs {
            tech(&mut f, Color::Red, 1, Zone::Hand(B));
        }
        tech(&mut f, Color::Green, 3, Zone::Hand(A));
        let drawn = draw_card(&mut f, Color::Purple, 2);
        let mut game = start(f, card, 0, E);
        finish(&mut game, &mut vec![], |_| 0);
        assert_eq!(
            game.position.locate(drawn) == Some(Zone::Score(A)),
            ours > theirs
        );
    }
}

#[test]
fn noodles_reveals_before_scoring_the_live_hand_ones() {
    for (color, age) in [(Color::Yellow, 1), (Color::Blue, 1), (Color::Yellow, 2)] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E005");
        let old = tech(&mut f, Color::Green, 1, Zone::Hand(A));
        let other = tech(&mut f, Color::Red, 3, Zone::Hand(A));
        let drawn = draw_card(&mut f, color, age);
        let mut game = start(f, card, 1, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(reveals(&events), [drawn]);
        assert_eq!(
            game.position.locate(old) == Some(Zone::Score(A)),
            color == Color::Yellow
        );
        assert_eq!(
            game.position.locate(drawn) == Some(Zone::Score(A)),
            color == Color::Yellow && age == 1
        );
        assert_eq!(game.position.locate(other), Some(Zone::Hand(A)));
        if color == Color::Yellow {
            let reveal = events
                .iter()
                .position(|e| matches!(e, Event::Revealed { .. }))
                .unwrap();
            let score = events
                .iter()
                .position(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Score)))
                .unwrap();
            assert!(reveal < score);
        }
    }
}

#[test]
fn soap_keeps_tuck_order_and_checks_the_chosen_live_top_after_a_real_tuck() {
    for (opponent, take, reverse, accept) in [
        (0, true, false, true),
        (4, true, true, true),
        (5, true, false, true),
        (0, false, false, true),
        (0, true, false, false),
    ] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E006");
        let top = tech(&mut f, Color::Blue, 5, Zone::Board(A, Color::Blue));
        if opponent > 0 {
            tech(&mut f, Color::Blue, opponent, Zone::Board(B, Color::Blue));
        }
        tech(&mut f, Color::Red, 3, Zone::Score(A)); // Soap's two bonus points complete the cost.
        let first = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        let second = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
        let claim = tech(&mut f, Color::Green, 1, Zone::Hand(A));
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        color(&mut game, &mut events, Color::Blue);
        let m = menu(&mut game, &mut events);
        assert_eq!(offered(&m), [first, second]);
        if take {
            let order = if reverse {
                [second, first]
            } else {
                [first, second]
            };
            pick_card(&mut game, &m, order[0]);
            let m = menu(&mut game, &mut events);
            pick_card(&mut game, &m, order[1]);
            if opponent < 5 {
                let m = menu(&mut game, &mut events);
                assert_eq!(offered(&m), [claim]);
                if accept {
                    pick_card(&mut game, &m, claim)
                } else {
                    decline(&mut game, &m)
                };
            }
        } else {
            decline(&mut game, &m);
        }
        finish(&mut game, &mut events, |_| panic!("no extra choice"));
        assert_eq!(game.position.top(A, Color::Blue), Some(top));
        assert_eq!(
            game.position.locate(claim) == Some(Zone::Achievements(A)),
            take && opponent < 5 && accept
        );
        if take {
            let pile = game.position.cards(Zone::Board(A, Color::Blue));
            let expected = if reverse {
                [first, second]
            } else {
                [second, first]
            };
            assert_eq!(&pile[..2], &expected);
        }
    }
}

#[test]
fn comb_reveals_five_original_draws_before_returns_and_excludes_karma_draws() {
    for figure in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E007");
        let old = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
        let packet: Vec<_> = [
            Color::Red,
            Color::Blue,
            Color::Red,
            Color::Purple,
            Color::Red,
        ]
        .into_iter()
        .map(|c| draw_card(&mut f, c, 1))
        .collect();
        let extra = if figure {
            let printed = figure_card(Figure::MarieCurie.code()).unwrap();
            f.add(printed.clone(), Zone::Board(A, printed.color));
            Some(draw_card(&mut f, Color::Blue, 9))
        } else {
            None
        };
        let mut game = start(f, card, 0, if figure { EF } else { E });
        let mut events = vec![];
        color(&mut game, &mut events, Color::Red);
        finish(&mut game, &mut events, |_| 0);
        let mut disclosed = reveals(&events);
        disclosed.sort();
        let mut expected = packet.clone();
        expected.sort();
        assert_eq!(disclosed, expected);
        assert_eq!(game.position.locate(old), Some(Zone::Hand(A)));
        for &drawn in &packet {
            assert_eq!(
                game.position.locate(drawn),
                Some(if game.position.card(drawn).color == Color::Red {
                    Zone::Hand(A)
                } else {
                    Zone::Deck(Deck::base(1))
                })
            );
        }
        let first_return = events
            .iter()
            .position(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Return)))
            .unwrap();
        assert_eq!(reveals(&events[..first_return]).len(), 5);
        if let Some(extra) = extra {
            assert_eq!(game.position.locate(extra), Some(Zone::Hand(A)));
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e,Event::Moved(m) if m.card==extra && m.inside_karma))
            );
        }
    }
}

#[test]
fn comb_foreseen_returns_both_boards_at_every_depth_after_the_draw_packet() {
    let mut f = Fixture::default();
    let comb = f.add(echoes_card("E007").unwrap(), Zone::Forecast(A));
    let promotion = tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
    let red: Vec<_> = [A, A, B]
        .into_iter()
        .enumerate()
        .map(|(i, p)| tech(&mut f, Color::Red, i as u8 + 2, Zone::Board(p, Color::Red)))
        .collect();
    let untouched = tech(&mut f, Color::Blue, 3, Zone::Board(B, Color::Blue));
    for _ in 0..5 {
        draw_card(&mut f, Color::Red, 1);
    }
    let mut game = Game::seeded(f.position(), E, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
        .unwrap();
    let mut events = vec![];
    color(&mut game, &mut events, Color::Red);
    finish(&mut game, &mut events, |_| 0);
    for c in red {
        assert!(matches!(game.position.locate(c), Some(Zone::Deck(_))));
    }
    assert_eq!(
        game.position.locate(untouched),
        Some(Zone::Board(B, Color::Blue))
    );
    assert_eq!(
        game.position.locate(comb),
        Some(Zone::Board(A, Color::Green))
    );
    let first_return = events
        .iter()
        .position(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Return)))
        .unwrap();
    assert_eq!(reveals(&events[..first_return]).len(), 5);
}

#[test]
fn umbrella_echo_is_optional_and_does_not_promote_forecast() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E009");
        let melded = tech(&mut f, Color::Blue, 4, Zone::Hand(A));
        let forecast = f.add(echoes_card("E001").unwrap(), Zone::Forecast(A));
        let mut game = start(f, card, Card::ECHO, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        if accept {
            pick_card(&mut game, &m, melded)
        } else {
            decline(&mut game, &m)
        };
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(melded),
            Some(if accept {
                Zone::Board(A, Color::Blue)
            } else {
                Zone::Hand(A)
            })
        );
        assert_eq!(game.position.locate(forecast), Some(Zone::Forecast(A)));
    }
}

#[test]
fn umbrella_scores_twice_actual_returns_only_after_returning_finishes() {
    for returned in 0..=3 {
        let mut f = Fixture::default();
        let card = source(&mut f, "E009");
        let cards: Vec<_> = (1..=4)
            .map(|age| tech(&mut f, Color::Purple, age, Zone::Hand(A)))
            .collect();
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        for &c in cards.iter().take(returned) {
            let m = menu(&mut game, &mut events);
            pick_card(&mut game, &m, c);
        }
        let m = menu(&mut game, &mut events);
        decline(&mut game, &m);
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.cards(Zone::Score(A)).len(),
            (2 * returned).min(4 - returned)
        );
        let verbs: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) => m.verb,
                _ => None,
            })
            .collect();
        assert!(verbs.iter().take(returned).all(|v| *v == Verb::Return));
        assert!(verbs.iter().skip(returned).all(|v| *v == Verb::Score));
    }
}

#[test]
fn perfume_demand_excludes_every_demander_value_and_retains_transfer_value() {
    for absent in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E011");
        let same = tech(&mut f, Color::Yellow, 2, Zone::Board(A, Color::Yellow));
        let different = tech(&mut f, Color::Purple, 3, Zone::Board(A, Color::Purple));
        tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
        if !absent {
            tech(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
            tech(&mut f, Color::Green, 2, Zone::Board(B, Color::Green));
        }
        let drawn = draw_card(&mut f, Color::Red, 3);
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        assert_eq!(offered(&m).contains(&same), absent);
        assert!(offered(&m).contains(&different));
        pick_card(&mut game, &m, different);
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(different),
            Some(Zone::Board(B, Color::Purple))
        );
        assert_eq!(
            game.position.locate(drawn),
            Some(Zone::Board(A, Color::Red))
        );
        assert_eq!(
            game.position.locate(same),
            Some(Zone::Board(A, Color::Yellow))
        );
    }
}

#[test]
fn perfume_echo_repeats_on_its_original_draw_and_reveals_if_tuck_is_replaced() {
    for replaced in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E011");
        let stopping = draw_card(&mut f, Color::Yellow, 1);
        let repeating = draw_card(&mut f, Color::Green, 1);
        f.cards[repeating.0 as usize].symbols[0] = Symbol::Icon(Icon::Authority);
        let mut game = start(f, card, Card::ECHO, E);
        let mut events = vec![];
        if replaced {
            for _ in 0..100 {
                if let Some(Frame::Operation {
                    request:
                        Operation::Move {
                            verb: Verb::Tuck,
                            target,
                            ..
                        },
                    phase,
                    ..
                }) = game.frames.last_mut()
                    && target.card == repeating
                    && *phase == Phase::Discover
                {
                    *phase = Phase::Replaced;
                    break;
                }
                assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
            }
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(stopping),
            Some(Zone::Board(A, Color::Yellow))
        );
        assert_eq!(
            game.position.locate(repeating),
            Some(if replaced {
                Zone::Hand(A)
            } else {
                Zone::Board(A, Color::Green)
            })
        );
        assert_eq!(
            reveals(&events),
            if replaced { vec![repeating] } else { vec![] }
        );
    }
}

#[test]
fn puppet_uses_score_card_values_and_native_private_back_choices() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E015");
    let score = tech(&mut f, Color::Red, 2, Zone::Score(A));
    f.cards[score.0 as usize].set = CardSet::Echoes;
    let base: Vec<_> = (0..2)
        .map(|_| tech(&mut f, Color::Blue, 2, Zone::AvailableAchievements))
        .collect();
    let echo = f.add(echoes_card("E016").unwrap(), Zone::AvailableAchievements);
    let wrong = tech(&mut f, Color::Green, 3, Zone::AvailableAchievements);
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    let expected = vec![
        Alternative::BlindSelect {
            source: Zone::AvailableAchievements,
            back: Deck::base(2),
        },
        Alternative::BlindSelect {
            source: Zone::AvailableAchievements,
            back: Deck {
                set: CardSet::Echoes,
                age: 2,
            },
        },
    ];
    assert_eq!(menu(&mut game, &mut events).options, expected);
    for seed in 0..16 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            assert_eq!(menu(&mut world, &mut vec![]).options, expected);
            let candidates: Vec<_> = world
                .position
                .cards(Zone::AvailableAchievements)
                .iter()
                .copied()
                .filter(|c| world.position.card(*c).origin_deck() == Deck::base(2))
                .collect();
            let mut rng = world.rng.clone();
            let selected = candidates[(rng.next_u64() % 2) as usize];
            world.choose(0).unwrap();
            let mut events = vec![];
            finish(&mut world, &mut events, |_| 0);
            assert_eq!(world.rng, rng);
            assert_eq!(world.position.locate(selected), Some(Zone::Junk));
            assert!(reveals(&events).is_empty());
            assert_eq!(
                world.position.locate(wrong),
                Some(Zone::AvailableAchievements)
            );
        }
    }
    game.choose(1).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(echo), Some(Zone::Junk));
    assert!(
        base.iter()
            .all(|c| game.position.locate(*c) == Some(Zone::AvailableAchievements))
    );
}

#[test]
fn value_membership_is_back_only_but_tracks_nested_operand_front_reads() {
    use build::{HAND, SCORE, filtered};
    assert!(!Test::ValueIn(&SCORE).reads_front());
    assert!(!Cond::Card(Reg::Bound, Filter::Has(Test::ValueIn(&SCORE))).reads_card_front());
    const FRONT_SOURCE: Selector = filtered(
        &Selector::Reg(Reg::Bound),
        &[Filter::Has(Test::Icon(Icon::Authority))],
    );
    assert!(Cond::Card(Reg::Last, Filter::Has(Test::ValueIn(&FRONT_SOURCE))).reads_card_front());
    let mut f = Fixture::default();
    let candidate = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
    let witness = tech(&mut f, Color::Red, 2, Zone::Score(A));
    f.cards[witness.0 as usize].set = CardSet::Echoes;
    let duplicate = tech(&mut f, Color::Green, 2, Zone::Score(A));
    tech(&mut f, Color::Purple, 2, Zone::Hand(B));
    let mut game = Game::new(f.position(), E).unwrap();
    let query = filtered(&HAND, &[Filter::Has(Test::ValueIn(&SCORE))]);
    assert_eq!(
        game.select(query, Context::new(A), Regs::EMPTY, Excluded::default())
            .unwrap()
            .iter()
            .map(|t| t.card)
            .collect::<Vec<_>>(),
        [candidate]
    );
    for c in [duplicate, witness] {
        game.position.zone_mut(Zone::Score(A)).retain(|x| *x != c);
        game.position.zone_mut(Zone::Junk).push(c);
        assert_eq!(
            game.select(query, Context::new(A), Regs::EMPTY, Excluded::default())
                .unwrap()
                .len(),
            usize::from(c == duplicate)
        );
    }
    assert_eq!(
        game.select(
            Selector::Junk,
            Context::new(A),
            Regs::EMPTY,
            Excluded::default()
        )
        .unwrap()
        .len(),
        2
    );
    const PRIVATE_SOURCE: Selector = filtered(
        &Selector::Hand(Who::Opponent),
        &[Filter::Has(Test::Icon(Icon::Authority))],
    );
    let private_query = filtered(&HAND, &[Filter::Has(Test::ValueIn(&PRIVATE_SOURCE))]);
    assert!(
        matches!(
            game.select(
                private_query,
                Context::new(A),
                Regs::EMPTY,
                Excluded::default()
            ),
            Err(Error::Unsupported(_))
        ),
        "back-only membership must retain the source's front-inspection checks"
    );
    assert!(game.position.check_conservation());
}

fn figure(f: &mut Fixture, which: Figure, player: Player) -> CardId {
    let printed = figure_card(which.code()).unwrap();
    let color = printed.color;
    f.add(printed, Zone::Board(player, color))
}

#[derive(Debug, PartialEq)]
struct Outcome {
    position: Position,
    rng: Rng,
    status: Status,
    events: Vec<String>,
    views: [crate::observation::Observation; 2],
    beliefs: [Vec<Vec<crate::tracking::BeliefSlot>>; 3],
}
struct Path {
    outcome: Outcome,
    chronological: Vec<Event>,
}
fn paths(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<Path> {
    assert!(depth < 24, "bounded age-one oracle");
    match advance(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut child = game.clone();
                child.choose(choice).unwrap();
                paths(child, events.clone(), depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            for p in Player::ALL {
                assert!(game.tracker(p).unwrap().check_truth(&game.position));
            }
            let views = Player::ALL.map(|player| game.observe(player).unwrap());
            let beliefs = [
                game.tracker(A).unwrap(),
                game.tracker(B).unwrap(),
                game.public_tracker().unwrap(),
            ]
            .map(|tracker| {
                crate::tracking::zones()
                    .into_iter()
                    .map(|zone| tracker.belief(zone))
                    .collect()
            });
            game.position.sort_unordered_piles();
            let mut bag: Vec<_> = events.iter().map(|e| format!("{e:?}")).collect();
            bag.sort();
            vec![Path {
                outcome: Outcome {
                    position: game.position,
                    rng: game.rng,
                    status,
                    events: bag,
                    views,
                    beliefs,
                },
                chronological: events,
            }]
        }
    }
}
fn oracle(game: Game, reduces: bool) -> Vec<Path> {
    compare_paths(game, Some(reduces))
}
fn compare_paths(game: Game, reduces: Option<bool>) -> Vec<Path> {
    let mut full = game.clone();
    full.shortcuts_disabled = true;
    let reduced = paths(game, vec![], 0);
    let full = paths(full, vec![], 0);
    if reduces == Some(true) {
        assert!(
            reduced.len() < full.len(),
            "expected reduction: {} versus {}",
            reduced.len(),
            full.len()
        );
    } else if reduces == Some(false) {
        assert_eq!(
            reduced.len(),
            full.len(),
            "reaction must preserve the order menu"
        );
    }
    for path in &full {
        assert!(
            reduced.iter().any(|r| r.outcome == path.outcome),
            "lost outcome {:#?}",
            path.outcome
        );
    }
    for path in &reduced {
        assert!(
            full.iter()
                .any(|f| f.outcome == path.outcome && f.chronological == path.chronological),
            "canonical path must occur unchanged in unrestricted play"
        );
    }
    reduced
}

#[test]
fn plumbing_deck_top_oracle_preserves_every_path_and_a_real_junk_replacement() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E003");
        for color in [Color::Blue, Color::Yellow] {
            draw_card(&mut f, color, 1);
        }
        if reacting {
            figure(&mut f, Figure::RodSerling, B);
        }
        let leaves = oracle(start(f, card, 0, if reacting { EF } else { E }), !reacting);
        assert!(leaves.iter().all(|p| if reacting {
            p.outcome.status == Status::Winner(B)
        } else {
            matches!(p.outcome.status, Status::Complete(_))
        }));
    }
}

#[test]
fn chopsticks_optional_deck_top_oracle_keeps_acceptance_and_a_real_junk_replacement() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E004");
        for color in [Color::Blue, Color::Yellow, Color::Purple] {
            draw_card(&mut f, color, 1);
        }
        tech(&mut f, Color::Red, 10, Zone::Score(A));
        tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
        tech(&mut f, Color::Green, 2, Zone::Junk);
        if reacting {
            figure(&mut f, Figure::RodSerling, B);
        }
        let leaves = oracle(start(f, card, 0, if reacting { EF } else { E }), !reacting);
        assert!(
            leaves
                .iter()
                .any(|p| matches!(p.outcome.status, Status::Complete(_)))
        );
        if reacting {
            assert!(leaves.iter().any(|p| p.outcome.status == Status::Winner(B)));
        } else {
            assert!(
                leaves
                    .iter()
                    .any(|p| p.outcome.position.cards(Zone::Achievements(A)).len() == 1)
            );
        }
    }
}

#[test]
fn noodles_score_all_oracle_preserves_every_path_and_sunshu_aos_sixth_score() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E005");
        tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        tech(&mut f, Color::Green, 1, Zone::Hand(A));
        let extra = draw_card(&mut f, Color::Purple, 1);
        draw_card(&mut f, Color::Yellow, 1);
        if reacting {
            figure(&mut f, Figure::SunshuAo, A);
        }
        let mut game = start(f, card, 1, if reacting { EF } else { E });
        game.position.turn_moves[A.index()][1] = 5;
        let leaves = oracle(game, !reacting);
        for leaf in leaves {
            assert_eq!(leaf.outcome.position.cards(Zone::Score(A)).len(), 3);
            if reacting {
                assert!(leaf.chronological.iter().any(|e| matches!(e, Event::Moved(m) if m.card == extra && m.verb == Some(Verb::Achieve) && m.inside_karma)));
            }
        }
    }
}

#[test]
fn comb_packet_return_oracle_preserves_every_path_and_maxwell_draws() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E007");
        for _ in 0..3 {
            draw_card(&mut f, Color::Red, 1);
        }
        draw_card(&mut f, Color::Blue, 2);
        draw_card(&mut f, Color::Purple, 3);
        if reacting {
            figure(&mut f, Figure::JamesClerkMaxwell, B);
            for _ in 0..2 {
                draw_card(&mut f, Color::Blue, 8);
            }
        }
        let mut game = start(f, card, 0, if reacting { EF } else { E });
        color(&mut game, &mut vec![], Color::Red);
        let leaves = oracle(game, !reacting);
        for leaf in leaves {
            assert_eq!(leaf.outcome.position.cards(Zone::Hand(A)).len(), 3);
            if reacting {
                assert_eq!(
                    leaf.chronological
                        .iter()
                        .filter(
                            |e| matches!(e, Event::Moved(m) if m.inside_karma && m.verb.is_none())
                        )
                        .count(),
                    2
                );
            }
        }
    }
}

#[test]
fn comb_foreseen_board_return_oracle_preserves_every_path_and_maxwell_draws() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        f.add(echoes_card("E007").unwrap(), Zone::Forecast(A));
        let promotion = tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
        for (player, age) in [(A, 2), (A, 3), (B, 4)] {
            tech(&mut f, Color::Red, age, Zone::Board(player, Color::Red));
        }
        for _ in 0..5 {
            draw_card(&mut f, Color::Red, 1);
        }
        if reacting {
            figure(&mut f, Figure::JamesClerkMaxwell, B);
            for _ in 0..3 {
                draw_card(&mut f, Color::Blue, 8);
            }
        }
        let mut game = Game::seeded(f.position(), if reacting { EF } else { E }, 53)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
            .unwrap();
        color(&mut game, &mut vec![], Color::Red);
        let leaves = oracle(game, !reacting);
        for leaf in leaves {
            assert!(Player::ALL.into_iter().all(|p| {
                leaf.outcome
                    .position
                    .cards(Zone::Board(p, Color::Red))
                    .is_empty()
            }));
            if reacting {
                assert_eq!(
                    leaf.chronological
                        .iter()
                        .filter(
                            |e| matches!(e, Event::Moved(m) if m.inside_karma && m.verb.is_none())
                        )
                        .count(),
                    3
                );
            }
        }
    }
}

#[test]
fn umbrella_subset_and_score_quota_oracle_preserves_every_path_and_maxwell_arrivals() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E009");
        for age in [1, 2, 2] {
            tech(&mut f, Color::Purple, age, Zone::Hand(A));
        }
        if reacting {
            figure(&mut f, Figure::JamesClerkMaxwell, B);
            for _ in 0..3 {
                draw_card(&mut f, Color::Blue, 8);
            }
        }
        let leaves = oracle(start(f, card, 0, if reacting { EF } else { E }), true);
        // Without Maxwell, cross-deck return interleavings and full-score
        // permutations both reduce. With Maxwell, only scoring commutes.
        assert_eq!(leaves.len(), if reacting { 16 } else { 10 });
        if reacting {
            assert!(leaves.iter().any(|leaf| {
                leaf.chronological
                    .iter()
                    .any(|e| matches!(e, Event::Moved(m) if m.inside_karma && m.verb.is_none()))
            }));
        } else {
            assert!(
                leaves
                    .iter()
                    .any(|leaf| leaf.outcome.position.cards(Zone::Hand(A)).len() == 3)
            );
            let ordered: Vec<_> = leaves
                .iter()
                .filter_map(|leaf| {
                    let deck = leaf.outcome.position.cards(Zone::Deck(Deck::base(2)));
                    (deck.len() == 2).then(|| deck.to_vec())
                })
                .collect();
            assert!(
                ordered
                    .iter()
                    .any(|a| ordered.iter().any(|b| a[0] == b[1] && a[1] == b[0])),
                "same-deck return order remains meaningful"
            );
        }
    }
}

#[test]
fn candles_meiji_aliases_need_no_disclosure_and_inapplicable_transfers_earn_no_reward() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E002");
    figure(&mut f, Figure::EmperorMeiji, A);
    let own = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
    let borrowed = tech(&mut f, Color::Purple, 1, Zone::Hand(B));
    f.cards[borrowed.0 as usize].symbols[0] = Symbol::Icon(Icon::Concept);
    let drawn = draw_card(&mut f, Color::Blue, 1);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    assert!(reveals(&events).is_empty());
    assert_eq!(game.position.locate(own), Some(Zone::Hand(A)));
    assert_eq!(game.position.locate(borrowed), Some(Zone::Hand(B)));
    assert_eq!(
        game.position.locate(drawn),
        Some(Zone::Deck(Deck::base(1))),
        "an inapplicable original transfer earns no reward"
    );
}

#[test]
fn soap_and_umbrella_count_original_successes_after_napoleon_replaces_a_move() {
    for code in ["E006", "E009"] {
        let mut f = Fixture::default();
        let card = source(&mut f, code);
        let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
        tech(&mut f, Color::Blue, 5, Zone::Board(A, Color::Blue));
        let first = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        let second = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
        let claim = tech(&mut f, Color::Green, 1, Zone::Hand(A));
        let mut game = start(f, card, 0, EF);
        let mut events = vec![];
        if code == "E006" {
            color(&mut game, &mut events, Color::Blue);
        }
        let m = menu(&mut game, &mut events);
        pick_card(&mut game, &m, first);
        let m = menu(&mut game, &mut events);
        pick_card(&mut game, &m, napoleon);
        let m = menu(&mut game, &mut events);
        if code == "E006" {
            pick_card(&mut game, &m, second);
        } else {
            decline(&mut game, &m);
        }
        finish(&mut game, &mut events, |_| {
            panic!("a replaced attempt does not grant the reward")
        });
        assert_eq!(
            game.position.locate(napoleon),
            Some(Zone::Score(A)),
            "Napoleon actually reacts and scores itself"
        );
        assert_eq!(game.position.locate(claim), Some(Zone::Hand(A)));
        assert_eq!(game.position.cards(Zone::Achievements(A)).len(), 0);
        assert_eq!(game.position.cards(Zone::Score(A)), [napoleon]);
        assert!(events.iter().any(|e|matches!(e,Event::Moved(m) if m.card==first && m.verb==Some(Verb::Meld) && m.inside_karma)));
    }
}

#[test]
fn perfume_repeat_recomputes_echoes_routing_after_the_first_tuck() {
    let mut f = Fixture::default();
    let card = f.add(echoes_card("E011").unwrap(), Zone::Board(A, Color::Blue));
    let repeated = tech(
        &mut f,
        Color::Green,
        1,
        Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 1,
        }),
    );
    f.cards[repeated.0 as usize].set = CardSet::Echoes;
    f.cards[repeated.0 as usize].symbols[0] = Symbol::Icon(Icon::Authority);
    let stopped = draw_card(&mut f, Color::Yellow, 1);
    let mut game = start(f, card, Card::ECHO, E);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    let draws: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb.is_none() => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(draws, [repeated, stopped]);
    assert_eq!(
        game.position.locate(repeated),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(
        game.position.locate(stopped),
        Some(Zone::Board(A, Color::Yellow))
    );
}

#[test]
fn puppet_uses_logical_score_membership_and_keeps_the_chosen_reference_through_sampling() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E015");
    figure(&mut f, Figure::WernherVonBraun, A);
    tech(&mut f, Color::Blue, 2, Zone::Junk);
    for _ in 0..3 {
        tech(&mut f, Color::Green, 2, Zone::AvailableAchievements);
    }
    f.add(echoes_card("E016").unwrap(), Zone::AvailableAchievements);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    assert!(game.position.cards(Zone::Score(A)).is_empty());
    let m = menu(&mut game, &mut events);
    assert_eq!(m.options.len(), 2);
    assert_eq!(
        m.options[0],
        Alternative::BlindSelect {
            source: Zone::AvailableAchievements,
            back: Deck::base(2)
        }
    );
    let chosen = |g: &Game| {
        g.frames
            .iter()
            .find_map(|frame| match frame {
                Frame::Program {
                    chosen: SelectionParameter::Card(reference),
                    ..
                } => Some(*reference),
                _ => None,
            })
            .unwrap()
    };
    let pending = |g: &Game| {
        g.frames
            .iter()
            .find_map(|frame| match frame {
                Frame::Operation {
                    request:
                        Operation::Move {
                            verb: Verb::Junk,
                            target,
                            ..
                        },
                    knowledge: Some(knowledge),
                    ..
                } => Some(Captured::new(target.card, knowledge.evidence)),
                _ => None,
            })
            .unwrap()
    };
    for seed in 0..16 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
            game.determinize(A, seed)
                .unwrap()
                .determinize(A, seed + 31)
                .unwrap(),
        ] {
            assert_eq!(menu(&mut world, &mut vec![]), m);
            world.choose(0).unwrap();
            let mut events = vec![];
            for _ in 0..100 {
                if matches!(
                    world.frames.last(),
                    Some(Frame::Operation {
                        request: Operation::Move {
                            verb: Verb::Junk,
                            ..
                        },
                        phase: Phase::Discover,
                        ..
                    })
                ) {
                    break;
                }
                assert_eq!(world.advance(1, &mut events).unwrap(), Status::WorkLimit);
            }
            assert_eq!(
                chosen(&world),
                pending(&world),
                "the move reuses the selected pick, including before any First reaction"
            );
            let selected = chosen(&world).inner;
            finish(&mut world, &mut events, |_| {
                panic!("junk cannot make a second choice")
            });
            assert_eq!(world.position.locate(selected), Some(Zone::Junk));
            assert!(reveals(&events).is_empty());
        }
    }
}

#[test]
fn printed_boxed_values_are_adjusted_by_archimedes_in_draws_counts_and_deck_sweeps() {
    for code in ["E002", "E003", "E005"] {
        let mut f = Fixture::default();
        let card = source(&mut f, code);
        figure(&mut f, Figure::Archimedes, A);
        let one = draw_card(&mut f, Color::Red, 1);
        let two = draw_card(&mut f, Color::Blue, 2);
        let three = draw_card(&mut f, Color::Purple, 3);
        let four = draw_card(&mut f, Color::Green, 4);
        tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
        let (effect, expected, zone) = match code {
            "E002" => {
                tech(&mut f, Color::Yellow, 10, Zone::Score(B));
                (Card::ECHO, four, Zone::Hand(A))
            }
            "E003" => (0, two, Zone::Junk),
            _ => (0, three, Zone::Score(A)),
        };
        let mut game = start(f, card, effect, EF);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.locate(expected), Some(zone));
        assert_eq!(game.position.locate(one), Some(Zone::Deck(Deck::base(1))));
    }
}

#[test]
fn chopsticks_junk_achievement_ties_stay_blind_through_hedys_first_splay() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E004");
    figure(&mut f, Figure::HedyLamarr, A);
    for _ in 0..2 {
        tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
    }
    for _ in 0..2 {
        tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    }
    tech(&mut f, Color::Yellow, 10, Zone::Score(A));
    for _ in 0..2 {
        tech(&mut f, Color::Blue, 2, Zone::Junk);
    }
    let echo = f.add(echoes_card("E016").unwrap(), Zone::Junk);
    draw_card(&mut f, Color::Purple, 1);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    menu(&mut game, &mut events);
    game.choose(0).unwrap();
    let m = menu(&mut game, &mut events);
    assert_eq!(m.options.len(), 2);
    assert!(m.options.iter().all(|a| matches!(
        a,
        Alternative::BlindMove {
            verb: Verb::Achieve,
            source: Zone::Junk,
            ..
        }
    )));
    let index = m
        .options
        .iter()
        .position(|a| matches!(a,Alternative::BlindMove{back, ..} if *back==Deck::base(2)))
        .unwrap();
    let mut rng = game.rng.clone();
    rng.next_u64();
    game.choose(index).unwrap();
    let pending_menu = menu(&mut game, &mut events);
    assert_eq!(game.rng, rng);
    assert!(
        pending_menu
            .options
            .iter()
            .all(|a| matches!(a, Alternative::Splay { .. }))
    );
    assert!(reveals(&events).is_empty());
    for player in [A] {
        for seed in 0..12 {
            for mut world in [
                game.determinize(player, seed).unwrap(),
                game.sample_world(player, seed).unwrap().world,
            ] {
                assert_eq!(menu(&mut world, &mut vec![]), pending_menu);
                let selected = world
                    .frames
                    .iter()
                    .find_map(|frame| match frame {
                        Frame::Operation {
                            request: Operation::Claim { target, .. },
                            ..
                        } => Some(target.card),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(world.position.card(selected).origin_deck(), Deck::base(2));
                assert_eq!(world.position.locate(selected), Some(Zone::Junk));
                let mut events = vec![];
                world.choose(0).unwrap();
                finish(&mut world, &mut events, |_| 0);
                assert_eq!(world.position.locate(selected), Some(Zone::Achievements(A)));
                assert_eq!(world.position.locate(echo), Some(Zone::Junk));
                assert!(reveals(&events).is_empty());
                assert!(
                    events.iter().any(|e| matches!(e, Event::Splayed { .. })),
                    "Hedy actually changes a splay"
                );
            }
        }
    }
}

#[test]
fn comb_foreseen_return_oracle_retains_orders_that_expose_a_covered_maxwell() {
    let mut f = Fixture::default();
    let comb = f.add(echoes_card("E007").unwrap(), Zone::Forecast(A));
    let promotion = tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
    let maxwell = figure(&mut f, Figure::JamesClerkMaxwell, B);
    tech(&mut f, Color::Blue, 9, Zone::Board(B, Color::Blue));
    for age in [2, 3] {
        tech(&mut f, Color::Blue, age, Zone::Board(A, Color::Blue));
    }
    for _ in 0..5 {
        draw_card(&mut f, Color::Blue, 1);
    }
    let extras: Vec<_> = (0..3).map(|_| draw_card(&mut f, Color::Blue, 8)).collect();
    let mut game = Game::seeded(f.position(), EF, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
        .unwrap();
    color(&mut game, &mut vec![], Color::Blue);
    let leaves = compare_paths(game, None);
    assert!(leaves.iter().all(|leaf| leaf.outcome.position.locate(comb)
        == Some(Zone::Board(A, Color::Green))
        && matches!(leaf.outcome.position.locate(maxwell), Some(Zone::Deck(_)))));
    assert!(leaves.iter().any(|leaf| {
        leaf.chronological
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if extras.contains(&m.card) && m.inside_karma))
    }));
    assert!(leaves.iter().any(|leaf| {
        extras
            .iter()
            .all(|c| leaf.outcome.position.locate(*c) == Some(Zone::Deck(Deck::base(8))))
    }));
}

#[test]
fn chopsticks_newly_junked_same_back_claim_oracle() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E004");
    let candidates = [
        draw_card(&mut f, Color::Blue, 1),
        draw_card(&mut f, Color::Yellow, 1),
    ];
    tech(&mut f, Color::Red, 5, Zone::Score(A));
    let initial = start(f, card, 0, E);
    // For two candidates the native modulo classes have equal probability.
    // One representative of each class exhausts its complete probability law;
    // the chosen identity must not be pinned to its original physical order.
    let seeds: [u64; 2] = std::array::from_fn(|index| {
        (0..100)
            .find(|&seed| Rng::new(seed).next_u64() % 2 == index as u64)
            .unwrap()
    });
    let mut distributions: [Vec<Path>; 2] = std::array::from_fn(|_| vec![]);
    for (disabled, distribution) in distributions.iter_mut().enumerate() {
        for seed in seeds {
            let mut game = initial.clone();
            game.shortcuts_disabled = disabled == 1;
            game.rng = Rng::new(seed);
            let leaves = paths(game, vec![], 0);
            assert_eq!(
                leaves.len(),
                if disabled == 1 { 3 } else { 2 },
                "every accept/decline and hidden junk interleaving"
            );
            for mut leaf in leaves {
                let claimed = leaf.outcome.position.cards(Zone::Achievements(A));
                let mut expected_rng = Rng::new(seed);
                if !claimed.is_empty() {
                    expected_rng.next_u64();
                }
                assert_eq!(
                    leaf.outcome.rng, expected_rng,
                    "exact native RNG advancement on every path"
                );
                // RNG advancement is proved above. Compare the native law
                // across both classes, not a false same-index/identity pairing.
                leaf.outcome.rng = Rng::new(0);
                distribution.push(leaf);
            }
        }
    }
    for accepted in [false, true] {
        let conditional: Vec<Vec<_>> = distributions
            .iter()
            .map(|leaves| {
                leaves
                    .iter()
                    .filter(|leaf| {
                        leaf.outcome
                            .position
                            .cards(Zone::Achievements(A))
                            .is_empty()
                            != accepted
                    })
                    .collect()
            })
            .collect();
        let reduced = &conditional[0];
        let full = &conditional[1];
        assert_eq!(reduced.len(), 2);
        assert_eq!(full.len(), if accepted { 4 } else { 2 });
        let junk_order = |leaf: &Path| {
            leaf.chronological
                .iter()
                .filter_map(|e| match e {
                    Event::Moved(m) if m.verb == Some(Verb::Junk) => Some(m.card),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let mut orders = vec![];
        for leaf in full {
            let order = junk_order(leaf);
            if !orders.contains(&order) {
                orders.push(order);
            }
        }
        assert_eq!(orders.len(), if accepted { 2 } else { 1 });
        for order in orders {
            let full_order: Vec<_> = full
                .iter()
                .filter(|leaf| junk_order(leaf) == order)
                .copied()
                .collect();
            assert_eq!(
                full_order.len(),
                2,
                "both native classes for this particular order"
            );
            // Player order choices are not random: prove the same law for
            // each order separately, never merely average different orders.
            for leaf in full_order.iter().chain(reduced.iter()) {
                let reduced_count = reduced.iter().filter(|r| r.outcome == leaf.outcome).count();
                let full_count = full_order
                    .iter()
                    .filter(|f| f.outcome == leaf.outcome)
                    .count();
                assert_eq!(
                    reduced_count * full_order.len(),
                    full_count * reduced.len(),
                    "identical native probability mass for every outcome at every order"
                );
                assert_eq!(
                    leaf.outcome.views, reduced[0].outcome.views,
                    "neither player sees which hidden identity was sampled"
                );
            }
        }
        for leaf in reduced {
            assert!(
                full.iter()
                    .any(|f| f.outcome == leaf.outcome && f.chronological == leaf.chronological),
                "the canonical chronological path also exists unrestricted"
            );
        }
        if accepted {
            for candidate in candidates {
                assert_eq!(
                    reduced
                        .iter()
                        .filter(|leaf| leaf.outcome.position.cards(Zone::Achievements(A))
                            == [candidate])
                        .count(),
                    1,
                    "both physical candidates retain one half of the native probability"
                );
            }
        }
    }
}

#[test]
fn chopsticks_three_card_sweep_takes_the_deck_top_and_keeps_native_weights() {
    // Shortcut policy: a whole-deck junk takes the deck top even when the later blind
    // claim's cohort is not a power of two. Unrestricted play still offers every order;
    // the reduced path is the top-first order with its exact native weights.
    let mut f = Fixture::default();
    let card = source(&mut f, "E004");
    let candidates: [CardId; 3] =
        [Color::Blue, Color::Yellow, Color::Purple].map(|color| draw_card(&mut f, color, 1));
    tech(&mut f, Color::Red, 5, Zone::Score(A));
    let initial = start(f, card, 0, E);
    let seeds: [u64; 3] = std::array::from_fn(|index| {
        (0..1000)
            .find(|&seed| Rng::new(seed).next_u64() % 3 == index as u64)
            .unwrap()
    });
    let words = 1_u128 << 64;
    let weights: [u128; 3] =
        std::array::from_fn(|index| words / 3 + u128::from((index as u128) < words % 3));
    let junk_order = |leaf: &Path| {
        leaf.chronological
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) if m.verb == Some(Verb::Junk) => Some(m.card),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut reduced: Vec<(Path, u128)> = vec![];
    let mut full: Vec<(Path, u128)> = vec![];
    for (index, seed) in seeds.into_iter().enumerate() {
        for disabled in [false, true] {
            let mut game = initial.clone();
            game.shortcuts_disabled = disabled;
            game.rng = Rng::new(seed);
            let leaves = paths(game, vec![], 0);
            assert_eq!(leaves.len(), if disabled { 7 } else { 2 });
            for mut leaf in leaves {
                let accepted = !leaf
                    .outcome
                    .position
                    .cards(Zone::Achievements(A))
                    .is_empty();
                let mut expected_rng = Rng::new(seed);
                if accepted {
                    expected_rng.next_u64();
                }
                assert_eq!(leaf.outcome.rng, expected_rng);
                leaf.outcome.rng = Rng::new(0);
                if disabled {
                    full.push((leaf, weights[index]));
                } else {
                    reduced.push((leaf, weights[index]));
                }
            }
        }
    }
    let orders: Vec<_> = reduced
        .iter()
        .map(|(leaf, _)| junk_order(leaf))
        .filter(|order| !order.is_empty())
        .collect();
    assert!(orders.iter().all(|order| order == &orders[0]));
    let top_first = &orders[0];
    assert_eq!(top_first.len(), 3);
    for (leaf, _) in &reduced {
        assert!(
            full.iter().any(|(other, _)| other.outcome == leaf.outcome
                && other.chronological == leaf.chronological)
        );
    }
    for candidate in candidates {
        let weight = reduced
            .iter()
            .filter(|(leaf, _)| leaf.outcome.position.cards(Zone::Achievements(A)) == [candidate])
            .map(|(_, w)| *w)
            .sum::<u128>();
        assert_eq!(
            weight,
            words / 3 + u128::from(candidate == top_first[0]),
            "the first junked card keeps the native remainder weight"
        );
    }
}
