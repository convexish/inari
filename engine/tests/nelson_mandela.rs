use inno_engine::*;

mod common;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = menu
                    .options
                    .iter()
                    .position(|option| {
                        matches!(
                            option,
                            Alternative::Karma {
                                figure: Figure::NelsonMandela,
                                ..
                            }
                        )
                    })
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("Mandela action did not terminate");
}

fn transfer(game: &mut Game, card: CardId, source: Zone) {
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(card, source),
            destination: Zone::Hand(A),
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(finish(game, &mut Vec::new()), Status::Complete(_)));
    assert!(game.position().check_conservation());
}

#[test]
fn each_counts_basic_pairs_live_and_independently_without_physical_claim_costs() {
    let mut f = Fixture::default();
    let mandela = f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
    let maxwell = f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    f.figure(Figure::NiccoloMachiavelli, Zone::Board(A, Color::Purple));
    let polhem = f.figure(Figure::ChristopherPolhem, Zone::Board(A, Color::Yellow));
    base(&mut f, "B007", Zone::Board(A, Color::Green));
    base(&mut f, "B008", Zone::Board(A, Color::Green));
    let claim = base(&mut f, "B001", Zone::AvailableAchievements);
    base(&mut f, "B062", Zone::Score(A));
    let cover = base(&mut f, "B001", Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().basic_icon_count(A, Icon::Avatar), 4);
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 5);
    assert_eq!(game.position().effective_achievement_count(A), 2);
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .effective_achievement_count(A),
        0
    );
    assert!(game.position().eligible_for_achievement(A, claim));
    let score = game.position().score(A);
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Green,
            direction: Splay::Right,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().effective_achievement_count(A), 3);
    assert_eq!(game.position().score(A), score);
    assert!(game.position().eligible_for_achievement(A, claim));
    assert_eq!(game.position().achievement_count(A), 0);
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());

    transfer(&mut game, polhem, Zone::Board(A, Color::Yellow));
    assert_eq!(game.position().basic_icon_count(A, Icon::Avatar), 3);
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 5);
    assert_eq!(game.position().effective_achievement_count(A), 2);
    transfer(&mut game, maxwell, Zone::Board(A, Color::Blue));
    assert_eq!(game.position().basic_icon_count(A, Icon::Avatar), 2);
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 2);
    assert_eq!(game.position().effective_achievement_count(A), 2);
    // Six points pay for the first age-one claim, but not a duplicate.
    assert_eq!(game.position().score(A), 6);
    assert!(game.position().eligible_for_achievement(A, claim));

    // Covering the source cancels Mandela, not Machiavelli's independent Each.
    let uncovered = game.clone();
    game.start(meld(cover, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().effective_achievement_count(A), 1);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        &[mandela, cover]
    );
    assert!(game.position().eligible_for_achievement(A, claim));
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    assert!(game.position().check_conservation());
    let mut removed = uncovered;
    transfer(&mut removed, mandela, Zone::Board(A, Color::Red));
    assert_eq!(removed.position().effective_achievement_count(A), 1);
    assert_eq!(removed.position().achievement_count(A), 0);
}

#[test]
fn second_regular_foreign_dogma_replaces_once_preserving_demander_and_suppressing_nobel() {
    let mut f = Fixture::default();
    let mandela = f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
    let brin = f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    let duke = f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    f.figure(Figure::AlfredNobel, Zone::Board(B, Color::Green));
    let gunpowder = base(&mut f, "B037", Zone::Board(B, Color::Red));
    let payment = base(&mut f, "B024", Zone::Board(B, Color::Purple));
    let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let protected = base(&mut f, "B075", Zone::Deck(Deck::base(7)));
    let spare = base(&mut f, "B099", Zone::Deck(Deck::base(10)));
    let draw = base(&mut f, "B105", Zone::Deck(Deck::base(10)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().dogma_source_owner(A, gunpowder), Some(B));
    assert_eq!(game.position().basic_icon_count(A, Icon::Industry), 0);
    assert_eq!(game.position().basic_icon_count(B, Icon::Authority), 3);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[draw]);
    assert_eq!(game.position().regular_actions_started(), 1);
    assert_eq!(game.position().top(A, Color::Red), Some(mandela));
    assert_eq!(game.position().top(A, Color::Green), Some(brin));
    assert_eq!(game.position().top(A, Color::Purple), Some(duke));
    events.clear();
    game.take_action(
        Action::Dogma {
            card: gunpowder,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 3);
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).card, moved(e).executor, moved(e).inside_karma))
            .collect::<Vec<_>>(),
        [(payment, B, true), (reward, A, true), (reward, A, true)]
    );
    assert_eq!(moved(&events[0]).source, Zone::Board(B, Color::Purple));
    assert_eq!(moved(&events[0]).destination, Zone::Score(A));
    assert_eq!(game.position().cards(Zone::Score(A)), &[payment, reward]);
    assert!(game.position().cards(Zone::Score(B)).is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(7))),
        &[protected]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[spare]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[draw]);
    assert_eq!(game.position().top(B, Color::Red), Some(gunpowder));
    assert_eq!(game.position().splay(A, Color::Green), Splay::None);
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(game.position().regular_actions_started(), 0);
    assert!(game.position().check_conservation());
    let completed = game.clone();
    let count = events.len();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), count);
    assert_eq!(game, completed);
}

#[test]
fn first_regular_and_free_foreign_dogmas_keep_ordinary_participant_rules() {
    for (kind, previous) in [
        (ActionKind::Regular, 0),
        (ActionKind::Free, 0),
        (ActionKind::Free, 1),
        (ActionKind::Free, 2),
    ] {
        let mut f = Fixture::default();
        f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
        f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
        f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
        let gunpowder = base(&mut f, "B037", Zone::Board(B, Color::Red));
        let payment = base(&mut f, "B024", Zone::Board(B, Color::Purple));
        let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        for _ in 0..previous {
            game.begin_action(ActionKind::Regular).unwrap();
        }
        game.take_action(
            Action::Dogma {
                card: gunpowder,
                auspice: false,
            },
            kind,
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        // B has more Industry: ordinary Gunpowder never demands its payment.
        assert!(events.iter().all(|event| !matches!(event, Event::Moved(_))));
        assert_eq!(game.position().top(B, Color::Purple), Some(payment));
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[reward]);
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        assert_eq!(game.position().turn_owner(), A);
        assert_eq!(
            game.position().regular_actions_started(),
            previous + u8::from(kind == ActionKind::Regular)
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn opponents_second_regular_dogma_does_not_borrow_mandelas_replacement() {
    let mut f = Fixture::default();
    f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
    let writing = f.writing(Zone::Board(B, Color::Blue));
    let draw = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_turn(B).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    game.take_action(
        Action::Dogma {
            card: writing,
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
    assert_eq!(game.position().cards(Zone::Hand(B)), &[draw]);
    assert!(events.iter().all(|event| !moved(event).inside_karma));
    assert_eq!(game.position().turn_owner(), A);
}

#[test]
fn second_regular_retains_foreign_source_after_its_own_demand_transfers_it() {
    let mut f = Fixture::default();
    let mandela = f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
    let brin = f.figure(Figure::SergeyBrin, Zone::Board(A, Color::Green));
    let monotheism = base(&mut f, "B024", Zone::Board(B, Color::Purple));
    let own = base(&mut f, "B001", Zone::Deck(Deck::base(1)));
    let demanded = base(&mut f, "B012", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    let mut without_permission = game.clone();
    transfer(&mut without_permission, brin, Zone::Board(A, Color::Green));
    let unchanged = without_permission.clone();
    assert_eq!(
        without_permission.take_action(
            Action::Dogma {
                card: monotheism,
                auspice: false
            },
            ActionKind::Regular
        ),
        Err(Error::InvalidAction)
    );
    assert_eq!(without_permission, unchanged);

    game.take_action(
        Action::Dogma {
            card: monotheism,
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
    assert_eq!(
        events
            .iter()
            .map(|event| (moved(event).card, moved(event).executor))
            .collect::<Vec<_>>(),
        [
            (monotheism, B),
            (demanded, B),
            (demanded, B),
            (own, A),
            (own, A),
            (mandela, A)
        ]
    );
    assert!(events[..5].iter().all(|event| moved(event).inside_karma));
    assert_eq!(moved(&events[5]).verb, Some(Verb::Score));
    assert!(
        !moved(&events[5]).inside_karma,
        "ordinary action end follows the completed replacement"
    );
    assert_eq!(
        game.position().cards(Zone::Score(A)),
        &[monotheism, mandela]
    );
    assert_eq!(game.position().top(B, Color::Purple), None);
    assert_eq!(game.position().top(B, Color::Blue), Some(demanded));
    assert_eq!(game.position().top(A, Color::Red), Some(own));
    assert!(!game.position().has_figure(A, Figure::NelsonMandela));
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(game.position().regular_actions_started(), 0);
    assert!(game.position().check_conservation());
}

#[test]
fn own_second_regular_dogma_runs_its_native_non_demand_under_suppression() {
    let mut f = Fixture::default();
    f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
    f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    let writing = f.writing(Zone::Board(A, Color::Blue));
    let draw = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    game.take_action(
        Action::Dogma {
            card: writing,
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
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, draw);
    assert_eq!(moved(&events[0]).executor, A);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[draw]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(game.position().regular_actions_started(), 0);
    assert!(game.position().check_conservation());
}
