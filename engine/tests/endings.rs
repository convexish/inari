use inno_engine::*;

mod common;
use common::*;

fn achievements(f: &mut Fixture, player: Player, count: usize) {
    for _ in 0..count {
        f.tech(Color::Blue, 1, Zone::Achievements(player), vec![]);
    }
}

#[test]
fn standard_claim_requires_points_top_value_and_extra_points_for_duplicates() {
    for (score, top, duplicates, qualifies) in [
        (9, 2, 0, false),
        (10, 1, 0, false),
        (10, 2, 0, true),
        (10, 2, 1, false),
        (20, 2, 1, true),
    ] {
        let mut f = Fixture::default();
        f.tech(Color::Red, top, Zone::Board(A, Color::Red), vec![]);
        for _ in 0..score / 5 {
            f.tech(Color::Green, 5, Zone::Score(A), vec![]);
        }
        if score % 5 != 0 {
            f.tech(Color::Green, score % 5, Zone::Score(A), vec![]);
        }
        if duplicates != 0 {
            f.tech(Color::Purple, 2, Zone::Achievements(A), vec![]);
        }
        let card = f.tech(Color::Yellow, 2, Zone::AvailableAchievements, vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        assert_eq!(
            game.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_achievement(A, card),
            qualifies
        );
        game.start(
            Operation::Claim {
                target: target(card, Zone::AvailableAchievements),
                eligibility: ClaimEligibility::Available,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let Status::Complete(ReturnValue::Operation(result)) = settle(&mut game, &mut events)
        else {
            panic!("claim must not check victory");
        };
        assert_eq!(
            result.disposition,
            if qualifies {
                Disposition::Performed
            } else {
                Disposition::Inapplicable
            }
        );
        assert_eq!(
            game.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .score(A),
            i64::from(score)
        ); // points are not spent
        assert_eq!(
            game.position().achievement_count(A),
            duplicates + usize::from(qualifies)
        );
        assert_eq!(events.len(), usize::from(qualifies));
        if qualifies {
            assert_eq!(moved(&events[0]).verb, Some(Verb::Achieve));
            assert!(
                game.position()
                    .cards(Zone::AvailableAchievements)
                    .is_empty()
            );
            assert!(matches!(
                game.position()
                    .clone()
                    .with_expansions(Expansions::BASE)
                    .descriptor(target(card, Zone::Achievements(A)), A),
                TargetDescriptor::Hidden {
                    back: Deck { age: 2, .. },
                    ..
                }
            ));
        }
        assert!(game.position().check_conservation());
    }
}

#[test]
fn an_effect_claim_bypasses_eligibility_but_still_requires_the_selected_card() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Yellow, 11, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let request = Operation::Claim {
        target: target(card, Zone::Hand(A)),
        eligibility: ClaimEligibility::Ignore,
    };
    game.start(request, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[card]);
    game.start(request, Context::new(A)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert_eq!(events.len(), 1);
    assert!(game.position().check_conservation());
}

#[test]
fn achievement_victory_is_explicit_and_uses_threshold_counts_and_noncurrent_tiebreak() {
    for (figures, a, b, turn, winner) in [
        (false, 6, 0, A, Some(A)),
        (true, 6, 0, A, None),
        (true, 7, 0, A, Some(A)),
        (true, 7, 8, A, Some(B)),
        (true, 7, 7, A, Some(B)),
        (true, 7, 7, B, Some(A)),
    ] {
        let mut f = Fixture::default();
        achievements(&mut f, A, a);
        achievements(&mut f, B, b);
        let mut position = f.position();
        position.begin_turn(turn);
        let mut events = Vec::new();
        let status = if figures {
            let mut game = Game::new(position, Expansions::FIGURES).unwrap();
            game.start(Operation::CheckAchievementVictory, Context::new(turn))
                .unwrap();
            let status = settle(&mut game, &mut events);
            assert_eq!(
                game.victory().map(|victory| victory.reason),
                winner.map(|_| WinReason::Achievements)
            );
            status
        } else {
            let mut game = Game::new(position, Expansions::BASE).unwrap();
            game.start(Operation::CheckAchievementVictory, Context::new(turn))
                .unwrap();
            let status = settle(&mut game, &mut events);
            assert_eq!(
                game.victory().map(|victory| victory.reason),
                winner.map(|_| WinReason::Achievements)
            );
            status
        };
        assert_eq!(
            status,
            winner.map_or(
                Status::Complete(ReturnValue::Operation(OperationOutcome {
                    disposition: Disposition::Inapplicable,
                    card: None,
                    value: None,
                    short: false,
                })),
                Status::Winner
            )
        );
        assert!(events.is_empty());
    }
}

#[test]
fn exhaustion_ranks_score_before_achievements_then_draws_on_a_full_tie() {
    for (score_a, score_b, achieved_a, achieved_b, winner) in [
        (3, 2, 0, 7, Some(A)),
        (2, 3, 7, 0, Some(B)),
        (2, 2, 1, 0, Some(A)),
        (2, 2, 0, 1, Some(B)),
        (2, 2, 1, 1, None),
    ] {
        let mut f = Fixture::default();
        f.tech(Color::Red, score_a, Zone::Score(A), vec![]);
        f.tech(Color::Blue, score_b, Zone::Score(B), vec![]);
        achievements(&mut f, A, achieved_a);
        achievements(&mut f, B, achieved_b);
        let position = f.position();
        for value in [1, 12] {
            let mut game = Game::new(position.clone(), Expansions::BASE).unwrap();
            game.start(Operation::Draw { value, set: None }, Context::new(A))
                .unwrap();
            let mut events = Vec::new();
            let expected = winner.map_or(Status::Draw(DrawReason::Exhaustion), Status::Winner);
            assert_eq!(settle(&mut game, &mut events), expected);
            assert_eq!(
                game.victory().map(|victory| victory.reason),
                winner.map(|_| WinReason::Exhaustion)
            );
            let terminal = game.clone();
            assert_eq!(settle(&mut game, &mut events), expected);
            assert_eq!(game, terminal);
            assert_eq!(
                game.start(Operation::CheckAchievementVictory, Context::new(A)),
                Err(Error::Busy)
            );
            assert_eq!(game.position(), &position);
            assert!(events.is_empty());
        }
    }
}

#[test]
fn taking_the_last_eleven_does_not_end_the_game_but_the_next_draw_does() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let request = Operation::Draw {
        value: 11,
        set: None,
    };
    let mut events = Vec::new();
    game.start(request, Context::new(A)).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
    game.start(request, Context::new(A)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(events.len(), 1);
}

#[test]
fn jackie_can_interrupt_exhaustion_or_achievement_wins_but_not_draws() {
    for achievement_win in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
        let scored = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
        f.tech(Color::Blue, 2, Zone::Score(B), vec![]);
        if achievement_win {
            achievements(&mut f, B, 7);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let request = if achievement_win {
            Operation::CheckAchievementVictory
        } else {
            Operation::Draw {
                value: 12,
                set: None,
            }
        };
        game.start(request, Context::new(A)).unwrap();
        let mut expected = game.clone();
        let mut expected_events = Vec::new();
        assert_eq!(
            settle(&mut expected, &mut expected_events),
            Status::Winner(A)
        );
        let mut events = Vec::new();
        for _ in 0..100 {
            let mut fork = game.clone();
            let mut suffix = Vec::new();
            assert_eq!(settle(&mut fork, &mut suffix), Status::Winner(A));
            assert_eq!(fork, expected);
            assert_eq!(
                events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
                expected_events
            );
            if game.advance(1, &mut events).unwrap() == Status::Winner(A) {
                break;
            }
        }
        assert_eq!(game, expected);
        assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
        assert_eq!(events.len(), 1);
        assert!(moved(&events[0]).inside_karma);
    }
    for reason in [DrawReason::Effect, DrawReason::Exhaustion] {
        let mut f = Fixture::default();
        f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(Operation::ProposeDraw { reason }, Context::new(B))
            .unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), Status::Draw(reason));
        assert!(events.is_empty());
    }
}

#[test]
fn exhaustion_inside_yis_reaction_keeps_win_reactions_suppressed() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red)); // 4-point bonus
    f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
    let card = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(card, Zone::Hand(B)),
            destination: Zone::Hand(A),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Tuck));
    assert!(moved(&events[0]).inside_karma);
    assert!(game.position().cards(Zone::Score(B)).is_empty());
}
