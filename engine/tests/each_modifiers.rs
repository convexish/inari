use inno_engine::*;

mod common;
use common::*;

fn icons(f: &mut Fixture, card: CardId, symbols: &[Icon]) {
    f.cards[usize::from(card.0)].symbols = [Symbol::Blank; SLOTS];
    for (slot, &icon) in symbols.iter().enumerate() {
        f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
    }
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("effect did not terminate");
}

#[test]
fn gilgamesh_adds_independent_icons_and_allows_negative_opponent_scores() {
    let mut f = Fixture::default();
    let gilgamesh = f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    let own = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    icons(&mut f, own, &[Icon::Authority; 3]);
    let opponent = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    icons(&mut f, opponent, &[Icon::Authority; 4]);
    f.tech(Color::Yellow, 2, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().basic_icon_count(A, Icon::Authority), 4);
    assert_eq!(game.position().icon_count(A, Icon::Authority), 8);
    assert_eq!(game.position().icon_count(A, Icon::Industry), 4);
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .icon_count(A, Icon::Authority),
        4
    );
    assert_eq!(game.position().score(B), -2);
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .score(B),
        2
    );
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(gilgamesh, Zone::Board(A, Color::Red)),
            destination: Zone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().icon_count(A, Icon::Authority), 3);
    assert_eq!(game.position().icon_count(A, Icon::Industry), 0);
    assert_eq!(game.position().score(B), 2);
}

#[test]
fn maxwells_added_authority_does_not_feed_gilgameshs_score_penalty() {
    let mut f = Fixture::default();
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(B, Color::Blue));
    let top = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    icons(&mut f, top, &[Icon::Authority]);
    for _ in 0..3 {
        f.tech(Color::Yellow, 1, Zone::Hand(B), vec![]);
    }
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.icon_count(B, Icon::Authority), 4);
    assert_eq!(position.score(B), 6); // Printed bonus 7 minus one basic Authority.
}

#[test]
fn maxwell_gilgamesh_and_nobel_do_not_recursively_feed_each_other() {
    let mut f = Fixture::default();
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
    for _ in 0..5 {
        f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    }
    let opponent = f.tech(Color::Yellow, 1, Zone::Board(B, Color::Yellow), vec![]);
    icons(
        &mut f,
        opponent,
        &[
            Icon::Authority,
            Icon::Efficiency,
            Icon::Prosperity,
            Icon::Concept,
        ],
    );
    let avatars = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
    icons(&mut f, avatars, &[Icon::Avatar; 2]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.icon_count(A, Icon::Authority), 7); // 1 + Gilgamesh 1 + hand 5.
    assert_eq!(position.icon_count(A, Icon::Industry), 1); // Not a basic type for Maxwell.
    assert_eq!(position.icon_count(A, Icon::Efficiency), 6);
    assert_eq!(position.icon_count(A, Icon::Prosperity), 7);
    assert_eq!(position.icon_count(A, Icon::Avatar), 8);
    assert_eq!(position.effective_achievement_count(A), 1); // Only basic Prosperity qualifies.
    assert_eq!(
        position
            .clone()
            .with_expansions(Expansions::BASE)
            .effective_achievement_count(A),
        0
    );
    assert_eq!(position.achievement_count(A), 0);
}

#[test]
fn maxwell_counts_physical_hand_not_meijis_added_membership() {
    let mut f = Fixture::default();
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    for _ in 0..4 {
        f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    }
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.targets(LogicalZone::Hand(A)).count(), 5);
    assert_eq!(position.basic_icon_count(A, Icon::Efficiency), 3);
    assert_eq!(position.icon_count(A, Icon::Efficiency), 4);
}

#[test]
fn modified_icons_drive_demand_vulnerability_not_only_query_outputs() {
    let mut f = Fixture::default();
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![build::draw(1)],
    );
    f.cards[usize::from(source.0)].effects[0].kind = EffectKind::Demand;
    let opponent = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    icons(&mut f, opponent, &[Icon::Authority]);
    let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().basic_icon_count(A, Icon::Authority), 1);
    assert_eq!(game.position().basic_icon_count(B, Icon::Authority), 1);
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[drawn]);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(events.len(), 1);
}

#[test]
fn maxwells_avatar_bonus_changes_auspiced_demand_vulnerability() {
    let mut f = Fixture::default();
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let source = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![build::draw(1)],
    );
    f.cards[usize::from(source.0)].featured = Some(Icon::Efficiency);
    f.cards[usize::from(source.0)].effects[0].kind = EffectKind::Demand;
    for _ in 0..2 {
        f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
    }
    let opponent = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    icons(&mut f, opponent, &[Icon::Avatar; 2]);
    // Without Auspice the opponent would share, so Auspice stays offered.
    let efficient = f.tech(Color::Yellow, 1, Zone::Board(B, Color::Yellow), vec![]);
    icons(&mut f, efficient, &[Icon::Efficiency; SLOTS]);
    let drawn = f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let position = game.position();
    assert!(position.icon_count(B, Icon::Efficiency) >= position.icon_count(A, Icon::Efficiency));
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: true,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[drawn]);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, drawn);
    assert!(!moved(&events[0]).inside_karma);
}

#[test]
fn special_requirements_consume_modified_icons_and_claim_only_at_action_end() {
    let mut f = Fixture::default();
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    for _ in 0..11 {
        f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().basic_icon_count(A, Icon::Efficiency), 1);
    assert!(game.position().eligible_for_special(A, Special::World));
    assert!(
        !game
            .position()
            .clone()
            .with_expansions(Expansions::BASE)
            .eligible_for_special(A, Special::World)
    );
    assert_eq!(
        game.position().special_location(Special::World),
        SpecialLocation::Available
    );
    game.finish_action().unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        game.position().special_location(Special::World),
        SpecialLocation::Owned(A)
    );
    assert_eq!(events.len(), 1);
}

#[test]
fn food_joins_standard_icons_only_after_age_zero_introduction() {
    let mut f = Fixture::default();
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
    f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let food = f.tech(Color::Red, 0, Zone::AgeZeroReserve, vec![]);
    icons(&mut f, food, &[Icon::Food, Icon::Avatar]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(!game.position().standard_icons().any(|i| i == Icon::Food));
    game.start(Operation::IntroduceAgeZero, Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    game.start(
        Operation::Draw {
            value: 0,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    game.start(meld(food, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    assert!(game.position().standard_icons().any(|i| i == Icon::Food));
    assert_eq!(game.position().icon_count(A, Icon::Food), 2);
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 4);
    assert_eq!(game.position().effective_achievement_count(A), 4); // Prosperity, Efficiency, Avatar, Food.
}

#[test]
fn nobel_counts_present_types_only_and_does_not_materialize_or_duplicate_age_achievements() {
    let mut f = Fixture::default();
    f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 5, Zone::Score(A), vec![]);
    let achievement = f.tech(Color::Blue, 1, Zone::AvailableAchievements, vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.effective_achievement_count(A), 2); // Prosperity and Avatar; absent types do not count.
    assert_eq!(position.achievement_count(A), 0);
    assert!(position.cards(Zone::Achievements(A)).is_empty());
    assert!(position.eligible_for_achievement(A, achievement)); // Still costs five, not ten.
}

#[test]
fn nobels_virtual_achievement_wins_at_action_end_but_can_disappear_during_fading() {
    for fade_nobel in [false, true] {
        let mut f = Fixture::default();
        let nobel = f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
        if fade_nobel {
            f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
        }
        for _ in 0..5 {
            f.tech(Color::Blue, 1, Zone::Achievements(A), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.finish_action().unwrap();
        let mut events = Vec::new();
        if fade_nobel {
            choose_card(&mut game, &mut events, nobel);
        }
        assert_eq!(
            finish(&mut game, &mut events),
            if fade_nobel {
                Status::Complete(ReturnValue::Unit)
            } else {
                Status::Winner(A)
            }
        );
        assert_eq!(game.position().achievement_count(A), 5);
        assert_eq!(
            game.position().effective_achievement_count(A),
            if fade_nobel { 5 } else { 7 }
        );
        assert!(!events.iter().any(|e| matches!(
            e,
            Event::Moved(Movement {
                verb: Some(Verb::Achieve),
                ..
            })
        )));
    }
}

#[test]
fn nobels_condition_rechecks_opponents_basic_icons_after_a_splay() {
    let mut f = Fixture::default();
    f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
    let below = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    icons(&mut f, below, &[Icon::Prosperity; 4]);
    let top = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    icons(&mut f, top, &[Icon::Prosperity]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().effective_achievement_count(A), 2);
    game.start(
        Operation::Splay {
            player: B,
            color: Color::Red,
            direction: Splay::Right,
        },
        Context::new(B),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().basic_icon_count(B, Icon::Prosperity), 3);
    assert_eq!(game.position().effective_achievement_count(A), 1); // Avatar remains.
}

#[test]
fn exhaustion_uses_negative_scores_and_effective_achievement_tiebreaks() {
    let mut f = Fixture::default();
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    let b = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    icons(&mut f, b, &[Icon::Authority; 4]);
    f.tech(Color::Yellow, 2, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().score(B), -2);
    game.start(
        Operation::Draw {
            value: 12,
            set: None,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(finish(&mut game, &mut Vec::new()), Status::Winner(A)); // Would be B without penalty.
    let mut f = Fixture::default();
    f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 12,
            set: None,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(finish(&mut game, &mut Vec::new()), Status::Winner(A)); // Scores and physical achievements tied.
}

#[test]
fn maxwell_first_draws_for_an_opponents_low_return_and_suppresses_trebek() {
    for value in [1, 7, 8, 11] {
        let mut f = Fixture::default();
        f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
        f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
        let returned = f.tech(Color::Green, value, Zone::Hand(B), vec![]);
        let drawn = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Move {
                verb: Verb::Return,
                target: target(returned, Zone::Hand(B)),
                destination: Zone::Deck(Deck::base(value)),
            },
            Context::new(B),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Performed,
                card: Some(returned),
                value: Some(value),
                short: false,
            }))
        );
        if value < 8 {
            assert_eq!(events.len(), 2);
            assert_eq!(moved(&events[0]).card, drawn);
            assert_eq!(moved(&events[0]).executor, A);
            assert!(moved(&events[0]).inside_karma);
        } else {
            assert_eq!(events.len(), 1);
        }
        assert_eq!(moved(events.last().unwrap()).card, returned);
        assert!(!moved(events.last().unwrap()).inside_karma);
    }
}

#[test]
fn maxwell_first_exhaustion_cancels_the_pending_return() {
    let mut f = Fixture::default();
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let returned = f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
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
    assert_eq!(finish(&mut game, &mut events), Status::Winner(A));
    assert_eq!(game.position().cards(Zone::Hand(B)), &[returned]);
    assert!(events.is_empty());
}

fn transfer_fixture() -> (Fixture, CardId, CardId, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::AlfredNobel, Zone::Board(B, Color::Green));
    let transferred = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let first = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let second = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
    f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
    (f, transferred, first, second)
}

fn transfer(game: &mut Game, card: CardId) {
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(card, Zone::Hand(A)),
            destination: Zone::Hand(B),
        },
        Context::new(A),
    )
    .unwrap();
}

#[test]
fn nobel_first_junks_only_base_sevens_then_returns_the_original_transfer_result() {
    let (f, card, first, second) = transfer_fixture();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    transfer(&mut game, card);
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), &[first]);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(card),
            value: Some(3),
            short: false,
        }))
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [second, first, card]
    );
    assert!(events[..2].iter().all(|e| moved(e).verb == Some(Verb::Junk)
        && moved(e).executor == B
        && moved(e).inside_karma));
    assert!(!moved(&events[2]).inside_karma);
    assert_eq!(game.position().cards(Zone::Deck(Deck::figures(7))).len(), 1);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(8))).len(), 1);
}

#[test]
fn exchange_is_not_a_transfer_for_nobel_and_the_current_player_selects_competing_karma() {
    let (mut f, card, first, second) = transfer_fixture();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    transfer(&mut game, card);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    let index = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Karma {
                    figure: Figure::AlfredNobel,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(index).unwrap();
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[card]);
    assert_eq!(game.position().cards(Zone::Junk), &[second, first]);
    let (f, card, first, second) = transfer_fixture();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExchangeAll {
            left: LogicalZone::Hand(A),
            right: LogicalZone::Hand(B),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], Event::Exchanged { .. }));
    assert_eq!(game.position().cards(Zone::Hand(B)), &[card]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(7))),
        &[first, second]
    );
}

#[test]
fn cloning_every_nobel_boundary_retains_deck_progress_and_pending_transfer() {
    let (f, card, _, _) = transfer_fixture();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    transfer(&mut game, card);
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..100 {
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
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("Nobel did not terminate");
}
