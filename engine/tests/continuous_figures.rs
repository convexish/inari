use inno_engine::*;

mod common;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    match settle(game, events) {
        Status::Decision(menu) => panic!("unexpected choice: {menu:?}"),
        status => status,
    }
}

fn splay(game: &mut Game, player: Player, color: Color, direction: Splay) -> Vec<Event> {
    game.start(
        Operation::Splay {
            player,
            color,
            direction,
        },
        Context::new(player),
    )
    .unwrap();
    let mut events = Vec::new();
    finish(game, &mut events);
    events
}

#[test]
fn machiavelli_unsplays_only_the_matching_opponent_color_before_original_splay() {
    let mut f = Fixture::default();
    f.figure(Figure::NiccoloMachiavelli, Zone::Board(A, Color::Purple));
    for player in [A, B] {
        base(&mut f, "B001", Zone::Board(player, Color::Red));
        base(&mut f, "B002", Zone::Board(player, Color::Red));
        base(&mut f, "B010", Zone::Board(player, Color::Blue));
        base(&mut f, "B012", Zone::Board(player, Color::Blue));
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    splay(&mut game, B, Color::Red, Splay::Right);
    splay(&mut game, B, Color::Blue, Splay::Up);
    let events = splay(&mut game, A, Color::Red, Splay::Right);
    assert_eq!(
        events,
        [
            Event::Splayed {
                player: B,
                color: Color::Red,
                from: Splay::Right,
                to: Splay::None,
                executor: A,
                inside_karma: true
            },
            Event::Splayed {
                player: A,
                color: Color::Red,
                from: Splay::None,
                to: Splay::Right,
                executor: A,
                inside_karma: false
            },
        ]
    );
    assert_eq!(game.position().splay(B, Color::Blue), Splay::Up);
    assert_eq!(game.position().effective_achievement_count(A), 1);
    assert_eq!(game.position().achievement_count(A), 0);
    splay(&mut game, B, Color::Red, Splay::Right);
    assert_eq!(game.position().effective_achievement_count(A), 0);
    splay(&mut game, B, Color::Red, Splay::Left);
    assert_eq!(game.position().effective_achievement_count(A), 1);
    // An unsplay is not a splay trigger, so the opponent's left splay remains.
    let events = splay(&mut game, A, Color::Red, Splay::None);
    assert_eq!(events.len(), 1);
    assert_eq!(game.position().splay(B, Color::Red), Splay::Left);
}

#[test]
fn machiavelli_virtual_achievements_compose_with_nobel_without_claim_cost_or_polhem_points() {
    let mut f = Fixture::default();
    let machiavelli = f.figure(Figure::NiccoloMachiavelli, Zone::Board(A, Color::Purple));
    f.figure(Figure::AlfredNobel, Zone::Board(A, Color::Green));
    f.figure(Figure::ChristopherPolhem, Zone::Board(A, Color::Yellow));
    base(&mut f, "B001", Zone::Board(A, Color::Red));
    base(&mut f, "B002", Zone::Board(A, Color::Red));
    let claim = base(&mut f, "B001", Zone::AvailableAchievements);
    base(&mut f, "B062", Zone::Score(A));
    let cover = base(&mut f, "B014", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let score = game.position().score(A);
    let eligible = game.position().eligible_for_achievement(A, claim);
    assert!(eligible);
    let count = game.position().effective_achievement_count(A);
    splay(&mut game, A, Color::Red, Splay::Right);
    // Both exposed types already exist, so Nobel's contribution is unchanged.
    assert_eq!(game.position().effective_achievement_count(A), count + 1);
    assert_eq!(game.position().score(A), score);
    assert_eq!(game.position().eligible_for_achievement(A, claim), eligible);
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    game.start(
        meld(cover, Zone::Hand(A), A, Color::Purple),
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert!(!game.position().has_figure(A, Figure::NiccoloMachiavelli));
    let noble = game
        .position()
        .standard_icons()
        .filter(|&icon| {
            let own = game.position().basic_icon_count(A, icon);
            own > 0 && own >= 2 * game.position().basic_icon_count(B, icon)
        })
        .count();
    assert_eq!(game.position().effective_achievement_count(A), noble);
    assert!(
        game.position()
            .cards(Zone::Board(A, Color::Purple))
            .contains(&machiavelli)
    );
}

#[test]
fn catherine_each_uses_basic_concepts_independently_of_maxwell_and_disappears_on_removal() {
    let mut f = Fixture::default();
    let catherine = f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    let maxwell = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
    let color = maxwell.color;
    f.add(maxwell, Zone::Board(A, color));
    base(&mut f, "B001", Zone::Hand(A));
    base(&mut f, "B002", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for icon in [Icon::Concept, Icon::Efficiency] {
        let basic = game.position().basic_icon_count(A, icon);
        let expected = basic
            + game.position().basic_icon_count(A, Icon::Concept)
            + if basic > 0 { 2 } else { 0 };
        assert_eq!(game.position().icon_count(A, icon), expected);
        assert_eq!(
            game.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .icon_count(A, icon),
            basic
        );
    }
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(catherine, Zone::Board(A, Color::Purple)),
            destination: Zone::Hand(A),
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    for icon in [Icon::Concept, Icon::Efficiency] {
        let basic = game.position().basic_icon_count(A, icon);
        assert_eq!(
            game.position().icon_count(A, icon),
            basic + if basic > 0 { 3 } else { 0 }
        );
    }
}

#[test]
fn catherine_transfers_the_melder_top_with_suppression_then_preserves_original_outcome() {
    for player in [A, B] {
        let mut f = Fixture::default();
        f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
        f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
        let top = base(&mut f, "B012", Zone::Board(player, Color::Blue));
        let incoming = base(&mut f, "B011", Zone::Hand(player));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(incoming, Zone::Hand(player), player, Color::Blue),
            Context::new(player),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Performed,
                card: Some(incoming),
                value: Some(1),
                short: false,
            }))
        );
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            Event::Revealed {
                card: incoming,
                source: Zone::Hand(player),
                executor: player,
                inside_karma: true
            }
        );
        let events = &events[1..];
        assert_eq!(moved(&events[0]).card, top);
        assert_eq!(moved(&events[0]).executor, player);
        assert_eq!(moved(&events[0]).destination, Zone::Hand(player));
        assert!(moved(&events[0]).inside_karma);
        assert_eq!(moved(&events[1]).card, incoming);
        assert!(!moved(&events[1]).inside_karma);
        assert_eq!(game.position().cards(Zone::Hand(player)), &[top]);
    }
}

#[test]
fn catherine_can_transfer_itself_and_resume_the_exact_pending_board_meld() {
    let mut f = Fixture::default();
    let catherine = f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(catherine, Zone::Board(A, Color::Purple), A, Color::Purple),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(catherine),
            value: Some(6),
            short: false,
        }))
    );
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, catherine);
    assert_eq!(moved(&events[1]).source, Zone::Hand(A));
    assert_eq!(game.position().top(A, Color::Purple), Some(catherine));
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn brin_foreign_sailing_survives_source_and_permission_removal_and_rewards_activator() {
    let mut f = Fixture::default();
    base(&mut f, "B007", Zone::Board(A, Color::Green));
    let brin = f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    let sailing = f.sailing(Zone::Board(B, Color::Green));
    let own_draw = base(&mut f, "B009", Zone::Deck(Deck::base(1)));
    let shared_draw = base(&mut f, "B007", Zone::Deck(Deck::base(1)));
    base(&mut f, "B062", Zone::Deck(Deck::base(6)));
    let reward = f.figure(Figure::CatherineTheGreat, Zone::Deck(Deck::figures(6)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().dogma_source_owner(A, sailing), Some(B));
    game.take_action(
        Action::Dogma {
            card: sailing,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert!(matches!(
        events[0],
        Event::Splayed {
            player: A,
            color: Color::Green,
            to: Splay::Up,
            inside_karma: true,
            ..
        }
    ));
    let moves: Vec<_> = events
        .iter()
        .filter_map(|event| {
            if let Event::Moved(m) = event {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        moves.iter().map(|m| m.card).collect::<Vec<_>>(),
        [
            shared_draw,
            sailing,
            shared_draw,
            own_draw,
            brin,
            own_draw,
            reward
        ]
    );
    assert_eq!(
        moves.iter().map(|m| m.executor).collect::<Vec<_>>(),
        [B, B, B, A, A, A, A]
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[sailing]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[brin, reward]);
    assert_eq!(game.position().dogma_source_owner(A, shared_draw), None);
    assert!(game.position().check_conservation());
}

#[test]
fn brin_admits_every_top_base_color_but_never_figures_or_buried_cards() {
    let mut f = Fixture::default();
    let brin = f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    let sources: Vec<_> = ["B001", "B004", "B007", "B012", "B014"]
        .into_iter()
        .map(|code| {
            let card = base_card(code).unwrap();
            let color = card.color;
            f.add(card, Zone::Board(B, color))
        })
        .collect();
    let buried = f.writing(Zone::Board(A, Color::Blue));
    let figure = f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for source in sources {
        assert_eq!(game.position().dogma_source_owner(A, source), Some(B));
        assert_eq!(
            game.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .dogma_source_owner(A, source),
            None
        );
    }
    for card in [brin, buried, figure] {
        let before = game.clone();
        assert_eq!(
            game.take_action(
                Action::Dogma {
                    card,
                    auspice: false
                },
                ActionKind::Regular
            ),
            Err(Error::InvalidAction)
        );
        assert_eq!(game, before);
    }
}

#[test]
fn brin_foreign_oars_keeps_the_activator_as_demander_and_does_not_reward_demands() {
    let mut f = Fixture::default();
    f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    f.metalworking(Zone::Board(A, Color::Red));
    let oars = f.oars(Zone::Board(B, Color::Red));
    let payment = f.sailing(Zone::Hand(B));
    let draw = f.metalworking(Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: oars,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(events.len(), 3);
    assert_eq!(moved(&events[0]).card, payment);
    assert_eq!(moved(&events[0]).executor, B);
    assert_eq!(moved(&events[0]).destination, Zone::Score(A));
    assert_eq!(moved(&events[1]).card, draw);
    assert_eq!(moved(&events[1]).executor, B);
    assert!(
        matches!(events[2], Event::Revealed { card, source: Zone::Hand(B), executor: B, .. } if card == draw)
    );
    assert_eq!(game.position().top(B, Color::Red), Some(oars));
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn brin_does_not_splay_for_own_dogma_and_foreign_permission_disappears_under_cover() {
    let mut f = Fixture::default();
    f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    f.add(base_card("B010").unwrap(), Zone::Board(A, Color::Blue));
    let writing = f.writing(Zone::Board(A, Color::Blue));
    let foreign = f.sailing(Zone::Board(B, Color::Green));
    let cover = f.sailing(Zone::Hand(A));
    let draw = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: writing,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, draw);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
    game.start(meld(cover, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().dogma_source_owner(A, foreign), None);
}

#[test]
fn catherine_pending_meld_clones_across_every_transfer_and_source_removal_boundary() {
    let mut f = Fixture::default();
    let catherine = f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    let incoming = base(&mut f, "B014", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(A), A, Color::Purple),
        Context::new(A),
    )
    .unwrap();
    let mut expected = game.clone();
    let mut all = Vec::new();
    let status = finish(&mut expected, &mut all);
    assert_eq!(expected.position().cards(Zone::Hand(A)), &[catherine]);
    assert!(!expected.position().has_figure(A, Figure::CatherineTheGreat));
    let mut prefix = Vec::new();
    for _ in 0..100 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), status);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            result => {
                assert_eq!(result, status);
                assert_eq!(game, expected);
                return;
            }
        }
    }
    panic!("pending meld did not terminate");
}

#[test]
fn catherine_meld_to_an_empty_color_performs_only_the_original_movement() {
    let mut f = Fixture::default();
    f.figure(Figure::CatherineTheGreat, Zone::Board(B, Color::Purple));
    let incoming = f.writing(Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(A), A, Color::Blue),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        Event::Revealed {
            card: incoming,
            source: Zone::Hand(A),
            executor: A,
            inside_karma: true
        }
    );
    let events = &events[1..];
    assert_eq!(moved(&events[0]).card, incoming);
    assert!(!moved(&events[0]).inside_karma);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}
