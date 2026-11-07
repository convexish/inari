use inno_engine::*;

mod common;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn invoke(game: &mut Game, card: CardId, effect: usize) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: effect.try_into().unwrap(),
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn stop(game: &mut Game, events: &mut Vec<Event>) {
    let menu = decision(game, events);
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Decline)
            .unwrap(),
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                game.choose(0).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                assert!(game.position().check_conservation());
                return;
            }
        }
    }
    panic!("batch did not complete");
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

fn masonry<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B006", Zone::Board(A, Color::Yellow));
    let first = base(&mut f, "B001", Zone::Hand(A));
    let second = base(&mut f, "B002", Zone::Hand(A));
    let green = base(&mut f, "B009", Zone::Hand(A));
    let excluded = base(&mut f, "B004", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let eligible = known(&menu);
    assert_eq!(eligible.len(), 3);
    for card in [first, second, green] {
        assert!(eligible.contains(&card));
    }
    assert!(!eligible.contains(&excluded));
    choose_card(&mut game, &mut events, second);
    stop(&mut game, &mut events);
    finish(&mut game, &mut events);
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        [first, green, excluded]
    );
    assert_eq!(game.position().top(A, Color::Red), Some(second));
    assert_eq!(events.len(), 1);
}

#[test]
fn masonry_keeps_genuine_printed_authority_subsets_in_both_kernels() {
    masonry::<false>();
    masonry::<true>();
}

fn monument<const F: bool>() {
    for count in [0, 2, 3, 4] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B006", Zone::Junk);
        for _ in 0..count {
            base(&mut f, "B001", Zone::Board(A, Color::Red));
        }
        base(&mut f, "B002", Zone::Board(B, Color::Red));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 1);
        finish(&mut game, &mut Vec::new());
        assert_eq!(
            game.position().special_location(Special::Monument),
            if count == 3 {
                SpecialLocation::Owned(A)
            } else {
                SpecialLocation::Available
            }
        );
    }
}

#[test]
fn masonry_counts_covered_physical_red_cards_exactly() {
    monument::<false>();
    monument::<true>();
}

fn translation<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B033", Zone::Board(A, Color::Blue));
    let first = base(&mut f, "B001", Zone::Score(A));
    let second = base(&mut f, "B002", Zone::Score(A));
    let green = base(&mut f, "B008", Zone::Score(A));
    let initial = f.position();
    let mut game = Game::new(initial.clone(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    let mut declined = game.clone();
    stop(&mut declined, &mut Vec::new());
    finish(&mut declined, &mut Vec::new());
    assert_eq!(declined.position(), &initial);
    choose_card(&mut game, &mut events, second);
    finish(&mut game, &mut events);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [second, first]
    );
    assert_eq!(game.position().top(A, Color::Green), Some(green));
}

#[test]
fn translation_commits_to_all_and_preserves_same_color_order() {
    translation::<false>();
    translation::<true>();
}

fn world<const F: bool>() {
    for (tops, claim) in [
        (vec![], true),
        (vec!["B033", "B008"], true),
        (vec!["B033", "B001"], false),
    ] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B033", Zone::Junk);
        for code in tops {
            let card = base_card(code).unwrap();
            let color = card.color;
            f.add(card, Zone::Board(A, color));
        }
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 1);
        finish(&mut game, &mut Vec::new());
        assert_eq!(
            game.position().special_location(Special::World),
            if claim {
                SpecialLocation::Owned(A)
            } else {
                SpecialLocation::Available
            }
        );
    }
}

#[test]
fn translation_universal_is_printed_live_and_vacuously_true() {
    world::<false>();
    world::<true>();
}

#[test]
fn translation_requeries_logical_score_after_covering_michelangelo() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B033", Zone::Junk);
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let cover = base(&mut f, "B004", Zone::Hand(A));
    let stays = base(&mut f, "B001", Zone::Hand(A));
    let physical = base(&mut f, "B008", Zone::Score(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    for card in [cover, stays, physical] {
        assert!(known(&menu).contains(&card));
    }
    choose_card(&mut game, &mut events, cover);
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [stays]);
    assert_eq!(game.position().top(A, Color::Green), Some(physical));
}

#[test]
fn translation_prevented_original_melds_are_not_retried() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B033", Zone::Junk);
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    let cards = [
        base(&mut f, "B001", Zone::Score(A)),
        base(&mut f, "B008", Zone::Score(A)),
    ];
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, cards[1]);
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), cards);
    assert!(events.is_empty());
}

fn socialism<const F: bool>() {
    for commit in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B085", Zone::Board(A, Color::Purple));
        let bottom = base(&mut f, "B001", Zone::Board(A, Color::Red));
        let top = base(&mut f, "B002", Zone::Board(A, Color::Red));
        let hand = base(&mut f, "B003", Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        if commit {
            choose_card(&mut game, &mut events, top);
        } else {
            stop(&mut game, &mut events);
        }
        finish(&mut game, &mut events);
        assert_eq!(
            game.position().cards(Zone::Board(A, Color::Red)),
            if commit {
                vec![hand, top, bottom]
            } else {
                vec![bottom, top]
            }
        );
        assert_eq!(game.position().cards(Zone::Hand(A)).is_empty(), commit);
        assert_eq!(events.len(), if commit { 2 } else { 0 });
    }
    // With no own top, the original optional tuck is inapplicable, not paid.
    let mut f = Fixture::default();
    let source = base(&mut f, "B085", Zone::Junk);
    let hand = base(&mut f, "B001", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().cards(Zone::Hand(A)), [hand]);
}

#[test]
fn socialism_tucks_top_then_all_hand_only_after_original_success() {
    socialism::<false>();
    socialism::<true>();
}

fn socialism_achievements<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B085", Zone::Junk);
    let eligible =
        ["B080", "B088", "B099"].map(|code| base(&mut f, code, Zone::AvailableAchievements));
    let other = base(&mut f, "B075", Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 1);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 4);
    let ages: Vec<_> = menu
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::BlindMove {
                verb: Verb::Junk,
                source: Zone::AvailableAchievements,
                back: Deck { age: value, .. },
            } => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(ages.len(), 3);
    for value in [8, 9, 10] {
        assert!(ages.contains(&value));
    }
    let index = menu
        .options
        .iter()
        .position(|option| {
            matches!(
                option,
                Alternative::BlindMove {
                    back: Deck { age: 9, .. },
                    ..
                }
            )
        })
        .unwrap();
    game.choose(index).unwrap();
    finish(&mut game, &mut events);
    assert!(game.position().cards(Zone::Junk).contains(&eligible[1]));
    assert!(
        game.position()
            .cards(Zone::AvailableAchievements)
            .contains(&other)
    );
}

#[test]
fn socialism_second_effect_is_independent_optional_achievement_junk() {
    socialism_achievements::<false>();
    socialism_achievements::<true>();
}

fn suburbia<const F: bool>() {
    for count in 0..=2 {
        let mut f = Fixture::default();
        let source = base(&mut f, "B089", Zone::Board(A, Color::Yellow));
        let hand = ["B001", "B002", "B003"].map(|code| base(&mut f, code, Zone::Hand(A)));
        let draws = ["B004", "B008"].map(|code| base(&mut f, code, Zone::Deck(Deck::base(1))));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 0);
        let mut events = Vec::new();
        for &card in &hand[..count] {
            choose_card(&mut game, &mut events, card);
        }
        stop(&mut game, &mut events);
        finish(&mut game, &mut events);
        assert_eq!(game.position().cards(Zone::Hand(A)), &hand[count..]);
        assert_eq!(
            game.position().cards(Zone::Score(A)),
            &draws.iter().rev().take(count).copied().collect::<Vec<_>>()
        );
        let movements: Vec<_> = events.iter().map(moved).collect();
        assert_eq!(movements.len(), count * 3);
        assert!(
            movements[..count]
                .iter()
                .all(|m| m.verb == Some(Verb::Tuck))
        );
        for pair in movements[count..].as_chunks::<2>().0 {
            assert_eq!(pair[0].card, pair[1].card);
            assert_eq!(pair[0].source, Zone::Deck(Deck::base(1)));
            assert_eq!(pair[1].verb, Some(Verb::Score));
        }
    }
}

#[test]
fn suburbia_stopping_preserves_success_count_and_draw_score_order() {
    suburbia::<false>();
    suburbia::<true>();
}

fn suburbia_deck<const F: bool>() {
    for commit in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B089", Zone::Junk);
        let deck = ["B088", "B091"].map(|code| base(&mut f, code, Zone::Deck(Deck::base(9))));
        let higher = base(&mut f, "B099", Zone::Deck(Deck::base(10)));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 1);
        let mut events = Vec::new();
        if commit {
            let menu = decision(&mut game, &mut events);
            let index = menu
                .options
                .iter()
                .position(|o| matches!(o, Alternative::Move { .. }))
                .unwrap();
            game.choose(index).unwrap();
        } else {
            stop(&mut game, &mut events);
        }
        finish(&mut game, &mut events);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(9))).is_empty(),
            commit
        );
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), [higher]);
        if commit {
            assert_eq!(
                events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                [deck[1], deck[0]]
            );
        }
    }
}

#[test]
fn suburbia_second_effect_sweeps_only_physical_nine_after_commitment() {
    suburbia_deck::<false>();
    suburbia_deck::<true>();
}

#[test]
fn socialism_requeries_hand_after_initial_top_tuck_removes_meiji_membership() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B085", Zone::Board(A, Color::Purple));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let own = base(&mut f, "B001", Zone::Hand(A));
    let borrowed = base(&mut f, "B008", Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, meiji);
    finish(&mut game, &mut events);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Purple)),
        [meiji, source]
    );
    assert_eq!(game.position().top(A, Color::Red), Some(own));
    assert_eq!(game.position().cards(Zone::Hand(B)), [borrowed]);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [meiji, own]
    );
}

#[test]
fn suburbia_reward_is_boxed_but_number_of_original_tucks_is_not() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B089", Zone::Junk);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let tucked = base(&mut f, "B001", Zone::Hand(A));
    let remaining = base(&mut f, "B002", Zone::Hand(A));
    let untouched = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, tucked);
    stop(&mut game, &mut events);
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [remaining]);
    assert_eq!(game.position().cards(Zone::Score(A)), [reward]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        [untouched]
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [tucked, reward, reward]
    );
}

#[test]
fn suburbia_replaced_scores_do_not_change_the_original_tuck_reward_count() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B089", Zone::Junk);
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    let tucked = ["B001", "B002"].map(|code| base(&mut f, code, Zone::Hand(A)));
    let rewards = ["B008", "B009"].map(|code| base(&mut f, code, Zone::Deck(Deck::base(1))));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    for card in tucked {
        choose_card(&mut game, &mut events, card);
    }
    finish(&mut game, &mut events);
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        [rewards[1], rewards[0]]
    );
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Moved(m) if m.source == Zone::Deck(Deck::base(1))))
            .count(),
        2
    );
}

#[test]
fn socialism_achievement_ages_are_ordinary_values_not_boxed_operands() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B085", Zone::Junk);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    for code in ["B080", "B088", "B099", "B115"] {
        base(&mut f, code, Zone::AvailableAchievements);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    stop(&mut game, &mut events);
    let menu = decision(&mut game, &mut events);
    let ages: Vec<_> = menu
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::BlindMove {
                verb: Verb::Junk,
                source: Zone::AvailableAchievements,
                back: Deck { age: value, .. },
            } => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(ages.len(), 3);
    for value in [8, 9, 10] {
        assert!(ages.contains(&value));
    }
    assert!(!ages.contains(&11));
    stop(&mut game, &mut events);
    finish(&mut game, &mut events);
    assert!(events.is_empty());
}

#[test]
fn suburbia_empty_nine_never_routes_to_a_higher_deck() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B089", Zone::Junk);
    let higher = base(&mut f, "B099", Zone::Deck(Deck::base(10)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 1);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), [higher]);
}

#[test]
fn socialism_tucking_a_top_card_into_its_own_pile_keeps_the_splay() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B085", Zone::Junk);
    let bottom = base(&mut f, "B001", Zone::Board(A, Color::Red));
    let top = base(&mut f, "B002", Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Red,
            direction: Splay::Right,
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, top);
    finish(&mut game, &mut events);
    // The pile never shrinks below two cards, so it stays splayed.
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [top, bottom]
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
}
