//! Age-eight clauses, hidden opponent score choices and complete shortcut paths.
use super::common::*;
use super::*;

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
    let action = tech(&mut f, Color::Red, 11, Zone::Hand(A));
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
        if let Some(tracker) = game.tracker(viewer) {
            assert!(tracker.check_truth(&game.position));
        }
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
fn age8_catalog_clauses_are_complete() {
    for (code, echo, count, demand) in [
        ("E078", true, 2, true),
        ("E079", true, 3, false),
        ("E080", false, 2, false),
        ("E081", true, 1, false),
        ("E082", false, 1, false),
        ("E085", true, 1, false),
    ] {
        let card = echoes_card(code).unwrap();
        assert_eq!(
            (
                card.echo.is_some(),
                card.effects.len(),
                card.effects[0].demand()
            ),
            (echo, count, demand),
            "{code}"
        );
    }
    assert!(size_of::<Instruction>() <= 96);
    assert_eq!(size_of::<Frame>(), 56);
}

#[test]
fn air_conditioner_echo_is_optional_and_demand_matches_values_across_sets() {
    for pass in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E078");
        let hand = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
        let mut game = start(f, source, Card::ECHO, A, false);
        finish(&mut game, |menu| if pass { decline(menu) } else { 0 });
        assert_eq!(
            game.position.locate(hand),
            Some(if pass { Zone::Hand(A) } else { Zone::Score(A) })
        );
    }
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E078");
    tech(&mut f, Color::Red, 3, Zone::Board(B, Color::Red));
    tech(&mut f, Color::Blue, 6, Zone::Board(B, Color::Blue));
    let one = tech(&mut f, Color::Green, 3, Zone::Score(B));
    let two = tech(&mut f, Color::Yellow, 6, Zone::Score(B));
    f.cards[usize::from(two.0)].set = CardSet::Echoes;
    let untouched = tech(&mut f, Color::Blue, 4, Zone::Score(B));
    let own = tech(&mut f, Color::Purple, 3, Zone::Score(A));
    let mut game = start(f, source, 0, B, false);
    let events = finish(&mut game, |_| 0);
    assert_eq!(game.position.locate(one), Some(Zone::Deck(Deck::base(3))));
    assert_eq!(
        game.position.locate(two),
        Some(Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 6
        }))
    );
    assert_eq!(game.position.locate(untouched), Some(Zone::Score(B)));
    assert_eq!(game.position.locate(own), Some(Zone::Score(A)));
    assert!(revealed(&events).is_empty(), "the filter reads backs only");
}

#[test]
fn air_conditioner_junks_only_the_boxed_base_deck_without_an_order_menu() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E078");
        let targets = [
            deck(&mut f, Color::Red, 9 + increase),
            deck(&mut f, Color::Green, 9 + increase),
        ];
        let other = deck(&mut f, Color::Blue, 8);
        let expansion = tech(
            &mut f,
            Color::Purple,
            9 + increase,
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 9 + increase,
            }),
        );
        f.cards[usize::from(expansion.0)].set = CardSet::Echoes;
        let mut game = Game::seeded(f.position(), E, 41)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut boxed = BoxedValues::ZERO;
        boxed.increase(increase).unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 1,
                boxed,
            },
            Context::new(A),
        )
        .unwrap();
        finish(&mut game, |_| panic!("deck sweep has no order choice"));
        assert!(
            targets
                .iter()
                .all(|c| game.position.locate(*c) == Some(Zone::Junk))
        );
        assert_eq!(game.position.locate(other), Some(Zone::Deck(Deck::base(8))));
        assert_eq!(
            game.position.locate(expansion),
            Some(Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 9 + increase
            }))
        );
    }
}

#[test]
fn tractor_echo_and_separate_dogmas_draw_and_score_only_their_own_card() {
    for effect in [Card::ECHO, 0, 1] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E079");
        let old = tech(&mut f, Color::Red, 2, Zone::Hand(A));
        let drawn = deck(&mut f, Color::Green, 7);
        let mut game = start(f, source, effect, A, false);
        finish(&mut game, |_| 0);
        assert_eq!(
            game.position.locate(drawn),
            Some(if effect == 0 {
                Zone::Score(A)
            } else {
                Zone::Hand(A)
            })
        );
        assert_eq!(game.position.locate(old), Some(Zone::Hand(A)));
    }
}

#[test]
fn tractor_foreseen_scores_seven_interleaved_draws_and_checks_gate() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E079");
        let drawn: Vec<_> = (0..8).map(|_| deck(&mut f, Color::Green, 7)).collect();
        let mut game = if foreseen {
            promoted(f, source, 2)
        } else {
            start(f, source, 2, A, false)
        };
        let events = finish(&mut game, |_| 0);
        let movements: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) if drawn.contains(&m.card) => Some((m.card, m.verb)),
                _ => None,
            })
            .collect();
        assert_eq!(movements.len(), if foreseen { 14 } else { 0 });
        for pair in movements.chunks(2) {
            assert_eq!(pair[0].0, pair[1].0);
            assert_eq!((pair[0].1, pair[1].1), (None, Some(Verb::Score)));
        }
        assert_eq!(
            drawn
                .iter()
                .filter(|c| game.position.locate(**c) == Some(Zone::Score(A)))
                .count(),
            if foreseen { 7 } else { 0 }
        );
    }
}

#[test]
fn tractor_draw_routing_uses_echoes_only_after_base_age_resolution() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E079");
    tech(&mut f, Color::Blue, 8, Zone::Board(A, Color::Blue));
    let skipped = tech(
        &mut f,
        Color::Green,
        7,
        Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 7,
        }),
    );
    f.cards[usize::from(skipped.0)].set = CardSet::Echoes;
    let base = deck(&mut f, Color::Red, 8);
    let chosen = tech(
        &mut f,
        Color::Yellow,
        8,
        Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 8,
        }),
    );
    f.cards[usize::from(chosen.0)].set = CardSet::Echoes;
    let mut game = start(f, source, 0, A, false);
    finish(&mut game, |_| 0);
    assert_eq!(game.position.locate(chosen), Some(Zone::Score(A)));
    assert_eq!(game.position.locate(base), Some(Zone::Deck(Deck::base(8))));
    assert_eq!(
        game.position.locate(skipped),
        Some(Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 7
        }))
    );
}

#[test]
fn nylon_interleaves_three_tucks_then_repeats_and_clears_old_green() {
    for green_at in [0, 2] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E080");
        let mut cards = Vec::new();
        // Deck top is the end: the seventh card proves the second round stops.
        for index in (0..7).rev() {
            cards.push(deck(
                &mut f,
                if index == green_at {
                    Color::Green
                } else {
                    Color::Red
                },
                8,
            ));
        }
        cards.reverse();
        let mut game = start(f, source, 0, A, false);
        let events = finish(&mut game, |_| 0);
        let movements: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) if cards.contains(&m.card) => Some((m.card, m.verb)),
                _ => None,
            })
            .collect();
        assert_eq!(movements.len(), 12);
        for (index, pair) in movements.chunks(2).enumerate() {
            assert_eq!(
                pair,
                [(cards[index], None), (cards[index], Some(Verb::Tuck))]
            );
        }
        assert_eq!(
            game.position.locate(cards[6]),
            Some(Zone::Deck(Deck::base(8)))
        );
    }
}

#[test]
fn nylon_replaced_green_tuck_does_not_repeat_or_count_karma_melds() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E080");
    let figure = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
    f.add(figure.clone(), Zone::Board(A, figure.color));
    let spare = deck(&mut f, Color::Yellow, 8);
    for color in [Color::Yellow, Color::Yellow, Color::Green] {
        deck(&mut f, color, 8);
    }
    let mut game = start(f, source, 0, A, true);
    let events = finish(&mut game, |_| 0);
    assert_eq!(game.position.locate(spare), Some(Zone::Deck(Deck::base(8))));
    assert_eq!(
        events
            .iter()
            .filter(
                |e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Tuck) && !m.inside_karma)
            )
            .count(),
        2
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Meld) && m.inside_karma))
    );
}

#[test]
fn nylon_red_up_is_optional() {
    for pass in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E080");
        for color in [Color::Red, Color::Blue] {
            for _ in 0..2 {
                tech(&mut f, color, 1, Zone::Board(A, color));
            }
        }
        let mut game = start(f, source, 1, A, false);
        finish(&mut game, |menu| if pass { decline(menu) } else { 0 });
        assert_eq!(
            game.position.splay(A, Color::Red),
            if pass { Splay::None } else { Splay::Up }
        );
        assert_eq!(game.position.splay(A, Color::Blue), Splay::None);
    }
}

#[test]
fn sliced_bread_echo_finishes_returns_before_two_draws() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E081");
    let originals = [
        tech(&mut f, Color::Red, 2, Zone::Hand(A)),
        tech(&mut f, Color::Blue, 3, Zone::Hand(A)),
    ];
    let drawn = [
        deck(&mut f, Color::Green, 8),
        deck(&mut f, Color::Yellow, 8),
    ];
    let mut game = start(f, source, Card::ECHO, A, false);
    let events = finish(&mut game, |_| 0);
    assert!(
        originals
            .iter()
            .all(|c| matches!(game.position.locate(*c), Some(Zone::Deck(_))))
    );
    assert!(
        drawn
            .iter()
            .all(|c| game.position.locate(*c) == Some(Zone::Hand(A)))
    );
    let moves: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(
        moves.iter().map(|m| m.verb).collect::<Vec<_>>(),
        [Some(Verb::Return), Some(Verb::Return), None, None]
    );
}

#[test]
fn sliced_bread_uses_successful_value_and_draws_zero_when_return_absent_or_replaced() {
    for mode in 0..3 {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E081");
        let original = (mode != 0).then(|| tech(&mut f, Color::Blue, 5, Zone::Score(A)));
        if mode == 2 {
            let figure = figure_card(Figure::Nightingale.code()).unwrap();
            f.add(figure.clone(), Zone::Board(A, figure.color));
        }
        let age = if mode == 1 { 4 } else { 0 };
        let drawn = [Color::Green, Color::Yellow].map(|color| {
            tech(
                &mut f,
                color,
                age,
                if age == 0 {
                    Zone::AgeZeroReserve
                } else {
                    Zone::Deck(Deck::base(age))
                },
            )
        });
        let mut game = Game::seeded(f.position(), EF, 41)
            .unwrap()
            .with_tracking()
            .unwrap();
        if age == 0 {
            game.start(Operation::IntroduceAgeZero, Context::new(A))
                .unwrap();
            finish(&mut game, |_| 0);
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
        finish(&mut game, |_| 0);
        assert!(
            drawn
                .iter()
                .all(|c| game.position.locate(*c) == Some(Zone::Score(A)))
        );
        if let Some(original) = original {
            assert_eq!(
                game.position.locate(original),
                Some(if mode == 1 {
                    Zone::Deck(Deck::base(5))
                } else {
                    Zone::Score(A)
                })
            );
        }
    }
}

#[test]
fn radio_telescope_counts_concept_colors_and_all_foreseen_colors_before_melding() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E082");
        if !foreseen {
            f.placements[usize::from(source.0)].1 = Zone::Board(A, Color::Blue);
        }
        for color in [Color::Red, Color::Green, Color::Yellow, Color::Purple] {
            let top = tech(&mut f, color, 4, Zone::Board(A, color));
            if color == Color::Green && !foreseen {
                f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(Icon::Concept);
            }
        }
        let unrelated = tech(&mut f, Color::Red, 9, Zone::Hand(A));
        let nines: Vec<_> = (0..3).map(|_| deck(&mut f, Color::Green, 9)).collect();
        let tens: Vec<_> = (0..6).map(|_| deck(&mut f, Color::Yellow, 10)).collect();
        let selected = nines[2];
        let mut game = if foreseen {
            promoted(f, source, 0)
        } else {
            start(f, source, 0, A, false)
        };
        let events = finish(&mut game, |menu| choose_card(menu, selected));
        let drawn: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m)
                    if m.verb.is_none() && (nines.contains(&m.card) || tens.contains(&m.card)) =>
                {
                    Some(m.card)
                }
                _ => None,
            })
            .collect();
        assert_eq!(drawn.iter().filter(|c| nines.contains(c)).count(), 2);
        assert_eq!(
            drawn.iter().filter(|c| tens.contains(c)).count(),
            if foreseen { 5 } else { 0 }
        );
        assert_eq!(
            game.position.locate(selected),
            Some(Zone::Board(A, Color::Green))
        );
        assert_eq!(game.position.locate(unrelated), Some(Zone::Hand(A)));
        assert!(
            drawn
                .iter()
                .filter(|c| **c != selected)
                .all(|c| matches!(game.position.locate(*c), Some(Zone::Deck(_))))
        );
        let first_meld = events.iter().position(|e| matches!(e, Event::Moved(m) if m.card == selected && m.verb == Some(Verb::Meld))).unwrap();
        assert!(
            events
                .iter()
                .enumerate()
                .filter_map(|(i, e)| match e {
                    Event::Moved(m) if m.verb.is_none() && drawn.contains(&m.card) => Some(i),
                    _ => None,
                })
                .all(|i| i < first_meld)
        );
    }
}

#[test]
fn radio_telescope_keeps_only_original_draws_when_champlain_adds_a_private_card() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E082");
    let concept = tech(&mut f, Color::Yellow, 4, Zone::Board(A, Color::Yellow));
    f.cards[usize::from(concept.0)].symbols[0] = Symbol::Icon(Icon::Concept);
    let figure = figure_card(Figure::SamuelDeChamplain.code()).unwrap();
    f.add(figure.clone(), Zone::Board(A, figure.color));
    let extra = deck(&mut f, Color::Red, 6);
    let original = deck(&mut f, Color::Blue, 9);
    let mut game = start(f, source, 0, A, true);
    let events = finish(&mut game, |menu| choose_card(menu, original));
    assert_eq!(
        game.position.locate(original),
        Some(Zone::Board(A, Color::Blue))
    );
    assert_eq!(game.position.locate(extra), Some(Zone::Hand(A)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Moved(m) if m.card == extra && m.inside_karma))
    );
}

#[test]
fn radio_telescope_wins_for_original_ai_meld_only_after_all_returns() {
    for replaced in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E082");
        for color in [Color::Green, Color::Yellow] {
            let top = tech(&mut f, color, 4, Zone::Board(A, color));
            f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        }
        if replaced {
            let figure = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
            f.add(figure.clone(), Zone::Board(A, figure.color));
        }
        let other = deck(&mut f, Color::Red, 10);
        let ai = f.add(base_card("B104").unwrap(), Zone::Deck(Deck::base(10)));
        let mut game = start(f, source, 0, A, replaced);
        let events = finish(&mut game, |menu| choose_card(menu, ai));
        let status = game.advance(1000, &mut Vec::new()).unwrap();
        assert_eq!(matches!(status, Status::Winner(_)), !replaced);
        if !replaced {
            assert_eq!(
                game.position.locate(other),
                Some(Zone::Deck(Deck::base(10)))
            );
            let returns = events
                .iter()
                .filter(|e| matches!(e, Event::Moved(m) if m.verb == Some(Verb::Return)))
                .count();
            assert_eq!(returns, 1, "the win cannot preempt remaining returns");
        } else {
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Moved(m) if m.card == ai && m.inside_karma))
            );
        }
    }
}

#[test]
fn radio_telescope_without_concept_draws_or_old_ai_in_hand_does_nothing() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E082");
    let old = f.add(base_card("B104").unwrap(), Zone::Hand(A));
    let reserve = deck(&mut f, Color::Yellow, 9);
    let mut game = start(f, source, 0, A, false);
    let events = finish(&mut game, |_| panic!("empty original packet"));
    assert_eq!(game.position.locate(old), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(reserve),
        Some(Zone::Deck(Deck::base(9)))
    );
    assert!(events.iter().all(|e| !matches!(e, Event::Moved(_))));
}

#[test]
fn television_echo_melds_its_draw_and_value_selects_opponent_score_backs() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E085");
    let drawn = deck(&mut f, Color::Blue, 8);
    let mut game = start(f, source, Card::ECHO, A, false);
    finish(&mut game, |_| 0);
    assert_eq!(
        game.position.locate(drawn),
        Some(Zone::Board(A, Color::Blue))
    );

    for can_claim in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E085");
        tech(&mut f, Color::Green, 8, Zone::Board(A, Color::Green));
        tech(&mut f, Color::Purple, 10, Zone::Score(A));
        let candidates = [
            tech(&mut f, Color::Red, 2, Zone::Score(B)),
            tech(&mut f, Color::Blue, 2, Zone::Score(B)),
        ];
        let untouched = tech(&mut f, Color::Yellow, 3, Zone::Score(B));
        if can_claim {
            tech(&mut f, Color::Green, 2, Zone::Achievements(B));
        }
        let mut game = start(f, source, 0, A, false);
        let mut events = Vec::new();
        let Status::Decision(value) = game.advance(1000, &mut events).unwrap() else {
            panic!("value choice");
        };
        game.choose(
            value
                .options
                .iter()
                .position(|o| *o == Alternative::Value(2))
                .unwrap(),
        )
        .unwrap();
        let mut expected_rng = game.rng.clone();
        let transferred = candidates[(expected_rng.next_u64() % 2) as usize];
        events.extend(finish(&mut game, |menu| {
            assert!(
                menu.options.iter().all(|o| matches!(
                    o,
                    Alternative::BlindMove { .. } | Alternative::BlindSelect { .. }
                )),
                "opponent score faces stay private"
            );
            0
        }));
        assert_eq!(
            game.position.locate(transferred),
            Some(Zone::Board(B, game.position.card(transferred).color))
        );
        assert_eq!(
            candidates
                .iter()
                .filter(|c| game.position.locate(**c) == Some(Zone::Achievements(A)))
                .count(),
            usize::from(can_claim)
        );
        assert_eq!(game.position.locate(untouched), Some(Zone::Score(B)));
        assert!(revealed(&events).is_empty());
    }
}

#[test]
fn television_claim_is_independent_of_replaced_transfer_but_requires_eligibility() {
    for eligible in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E085");
        let figure = figure_card(Figure::Nightingale.code()).unwrap();
        f.add(figure.clone(), Zone::Board(B, figure.color));
        tech(&mut f, Color::Green, 8, Zone::Board(A, Color::Green));
        tech(
            &mut f,
            Color::Yellow,
            if eligible { 10 } else { 9 },
            Zone::Score(A),
        );
        tech(&mut f, Color::Purple, 2, Zone::Achievements(B));
        let target = tech(&mut f, Color::Red, 2, Zone::Score(B));
        let mut game = start(f, source, 0, A, true);
        let events = finish(&mut game, |menu| {
            menu.options
                .iter()
                .position(|o| *o == Alternative::Value(2))
                .unwrap_or(0)
        });
        assert_eq!(
            game.position.locate(target),
            Some(if eligible {
                Zone::Achievements(A)
            } else {
                Zone::Score(B)
            })
        );
        assert!(!events.iter().any(
            |e| matches!(e, Event::Moved(m) if m.card == target && m.verb == Some(Verb::Transfer))
        ));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct OutcomeView {
    position: Position,
    rng: Rng,
    status: Status,
    beliefs: [Vec<Vec<crate::tracking::BeliefSlot>>; 3],
    views: [Vec<u32>; 2],
}
fn paths(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<(OutcomeView, Vec<Event>)> {
    assert!(depth < 24, "bounded age-eight oracle");
    match game.advance(1000, &mut events).unwrap() {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|i| {
                let mut child = game.clone();
                child.choose(i).unwrap();
                paths(child, events.clone(), depth + 1)
            })
            .collect(),
        status => {
            assert!(!matches!(status, Status::WorkLimit));
            assert!(game.position.check_conservation());
            let trackers = &game.knowledge.as_ref().unwrap().trackers;
            let beliefs = std::array::from_fn(|observer| {
                crate::tracking::zones()
                    .into_iter()
                    .map(|zone| trackers[observer].belief(zone))
                    .collect()
            });
            let views = Player::ALL.map(|p| {
                game.observe(p)
                    .unwrap()
                    .row()
                    .into_iter()
                    .map(f32::to_bits)
                    .collect()
            });
            game.position.sort_unordered_piles();
            vec![(
                OutcomeView {
                    position: game.position,
                    rng: game.rng,
                    status,
                    beliefs,
                    views,
                },
                events,
            )]
        }
    }
}

#[test]
fn age8_return_shortcuts_match_every_path_with_actual_reacting_figures() {
    for code in ["E078", "E081", "E082"] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = echo_source(&mut f, code);
            let (mut executor, mut effect) = (A, 0);
            match code {
                "E078" => {
                    executor = B;
                    for (color, age) in [(Color::Green, 1), (Color::Yellow, 2)] {
                        tech(&mut f, color, age, Zone::Board(B, color));
                        tech(&mut f, color, age, Zone::Score(B));
                    }
                }
                "E081" => {
                    effect = Card::ECHO;
                    tech(&mut f, Color::Green, 1, Zone::Hand(A));
                    tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
                    for _ in 0..2 {
                        deck(&mut f, Color::Blue, 8);
                    }
                }
                "E082" => {
                    let occupied = if reacting {
                        let figure = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
                        f.add(figure.clone(), Zone::Board(A, figure.color));
                        Some(figure.color)
                    } else {
                        None
                    };
                    for color in Color::ALL
                        .into_iter()
                        .filter(|c| Some(*c) != occupied)
                        .take(3)
                    {
                        let top = tech(&mut f, color, 4, Zone::Board(A, color));
                        f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(Icon::Concept);
                    }
                    for age in [9, 10, 11] {
                        deck(&mut f, Color::Green, age);
                    }
                }
                _ => unreachable!(),
            }
            if reacting && code != "E082" {
                let figure = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
                f.add(figure.clone(), Zone::Board(B, figure.color));
                for _ in 0..2 {
                    deck(&mut f, Color::Purple, 8);
                }
            }
            let game = start(f, source, effect, executor, reacting);
            let mut unrestricted = game.clone();
            unrestricted.shortcuts_disabled = true;
            let reduced = paths(game, Vec::new(), 0);
            let full = paths(unrestricted, Vec::new(), 0);
            assert!(
                reduced
                    .iter()
                    .all(|(outcome, _)| full.iter().any(|(other, _)| outcome == other)),
                "{code}: invented outcome"
            );
            assert!(
                full.iter()
                    .all(|(outcome, _)| reduced.iter().any(|(other, _)| outcome == other)),
                "{code}: lost outcome"
            );
            if reacting {
                assert!(
                    reduced.iter().any(|(_, events)| events
                        .iter()
                        .any(|e| matches!(e,Event::Moved(m) if m.inside_karma))),
                    "{code}: Figure must actually react"
                );
            } else {
                assert!(
                    reduced.len() < full.len(),
                    "{code}: expected a redundant order to disappear"
                );
            }
        }
    }
}

#[test]
fn television_mixed_back_choices_keep_the_checked_claim_private_through_first_and_samples() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E085");
        tech(&mut f, Color::Red, 8, Zone::Board(A, Color::Red));
        tech(&mut f, Color::Purple, 10, Zone::Score(A));
        for figure in [Figure::HedyLamarr, Figure::ZhangHeng] {
            let printed = figure_card(figure.code()).unwrap();
            tech(&mut f, printed.color, 1, Zone::Board(A, printed.color));
            f.add(printed.clone(), Zone::Board(A, printed.color));
        }
        deck(&mut f, Color::Purple, 3);
        tech(&mut f, Color::Purple, 2, Zone::Achievements(B));
        let hidden = [Color::Red, Color::Green, Color::Blue]
            .map(|color| tech(&mut f, color, 2, Zone::Score(B)));
        let expansion = tech(&mut f, Color::Yellow, 2, Zone::Score(B));
        f.cards[usize::from(expansion.0)].set = CardSet::Echoes;
        let mut game = start(f, source, 0, A, true);
        if !tracked {
            game.knowledge = None;
        }
        let mut events = Vec::new();
        let Status::Decision(values) = game.advance(1000, &mut events).unwrap() else {
            panic!("values");
        };
        game.choose(
            values
                .options
                .iter()
                .position(|o| *o == Alternative::Value(2))
                .unwrap(),
        )
        .unwrap();
        let Status::Decision(backs) = game.advance(1000, &mut events).unwrap() else {
            panic!("mixed back transfer");
        };
        assert_eq!(backs.player, A);
        assert_eq!(
            backs.options,
            [CardSet::Base, CardSet::Echoes].map(|set| Alternative::BlindSelect {
                source: Zone::Score(B),
                back: Deck { set, age: 2 },
            })
        );
        game.choose(1).unwrap();
        let Status::Decision(first) = game.advance(1000, &mut events).unwrap() else {
            panic!("claim First ordering");
        };
        assert!(
            first
                .options
                .iter()
                .all(|o| matches!(o, Alternative::Karma { .. }))
        );
        assert_eq!(
            game.position.locate(expansion),
            Some(Zone::Board(B, Color::Yellow))
        );
        let (selected, evidence) = game
            .frames
            .iter()
            .find_map(|frame| match frame {
                Frame::Operation {
                    request: Operation::Claim { target, .. },
                    knowledge: Some(known),
                    ..
                } => Some((target.card, known.evidence)),
                _ => None,
            })
            .expect("pending checked claim retains its private operand");
        assert!(hidden.contains(&selected));
        assert!(!evidence.known(A));
        assert_eq!(game.position.locate(selected), Some(Zone::Score(B)));
        assert!(revealed(&events).is_empty());
        if tracked {
            let observation = game.observe(A).unwrap();
            let mut different = false;
            for seed in 0..24 {
                for mut world in [
                    game.determinize(A, seed).unwrap(),
                    game.sample_world(A, seed).unwrap().world,
                ] {
                    if world.knowledge.is_some() {
                        assert_eq!(world.observe(A).unwrap(), observation);
                    }
                    let Status::Decision(sampled) = world.advance(1000, &mut Vec::new()).unwrap()
                    else {
                        panic!("sampled First menu");
                    };
                    assert_eq!(sampled.player, first.player);
                    assert_eq!(sampled.options, first.options);
                    assert_eq!(sampled.source_card(A), first.source_card(A));
                    for option in 0..first.options.len() {
                        assert_eq!(
                            sampled.option_label(option, A),
                            first.option_label(option, A)
                        );
                    }
                    different |= world.frames.iter().any(|frame|matches!(frame,Frame::Operation{request:Operation::Claim{target,..},..} if target.card!=selected));
                    finish(&mut world, |_| 0);
                    assert_eq!(world.position.cards(Zone::Achievements(A)).len(), 1);
                }
            }
            assert!(
                different,
                "pending private claim must really resample its hidden identity"
            );
        }
        let suffix = finish(&mut game, |_| 0);
        assert!(
            suffix
                .iter()
                .any(|event| matches!(event,Event::Moved(m) if m.inside_karma))
        );
        assert_eq!(game.position.locate(selected), Some(Zone::Achievements(A)));
    }
}

#[test]
fn tractor_foreseen_box_adjustment_changes_age_not_seven_count_and_exhaustion_stops() {
    for supply in [3, 8] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E079");
        let figure = figure_card(Figure::Archimedes.code()).unwrap();
        f.add(figure.clone(), Zone::Board(A, figure.color));
        let untouched = deck(&mut f, Color::Red, 7);
        let drawn: Vec<_> = (0..supply).map(|_| deck(&mut f, Color::Green, 8)).collect();
        let mut game = promoted(f, source, 2);
        finish(&mut game, |_| 0);
        assert_eq!(
            drawn
                .iter()
                .filter(|card| game.position.locate(**card) == Some(Zone::Score(A)))
                .count(),
            supply.min(7)
        );
        assert_eq!(
            game.position.locate(untouched),
            Some(Zone::Deck(Deck::base(7)))
        );
        assert_eq!(
            matches!(
                game.advance(1000, &mut Vec::new()).unwrap(),
                Status::Winner(_)
            ),
            supply == 3
        );
    }
}

#[test]
fn radio_telescope_freezes_both_color_counts_before_a_first_draw_adds_a_color() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E082");
    let figure = figure_card(Figure::Rhazes.code()).unwrap();
    f.add(figure.clone(), Zone::Board(A, figure.color));
    let new_color = tech(&mut f, Color::Green, 9, Zone::Hand(A));
    for _ in 0..3 {
        deck(&mut f, Color::Red, 9);
    }
    let tens: Vec<_> = (0..6).map(|_| deck(&mut f, Color::Red, 10)).collect();
    // Promotion starts with Radio blue, Rhazes yellow, the purple boost,
    // and the red action card: four colors. Rhazes adds green before draw 1.
    let mut game = promoted(f, source, 0);
    let events = finish(&mut game, |_| 0);
    assert_eq!(
        game.position.locate(new_color),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(events.iter().filter(|e|matches!(e,Event::Moved(m) if m.verb.is_none() && !m.inside_karma && tens.contains(&m.card))).count(),4);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==new_color && m.inside_karma))
    );
}

#[test]
fn air_conditioner_deck_top_oracle_includes_archimedes_actual_box_reaction() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = echo_source(&mut f, "E078");
        if reacting {
            let figure = figure_card(Figure::Archimedes.code()).unwrap();
            f.add(figure.clone(), Zone::Board(A, figure.color));
        }
        let nines = [deck(&mut f, Color::Red, 9), deck(&mut f, Color::Green, 9)];
        let tens = [
            deck(&mut f, Color::Blue, 10),
            deck(&mut f, Color::Yellow, 10),
        ];
        let game = start(f, source, 1, A, reacting);
        let mut unrestricted = game.clone();
        unrestricted.shortcuts_disabled = true;
        let reduced = paths(game, Vec::new(), 0);
        let full = paths(unrestricted, Vec::new(), 0);
        assert!(reduced.len() < full.len());
        assert!(
            full.iter()
                .all(|(outcome, _)| reduced.iter().any(|(other, _)| outcome == other))
        );
        assert!(
            reduced
                .iter()
                .all(|(outcome, _)| full.iter().any(|(other, _)| outcome == other))
        );
        for (outcome, _) in reduced {
            for card in nines {
                assert_eq!(
                    outcome.position.locate(card),
                    Some(if reacting {
                        Zone::Deck(Deck::base(9))
                    } else {
                        Zone::Junk
                    })
                );
            }
            for card in tens {
                assert_eq!(
                    outcome.position.locate(card),
                    Some(if reacting {
                        Zone::Junk
                    } else {
                        Zone::Deck(Deck::base(10))
                    })
                );
            }
        }
    }
}

#[test]
fn air_conditioner_live_matching_admits_a_new_value_exposed_by_napoleon() {
    let mut f = Fixture::default();
    let source = echo_source(&mut f, "E078");
    let figure = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
    let napoleon = f.add(figure.clone(), Zone::Board(B, figure.color));
    tech(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue));
    tech(&mut f, Color::Yellow, 3, Zone::Board(B, Color::Yellow));
    let five = tech(&mut f, Color::Yellow, 5, Zone::Board(B, Color::Yellow));
    let first = tech(&mut f, Color::Purple, 2, Zone::Score(B));
    let newly_matching = tech(&mut f, Color::Green, 3, Zone::Score(B));
    let mut game = start(f, source, 0, B, true);
    let events = finish(&mut game, |menu| choose_card(menu, five));
    assert_eq!(
        game.position.locate(first),
        Some(Zone::Board(B, Color::Purple))
    );
    assert_eq!(
        game.position.locate(newly_matching),
        Some(Zone::Board(B, Color::Green))
    );
    assert_eq!(game.position.locate(five), Some(Zone::Score(B)));
    assert_eq!(game.position.locate(napoleon), Some(Zone::Score(B)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==five && m.inside_karma))
    );
}
