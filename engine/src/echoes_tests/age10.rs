//! Age-ten clauses, live board sweeps and Forecast return order.
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
    panic!("age-ten card failed to finish");
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
fn camcorder_transfers_the_whole_hand_then_draws_even_when_empty() {
    for count in 0..=3 {
        let mut f = Fixture::default();
        let source = card(&mut f, "E096", Some(Zone::Junk));
        let hand: Vec<_> = (0..count)
            .map(|i| tech(&mut f, Color::Blue, i + 1, Zone::Hand(A)))
            .collect();
        let nine = deck(&mut f, CardSet::Base, 9, Color::Red);
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut context = Context::new(A);
        context.demander = B;
        start(&mut game, source, 0, context, 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Hand(A)), [nine]);
        assert_eq!(game.position.cards(Zone::Hand(B)), hand);
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
        let draw = events
            .iter()
            .position(|e| matches!(e,Event::Moved(m) if m.card==nine))
            .unwrap();
        assert!(
            events
                .iter()
                .skip(draw + 1)
                .all(|e| !matches!(e,Event::Moved(m) if m.verb==Some(Verb::Transfer)))
        );
    }
}

#[test]
fn camcorder_melds_boxed_nines_before_returning_the_whole_remainder_and_drawing_three() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E096", Some(Zone::Junk));
        let candidates: [CardId; 2] =
            [Color::Red, Color::Blue].map(|c| tech(&mut f, c, 9 + increase, Zone::Hand(A)));
        let other = tech(&mut f, Color::Purple, 8, Zone::Hand(A));
        let draws: Vec<_> = (0..3)
            .map(|_| deck(&mut f, CardSet::Base, 9 + increase, Color::Yellow))
            .collect();
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        start(&mut game, source, 1, Context::new(A), increase);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        for (id, color) in candidates.into_iter().zip([Color::Red, Color::Blue]) {
            assert_eq!(game.position.locate(id), Some(Zone::Board(A, color)));
        }
        assert_eq!(game.position.locate(other), Some(Zone::Deck(Deck::base(8))));
        assert_eq!(game.position.cards(Zone::Hand(A)).len(), 3);
        assert!(
            draws
                .iter()
                .all(|&id| game.position.locate(id) == Some(Zone::Hand(A)))
        );
        let returned = events
            .iter()
            .position(|e| matches!(e,Event::Moved(m) if m.card==other&&m.verb==Some(Verb::Return)))
            .unwrap();
        assert!(
            events[..returned]
                .iter()
                .filter(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Meld)))
                .count()
                == 2
        );
        assert!(
            !events[..returned]
                .iter()
                .any(|e| matches!(e,Event::Moved(m) if draws.contains(&m.card)))
        );
    }
}

#[test]
fn social_networking_offers_every_native_standard_type_and_exhausts_live_prefixes() {
    for age_zero in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E097", Some(Zone::Junk));
        let stop = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        f.cards[usize::from(stop.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        let middle = tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
        let top = tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
        let unrelated = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        f.cards[usize::from(unrelated.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        if age_zero {
            tech(&mut f, Color::Yellow, 0, Zone::Deck(Deck::base(0)));
        }
        let position = f.position();
        let mut game = Game::new(position, BOTH).unwrap().with_tracking().unwrap();
        let mut context = Context::new(A);
        context.demander = B;
        start(&mut game, source, 0, context, 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options,
            game.position
                .standard_icons()
                .map(Alternative::Icon)
                .collect::<Vec<_>>()
        );
        assert_eq!(menu.options.len(), if age_zero { 8 } else { 7 });
        assert!(!menu.options.contains(&Alternative::BonusIcon));
        let choice = menu
            .options
            .iter()
            .position(|a| *a == Alternative::Icon(Icon::Concept))
            .unwrap();
        game.choose(choice).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Score(B)), [top, middle]);
        assert_eq!(game.position.cards(Zone::Board(A, Color::Blue)), [stop]);
        assert_eq!(
            game.position.locate(unrelated),
            Some(Zone::Board(A, Color::Red))
        );
    }
}

#[test]
fn social_networking_echo_excludes_red_and_win_requires_three_strict_comparisons() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E097", Some(Zone::Junk));
    let red = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let blue = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
    let mut game = Game::new(f.position(), BOTH).unwrap();
    start(&mut game, source, Card::ECHO, Context::new(A), 0);
    finish(&mut game, &mut Vec::new(), |_| 0);
    assert_eq!(game.position.locate(blue), Some(Zone::Score(A)));
    assert_eq!(game.position.locate(red), Some(Zone::Board(A, Color::Red)));
    for equality in [
        None,
        Some(Icon::Industry),
        Some(Icon::Prosperity),
        Some(Icon::Authority),
    ] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E097", Some(Zone::Junk));
        let other = tech(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
        f.cards[usize::from(other.0)].symbols = [
            Symbol::Icon(Icon::Industry),
            Symbol::Icon(Icon::Prosperity),
            Symbol::Icon(Icon::Authority),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ];
        if let Some(icon) = equality {
            let own = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
            f.cards[usize::from(own.0)].symbols[0] = Symbol::Icon(icon);
        }
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 1, Context::new(A), 0);
        assert_eq!(
            finish(&mut game, &mut Vec::new(), |_| 0),
            if equality.is_none() {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
    }
}

#[test]
fn mp3_scores_its_echo_and_offers_own_and_figure_bonuses_as_unboxed_values() {
    for napoleon in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E099", None);
        let napoleon_id = napoleon.then(|| figure(&mut f, Figure::NapoleonBonaparte, A));
        deck(&mut f, CardSet::Base, 10, Color::Purple);
        let ten = deck(&mut f, CardSet::Base, 10, Color::Blue);
        let six = deck(&mut f, CardSet::Base, 6, Color::Green);
        let eleven = deck(&mut f, CardSet::Base, 11, Color::Purple);
        figure(&mut f, Figure::Archimedes, B);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 1);
        let mut events = Vec::new();
        if napoleon {
            let menu = decision(&mut game, &mut events);
            assert!(menu.options.contains(&Alternative::Value(6)));
            game.choose(
                menu.options
                    .iter()
                    .position(|a| *a == Alternative::Value(6))
                    .unwrap(),
            )
            .unwrap();
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.cards(Zone::Score(A)),
            [napoleon_id.unwrap_or(ten)]
        );
        assert_eq!(
            game.position.locate(eleven),
            Some(Zone::Deck(Deck::base(11)))
        );
        if napoleon {
            assert_eq!(
                game.position.locate(six),
                Some(Zone::Board(A, Color::Green))
            );
            assert!(events.iter().any(|e| matches!(e, Event::Moved(m) if m.card == six && m.inside_karma && m.verb == Some(Verb::Meld))));
        }
        start(&mut game, source, Card::ECHO, Context::new(A), 1);
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.locate(eleven), Some(Zone::Score(A)));
    }
}

#[test]
fn mp3_claim_quota_counts_successful_original_returns_and_keeps_high_age_choices() {
    for count in 0..=2 {
        let mut f = Fixture::default();
        let source = card(&mut f, "E099", None);
        for _ in 0..3 {
            tech(&mut f, Color::Green, 10, Zone::Score(A));
        }
        let hands: Vec<_> = (0..count)
            .map(|i| tech(&mut f, Color::Red, 8 + i, Zone::Hand(A)))
            .collect();
        let achievements: Vec<_> = (1..=5)
            .map(|i| tech(&mut f, Color::Blue, i, Zone::AvailableAchievements))
            .collect();
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 1, Context::new(A), 0);
        let mut events = Vec::new();
        let mut menus = Vec::new();
        finish(&mut game, &mut events, |menu| {
            menus.push(menu.options.clone());
            if menu.options.iter().any(|a| {
                matches!(
                    a,
                    Alternative::Move {
                        verb: Verb::Return,
                        ..
                    }
                )
            }) {
                0
            } else {
                menu.options
                    .iter()
                    .rposition(|a| matches!(a, Alternative::BlindSelect { .. }))
                    .unwrap_or(0)
            }
        });
        assert_eq!(returned(&events).len(), usize::from(count));
        assert!(
            hands
                .iter()
                .all(|&id| matches!(game.position.locate(id), Some(Zone::Deck(_))))
        );
        assert_eq!(
            game.position.cards(Zone::Achievements(A)).len(),
            usize::from(count) * 2
        );
        if count > 0 {
            assert_eq!(
                game.position.locate(achievements[4]),
                Some(Zone::Achievements(A)),
                "limited claim quota must retain which ages are claimed: {menus:?}"
            );
            assert!(menus.iter().any(|m| {
                m.iter()
                    .filter(|a| matches!(a, Alternative::BlindSelect { .. }))
                    .count()
                    == 5
            }));
        } else {
            assert!(menus.is_empty());
        }
    }
}

#[test]
fn gps_demand_keeps_owner_visible_same_deck_return_order_and_echoes_set_backs() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E100", Some(Zone::Junk));
    let first = tech(&mut f, Color::Red, 3, Zone::Forecast(A));
    let second = tech(&mut f, Color::Blue, 3, Zone::Forecast(A));
    let echo = tech(&mut f, Color::Green, 3, Zone::Forecast(A));
    f.cards[usize::from(echo.0)].set = CardSet::Echoes;
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    start(&mut game, source, 0, context, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert!(menu.options.iter().all(|a| matches!(
        a,
        Alternative::Move {
            target: TargetDescriptor::Known { .. },
            ..
        }
    )));
    game.choose(pick(&menu, second)).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.cards(Zone::Deck(Deck::base(3))),
        [first, second]
    );
    assert_eq!(
        game.position.cards(Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 3
        })),
        [echo]
    );
    assert!(game.position.cards(Zone::Forecast(A)).is_empty());
    assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
}

#[test]
fn gps_native_foreseen_draws_all_three_before_forecast_admission_and_keeps_capacity_choices() {
    for foreseen in [false, true] {
        for occupied in [0, 4, 5] {
            let mut f = Fixture::default();
            let mut printed = echoes_card("E100").unwrap();
            let color = printed.color;
            printed.effects = vec![printed.effects[2].clone()];
            let source = f.add(
                printed,
                if foreseen {
                    Zone::Forecast(A)
                } else {
                    Zone::Board(A, color)
                },
            );
            let driver = tech(&mut f, Color::Blue, 10, Zone::Hand(A));
            for _ in 0..occupied {
                tech(&mut f, Color::Red, 11, Zone::Forecast(A));
            }
            let draws: Vec<_> = (0..3)
                .map(|i| deck(&mut f, CardSet::Base, 11, Color::ALL[i]))
                .collect();
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            if foreseen {
                game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                    .unwrap();
            } else {
                start(&mut game, source, 0, Context::new(A), 0);
            }
            let mut events = Vec::new();
            finish(&mut game, &mut events, |menu| {
                menu.options.iter().position(|a|matches!(a,Alternative::Select{target:TargetDescriptor::Known{card,..}} if *card==source)).unwrap_or(0)
            });
            assert_eq!(
                draws
                    .iter()
                    .filter(|&&id| game.position.locate(id) == Some(Zone::Forecast(A)))
                    .count(),
                if foreseen { (5 - occupied).min(3) } else { 0 }
            );
            let drawn: Vec<_> = events
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    matches!(e,Event::Moved(m) if draws.contains(&m.card)&&m.verb.is_none())
                        .then_some(i)
                })
                .collect();
            assert_eq!(drawn.len(), 3);
            if let Some(first_forecast)=events.iter().position(|e|matches!(e,Event::Moved(m) if draws.contains(&m.card)&&m.verb==Some(Verb::Foreshadow))) {assert!(*drawn.last().unwrap()<first_forecast);}
        }
    }
}

#[test]
fn age_ten_catalog_has_every_printed_clause() {
    for (code, dogmas, echo) in [
        ("E096", 2, false),
        ("E097", 2, true),
        ("E099", 2, true),
        ("E100", 3, false),
        ("E104", 2, true),
    ] {
        let card = echoes_card(code).unwrap();
        assert_eq!(card.effects.len(), dogmas, "{code}");
        assert_eq!(card.echo.is_some(), echo, "{code}");
        assert!(
            card.effects.iter().all(|e| !e.instructions.is_empty()),
            "{code}"
        );
    }
}

#[test]
fn camcorder_foreseen_melds_nines_and_keeps_the_other_cards_without_drawing() {
    let mut f = Fixture::default();
    let mut printed = echoes_card("E096").unwrap();
    printed.effects = vec![printed.effects[1].clone()];
    let source = f.add(printed, Zone::Forecast(A));
    let driver = tech(&mut f, Color::Purple, 10, Zone::Hand(A));
    let nine = tech(&mut f, Color::Blue, 9, Zone::Hand(A));
    let other = tech(&mut f, Color::Green, 8, Zone::Hand(A));
    let untouched = deck(&mut f, CardSet::Base, 9, Color::Yellow);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events, |menu| {
        menu.options.iter().position(|a|matches!(a,Alternative::Select{target:TargetDescriptor::Known{card,..}} if *card==source)).unwrap_or(0)
    });
    assert_eq!(
        game.position.locate(source),
        Some(Zone::Board(A, Color::Red))
    );
    assert_eq!(
        game.position.locate(nine),
        Some(Zone::Board(A, Color::Blue))
    );
    assert_eq!(game.position.locate(other), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(untouched),
        Some(Zone::Deck(Deck::base(9)))
    );
    assert!(returned(&events).is_empty());
}

#[test]
fn mp3_absent_bonus_is_no_draw_and_bonus_twelve_uses_exhaustion() {
    for bonus in [None, Some(12)] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E099", Some(Zone::Junk));
        let own = tech(&mut f, Color::Blue, 10, Zone::Board(A, Color::Blue));
        if let Some(bonus) = bonus {
            f.cards[usize::from(own.0)].symbols[0] = Symbol::Bonus(bonus);
        }
        let other = tech(&mut f, Color::Red, 9, Zone::Board(B, Color::Red));
        f.cards[usize::from(other.0)].symbols[0] = Symbol::Bonus(11);
        let untouched = deck(&mut f, CardSet::Base, 11, Color::Green);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 1);
        let mut events = Vec::new();
        let status = finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            status,
            if bonus.is_some() {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
        assert_eq!(
            game.position.locate(untouched),
            Some(Zone::Deck(Deck::base(11)))
        );
        assert!(!events.iter().any(|e| matches!(e, Event::Moved(_))));
    }
}

#[test]
fn mp3_replaced_returns_do_not_create_claim_quota() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E099", Some(Zone::Junk));
    figure(&mut f, Figure::NapoleonBonaparte, A);
    tech(&mut f, Color::Purple, 10, Zone::Board(A, Color::Purple));
    tech(&mut f, Color::Green, 10, Zone::Score(A));
    let hand = tech(&mut f, Color::Blue, 8, Zone::Hand(A));
    let achievement = tech(&mut f, Color::Red, 1, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    start(&mut game, source, 1, Context::new(A), 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.locate(hand),
        Some(Zone::Board(A, Color::Blue))
    );
    assert_eq!(
        game.position.locate(achievement),
        Some(Zone::AvailableAchievements)
    );
    assert!(returned(&events).is_empty());
    assert!(events.iter().any(
        |e| matches!(e,Event::Moved(m) if m.inside_karma&&m.card==hand&&m.verb==Some(Verb::Meld))
    ));
}

#[test]
fn mp3_each_same_value_claim_pays_its_increased_cost_and_excludes_board_aliases() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E099", Some(Zone::Junk));
    figure(&mut f, Figure::CaiLun, A);
    let writing = f.add(base_card("B012").unwrap(), Zone::Board(A, Color::Blue));
    tech(&mut f, Color::Purple, 10, Zone::Board(A, Color::Purple));
    tech(&mut f, Color::Green, 10, Zone::Score(A));
    tech(&mut f, Color::Red, 8, Zone::Hand(A));
    tech(&mut f, Color::Yellow, 9, Zone::Hand(A));
    let first = tech(&mut f, Color::Blue, 2, Zone::AvailableAchievements);
    let second = tech(&mut f, Color::Red, 2, Zone::AvailableAchievements);
    f.cards[usize::from(second.0)].set = CardSet::Echoes;
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 1, Context::new(A), 0);
    finish(&mut game, &mut Vec::new(), |_| 0);
    assert_eq!(game.position.cards(Zone::Achievements(A)).len(), 1);
    assert_ne!(game.position.locate(first), game.position.locate(second));
    assert_eq!(
        game.position.locate(writing),
        Some(Zone::Board(A, Color::Blue))
    );
}

#[test]
fn gps_yellow_splay_is_optional_and_does_not_splay_other_colors() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E100", Some(Zone::Junk));
        for color in [Color::Yellow, Color::Blue] {
            for age in 1..=2 {
                tech(&mut f, color, age, Zone::Board(A, color));
            }
        }
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 1, Context::new(A), 0);
        finish(&mut game, &mut Vec::new(), |menu| {
            if accept { 0 } else { decline(menu) }
        });
        assert_eq!(
            game.position.splay(A, Color::Yellow),
            if accept { Splay::Up } else { Splay::None }
        );
        assert_eq!(game.position.splay(A, Color::Blue), Splay::None);
    }
}

#[test]
fn gps_foreseen_foreshadows_only_its_original_draw_packet_after_curie() {
    let mut f = Fixture::default();
    let mut printed = echoes_card("E100").unwrap();
    printed.effects = vec![printed.effects[2].clone()];
    let source = f.add(printed, Zone::Forecast(A));
    let driver = tech(&mut f, Color::Red, 10, Zone::Hand(A));
    figure(&mut f, Figure::MarieCurie, A);
    let extra = deck(&mut f, CardSet::Base, 9, Color::Purple);
    let draws: Vec<_> = (0..3)
        .map(|_| deck(&mut f, CardSet::Base, 11, Color::Red))
        .collect();
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events, |menu| {
        menu.options.iter().position(|a|matches!(a,Alternative::Select{target:TargetDescriptor::Known{card,..}} if *card==source)).unwrap_or(0)
    });
    assert_eq!(game.position.locate(extra), Some(Zone::Hand(A)));
    assert!(
        draws
            .iter()
            .all(|&id| game.position.locate(id) == Some(Zone::Forecast(A)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==extra&&m.inside_karma))
    );
}

#[test]
fn puzzle_cube_keeps_one_color_and_zero_one_or_two_bottom_cards() {
    for count in 0..=2 {
        let mut f = Fixture::default();
        let source = card(&mut f, "E104", Some(Zone::Junk));
        let red: Vec<_> = (1..=3)
            .map(|i| tech(&mut f, Color::Red, i, Zone::Board(A, Color::Red)))
            .collect();
        let blue: Vec<_> = (4..=5)
            .map(|i| tech(&mut f, Color::Blue, i, Zone::Board(A, Color::Blue)))
            .collect();
        let position = f
            .position()
            .with_splay(A, Color::Red, Splay::Left)
            .unwrap()
            .with_splay(A, Color::Blue, Splay::Left)
            .unwrap();
        let mut game = Game::new(position, BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut scored = 0;
        let mut events = Vec::new();
        let status = finish(&mut game, &mut events, |menu| {
            if menu
                .options
                .contains(&Alternative::Colors(1 << Color::Red as u8))
            {
                menu.options
                    .iter()
                    .position(|a| *a == Alternative::Colors(1 << Color::Red as u8))
                    .unwrap()
            } else if scored < count {
                scored += 1;
                pick(menu, red[scored - 1])
            } else {
                decline(menu)
            }
        });
        assert_eq!(game.position.cards(Zone::Score(A)), &red[..count]);
        assert_eq!(
            game.position.cards(Zone::Board(A, Color::Red)),
            &red[count..]
        );
        assert_eq!(game.position.cards(Zone::Board(A, Color::Blue)), blue);
        assert_eq!(
            status,
            if count == 1 {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
    }
}

#[test]
fn puzzle_cube_counts_visible_cards_in_present_colors_and_wins_on_an_empty_board() {
    for (colors, splayed, win) in [
        // An empty board wins vacuously.
        (0, false, true),
        (1, false, true),
        (2, false, true),
        (2, true, false),
        (5, false, true),
    ] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E104", Some(Zone::Junk));
        for (i, &color) in Color::ALL.iter().take(colors).enumerate() {
            for _ in 0..=i {
                tech(&mut f, color, 1, Zone::Board(A, color));
            }
        }
        let mut position = f.position();
        if splayed {
            position = position.with_splay(A, Color::ALL[1], Splay::Right).unwrap();
        }
        let mut game = Game::new(position, BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let status = finish(&mut game, &mut Vec::new(), decline);
        assert_eq!(
            status,
            if win {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            },
            "colors={colors}, splayed={splayed}"
        );
        assert!(game.position.cards(Zone::Score(A)).is_empty());
    }
}

#[test]
fn puzzle_cube_replaced_score_is_considered_once_and_live_splay_changes_its_win() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E104", Some(Zone::Junk));
    figure(&mut f, Figure::ShenKuo, A);
    let red = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), BOTH).unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    let mut events = Vec::new();
    let mut attempts = 0;
    let status = finish(&mut game, &mut events, |menu| {
        if let Some(i) = menu
            .options
            .iter()
            .position(|a| *a == Alternative::Colors(1 << Color::Red as u8))
        {
            i
        } else {
            attempts += 1;
            pick(menu, red)
        }
    });
    assert_eq!(attempts, 1);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position.splay(A, Color::Red), Splay::Right);
    assert!(game.position.cards(Zone::Score(A)).is_empty());
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Splayed {
            to: Splay::Right,
            ..
        }
    )));
}

#[test]
fn puzzle_cube_echo_melds_from_score_and_boxed_followup_melds_the_actual_draw() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E104", Some(Zone::Junk));
        let score = tech(&mut f, Color::Blue, 3, Zone::Score(A));
        let hand = tech(&mut f, Color::Red, 3, Zone::Hand(A));
        if increase == 1 {
            deck(&mut f, CardSet::Base, 10, Color::Purple);
        }
        let drawn = deck(&mut f, CardSet::Base, 10 + increase, Color::Green);
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, Card::ECHO, Context::new(A), increase);
        finish(&mut game, &mut Vec::new(), |_| 0);
        assert_eq!(
            game.position.locate(score),
            Some(Zone::Board(A, Color::Blue))
        );
        assert_eq!(game.position.locate(hand), Some(Zone::Hand(A)));
        start(&mut game, source, 1, Context::new(A), increase);
        finish(&mut game, &mut Vec::new(), |_| 0);
        assert_eq!(
            game.position.locate(drawn),
            Some(Zone::Board(A, Color::Green))
        );
    }
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
    panic!("age-ten program did not settle");
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
    assert!(depth < 24, "bounded age-ten oracle");
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

fn executing(f: Fixture, source: CardId, effect: u16) -> Game {
    let mut game = Game::seeded(f.position(), BOTH, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    start(&mut game, source, effect, context, 0);
    game
}

#[test]
fn camcorder_whole_hand_transfer_oracle_preserves_nobels_actual_first() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E096", Some(Zone::Junk));
        for (color, age) in [(Color::Blue, 1), (Color::Red, 2), (Color::Purple, 2)] {
            tech(&mut f, color, age, Zone::Hand(A));
        }
        if reacting {
            figure(&mut f, Figure::AlfredNobel, B);
        }
        let junked = deck(&mut f, CardSet::Base, 7, Color::Green);
        deck(&mut f, CardSet::Base, 9, Color::Yellow);
        let leaves = oracle(executing(f, source, 0), !reacting);
        assert!(
            leaves
                .iter()
                .all(|p| p.outcome.position.cards(Zone::Hand(B)).len() == 3)
        );
        if reacting {
            assert!(leaves.iter().all(|p|p.chronological.iter().any(|e|matches!(e,Event::Moved(m) if m.card==junked&&m.inside_karma&&m.verb==Some(Verb::Junk)))));
        }
    }
}

#[test]
fn camcorder_meld_and_return_oracles_preserve_same_color_orders_and_napoleon() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E096", Some(Zone::Junk));
        let red1 = tech(&mut f, Color::Red, 9, Zone::Hand(A));
        let red2 = tech(&mut f, Color::Red, 9, Zone::Hand(A));
        tech(&mut f, Color::Blue, 9, Zone::Hand(A));
        tech(&mut f, Color::Green, 8, Zone::Hand(A));
        let other = tech(&mut f, Color::Purple, 7, Zone::Hand(A));
        f.cards[usize::from(other.0)].set = CardSet::Echoes;
        if reacting {
            figure(&mut f, Figure::NapoleonBonaparte, A);
        }
        for _ in 0..3 {
            deck(&mut f, CardSet::Base, 9, Color::Yellow);
        }
        let leaves = compare_paths(
            executing(f, source, 1),
            if reacting { None } else { Some(true) },
        );
        if !reacting {
            assert_eq!(
                leaves.len(),
                2,
                "only the two same-color meld orders remain"
            );
        }
        for order in [[red1, red2], [red2, red1]] {
            assert!(
                leaves
                    .iter()
                    .any(|p| p.outcome.position.cards(Zone::Board(A, Color::Red)) == order)
            );
        }
        if reacting {
            assert!(leaves.iter().all(|p| {
                p.chronological.iter().any(
                    |e| matches!(e,Event::Moved(m) if m.inside_karma&&m.verb==Some(Verb::Meld)),
                )
            }));
        }
    }
}

#[test]
fn social_networking_live_prefix_oracle_preserves_nobels_transfer_reaction() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E097", Some(Zone::Junk));
        for color in [Color::Blue, Color::Red] {
            for age in 1..=2 {
                tech(&mut f, color, age, Zone::Board(A, color));
            }
        }
        if reacting {
            figure(&mut f, Figure::AlfredNobel, B);
        }
        let junked = deck(&mut f, CardSet::Base, 7, Color::Yellow);
        let leaves = oracle(executing(f, source, 0), !reacting);
        assert!(
            leaves
                .iter()
                .all(|p| p.outcome.position.cards(Zone::Score(B)).len() == 4)
        );
        if reacting {
            assert!(leaves.iter().all(|p| {
                p.chronological
                    .iter()
                    .any(|e| matches!(e,Event::Moved(m) if m.card==junked&&m.inside_karma))
            }));
        }
    }
}

#[test]
fn mp3_optional_return_oracle_preserves_every_subset_and_peters_first() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E099", Some(Zone::Junk));
        for age in [8, 9] {
            tech(&mut f, Color::Blue, age, Zone::Hand(A));
        }
        tech(&mut f, Color::Red, 8, Zone::Hand(A));
        let scored = tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        if reacting {
            figure(&mut f, Figure::PeterTheGreat, A);
        }
        let leaves = oracle(executing(f, source, 1), !reacting);
        for size in 0..=3 {
            assert!(
                leaves
                    .iter()
                    .any(|p| p.outcome.position.cards(Zone::Hand(A)).len() == size)
            );
        }
        if reacting {
            assert!(leaves.iter().any(|p|p.chronological.iter().any(|e|matches!(e,Event::Moved(m) if m.card==scored&&m.inside_karma&&m.verb==Some(Verb::Score)))));
        }
    }
}

#[test]
fn gps_forecast_return_oracle_keeps_deck_order_and_peters_actual_first() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E100", Some(Zone::Junk));
        let first = tech(&mut f, Color::Red, 3, Zone::Forecast(A));
        let second = tech(&mut f, Color::Blue, 3, Zone::Forecast(A));
        let third = tech(&mut f, Color::Purple, 3, Zone::Forecast(A));
        f.cards[usize::from(third.0)].set = CardSet::Echoes;
        let scored = tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        if reacting {
            figure(&mut f, Figure::PeterTheGreat, A);
        }
        let leaves = oracle(executing(f, source, 0), !reacting);
        for order in [[first, second], [second, first]] {
            assert!(
                leaves
                    .iter()
                    .any(|p| p.outcome.position.cards(Zone::Deck(Deck::base(3))) == order)
            );
        }
        if reacting {
            assert!(leaves.iter().all(|p|p.chronological.iter().any(|e|matches!(e,Event::Moved(m) if m.card==scored&&m.inside_karma&&m.verb==Some(Verb::Score)))));
        }
    }
}

#[test]
fn gps_draw_packet_oracle_reduces_only_when_all_remaining_cards_fit() {
    for occupied in [0, 3, 4] {
        let mut f = Fixture::default();
        let mut printed = echoes_card("E100").unwrap();
        printed.effects = vec![printed.effects[2].clone()];
        f.add(printed, Zone::Forecast(A));
        let driver = tech(&mut f, Color::Red, 10, Zone::Hand(A));
        // Curie actually draws the extra nine before the first original eleven.
        figure(&mut f, Figure::MarieCurie, A);
        let extra = deck(&mut f, CardSet::Base, 9, Color::Green);
        for _ in 0..occupied {
            tech(&mut f, Color::Red, 11, Zone::Forecast(A));
        }
        for color in [Color::Red, Color::Blue, Color::Purple] {
            deck(&mut f, CardSet::Base, 11, color);
        }
        if occupied == 4 {
            for age in 1..=2 {
                tech(&mut f, Color::Yellow, age, Zone::Board(A, Color::Yellow));
            }
        }
        let mut position = f.position();
        if occupied == 4 {
            position = position.with_splay(A, Color::Yellow, Splay::Left).unwrap();
        }
        let mut game = Game::seeded(position, BOTH, 53)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
            .unwrap();
        let leaves = oracle(game, occupied != 3);
        assert!(
            leaves
                .iter()
                .all(|p| p.outcome.position.locate(extra) == Some(Zone::Hand(A)))
        );
        assert!(leaves.iter().all(|p| {
            p.chronological
                .iter()
                .any(|e| matches!(e,Event::Moved(m) if m.card==extra&&m.inside_karma))
        }));
    }
}

#[test]
fn mp3_duplicate_hidden_backs_remain_private_through_hedy_and_world_sampling() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E099", None);
    figure(&mut f, Figure::HedyLamarr, A);
    for color in [Color::Blue, Color::Red] {
        for age in 1..=2 {
            tech(&mut f, color, age, Zone::Board(A, color));
        }
    }
    tech(&mut f, Color::Green, 10, Zone::Score(A));
    tech(&mut f, Color::Red, 8, Zone::Hand(A));
    for set in [CardSet::Base, CardSet::Echoes] {
        for color in [Color::Blue, Color::Red] {
            let id = tech(&mut f, color, 1, Zone::AvailableAchievements);
            f.cards[usize::from(id.0)].set = set;
        }
    }
    let mut game = Game::seeded(f.position(), BOTH, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 1, Context::new(A), 0);
    let mut events = Vec::new();
    let returned_menu = decision(&mut game, &mut events);
    assert!(returned_menu.options.iter().any(|a| matches!(
        a,
        Alternative::Move {
            verb: Verb::Return,
            ..
        }
    )));
    game.choose(0).unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options,
        [CardSet::Base, CardSet::Echoes].map(|set| Alternative::BlindSelect {
            source: Zone::AvailableAchievements,
            back: Deck { set, age: 1 }
        })
    );
    let mut rng = game.rng.clone();
    rng.next_u64();
    game.choose(0).unwrap();
    let first = decision(&mut game, &mut events);
    assert_eq!(game.rng, rng);
    assert!(
        first
            .options
            .iter()
            .all(|a| matches!(a, Alternative::Splay { .. }))
    );
    assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
    for player in [A] {
        for seed in 0..12 {
            let tracked = game.determinize(player, seed).unwrap();
            assert_eq!(
                tracked.observe(player).unwrap(),
                game.observe(player).unwrap()
            );
            for mut world in [tracked, game.sample_world(player, seed).unwrap().world] {
                assert_eq!(decision(&mut world, &mut Vec::new()).options, first.options);
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
                assert_eq!(world.position.card(selected).origin_deck(), Deck::base(1));
                let mut events = Vec::new();
                world.choose(0).unwrap();
                finish(&mut world, &mut events, |_| 0);
                assert_eq!(world.position.locate(selected), Some(Zone::Achievements(A)));
                assert_eq!(world.position.cards(Zone::Achievements(A)).len(), 2);
                assert!(events.iter().any(|e| matches!(e, Event::Splayed { .. })));
                assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
            }
        }
    }
}
