//! Artifacts: the printed catalog, Yamato's two values, the
//! public display and Museum zones, and compel dispatch. Artifacts games
//! are not constructible until every program is authored.

use std::collections::BTreeMap;

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
    panic!("Dogma did not terminate");
}

fn icons(f: &mut Fixture, card: CardId, icon: Icon, count: usize) {
    let symbols = &mut f.cards[usize::from(card.0)].symbols;
    *symbols = [Symbol::Blank; SLOTS];
    symbols[..count].fill(Symbol::Icon(icon));
}

#[test]
fn printed_catalog_matches_the_publisher_decks_and_reserved_identities() {
    assert_eq!(ARTIFACTS_METADATA.len(), 115);
    assert!(ARTIFACTS_METADATA.windows(2).all(|w| w[0].code < w[1].code));
    let mut per_age = BTreeMap::new();
    for metadata in &ARTIFACTS_METADATA {
        assert_eq!(metadata.set, CardSet::Artifacts);
        *per_age.entry(metadata.value).or_insert(0) += 1;
        let image = metadata
            .symbols
            .iter()
            .filter(|&&s| s == Symbol::Image)
            .count();
        assert_eq!(image, 1, "{} has one image slot", metadata.code);
        let id = StableId::from_code(metadata.code).unwrap();
        assert!((460..575).contains(&id.0), "{}", metadata.code);
    }
    assert_eq!(per_age[&1], 15);
    assert!((2..=11).all(|age| per_age[&age] == 10));
    assert!(Expansions::ARTIFACTS.includes(CardSet::Artifacts));
    assert_eq!(Expansions::ARTIFACTS.count(), 1);
    assert!(!Expansions::BASE.includes(CardSet::Artifacts));
}

#[test]
fn unauthored_programs_and_games_fail_explicitly() {
    assert_eq!(
        artifacts_card("A999").unwrap_err(),
        Error::InvalidSetup("unknown Artifacts card")
    );
    // While the catalog is incomplete, every unauthored card and any full
    // game is rejected rather than stubbed.
    let unauthored: Vec<_> = ARTIFACTS_METADATA
        .iter()
        .filter(|m| artifacts_card(m.code).is_err())
        .collect();
    for metadata in &unauthored {
        assert_eq!(
            artifacts_card(metadata.code).unwrap_err(),
            Error::Unsupported("Artifacts program not authored")
        );
    }
    if !unauthored.is_empty() {
        assert_eq!(
            Game::new_game(1, Expansions::ARTIFACTS).unwrap_err(),
            Error::Unsupported("Artifacts program not authored")
        );
    } else {
        let game = Game::new_game(1, Expansions::ARTIFACTS).unwrap();
        assert!(game.position().check_conservation());
    }
}

#[test]
fn yamato_is_eleven_only_on_boards_displays_and_museums() {
    let yamato = artifacts_card("A077").unwrap();
    assert_eq!((yamato.value, yamato.face_value()), (8, 11));
    assert_eq!(
        yamato.origin_deck(),
        Deck {
            set: CardSet::Artifacts,
            age: 8
        }
    );
    assert!(yamato.effects.is_empty());
    assert_eq!(base_card("B001").unwrap().face_value(), 1);
    let cases = [
        (Zone::Hand(A), 8),
        (Zone::Score(A), 8),
        (Zone::Deck(yamato.origin_deck()), 8),
        (Zone::Junk, 8),
        (Zone::Board(A, Color::Red), 11),
        (Zone::Display(B), 11),
        (Zone::Museum(A), 11),
    ];
    for (zone, expected) in cases {
        let mut f = Fixture::default();
        let card = f.add(yamato.clone(), zone);
        let position = f.position();
        assert_eq!(position.value(card), expected, "{zone:?}");
        assert!(position.check_conservation());
        let game = Game::new(position, Expansions::ARTIFACTS).unwrap();
        assert_eq!(game.position().cards(zone), &[card]);
    }
}

#[test]
fn display_and_museum_zones_need_the_expansion_and_count_as_empty_when_unused() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Red, 1, Zone::Display(A), vec![]);
    let position = f.position();
    assert_eq!(position.cards(Zone::Display(A)), &[card]);
    assert_eq!(position.available_museums(), 5);
    assert_eq!(
        Game::new(position.clone(), Expansions::BASE).unwrap_err(),
        Error::InvalidSetup("a display or Museum without Artifacts")
    );
    assert!(Game::new(position, Expansions::ARTIFACTS).is_ok());

    let mut f = Fixture::default();
    let a1 = f.tech(Color::Red, 1, Zone::Museum(A), vec![]);
    let a2 = f.tech(Color::Blue, 2, Zone::Museum(A), vec![]);
    let b1 = f.tech(Color::Green, 3, Zone::Museum(B), vec![]);
    let position = f.position();
    assert_eq!(position.cards(Zone::Museum(A)), &[a1, a2]);
    assert_eq!(position.cards(Zone::Museum(B)), &[b1]);
    assert_eq!(position.available_museums(), 2);
    assert_eq!(position.claimed_museums(A), 0);
    assert_eq!(position.claimed_achievement_count(A), 0);
    assert!(position.check_conservation());
}

/// A compel's victim is an eligible sharer, never the owner; it does not
/// earn the sharing reward, which a shared non-demand effect still does.
#[test]
fn compels_run_for_eligible_sharers_without_a_sharing_reward() {
    for (b_icons, compel_only, expected) in [
        (1, false, vec![B, B, A, A]),
        (1, true, vec![B]),
        (0, false, vec![A]),
        (0, true, vec![]),
    ] {
        let mut f = Fixture::default();
        let source = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        icons(&mut f, source, Icon::Authority, 1);
        let mut effects = vec![Effect {
            kind: EffectKind::Compel,
            instructions: vec![build::draw(1)],
        }];
        if !compel_only {
            effects.push(Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![build::draw(1)],
            });
        }
        f.cards[usize::from(source.0)].effects = effects;
        let b_top = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
        icons(&mut f, b_top, Icon::Authority, b_icons);
        for _ in 0..4 {
            f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
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
            Status::Complete(ReturnValue::Unit),
            "{b_icons} {compel_only}"
        );
        assert_eq!(
            events.iter().map(|e| moved(e).executor).collect::<Vec<_>>(),
            expected,
            "{b_icons} {compel_only}"
        );
        assert!(game.position().check_conservation());
    }
}

const MOVED: Slot = Slot::effect(0);
const RED_TOPS: Selector = build::filtered(&build::TOPS, &[Filter::Has(Test::Color(Color::Red))]);
const GREEN_TOPS: Selector =
    build::filtered(&build::TOPS, &[Filter::Has(Test::Color(Color::Green))]);
const MOVED_VALUE: Expr = Expr::Value(Reg::Slot(MOVED));

fn run(game: &mut Game) -> Status {
    let mut events = Vec::new();
    finish(game, &mut events)
}

/// A record keeps the value its move showed: a Yamato returned from the
/// board or melded from hand reads 11 afterwards, one junked from hand 8.
/// The previous result's card reads the same way.
#[test]
fn records_and_results_keep_the_value_their_move_showed() {
    use build::*;
    let cases: [(Zone, Vec<Instruction>, u8); 4] = [
        (
            Zone::Board(A, Color::Red),
            vec![return_(RED_TOPS).keep(MOVED), draw_age(MOVED_VALUE)],
            11,
        ),
        (
            Zone::Hand(A),
            vec![meld(HAND).keep(MOVED), draw_age(MOVED_VALUE)],
            11,
        ),
        (
            Zone::Hand(A),
            vec![junk(HAND).keep(MOVED), draw_age(MOVED_VALUE)],
            8,
        ),
        (
            Zone::Board(A, Color::Red),
            vec![return_(RED_TOPS), draw_age(Expr::Value(Reg::Last))],
            11,
        ),
    ];
    for (zone, program, expected) in cases {
        let mut f = Fixture::default();
        let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), program);
        let yamato = f.add(artifacts_card("A077").unwrap(), zone);
        let eight = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
        let eleven = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
        game.self_execute(source, Context::new(A)).unwrap();
        assert_eq!(
            run(&mut game),
            Status::Complete(ReturnValue::Unit),
            "{zone:?}"
        );
        let drawn = if expected == 11 { eleven } else { eight };
        assert!(
            game.position().cards(Zone::Hand(A)).contains(&drawn),
            "{zone:?}: {:?}",
            game.position().cards(Zone::Hand(A))
        );
        assert!(game.position().check_conservation());
        let _ = yamato;
    }
}

/// An explicit reveal makes a hand Yamato 11 through the revealing effect
/// and its children; a sibling effect, or anything after an abort, sees
/// the remembered but hidden card as 8 again.
#[test]
fn an_explicit_reveal_shows_yamatos_face_through_the_effect_and_its_children() {
    use build::*;
    let mut f = Fixture::default();
    let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    // One icon keeps B from sharing (and drawing first).
    icons(&mut f, source, Icon::Authority, 1);
    f.cards[usize::from(source.0)].effects = vec![
        Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![
                reveal(HAND),
                draw_age(Expr::Max(&HAND)),
                score(LAST),
                Instruction::Bind(Binding::Card(GREEN_TOPS)),
                build::execute(ExecutionMode::SelfExecute, Who::Executor),
            ],
        },
        Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![draw_age(Expr::Max(&HAND)), score(LAST)],
        },
    ];
    f.tech(
        Color::Green,
        1,
        Zone::Board(A, Color::Green),
        vec![draw_age(Expr::Max(&HAND)), score(LAST)],
    );
    let yamato = f.add(artifacts_card("A077").unwrap(), Zone::Hand(A));
    let eight = f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let first = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let second = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    assert_eq!(run(&mut game), Status::Complete(ReturnValue::Unit));
    assert_eq!(
        game.position().cards(Zone::Score(A)),
        &[second, first, eight]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[yamato]);
    assert_eq!(game.position().value(yamato), 8);

    // Ending the action ends the reveal too.
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![
            reveal(HAND),
            draw_age(Expr::Max(&HAND)),
            Instruction::EndAction,
            draw_age(Expr::Max(&HAND)),
        ],
    );
    icons(&mut f, source, Icon::Authority, 1);
    let yamato = f.add(artifacts_card("A077").unwrap(), Zone::Hand(A));
    f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let eleven = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    assert_eq!(run(&mut game), Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[yamato, eleven]);
    assert_eq!(game.position().value(yamato), 8);
    assert!(game.position().check_conservation());
}
