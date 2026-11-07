//! Age-four clauses, conditional proof privacy and complete shortcut paths.
use super::common::*;
use super::*;
use program::build;

const E: Expansions = Expansions::ECHOES;
const EF: Expansions = E.union(Expansions::FIGURES);

fn tech(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, Vec::new())
}
fn deck(f: &mut Fixture, color: Color, value: u8) -> CardId {
    tech(f, color, value, Zone::Deck(Deck::base(value)))
}
fn echo_source(f: &mut Fixture, code: &str) -> CardId {
    f.add(echoes_card(code).unwrap(), Zone::Junk)
}
fn start(f: Fixture, source: CardId, effect: u16, executor: Player, figures: bool) -> Game {
    let expansions = if figures { EF } else { E };
    let mut game = Game::seeded(f.position(), expansions, 41)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(executor);
    context.demander = A;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}
fn promoted(mut f: Fixture, source: CardId, effect: usize) -> Game {
    let selected = f.cards[usize::from(source.0)].effects[effect].clone();
    f.cards[usize::from(source.0)].effects = vec![selected];
    f.cards[usize::from(source.0)].echo = None;
    f.placements
        .iter_mut()
        .find(|(card, _)| *card == source)
        .unwrap()
        .1 = Zone::Forecast(A);
    let featured = f.cards[usize::from(source.0)].featured.unwrap();
    let boost = tech(&mut f, Color::Purple, 10, Zone::Board(A, Color::Purple));
    f.cards[usize::from(boost.0)].symbols = [Symbol::Icon(featured); SLOTS];
    let action = tech(&mut f, Color::Blue, 11, Zone::Hand(A));
    let mut game = Game::seeded(f.position(), EF, 41)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Meld { card: action }, ActionKind::Regular)
        .unwrap();
    game
}
fn finish(game: &mut Game, mut pick: impl FnMut(&Decision) -> usize) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            Status::Complete(_) | Status::Winner(_) | Status::Draw(_) => break,
            status => panic!("unexpected {status:?}"),
        }
    }
    assert!(game.position.check_conservation());
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(&game.position));
    }
    events
}
fn decline(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|o| matches!(o, Alternative::Branch(1) | Alternative::Decline))
        .unwrap_or(0)
}
fn choose_card(menu: &Decision, wanted: CardId) -> usize {
    menu.options
        .iter()
        .position(|o| {
            matches!(o,
        Alternative::Move { target:TargetDescriptor::Known {card,..},.. }
        | Alternative::Select { target:TargetDescriptor::Known {card,..} }
        | Alternative::Reveal { target:TargetDescriptor::Known {card,..} } if *card==wanted)
        })
        .unwrap_or(0)
}
fn revealed(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

#[test]
fn age4_catalog_clauses_and_operand_sizes_match_contract() {
    for (code, echo, dogmas, demand) in [
        ("E036", true, 3, true),
        ("E037", false, 2, true),
        ("E038", true, 2, false),
        ("E039", true, 1, false),
        ("E040", false, 3, false),
        ("E041", false, 2, false),
        ("E042", false, 2, false),
        ("E044", true, 1, true),
    ] {
        let card = echoes_card(code).unwrap();
        assert_eq!(card.echo.is_some(), echo, "{code}");
        assert_eq!(card.effects.len(), dogmas, "{code}");
        assert_eq!(card.effects[0].demand(), demand, "{code}");
    }
    assert!(size_of::<Instruction>() <= 96);
    assert_eq!(size_of::<Frame>(), 56);
}

#[test]
fn kobukson_echo_keeps_both_owners_and_nonempty_noops() {
    for selected in [A, B] {
        let mut f = Fixture::default();
        let card = echo_source(&mut f, "E036");
        tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        for _ in 0..2 {
            tech(&mut f, Color::Green, 1, Zone::Board(B, Color::Green));
        }
        let mut game = start(f, card, Card::ECHO, A, false);
        let events = finish(&mut game, |menu| {
            assert_eq!(menu.options.len(), 2);
            assert!(menu.options.iter().all(|o| matches!(
                o,
                Alternative::Splay {
                    color: Color::Green,
                    direction: Splay::Left,
                    ..
                }
            )));
            menu.options
                .iter()
                .position(|o| matches!(o,Alternative::Splay{player,..} if *player==selected))
                .unwrap()
        });
        assert_eq!(game.position.splay(A, Color::Green), Splay::None);
        assert_eq!(
            game.position.splay(B, Color::Green),
            if selected == B {
                Splay::Left
            } else {
                Splay::None
            }
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Splayed { .. }))
                .count(),
            usize::from(selected == B)
        );
    }
    let mut f = Fixture::default();
    let card = echo_source(&mut f, "E036");
    let mut game = start(f, card, Card::ECHO, A, false);
    assert!(finish(&mut game, |_| panic!("no nonempty board")).is_empty());
}

#[test]
fn kobukson_demands_one_per_color_and_draws_even_without_returns() {
    for authority in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E036");
        let bottom = tech(&mut f, Color::Green, 1, Zone::Board(B, Color::Green));
        let top = tech(&mut f, Color::Green, 2, Zone::Board(B, Color::Green));
        if authority {
            for id in [bottom, top] {
                f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Authority);
            }
        }
        let drawn = deck(&mut f, Color::Green, 4);
        let mut game = start(f, source, 0, B, false);
        finish(&mut game, |_| 0);
        assert_eq!(
            game.position.locate(bottom),
            Some(Zone::Board(B, Color::Green))
        );
        assert_eq!(
            game.position.locate(top),
            Some(if authority {
                Zone::Deck(Deck::base(2))
            } else {
                Zone::Board(B, Color::Green)
            })
        );
        assert_eq!(game.position.cards(Zone::Board(B, Color::Green))[0], drawn);
    }
    for effect in [1, 2] {
        for foreseen in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E036");
            let drawn = deck(&mut f, Color::Yellow, if effect == 1 { 4 } else { 5 });
            let mut game = if foreseen {
                promoted(f, source, effect as usize)
            } else {
                start(f, source, effect, A, false)
            };
            finish(&mut game, |_| 0);
            assert_eq!(
                game.position.locate(drawn) == Some(Zone::Board(A, Color::Yellow)),
                effect == 1 || foreseen
            );
        }
    }
}

#[test]
fn shuriken_preserves_two_board_arrivals_before_foreseen_achievements() {
    for foreseen in [false, true] {
        for count in [1, 2, 3] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E037");
            let mut eligible = Vec::new();
            for (index, color) in [Color::Green, Color::Yellow, Color::Blue]
                .into_iter()
                .take(count)
                .enumerate()
            {
                let id = tech(&mut f, color, 1, Zone::Board(B, color));
                f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(if index == 0 {
                    Icon::Authority
                } else {
                    Icon::Avatar
                });
                eligible.push(id);
            }
            let red = tech(&mut f, Color::Red, 3, Zone::Board(B, Color::Red));
            f.cards[usize::from(red.0)].symbols[0] = Symbol::Icon(Icon::Authority);
            let mut game = if foreseen {
                promoted(f, source, 0)
            } else {
                start(f, source, 0, B, false)
            };
            let events = finish(&mut game, |_| 0);
            let moved: Vec<_> = eligible
                .iter()
                .filter(|&&id| !matches!(game.position.locate(id), Some(Zone::Board(B, _))))
                .copied()
                .collect();
            assert_eq!(moved.len(), count.min(2));
            assert_eq!(game.position.locate(red), Some(Zone::Board(B, Color::Red)));
            for id in moved {
                assert_eq!(
                    game.position.locate(id) == Some(Zone::Achievements(A)),
                    foreseen && count >= 2
                );
            }
            if foreseen && count >= 2 {
                let arrivals: Vec<_> = events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Moved(m) if eligible.contains(&m.card) => Some(m.destination),
                        _ => None,
                    })
                    .collect();
                assert!(matches!(
                    arrivals.as_slice(),
                    [
                        Zone::Board(A, _),
                        Zone::Board(A, _),
                        Zone::Achievements(A),
                        Zone::Achievements(A)
                    ]
                ));
            }
        }
    }
    for pass in [false, true] {
        let mut f = Fixture::default();
        let card = echo_source(&mut f, "E037");
        for _ in 0..2 {
            tech(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
        }
        let mut game = start(f, card, 1, A, false);
        finish(&mut game, |menu| if pass { decline(menu) } else { 0 });
        assert_eq!(
            game.position.splay(A, Color::Purple),
            if pass { Splay::None } else { Splay::Right }
        );
    }
}

#[test]
fn chintz_counts_hand_once_then_scores_only_second_reward_and_foreseen_transfers() {
    for size in [0, 1, 2] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E040");
        for _ in 0..size {
            tech(&mut f, Color::Red, 1, Zone::Hand(A));
        }
        let scored = deck(&mut f, Color::Blue, 4);
        let kept = deck(&mut f, Color::Green, 4);
        let mut game = start(f, source, 1, A, false);
        finish(&mut game, |_| 0);
        assert_eq!(
            game.position.locate(scored) == Some(Zone::Score(A)),
            size == 1
        );
        assert_eq!(game.position.locate(kept) == Some(Zone::Hand(A)), size == 1);
        assert_eq!(
            game.position.cards(Zone::Hand(A)).len(),
            size + usize::from(size == 1)
        );
    }
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E040");
    let drawn = deck(&mut f, Color::Blue, 4);
    let mut game = start(f, source, 0, A, false);
    finish(&mut game, |_| 0);
    assert_eq!(game.position.locate(drawn), Some(Zone::Hand(A)));
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E040");
        let hand = tech(&mut f, Color::Red, 1, Zone::Hand(A));
        deck(&mut f, Color::Green, 1);
        let mut game = if foreseen {
            promoted(f, source, 2)
        } else {
            start(f, source, 2, A, false)
        };
        finish(&mut game, |_| 0);
        assert_eq!(
            game.position.locate(hand),
            Some(if foreseen {
                Zone::AvailableAchievements
            } else {
                Zone::Hand(A)
            })
        );
        if foreseen {
            assert_eq!(game.position.cards(Zone::AvailableAchievements), [hand]);
            assert_eq!(
                game.tracker(A).unwrap().belief(Zone::AvailableAchievements)[0].known(),
                Some(hand)
            );
            assert_eq!(
                game.tracker(B).unwrap().belief(Zone::AvailableAchievements)[0].known(),
                None,
                "supply transfer preserves the other player's uncertainty"
            );
        }
    }
}

#[test]
fn slide_rule_uses_lowest_top_plus_splayed_color_count_and_optional_yellow() {
    for direction in [
        Splay::None,
        Splay::Left,
        Splay::Right,
        Splay::Up,
        Splay::Aslant,
    ] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E042");
        for _ in 0..3 {
            tech(&mut f, Color::Yellow, 2, Zone::Board(A, Color::Yellow));
        }
        tech(&mut f, Color::Blue, 7, Zone::Board(A, Color::Blue));
        let value = 2 + u8::from(direction != Splay::None);
        let drawn = deck(&mut f, Color::Red, value);
        let mut game = start(f, source, 1, A, false);
        game.position.players[A.index()].splays[Color::Yellow as usize] = direction;
        finish(&mut game, |_| 0);
        assert_eq!(game.position.locate(drawn), Some(Zone::Hand(A)));
    }
    for pass in [false, true] {
        let mut f = Fixture::default();
        let card = echo_source(&mut f, "E042");
        for _ in 0..2 {
            tech(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
        }
        let mut game = start(f, card, 0, A, false);
        finish(&mut game, |menu| if pass { decline(menu) } else { 0 });
        assert_eq!(
            game.position.splay(A, Color::Yellow),
            if pass { Splay::None } else { Splay::Right }
        );
    }
}

#[test]
fn clock_chooses_demander_visible_count_then_transfers_both_piles_and_junks_independently() {
    for (chosen, count) in [(Color::Red, 0), (Color::Green, 1), (Color::Blue, 3)] {
        for matching in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E044");
            for _ in 0..4 {
                tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
            }
            for _ in 0..3 {
                tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
            }
            // Victim's chosen color differs, so executor-relative counting fails.
            tech(&mut f, chosen, 5, Zone::Board(B, chosen));
            let hand = tech(
                &mut f,
                Color::Yellow,
                if matching && count > 0 { count } else { 8 },
                Zone::Hand(B),
            );
            let score = tech(
                &mut f,
                Color::Purple,
                if matching && count > 0 { count } else { 8 },
                Zone::Score(B),
            );
            let achievement = tech(
                &mut f,
                Color::Red,
                count.max(1),
                Zone::AvailableAchievements,
            );
            let mut game = start(f, source, 0, B, false);
            game.position.players[A.index()].splays[Color::Blue as usize] = Splay::Left;
            let events = finish(&mut game, |menu| {
                if menu
                    .options
                    .iter()
                    .any(|o| matches!(o, Alternative::Colors(_)))
                {
                    assert_eq!(menu.player, A);
                    menu.options
                        .iter()
                        .position(|o| *o == Alternative::Colors(1 << chosen as u8))
                        .unwrap()
                } else {
                    0
                }
            });
            assert_eq!(
                game.position.locate(achievement),
                Some(if count > 0 {
                    Zone::Junk
                } else {
                    Zone::AvailableAchievements
                })
            );
            for card in [hand, score] {
                assert_eq!(
                    game.position.locate(card) == Some(Zone::Score(A)),
                    matching && count > 0
                );
            }
            assert!(
                revealed(&events).is_empty(),
                "back values need no front proof"
            );
        }
    }
}

#[test]
fn clock_echo_finds_total_stack_maxima_before_splay_admission() {
    for largest_right in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E044");
        for _ in 0..3 {
            tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        }
        for _ in 0..2 {
            tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        }
        let mut game = start(f, source, Card::ECHO, A, false);
        game.position.players[A.index()].splays[Color::Blue as usize] = Splay::Left;
        if largest_right {
            game.position.players[A.index()].splays[Color::Green as usize] = Splay::Right;
        }
        finish(&mut game, |menu| {
            assert!(!largest_right, "smaller legal pile must not qualify");
            assert!(menu.options.iter().all(|o| matches!(
                o,
                Alternative::Splay {
                    color: Color::Green,
                    ..
                } | Alternative::Branch(_)
                    | Alternative::Decline
            )));
            0
        });
        assert_eq!(game.position.splay(A, Color::Green), Splay::Right);
        assert_eq!(game.position.splay(A, Color::Blue), Splay::Left);
    }
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E044");
    for color in [Color::Green, Color::Blue] {
        for _ in 0..3 {
            tech(&mut f, color, 1, Zone::Board(A, color));
        }
    }
    let mut game = start(f, source, Card::ECHO, A, false);
    finish(&mut game, |menu| {
        assert_eq!(
            menu.options
                .iter()
                .filter(|option| matches!(option, Alternative::Splay { .. }))
                .count(),
            2,
            "all largest ties remain choices"
        );
        decline(menu)
    });
    assert_eq!(game.position.splay(A, Color::Green), Splay::None);
    assert_eq!(game.position.splay(A, Color::Blue), Splay::None);
}

#[test]
fn pencil_rewards_actual_returns_uses_highest_and_keeps_only_its_draw_packet() {
    for returned in [0, 1, 2] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E039");
            let first = tech(&mut f, Color::Green, 2, Zone::Hand(A));
            let second = tech(&mut f, Color::Red, 1, Zone::Hand(A));
            let reward1 = deck(&mut f, Color::Yellow, 3);
            let reward2 = deck(&mut f, Color::Blue, 3);
            if full {
                for _ in 0..5 {
                    tech(&mut f, Color::Purple, 5, Zone::Forecast(A));
                }
            }
            let mut game = start(f, source, 0, A, false);
            game.shortcuts_disabled = true;
            let mut picked = 0;
            finish(&mut game, |menu| {
                let initial=menu.options.iter().any(|o|matches!(o,Alternative::Move{verb:Verb::Return,target:TargetDescriptor::Known{card,..}} if *card==first || *card==second));
                if initial && picked >= returned {
                    return decline(menu);
                }
                if initial {
                    let id = if picked == 0 { first } else { second };
                    picked += 1;
                    choose_card(menu, id)
                } else {
                    0
                }
            });
            assert_eq!(
                game.position.cards(Zone::Forecast(A)).len(),
                if full { 5 } else { usize::from(returned > 0) }
            );
            let returned_originals = [first, second]
                .iter()
                .filter(|&&id| matches!(game.position.locate(id), Some(Zone::Deck(_))))
                .count();
            assert_eq!(returned_originals, returned);
            let claimed = [reward1, reward2]
                .iter()
                .filter(|&&id| {
                    matches!(
                        game.position.locate(id),
                        Some(Zone::Hand(A) | Zone::Forecast(A))
                    )
                })
                .count();
            assert_eq!(claimed, usize::from(returned > 0));
            if returned > 0 && !full {
                assert!(
                    game.position
                        .cards(Zone::Forecast(A))
                        .iter()
                        .any(|id| [reward1, reward2].contains(id)),
                    "only its original draw packet can supply the retained card"
                );
            }
            if returned == 1 {
                assert_eq!(
                    game.position.locate(second),
                    Some(Zone::Hand(A)),
                    "an unreturned old hand card is not in the draw packet"
                );
            }
            if returned == 2 {
                assert_eq!(game.position.cards(Zone::Deck(Deck::base(3))).len(), 1);
            }
        }
    }
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E039");
    let drawn = deck(&mut f, Color::Blue, 5);
    let mut game = start(f, source, Card::ECHO, A, false);
    finish(&mut game, |_| 0);
    assert_eq!(game.position.locate(drawn), Some(Zone::Hand(A)));
}

#[test]
fn barometer_echo_and_bonus_draw_respect_any_board_and_capacity() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E038");
    let five = tech(&mut f, Color::Blue, 5, Zone::Forecast(A));
    let four = tech(&mut f, Color::Blue, 4, Zone::Forecast(A));
    let mut game = start(f, source, Card::ECHO, A, false);
    finish(&mut game, |_| 0);
    assert_eq!(game.position.locate(five), Some(Zone::Hand(A)));
    assert_eq!(game.position.locate(four), Some(Zone::Forecast(A)));
    for full in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E038");
        let printed = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
        let napoleon = f.add(printed.clone(), Zone::Board(B, printed.color));
        assert_eq!(f.cards[usize::from(napoleon.0)].bonus(), Some(6));
        let drawn = deck(&mut f, Color::Green, 8);
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Purple, 1, Zone::Forecast(A));
            }
        }
        let mut game = start(f, source, 0, A, true);
        finish(&mut game, |_| 0);
        assert_eq!(
            game.position.locate(drawn),
            Some(if full {
                Zone::Hand(A)
            } else {
                Zone::Forecast(A)
            })
        );
    }
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E038");
    let untouched = deck(&mut f, Color::Green, 2);
    let mut game = start(f, source, 0, A, false);
    finish(&mut game, |_| panic!("no bonus"));
    assert_eq!(
        game.position.locate(untouched),
        Some(Zone::Deck(Deck::base(2)))
    );
}

#[test]
fn barometer_verifies_first_blue_only_and_proves_failure_without_exposing_declines() {
    for blue_count in [0, 1, 2] {
        for pass in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E038");
            let red = tech(&mut f, Color::Red, 1, Zone::Forecast(A));
            let blue: Vec<_> = (0..blue_count)
                .map(|_| tech(&mut f, Color::Blue, 1, Zone::Forecast(A)))
                .collect();
            let mut game = start(f, source, 1, A, false);
            let events = finish(&mut game, |menu| {
                if pass {
                    decline(menu)
                } else {
                    choose_card(menu, red)
                }
            });
            let shown = revealed(&events);
            assert_eq!(
                game.position.special_location(Special::Destiny) == SpecialLocation::Owned(A),
                blue_count > 0 && !pass
            );
            if pass {
                assert!(shown.is_empty());
                assert_eq!(game.position.cards(Zone::Forecast(A)).len(), blue_count + 1);
            } else if blue_count > 0 {
                assert_eq!(shown.len(), 1);
                assert!(blue.contains(&shown[0]));
                assert!(!shown.contains(&red));
                let returned = events
                    .iter()
                    .position(|e| matches!(e,Event::Moved(m) if m.card==shown[0]))
                    .unwrap();
                let proof = events
                    .iter()
                    .position(|e| matches!(e,Event::Revealed{card,..} if *card==shown[0]))
                    .unwrap();
                assert!(proof < returned);
            } else {
                assert!(!shown.is_empty());
                assert!(shown.iter().all(|&id| id == red));
            }
        }
    }
}

#[test]
fn globe_verifies_one_of_each_required_color_then_all_draws_even_at_capacity() {
    for missing in [
        None,
        Some(Color::Blue),
        Some(Color::Green),
        Some(Color::Yellow),
    ] {
        for pass in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E041");
            let mut required = Vec::new();
            for color in [Color::Blue, Color::Green, Color::Yellow] {
                if Some(color) != missing {
                    required.push(tech(&mut f, color, 1, Zone::Hand(A)));
                }
            }
            let spare = tech(&mut f, Color::Red, 1, Zone::Hand(A));
            let duplicate = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
            // A missing blue scenario must not accidentally supply its witness.
            if missing == Some(Color::Blue) {
                f.cards[usize::from(duplicate.0)].color = Color::Purple;
            }
            for _ in 0..2 {
                tech(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
            }
            for _ in 0..4 {
                tech(&mut f, Color::Red, 2, Zone::Forecast(A));
            }
            let drawn = [
                deck(&mut f, Color::Red, 6),
                deck(&mut f, Color::Green, 7),
                deck(&mut f, Color::Blue, 8),
            ];
            let mut game = start(f, source, 0, A, false);
            let events = finish(&mut game, |menu| {
                if pass {
                    decline(menu)
                } else {
                    choose_card(menu, spare)
                }
            });
            let success = missing.is_none() && !pass;
            let shown = revealed(&events);
            if pass {
                assert!(shown.is_empty());
            } else if success {
                assert_eq!(shown.len(), 3);
                assert!(!shown.contains(&spare));
                let mut colors: Vec<_> = shown
                    .iter()
                    .map(|&id| game.position.card(id).color)
                    .collect();
                colors.sort_by_key(|color| *color as u8);
                let mut expected = vec![Color::Blue, Color::Green, Color::Yellow];
                expected.sort_by_key(|color| *color as u8);
                assert_eq!(colors, expected);
            } else {
                for id in required.into_iter().chain([spare, duplicate]) {
                    assert!(shown.contains(&id));
                }
            }
            assert_eq!(
                game.position.locate(drawn[0]) == Some(Zone::Forecast(A)),
                success
            );
            for id in [drawn[1], drawn[2]] {
                assert_eq!(game.position.locate(id) == Some(Zone::Hand(A)), success);
            }
            assert_eq!(
                game.position.splay(A, Color::Purple),
                if success { Splay::Right } else { Splay::None }
            );
        }
    }
}

#[test]
fn globe_foreseen_can_foreshadow_other_board_and_refuses_full_forecast_before_removal() {
    for foreseen in [false, true] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, "E041");
            let other = tech(&mut f, Color::Yellow, 1, Zone::Board(B, Color::Yellow));
            if full {
                for _ in 0..5 {
                    tech(&mut f, Color::Red, 11, Zone::Forecast(A));
                }
            }
            let mut game = if foreseen {
                promoted(f, source, 1)
            } else {
                start(f, source, 1, A, false)
            };
            finish(&mut game, |menu| choose_card(menu, other));
            assert_eq!(
                game.position.locate(other),
                Some(if foreseen && !full {
                    Zone::Forecast(A)
                } else {
                    Zone::Board(B, Color::Yellow)
                })
            );
        }
    }
}

#[test]
fn conditional_verification_preserves_bound_and_projects_keep_add_successes_only() {
    use build::*;
    const RECORD: Slot = Slot::effect(0);
    const OTHER: Selector = filtered(&HAND, &[Filter::Lacks(Test::Color(Color::Blue))]);
    const EMPTY_PROGRESS: Cond = Cond::Compare(
        Expr::Count(&Selector::Slot(RECORD)),
        Cmp::Equal,
        Expr::Const(0),
    );
    for append in [false, true] {
        let mut f = Fixture::default();
        let anchor = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        let first = tech(&mut f, Color::Red, 1, Zone::Hand(A));
        let second = tech(&mut f, Color::Green, 2, Zone::Hand(A));
        let returns = return_(OTHER).all().verify_when(&EMPTY_PROGRESS);
        let instructions = vec![
            bind(filtered(&HAND, &[Filter::Has(Test::Color(Color::Blue))])),
            reveal(Selector::Reg(Reg::Bound)).keep(RECORD),
            if append {
                returns.add(RECORD)
            } else {
                returns.keep(RECORD)
            },
            score(Selector::Reg(Reg::Bound)),
        ];
        let source = f.tech(
            Color::Purple,
            4,
            Zone::Board(A, Color::Purple),
            instructions,
        );
        let mut game = start(f, source, 0, A, false);
        let events = finish(&mut game, |menu| choose_card(menu, first));
        let shown = revealed(&events);
        assert_eq!(
            shown,
            if append {
                vec![anchor]
            } else {
                vec![anchor, first]
            }
        );
        assert_eq!(
            game.position.locate(anchor),
            Some(Zone::Score(A)),
            "temporary Bound must not escape"
        );
        assert_eq!(
            game.position.locate(second),
            Some(Zone::Deck(Deck::base(2)))
        );
    }
    // Conditional front-reading is attributed to a selected operand, not to
    // an unrelated caller Bound. Source/Last operands retain normal traversal.
    let matching = return_(Selector::Reg(Reg::Bound)).verify_when(&Cond::Card(
        Reg::Bound,
        Filter::Has(Test::Color(Color::Blue)),
    ));
    let mut karma = Figure::JamesClerkMaxwell.def().karma[0];
    karma.body = Body::Program(Box::leak(vec![matching].into_boxed_slice()));
    assert!(karma.body_reads_card_front());
}

#[test]
fn conditional_verification_restores_pending_records_on_query_error() {
    use build::*;
    const RECORD: Slot = Slot::effect(0);
    let mut f = Fixture::default();
    tech(&mut f, Color::Blue, 1, Zone::Hand(A));
    tech(&mut f, Color::Green, 2, Zone::Hand(A));
    tech(&mut f, Color::Yellow, 3, Zone::Hand(A));
    tech(&mut f, Color::Red, 1, Zone::Hand(B));
    let source = f.tech(
        Color::Purple,
        4,
        Zone::Board(A, Color::Purple),
        vec![
            reveal(filtered(&HAND, &[Filter::Has(Test::Color(Color::Blue))])).keep(RECORD),
            return_(HAND).all().add(RECORD),
        ],
    );
    let mut game = start(f, source, 0, A, false);
    let mut events = Vec::new();
    assert!(matches!(
        game.advance(1000, &mut events).unwrap(),
        Status::Decision(_)
    ));
    game.choose(0).unwrap();
    assert!(matches!(
        game.advance(1000, &mut events).unwrap(),
        Status::Decision(_)
    ));
    let menu = game.frames.pop().unwrap();
    let Frame::MoveAll(batch) = game.frames.last().unwrap() else {
        panic!("return batch")
    };
    let (context, regs) = (batch.context, batch.regs);
    let before = game.frames.clone();
    const BAD: Cond = Cond::Compare(
        Expr::Count(&filtered(
            &Selector::Hand(Who::Opponent),
            &[Filter::Has(Test::Color(Color::Red))],
        )),
        Cmp::Greater,
        Expr::Const(0),
    );
    assert!(matches!(
        game.verification_condition(BAD, context, regs),
        Err(Error::Unsupported("private filtered card selection"))
    ));
    assert_eq!(
        game.frames, before,
        "all frames including pending add record restored"
    );
    game.frames.push(menu);
    finish(&mut game, |_| 0);
}

#[test]
fn barometer_first_reaction_preserves_private_remainder_and_failed_returns_do_not_prove_blue() {
    for replace_blue in [false, true] {
        let mut f = Fixture::default();
        let mut printed = echoes_card("E038").unwrap();
        printed.effects[1]
            .instructions
            .push(crate::program::build::choose(
                Who::Opponent,
                Choice::Value(ValueChoices::Boxed(&[1, 2])),
            ));
        let source = f.add(printed, Zone::Board(A, Color::Yellow));
        let red = tech(&mut f, Color::Red, 1, Zone::Forecast(A));
        let blue = tech(&mut f, Color::Blue, 1, Zone::Forecast(A));
        let green = tech(&mut f, Color::Green, 1, Zone::Forecast(A));
        let maxwell = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
        f.add(maxwell.clone(), Zone::Board(B, maxwell.color));
        let extra = [
            deck(&mut f, Color::Purple, 8),
            deck(&mut f, Color::Purple, 8),
            deck(&mut f, Color::Purple, 8),
        ];
        let mut game = start(f, source, 1, A, true);
        let mut events = Vec::new();
        let mut intercepted = false;
        let mut saw_private_first = false;
        let mut searched_after_proof = false;
        for _ in 0..1000 {
            if let Some(Frame::Operation {
                request:
                    Operation::Move {
                        verb: Verb::Return,
                        target,
                        ..
                    },
                phase: Phase::Discover,
                knowledge: Some(known),
                ..
            }) = game.frames.last()
                && target.card == blue
                && !known.verify
                && !intercepted
                && replace_blue
            {
                let Some(Frame::Operation { phase, .. }) = game.frames.last_mut() else {
                    unreachable!()
                };
                *phase = Phase::Replaced;
                intercepted = true;
            }
            if game
                .frames
                .iter()
                .any(|frame| matches!(frame,Frame::Karma{hook,..} if hook.source==CardId(4)))
                && game.position.locate(red) == Some(Zone::Forecast(A))
            {
                saw_private_first = true;
                assert!(
                    !game
                        .context_refs(B)
                        .iter()
                        .any(|r| r.sight == Sight::Card(red))
                );
                assert!(
                    !game
                        .context_refs(B)
                        .iter()
                        .any(|r| r.sight == Sight::Card(green))
                );
            }
            match game.advance(1, &mut events).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(menu) => {
                    if menu.options == [Alternative::Value(1), Alternative::Value(2)] {
                        assert_eq!(menu.player, B);
                        let world = game.determinize(B, 31).unwrap();
                        assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
                        assert_eq!(
                            game.search(SearchConfig {
                                iterations: 4,
                                rollout_actions: 0,
                                seed: 31,
                                ..SearchConfig::default()
                            })
                            .unwrap()
                            .iterations,
                            4
                        );
                        searched_after_proof = true;
                    }
                    game.choose(choose_card(&menu, red)).unwrap();
                }
                Status::Complete(_) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(saw_private_first, "actual Maxwell First was reached");
        assert!(searched_after_proof);
        assert_eq!(intercepted, replace_blue);
        assert_eq!(
            game.position.special_location(Special::Destiny) == SpecialLocation::Owned(A),
            !replace_blue
        );
        let shown = revealed(&events);
        if replace_blue {
            assert!(shown.contains(&red) && shown.contains(&green));
            assert_eq!(game.position.locate(blue), Some(Zone::Forecast(A)));
        } else {
            assert_eq!(shown, vec![blue]);
        }
        assert_eq!(
            shown.iter().filter(|&&id| id == blue).count(),
            1,
            "failed blue never enters failure packet"
        );
        assert!(
            extra.iter().all(|id| !shown.contains(id)),
            "karma children never enter return proof"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Moved(Movement {
                verb: None,
                executor: B,
                inside_karma: true,
                ..
            })
        )));
        assert!(game.position.check_conservation());
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Leaf {
    position: Position,
    rng: Rng,
    status: Status,
    events: Vec<String>,
    beliefs: Vec<Vec<crate::tracking::BeliefSlot>>,
}
fn paths(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<Leaf> {
    assert!(depth < 20, "bounded age-four oracle");
    match game.advance(1000, &mut events).unwrap() {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut child = game.clone();
                child.choose(choice).unwrap();
                paths(child, events.clone(), depth + 1)
            })
            .collect(),
        status => {
            assert!(!matches!(status, Status::WorkLimit));
            assert!(game.position.check_conservation());
            let beliefs = Player::ALL
                .into_iter()
                .flat_map(|p| {
                    [
                        Zone::Hand(A),
                        Zone::Hand(B),
                        Zone::Forecast(A),
                        Zone::Score(A),
                        Zone::Score(B),
                        Zone::AvailableAchievements,
                        Zone::Deck(Deck::base(1)),
                        Zone::Deck(Deck::base(2)),
                        Zone::Deck(Deck::base(3)),
                    ]
                    .map(|zone| game.tracker(p).unwrap().belief(zone))
                })
                .collect();
            game.position.sort_unordered_piles();
            let mut events: Vec<_> = events.into_iter().map(|e| format!("{e:?}")).collect();
            events.sort();
            vec![Leaf {
                position: game.position,
                rng: game.rng,
                status,
                events,
                beliefs,
            }]
        }
    }
}

#[test]
fn age4_batch_shortcuts_match_every_unrestricted_path_with_reacting_figures() {
    for code in ["E036", "E037", "E038", "E039", "E040", "E041", "E044"] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, code);
            let mut executor = A;
            let mut effect = 0;
            match code {
                "E036" => {
                    executor = B;
                    for (color, age) in [(Color::Green, 1), (Color::Blue, 2)] {
                        let id = tech(&mut f, color, age, Zone::Board(B, color));
                        f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Authority);
                    }
                    deck(&mut f, Color::Yellow, 4);
                }
                "E037" => {
                    executor = B;
                    for (color, age) in [(Color::Green, 1), (Color::Yellow, 2)] {
                        let id = tech(&mut f, color, age, Zone::Board(B, color));
                        f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Authority);
                    }
                }
                "E038" => {
                    effect = 1;
                    tech(&mut f, Color::Blue, 1, Zone::Forecast(A));
                    tech(&mut f, Color::Red, 2, Zone::Forecast(A));
                }
                "E039" => {
                    tech(&mut f, Color::Blue, 1, Zone::Hand(A));
                    tech(&mut f, Color::Red, 2, Zone::Hand(A));
                    for age in [2, 3, 9] {
                        for _ in 0..3 {
                            deck(&mut f, Color::Green, age);
                        }
                    }
                }
                "E040" => {
                    effect = 2;
                    tech(&mut f, Color::Blue, 1, Zone::Hand(A));
                    tech(&mut f, Color::Red, 2, Zone::Hand(A));
                }
                "E041" => {
                    for (color, age) in [(Color::Blue, 1), (Color::Green, 2), (Color::Yellow, 3)] {
                        tech(&mut f, color, age, Zone::Hand(A));
                    }
                    for age in [6, 7, 8] {
                        deck(&mut f, Color::Red, age);
                    }
                }
                "E044" => {
                    executor = B;
                    tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
                    tech(&mut f, Color::Blue, 1, Zone::Hand(B));
                    tech(&mut f, Color::Red, 1, Zone::Score(B));
                }
                _ => unreachable!(),
            }
            if reacting {
                let figure = if matches!(code, "E037" | "E040" | "E044") {
                    Figure::AlfredNobel
                } else {
                    Figure::JamesClerkMaxwell
                };
                let printed = figure_card(figure.code()).unwrap();
                f.add(printed.clone(), Zone::Board(B, printed.color));
                if figure == Figure::AlfredNobel {
                    deck(&mut f, Color::Purple, 7);
                } else {
                    for _ in 0..6 {
                        deck(&mut f, Color::Purple, 8);
                    }
                }
            }
            let game = if code == "E040" {
                promoted(f, source, effect as usize)
            } else {
                start(f, source, effect, executor, true)
            };
            let mut unrestricted = game.clone();
            unrestricted.shortcuts_disabled = true;
            let reduced = paths(game, Vec::new(), 0);
            let full = paths(unrestricted, Vec::new(), 0);
            assert!(
                full.iter().all(|leaf| reduced.contains(leaf)),
                "{code} reacting={reacting}: lost outcome; reduced {}, full {}",
                reduced.len(),
                full.len()
            );
            assert!(
                reduced.iter().all(|leaf| full.contains(leaf)),
                "{code}: invented outcome"
            );
            if reacting {
                assert!(
                    reduced.iter().any(|leaf| leaf
                        .events
                        .iter()
                        .any(|event| event.contains("inside_karma: true"))),
                    "{code}: Figure must actually react"
                );
            } else if !matches!(code, "E038" | "E041") {
                assert!(
                    reduced.len() < full.len(),
                    "{code}: declared reduction must remove an irrelevant order"
                );
            }
        }
    }
}

#[test]
fn conditional_witness_shortcuts_preserve_every_private_identity_disclosure() {
    for code in ["E038", "E041"] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, code);
        let zone = if code == "E038" {
            Zone::Forecast(A)
        } else {
            Zone::Hand(A)
        };
        let blues = [
            tech(&mut f, Color::Blue, 1, zone),
            tech(&mut f, Color::Blue, 2, zone),
        ];
        if code == "E041" {
            tech(&mut f, Color::Green, 1, zone);
            tech(&mut f, Color::Yellow, 1, zone);
            for age in [6, 7, 8] {
                deck(&mut f, Color::Red, age);
            }
        }
        let game = start(f, source, u16::from(code == "E038"), A, false);
        let mut unrestricted = game.clone();
        unrestricted.shortcuts_disabled = true;
        let reduced = paths(game, Vec::new(), 0);
        let full = paths(unrestricted, Vec::new(), 0);
        assert!(
            full.iter().all(|leaf| reduced.contains(leaf)),
            "{code}: witness disclosure lost"
        );
        assert!(reduced.iter().all(|leaf| full.contains(leaf)));
        for blue in blues {
            assert!(
                reduced.iter().any(|leaf| leaf
                    .events
                    .iter()
                    .any(|event| event.starts_with("Revealed")
                        && event.contains(&format!("card: {blue:?},")))),
                "each blue can be the public witness"
            );
        }
    }
}

#[test]
fn conditional_single_choice_remaps_caller_references_before_future_first() {
    use build::*;
    const SAME: Cond = Cond::Compare(
        Expr::Value(Reg::Bound),
        Cmp::Equal,
        Expr::Value(Reg::Chosen),
    );
    let mut f = Fixture::default();
    let own = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
    tech(&mut f, Color::Purple, 2, Zone::Hand(A));
    for color in [Color::Blue, Color::Red, Color::Green] {
        tech(&mut f, color, 1, Zone::Hand(B));
    }
    deck(&mut f, Color::Yellow, 1);
    let source = f.tech(
        Color::Purple,
        4,
        Zone::Board(A, Color::Purple),
        vec![
            choose(Who::Executor, Choice::Card(&Selector::Hand(Who::Opponent))),
            return_(HAND).verify_when(&SAME),
        ],
    );
    let maxwell = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
    f.add(maxwell.clone(), Zone::Board(B, maxwell.color));
    deck(&mut f, Color::Purple, 8);
    let mut game = start(f, source, 0, A, true);
    let mut events = Vec::new();
    // The sole blind back is sampled natively before the real return choice.
    assert!(matches!(
        game.advance(1000, &mut events).unwrap(),
        Status::Decision(_)
    ));
    let view = game.observe(A).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..24 {
        for tracked in [false, true] {
            let mut world = if tracked {
                game.clone().determinize(A, seed).unwrap()
            } else {
                game.sample_world(A, seed).unwrap().world
            };
            if tracked {
                assert_eq!(world.observe(A).unwrap(), view);
            }
            let chosen = world
                .frames
                .iter()
                .find_map(|frame| match frame {
                    Frame::Program {
                        chosen: SelectionParameter::Card(held),
                        ..
                    } => Some(*held),
                    _ => None,
                })
                .unwrap();
            seen.insert(chosen.inner);
            let Some(Frame::Choose { tasks, .. }) = world.frames.last() else {
                panic!("pending conditional choice")
            };
            for task in tasks {
                let Task::VerifyWhen(proof) = task else {
                    panic!("conditional movement task")
                };
                assert_eq!(
                    proof.regs.chosen,
                    SelectionParameter::Card(chosen),
                    "task and program retain the same sampled reference"
                );
            }
            let mut events = Vec::new();
            let Status::Decision(menu) = world.advance(1000, &mut events).unwrap() else {
                panic!("return choice")
            };
            world.choose(choose_card(&menu, own)).unwrap();
            loop {
                match world.advance(1000, &mut events).unwrap() {
                    Status::Decision(menu) => world.choose(choose_card(&menu, own)).unwrap(),
                    Status::Complete(_) => break,
                    status => panic!("unexpected {status:?}"),
                }
            }
            assert_eq!(
                revealed(&events),
                [own],
                "value membership needs only the hidden reference's back"
            );
            assert!(events.iter().any(|event| matches!(
                event,
                Event::Moved(Movement {
                    verb: None,
                    executor: B,
                    inside_karma: true,
                    ..
                })
            )));
            assert!(world.position.check_conservation());
            if tracked {
                for viewer in Player::ALL {
                    assert!(world.tracker(viewer).unwrap().check_truth(&world.position));
                }
            }
        }
    }
    assert!(
        seen.len() > 1,
        "sampled private reference genuinely changes identity"
    );
}

#[test]
fn globe_live_returns_include_first_draws_without_disclosing_extra_faces() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E041");
    let originals: [CardId; 3] = [Color::Blue, Color::Green, Color::Yellow]
        .map(|color| tech(&mut f, color, 1, Zone::Hand(A)));
    let maxwell = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
    f.add(maxwell.clone(), Zone::Board(A, maxwell.color));
    for _ in 0..6 {
        deck(&mut f, Color::Purple, 8);
    }
    deck(&mut f, Color::Red, 6);
    deck(&mut f, Color::Red, 7);
    let mut game = start(f, source, 0, A, true);
    let events = finish(&mut game, |_| 0);
    assert_eq!(
        game.position.cards(Zone::Hand(A)).len(),
        0,
        "all includes new First arrivals"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                Event::Moved(Movement {
                    verb: Some(Verb::Return),
                    inside_karma: false,
                    ..
                })
            ))
            .count(),
        6
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                Event::Moved(Movement {
                    verb: None,
                    inside_karma: true,
                    ..
                })
            ))
            .count(),
        3
    );
    let shown = revealed(&events);
    assert_eq!(shown.len(), 3);
    assert!(shown.iter().all(|id| originals.contains(id)));
    assert_eq!(game.position.cards(Zone::Forecast(A)).len(), 3);
}

#[test]
fn clock_junks_even_when_nightingale_replaces_a_score_transfer() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E044");
    tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
    let hand = tech(&mut f, Color::Blue, 1, Zone::Hand(B));
    let score = tech(&mut f, Color::Red, 1, Zone::Score(B));
    let achievement = tech(&mut f, Color::Yellow, 1, Zone::AvailableAchievements);
    let nightingale = figure_card(Figure::Nightingale.code()).unwrap();
    f.add(nightingale.clone(), Zone::Board(B, nightingale.color));
    let mut game = start(f, source, 0, B, true);
    finish(&mut game, |menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Green as u8))
            .unwrap_or(0)
    });
    assert_eq!(game.position.locate(hand), Some(Zone::Score(A)));
    assert_eq!(game.position.locate(score), Some(Zone::Score(B)));
    assert_eq!(game.position.locate(achievement), Some(Zone::Junk));
}

#[test]
fn conditional_verification_checks_admission_before_any_private_predicate() {
    use build::*;
    const PRIVATE: Cond = Cond::Compare(
        Expr::Count(&filtered(
            &Selector::Hand(Who::Opponent),
            &[Filter::Has(Test::Color(Color::Red))],
        )),
        Cmp::Greater,
        Expr::Const(0),
    );
    for full in [false, true] {
        let mut f = Fixture::default();
        tech(&mut f, Color::Red, 2, Zone::Hand(B));
        tech(&mut f, Color::Blue, 2, Zone::Hand(B));
        let hidden = tech(&mut f, Color::Blue, 1, Zone::Forecast(B));
        deck(&mut f, Color::Green, 1);
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Purple, 3, Zone::Forecast(A));
            }
        }
        let source = f.tech(
            Color::Purple,
            4,
            Zone::Board(A, Color::Purple),
            vec![foreshadow(Selector::Forecast(Who::Opponent)).verify_when(&PRIVATE)],
        );
        let mut game = start(f, source, 0, A, false);
        let mut events = Vec::new();
        let result = game.advance(1000, &mut events);
        if full {
            assert!(matches!(result, Ok(Status::Complete(_))));
        } else {
            assert!(matches!(
                result,
                Err(Error::Unsupported("private filtered card selection"))
            ));
        }
        assert!(
            events.is_empty(),
            "no face disclosure or move before admission/query success"
        );
        assert_eq!(game.position.locate(hidden), Some(Zone::Forecast(B)));
    }
    let mut f = Fixture::default();
    let hand = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
    tech(&mut f, Color::Red, 2, Zone::Hand(B));
    tech(&mut f, Color::Blue, 2, Zone::Hand(B));
    let source = f.tech(
        Color::Purple,
        4,
        Zone::Board(A, Color::Purple),
        vec![transfer(HAND, Destination::Hand(Who::Executor)).verify_when(&PRIVATE)],
    );
    let mut game = start(f, source, 0, A, false);
    assert!(finish(&mut game, |_| 0).is_empty());
    assert_eq!(game.position.locate(hand), Some(Zone::Hand(A)));
}

#[test]
fn conditional_private_queries_stop_foreign_samples_unless_a_witness_test_then_retire() {
    use build::*;
    const OTHER_RED: Cond = Cond::Compare(
        Expr::Count(&filtered(&HAND, &[Filter::Has(Test::Color(Color::Red))])),
        Cmp::Greater,
        Expr::Const(0),
    );
    const BLUE: Selector = filtered(&HAND, &[Filter::Has(Test::Color(Color::Blue))]);
    const FALSE_BLUE: Cond = Cond::Card(Reg::Bound, Filter::Has(Test::Color(Color::Red)));
    const PACKET: Slot = Slot::effect(0);
    for public_witness in [false, true] {
        for tracked in [false, true] {
            let mut f = Fixture::default();
            let blue = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
            let red = tech(&mut f, Color::Red, 1, Zone::Hand(A));
            deck(&mut f, Color::Green, 1);
            tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
            let kalashnikov = figure_card(Figure::Kalashnikov.code()).unwrap();
            f.add(kalashnikov.clone(), Zone::Board(B, kalashnikov.color));
            let mut body = Vec::new();
            if public_witness {
                body.push(reveal(BLUE).keep(PACKET));
                body.push(meld(Selector::Slot(PACKET)).verify_when(&OTHER_RED));
            } else {
                body.push(meld(BLUE).verify_when(&FALSE_BLUE));
            }
            body.push(choose(
                Who::Opponent,
                Choice::Branch(&[(0, "Continue"), (0, "Finish")]),
            ));
            let source = f.tech(Color::Purple, 4, Zone::Board(A, Color::Purple), body);
            let mut game = Game::seeded(f.position(), EF, 41).unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            let Status::Decision(menu) = game.advance(1000, &mut events).unwrap() else {
                panic!("Kalashnikov's actual First branch")
            };
            assert_eq!(menu.player, B);
            assert!(game.frames.iter().any(
                |frame| matches!(frame,Frame::Karma{hook,..} if hook.figure==Figure::Kalashnikov)
            ));
            assert_eq!(
                game.context_refs(B)
                    .iter()
                    .any(|reference| reference.sight == Sight::Card(blue)),
                public_witness
            );
            assert!(
                !game
                    .context_refs(B)
                    .iter()
                    .any(|reference| reference.sight == Sight::Card(red))
            );
            if tracked && public_witness {
                // A predicate over the whole hand keeps its disclosure branch
                // private until the meld completes.
                let expected = tracking::remap::PRIVATE_PROOF;
                assert_eq!(
                    game.sample_world(B, 9).err(),
                    Some(Error::Unsupported(expected))
                );
                assert!(
                    matches!(game.clone().determinize(B,9),Err(Error::Unsupported(why)) if why==expected)
                );
            } else if tracked {
                // A witness test on the operand alone: not revealing it proves only
                // that it is not red, so the opponent samples worlds where the pending
                // meld is never red.
                for seed in 0..16 {
                    let world = game.clone().determinize(B, seed).unwrap();
                    assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
                    let pending = world.frames.iter().find_map(|frame| match frame {
                        Frame::Operation {
                            request: Operation::Move { target, .. },
                            ..
                        } => Some(target.card),
                        _ => None,
                    });
                    let pending = pending.expect("the pending meld");
                    assert_ne!(world.position.card(pending).color, Color::Red);
                }
            }
            game.choose(0).unwrap();
            let Status::Decision(menu) = game.advance(1000, &mut events).unwrap() else {
                panic!("ordinary continuation after meld")
            };
            assert_eq!(menu.player, B);
            assert_eq!(
                game.position.locate(blue),
                Some(Zone::Board(A, Color::Blue))
            );
            assert!(!game.frames.iter().any(|frame|matches!(frame,Frame::Operation{knowledge:Some(known),..} if known.private_query.is_some())));
            if tracked {
                assert!(game.sample_world(B, 9).is_ok());
                let sample = game.clone().determinize(B, 9).unwrap();
                assert_eq!(sample.observe(B).unwrap(), game.observe(B).unwrap());
            }
            assert!(events.iter().any(|event| matches!(
                event,
                Event::Moved(Movement {
                    inside_karma: true,
                    ..
                })
            )));
            assert!(game.position.check_conservation());
        }
    }
}

#[test]
fn pencil_three_return_cap_draws_the_full_packet_before_its_return_phase() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E039");
    let original = [
        (Color::Blue, 2),
        (Color::Red, 1),
        (Color::Green, 1),
        (Color::Purple, 1),
    ]
    .map(|(color, value)| tech(&mut f, color, value, Zone::Hand(A)));
    let drawn = [
        deck(&mut f, Color::Yellow, 3),
        deck(&mut f, Color::Green, 3),
        deck(&mut f, Color::Purple, 3),
    ];
    let retained = drawn[2];
    let mut game = start(f, source, 0, A, false);
    game.shortcuts_disabled = true;
    let mut picks = 0;
    let events = finish(&mut game, |menu| {
        if menu.options.iter().any(|option|matches!(option,Alternative::Move{verb:Verb::Return,target:TargetDescriptor::Known{card,..}} if original.contains(card))) {
            assert!(picks<3,"three returns exhaust the printed quota");let choice=choose_card(menu,original[picks]);picks+=1;choice
        } else {
            menu.options.iter().position(|option| matches!(option,
                Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. }
                if *card != retained)).unwrap_or(0)
        }
    });
    assert_eq!(picks, 3);
    for card in &original[..3] {
        assert!(matches!(game.position.locate(*card), Some(Zone::Deck(_))));
    }
    assert_eq!(game.position.locate(original[3]), Some(Zone::Hand(A)));
    assert_eq!(game.position.cards(Zone::Forecast(A)), [retained]);
    let draw_events: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            Event::Moved(m) if m.verb.is_none() && drawn.contains(&m.card) => Some(index),
            _ => None,
        })
        .collect();
    assert_eq!(draw_events.len(), 3);
    let original_returns: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            Event::Moved(m) if m.verb == Some(Verb::Return) && original.contains(&m.card) => {
                Some(index)
            }
            _ => None,
        })
        .collect();
    assert!(original_returns.iter().all(|index| *index < draw_events[0]));
    assert!(
        events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| match event {
                Event::Moved(m) if m.verb == Some(Verb::Return) && drawn.contains(&m.card) =>
                    Some(index),
                _ => None,
            })
            .all(|index| index > draw_events[2])
    );
}

#[test]
fn positive_identity_intersections_preserve_operand_front_dependencies() {
    const NAMED: Selector = build::filtered(
        &build::HAND,
        &[program::Filter::Has(program::Test::Is(Reg::Bound))],
    );
    const NESTED: Selector = build::filtered(
        &Selector::Union(&[
            Selector::Score(Who::Executor),
            build::filtered(&NAMED, &[program::Filter::Has(program::Test::Values(&[1]))]),
        ]),
        &[program::Filter::Has(program::Test::Set(CardSet::Base))],
    );
    const BLUE: program::Cond = program::Cond::Card(
        Reg::Bound,
        program::Filter::Has(program::Test::Color(Color::Blue)),
    );
    const READS: &[&[Instruction]] = &[
        &[build::return_(NAMED).verify_when(&BLUE)],
        &[build::return_(NESTED).verify_when(&BLUE)],
        &[
            build::choose(Who::Executor, Choice::Card(&NAMED)),
            build::if_(
                &program::Cond::Card(
                    Reg::Chosen,
                    program::Filter::Has(program::Test::Color(Color::Blue)),
                ),
                0,
            ),
        ],
        &[
            build::score(NAMED),
            build::if_(
                &program::Cond::Card(
                    Reg::Last,
                    program::Filter::Has(program::Test::Color(Color::Blue)),
                ),
                0,
            ),
        ],
        &[build::bind(build::filtered(
            &build::HAND,
            &[
                program::Filter::Has(program::Test::Is(Reg::Bound)),
                program::Filter::Has(program::Test::Color(Color::Blue)),
            ],
        ))],
    ];
    for body in READS {
        assert!(
            build::first(On::Move(&[Verb::Return]), Who::Executor, body).body_reads_card_front()
        );
    }
    const BACKS: &[&[Instruction]] = &[
        &[build::return_(NAMED)],
        &[build::return_(build::filtered(
            &build::HAND,
            &[program::Filter::Lacks(program::Test::Is(Reg::Bound))],
        ))
        .verify_when(&BLUE)],
        &[build::bind(build::filtered(
            &build::HAND,
            &[
                program::Filter::Lacks(program::Test::Is(Reg::Bound)),
                program::Filter::Has(program::Test::Color(Color::Blue)),
            ],
        ))],
    ];
    for body in BACKS {
        assert!(
            !build::first(On::Move(&[Verb::Return]), Who::Executor, body).body_reads_card_front()
        );
    }
}

fn pencil_reaction_fixture(
    private_blue: bool,
    literal: bool,
) -> (Game, [CardId; 3], [CardId; 3], CardId) {
    const RETURNS: Slot = Slot::effect(0);
    const DRAWS: Slot = Slot::effect(2);
    const PACKET: Selector = build::filtered(
        &build::HAND,
        &[program::Filter::Has(program::Test::InSlot(DRAWS))],
    );
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E039");
    if literal {
        // Independent printed-order reference: return one card per block
        // pass, then choose the still-live survivor after every reaction.
        f.cards[usize::from(source.0)].effects[0].instructions = vec![
            build::return_(build::HAND)
                .up_to(Some(Expr::Const(3)))
                .keep(RETURNS),
            build::if_(&program::Cond::Did(Outcome::All), 4),
            build::draws_age(
                Expr::Count(&Selector::Slot(RETURNS)),
                Expr::Add(&Expr::Max(&Selector::Slot(RETURNS)), 1),
                Then::Keep,
            )
            .keep(DRAWS),
            build::for_each(
                Domain::Times(Expr::Add(&Expr::Count(&PACKET), -1)),
                Order::Printed,
                None,
                1,
            ),
            build::return_(PACKET),
            build::foreshadow(PACKET),
        ];
    }
    let original = Color::ALL[..3]
        .try_into()
        .map(|colors: [Color; 3]| colors.map(|color| tech(&mut f, color, 1, Zone::Hand(A))))
        .unwrap();
    let packet = [Color::Red, Color::Green, Color::Yellow].map(|color| deck(&mut f, color, 2));
    let figure = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
    f.add(figure.clone(), Zone::Board(A, figure.color));
    // Three initial return reactions occur before the reward packet exists.
    // The fourth draw is the new private information affecting its survivor.
    deck(&mut f, Color::Purple, 8);
    let private = deck(
        &mut f,
        if private_blue {
            Color::Blue
        } else {
            Color::Green
        },
        8,
    );
    for _ in 0..3 {
        deck(&mut f, Color::Red, 8);
    }
    (start(f, source, 0, A, true), original, packet, private)
}

fn pencil_reward_menu(mut game: Game, original: [CardId; 3]) -> (Game, Vec<Event>, Decision) {
    let mut events = Vec::new();
    for card in original {
        let Status::Decision(menu) = game.advance(1000, &mut events).unwrap() else {
            panic!("initial optional returns");
        };
        game.choose(choose_card(&menu, card)).unwrap();
    }
    let Status::Decision(menu) = game.advance(1000, &mut events).unwrap() else {
        panic!("reward return choice");
    };
    (game, events, menu)
}

#[test]
fn pencil_keeps_survivor_open_until_private_first_return_reactions_finish() {
    for private_blue in [false, true] {
        let (game, originals, packet, private) = pencil_reaction_fixture(private_blue, false);
        let (mut game, mut events, menu) = pencil_reward_menu(game, originals);
        assert_eq!(menu.options.len(), 3);
        assert!(
            menu.options.iter().all(|option| matches!(option,
            Alternative::Move { verb: Verb::Return, target: TargetDescriptor::Known { card, .. } }
            if packet.contains(card))),
            "the first choice returns a card, not a precommitted survivor"
        );
        assert!(matches!(game.position.locate(private), Some(Zone::Deck(_))));
        game.choose(choose_card(&menu, packet[0])).unwrap();
        let Status::Decision(next) = game.advance(1000, &mut events).unwrap() else {
            panic!("second return remains a choice");
        };
        assert_eq!(game.position.locate(private), Some(Zone::Hand(A)));
        assert_eq!(next.options.len(), 2);
        assert!(next.options.iter().all(|option| matches!(option,
            Alternative::Move { verb: Verb::Return, target: TargetDescriptor::Known { card, .. } }
            if packet[1..].contains(card))));
        let after_reading_private = if game.position.card(private).color == Color::Blue {
            packet[1]
        } else {
            packet[2]
        };
        let survivor = if private_blue { packet[2] } else { packet[1] };
        game.choose(choose_card(&next, after_reading_private))
            .unwrap();
        events.extend(finish(&mut game, |_| 0));
        assert_eq!(game.position.cards(Zone::Forecast(A)), [survivor]);
        assert!(
            events.iter().any(
                |event| matches!(event, Event::Moved(m) if m.card == private && m.inside_karma)
            )
        );
        assert!(
            revealed(&events).is_empty(),
            "return reaction draws stay private"
        );
    }
}

#[test]
fn pencil_quota_matches_literal_sequential_returns_over_every_reacting_path() {
    for private_blue in [false, true] {
        let (game, originals, _, _) = pencil_reaction_fixture(private_blue, false);
        let (game, events, _) = pencil_reward_menu(game, originals);
        let mut disabled = game.clone();
        disabled.shortcuts_disabled = true;
        let reduced = paths(game, events.clone(), 0);
        let full = paths(disabled, events, 0);
        let (reference, originals, _, _) = pencil_reaction_fixture(private_blue, true);
        let (reference, events, _) = pencil_reward_menu(reference, originals);
        let mut printed = paths(reference, events, 0);
        for leaf in &mut printed {
            // Position compares its catalog too. Only this fixture's source
            // instruction list intentionally differs in the reference run.
            let mut cards = leaf.position.catalog.to_vec();
            cards[0].effects = reduced[0].position.catalog[0].effects.clone();
            assert_eq!(cards.as_slice(), reduced[0].position.catalog.as_ref());
            leaf.position.catalog = cards.into();
        }
        for candidates in [&full, &printed] {
            assert!(
                reduced.iter().all(|leaf| candidates.contains(leaf)),
                "invented printed-order outcome"
            );
            assert!(
                candidates.iter().all(|leaf| reduced.contains(leaf)),
                "lost printed-order outcome"
            );
        }
        assert_eq!(reduced.len(), 6, "all ordered pairs remain with Maxwell");
        assert!(reduced.iter().all(|leaf| {
            leaf.events
                .iter()
                .any(|event| event.contains("inside_karma: true"))
        }));
    }
}
