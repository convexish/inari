use inno_engine::*;

mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("Figure scenario did not terminate");
}

fn blank(f: &mut Fixture, card: CardId) {
    f.cards[usize::from(card.0)].symbols = [Symbol::Blank; SLOTS];
}

fn cover(game: &mut Game, card: CardId, color: Color) {
    game.start(
        Operation::Move {
            verb: Verb::Meld,
            target: target(card, Zone::Hand(A)),
            destination: Zone::Board(A, color),
        },
        Context::new(A),
    )
    .unwrap();
    finish(game, &mut Vec::new());
}

#[test]
fn tigernmas_counts_only_physical_hand_and_covering_disables_it() {
    let mut f = Fixture::default();
    let tiger = f.figure(Figure::Tigernmas, Zone::Board(A, Color::Red));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    blank(&mut f, tiger);
    blank(&mut f, meiji);
    let covering = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    for _ in 0..4 {
        f.tech(Color::Yellow, 1, Zone::Hand(B), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().targets(LogicalZone::Hand(A)).count(), 6);
    assert_eq!(game.position().score(A), 2);
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .score(A),
        0
    );
    cover(&mut game, covering, Color::Red);
    assert_eq!(game.position().score(A), 0);
}

#[test]
fn sinuhe_ignores_gilgamesh_and_maxwell_added_authority() {
    let mut f = Fixture::default();
    for (figure, color) in [
        (Figure::Sinuhe, Color::Purple),
        (Figure::Gilgamesh, Color::Red),
        (Figure::JamesClerkMaxwell, Color::Blue),
    ] {
        let card = f.figure(figure, Zone::Board(A, color));
        blank(&mut f, card);
    }
    let authority = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.cards[usize::from(authority.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    let covering = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().icon_count(A, Icon::Authority), 9);
    assert_eq!(game.position().score(A), 4);
    cover(&mut game, covering, Color::Purple);
    assert_eq!(game.position().score(A), 0);
}

#[test]
fn polhem_uses_basic_industry_and_physical_special_and_decree_achievements() {
    let mut f = Fixture::default();
    for (figure, color) in [
        (Figure::ChristopherPolhem, Color::Yellow),
        (Figure::Gilgamesh, Color::Red),
        (Figure::JamesClerkMaxwell, Color::Blue),
        (Figure::AlfredNobel, Color::Green),
    ] {
        let card = f.figure(figure, Zone::Board(A, color));
        blank(&mut f, card);
    }
    let icons = f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.cards[usize::from(icons.0)].symbols = four_slots([
        Symbol::Icon(Icon::Industry),
        Symbol::Icon(Icon::Industry),
        Symbol::Icon(Icon::Authority),
        Symbol::Blank,
    ]);
    f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
    let covering = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().icon_count(A, Icon::Industry), 4);
    assert_eq!(game.position().achievement_count(A), 1);
    assert!(game.position().effective_achievement_count(A) > 1);
    assert_eq!(game.position().score(A), 2);
    game.start(
        Operation::ClaimSpecial {
            special: Special::Empire,
            check_eligibility: false,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    game.start(
        Operation::ClaimDecree {
            decree: Decree::War,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().achievement_count(A), 3);
    assert_eq!(game.position().score(A), 6);
    cover(&mut game, covering, Color::Yellow);
    assert_eq!(game.position().score(A), 0);
}

fn daedalus_fixture() -> (Fixture, CardId, CardId, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    for _ in 0..3 {
        f.tech(Color::Yellow, 10, Zone::Score(A), vec![]);
    }
    let age_three = f.tech(Color::Green, 3, Zone::AvailableAchievements, vec![]);
    let age_four = f.tech(Color::Purple, 4, Zone::AvailableAchievements, vec![]);
    // Keep the Base age available for the opponent's Figures draw response.
    f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.figure(Figure::Sinuhe, Zone::Deck(Deck::figures(1)));
    (f, age_three, age_four, drawn)
}

#[test]
fn daedalus_claims_use_only_nonempty_unsplayed_colors_and_do_not_change_draws() {
    let (f, age_three, age_four, drawn) = daedalus_fixture();
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.highest_top_value(A), 1);
    assert_eq!(position.highest_claim_value(A), 3);
    assert_eq!(
        position
            .clone()
            .with_expansions(Expansions::BASE)
            .highest_claim_value(A),
        1
    );
    assert!(!position.eligible_for_special(A, Special::Universe));
    let response = position.cards(Zone::Deck(Deck::figures(1)))[0];
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    assert_eq!(
        game.take_action(Action::Achieve { card: age_four }, ActionKind::Free),
        Err(Error::InvalidAction)
    );
    game.take_action(Action::Draw, ActionKind::Free).unwrap();
    assert_eq!(
        finish(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    game.take_action(Action::Achieve { card: age_three }, ActionKind::Free)
        .unwrap();
    assert_eq!(
        finish(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[age_three]);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[response]);
}

#[test]
fn daedalus_checked_operation_rechecks_splay_and_covering() {
    for covered in [false, true] {
        let (mut f, age_three, _, _) = daedalus_fixture();
        let covering = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if covered {
            cover(&mut game, covering, Color::Blue);
            assert_eq!(game.position().highest_claim_value(A), 1);
        } else {
            game.start(
                Operation::Splay {
                    player: A,
                    color: Color::Red,
                    direction: Splay::Left,
                },
                Context::new(A),
            )
            .unwrap();
            finish(&mut game, &mut Vec::new());
            assert_eq!(game.position().highest_claim_value(A), 2);
        }
        game.start(
            Operation::Claim {
                target: target(age_three, Zone::AvailableAchievements),
                eligibility: ClaimEligibility::Available,
            },
            Context::new(A),
        )
        .unwrap();
        finish(&mut game, &mut Vec::new());
        assert!(game.position().cards(Zone::Achievements(A)).is_empty());
        assert_eq!(
            game.take_action(Action::Achieve { card: age_three }, ActionKind::Free),
            Err(Error::InvalidAction)
        );
    }
}

fn decree_finish(game: &mut Game, decree: Decree, events: &mut Vec<Event>) {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = menu
                    .options
                    .iter()
                    .position(|o| *o == Alternative::Decree(decree))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                assert_eq!(
                    game.position().decree_location(decree),
                    DecreeLocation::Owned(A)
                );
                return;
            }
        }
    }
    panic!("Decree did not terminate");
}

#[test]
fn all_new_decree_permissions_admit_pay_and_execute_real_actions() {
    for (figure, color, count, decree) in [
        (Figure::Tigernmas, Color::Red, 2, Decree::War),
        (Figure::Sinuhe, Color::Purple, 2, Decree::Rivalry),
        (Figure::Ptolemy, Color::Green, 2, Decree::Trade),
        (
            Figure::JohnLoudonMcAdam,
            Color::Yellow,
            2,
            Decree::Expansion,
        ),
        (
            Figure::ChristopherPolhem,
            Color::Yellow,
            2,
            Decree::Expansion,
        ),
        (
            Figure::WilliamShakespeare,
            Color::Purple,
            1,
            Decree::Rivalry,
        ),
        (Figure::WilliamShakespeare, Color::Purple, 3, Decree::War),
        (
            Figure::WilliamShakespeare,
            Color::Purple,
            3,
            Decree::Expansion,
        ),
        (Figure::WilliamShakespeare, Color::Purple, 3, Decree::Trade),
        (
            Figure::WilliamShakespeare,
            Color::Purple,
            3,
            Decree::Advancement,
        ),
        (
            Figure::WilliamShakespeare,
            Color::Purple,
            3,
            Decree::Rivalry,
        ),
    ] {
        for kind in [ActionKind::Regular, ActionKind::Free] {
            let mut f = Fixture::default();
            f.figure(figure, Zone::Board(A, color));
            let mut payment = Vec::new();
            for hand_figure in [Figure::Daedalus, Figure::Tigernmas, Figure::Sinuhe]
                .into_iter()
                .take(count)
            {
                payment.push(f.figure(hand_figure, Zone::Hand(A)));
            }
            payment.push(f.tech(Color::Green, 1, Zone::Hand(A), vec![]));
            for _ in 0..3 {
                f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
            }
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.take_action(Action::Decree, kind).unwrap();
            let mut events = Vec::new();
            decree_finish(&mut game, decree, &mut events);
            assert!(
                payment
                    .iter()
                    .all(|card| game.position().cards(Zone::Junk).contains(card))
            );
            assert_eq!(
                game.position().regular_actions_started(),
                u8::from(kind == ActionKind::Regular)
            );
            assert!(events.iter().any(|e| matches!(e, Event::DecreeMoved { decree: d, destination: DecreeLocation::Owned(A), .. } if *d == decree)));
        }
    }
}

#[test]
fn covered_opponent_and_insufficient_permission_sources_cannot_admit_decrees() {
    for (figure, color, required) in [
        (Figure::Tigernmas, Color::Red, 2),
        (Figure::Sinuhe, Color::Purple, 2),
        (Figure::ChristopherPolhem, Color::Yellow, 2),
        (Figure::WilliamShakespeare, Color::Purple, 1),
        (Figure::Ptolemy, Color::Green, 2),
        (Figure::JohnLoudonMcAdam, Color::Yellow, 2),
    ] {
        for case in 0..3 {
            let mut f = Fixture::default();
            let owner = if case == 1 { B } else { A };
            f.figure(figure, Zone::Board(owner, color));
            if case == 0 {
                f.tech(color, 1, Zone::Board(A, color), vec![]);
            }
            let count = if case == 2 { required - 1 } else { required };
            for hand_figure in [Figure::Daedalus, Figure::Tigernmas]
                .into_iter()
                .take(count)
            {
                f.figure(hand_figure, Zone::Hand(A));
            }
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let before = game.clone();
            assert_eq!(
                game.take_action(Action::Decree, ActionKind::Regular),
                Err(Error::InvalidAction)
            );
            assert_eq!(game, before);
        }
    }
}

#[test]
fn base_specialization_rejects_each_new_figure() {
    for (figure, color) in [
        (Figure::Tigernmas, Color::Red),
        (Figure::Daedalus, Color::Blue),
        (Figure::Sinuhe, Color::Purple),
        (Figure::WilliamShakespeare, Color::Purple),
        (Figure::ChristopherPolhem, Color::Yellow),
        (Figure::Ptolemy, Color::Green),
        (Figure::JohnLoudonMcAdam, Color::Yellow),
    ] {
        let mut f = Fixture::default();
        f.figure(figure, Zone::Board(A, color));
        assert!(Game::new(f.position(), Expansions::BASE).is_err());
        assert!(
            !figure
                .shortcut_dependencies()
                .contains(&ShortcutDependency::HandMembership)
        );
    }
}

#[test]
fn tigernmas_permission_uses_initial_logical_hand_and_action_kind() {
    for kind in [ActionKind::Regular, ActionKind::Free] {
        let mut f = Fixture::default();
        f.figure(Figure::Tigernmas, Zone::Board(A, Color::Red));
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        let borrowed = [
            f.figure(Figure::Daedalus, Zone::Hand(B)),
            f.figure(Figure::Sinuhe, Zone::Hand(B)),
            f.tech(Color::Green, 1, Zone::Hand(B), vec![]),
        ];
        let own = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if kind == ActionKind::Free {
            game.begin_action(ActionKind::Regular).unwrap();
            let before = game.clone();
            assert_eq!(
                game.take_action(Action::Decree, ActionKind::Regular),
                Err(Error::InvalidAction)
            );
            assert_eq!(game, before);
        }
        game.take_action(Action::Decree, kind).unwrap();
        let mut events = Vec::new();
        decree_finish(&mut game, Decree::War, &mut events);
        assert!(
            borrowed
                .into_iter()
                .chain([own])
                .all(|card| game.position().cards(Zone::Junk).contains(&card))
        );
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(game.position().regular_actions_started(), 1);
    }
}

#[test]
fn polhem_unions_normal_permissions_and_homer_preserves_initial_color_snapshot() {
    let mut f = Fixture::default();
    f.figure(Figure::ChristopherPolhem, Zone::Board(A, Color::Yellow));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let figures = [Figure::AlexanderTheGreat, Figure::ShenKuo, Figure::Daedalus]
        .map(|figure| f.figure(figure, Zone::Hand(A)));
    let base = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Decree, ActionKind::Free).unwrap();
    let mut events = Vec::new();
    for card in figures {
        choose_card(&mut game, &mut events, card);
    }
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options,
        [
            Alternative::Decree(Decree::War),
            Alternative::Decree(Decree::Expansion),
            Alternative::Decree(Decree::Trade),
            Alternative::Decree(Decree::Advancement),
        ]
    );
    assert!(
        events[..3]
            .iter()
            .all(|event| moved(event).verb == Some(Verb::Tuck) && moved(event).inside_karma)
    );
    assert_eq!(game.position().cards(Zone::Junk), &[base]);
    game.choose(1).unwrap();
    decree_finish(&mut game, Decree::Expansion, &mut events);
}

#[test]
fn polhem_score_product_does_not_overflow_the_admitted_card_domain() {
    let mut f = Fixture::default();
    f.figure(Figure::ChristopherPolhem, Zone::Board(A, Color::Yellow));
    for _ in 0..25_000 {
        let card = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        f.cards[usize::from(card.0)].symbols = four_slots([Symbol::Icon(Icon::Industry); 4]);
    }
    for _ in 0..30_000 {
        f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Green,
            direction: Splay::Aslant,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().score(A), 3_000_060_000_i64);
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .score(A),
        0
    );
}

#[test]
fn ptolemy_and_mcadam_union_their_permissions_with_normal_and_other_each_permissions() {
    for shakespeare in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
        f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
        if shakespeare {
            f.figure(Figure::WilliamShakespeare, Zone::Board(A, Color::Purple));
        }
        for figure in [
            Figure::Daedalus,
            Figure::AlexanderTheGreat,
            Figure::GalileoGalilei,
        ] {
            f.figure(figure, Zone::Hand(A));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(Action::Decree, ActionKind::Regular)
            .unwrap();
        let mut events = Vec::new();
        let menu = loop {
            let menu = decision(&mut game, &mut events);
            if menu
                .options
                .iter()
                .all(|option| matches!(option, Alternative::Decree(_)))
            {
                break menu;
            }
            game.choose(0).unwrap();
        };
        let mut expected = vec![
            Alternative::Decree(Decree::War),
            Alternative::Decree(Decree::Expansion),
            Alternative::Decree(Decree::Trade),
            Alternative::Decree(Decree::Advancement),
        ];
        if shakespeare {
            expected.push(Alternative::Decree(Decree::Rivalry));
        }
        assert_eq!(menu.options, expected);
    }
}

#[test]
fn hedy_first_can_remove_daedalus_claim_eligibility_before_commit() {
    let mut f = Fixture::default();
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
    let achievement = f.tech(Color::Red, 10, Zone::AvailableAchievements, vec![]);
    for _ in 0..5 {
        f.tech(Color::Red, 10, Zone::Score(A), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Green,
            direction: Splay::Up,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().highest_claim_value(A), 10);
    assert!(game.position().eligible_for_achievement(A, achievement));
    game.start(
        Operation::Claim {
            target: target(achievement, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Available,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            ..
        }))
    ));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Up);
    assert_eq!(game.position().highest_claim_value(A), 9);
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        &[achievement]
    );
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    assert!(
        events
            .iter()
            .all(|event| matches!(event, Event::Splayed { .. }))
    );
}
