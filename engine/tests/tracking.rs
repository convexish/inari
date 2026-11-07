use inno_engine::*;
mod common;
use common::*;

fn tracked(f: Fixture, figures: bool) -> Game {
    Game::new(f.position(), Expansions::with_figures(figures))
        .unwrap()
        .with_tracking()
        .unwrap()
}

fn beliefs(game: &Game, viewer: Player, zone: Zone) -> Vec<BeliefSlot> {
    game.tracker(viewer).unwrap().belief(zone)
}

fn truth(game: &Game) {
    assert!(game.public_tracker().unwrap().check_truth(game.position()));
    for viewer in Player::ALL {
        assert!(
            game.tracker(viewer).unwrap().check_truth(game.position()),
            "{viewer:?}"
        );
    }
}

fn run(game: &mut Game, operation: Operation, executor: Player) {
    game.start(operation, Context::new(executor)).unwrap();
    assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
    truth(game);
}

fn move_card(game: &mut Game, card: CardId, source: Zone, destination: Zone, executor: Player) {
    run(
        game,
        Operation::Move {
            verb: Verb::Transfer,
            target: target(card, source),
            destination,
        },
        executor,
    );
}

#[test]
fn private_draw_public_reveal_known_return_and_shuffle() {
    let mut f = Fixture::default();
    let deck = Zone::Deck(Deck::base(1));
    let ids: Vec<_> = Color::ALL
        .into_iter()
        .map(|c| f.tech(c, 1, deck, vec![]))
        .collect();
    let mut game = tracked(f, false);
    run(
        &mut game,
        Operation::Draw {
            value: 1,
            set: None,
        },
        A,
    );
    assert_eq!(beliefs(&game, A, Zone::Hand(A))[0].known(), Some(ids[4]));
    assert_eq!(beliefs(&game, B, Zone::Hand(A))[0].candidates, ids);
    run(
        &mut game,
        Operation::Reveal {
            target: target(ids[4], Zone::Hand(A)),
        },
        A,
    );
    assert_eq!(beliefs(&game, B, Zone::Hand(A))[0].known(), Some(ids[4]));
    move_card(&mut game, ids[4], Zone::Hand(A), deck, A);
    assert_eq!(beliefs(&game, B, deck)[0].known(), Some(ids[4]));
    run(
        &mut game,
        Operation::Shuffle {
            deck: Deck::base(1),
        },
        A,
    );
    assert!(beliefs(&game, B, deck).iter().all(|s| s.candidates == ids));
    let clone = game.clone();
    assert_eq!(
        game.determinize(B, 7).unwrap(),
        game.determinize(B, 7).unwrap()
    );
    assert_eq!(game, clone, "sampling must not consume real RNG or history");
}

#[test]
fn ambiguous_departure_forgets_which_known_same_back_member_left() {
    let mut f = Fixture::default();
    let ids: Vec<_> = Color::ALL[..3]
        .iter()
        .map(|&c| f.tech(c, 1, Zone::Hand(A), vec![]))
        .collect();
    let mut game = tracked(f, false);
    run(
        &mut game,
        Operation::Reveal {
            target: target(ids[0], Zone::Hand(A)),
        },
        A,
    );
    assert!(
        beliefs(&game, B, Zone::Hand(A))
            .iter()
            .any(|s| s.known() == Some(ids[0]))
    );
    move_card(
        &mut game,
        ids[0],
        Zone::Hand(A),
        Zone::Deck(Deck::base(1)),
        A,
    );
    assert_eq!(
        beliefs(&game, B, Zone::Deck(Deck::base(1)))[0].candidates,
        ids
    );
    assert!(
        beliefs(&game, B, Zone::Hand(A))
            .iter()
            .all(|s| s.candidates == ids)
    );
}

#[test]
fn revelation_then_private_return_does_not_reveal_the_return() {
    // Knowing every card in an opponent's hand is not permission to watch
    // which same-back physical card they return.
    let mut f = Fixture::default();
    let ids: Vec<_> = Color::ALL[..3]
        .iter()
        .map(|&c| f.tech(c, 1, Zone::Hand(A), vec![]))
        .collect();
    let mut game = tracked(f, false);
    for &id in &ids {
        run(
            &mut game,
            Operation::Reveal {
                target: target(id, Zone::Hand(A)),
            },
            A,
        );
    }
    move_card(
        &mut game,
        ids[1],
        Zone::Hand(A),
        Zone::Deck(Deck::base(1)),
        A,
    );
    assert_eq!(
        beliefs(&game, B, Zone::Deck(Deck::base(1)))[0].candidates,
        ids
    );
}

#[test]
fn meiji_inspection_is_timed_and_remembered_but_aliases_do_not_grant_it() {
    let mut f = Fixture::default();
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let one = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let two = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let three = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = tracked(f, true);
    assert!(game.position().meiji_active(A));
    assert_eq!(beliefs(&game, A, Zone::Hand(B))[0].known(), Some(one));
    game.begin_action(ActionKind::Regular).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    assert!(!game.position().meiji_active(A));
    assert_eq!(beliefs(&game, A, Zone::Hand(B))[0].known(), Some(one));
    assert_eq!(
        game.public_tracker().unwrap().belief(Zone::Hand(B))[0].known(),
        Some(one)
    );
    run(
        &mut game,
        Operation::Draw {
            value: 1,
            set: None,
        },
        B,
    );
    assert!(
        beliefs(&game, A, Zone::Hand(B))
            .iter()
            .any(|s| s.candidates == [two, three])
    );
    // Removing Meiji does not erase the earlier learned hand member.
    move_card(
        &mut game,
        meiji,
        Zone::Board(A, Color::Purple),
        Zone::Score(A),
        A,
    );
    assert!(
        beliefs(&game, A, Zone::Hand(B))
            .iter()
            .any(|s| s.known() == Some(one))
    );
    assert!(matches!(
        game.position().descriptor(target(three, Zone::Hand(B)), A),
        TargetDescriptor::Hidden { .. }
    ));
    let world = game.determinize(B, 8).unwrap();
    assert!(
        beliefs(&world, A, Zone::Hand(B))
            .iter()
            .any(|slot| slot.known() == Some(one))
    );
    truth(&world);
}

#[test]
fn age_zero_is_outside_hidden_sampling_until_introduced_and_has_its_own_back() {
    let mut f = Fixture::default();
    let reserve: Vec<_> = Color::ALL[..3]
        .iter()
        .map(|&c| f.tech(c, 0, Zone::AgeZeroReserve, vec![]))
        .collect();
    f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = tracked(f, true);
    let world = game.determinize(A, 17).unwrap();
    assert_eq!(world.position().cards(Zone::AgeZeroReserve), reserve);
    run(&mut game, Operation::IntroduceAgeZero, A);
    assert!(beliefs(&game, A, Zone::AgeZeroReserve).is_empty());
    assert!(
        beliefs(&game, A, Zone::Deck(Deck::base(0)))
            .iter()
            .all(|s| s.candidates == reserve)
    );
    let world = game.determinize(A, 18).unwrap();
    truth(&world);
    assert!(world.position().age_zero_introduced());
    assert_eq!(world.position().cards(Zone::Deck(Deck::base(0))).len(), 3);
    run(&mut game, Operation::IntroduceAgeZero, A);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(0))).len(), 3);
}

#[test]
fn base_and_figure_backs_and_available_claimed_junk_zones_are_separate() {
    let mut f = Fixture::default();
    let base = f.tech(Color::Red, 1, Zone::AvailableAchievements, vec![]);
    let other = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let figure = f.figure(Figure::Homer, Zone::Deck(Deck::figures(1)));
    let mut game = tracked(f, true);
    run(
        &mut game,
        Operation::Claim {
            target: target(base, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Ignore,
        },
        A,
    );
    assert_eq!(
        beliefs(&game, A, Zone::Achievements(A))[0].candidates,
        [base, other]
    );
    move_card(&mut game, base, Zone::Achievements(A), Zone::Junk, B);
    assert_eq!(beliefs(&game, A, Zone::Junk)[0].candidates, [base, other]);
    for seed in 0..8 {
        let world = game.determinize(A, seed).unwrap();
        truth(&world);
        assert_eq!(
            world.position().cards(Zone::Deck(Deck::figures(1))),
            [figure]
        );
    }
}

#[test]
fn simultaneous_exchange_and_alias_overlap_never_duplicate_physical_beliefs() {
    let mut f = Fixture::default();
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let hand = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let score = f.tech(Color::Green, 1, Zone::Score(A), vec![]);
    f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = tracked(f, true);
    run(
        &mut game,
        Operation::ExchangeAll {
            left: LogicalZone::Hand(A),
            right: LogicalZone::Score(A),
        },
        A,
    );
    assert!(game.position().cards(Zone::Hand(A)).contains(&hand));
    assert!(game.position().cards(Zone::Hand(A)).contains(&score));
    assert!(beliefs(&game, A, Zone::Score(A)).is_empty());
    assert_eq!(beliefs(&game, A, Zone::Hand(A)).len(), 2);
    truth(&game.determinize(B, 1).unwrap());
}

fn hidden_world(swapped: bool) -> Game {
    let mut f = Fixture::default();
    let one = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let two = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    if swapped {
        f.placements[usize::from(one.0)].1 = Zone::Deck(Deck::base(1));
        f.placements[usize::from(two.0)].1 = Zone::Hand(B);
    }
    tracked(f, false)
}

#[test]
fn indistinguishable_worlds_give_identical_samples_and_do_not_import_opponent_knowledge() {
    let left = hidden_world(false);
    let right = hidden_world(true);
    assert_eq!(left.tracker(A), right.tracker(A));
    assert_ne!(left.tracker(B), right.tracker(B));
    let mut hands = Vec::new();
    for seed in 0..24 {
        let l = left.determinize(A, seed).unwrap();
        let r = right.determinize(A, seed).unwrap();
        assert_eq!(l.position(), r.position());
        assert_eq!(l.tracker(A), r.tracker(A));
        assert_eq!(l.tracker(B), r.tracker(B));
        truth(&l);
        truth(&r);
        hands.push(l.position().cards(Zone::Hand(B))[0]);
    }
    hands.sort();
    hands.dedup();
    assert_eq!(
        hands.len(),
        3,
        "opponent's real known hand must not pin sampling"
    );
}

#[test]
fn opening_commitments_remap_without_revealing_opponents_selected_card() {
    for figures in [false, true] {
        let mut game = if figures {
            Game::new_figures(37)
        } else {
            Game::new_base(37)
        }
        .unwrap()
        .with_tracking()
        .unwrap();
        let first = decision(&mut game, &mut Vec::new());
        assert_eq!(first.player, A);
        let mut other = game.clone();
        game.choose(0).unwrap();
        other.choose(1).unwrap();
        let second = decision(&mut game, &mut Vec::new());
        assert_eq!(second.player, B);
        assert_eq!(decision(&mut other, &mut Vec::new()), second);
        assert_eq!(game.observe(B).unwrap(), other.observe(B).unwrap());
        let root = game.clone();
        let mut melds = std::collections::BTreeSet::new();
        for seed in 0..16 {
            let mut world = game.determinize(B, seed).unwrap();
            assert!(
                world == other.determinize(B, seed).unwrap(),
                "the secret commitment's real hand slot must not reach the sample"
            );
            assert_eq!(decision(&mut world, &mut Vec::new()), second);
            world.choose(0).unwrap();
            assert!(matches!(
                settle(&mut world, &mut Vec::new()),
                Status::Complete(_)
            ));
            truth(&world);
            melds.extend(
                Color::ALL
                    .into_iter()
                    .filter_map(|c| world.position().top(A, c)),
            );
        }
        // Openings are simultaneous: B's samples must not keep A's real pick.
        assert!(melds.len() > 1, "A's opening pick is pinned");
        assert_eq!(game, root);
        // Real play must still reveal the actual commitment.
        for (game, option) in [(&mut game, 0), (&mut other, 1)] {
            let card = game.position().cards(Zone::Hand(A))[option];
            game.choose(0).unwrap();
            assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
            assert_eq!(
                game.position().top(A, game.position().card(card).color),
                Some(card)
            );
            truth(game);
        }
    }
}

#[test]
fn opening_search_does_not_depend_on_the_opponents_secret_commitment() {
    for figures in [false, true] {
        let mut left = if figures {
            Game::new_figures(37)
        } else {
            Game::new_base(37)
        }
        .unwrap()
        .with_tracking()
        .unwrap();
        assert_eq!(decision(&mut left, &mut Vec::new()).player, A);
        let mut right = left.clone();
        left.choose(0).unwrap();
        right.choose(1).unwrap();
        assert_eq!(decision(&mut left, &mut Vec::new()).player, B);
        assert_eq!(decision(&mut right, &mut Vec::new()).player, B);
        let config = SearchConfig {
            iterations: 32,
            rollout_actions: 2,
            seed: 1,
            threads: 1,
            ..SearchConfig::default()
        };
        assert_eq!(
            left.search(config).unwrap(),
            right.search(config).unwrap(),
            "same observations and seed must produce the same action and statistics"
        );
    }
}

#[test]
fn hidden_pending_meld_is_resampled_and_follows_its_sampled_identity_after_first() {
    let make = |swapped| {
        let mut f = Fixture::default();
        f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
        f.tech(
            Color::Red,
            9,
            Zone::Board(B, Color::Red),
            vec![build::verb(Verb::Return, Selector::Hand(Who::Opponent)).to(Destination::Deck)],
        );
        let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let green = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
        let incoming = if swapped {
            f.placements[usize::from(blue.0)].1 = Zone::Deck(Deck::base(2));
            f.placements[usize::from(green.0)].1 = Zone::Hand(B);
            green
        } else {
            blue
        };
        let color = f.cards[usize::from(incoming.0)].color;
        let mut game = tracked(f, true);
        game.start(meld(incoming, Zone::Hand(B), B, color), Context::new(B))
            .unwrap();
        decision(&mut game, &mut Vec::new());
        game
    };
    let left = make(false);
    let right = make(true);
    assert_eq!(left.tracker(A), right.tracker(A));
    assert_eq!(left.pending_melds(A), right.pending_melds(A));
    let mut melded = Vec::new();
    for seed in 0..16 {
        let mut l = left.determinize(A, seed).unwrap();
        let mut r = right.determinize(A, seed).unwrap();
        assert_eq!(l.position(), r.position());
        assert_eq!(
            decision(&mut l, &mut Vec::new()),
            decision(&mut r, &mut Vec::new())
        );
        let selected = l.position().cards(Zone::Hand(B))[0];
        for game in [&mut l, &mut r] {
            // Kalashnikov self-executes B's red card, returning the selected
            // meld beneath the other card, then resumes that exact identity.
            game.choose(1).unwrap();
            let mut events = Vec::new();
            assert!(matches!(settle(game, &mut events), Status::Complete(_)));
            assert_eq!(moved(&events[0]).card, selected);
            assert_eq!(moved(&events[1]).card, selected);
            assert_eq!(moved(&events[1]).source, Zone::Deck(Deck::base(2)));
            let color = game.position().card(selected).color;
            assert_eq!(game.position().top(B, color), Some(selected));
            truth(game);
        }
        assert_eq!(l.position(), r.position());
        melded.push(selected);
    }
    melded.sort();
    melded.dedup();
    assert_eq!(melded.len(), 2);
}

#[test]
fn remembered_hand_members_do_not_pin_an_unknown_pending_selection() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.tech(Color::Red, 9, Zone::Board(B, Color::Red), vec![]);
    let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let green = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    let mut game = tracked(f, true);
    for card in [blue, green] {
        run(
            &mut game,
            Operation::Reveal {
                target: target(card, Zone::Hand(B)),
            },
            B,
        );
    }
    assert!(
        beliefs(&game, A, Zone::Hand(B))
            .iter()
            .all(|s| s.known().is_some())
    );
    game.start(meld(blue, Zone::Hand(B), B, Color::Blue), Context::new(B))
        .unwrap();
    decision(&mut game, &mut Vec::new());
    assert!(matches!(game.pending_melds(A)[0].card, CardView::Back(_)));
    let mut selected = Vec::new();
    for seed in 0..24 {
        let mut world = game.determinize(A, seed).unwrap();
        assert_eq!(
            beliefs(&game, A, Zone::Hand(B)),
            beliefs(&world, A, Zone::Hand(B))
        );
        world.choose(1).unwrap();
        let mut events = Vec::new();
        settle(&mut world, &mut events);
        selected.push(moved(events.last().unwrap()).card);
        truth(&world);
    }
    selected.sort();
    selected.dedup();
    assert_eq!(selected, [blue, green]);
}

#[test]
fn trebek_names_are_public_evidence_but_do_not_disclose_deck_order() {
    for hit in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
        let named = f.figure(
            Figure::Nightingale,
            if hit {
                Zone::Deck(Deck::figures(7))
            } else {
                Zone::Hand(B)
            },
        );
        f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
        f.figure(Figure::JohnEricsson, Zone::Deck(Deck::figures(7)));
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = tracked(f, true);
        game.start(
            Operation::Draw {
                value: 1,
                set: None,
            },
            Context::new(A),
        )
        .unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        game.choose(
            menu.options
                .iter()
                .position(|o| *o == Alternative::Deck(Deck::figures(7)))
                .unwrap(),
        )
        .unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        let name = FIGURE_NAMES.iter().find(|n| n.code == "F069").unwrap();
        game.choose(
            menu.options
                .iter()
                .position(|o| *o == Alternative::NameFigure(name))
                .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        truth(&game);
        assert!(
            beliefs(&game, A, Zone::Deck(Deck::figures(7)))
                .iter()
                .all(|s| !s.candidates.contains(&named))
        );
        assert!(
            game.public_tracker()
                .unwrap()
                .belief(Zone::Deck(Deck::figures(7)))
                .iter()
                .all(|s| !s.candidates.contains(&named))
        );
        if hit {
            assert!(
                beliefs(&game, B, Zone::Hand(A))
                    .iter()
                    .any(|s| s.known() == Some(named))
            );
        }
        for seed in 0..4 {
            truth(&game.determinize(B, seed).unwrap());
        }
    }
}

#[test]
fn unentered_front_dependent_reactions_do_not_disclose_the_operand() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 9, Zone::Board(B, Color::Red), vec![]);
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = tracked(f, true);
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    let emma = menu
        .options
        .iter()
        .position(|option| {
            matches!(
                option,
                Alternative::Karma {
                    figure: Figure::EmmaWatson,
                    ..
                }
            )
        })
        .unwrap();
    let mut melds = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let mut world = game.determinize(A, seed).unwrap();
        melds.insert(world.position().cards(Zone::Hand(B))[0]);
        world.choose(emma).unwrap();
        assert!(matches!(
            settle(&mut world, &mut Vec::new()),
            Status::Complete(_)
        ));
        truth(&world);
    }
    assert_eq!(
        melds.len(),
        2,
        "offering Emma has not read the incoming color"
    );
    truth(&game);
}

#[test]
fn an_unoffered_conditional_hook_reveals_its_operand_first() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    f.figure(Figure::BartolomeoCristofori, Zone::Board(B, Color::Purple));
    for _ in 0..4 {
        f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    }
    let incoming = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = tracked(f, true);
    game.start(
        Operation::Splay {
            player: B,
            color: Color::Blue,
            direction: Splay::Left,
        },
        Context::new(B),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Green),
        Context::new(B),
    )
    .unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    assert!(!menu.options.iter().any(|option| matches!(
        option,
        Alternative::Karma {
            figure: Figure::BartolomeoCristofori,
            ..
        }
    )));
    // Cristofori's condition reads the incoming color, so the card is
    // revealed before the hooks are offered: every sampled world keeps it,
    // and the two-option menu leaks nothing.
    for seed in 0..8 {
        let world = game.determinize(A, seed).unwrap();
        assert!(world.position().cards(Zone::Hand(B)).contains(&incoming));
        truth(&world);
    }
    truth(&game);
}

#[test]
fn elimination_moves_all_cards_through_tracking_without_stale_slots() {
    let mut f = Fixture::default();
    let top = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Yellow, 1, Zone::Score(A), vec![]);
    f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
    f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = tracked(f, false);
    game.start(Operation::Lose { player: A }, Context::new(A))
        .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(B));
    truth(&game);
    assert_eq!(beliefs(&game, B, Zone::Junk).len(), 4);
    assert!(
        beliefs(&game, B, Zone::Junk)
            .iter()
            .any(|s| s.known() == Some(top))
    );
    for zone in [Zone::Hand(A), Zone::Score(A), Zone::Achievements(A)] {
        assert!(beliefs(&game, A, zone).is_empty());
        assert!(beliefs(&game, B, zone).is_empty());
    }
}

#[test]
fn requires_tracking_and_rejects_internal_scheduler_states() {
    let mut game = Game::new_base(0).unwrap();
    decision(&mut game, &mut Vec::new());
    assert!(game.determinize(A, 0).is_err());
    assert!(game.with_tracking().is_err());
    let mut game = Game::new_base(0).unwrap().with_tracking().unwrap();
    assert_eq!(game.determinize(A, 0), Err(Error::Busy));
    decision(&mut game, &mut Vec::new());
    game.choose(0).unwrap();
    assert_eq!(game.determinize(A, 0), Err(Error::Busy));
}

#[test]
fn bounded_seeded_games_preserve_untracked_replays_and_sampled_truth_after_choices() {
    let mut sampled = 0;
    let mut guarded = 0;
    for figures in [false, true] {
        for seed in 0..8 {
            let make = if figures {
                Game::new_figures
            } else {
                Game::new_base
            };
            let mut reference = make(seed).unwrap();
            let mut game = reference.clone().with_tracking().unwrap();
            let mut rng = Rng::new(seed + 1024);
            let mut ended = false;
            for step in 0..1000 {
                let mut events = Vec::new();
                let status = game.advance(10000, &mut events).unwrap();
                let mut reference_events = Vec::new();
                assert_eq!(
                    status,
                    reference.advance(10000, &mut reference_events).unwrap(),
                    "figures={figures} seed={seed} step={step}"
                );
                assert_eq!(events, reference_events);
                assert_eq!(game.position(), reference.position());
                truth(&game);
                match status {
                    Status::Decision(menu) => {
                        if step % 7 == 0 {
                            match game.determinize(menu.player, rng.next_u64()) {
                                Ok(mut world) => {
                                    sampled += 1;
                                    assert_eq!(decision(&mut world, &mut Vec::new()), menu);
                                    world.choose(0).unwrap();
                                    world.advance(10000, &mut Vec::new()).unwrap();
                                    truth(&world);
                                }
                                Err(Error::Unsupported(
                                    "a karma already read a hidden operand's front"
                                    | "a running program's hidden source needs a pinned identity",
                                )) => guarded += 1,
                                Err(error) => panic!("unexpected sample failure: {error:?}"),
                            }
                        }
                        let choice = (rng.next_u64() % menu.options.len() as u64) as usize;
                        game.choose(choice).unwrap();
                        reference.choose(choice).unwrap();
                    }
                    Status::Complete(_) => {
                        if step % 7 == 0 {
                            truth(
                                &game
                                    .determinize(game.position().turn_owner(), rng.next_u64())
                                    .unwrap(),
                            );
                        }
                        let actions = game.legal_actions();
                        assert_eq!(actions, reference.legal_actions());
                        let action = actions[(rng.next_u64() % actions.len() as u64) as usize];
                        game.take_action(action, ActionKind::Regular).unwrap();
                        reference.take_action(action, ActionKind::Regular).unwrap();
                    }
                    Status::Winner(_) | Status::Draw(_) => {
                        ended = true;
                        break;
                    }
                    Status::WorkLimit => panic!("unexpected work cap"),
                }
            }
            assert!(ended, "bounded replay did not end");
        }
    }
    assert!(sampled > 20);
    // Front-reading karma predicates remain guarded (see the scenarios
    // above); none occur in these games, so every root must sample.
    assert_eq!(guarded, 0, "every real decision root samples");
    println!("16 replay-identical games: {sampled} sampled decision continuations");
}
