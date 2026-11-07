//! Age-three printed clauses; generalized splay/Forecast proofs live separately.
use super::*;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn card(f: &mut Fixture, code: &str, zone: Option<Zone>) -> CardId {
    let printed = echoes_card(code).unwrap();
    let color = printed.color;
    f.add(printed, zone.unwrap_or(Zone::Board(A, color)))
}
fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, Vec::new())
}
fn figure(f: &mut Fixture, figure: Figure, owner: Player) -> CardId {
    let printed = figure_card(figure.code()).unwrap();
    let color = printed.color;
    f.add(printed, Zone::Board(owner, color))
}
fn deck(f: &mut Fixture, set: CardSet, age: u8, color: Color) -> CardId {
    let id = tech(f, color, age, Zone::Deck(Deck { set, age }));
    f.cards[usize::from(id.0)].set = set;
    id
}
fn icon(f: &mut Fixture, card: CardId, icon: Icon) {
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(icon);
}
fn start(game: &mut Game, card: CardId, effect: u16, context: Context, increase: u8) {
    let mut boxed = BoxedValues::ZERO;
    boxed.increase(increase).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed,
        },
        context,
    )
    .unwrap();
}
fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut choose: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..150 {
        let mut clone = game.clone();
        let mut next = Vec::new();
        let status = game.advance(1000, &mut next).unwrap();
        let mut expected = Vec::new();
        assert_eq!(clone.advance(1000, &mut expected).unwrap(), status);
        assert_eq!(clone, *game);
        assert_eq!(expected, next);
        events.extend(next);
        match status {
            Status::Decision(menu) => game.choose(choose(&menu)).unwrap(),
            status => {
                assert!(game.position.check_conservation());
                for viewer in Player::ALL {
                    if let Some(tracker) = game.tracker(viewer) {
                        assert!(tracker.check_truth(&game.position));
                    }
                }
                return status;
            }
        }
    }
    panic!("age-three card failed to finish");
}
fn pick(menu: &Decision, card: CardId) -> usize {
    menu.options.iter().position(|option| matches!(option, Alternative::Move { target: TargetDescriptor::Known { card: target, .. }, .. } if *target == card)).unwrap()
}
fn decline(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Decline)
        .unwrap_or(0)
}
fn returned(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Return) && !m.inside_karma => Some(m.card),
            _ => None,
        })
        .collect()
}

#[test]
fn katana_junks_every_standard_supply_card_only_for_one_foreseen_original() {
    for foreseen in [false, true] {
        for count in 0..=2 {
            let mut f = Fixture::default();
            let source = card(&mut f, "E027", foreseen.then_some(Zone::Forecast(A)));
            let promotion = tech(&mut f, Color::Blue, 4, Zone::Hand(A));
            let transferred: Vec<_> = [Color::Yellow, Color::Green]
                .into_iter()
                .take(count)
                .map(|color| {
                    let target = tech(&mut f, color, 1, Zone::Board(B, color));
                    icon(&mut f, target, Icon::Authority);
                    target
                })
                .collect();
            let mut supply = Vec::new();
            for set in [CardSet::Base, CardSet::Base, CardSet::Echoes] {
                let target = tech(&mut f, Color::Purple, 2, Zone::AvailableAchievements);
                f.cards[usize::from(target.0)].set = set;
                supply.push(target);
            }
            let mut game = Game::new(f.position(), Expansions::ECHOES)
                .unwrap()
                .with_tracking()
                .unwrap();
            if foreseen {
                game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                    .unwrap();
            } else {
                let mut context = Context::new(B);
                context.demander = A;
                start(&mut game, source, 0, context, 0);
            }
            finish(&mut game, &mut Vec::new(), |_| 0);
            let mut scored = game.position.cards(Zone::Score(A)).to_vec();
            scored.sort_unstable();
            assert_eq!(scored, transferred);
            for target in supply {
                assert_eq!(
                    game.position.locate(target),
                    Some(if foreseen && count == 1 {
                        Zone::Junk
                    } else {
                        Zone::AvailableAchievements
                    })
                );
            }
            assert_eq!(
                game.position.special_location(Special::Supremacy),
                SpecialLocation::Available
            );
        }
    }
}

#[test]
fn katana_reselects_exposed_same_color_tops_and_nobel_reacts_before_each_transfer() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E027", Some(Zone::Board(B, Color::Red)));
    figure(&mut f, Figure::AlfredNobel, B);
    let lower = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let upper = tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    let untouched = tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
    for target in [lower, upper, untouched] {
        icon(&mut f, target, Icon::Authority);
    }
    let seven = deck(&mut f, CardSet::Base, 7, Color::Yellow);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    start(&mut game, source, 0, context, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    game.choose(pick(&menu, upper)).unwrap();
    let menu = decision(&mut game, &mut events);
    game.choose(pick(&menu, lower)).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.cards(Zone::Score(B)), [upper, lower]);
    assert_eq!(
        game.position.locate(untouched),
        Some(Zone::Board(A, Color::Blue))
    );
    assert_eq!(game.position.locate(seven), Some(Zone::Junk));
    let junk = events
        .iter()
        .position(|event| matches!(event, Event::Moved(m) if m.card == seven && m.inside_karma))
        .unwrap();
    let transfer = events.iter().position(|event| matches!(event, Event::Moved(m) if m.card == upper && m.verb == Some(Verb::Transfer))).unwrap();
    assert!(junk < transfer);
}

#[test]
fn deodorant_checks_printed_top_icons_and_keeps_else_draw_in_hand() {
    for authority in [false, true] {
        for industry in [false, true] {
            for increase in [0, 1] {
                let mut f = Fixture::default();
                let source = card(&mut f, "E028", None);
                let buried = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
                icon(&mut f, buried, Icon::Authority);
                let top = tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
                if authority {
                    icon(&mut f, top, Icon::Authority);
                }
                let drawn = deck(&mut f, CardSet::Base, 3 + increase, Color::Blue);
                if industry {
                    icon(&mut f, drawn, Icon::Industry);
                }
                let four = deck(&mut f, CardSet::Base, 4 + increase, Color::Purple);
                let other = deck(&mut f, CardSet::Base, 4 + increase, Color::Green);
                let echo = deck(&mut f, CardSet::Echoes, 4 + increase, Color::Yellow);
                let mut position = f.position();
                position.players[A.index()].splays[Color::Red as usize] = Splay::Right;
                let mut game = Game::new(position, Expansions::ECHOES).unwrap();
                start(&mut game, source, 0, Context::new(A), increase);
                finish(&mut game, &mut Vec::new(), |_| 0);
                if authority {
                    assert_eq!(
                        game.position.locate(drawn),
                        Some(Zone::Board(A, Color::Blue))
                    );
                } else {
                    assert_eq!(
                        game.position.locate(drawn),
                        Some(Zone::Deck(Deck::base(3 + increase)))
                    );
                    assert_eq!(game.position.locate(other), Some(Zone::Hand(A)));
                }
                start(&mut game, source, 1, Context::new(A), increase);
                finish(&mut game, &mut Vec::new(), |_| 0);
                assert_eq!(
                    game.position.locate(four),
                    Some(if authority && industry {
                        Zone::Junk
                    } else {
                        Zone::Deck(Deck::base(4 + increase))
                    })
                );
                assert_eq!(
                    game.position.locate(echo),
                    Some(Zone::Deck(Deck {
                        set: CardSet::Echoes,
                        age: 4 + increase
                    }))
                );
            }
        }
    }
}

#[test]
fn deodorant_echo_changes_authority_then_dogma_changes_industry_before_junk() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E028", None);
    let second = deck(&mut f, CardSet::Base, 3, Color::Red);
    icon(&mut f, second, Icon::Industry);
    let first = deck(&mut f, CardSet::Echoes, 3, Color::Red);
    icon(&mut f, first, Icon::Authority);
    let fourth = deck(&mut f, CardSet::Base, 4, Color::Purple);
    let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.cards(Zone::Board(A, Color::Red)),
        [first, second]
    );
    assert_eq!(game.position.locate(fourth), Some(Zone::Junk));
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(
        events
            .iter()
            .filter_map(|event| match event {
                Event::Moved(m) if m.verb == Some(Verb::Meld) => Some(m.card),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [first, second]
    );
}

#[test]
fn deodorant_echo_is_a_boxed_draw_and_meld_with_sneferus_actual_first() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E028", None);
        figure(&mut f, Figure::Sneferu, A);
        let tuck = deck(&mut f, CardSet::Base, 2, Color::Green);
        icon(&mut f, tuck, Icon::Authority);
        let melded = deck(&mut f, CardSet::Base, 3 + increase, Color::Red);
        if increase > 0 {
            deck(&mut f, CardSet::Base, 3, Color::Blue);
        }
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, Card::ECHO, Context::new(A), increase);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(melded),
            Some(Zone::Board(A, Color::Red))
        );
        assert_eq!(
            game.position.locate(tuck),
            Some(Zone::Board(A, Color::Green))
        );
        assert!(events.iter().any(|event| matches!(event, Event::Moved(m) if m.card == tuck && m.verb == Some(Verb::Tuck) && m.inside_karma)));
    }
}

#[test]
fn sandpaper_finishes_successful_returns_before_rewards_and_excludes_maxwells_draws() {
    for count in 0..=2 {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E029", None);
            let originals = [
                tech(&mut f, Color::Red, 1, Zone::Hand(A)),
                tech(&mut f, Color::Green, 2, Zone::Hand(A)),
            ];
            let rewards = [
                deck(&mut f, CardSet::Base, 3, Color::Blue),
                deck(&mut f, CardSet::Base, 3, Color::Purple),
            ];
            let mut eights = Vec::new();
            if reacting {
                figure(&mut f, Figure::JamesClerkMaxwell, B);
                for _ in 0..2 {
                    eights.push(deck(&mut f, CardSet::Base, 8, Color::Red));
                }
            }
            let mut game = Game::new(f.position(), BOTH).unwrap();
            start(&mut game, source, 0, Context::new(A), 0);
            let mut chosen = 0;
            let mut events = Vec::new();
            finish(&mut game, &mut events, |menu| {
                if chosen < count {
                    let choice = pick(menu, originals[chosen]);
                    chosen += 1;
                    choice
                } else {
                    decline(menu)
                }
            });
            assert_eq!(returned(&events), originals[..count]);
            assert_eq!(game.position.cards(Zone::Hand(A)).len(), 2);
            assert_eq!(
                rewards
                    .iter()
                    .filter(|&&target| game.position.locate(target) == Some(Zone::Hand(A)))
                    .count(),
                count
            );
            if count > 0 {
                let last_return = events.iter().rposition(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Return) && !m.inside_karma)).unwrap();
                let first_reward = events
                    .iter()
                    .position(|event| matches!(event, Event::Moved(m) if rewards.contains(&m.card)))
                    .unwrap();
                assert!(last_return < first_reward);
            }
            if reacting {
                assert_eq!(
                    eights
                        .iter()
                        .filter(|&&target| game.position.locate(target) == Some(Zone::Hand(B)))
                        .count(),
                    count
                );
                assert_eq!(events.iter().filter(|event| matches!(event, Event::Moved(m) if eights.contains(&m.card) && m.inside_karma)).count(), count);
            }
        }
    }
}

#[test]
fn sandpaper_replaced_returns_earn_no_draw_and_still_has_real_meld_choices() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E029", None);
    figure(&mut f, Figure::NapoleonBonaparte, A);
    let returned = tech(&mut f, Color::Green, 1, Zone::Hand(A));
    let reward = deck(&mut f, CardSet::Base, 3, Color::Purple);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    finish(&mut game, &mut Vec::new(), |_| 0);
    assert_eq!(
        game.position.locate(returned),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(
        game.position.locate(reward),
        Some(Zone::Deck(Deck::base(3)))
    );

    let mut f = Fixture::default();
    let source = card(&mut f, "E029", None);
    let first = tech(&mut f, Color::Red, 1, Zone::Hand(A));
    let second = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
    start(&mut game, source, 1, Context::new(A), 0);
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options.len(), 2);
    game.choose(pick(&menu, second)).unwrap();
    finish(&mut game, &mut Vec::new(), |_| 0);
    assert_eq!(game.position.locate(first), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(second),
        Some(Zone::Board(A, Color::Blue))
    );
}

#[test]
fn sandpaper_native_foreseen_melds_one_then_foreshadows_the_remaining_hand() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E029", foreseen.then_some(Zone::Forecast(A)));
        let promotion = tech(&mut f, Color::Purple, 4, Zone::Hand(A));
        let first = tech(&mut f, Color::Red, 1, Zone::Hand(A));
        let second = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        if foreseen {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, 2, Context::new(A), 0);
        }
        finish(&mut game, &mut Vec::new(), |menu| {
            if menu.options.contains(&Alternative::Decline) {
                decline(menu)
            } else {
                pick(menu, first)
            }
        });
        assert_eq!(
            game.position.locate(second),
            Some(if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Hand(A)
            })
        );
        assert_eq!(
            game.position.locate(first),
            Some(if foreseen {
                Zone::Board(A, Color::Red)
            } else {
                Zone::Hand(A)
            })
        );
    }
}

#[test]
fn homing_pigeons_matches_values_across_sets_without_consuming_the_hand_witness() {
    for matching in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E031", None);
        let witness = tech(
            &mut f,
            Color::Purple,
            if matching { 2 } else { 3 },
            Zone::Hand(A),
        );
        let base = tech(&mut f, Color::Red, 2, Zone::Score(B));
        let echo = tech(&mut f, Color::Yellow, 2, Zone::Score(B));
        f.cards[usize::from(echo.0)].set = CardSet::Echoes;
        let wrong = tech(&mut f, Color::Blue, 1, Zone::Score(B));
        // The executor's hand must not supply an extra matching value.
        tech(&mut f, Color::Purple, 1, Zone::Hand(B));
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut context = Context::new(B);
        context.demander = A;
        start(&mut game, source, 0, context, 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        for target in [base, echo] {
            assert_eq!(
                game.position.locate(target),
                Some(if matching {
                    Zone::Deck(game.position.card(target).origin_deck())
                } else {
                    Zone::Score(B)
                })
            );
        }
        assert_eq!(game.position.locate(wrong), Some(Zone::Score(B)));
        assert_eq!(game.position.locate(witness), Some(Zone::Hand(A)));
        assert_eq!(returned(&events).len(), if matching { 2 } else { 0 });
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Revealed { .. }))
        );
    }
}

#[test]
fn homing_pigeons_quota_retains_real_subsets_and_maxwell_returns_are_original_only() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E031", None);
    figure(&mut f, Figure::JamesClerkMaxwell, A);
    for age in [1, 2, 3] {
        tech(&mut f, Color::Purple, age, Zone::Hand(A));
    }
    let scored: Vec<_> = [1, 2, 3]
        .into_iter()
        .map(|age| tech(&mut f, Color::Red, age, Zone::Score(B)))
        .collect();
    let eights = [
        deck(&mut f, CardSet::Base, 8, Color::Red),
        deck(&mut f, CardSet::Base, 8, Color::Blue),
    ];
    let mut game = Game::new(f.position(), BOTH).unwrap();
    let mut context = Context::new(B);
    context.demander = A;
    start(&mut game, source, 0, context, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3);
    // The demanded player chooses from their own inspectable score pile.
    game.choose(pick(&menu, scored[2])).unwrap();
    let menu = decision(&mut game, &mut events);
    game.choose(pick(&menu, scored[1])).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.cards(Zone::Score(B)), [scored[0]]);
    assert_eq!(returned(&events), [scored[2], scored[1]]);
    for eight in eights {
        assert_eq!(game.position.locate(eight), Some(Zone::Hand(A)));
    }
    assert!(events.iter().any(
        |event| matches!(event, Event::Moved(m) if eights.contains(&m.card) && m.inside_karma)
    ));
}

#[test]
fn homing_pigeons_foreseen_splays_all_colors_even_after_decline() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        for color in [Color::Red, Color::Green, Color::Blue] {
            for _ in 0..2 {
                tech(&mut f, color, 1, Zone::Board(A, color));
            }
        }
        let source = card(&mut f, "E031", foreseen.then_some(Zone::Forecast(A)));
        let promotion = tech(&mut f, Color::Purple, 4, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        if foreseen {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, 1, Context::new(A), 0);
        }
        finish(&mut game, &mut Vec::new(), decline);
        for color in [Color::Red, Color::Green, Color::Blue] {
            assert_eq!(
                game.position.splay(A, color),
                if foreseen { Splay::Left } else { Splay::None }
            );
        }
    }
}

#[test]
fn almanac_echo_resolves_age_from_base_before_explicit_echoes_and_respects_capacity() {
    for increase in [0, 1] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E032", None);
            // Tied highest tops keep ordinary draws in Base; the printed
            // explicit Echoes draw must select Echoes regardless of that tie.
            let source_color = f.cards[usize::from(source.0)].color;
            let tied_color = Color::ALL
                .into_iter()
                .find(|&color| color != source_color)
                .unwrap();
            tech(&mut f, tied_color, 3, Zone::Board(A, tied_color));
            if full {
                for _ in 0..5 {
                    tech(&mut f, Color::Red, 1, Zone::Forecast(A));
                }
            }
            let base = deck(&mut f, CardSet::Base, 5 + increase, Color::Purple);
            let wrong = deck(&mut f, CardSet::Echoes, 4 + increase, Color::Blue);
            let drawn = deck(&mut f, CardSet::Echoes, 5 + increase, Color::Green);
            if increase > 0 {
                deck(&mut f, CardSet::Base, 4, Color::Red);
            }
            let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
            start(&mut game, source, Card::ECHO, Context::new(A), increase);
            finish(&mut game, &mut Vec::new(), |_| 0);
            assert_eq!(
                game.position.locate(drawn),
                Some(if full {
                    Zone::Hand(A)
                } else {
                    Zone::Forecast(A)
                })
            );
            assert_eq!(
                game.position.locate(base),
                Some(Zone::Deck(Deck::base(5 + increase)))
            );
            assert_eq!(
                game.position.locate(wrong),
                Some(Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 4 + increase
                }))
            );
        }
    }
}

#[test]
fn almanac_returns_only_a_bonus_and_uses_printed_bonus_including_a_figure() {
    for napoleon in [false, true] {
        for pass in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E032", None);
            let bonus = if napoleon {
                f.add(
                    figure_card(Figure::NapoleonBonaparte.code()).unwrap(),
                    Zone::Forecast(A),
                )
            } else {
                let target = tech(&mut f, Color::Purple, 1, Zone::Forecast(A));
                f.cards[usize::from(target.0)].set = CardSet::Echoes;
                f.cards[usize::from(target.0)].symbols[0] = Symbol::Bonus(3);
                target
            };
            let plain = tech(&mut f, Color::Red, 1, Zone::Forecast(A));
            let reward = deck(
                &mut f,
                CardSet::Base,
                if napoleon { 7 } else { 4 },
                Color::Green,
            );
            if !napoleon {
                deck(&mut f, CardSet::Base, 2, Color::Blue);
            }
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, source, 0, Context::new(A), 1);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options.len(), 2);
            game.choose(if pass {
                decline(&menu)
            } else {
                pick(&menu, bonus)
            })
            .unwrap();
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(game.position.locate(plain), Some(Zone::Forecast(A)));
            assert_eq!(
                game.position.locate(bonus),
                Some(if pass {
                    Zone::Forecast(A)
                } else {
                    Zone::Deck(game.position.card(bonus).origin_deck())
                })
            );
            assert_eq!(
                game.position.locate(reward),
                Some(if pass {
                    Zone::Deck(game.position.card(reward).origin_deck())
                } else {
                    Zone::Score(A)
                })
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, Event::Revealed { card, .. } if *card == bonus))
                    .count(),
                usize::from(!pass)
            );
        }
    }
}

#[test]
fn almanac_verifies_only_chosen_bonus_before_maxwell_and_keeps_other_backs_private() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E032", None);
    figure(&mut f, Figure::JamesClerkMaxwell, B);
    let cards: Vec<_> = (0..3)
        .map(|_| tech(&mut f, Color::Purple, 1, Zone::Forecast(A)))
        .collect();
    for &target in &cards {
        f.cards[usize::from(target.0)].set = CardSet::Echoes;
    }
    f.cards[usize::from(cards[0].0)].symbols[0] = Symbol::Bonus(3);
    let reward = deck(&mut f, CardSet::Base, 4, Color::Green);
    let karma = deck(&mut f, CardSet::Base, 8, Color::Red);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    game.choose(pick(&menu, cards[0])).unwrap();
    for _ in 0..1000 {
        if !events.is_empty() {
            break;
        }
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    assert!(matches!(events.first(), Some(Event::Revealed { card, .. }) if *card == cards[0]));
    assert!(
        game.tracker(B)
            .unwrap()
            .knows(&game.position, cards[0], Zone::Forecast(A))
    );
    for &target in &cards[1..] {
        assert!(
            !game
                .tracker(B)
                .unwrap()
                .knows(&game.position, target, Zone::Forecast(A))
        );
    }
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(reward), Some(Zone::Score(A)));
    assert_eq!(game.position.locate(karma), Some(Zone::Hand(B)));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Revealed { .. }))
            .count(),
        1
    );
    let draw = events
        .iter()
        .position(|event| matches!(event, Event::Moved(m) if m.card == karma && m.inside_karma))
        .unwrap();
    let original = events.iter().position(|event| matches!(event, Event::Moved(m) if m.card == cards[0] && m.verb == Some(Verb::Return))).unwrap();
    assert!(draw < original);
}

#[test]
fn almanac_replaced_return_does_not_reward_and_empty_optional_return_reveals_nothing() {
    for replaced in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E032", None);
        let forecast = tech(&mut f, Color::Green, 1, Zone::Forecast(A));
        f.cards[usize::from(forecast.0)].set = CardSet::Echoes;
        if replaced {
            f.cards[usize::from(forecast.0)].symbols[0] = Symbol::Bonus(3);
            figure(&mut f, Figure::NapoleonBonaparte, A);
        }
        let reward = deck(&mut f, CardSet::Base, 4, Color::Purple);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(reward),
            Some(Zone::Deck(Deck::base(4)))
        );
        assert_eq!(
            game.position.locate(forecast),
            Some(if replaced {
                Zone::Board(A, Color::Green)
            } else {
                Zone::Forecast(A)
            })
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Revealed { .. }))
                .count(),
            usize::from(replaced)
        );
    }
}

#[test]
fn almanac_native_foreseen_foreshadows_opponents_cards_after_its_own_echo() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E032", foreseen.then_some(Zone::Forecast(A)));
        let promotion = tech(&mut f, Color::Red, 5, Zone::Hand(A));
        let opponent = tech(&mut f, Color::Green, 2, Zone::Forecast(B));
        let own = deck(&mut f, CardSet::Echoes, 4, Color::Purple);
        deck(&mut f, CardSet::Base, 4, Color::Purple);
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        if foreseen {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, 1, Context::new(A), 0);
        }
        finish(&mut game, &mut Vec::new(), decline);
        assert_eq!(
            game.position.locate(opponent),
            Some(Zone::Forecast(if foreseen { A } else { B }))
        );
        assert_eq!(
            game.position.locate(own),
            Some(if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 4,
                })
            })
        );
    }
}

#[test]
fn sunglasses_echo_reveals_only_its_selected_matching_color_or_the_failed_hand() {
    for tracked in [false, true] {
        for revoked in [false, true] {
            for matching in [false, true] {
                let mut f = Fixture::default();
                let source = card(&mut f, "E035", None);
                for _ in 0..2 {
                    tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
                }
                let chosen_color = if matching { Color::Red } else { Color::Green };
                let chosen = tech(&mut f, chosen_color, 1, Zone::Hand(A));
                let alternative = tech(&mut f, chosen_color, 1, Zone::Hand(A));
                let others = [
                    tech(&mut f, Color::Green, 1, Zone::Hand(A)),
                    tech(&mut f, Color::Blue, 1, Zone::Hand(A)),
                ];
                let hand = [chosen, alternative, others[0], others[1]];
                let mut position = f.position();
                position.players[A.index()].splays[Color::Red as usize] = Splay::Right;
                position.hand_grants[A.index()][A.index()] = !revoked;
                let mut game = Game::new(position, Expansions::ECHOES).unwrap();
                if tracked {
                    game = game.with_tracking().unwrap();
                }
                start(&mut game, source, Card::ECHO, Context::new(A), 0);
                let mut events = Vec::new();
                if matching {
                    let menu = decision(&mut game, &mut events);
                    assert_eq!(
                        menu.options.as_slice(),
                        [chosen, alternative].map(|card| Alternative::Move {
                            verb: Verb::Score,
                            target: TargetDescriptor::Known {
                                card,
                                source: Zone::Hand(A)
                            },
                        })
                    );
                    if revoked {
                        assert_eq!(
                            events
                                .iter()
                                .filter_map(|event| match event {
                                    Event::Revealed { card, .. } => Some(*card),
                                    _ => None,
                                })
                                .collect::<Vec<_>>(),
                            hand
                        );
                    } else {
                        assert!(events.is_empty());
                    }
                    if tracked && revoked {
                        for seed in 0..16 {
                            for mut sample in [
                                game.determinize(A, seed).unwrap(),
                                game.sample_world(A, seed).unwrap().world,
                            ] {
                                let sampled_menu = decision(&mut sample, &mut Vec::new());
                                assert_eq!(sampled_menu.options, menu.options);
                                sample.choose(pick(&sampled_menu, alternative)).unwrap();
                                finish(&mut sample, &mut Vec::new(), |_| 0);
                                assert_eq!(sample.position.cards(Zone::Score(A)), [alternative]);
                                assert_eq!(sample.position.locate(chosen), Some(Zone::Hand(A)));
                            }
                        }
                    }
                    game.choose(pick(&menu, chosen)).unwrap();
                }
                finish(&mut game, &mut events, |_| 0);
                assert_eq!(
                    game.position.locate(chosen),
                    Some(if matching {
                        Zone::Score(A)
                    } else {
                        Zone::Hand(A)
                    })
                );
                let revealed: Vec<_> = events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Revealed { card, .. } => Some(*card),
                        _ => None,
                    })
                    .collect();
                if matching && !revoked {
                    assert_eq!(revealed, [chosen]);
                    if tracked {
                        assert!(game.tracker(B).unwrap().knows(
                            &game.position,
                            chosen,
                            Zone::Score(A)
                        ));
                        for target in [alternative, others[0], others[1]] {
                            assert!(!game.tracker(B).unwrap().knows(
                                &game.position,
                                target,
                                Zone::Hand(A)
                            ));
                        }
                    }
                } else {
                    let mut identities = revealed;
                    identities.sort_unstable();
                    identities.dedup();
                    assert_eq!(identities, hand);
                    if tracked {
                        for target in hand {
                            assert!(game.tracker(B).unwrap().knows(
                                &game.position,
                                target,
                                game.position.locate(target).unwrap()
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn sunglasses_authored_copy_choices_keep_both_directions_and_decline() {
    for direction in [Splay::Left, Splay::Right, Splay::Up, Splay::Aslant] {
        for to_purple in [false, true] {
            let mut f = Fixture::default();
            for color in [Color::Red, Color::Purple] {
                tech(&mut f, color, 1, Zone::Board(A, color));
            }
            tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
            let source = card(&mut f, "E035", None);
            let mut position = f.position();
            position.players[A.index()].splays
                [if to_purple { Color::Red } else { Color::Purple } as usize] = direction;
            let mut game = Game::new(position, Expansions::ECHOES).unwrap();
            start(&mut game, source, 0, Context::new(A), 0);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            let target = if to_purple { Color::Purple } else { Color::Red };
            assert_eq!(
                menu.options.as_slice(),
                [
                    Alternative::Splay {
                        player: A,
                        color: target,
                        direction
                    },
                    Alternative::Decline
                ]
            );
            game.choose(0).unwrap();
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(game.position.splay(A, target), direction);
        }
    }
}
