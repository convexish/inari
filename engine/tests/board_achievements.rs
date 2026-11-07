use inno_engine::*;

mod common;
use common::*;

fn industry(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    let card = f.tech(color, value, zone, vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Industry);
    card
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("board claim did not terminate");
}

fn scored(f: &mut Fixture, value: u8, count: usize) {
    for _ in 0..count {
        f.tech(Color::Purple, value, Zone::Score(A), vec![]);
    }
}

#[test]
fn native_alias_claim_preserves_public_source_and_runs_opponent_response() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    let bottom = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let card = industry(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    scored(&mut f, 2, 5);
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let response = f.figure(Figure::Tigernmas, Zone::Deck(Deck::figures(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Red,
            direction: Splay::Right,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    let alias = game
        .position()
        .targets(LogicalZone::AvailableAchievements(A))
        .find(|t| t.card == card)
        .unwrap();
    assert_eq!(alias.physical, Zone::Board(A, Color::Red));
    for viewer in [A, B] {
        assert!(
            matches!(game.position().descriptor(alias, viewer), TargetDescriptor::Known { card: known, source: Zone::Board(A, Color::Red), .. } if known == card)
        );
    }
    assert!(
        !game
            .position()
            .targets(LogicalZone::AvailableAchievements(B))
            .any(|t| t.card == card)
    );
    assert!(
        !game
            .position()
            .clone()
            .with_expansions(Expansions::BASE)
            .targets(LogicalZone::AvailableAchievements(A))
            .any(|t| t.card == card)
    );
    game.take_action(Action::Achieve { card }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(moved(&events[0]).source, Zone::Board(A, Color::Red));
    assert_eq!(moved(&events[0]).destination, Zone::Achievements(A));
    assert_eq!(moved(&events[1]).card, response);
    assert_eq!(moved(&events[1]).executor, B);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), &[bottom]);
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[card]);
    assert!(game.position().check_conservation());
}

#[test]
fn printed_top_membership_does_not_inherit_board_icon_modifiers_or_specials() {
    let mut f = Fixture::default();
    let mcadam = f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let buried = industry(&mut f, Color::Green, 2, Zone::Board(A, Color::Green));
    let blank = f.tech(Color::Green, 3, Zone::Board(A, Color::Green), vec![]);
    let foreign = industry(&mut f, Color::Purple, 2, Zone::Board(B, Color::Purple));
    let supply = f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    let aliases: Vec<_> = position
        .targets(LogicalZone::AvailableAchievements(A))
        .map(|t| t.card)
        .collect();
    assert!(aliases.contains(&mcadam));
    assert!(aliases.contains(&supply));
    for forbidden in [buried, blank, foreign] {
        assert!(!aliases.contains(&forbidden));
    }
    assert_eq!(position.cards(Zone::AvailableAchievements), &[supply]);
    assert_eq!(
        position.special_location(Special::World),
        SpecialLocation::Available
    );
    assert!(Game::new(position, Expansions::BASE).is_err());
}

#[test]
fn covering_or_removing_permission_and_burying_target_revalidate_retained_alias() {
    for case in 0..3 {
        let mut f = Fixture::default();
        let mcadam = f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
        let card = industry(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
        let color = if case == 2 { Color::Red } else { Color::Yellow };
        let covering = f.tech(color, 1, Zone::Hand(A), vec![]);
        scored(&mut f, 2, 5);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let alias = game
            .position()
            .targets(LogicalZone::AvailableAchievements(A))
            .find(|t| t.card == card)
            .unwrap();
        let op = if case == 0 {
            Operation::Move {
                verb: Verb::Score,
                target: target(mcadam, Zone::Board(A, Color::Yellow)),
                destination: Zone::Score(A),
            }
        } else {
            meld(covering, Zone::Hand(A), A, color)
        };
        game.start(op, Context::new(A)).unwrap();
        finish(&mut game, &mut Vec::new());
        assert!(!game.position().eligible_for_achievement(A, card));
        game.start(
            Operation::Claim {
                target: alias,
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
        assert!(events.is_empty());
        assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    }
}

#[test]
fn mcadam_can_claim_himself_and_immediately_removes_remaining_aliases() {
    let mut f = Fixture::default();
    let mcadam = f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    let other = industry(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    scored(&mut f, 6, 5);
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.figure(Figure::Tigernmas, Zone::Deck(Deck::figures(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Achieve { card: mcadam }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[mcadam]);
    assert_eq!(game.position().top(A, Color::Yellow), None);
    assert!(!game.position().eligible_for_achievement(A, other));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Moved(m) if m.card == mcadam))
            .count(),
        1
    );
}

#[test]
fn board_claim_uses_duplicate_score_cost_and_daedalus_claim_only_value() {
    for enough in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
        let card = industry(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
        f.tech(Color::Purple, 2, Zone::Achievements(A), vec![]);
        scored(&mut f, 2, if enough { 10 } else { 9 });
        let position = f.position().with_expansions(Expansions::FIGURES);
        assert_eq!(position.eligible_for_achievement(A, card), enough);
    }
    let mut f = Fixture::default();
    f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
    let supply = f.tech(Color::Purple, 8, Zone::AvailableAchievements, vec![]);
    scored(&mut f, 8, 5);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.highest_top_value(A), 6);
    assert_eq!(position.highest_claim_value(A), 8);
    assert!(position.eligible_for_achievement(A, supply));
}

#[test]
fn hedy_first_precedes_physical_board_claim_and_does_not_replace_its_result() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let card = industry(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    for value in [1, 2] {
        f.tech(Color::Blue, value, Zone::Board(A, Color::Blue), vec![]);
    }
    scored(&mut f, 2, 5);
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.figure(Figure::Tigernmas, Zone::Deck(Deck::figures(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Achieve { card }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(events.is_empty());
    assert_eq!(game.position().top(A, Color::Red), Some(card));
    let index = menu
        .options
        .iter()
        .position(|o| {
            *o == Alternative::Splay {
                player: A,
                color: Color::Blue,
                direction: Splay::Up,
            }
        })
        .unwrap();
    game.choose(index).unwrap();
    finish(&mut game, &mut events);
    assert!(matches!(
        events[0],
        Event::Splayed {
            inside_karma: true,
            ..
        }
    ));
    assert_eq!(moved(&events[1]).card, card);
    assert_eq!(moved(&events[1]).source, Zone::Board(A, Color::Red));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[card]);
}
