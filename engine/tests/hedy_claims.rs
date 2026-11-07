use inno_engine::*;

mod common;
use common::*;

fn stacked(f: &mut Fixture, player: Player, color: Color) {
    for value in [1, 2] {
        f.tech(color, value, Zone::Board(player, color), vec![]);
    }
}

fn hedy_fixture() -> Fixture {
    let mut f = Fixture::default();
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    stacked(&mut f, A, Color::Red);
    stacked(&mut f, A, Color::Blue);
    f
}

fn outcome(disposition: Disposition, card: Option<CardId>, value: Option<u8>) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition,
        card,
        value,
        short: false,
    }))
}

fn splay_event(color: Color, from: Splay, inside_karma: bool) -> Event {
    Event::Splayed {
        player: A,
        color,
        from,
        to: Splay::Up,
        executor: A,
        inside_karma,
    }
}

/// Whether an option splays a pile that the splay changes.
fn changes(position: &Position, option: &Alternative) -> bool {
    matches!(*option, Alternative::Splay { player, color, direction }
        if position.cards(Zone::Board(player, color)).len() >= 2
            && position.splay(player, color) != direction)
}

/// An up-splay menu of A's whose options that change a pile are `colors`, in order;
/// every other option names a nonempty pile left as it is. Returns the `colors`
/// options' indexes.
fn splay_menu(game: &mut Game, events: &mut Vec<Event>, colors: &[Color]) -> Vec<usize> {
    let menu = decision(game, events);
    assert_eq!(menu.player, A);
    let position = game.position();
    let mut changing = Vec::new();
    for (i, option) in menu.options.iter().enumerate() {
        let Alternative::Splay {
            player,
            color,
            direction: Splay::Up,
        } = *option
        else {
            panic!("not an up splay: {option:?}");
        };
        assert_eq!(player, A);
        assert!(!position.cards(Zone::Board(A, color)).is_empty());
        if changes(position, option) {
            changing.push((color, i));
        }
    }
    assert_eq!(
        changing.iter().map(|&(color, _)| color).collect::<Vec<_>>(),
        colors
    );
    changing.into_iter().map(|(_, i)| i).collect()
}

/// Settle, answering each question with its first option that changes a
/// pile, or else its first option.
fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(d) => {
                let pick = d
                    .options
                    .iter()
                    .position(|o| changes(game.position(), o))
                    .unwrap_or(0);
                game.choose(pick).unwrap()
            }
            status => return status,
        }
    }
    panic!("claim scheduling did not terminate");
}

#[test]
fn effect_card_claim_and_achieve_movement_wait_for_first_and_retain_the_original_result() {
    // Both effect APIs must route through the same achievement reaction, even
    // though only Claim has an eligibility flag.
    for use_move in [false, true] {
        let mut f = hedy_fixture();
        let card = f.tech(Color::Purple, 10, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let request = if use_move {
            Operation::Move {
                verb: Verb::Achieve,
                target: target(card, Zone::Hand(A)),
                destination: Zone::Achievements(A),
            }
        } else {
            Operation::Claim {
                target: target(card, Zone::Hand(A)),
                eligibility: ClaimEligibility::Ignore,
            }
        };
        game.start(request, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = splay_menu(&mut game, &mut events, &[Color::Red, Color::Blue]);
        assert!(events.is_empty());
        assert_eq!(game.position().cards(Zone::Hand(A)), &[card]);
        assert!(game.position().cards(Zone::Achievements(A)).is_empty());
        game.choose(menu[1]).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            outcome(Disposition::Performed, Some(card), Some(10))
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], splay_event(Color::Blue, Splay::None, true));
        assert_eq!(
            events[1],
            Event::Moved(Movement {
                verb: Some(Verb::Achieve),
                card,
                source: Zone::Hand(A),
                destination: Zone::Achievements(A),
                executor: A,
                inside_karma: false,
            })
        );
        assert_eq!(game.position().cards(Zone::Achievements(A)), &[card]);
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(game.position().regular_actions_started(), 0);
    }
}

#[test]
fn special_and_decree_tokens_remain_unclaimed_until_the_mandatory_splay() {
    for special in [true, false] {
        let mut game = Game::new(hedy_fixture().position(), Expansions::FIGURES).unwrap();
        let request = if special {
            Operation::ClaimSpecial {
                special: Special::World,
                check_eligibility: false,
            }
        } else {
            Operation::ClaimDecree {
                decree: Decree::War,
            }
        };
        game.start(request, Context::new(A)).unwrap();
        let mut events = Vec::new();
        let menu = splay_menu(&mut game, &mut events, &[Color::Red, Color::Blue]);
        assert!(events.is_empty());
        assert_eq!(game.position().achievement_count(A), 0);
        assert_eq!(
            game.position().special_location(Special::World),
            SpecialLocation::Available
        );
        assert_eq!(
            game.position().decree_location(Decree::War),
            DecreeLocation::Supply
        );
        game.choose(menu[0]).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            outcome(Disposition::Performed, None, None)
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], splay_event(Color::Red, Splay::None, true));
        assert_eq!(game.position().achievement_count(A), 1);
        assert_eq!(
            events[1],
            if special {
                Event::SpecialMoved {
                    special: Special::World,
                    source: SpecialLocation::Available,
                    destination: SpecialLocation::Owned(A),
                    executor: A,
                    inside_karma: false,
                }
            } else {
                Event::DecreeMoved {
                    decree: Decree::War,
                    source: DecreeLocation::Supply,
                    destination: DecreeLocation::Owned(A),
                    executor: A,
                    inside_karma: false,
                }
            }
        );
    }
}

#[test]
fn already_up_and_singleton_colors_are_no_op_choices_and_impossible_splays_do_not_block() {
    let mut game = Game::new(hedy_fixture().position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Red,
            direction: Splay::Up,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    let mut events = Vec::new();
    // Blue is the only color the first claim's splay changes; up red and lone green
    // stay no-op choices beside it. Both stacks are up on the second claim, so every
    // choice would change nothing and none is asked.
    for (special, changing) in [(Special::World, &[Color::Blue][..]), (Special::Empire, &[])] {
        game.start(
            Operation::ClaimSpecial {
                special,
                check_eligibility: false,
            },
            Context::new(A),
        )
        .unwrap();
        if !changing.is_empty() {
            let menu = splay_menu(&mut game, &mut events, changing);
            game.choose(menu[0]).unwrap();
        }
        assert_eq!(
            settle(&mut game, &mut events),
            outcome(Disposition::Performed, None, None)
        );
        assert_eq!(
            game.position().special_location(special),
            SpecialLocation::Owned(A)
        );
    }
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], splay_event(Color::Blue, Splay::None, true));
    assert!(
        events[1..]
            .iter()
            .all(|e| matches!(e, Event::SpecialMoved { .. }))
    );
    assert_eq!(game.position().splay(A, Color::Green), Splay::None);
}

#[test]
fn foreign_claims_and_inapplicable_requests_never_earn_a_first_splay() {
    let mut f = hedy_fixture();
    let card = f.tech(Color::Yellow, 10, Zone::AvailableAchievements, vec![]);
    stacked(&mut f, B, Color::Purple);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut events = Vec::new();
    for request in [
        Operation::Claim {
            target: target(card, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Available,
        },
        Operation::Claim {
            target: target(card, Zone::Hand(A)), // A stale physical source.
            eligibility: ClaimEligibility::Ignore,
        },
        Operation::ClaimSpecial {
            special: Special::World,
            check_eligibility: true,
        },
    ] {
        game.start(request, Context::new(A)).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            outcome(Disposition::Inapplicable, None, None)
        );
        assert!(events.is_empty());
    }
    let claim = Operation::ClaimSpecial {
        special: Special::World,
        check_eligibility: false,
    };
    game.start(claim, Context::new(B)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        outcome(Disposition::Performed, None, None)
    );
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], Event::SpecialMoved { executor: B, .. }));
    game.start(claim, Context::new(A)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        outcome(Disposition::Inapplicable, None, None)
    );
    assert_eq!(events.len(), 1);
    for (player, color) in [(A, Color::Red), (A, Color::Blue), (B, Color::Purple)] {
        assert_eq!(game.position().splay(player, color), Splay::None);
    }
    let before = game.clone();
    assert_eq!(
        game.take_action(Action::Achieve { card }, ActionKind::Regular),
        Err(Error::InvalidAction)
    );
    assert_eq!(game, before);
}

#[test]
fn achieve_action_rewards_the_opponent_only_after_first_and_the_original_claim() {
    let mut f = hedy_fixture();
    f.tech(Color::Purple, 5, Zone::Score(A), vec![]);
    f.tech(Color::Green, 4, Zone::Board(B, Color::Green), vec![]);
    let card = f.tech(Color::Yellow, 1, Zone::AvailableAchievements, vec![]);
    f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let reward = f.figure(Figure::YiSunSin, Zone::Deck(Deck::figures(4)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Achieve { card }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let menu = splay_menu(&mut game, &mut events, &[Color::Red, Color::Blue]);
    assert!(events.is_empty());
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position().cards(Zone::AvailableAchievements), &[card]);
    game.choose(menu[0]).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], splay_event(Color::Red, Splay::None, true));
    assert_eq!(moved(&events[1]).card, card);
    assert_eq!(moved(&events[1]).destination, Zone::Achievements(A));
    assert!(!moved(&events[1]).inside_karma);
    assert_eq!(moved(&events[2]).card, reward);
    assert_eq!(moved(&events[2]).destination, Zone::Hand(B));
    assert!(!moved(&events[2]).inside_karma);
}

#[test]
fn losing_hedy_to_fading_removes_lowered_eligibility_before_specials_are_checked() {
    let mut f = Fixture::default();
    let hedy = f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    for color in [Color::Red, Color::Blue, Color::Purple] {
        let card = f.tech(color, 1, Zone::Board(A, color), vec![]);
        f.cards[usize::from(card.0)].effects[0].kind = EffectKind::Demand;
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(game.position().eligible_for_special(A, Special::Monument));
    game.finish_action().unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, hedy);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, hedy);
    assert_eq!(moved(&events[0]).destination, Zone::Score(A));
    assert_eq!(
        game.position().special_location(Special::Monument),
        SpecialLocation::Available
    );
    assert!(!game.position().eligible_for_special(A, Special::Monument));
}

#[test]
fn special_scheduler_requeries_earlier_tokens_after_first_and_finishes_all_claims_before_victory() {
    let mut f = Fixture::default();
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    for color in [Color::Red, Color::Blue, Color::Yellow, Color::Purple] {
        let buried = f.tech(color, 1, Zone::Board(A, color), vec![]);
        if color == Color::Red {
            // Hidden to the right, visible up: the eleventh Efficiency.
            f.cards[usize::from(buried.0)].symbols[3] = Symbol::Icon(Icon::Efficiency);
        }
        let top = f.tech(color, 2, Zone::Board(A, color), vec![]);
        if matches!(color, Color::Red | Color::Blue) {
            f.cards[usize::from(top.0)].symbols = four_slots([Symbol::Icon(Icon::Efficiency); 4]);
        }
    }
    for _ in 0..6 {
        f.tech(Color::Purple, 1, Zone::Achievements(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for color in [Color::Red, Color::Blue, Color::Yellow, Color::Purple] {
        game.start(
            Operation::Splay {
                player: A,
                color,
                direction: Splay::Right,
            },
            Context::new(A),
        )
        .unwrap();
        settle(&mut game, &mut Vec::new());
    }
    assert!(!game.position().eligible_for_special(A, Special::World));
    assert!(game.position().eligible_for_special(A, Special::Wonder));
    game.finish_action().unwrap();
    let mut events = Vec::new();
    let menu = splay_menu(
        &mut game,
        &mut events,
        &[Color::Red, Color::Blue, Color::Yellow, Color::Purple],
    );
    assert!(events.is_empty());
    game.choose(menu[0]).unwrap();
    let menu = splay_menu(
        &mut game,
        &mut events,
        &[Color::Blue, Color::Yellow, Color::Purple],
    );
    assert_eq!(game.position().achievement_count(A), 7);
    assert_eq!(
        game.position().special_location(Special::Wonder),
        SpecialLocation::Owned(A)
    );
    assert_eq!(
        game.position().special_location(Special::World),
        SpecialLocation::Available
    );
    assert_eq!(events.len(), 2);
    game.choose(menu[0]).unwrap();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
    assert_eq!(game.position().achievement_count(A), 8);
    assert_eq!(
        events,
        vec![
            splay_event(Color::Red, Splay::Right, true),
            Event::SpecialMoved {
                special: Special::Wonder,
                source: SpecialLocation::Available,
                destination: SpecialLocation::Owned(A),
                executor: A,
                inside_karma: false,
            },
            splay_event(Color::Blue, Splay::Right, true),
            Event::SpecialMoved {
                special: Special::World,
                source: SpecialLocation::Available,
                destination: SpecialLocation::Owned(A),
                executor: A,
                inside_karma: false,
            },
        ]
    );
}

#[test]
fn decree_issuance_finishes_its_effect_then_reacts_only_to_a_new_supply_claim() {
    let mut f = hedy_fixture();
    stacked(&mut f, A, Color::Purple);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let issue = Operation::IssueDecree {
        decree: Decree::Expansion,
    };
    game.start(issue, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = splay_menu(
        &mut game,
        &mut events,
        &[Color::Red, Color::Blue, Color::Purple],
    );
    game.choose(menu[0]).unwrap(); // Expansion's own splay, not Hedy's First.
    let menu = splay_menu(&mut game, &mut events, &[Color::Blue, Color::Purple]);
    assert_eq!(events, vec![splay_event(Color::Red, Splay::None, false)]);
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Supply
    );
    game.choose(menu[0]).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events[1], splay_event(Color::Blue, Splay::None, true));
    assert!(matches!(
        events[2],
        Event::DecreeMoved {
            destination: DecreeLocation::Owned(A),
            inside_karma: false,
            ..
        }
    ));
    assert_eq!(events.len(), 3);

    // Reset the two stacks so reissuing an owned token would expose a spurious
    // extra Hedy decision rather than silently skipping an impossible splay.
    for color in [Color::Red, Color::Blue] {
        game.start(
            Operation::Splay {
                player: A,
                color,
                direction: Splay::Right,
            },
            Context::new(A),
        )
        .unwrap();
        settle(&mut game, &mut Vec::new());
    }
    events.clear();
    game.start(issue, Context::new(A)).unwrap();
    let menu = splay_menu(
        &mut game,
        &mut events,
        &[Color::Red, Color::Blue, Color::Purple],
    );
    game.choose(menu[0]).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events, vec![splay_event(Color::Red, Splay::Right, false)]);

    // A second supply token belongs to the opponent; issuing it returns it
    // without either the Decree effect or an achievement reaction.
    game.start(
        Operation::ClaimDecree {
            decree: Decree::Rivalry,
        },
        Context::new(B),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    events.clear();
    game.start(
        Operation::IssueDecree {
            decree: Decree::Rivalry,
        },
        Context::new(A),
    )
    .unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events,
        vec![Event::DecreeMoved {
            decree: Decree::Rivalry,
            source: DecreeLocation::Owned(B),
            destination: DecreeLocation::Supply,
            executor: A,
            inside_karma: false,
        }]
    );
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
    assert_eq!(game.position().splay(A, Color::Purple), Splay::None);
}

#[test]
fn kepler_action_end_keeps_hedys_each_but_suppresses_her_would() {
    let mut f = Fixture::default();
    let kepler = f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let source = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(source.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    f.cards[usize::from(source.0)].effects[0].kind = EffectKind::Demand;
    for color in [Color::Yellow, Color::Purple] {
        let card = f.tech(color, 1, Zone::Board(A, color), vec![]);
        f.cards[usize::from(card.0)].effects[0].kind = EffectKind::Demand;
    }
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
    choose_card(&mut game, &mut events, kepler);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, kepler);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(
        events[1],
        Event::SpecialMoved {
            special: Special::Monument,
            source: SpecialLocation::Available,
            destination: SpecialLocation::Owned(A),
            executor: A,
            inside_karma: true,
        }
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(
        game.position()
            .top(A, Color::Green)
            .map(|card| game.position().card(card).figure),
        Some(Some(Figure::HedyLamarr))
    );
}

#[test]
fn cloning_every_boundary_preserves_pending_first_choices_events_and_original_results() {
    let mut f = hedy_fixture();
    let card = f.tech(Color::Yellow, 10, Zone::Hand(A), vec![]);
    let position = f.position();
    for request in [
        Operation::Claim {
            target: target(card, Zone::Hand(A)),
            eligibility: ClaimEligibility::Ignore,
        },
        Operation::ClaimSpecial {
            special: Special::World,
            check_eligibility: false,
        },
        Operation::IssueDecree {
            decree: Decree::Expansion,
        },
    ] {
        let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
        game.start(request, Context::new(A)).unwrap();
        let mut expected = game.clone();
        let mut expected_events = Vec::new();
        let result = finish(&mut expected, &mut expected_events);
        let mut events = Vec::new();
        let mut completed = false;
        let mut decisions = 0;
        for _ in 0..300 {
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
                    decisions += 1;
                    let before = game.clone();
                    assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
                    assert_eq!(game, before);
                    let mut pending_fork = game.clone();
                    let mut pending_suffix = Vec::new();
                    assert_eq!(finish(&mut pending_fork, &mut pending_suffix), result);
                    assert_eq!(pending_fork, expected);
                    assert_eq!(
                        events
                            .iter()
                            .chain(&pending_suffix)
                            .cloned()
                            .collect::<Vec<_>>(),
                        expected_events
                    );
                    // As `finish` answers.
                    let pick = menu
                        .options
                        .iter()
                        .position(|o| changes(game.position(), o))
                        .unwrap_or(0);
                    game.choose(pick).unwrap();
                }
                status => {
                    assert_eq!(status, result);
                    assert_eq!(game, expected);
                    assert_eq!(events, expected_events);
                    completed = true;
                    break;
                }
            }
            assert!(game.position().check_conservation());
        }
        assert!(completed, "claim did not terminate");
        assert!(decisions > 0, "fixture never paused for a real choice");
    }
}

#[test]
fn a_valid_claim_is_rechecked_after_first_hides_a_score_bonus_and_never_earns_a_reward() {
    let mut f = Fixture::default();
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    let buried = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(buried.0)].symbols[0] = Symbol::Bonus(5);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    stacked(&mut f, A, Color::Blue);
    let card = f.tech(Color::Yellow, 1, Zone::AvailableAchievements, vec![]);
    let reward = f.figure(Figure::Homer, Zone::Deck(Deck::figures(1)));
    let position = f.position();
    for action in [false, true] {
        let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Splay {
                player: A,
                color: Color::Red,
                direction: Splay::Aslant,
            },
            Context::new(A),
        )
        .unwrap();
        settle(&mut game, &mut Vec::new());
        assert_eq!(game.position().score(A), 5);
        assert!(game.position().eligible_for_achievement(A, card));
        if action {
            game.take_action(Action::Achieve { card }, ActionKind::Regular)
                .unwrap();
        } else {
            game.start(
                Operation::Claim {
                    target: target(card, Zone::AvailableAchievements),
                    eligibility: ClaimEligibility::Available,
                },
                Context::new(A),
            )
            .unwrap();
        }
        let mut events = Vec::new();
        let menu = splay_menu(&mut game, &mut events, &[Color::Red, Color::Blue]);
        assert!(events.is_empty());
        game.choose(menu[0]).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            if action {
                Status::Complete(ReturnValue::Unit)
            } else {
                outcome(Disposition::Inapplicable, None, None)
            }
        );
        assert_eq!(events, vec![splay_event(Color::Red, Splay::Aslant, true)]);
        assert_eq!(game.position().score(A), 0);
        assert_eq!(game.position().cards(Zone::AvailableAchievements), &[card]);
        assert!(game.position().cards(Zone::Achievements(A)).is_empty());
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(1))),
            &[reward]
        );
    }
}
