use inno_engine::*;

mod common;
use common::*;

fn printed(f: &mut Fixture, code: &str) -> CardId {
    let card = base_card(code).unwrap();
    let zone = Zone::Board(A, card.color);
    f.add(card, zone)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("query program did not finish");
}

fn done(game: &mut Game, events: &mut Vec<Event>) {
    assert_eq!(finish(game, events), Status::Complete(ReturnValue::Unit));
    assert!(game.position().check_conservation());
}

fn splay(game: &mut Game, color: Color, direction: Splay) {
    game.start(
        Operation::Splay {
            player: A,
            color,
            direction,
        },
        Context::new(A),
    )
    .unwrap();
    finish(game, &mut Vec::new());
    assert_eq!(game.position().splay(A, color), direction);
}

fn known(menu: &Decision) -> Vec<CardId> {
    menu.options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(*card),
            _ => None,
        })
        .collect()
}

fn movements(events: &[Event]) -> Vec<(Option<Verb>, CardId)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.verb, m.card)),
            _ => None,
        })
        .collect()
}

#[test]
fn domestication_keeps_every_lowest_tie_and_draws_after_the_selected_meld() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B005");
    let low = [Color::Red, Color::Blue].map(|color| f.tech(color, 1, Zone::Hand(A), vec![]));
    let high = f.tech(Color::Green, 3, Zone::Hand(A), vec![]);
    let drawn = f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let position = f.position();
    for selected in low {
        let mut game = Game::new(position.clone(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(known(&menu), low);
        assert!(!menu.options.contains(&Alternative::Decline));
        game.choose(usize::from(selected == low[1])).unwrap();
        done(&mut game, &mut events);
        assert_eq!(
            movements(&events),
            [(Some(Verb::Meld), selected), (None, drawn)]
        );
        assert!(game.position().cards(Zone::Hand(A)).contains(&high));
    }
}

#[test]
fn domestication_draw_is_not_conditional_on_having_a_card_to_meld() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B005");
    let drawn = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(movements(&events), [(None, drawn)]);
}

#[test]
fn clothing_filters_board_colors_then_runs_separate_draw_score_pairs() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B007");
    let ineligible = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let red = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let blue = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Purple, 1, Zone::Board(B, Color::Purple), vec![]);
    let second = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let first = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(known(&decision(&mut game, &mut events)), [red, blue]);
    game.choose(0).unwrap();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [
            (Some(Verb::Meld), red),
            (None, first),
            (Some(Verb::Score), first),
            (None, second),
            (Some(Verb::Score), second),
        ]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), [ineligible, blue]);
}

#[test]
fn clothing_zero_exclusive_colors_draws_nothing() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B007");
    f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert!(events.is_empty());
}

#[test]
fn code_of_laws_has_two_independent_optional_decisions_for_only_the_tucked_color() {
    for tuck in [false, true] {
        for do_splay in [false, true] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "B014");
            let purple = f.tech(Color::Purple, 1, Zone::Hand(A), vec![]);
            let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
            for _ in 0..2 {
                f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
            }
            let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
            game.self_execute(source, Context::new(A)).unwrap();
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(known(&menu), [purple]);
            assert_eq!(menu.options.last(), Some(&Alternative::Decline));
            game.choose(if tuck { 0 } else { 1 }).unwrap();
            if tuck {
                let menu = decision(&mut game, &mut events);
                assert_eq!(
                    menu.options,
                    [
                        Alternative::Splay {
                            player: A,
                            color: Color::Purple,
                            direction: Splay::Left
                        },
                        Alternative::Decline
                    ]
                );
                game.choose(if do_splay { 0 } else { 1 }).unwrap();
            }
            done(&mut game, &mut events);
            assert_eq!(
                game.position().splay(A, Color::Purple),
                if tuck && do_splay {
                    Splay::Left
                } else {
                    Splay::None
                }
            );
            assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
            assert!(game.position().cards(Zone::Hand(A)).contains(&red));
            if tuck {
                assert_eq!(
                    game.position().cards(Zone::Board(A, Color::Purple)),
                    [purple, source]
                );
            }
        }
    }
}

#[test]
fn mysticism_reveals_checks_live_color_and_continues_after_covering_its_source() {
    for color in [Color::Purple, Color::Red] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "B015");
        let reward = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let drawn = f.tech(color, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        done(&mut game, &mut events);
        assert!(matches!(events[1], Event::Revealed { card, .. } if card == drawn));
        if color == Color::Purple {
            assert_eq!(
                movements(&events),
                [(None, drawn), (Some(Verb::Meld), drawn), (None, reward)]
            );
            assert_eq!(
                game.position().cards(Zone::Board(A, color)),
                [source, drawn]
            );
        } else {
            assert_eq!(movements(&events), [(None, drawn)]);
            assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
        }
    }
}

#[test]
fn paper_scores_only_a_printed_health_top_and_counts_left_colors_after_scoring() {
    let mut f = Fixture::default();
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    let source = printed(&mut f, "B031");
    for _ in 0..2 {
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    }
    let buried = f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    f.cards[usize::from(buried.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let health = f.tech(Color::Yellow, 2, Zone::Board(A, Color::Yellow), vec![]);
    f.cards[usize::from(health.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let second = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let first = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    splay(&mut game, Color::Blue, Splay::Left);
    splay(&mut game, Color::Yellow, Splay::Left);
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options,
        [
            Alternative::Splay {
                player: A,
                color: Color::Green,
                direction: Splay::Left
            },
            Alternative::Decline
        ]
    );
    game.choose(0).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [health]);
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::None);
    assert_eq!(
        movements(&events),
        [(Some(Verb::Score), health), (None, first), (None, second)]
    );
}

#[test]
fn paper_does_not_use_effective_board_icon_counts_or_buried_health() {
    let mut f = Fixture::default();
    let buried = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.cards[usize::from(buried.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let source = printed(&mut f, "B031");
    f.figure(Figure::WilliamShakespeare, Zone::Board(A, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let decline = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Decline)
        .unwrap();
    game.choose(decline).unwrap();
    done(&mut game, &mut events);
    assert!(movements(&events).is_empty());
}

#[test]
fn education_keeps_highest_ties_and_queries_the_remaining_score_after_return() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B034");
    let low = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let highs = [Color::Red, Color::Green].map(|c| f.tech(c, 3, Zone::Score(A), vec![]));
    let drawn = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(known(&menu), highs);
    assert_eq!(menu.options.last(), Some(&Alternative::Decline));
    game.choose(1).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [low, highs[0]]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
}

#[test]
fn education_uses_zero_when_returning_the_only_score_and_two_above_a_lower_remainder() {
    for remaining in [None, Some(2)] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "B034");
        if let Some(value) = remaining {
            f.tech(Color::Blue, value, Zone::Score(A), vec![]);
        }
        let returned = f.tech(Color::Red, 7, Zone::Score(A), vec![]);
        let age = remaining.unwrap_or(0) + 2;
        let drawn = f.tech(Color::Green, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, returned);
        done(&mut game, &mut events);
        assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
    }
}

#[test]
fn printing_press_uses_live_purple_or_zero_and_keeps_its_optional_blue_splay() {
    for purple_age in [None, Some(3)] {
        let mut f = Fixture::default();
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        let source = printed(&mut f, "B043");
        if let Some(value) = purple_age {
            f.tech(Color::Purple, value, Zone::Board(A, Color::Purple), vec![]);
        }
        let returned = f.tech(Color::Red, 8, Zone::Score(A), vec![]);
        let age = purple_age.unwrap_or(0) + 2;
        let drawn = f.tech(Color::Green, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, returned);
        let menu = decision(&mut game, &mut events);
        assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
        assert_eq!(
            menu.options,
            [
                Alternative::Splay {
                    player: A,
                    color: Color::Blue,
                    direction: Splay::Right
                },
                Alternative::Decline
            ]
        );
        game.choose(0).unwrap();
        done(&mut game, &mut events);
        assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
    }
}

#[test]
fn optional_score_payments_can_be_declined_without_a_computed_draw() {
    for code in ["B034", "B043"] {
        let mut f = Fixture::default();
        let source = printed(&mut f, code);
        let held = f.tech(Color::Red, 1, Zone::Score(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        game.choose(
            menu.options
                .iter()
                .position(|o| *o == Alternative::Decline)
                .unwrap(),
        )
        .unwrap();
        done(&mut game, &mut events);
        assert_eq!(game.position().cards(Zone::Score(A)), [held]);
        assert!(events.is_empty());
    }
}

#[test]
fn reformation_counts_all_splay_directions_and_tucks_up_to_that_many() {
    for (hand_count, stop_after) in [(0, 0), (1, 1), (3, 2), (3, 1)] {
        let mut f = Fixture::default();
        f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
        let source = printed(&mut f, "B045");
        for _ in 0..2 {
            f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        }
        let hand: Vec<_> = (0..hand_count)
            .map(|_| f.tech(Color::Red, 1, Zone::Hand(A), vec![]))
            .collect();
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        splay(&mut game, Color::Blue, Splay::Up);
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options[0],
            Alternative::Splay {
                player: A,
                color: Color::Purple,
                direction: Splay::Right
            }
        );
        game.choose(0).unwrap();
        // Two splayed colors: every tuck may be the last one.
        for tucked in 0..stop_after {
            let menu = decision(&mut game, &mut events);
            assert_eq!(known(&menu), hand[tucked..]);
            assert_eq!(menu.options.last(), Some(&Alternative::Decline));
            game.choose(0).unwrap();
        }
        if stop_after < hand_count.min(2) {
            let menu = decision(&mut game, &mut events);
            game.choose(menu.options.len() - 1).unwrap();
        }
        done(&mut game, &mut events);
        assert_eq!(movements(&events).len(), stop_after);
        assert_eq!(
            game.position().cards(Zone::Board(A, Color::Red)),
            hand[..stop_after].iter().rev().copied().collect::<Vec<_>>()
        );
    }
}

#[test]
fn reformation_declining_the_first_tuck_tucks_nothing() {
    let mut f = Fixture::default();
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    let source = printed(&mut f, "B045");
    let held = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    splay(&mut game, Color::Purple, Splay::Right);
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(known(&menu), [held]);
    game.choose(menu.options.len() - 1).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [held]);
    assert!(events.is_empty());
}

#[test]
fn chemistry_uses_highest_physical_top_then_offers_every_logical_score_for_return() {
    let mut f = Fixture::default();
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let source = printed(&mut f, "B052");
    f.tech(Color::Red, 9, Zone::Board(A, Color::Red), vec![]); // Buried, not a top operand.
    f.tech(Color::Red, 6, Zone::Board(A, Color::Red), vec![]);
    let old = f.tech(Color::Yellow, 8, Zone::Score(A), vec![]);
    let drawn = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.last(), Some(&Alternative::Decline));
    game.choose(0).unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(known(&menu), [old, drawn]);
    assert!(!menu.options.contains(&Alternative::Decline));
    game.choose(1).unwrap();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [
            (None, drawn),
            (Some(Verb::Score), drawn),
            (Some(Verb::Return), drawn)
        ]
    );
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
}

#[test]
fn machine_tools_empty_score_draws_from_zero_and_skips_empty_ages_normally() {
    for age in [1, 3] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "B057");
        let drawn = f.tech(Color::Green, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        done(&mut game, &mut events);
        assert_eq!(
            movements(&events),
            [(None, drawn), (Some(Verb::Score), drawn)]
        );
    }
}

#[test]
fn machine_tools_empty_score_draws_a_real_introduced_age_zero() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B057");
    let zero = f.tech(Color::Green, 0, Zone::AgeZeroReserve, vec![]);
    let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(Operation::IntroduceAgeZero, Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [zero]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), [one]);
}

#[test]
fn machine_tools_uses_michelangelos_own_hand_but_not_meijis_borrowed_hand() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B057");
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let own = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let borrowed = f.tech(Color::Purple, 9, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [own]);
    assert_eq!(game.position().cards(Zone::Hand(B)), [borrowed]);
    assert_eq!(
        movements(&events),
        [(None, drawn), (Some(Verb::Score), drawn)]
    );
}

#[test]
fn education_can_return_michelangelos_physical_hand_and_recomputes_the_logical_remainder() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B034");
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Red, 6, Zone::Hand(A), vec![]);
    let remaining = f.tech(Color::Green, 2, Zone::Score(A), vec![]);
    let drawn = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
    assert_eq!(game.position().cards(Zone::Score(A)), [remaining]);
    assert_eq!(moved(&events[0]).source, Zone::Hand(A));
}

#[test]
fn domestication_selects_the_lowest_borrowed_meiji_hand_card_with_its_physical_owner() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B005");
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let own = f.tech(Color::Blue, 4, Zone::Hand(A), vec![]);
    let borrowed = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [own, drawn]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(moved(&events[0]).source, Zone::Hand(B));
    assert_eq!(game.position().top(A, Color::Red), Some(borrowed));
}

#[test]
fn filtered_hidden_fronts_never_control_an_unauthorized_menu() {
    for code in ["B007", "B014"] {
        for hidden_color in [Color::Purple, Color::Red] {
            let mut f = Fixture::default();
            let source = printed(&mut f, code);
            f.tech(hidden_color, 1, Zone::Hand(A), vec![]);
            let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
            let mut context = Context::new(A);
            context.controller = B;
            game.self_execute(source, context).unwrap();
            let mut events = Vec::new();
            assert_eq!(
                game.advance(1000, &mut events),
                Err(Error::Unsupported("private filtered card selection"))
            );
            assert!(events.is_empty());
        }
    }
}

#[test]
fn extrema_of_hidden_cards_use_public_back_values_without_exposing_fronts() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B005");
    let ties = [Color::Red, Color::Blue].map(|color| f.tech(color, 1, Zone::Hand(A), vec![]));
    let higher = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    game.self_execute(source, context).unwrap();
    // B sees only two [1] backs: one native sample, not a menu of slots.
    let mut events = Vec::new();
    assert!(!matches!(
        settle(&mut game, &mut events),
        Status::Decision(_)
    ));
    let hand = game.position().cards(Zone::Hand(A));
    assert_eq!(ties.iter().filter(|card| hand.contains(card)).count(), 1);
    assert!(hand.contains(&higher));
}

#[test]
fn color_icon_and_extremum_filters_compose_without_losing_ties() {
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(
                Verb::Score,
                build::filtered(
                    &build::HAND,
                    &[
                        Filter::Has(Test::Color(Color::Red)),
                        Filter::Has(Test::Icon(Icon::Health)),
                        Filter::Highest,
                    ],
                ),
            )
            .to(Destination::Score(Who::Executor)),
        ],
    );
    let ties = [0, 1].map(|_| {
        let card = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Health);
        card
    });
    f.tech(Color::Red, 3, Zone::Hand(A), vec![]); // Wrong printed icon.
    let blue = f.tech(Color::Blue, 4, Zone::Hand(A), vec![]);
    f.cards[usize::from(blue.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(known(&decision(&mut game, &mut events)), ties);
    game.choose(1).unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [ties[1]]);
}

#[test]
fn nightingale_replaced_payments_cannot_unlock_education_or_printing_press() {
    for code in ["B034", "B043"] {
        let mut f = Fixture::default();
        let source = printed(&mut f, code);
        f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
        let payment = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
        let untouched = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, payment);
        done(&mut game, &mut events);
        assert!(events.is_empty());
        assert_eq!(game.position().cards(Zone::Score(A)), [payment]);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(5))),
            [untouched]
        );
    }
}

#[test]
fn paper_replaced_score_does_not_draw_even_when_the_child_splay_succeeds() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B031");
    let shen = f.figure(Figure::ShenKuo, Zone::Hand(A));
    for _ in 0..2 {
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    }
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    }
    let health = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(health.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let untouched = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    splay(&mut game, Color::Blue, Splay::Left);
    game.start(meld(shen, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Decline)
            .unwrap(),
    )
    .unwrap();
    done(&mut game, &mut events);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
    assert_eq!(game.position().top(A, Color::Red), Some(health));
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(4))),
        [untouched]
    );
    assert!(movements(&events).is_empty());
}

#[test]
fn mysticism_reward_uses_the_revealed_color_not_the_replaced_meld_result() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B015");
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let reward = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let drawn = f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let chain = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [
            (None, drawn),
            (Some(Verb::Meld), drawn),
            (None, chain),
            (Some(Verb::Achieve), chain),
            (None, reward)
        ]
    );
    assert!(events.iter().any(|event| matches!(event, Event::Moved(m) if m.card == drawn && m.verb == Some(Verb::Meld) && m.inside_karma)));
    assert_eq!(game.position().cards(Zone::Hand(A)), [reward]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), [chain]);
}

#[test]
fn every_new_computed_age_ignores_archimedes_boxed_increase() {
    for (code, age) in [("B034", 2), ("B043", 2), ("B052", 6), ("B057", 3)] {
        let mut f = Fixture::default();
        let source = printed(&mut f, code);
        f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
        if code != "B052" {
            f.tech(Color::Red, 3, Zone::Score(A), vec![]);
        }
        // Chemistry's source is covered by Archimedes, so keep another age-5 physical top.
        if code == "B052" {
            f.tech(Color::Green, 5, Zone::Board(A, Color::Green), vec![]);
        }
        let drawn = f.tech(Color::Yellow, age, Zone::Deck(Deck::base(age)), vec![]);
        // A modified age would draw this card instead. Education may also
        // return its payment to this deck, so its whole contents are not fixed.
        f.tech(Color::Red, age + 1, Zone::Deck(Deck::base(age + 1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        let mut events = Vec::new();
        done(&mut game, &mut events);
        let draws: Vec<_> = movements(&events)
            .into_iter()
            .filter_map(|(verb, card)| verb.is_none().then_some(card))
            .collect();
        assert_eq!(draws, [drawn], "{code}");
    }
}

#[test]
fn clothing_boxed_draws_increase_under_archimedes_but_the_color_count_does_not() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B007");
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let unmodified = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let second = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let first = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [
            (None, first),
            (Some(Verb::Score), first),
            (None, second),
            (Some(Verb::Score), second)
        ]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        [unmodified]
    );
}

#[test]
fn keplers_action_increases_clothing_boxes_not_its_snapshot_count_or_machine_tools_age() {
    for code in ["B007", "B057"] {
        let mut f = Fixture::default();
        let source = printed(&mut f, code);
        f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
        // Machine Tools is age 6, so Kepler sees {4,6}; Clothing sees only {4}.
        let age = if code == "B007" { 2 } else { 3 };
        if code == "B057" {
            f.tech(Color::Purple, 3, Zone::Score(A), vec![]);
        }
        let second = f.tech(Color::Yellow, age, Zone::Deck(Deck::base(age)), vec![]);
        let first = f.tech(Color::Red, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        done(&mut game, &mut events);
        let drawn: Vec<_> = movements(&events)
            .into_iter()
            .filter(|(verb, _)| verb.is_none())
            .map(|(_, card)| card)
            .collect();
        assert_eq!(
            drawn,
            if code == "B007" {
                vec![first, second]
            } else {
                vec![first]
            }
        );
        assert!(
            events
                .iter()
                .filter_map(|event| match event {
                    Event::Moved(m) => Some(m),
                    _ => None,
                })
                .all(|m| m.inside_karma)
        );
    }
}

#[test]
fn sequential_count_is_snapshotted_before_score_reactions_change_its_operand() {
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::draws(
                Expr::Count(&Selector::Filtered(
                    &build::TOPS,
                    &[Filter::Has(Test::StackSplayed(Some(Splay::Left)))],
                )),
                1,
                Then::Score,
            ),
            build::if_(build::DID_ANY, 1),
            build::draw(2),
        ],
    );
    for color in [Color::Red, Color::Blue] {
        for _ in 0..2 {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
        }
    }
    let shen = f.figure(Figure::ShenKuo, Zone::Hand(A));
    let second = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let first = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let untouched = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for color in [Color::Red, Color::Blue] {
        splay(&mut game, color, Splay::Left);
    }
    game.start(meld(shen, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [first, second]);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
    // Neither replacement splay counts as an original score.
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        [untouched]
    );
    assert_eq!(movements(&events), [(None, first), (None, second)]);
}

fn clone_every_boundary(mut game: Game) {
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = finish(&mut expected, &mut all);
    let mut prefix = Vec::new();
    for _ in 0..500 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(_) => game.choose(0).unwrap(),
            terminal => {
                assert_eq!(terminal, result);
                assert_eq!(game, expected);
                return;
            }
        }
    }
    panic!("continuation did not terminate");
}

#[test]
fn draw_score_continuations_clone_at_every_boundary_in_both_kernels_and_unwind_on_exhaustion() {
    for cards in [1, 2] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "B007");
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        for _ in 0..cards {
            f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        let position = f.position();
        let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
        let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
        base.self_execute(source, Context::new(A)).unwrap();
        figures.self_execute(source, Context::new(A)).unwrap();
        clone_every_boundary(base.clone());
        clone_every_boundary(figures.clone());
        let mut events = Vec::new();
        let status = finish(&mut figures, &mut events);
        assert_eq!(
            status,
            if cards == 1 {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
        assert_eq!(movements(&events).len(), cards * 2);
        let mut base_events = Vec::new();
        assert_eq!(finish(&mut base, &mut base_events), status);
        assert_eq!(base_events, events);
        assert_eq!(base.position(), figures.position());
        if cards == 1 {
            assert_eq!(settle(&mut figures, &mut events), Status::Winner(A));
            assert_eq!(events.len(), 2);
        }
    }
}

#[test]
fn paper_draw_only_continuation_preserves_boxed_scope_and_clones_between_each_draw() {
    let mut f = Fixture::default();
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    let source = printed(&mut f, "B031");
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let health = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(health.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let second = f.tech(Color::Red, 5, Zone::Deck(Deck::base(5)), vec![]);
    let first = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let unmodified = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    splay(&mut game, Color::Blue, Splay::Left);
    game.self_execute(source, Context::new(A)).unwrap();
    clone_every_boundary(game.clone());
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [(Some(Verb::Score), health), (None, first), (None, second)]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(4))),
        [unmodified]
    );
}

#[test]
fn reformation_ordinary_count_is_not_boxed_and_preserves_remaining_subset_choices() {
    let mut f = Fixture::default();
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    let source = printed(&mut f, "B045");
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let hand =
        [Color::Red, Color::Yellow, Color::Green].map(|c| f.tech(c, 1, Zone::Hand(A), vec![]));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    splay(&mut game, Color::Blue, Splay::Up);
    game.self_execute(source, Context::new(A)).unwrap();
    clone_every_boundary(game.clone());
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(0).unwrap(); // Optional purple splay, then count is two.
    choose_card(&mut game, &mut events, hand[0]);
    choose_card(&mut game, &mut events, hand[2]);
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [hand[1]]);
    assert_eq!(
        movements(&events),
        [(Some(Verb::Tuck), hand[0]), (Some(Verb::Tuck), hand[2])]
    );
}

#[test]
fn chemistry_still_returns_a_score_card_after_its_score_is_replaced() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B052");
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    }
    let payment = f.tech(Color::Yellow, 3, Zone::Score(A), vec![]);
    let drawn = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [(None, drawn), (Some(Verb::Return), payment)]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
}

#[test]
fn education_retains_its_invocation_after_returning_the_source_card_itself() {
    let mut f = Fixture::default();
    let source = f.add(base_card("B034").unwrap(), Zone::Score(A));
    let drawn = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    clone_every_boundary(game.clone());
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [(Some(Verb::Return), source), (None, drawn)]
    );
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(3)))
            .contains(&source)
    );
}

#[test]
fn computed_highest_top_is_zero_without_a_physical_top() {
    let mut f = Fixture::default();
    let source = f.add(base_card("B052").unwrap(), Zone::Junk);
    let drawn = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(
        movements(&events),
        [
            (None, drawn),
            (Some(Verb::Score), drawn),
            (Some(Verb::Return), drawn)
        ]
    );
}

#[test]
fn kepler_suppression_survives_every_draw_pair_and_clone_boundary() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "B007");
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    let held = f.figure(Figure::Archimedes, Zone::Hand(A));
    let draws = [Color::Purple, Color::Yellow, Color::Red]
        .map(|c| f.tech(c, 3, Zone::Deck(Deck::base(3)), vec![]));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    clone_every_boundary(game.clone());
    let mut events = Vec::new();
    done(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [held]);
    let prefix = movements(&events);
    assert_eq!(
        &prefix[..6],
        &[
            (None, draws[2]),
            (Some(Verb::Score), draws[2]),
            (None, draws[1]),
            (Some(Verb::Score), draws[1]),
            (None, draws[0]),
            (Some(Verb::Score), draws[0]),
        ]
    );
    assert!(!prefix.iter().any(|(_, card)| *card == held));
}
