use inno_engine::*;

mod common;
use common::*;

fn splay(game: &mut Game, direction: Splay, events: &mut Vec<Event>) -> Status {
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Red,
            direction,
        },
        Context::new(A),
    )
    .unwrap();
    settle(game, events)
}

#[test]
fn every_splay_exposes_the_correct_slots_in_both_kernels() {
    let mut f = Fixture::default();
    let bottom = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let top = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(bottom.0)].symbols = four_slots([
        Symbol::Icon(Icon::Authority),
        Symbol::Icon(Icon::Health),
        Symbol::Icon(Icon::Prosperity),
        Symbol::Icon(Icon::Industry),
    ]);
    f.cards[usize::from(top.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    let mut base_events = Vec::new();
    let mut events = Vec::new();
    for (direction, counts) in [
        (Splay::Left, [0, 0, 0, 1]),
        (Splay::Right, [1, 1, 0, 0]),
        (Splay::Up, [0, 1, 1, 1]),
        (Splay::Aslant, [1, 1, 1, 1]),
        (Splay::None, [0, 0, 0, 0]),
    ] {
        assert_eq!(
            splay(&mut base, direction, &mut base_events),
            splay(&mut figures, direction, &mut events)
        );
        assert_eq!(base.position(), figures.position());
        assert_eq!(base_events, events);
        assert_eq!(base.position().splay(A, Color::Red), direction);
        for (icon, count) in [
            Icon::Authority,
            Icon::Health,
            Icon::Prosperity,
            Icon::Industry,
        ]
        .into_iter()
        .zip(counts)
        {
            assert_eq!(base.position().basic_icon_count(A, icon), count);
        }
        assert_eq!(base.position().basic_icon_count(A, Icon::Concept), 4);
        assert_eq!(
            base.position().visible_card_count(A),
            if direction == Splay::None { 1 } else { 2 }
        );
        let event_count = events.len();
        assert_eq!(
            splay(&mut figures, direction, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Inapplicable,
                card: None,
                value: None,
                short: false,
            }))
        );
        assert_eq!(events.len(), event_count);
    }
}

#[test]
fn hidden_bonuses_follow_spatial_visibility_and_yis_printed_bonus_is_counted() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let top = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(top.0)].symbols[3] = Symbol::Bonus(2);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().score(A), 2);
    let mut events = Vec::new();
    splay(&mut game, Splay::Left, &mut events);
    assert_eq!(game.position().score(A), 2);
    splay(&mut game, Splay::Right, &mut events);
    assert_eq!(game.position().score(A), 5); // max 4 plus one other bonus
    assert_eq!(game.position().basic_icon_count(A, Icon::Avatar), 0);
    splay(&mut game, Splay::Up, &mut events);
    assert_eq!(game.position().score(A), 2);
    assert_eq!(game.position().basic_icon_count(A, Icon::Avatar), 1);
}

#[test]
fn removing_a_card_forgets_a_singletons_splay_and_melding_does_not_restore_it() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let top = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let incoming = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut events = Vec::new();
    splay(&mut game, Splay::Up, &mut events);
    game.start(
        Operation::Move {
            verb: Verb::Return,
            target: target(top, Zone::Board(A, Color::Red)),
            destination: Zone::Deck(Deck::base(2)),
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(events.len(), 2); // implicit collapse is part of the movement
    assert_eq!(
        splay(&mut game, Splay::Right, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    game.start(
        meld(incoming, Zone::Hand(A), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    assert_eq!(game.position().visible_card_count(A), 1);
    assert!(game.position().check_conservation());
}

#[test]
fn splay_operation_clones_at_every_boundary_and_does_not_move_cards() {
    let mut f = Fixture::default();
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let cards = game.position().cards(Zone::Board(A, Color::Red)).to_vec();
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Red,
            direction: Splay::Left,
        },
        Context::new(B),
    )
    .unwrap();
    let mut fork = game.clone();
    let mut events = Vec::new();
    assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    let mut fork_events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        settle(&mut fork, &mut fork_events)
    );
    assert_eq!(game, fork);
    assert_eq!(events, fork_events);
    assert_eq!(
        events,
        [Event::Splayed {
            player: A,
            color: Color::Red,
            from: Splay::None,
            to: Splay::Left,
            executor: B,
            inside_karma: false
        }]
    );
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), cards);
}
