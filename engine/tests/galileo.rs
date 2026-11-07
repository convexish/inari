use inno_engine::*;

mod common;
use common::*;

fn draw(game: &mut Game, player: Player, value: u8, set: Option<CardSet>) {
    game.start(Operation::Draw { value, set }, Context::new(player))
        .unwrap();
}

fn choose_deck(game: &mut Game, events: &mut Vec<Event>, age: u8) {
    let menu = decision(game, events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options, [Alternative::Value(4), Alternative::Value(5)]);
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(age))
            .unwrap(),
    )
    .unwrap();
}

fn performed(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: false,
    }))
}

/// A draw that took a card above its requested age (an empty age was
/// skipped): performed, but short of the requested value.
fn escalated(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: true,
    }))
}

fn effect(game: &mut Game, card: CardId) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("Galileo did not terminate");
}

#[test]
fn both_base_decks_are_offered_unless_both_are_empty_even_if_figures_remain() {
    // Only the Base 4 and 5 decks count; a Figures 4 never keeps the choice.
    for nonempty in [None, Some(4), Some(5)] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        let expansion = f.figure(Figure::JohannesKepler, Zone::Deck(Deck::figures(4)));
        let kept = nonempty.map(|age| f.tech(Color::Red, age, Zone::Deck(Deck::base(age)), vec![]));
        let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, A, 1, None);
        let mut events = Vec::new();
        if let Some(age) = nonempty {
            // The empty deck is still offered: choosing it keeps the other.
            choose_deck(&mut game, &mut events, 9 - age);
            assert!(events.is_empty());
        }
        assert_eq!(settle(&mut game, &mut events), performed(drawn, 1));
        assert_eq!(events.len(), 1);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(4))),
            &[expansion]
        );
        assert!(game.position().cards(Zone::Junk).is_empty());
        if let (Some(age), Some(kept)) = (nonempty, kept) {
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(age))), &[kept]);
        }
    }
}

#[test]
fn sweep_is_base_only_physical_top_to_bottom_and_original_draw_skips_the_newly_empty_age() {
    for age in [4, 5] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        let bottom = f.tech(Color::Red, age, Zone::Deck(Deck::base(age)), vec![]);
        let top = f.tech(Color::Blue, age, Zone::Deck(Deck::base(age)), vec![]);
        let expansion = f.figure(Figure::JohannesKepler, Zone::Deck(Deck::figures(4)));
        let drawn = f.tech(Color::Yellow, 6, Zone::Deck(Deck::base(6)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, A, age, None);
        let mut events = Vec::new();
        choose_deck(&mut game, &mut events, age);
        assert_eq!(settle(&mut game, &mut events), escalated(drawn, 6));
        assert_eq!(
            events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
            [top, bottom, drawn]
        );
        for event in &events[..2] {
            assert_eq!(moved(event).verb, Some(Verb::Junk));
            assert_eq!(moved(event).source, Zone::Deck(Deck::base(age)));
            assert!(moved(event).inside_karma);
        }
        assert!(!moved(&events[2]).inside_karma);
        assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
        assert_eq!(game.position().cards(Zone::Junk), &[top, bottom]);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(4))),
            &[expansion]
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn figures_draw_reroutes_from_the_requested_age_and_keeps_figures_fallback_policy() {
    for figures_at_next_age in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        let junked = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
        let stranded = f.figure(Figure::JohannesKepler, Zone::Deck(Deck::figures(4)));
        let base = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
        let next = figures_at_next_age
            .then(|| f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7))));
        let later = f.figure(Figure::Kalashnikov, Zone::Deck(Deck::figures(9)));
        f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, A, 3, Some(CardSet::Figures));
        let mut events = Vec::new();
        choose_deck(&mut game, &mut events, 4);
        let expected = next.unwrap_or(base);
        assert_eq!(settle(&mut game, &mut events), escalated(expected, 7));
        assert_eq!(
            events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
            [junked, expected]
        );
        assert_eq!(game.position().cards(Zone::Hand(A)), &[expected]);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(4))),
            &[stranded]
        );
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(9))),
            &[later]
        );
    }
}

#[test]
fn printed_sailing_melds_its_draw_not_either_karma_child_result() {
    let mut f = Fixture::default();
    let sailing = f.sailing(Zone::Board(A, Color::Green));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let junked = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let achievement = f.tech(Color::Yellow, 3, Zone::AvailableAchievements, vec![]);
    let drawn = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, sailing);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 4);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [junked, drawn, achievement, drawn]
    );
    assert_eq!(
        events
            .iter()
            .map(|e| moved(e).inside_karma)
            .collect::<Vec<_>>(),
        [true, false, true, false]
    );
    assert_eq!(moved(&events[3]).verb, Some(Verb::Meld));
    assert_eq!(game.position().top(A, Color::Red), Some(drawn));
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Junk), &[junked, achievement]);
}

#[test]
fn achievement_menu_is_mandatory_hidden_and_filters_available_ordinary_values_only() {
    let mut f = Fixture::default();
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let candidates: Vec<_> = (1..=6)
        .map(|value| f.tech(Color::Blue, value, Zone::AvailableAchievements, vec![]))
        .collect();
    let own = f.tech(Color::Red, 3, Zone::Achievements(A), vec![]);
    let other = f.tech(Color::Red, 4, Zone::Achievements(B), vec![]);
    let pending = f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(pending, Zone::Hand(A), A, Color::Yellow),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 3);
    let values: Vec<_> = menu
        .options
        .iter()
        .map(|option| {
            let Alternative::BlindMove {
                verb: Verb::Junk,
                source,
                back: Deck { age: value, set },
            } = option
            else {
                panic!("achievement fronts must not enter the mandatory menu");
            };
            assert_eq!(*source, Zone::AvailableAchievements);
            assert_eq!(*set, CardSet::Base);
            *value
        })
        .collect();
    assert_eq!(values, [3, 4, 5]);
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[pending]);
    assert!(matches!(
        game.position()
            .descriptor(target(pending, Zone::Hand(A)), B),
        TargetDescriptor::Hidden { .. }
    ));
    let before = game.clone();
    assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
    assert_eq!(game, before);
    for (index, &value) in values.iter().enumerate() {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        fork.choose(index).unwrap();
        assert_eq!(settle(&mut fork, &mut suffix), performed(pending, 2));
        let selected = candidates[usize::from(value - 1)];
        assert_eq!(
            suffix.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
            [selected, pending]
        );
        assert_eq!(moved(&suffix[0]).verb, Some(Verb::Junk));
        assert!(moved(&suffix[0]).inside_karma);
        assert!(!moved(&suffix[1]).inside_karma);
        assert_eq!(fork.position().cards(Zone::Achievements(A)), &[own]);
        assert_eq!(fork.position().cards(Zone::Achievements(B)), &[other]);
        for &card in &candidates {
            assert_eq!(
                fork.position()
                    .cards(Zone::AvailableAchievements)
                    .contains(&card),
                card != selected
            );
        }
        for special in Special::ALL {
            assert_eq!(
                fork.position().special_location(special),
                SpecialLocation::Available
            );
        }
        assert!(fork.position().check_conservation());
    }
}

#[test]
fn no_eligible_achievement_is_a_noop_and_a_singleton_is_automatic() {
    for value in [None, Some(3), Some(4), Some(5)] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        let ineligible = f.tech(Color::Blue, 2, Zone::AvailableAchievements, vec![]);
        let eligible =
            value.map(|age| f.tech(Color::Blue, age, Zone::AvailableAchievements, vec![]));
        let pending = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(meld(pending, Zone::Hand(A), A, Color::Red), Context::new(A))
            .unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), performed(pending, 1));
        assert_eq!(events.len(), if eligible.is_some() { 2 } else { 1 });
        assert_eq!(game.position().cards(Zone::Junk), eligible.as_slice());
        assert_eq!(
            game.position().cards(Zone::AvailableAchievements),
            &[ineligible]
        );
        for special in Special::ALL {
            assert_eq!(
                game.position().special_location(special),
                SpecialLocation::Available
            );
        }
    }
}

#[test]
fn both_clauses_are_owner_only_and_a_covered_source_is_inactive() {
    for (executor, covered) in [(B, false), (A, true)] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        if covered {
            f.tech(Color::Green, 5, Zone::Board(A, Color::Green), vec![]);
        }
        let untouched = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        let achievement = f.tech(Color::Red, 3, Zone::AvailableAchievements, vec![]);
        let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut events = Vec::new();
        draw(&mut game, executor, 1, None);
        assert_eq!(settle(&mut game, &mut events), performed(drawn, 1));
        game.start(
            meld(drawn, Zone::Hand(executor), executor, Color::Yellow),
            Context::new(executor),
        )
        .unwrap();
        assert_eq!(settle(&mut game, &mut events), performed(drawn, 1));
        assert_eq!(events.len(), 2);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(4))),
            &[untouched]
        );
        assert_eq!(
            game.position().cards(Zone::AvailableAchievements),
            &[achievement]
        );
    }
}

#[test]
fn covering_galileo_finishes_the_current_first_but_disables_later_operations() {
    let mut f = Fixture::default();
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let achievement = f.tech(Color::Blue, 3, Zone::AvailableAchievements, vec![]);
    let cover = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let drawn = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(cover, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(cover, 1));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [achievement, cover]
    );
    draw(&mut game, A, 4, None);
    assert_eq!(settle(&mut game, &mut events), performed(drawn, 4));
    assert_eq!(game.position().cards(Zone::Junk), &[achievement]);
}

#[test]
fn competing_first_hooks_use_turn_controller_but_galileo_deck_choice_uses_owner() {
    for winner in [Figure::GalileoGalilei, Figure::AlexTrebek] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
        let four = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
        let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_turn(B).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        game.start(
            Operation::Draw {
                value: 1,
                set: None,
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        let hooks = decision(&mut game, &mut events);
        assert_eq!(hooks.player, B);
        assert_eq!(hooks.options.len(), 2);
        let index = hooks
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Karma { figure, .. } if *figure == winner))
            .unwrap();
        game.choose(index).unwrap();
        if winner == Figure::GalileoGalilei {
            choose_deck(&mut game, &mut events, 4);
        } else {
            let decks = decision(&mut game, &mut events);
            assert_eq!(decks.player, A);
            game.choose(
                decks
                    .options
                    .iter()
                    .position(|o| *o == Alternative::Deck(Deck::figures(1)))
                    .unwrap(),
            )
            .unwrap();
            let names = decision(&mut game, &mut events);
            game.choose(
                names
                    .options
                    .iter()
                    .position(|o| *o == Alternative::Pass)
                    .unwrap(),
            )
            .unwrap();
        }
        assert_eq!(settle(&mut game, &mut events), performed(drawn, 1));
        assert_eq!(
            game.position().cards(Zone::Junk).contains(&four),
            winner == Figure::GalileoGalilei
        );
        assert_eq!(
            events.len(),
            if winner == Figure::GalileoGalilei {
                2
            } else {
                3
            }
        );
    }
}

#[test]
fn maxwells_child_draw_does_not_run_galileos_first() {
    let mut f = Fixture::default();
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let four = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let returned = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Return,
            target: target(returned, Zone::Hand(B)),
            destination: Zone::Deck(Deck::base(1)),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(returned, 1));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [drawn, returned]
    );
    assert!(moved(&events[0]).inside_karma);
    assert!(!moved(&events[1]).inside_karma);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), &[four]);
}

#[test]
fn first_junks_before_original_meld_without_suppressing_its_fresh_when() {
    let mut f = Fixture::default();
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let emma = f.figure(Figure::EmmaWatson, Zone::Hand(A));
    let achievement = f.tech(Color::Yellow, 3, Zone::AvailableAchievements, vec![]);
    for (player, color) in [(A, Color::Purple), (B, Color::Blue)] {
        for age in [1, 2] {
            f.tech(color, age, Zone::Board(player, color), vec![]);
        }
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for (player, color) in [(A, Color::Purple), (B, Color::Blue)] {
        game.start(
            Operation::Splay {
                player,
                color,
                direction: Splay::Right,
            },
            Context::new(player),
        )
        .unwrap();
        settle(&mut game, &mut Vec::new());
    }
    game.start(meld(emma, Zone::Hand(A), A, Color::Purple), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(emma, 11));
    assert_eq!(events.len(), 4);
    assert_eq!(moved(&events[0]).card, achievement);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Junk));
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(moved(&events[1]).card, emma);
    assert_eq!(moved(&events[1]).verb, Some(Verb::Meld));
    assert!(!moved(&events[1]).inside_karma);
    for (event, (player, color)) in events[2..]
        .iter()
        .zip([(A, Color::Purple), (B, Color::Blue)])
    {
        assert_eq!(
            *event,
            Event::Splayed {
                player,
                color,
                from: Splay::Right,
                to: Splay::None,
                executor: A,
                inside_karma: true,
            }
        );
        assert_eq!(game.position().splay(player, color), Splay::None);
    }
    assert_eq!(game.position().top(A, Color::Purple), Some(emma));
}

#[test]
fn exhaustion_after_sweep_cancels_sailings_original_draw_and_meld() {
    let mut f = Fixture::default();
    let sailing = f.sailing(Zone::Board(A, Color::Green));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let last = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let achievement = f.tech(Color::Blue, 3, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, sailing);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 4);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, last);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Junk));
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        &[achievement]
    );
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert!(game.position().cards(Zone::Board(A, Color::Red)).is_empty());
}

#[test]
fn clones_at_every_boundary_preserve_deck_sweep_achievement_choice_and_compound_result() {
    let mut f = Fixture::default();
    let sailing = f.sailing(Zone::Board(A, Color::Green));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    for color in [Color::Red, Color::Blue, Color::Yellow] {
        f.tech(color, 4, Zone::Deck(Deck::base(4)), vec![]);
    }
    for age in [3, 4, 5] {
        f.tech(Color::Purple, age, Zone::AvailableAchievements, vec![]);
    }
    let drawn = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    effect(&mut game, sailing);
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    assert_eq!(result, Status::Complete(ReturnValue::Unit));
    assert_eq!(expected.position().top(A, Color::Red), Some(drawn));
    assert_eq!(expected_events.len(), 6);
    let mut events = Vec::new();
    let mut decks = 0;
    let mut achievements = 0;
    for step in 0..300 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => {
                if matches!(menu.options[0], Alternative::Value(_)) {
                    decks += 1;
                } else {
                    assert!(matches!(
                        menu.options[0],
                        Alternative::BlindMove {
                            verb: Verb::Junk,
                            source: Zone::AvailableAchievements,
                            ..
                        }
                    ));
                    achievements += 1;
                }
                game.choose(0).unwrap();
            }
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                assert_eq!((decks, achievements), (1, 1));
                return;
            }
        }
        assert!(game.position().check_conservation());
        assert!(step < 299, "Galileo did not terminate");
    }
}

#[test]
fn same_age_achievements_are_one_native_sample_independent_of_their_fronts() {
    let mut sampled = Vec::new();
    for colors in [[Color::Red, Color::Blue], [Color::Blue, Color::Red]] {
        let mut f = Fixture::default();
        f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
        let ties = colors.map(|color| f.tech(color, 3, Zone::AvailableAchievements, vec![]));
        let pending = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut events = Vec::new();
        game.start(
            Operation::ClaimSpecial {
                special: Special::World,
                check_eligibility: false,
            },
            Context::new(B),
        )
        .unwrap();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        events.clear();
        game.start(
            meld(pending, Zone::Hand(A), A, Color::Yellow),
            Context::new(A),
        )
        .unwrap();
        // Two identical [3] backs: no menu, one native sample.
        assert_eq!(settle(&mut game, &mut events), performed(pending, 1));
        let junk = game.position().cards(Zone::Junk);
        assert_eq!(junk.len(), 1);
        let slot = ties.iter().position(|&tie| tie == junk[0]).unwrap();
        assert_eq!(
            game.position().cards(Zone::AvailableAchievements),
            &[ties[1 - slot]]
        );
        for special in Special::ALL {
            assert_eq!(
                game.position().special_location(special),
                if special == Special::World {
                    SpecialLocation::Owned(B)
                } else {
                    SpecialLocation::Available
                }
            );
        }
        sampled.push(slot);
    }
    // The same seed takes the same physical slot whatever the fronts are.
    assert_eq!(sampled[0], sampled[1]);
}
