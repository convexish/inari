//! Age-six clauses and frozen quotas from card values.
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
    panic!("age-six card failed to finish");
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
fn shrapnel_tucks_then_transfers_two_live_tops_and_scores_demanders_bottom() {
    for count in 0..=3 {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E057", Some(Zone::Board(B, Color::Red)));
            let prior: Vec<_> = (0..count)
                .map(|i| tech(&mut f, Color::Blue, i + 1, Zone::Board(A, Color::Blue)))
                .collect();
            let bottom = tech(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
            let top = tech(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue));
            let drawn = deck(&mut f, CardSet::Base, 6, Color::Blue);
            let junked = deck(&mut f, CardSet::Base, 7, Color::Yellow);
            if reacting {
                figure(&mut f, Figure::AlfredNobel, B);
            }
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            let mut context = Context::new(A);
            context.demander = B;
            start(&mut game, source, 0, context, 0);
            let mut events = Vec::new();
            finish(&mut game, &mut events, |_| 0);
            let stack: Vec<_> = std::iter::once(drawn).chain(prior).collect();
            let transferred: Vec<_> = stack.iter().rev().take(2).copied().collect();
            assert_eq!(game.position.cards(Zone::Score(B)), transferred);
            assert_eq!(game.position.cards(Zone::Score(A)), [bottom]);
            assert_eq!(game.position.cards(Zone::Board(B, Color::Blue)), [top]);
            assert_eq!(
                game.position.cards(Zone::Board(A, Color::Blue)),
                &stack[..stack.len().saturating_sub(2)]
            );
            assert_eq!(
                game.position.locate(junked),
                Some(if reacting {
                    Zone::Junk
                } else {
                    Zone::Deck(Deck::base(7))
                })
            );
            if reacting {
                assert!(events.iter().any(
                    |event| matches!(event,Event::Moved(m) if m.card==junked&&m.inside_karma)
                ));
            }
            let tuck=events.iter().position(|event|matches!(event,Event::Moved(m) if m.card==drawn&&m.verb==Some(Verb::Tuck))).unwrap();
            let first_transfer = events
                .iter()
                .position(|event| matches!(event,Event::Moved(m) if m.verb==Some(Verb::Transfer)))
                .unwrap();
            assert!(tuck < first_transfer);
        }
    }
}

#[test]
fn shrapnel_replaced_tuck_has_no_original_color_for_later_moves() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E057", Some(Zone::Board(B, Color::Red)));
    figure(&mut f, Figure::NapoleonBonaparte, A);
    let original = deck(&mut f, CardSet::Base, 6, Color::Green);
    let untouched = tech(&mut f, Color::Green, 2, Zone::Board(B, Color::Green));
    let mut game = Game::new(f.position(), BOTH).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    start(&mut game, source, 0, context, 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.locate(original),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(
        game.position.locate(untouched),
        Some(Zone::Board(B, Color::Green))
    );
    assert!(game.position.cards(Zone::Score(B)).is_empty());
    assert!(events.iter().any(|event|matches!(event,Event::Moved(m) if m.card==original&&m.verb==Some(Verb::Meld)&&m.inside_karma)));
    assert!(!events.iter().any(
        |event| matches!(event,Event::Moved(m) if !m.inside_karma&&m.verb==Some(Verb::Transfer))
    ));
}

#[test]
fn hot_air_balloon_claims_only_matching_eligible_tops_and_branches_on_success() {
    for (matching, eligible, accept, green) in [
        (false, true, true, true),
        (true, false, true, true),
        (true, true, false, true),
        (true, true, true, true),
        (true, true, true, false),
    ] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E060", Some(Zone::Junk));
        tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
        if eligible {
            tech(&mut f, Color::Yellow, 10, Zone::Score(A));
        }
        let chosen = tech(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue));
        let distractor = tech(&mut f, Color::Yellow, 3, Zone::Board(B, Color::Yellow));
        tech(
            &mut f,
            Color::Red,
            if matching { 2 } else { 1 },
            Zone::Achievements(B),
        );
        let gifted = green.then(|| tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green)));
        let drawn = deck(&mut f, CardSet::Base, 7, Color::Purple);
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |menu| {
            if accept {
                pick(menu, chosen)
            } else {
                decline(menu)
            }
        });
        let claimed = matching && eligible && accept;
        assert_eq!(
            game.position.locate(chosen),
            Some(if claimed {
                Zone::Achievements(A)
            } else {
                Zone::Board(B, Color::Blue)
            })
        );
        assert_eq!(
            game.position.locate(distractor),
            Some(Zone::Board(B, Color::Yellow))
        );
        assert_eq!(
            game.position.locate(drawn),
            Some(if claimed {
                Zone::Deck(Deck::base(7))
            } else {
                Zone::Board(A, Color::Purple)
            })
        );
        if let Some(gifted) = gifted {
            assert_eq!(
                game.position.locate(gifted),
                Some(Zone::Board(if claimed { B } else { A }, Color::Green))
            );
        }
    }
}

#[test]
fn steamboat_reveals_the_original_draw_and_selects_the_exact_color_branch() {
    for color in Color::ALL {
        for scores in 0..=3 {
            let mut f = Fixture::default();
            let source = card(&mut f, "E061", Some(Zone::Board(B, Color::Green)));
            let original = deck(&mut f, CardSet::Base, 6, color);
            let hand = [
                tech(&mut f, Color::Red, 1, Zone::Hand(A)),
                tech(&mut f, Color::Blue, 2, Zone::Hand(A)),
            ];
            let points: Vec<_> = (0..scores)
                .map(|i| tech(&mut f, Color::Purple, i + 1, Zone::Score(A)))
                .collect();
            let mut game = Game::new(f.position(), Expansions::ECHOES)
                .unwrap()
                .with_tracking()
                .unwrap();
            let mut context = Context::new(A);
            context.demander = B;
            start(&mut game, source, 0, context, 0);
            let mut events = Vec::new();
            finish(&mut game, &mut events, |_| 0);
            let hands = matches!(color, Color::Blue | Color::Yellow);
            let score = matches!(color, Color::Red | Color::Green);
            assert_eq!(
                game.position.locate(original),
                Some(Zone::Hand(if hands { B } else { A }))
            );
            for target in hand {
                assert_eq!(
                    game.position.locate(target),
                    Some(Zone::Hand(if hands { B } else { A }))
                );
            }
            assert_eq!(
                game.position.cards(Zone::Score(B)).len(),
                if score { scores.min(2) as usize } else { 0 }
            );
            assert_eq!(
                points
                    .iter()
                    .filter(|target| game.position.locate(**target) == Some(Zone::Score(A)))
                    .count(),
                if score {
                    scores.saturating_sub(2) as usize
                } else {
                    scores as usize
                }
            );
            assert_eq!(
                events
                    .iter()
                    .filter_map(|event| if let Event::Revealed { card, .. } = event {
                        Some(*card)
                    } else {
                        None
                    })
                    .collect::<Vec<_>>(),
                [original]
            );
            let reveal = events
                .iter()
                .position(|event| matches!(event,Event::Revealed{card,..}if *card==original))
                .unwrap();
            if let Some(transfer) = events
                .iter()
                .position(|event| matches!(event,Event::Moved(m) if m.verb==Some(Verb::Transfer)))
            {
                assert!(reveal < transfer);
            }
        }
    }
}

#[test]
fn bifocals_echo_preserves_visible_forecast_choice_and_dogma_gates_up_on_real_splay() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E062", None);
        let first = tech(&mut f, Color::Red, 4, Zone::Forecast(A));
        let second = tech(&mut f, Color::Blue, 4, Zone::Forecast(A));
        for color in [Color::Red, Color::Green] {
            for value in [1, 2] {
                tech(&mut f, color, value, Zone::Board(A, color));
            }
        }
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, Card::ECHO, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 2);
        game.choose(pick(&menu, second)).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(second),
            Some(Zone::Deck(Deck::base(4)))
        );
        assert_eq!(game.position.locate(first), Some(Zone::Forecast(A)));
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
        start(&mut game, source, 1, Context::new(A), 0);
        let mut choices = 0;
        finish(&mut game, &mut events, |menu| {
            choices += 1;
            if choices == 1 && !accept {
                decline(menu)
            } else {
                menu.options
                    .iter()
                    .position(|x| {
                        matches!(
                            x,
                            Alternative::Splay {
                                color: Color::Red,
                                direction: Splay::Up,
                                ..
                            }
                        )
                    })
                    .unwrap_or(0)
            }
        });
        assert_eq!(
            game.position.splay(A, Color::Green),
            if accept { Splay::Right } else { Splay::None }
        );
        assert_eq!(
            game.position.splay(A, Color::Red),
            if accept { Splay::Up } else { Splay::None }
        );
        assert_eq!(choices, if accept { 2 } else { 1 });
        if accept {
            start(&mut game, source, 1, Context::new(A), 0);
            let mut extra = Vec::new();
            finish(&mut game, &mut extra, |_| {
                panic!("already-right green cannot earn an up splay")
            });
            assert!(extra.is_empty());
        }
    }
}

#[test]
fn bifocals_foreseen_uses_lowest_physical_standard_or_zero_and_still_draws_at_capacity() {
    for foreseen in [false, true] {
        for present in [false, true] {
            let mut f = Fixture::default();
            let mut printed = echoes_card("E062").unwrap();
            printed.echo = None;
            printed.effects.truncate(1);
            let source = f.add(
                printed,
                if foreseen {
                    Zone::Forecast(A)
                } else {
                    Zone::Board(A, Color::Blue)
                },
            );
            let driver = tech(&mut f, Color::Red, 6, Zone::Hand(A));
            let first = deck(&mut f, CardSet::Base, 7, Color::Green);
            let one = deck(&mut f, CardSet::Base, 1, Color::Blue);
            let lowest = deck(&mut f, CardSet::Base, 2, Color::Yellow);
            let higher = deck(&mut f, CardSet::Base, 4, Color::Purple);
            if present {
                tech(&mut f, Color::Red, 2, Zone::AvailableAchievements);
                tech(&mut f, Color::Blue, 4, Zone::AvailableAchievements);
            }
            for _ in 0..2 {
                tech(&mut f, Color::Red, 10, Zone::Forecast(A));
            }
            for value in [1, 2] {
                tech(&mut f, Color::Yellow, value, Zone::Board(A, Color::Yellow));
            }
            let mut position = f.position();
            position.players[A.index()].splays[Color::Yellow as usize] = Splay::Right;
            let mut game = Game::new(position, Expansions::ECHOES).unwrap();
            if foreseen {
                game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                    .unwrap();
            } else {
                start(&mut game, source, 0, Context::new(A), 0);
            }
            finish(&mut game, &mut Vec::new(), |_| 0);
            assert_eq!(game.position.locate(first), Some(Zone::Forecast(A)));
            assert_eq!(
                game.position.locate(one),
                Some(if foreseen && !present {
                    Zone::Hand(A)
                } else {
                    Zone::Deck(Deck::base(1))
                })
            );
            assert_eq!(
                game.position.locate(lowest),
                Some(if foreseen && present {
                    Zone::Hand(A)
                } else {
                    Zone::Deck(Deck::base(2))
                })
            );
            assert_eq!(
                game.position.locate(higher),
                Some(Zone::Deck(Deck::base(4)))
            );
            assert_eq!(game.position.cards(Zone::Forecast(A)).len(), 3);
        }
    }
}

#[test]
fn indian_clubs_keeps_two_return_phases_and_foreseen_exhausts_the_remainder() {
    for foreseen in [false, true] {
        for count in 0..=3 {
            let mut f = Fixture::default();
            let source = card(&mut f, "E064", foreseen.then_some(Zone::Forecast(A)));
            let driver = tech(&mut f, Color::Green, 6, Zone::Hand(A));
            let points: Vec<_> = (0..count)
                .map(|i| tech(&mut f, Color::Blue, i / 2 + 1, Zone::Score(B)))
                .collect();
            let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
            if foreseen {
                game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                    .unwrap();
            } else {
                let mut context = Context::new(B);
                context.demander = A;
                start(&mut game, source, 0, context, 0);
            }
            let mut events = Vec::new();
            finish(&mut game, &mut events, |_| 0);
            let expected = if foreseen { count } else { count.min(2) } as usize;
            assert_eq!(returned(&events).len(), expected);
            assert_eq!(
                points
                    .iter()
                    .filter(|target| matches!(game.position.locate(**target), Some(Zone::Deck(_))))
                    .count(),
                expected
            );
            assert_eq!(
                game.position.cards(Zone::Score(B)).len(),
                count as usize - expected
            );
        }
    }
}

#[test]
fn indian_clubs_freezes_distinct_score_values_and_preserves_same_age_identity_choices() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E064", None);
    for value in [1, 1, 2, 4] {
        tech(&mut f, Color::Green, value, Zone::Score(A));
    }
    let first = tech(&mut f, Color::Red, 1, Zone::Hand(A));
    let second = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
    let two = tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
    let three = tech(&mut f, Color::Purple, 3, Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::ECHOES)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 1, Context::new(A), 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    game.choose(pick(&menu, second)).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.cards(Zone::Hand(A)), [first, three]);
    assert!(
        game.position
            .cards(Zone::Score(A))
            .ends_with(&[second, two])
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Revealed { .. }))
    );
}

#[test]
fn per_value_card_sources_preserve_logical_membership_and_multiplicity() {
    for multiplicity in [Multiplicity::Distinct, Multiplicity::Every] {
        let mut f = Fixture::default();
        figure(&mut f, Figure::WernherVonBraun, A);
        for _ in 0..2 {
            tech(&mut f, Color::Green, 1, Zone::Junk);
        }
        let first = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        let second = tech(&mut f, Color::Red, 1, Zone::Hand(A));
        let unrelated = tech(&mut f, Color::Purple, 2, Zone::Hand(A));
        let quota = Box::leak(Box::new(PerValue {
            source: ValueSource::Cards(&Selector::Score(Who::Executor)),
            multiplicity,
        }));
        let source = f.tech(
            Color::Yellow,
            6,
            Zone::Board(A, Color::Yellow),
            vec![build::score(Selector::Hand(Who::Executor)).per_value(quota)],
        );
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            [first, second]
                .iter()
                .filter(|target| game.position.locate(**target) == Some(Zone::Score(A)))
                .count(),
            if multiplicity == Multiplicity::Every {
                2
            } else {
                1
            }
        );
        assert_eq!(game.position.locate(unrelated), Some(Zone::Hand(A)));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Revealed { .. }))
        );
        // Wernher's owner may look at the junk; the opponent still cannot.
        let knows_junk = |p: Player, known: bool| {
            game.tracker(p)
                .unwrap()
                .belief(Zone::Junk)
                .iter()
                .all(|slot| slot.known().is_some() == known)
        };
        assert!(knows_junk(A, true));
        assert!(knows_junk(B, false));
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn hot_air_balloon_echo_draws_and_scores_its_boxed_seven_under_archimedes() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E060", None);
        if reacting {
            figure(&mut f, Figure::Archimedes, A);
        }
        let seven = deck(&mut f, CardSet::Base, 7, Color::Blue);
        let eight = deck(&mut f, CardSet::Base, 8, Color::Red);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, Card::ECHO, Context::new(A), 0);
        finish(&mut game, &mut Vec::new(), |_| 0);
        assert_eq!(
            game.position.cards(Zone::Score(A)),
            [if reacting { eight } else { seven }]
        );
        assert_eq!(
            game.position.locate(if reacting { seven } else { eight }),
            Some(Zone::Deck(Deck::base(if reacting { 7 } else { 8 })))
        );
    }
}

#[test]
fn kaleidoscope_splays_only_the_original_successful_meld_color_and_is_optional() {
    for (reacting, accept) in [(false, false), (false, true), (true, true)] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E065", Some(Zone::Junk));
        let lower = tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green));
        let original = deck(&mut f, CardSet::Base, 7, Color::Green);
        if reacting {
            figure(&mut f, Figure::NapoleonBonaparte, A);
        }
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |menu| {
            if accept { 0 } else { decline(menu) }
        });
        assert_eq!(
            game.position.cards(Zone::Board(A, Color::Green)),
            [lower, original]
        );
        assert_eq!(
            game.position.splay(A, Color::Green),
            if accept && !reacting {
                Splay::Right
            } else {
                Splay::None
            }
        );
        if reacting {
            assert!(events.iter().any(|event|matches!(event,Event::Moved(m) if m.card==original&&m.verb==Some(Verb::Meld)&&m.inside_karma)));
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Event::Splayed { .. }))
            );
        }
    }
}

#[test]
fn kaleidoscope_foreseen_junks_lower_even_without_an_equal_standard_and_keeps_equal_set_choice() {
    for foreseen in [false, true] {
        for present in [false, true] {
            let mut f = Fixture::default();
            let source = card(&mut f, "E065", foreseen.then_some(Zone::Forecast(A)));
            let driver = tech(&mut f, Color::Red, 6, Zone::Hand(A));
            let first = deck(&mut f, CardSet::Base, 7, Color::Green);
            let lower = tech(&mut f, Color::Red, 1, Zone::AvailableAchievements);
            let higher = tech(&mut f, Color::Blue, 3, Zone::AvailableAchievements);
            let equal =
                present.then(|| tech(&mut f, Color::Yellow, 2, Zone::AvailableAchievements));
            let echo = present.then(|| {
                let card = tech(&mut f, Color::Green, 2, Zone::AvailableAchievements);
                f.cards[usize::from(card.0)].set = CardSet::Echoes;
                card
            });
            let mut game = Game::new(f.position(), Expansions::ECHOES)
                .unwrap()
                .with_tracking()
                .unwrap();
            if foreseen {
                game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                    .unwrap();
            } else {
                start(&mut game, source, 1, Context::new(A), 0);
            }
            let mut events = Vec::new();
            let mut equal_choice = false;
            finish(&mut game, &mut events, |menu| {
                if menu
                    .options
                    .iter()
                    .any(|option| matches!(option, Alternative::BlindSelect { .. }))
                {
                    equal_choice = true;
                    assert_eq!(menu.options.len(), 2);
                    menu.options
                        .iter()
                        .position(|option| {
                            matches!(
                                option,
                                Alternative::BlindSelect {
                                    back: Deck {
                                        set: CardSet::Echoes,
                                        age: 2
                                    },
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    0
                }
            });
            assert_eq!(equal_choice, present);
            assert_eq!(
                game.position.locate(lower),
                Some(if foreseen {
                    Zone::Junk
                } else {
                    Zone::AvailableAchievements
                })
            );
            assert_eq!(
                game.position.locate(higher),
                Some(Zone::AvailableAchievements)
            );
            if let Some(equal) = equal {
                assert_eq!(
                    game.position.locate(equal),
                    Some(Zone::AvailableAchievements)
                );
            }
            if let Some(echo) = echo {
                assert_eq!(game.position.locate(echo), Some(Zone::Junk));
            }
            assert_eq!(
                game.position.locate(first),
                Some(if foreseen {
                    Zone::Board(A, Color::Green)
                } else {
                    Zone::Deck(Deck::base(7))
                })
            );
            assert_eq!(
                game.position.special_location(Special::Supremacy),
                SpecialLocation::Available
            );
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Event::Revealed { .. }))
            );
        }
    }
}

#[test]
fn kaleidoscope_uses_effective_concept_count_including_maxwells_each() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E065", None);
    figure(&mut f, Figure::JamesClerkMaxwell, A);
    tech(&mut f, Color::Blue, 1, Zone::Hand(A));
    let two = tech(&mut f, Color::Red, 2, Zone::AvailableAchievements);
    let three = tech(&mut f, Color::Yellow, 3, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    assert_eq!(game.position.icon_count(A, Icon::Concept), 3);
    start(&mut game, source, 1, Context::new(A), 0);
    finish(&mut game, &mut Vec::new(), |_| 0);
    assert_eq!(game.position.locate(two), Some(Zone::AvailableAchievements));
    assert_eq!(game.position.locate(three), Some(Zone::Junk));
}

#[test]
fn per_value_source_front_metadata_follows_nested_card_and_bonus_queries() {
    const CASES: [(ValueSource, bool); 4] = [
        (ValueSource::Cards(&Selector::Reg(Reg::Bound)), false),
        (
            ValueSource::Cards(&build::filtered(
                &Selector::Reg(Reg::Bound),
                &[Filter::Has(Test::Icon(Icon::Authority))],
            )),
            true,
        ),
        (
            ValueSource::Bonuses(Bonuses {
                on: Who::Executor,
                only: BonusValues::Equal(Expr::Value(Reg::Bound)),
            }),
            false,
        ),
        (
            ValueSource::Bonuses(Bonuses {
                on: Who::Executor,
                only: BonusValues::Equal(Expr::IconsOn(Reg::Bound, Icon::Authority)),
            }),
            true,
        ),
    ];
    for (source, expected) in CASES {
        let quota = Box::leak(Box::new(PerValue {
            source,
            multiplicity: Multiplicity::Distinct,
        }));
        let instructions = Box::leak(Box::new([
            build::score(Selector::Hand(Who::Executor)).per_value(quota)
        ]));
        let karma = Karma {
            body: Body::Program(instructions),
            ..Figure::ShenKuo.def().karma[0]
        };
        assert_eq!(karma.body_reads_card_front(), expected);
    }
    for (count, expected) in [
        (Expr::IconsOn(Reg::Bound, Icon::Concept), true),
        (Expr::Value(Reg::Bound), false),
    ] {
        let instructions = Box::leak(Box::new([
            build::score(Selector::Hand(Who::Executor)).exactly(count)
        ]));
        let karma = Karma {
            body: Body::Program(instructions),
            ..Figure::ShenKuo.def().karma[0]
        };
        assert_eq!(karma.body_reads_card_front(), expected);
    }
}

#[derive(Clone)]
struct End {
    game: Game,
    status: Status,
    events: Vec<Event>,
}
fn paths(mut game: Game, mut events: Vec<Event>, decisions: usize) -> Vec<End> {
    assert!(decisions < 12, "bounded oracle");
    let status = game.advance(10000, &mut events).unwrap();
    if let Status::Decision(menu) = status {
        (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                paths(fork, events.clone(), decisions + 1)
            })
            .collect()
    } else {
        assert!(
            matches!(
                status,
                Status::Complete(_) | Status::Winner(_) | Status::Draw(_)
            ),
            "{status:?}"
        );
        assert!(game.position.check_conservation());
        for viewer in Player::ALL {
            if let Some(tracker) = game.tracker(viewer) {
                assert!(tracker.check_truth(&game.position));
            }
        }
        if let Some(tracker) = game.public_tracker() {
            assert!(tracker.check_truth(&game.position));
        }
        vec![End {
            game,
            status,
            events,
        }]
    }
}
#[derive(PartialEq, Eq)]
struct Outcome {
    position: Position,
    rng: Rng,
    status: Status,
    beliefs: [Vec<Vec<crate::tracking::BeliefSlot>>; 3],
    views: [Vec<u32>; 2],
}
fn outcomes(ends: &[End]) -> Vec<Outcome> {
    let mut result = Vec::new();
    for end in ends {
        let mut position = end.game.position.clone();
        position.sort_unordered_piles();
        let trackers = &end
            .game
            .knowledge
            .as_ref()
            .expect("tracked oracle")
            .trackers;
        let beliefs = std::array::from_fn(|observer| {
            crate::tracking::zones()
                .into_iter()
                .map(|zone| trackers[observer].belief(zone))
                .collect()
        });
        let item = Outcome {
            position,
            rng: end.game.rng.clone(),
            status: end.status.clone(),
            beliefs,
            views: Player::ALL.map(|p| {
                end.game
                    .observe(p)
                    .unwrap()
                    .row()
                    .into_iter()
                    .map(f32::to_bits)
                    .collect()
            }),
        };
        if !result.contains(&item) {
            result.push(item);
        }
    }
    result
}
fn oracle(game: Game) -> (Vec<End>, Vec<End>) {
    assert!(
        game.knowledge.is_some(),
        "attach tracking before the oracle invocation"
    );
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let reduced = paths(game, Vec::new(), 0);
    let unrestricted = paths(manual, Vec::new(), 0);
    let a = outcomes(&reduced);
    let b = outcomes(&unrestricted);
    assert_eq!(a.len(), b.len());
    assert!(a.iter().all(|item| b.contains(item)));
    let mut traces = Vec::new();
    for end in &unrestricted {
        if !traces.contains(&end.events.as_slice()) {
            traces.push(end.events.as_slice());
        }
    }
    assert!(
        reduced
            .iter()
            .all(|end| traces.contains(&end.events.as_slice())),
        "every canonical execution preserves an actual unrestricted event trace"
    );
    (reduced, unrestricted)
}

#[test]
fn steamboat_shortcut_oracle_keeps_score_subsets_and_nobels_actual_transfer_reaction() {
    for hand in [false, true] {
        for count in [2, 3] {
            for reacting in [false, true] {
                let mut f = Fixture::default();
                let source = card(&mut f, "E061", Some(Zone::Board(B, Color::Green)));
                deck(
                    &mut f,
                    CardSet::Base,
                    6,
                    if hand { Color::Blue } else { Color::Red },
                );
                for age in 1..=count {
                    tech(
                        &mut f,
                        Color::Purple,
                        age,
                        if hand { Zone::Hand(A) } else { Zone::Score(A) },
                    );
                }
                let seven = deck(&mut f, CardSet::Base, 7, Color::Yellow);
                if reacting {
                    figure(&mut f, Figure::AlfredNobel, B);
                }
                let mut game = Game::new(f.position(), BOTH)
                    .unwrap()
                    .with_tracking()
                    .unwrap();
                let mut context = Context::new(A);
                context.demander = B;
                start(&mut game, source, 0, context, 0);
                let (reduced, manual) = oracle(game);
                if !reacting && (hand || count == 2) {
                    assert_eq!(reduced.len(), 1);
                    assert!(manual.len() > reduced.len());
                } else {
                    assert_eq!(reduced.len(), manual.len());
                }
                for end in reduced.iter().chain(&manual) {
                    assert_eq!(end.status, Status::Complete(ReturnValue::Unit));
                    assert_eq!(
                        end.game.position.locate(seven),
                        Some(if reacting {
                            Zone::Junk
                        } else {
                            Zone::Deck(Deck::base(7))
                        })
                    );
                    if reacting {
                        assert!(end.events.iter().any(
                            |e| matches!(e, Event::Moved(m) if m.card == seven && m.inside_karma)
                        ));
                    }
                    assert_eq!(
                        end.game
                            .position
                            .cards(if hand { Zone::Hand(B) } else { Zone::Score(B) })
                            .len(),
                        if hand { usize::from(count) + 1 } else { 2 }
                    );
                }
            }
        }
    }
}

#[test]
fn indian_clubs_scoring_oracle_freezes_values_across_noethers_actual_junks() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E064", None);
        for value in [1, 2, 3] {
            tech(&mut f, Color::Green, value, Zone::Score(A));
        }
        let hands: Vec<_> = [1, 1, 2]
            .into_iter()
            .map(|value| tech(&mut f, Color::Blue, value, Zone::Hand(A)))
            .collect();
        if reacting {
            figure(&mut f, Figure::EmmyNoether, B);
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 1, Context::new(A), 0);
        let (reduced, manual) = oracle(game);
        if !reacting {
            assert!(reduced.len() < manual.len());
        }
        for end in reduced.iter().chain(&manual) {
            let scored: Vec<_> = end
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::Moved(m) if m.verb == Some(Verb::Score) && !m.inside_karma => {
                        Some(m.card)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                scored.len(),
                2,
                "the original score values survive intervening junking"
            );
            assert_eq!(
                scored
                    .iter()
                    .filter(|&&id| end.game.position.card(id).value == 1)
                    .count(),
                1
            );
            assert!(scored.contains(&hands[2]));
            assert_eq!(end.events.iter().filter(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Junk) && m.inside_karma)).count(), if reacting { 2 } else { 0 });
        }
    }
}

#[test]
fn indian_clubs_return_oracle_keeps_native_foreseen_phases_and_peters_added_score_cards() {
    for foreseen in [false, true] {
        for reacting in [false, true] {
            for count in [2, 3] {
                let mut f = Fixture::default();
                let source = card(&mut f, "E064", foreseen.then_some(Zone::Forecast(A)));
                let driver = tech(&mut f, Color::Green, 6, Zone::Hand(A));
                for age in 1..=count {
                    tech(&mut f, Color::Blue, age, Zone::Score(B));
                }
                let extra = tech(&mut f, Color::Green, 4, Zone::Board(B, Color::Green));
                if reacting {
                    figure(&mut f, Figure::PeterTheGreat, B);
                }
                let mut game = Game::new(f.position(), BOTH)
                    .unwrap()
                    .with_tracking()
                    .unwrap();
                if foreseen {
                    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                        .unwrap();
                } else {
                    let mut context = Context::new(B);
                    context.demander = A;
                    start(&mut game, source, 0, context, 0);
                }
                let (reduced, manual) = oracle(game);
                if !reacting {
                    assert!(reduced.len() < manual.len());
                }
                for end in reduced.iter().chain(&manual) {
                    let returns = returned(&end.events).len();
                    assert_eq!(
                        returns,
                        if foreseen {
                            usize::from(count) + usize::from(reacting)
                        } else {
                            2
                        }
                    );
                    if reacting {
                        assert!(end.events.iter().any(|e| matches!(e, Event::Moved(m) if m.card==extra && m.verb==Some(Verb::Score) && m.inside_karma)));
                    }
                    if foreseen {
                        assert!(end.game.position.cards(Zone::Score(B)).is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn kaleidoscope_lower_junk_oracle_preserves_foreseen_timing_and_rods_actual_loss() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E065", Some(Zone::Forecast(A)));
        let driver = tech(&mut f, Color::Red, 6, Zone::Hand(A));
        let drawn = deck(&mut f, CardSet::Base, 7, Color::Green);
        f.cards[usize::from(drawn.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        let lows: Vec<_> = [1, 2]
            .into_iter()
            .map(|age| tech(&mut f, Color::Yellow, age, Zone::AvailableAchievements))
            .collect();
        let high = tech(&mut f, Color::Blue, 4, Zone::AvailableAchievements);
        if reacting {
            figure(&mut f, Figure::RodSerling, B);
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
            .unwrap();
        let (reduced, manual) = oracle(game);
        assert_eq!(reduced.len(), if reacting { 2 } else { 1 });
        assert_eq!(manual.len(), 2);
        for end in reduced.iter().chain(&manual) {
            assert_eq!(
                end.game.position.locate(source),
                Some(if reacting {
                    Zone::Junk
                } else {
                    Zone::Board(A, Color::Purple)
                })
            );
            assert_eq!(
                end.game.position.locate(high),
                Some(Zone::AvailableAchievements)
            );
            assert_eq!(
                end.status,
                if reacting {
                    Status::Winner(B)
                } else {
                    Status::Complete(ReturnValue::Unit)
                }
            );
            for &low in &lows {
                assert_eq!(
                    end.game.position.locate(low),
                    Some(if reacting {
                        Zone::AvailableAchievements
                    } else {
                        Zone::Junk
                    })
                );
            }
        }
    }
}

#[test]
fn kaleidoscope_available_alias_oracle_keeps_cai_luns_disappearing_membership() {
    let mut f = Fixture::default();
    let source = card(&mut f, "E065", Some(Zone::Forecast(A)));
    let driver = tech(&mut f, Color::Red, 6, Zone::Hand(A));
    deck(&mut f, CardSet::Base, 7, Color::Green);
    let writing = f.add(base_card("B012").unwrap(), Zone::Board(A, Color::Blue));
    let cai = figure(&mut f, Figure::CaiLun, A);
    let middle = tech(&mut f, Color::Red, 3, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    let (reduced, manual) = oracle(game);
    assert_eq!(reduced.len(), 5);
    assert_eq!(manual.len(), 5);
    for paths in [&reduced, &manual] {
        assert!(
            paths
                .iter()
                .any(|end| end.game.position.locate(writing) == Some(Zone::Board(A, Color::Blue)))
        );
        assert!(
            paths
                .iter()
                .any(|end| end.game.position.locate(writing) == Some(Zone::Junk))
        );
        for end in paths {
            assert_eq!(end.game.position.locate(cai), Some(Zone::Junk));
            assert_eq!(
                end.game.position.locate(middle),
                Some(Zone::Junk),
                "original four Concepts survive Writing leaving"
            );
            assert_eq!(
                end.game.position.locate(source),
                Some(Zone::Board(A, Color::Purple))
            );
            assert_eq!(end.status, Status::Complete(ReturnValue::Unit));
        }
    }

    // A physical standards sweep is independent of Cai Lun's board aliases.
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Red,
        6,
        Zone::Board(A, Color::Red),
        vec![
            build::junk(Selector::StandardAchievements)
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    );
    let cai = figure(&mut f, Figure::CaiLun, A);
    let low = tech(&mut f, Color::Blue, 1, Zone::AvailableAchievements);
    let high = tech(&mut f, Color::Green, 2, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    let (reduced, manual) = oracle(game);
    assert_eq!(reduced.len(), 1);
    assert_eq!(manual.len(), 2);
    for end in reduced.iter().chain(&manual) {
        assert_eq!(
            end.game.position.locate(cai),
            Some(Zone::Board(A, Color::Yellow))
        );
        assert_eq!(end.game.position.locate(low), Some(Zone::Junk));
        assert_eq!(end.game.position.locate(high), Some(Zone::Junk));
    }
}

#[test]
fn kaleidoscope_large_concept_snapshot_handles_maxwell_and_254_hand_cards() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E065", foreseen.then_some(Zone::Forecast(A)));
        let maxwell = figure(&mut f, Figure::JamesClerkMaxwell, A);
        let hand: Vec<_> = (0..254)
            .map(|i| {
                let id = tech(&mut f, Color::Blue, (i % 11 + 1) as u8, Zone::Hand(A));
                if i / 11 % 2 == 0 {
                    f.cards[usize::from(id.0)].set = CardSet::Echoes;
                }
                id
            })
            .collect();
        let high = tech(&mut f, Color::Yellow, 11, Zone::AvailableAchievements);
        let driver = foreseen.then(|| tech(&mut f, Color::Red, 6, Zone::Hand(A)));
        if foreseen {
            deck(&mut f, CardSet::Base, 7, Color::Green);
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        assert_eq!(
            game.position.icon_count(A, Icon::Concept),
            if foreseen { 0 } else { 256 }
        );
        if let Some(driver) = driver {
            game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, 1, Context::new(A), 0);
        }
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events, |menu| {
                menu.options.iter().position(|option| matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == maxwell)).unwrap_or(0)
            }),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position.cards(Zone::Hand(A)), hand);
        assert_eq!(
            game.position.locate(high),
            Some(if foreseen {
                Zone::Junk
            } else {
                Zone::AvailableAchievements
            })
        );
        assert_eq!(
            game.position.locate(maxwell),
            Some(Zone::Board(A, Color::Blue))
        );
        assert_eq!(game.position.icon_count(A, Icon::Concept), 256);
        let junked: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Moved(m) if m.verb == Some(Verb::Junk) => Some(m.card),
                _ => None,
            })
            .collect();
        assert_eq!(junked, if foreseen { vec![high] } else { vec![] });
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Revealed { .. }))
        );
    }
}
