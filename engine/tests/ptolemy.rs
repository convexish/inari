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
    panic!("foreign Dogma did not terminate");
}

fn activate(game: &mut Game, card: CardId) {
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
}

#[test]
fn native_foreign_writing_shares_opponent_first_and_rewards_activator() {
    let mut f = Fixture::default();
    f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
    let writing = f.writing(Zone::Board(B, Color::Blue));
    // Base supply controls age skipping, including the later Figure reward.
    f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let own_draw = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let shared_draw = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let reward = f.figure(Figure::Ptolemy, Zone::Deck(Deck::figures(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().dogma_source_owner(A, writing), Some(B));
    activate(&mut game, writing);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [shared_draw, own_draw, reward]
    );
    assert_eq!(
        events.iter().map(|e| moved(e).executor).collect::<Vec<_>>(),
        [B, A, A]
    );
    assert_eq!(game.position().top(B, Color::Blue), Some(writing));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[own_draw, reward]);
}

#[test]
fn foreign_demand_keeps_activator_as_demander_not_physical_owner() {
    let mut f = Fixture::default();
    f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
    let force = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(force.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    let source = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    f.cards[usize::from(source.0)].effects = vec![
        Effect {
            kind: EffectKind::Demand,
            instructions: vec![
                build::verb(Verb::Transfer, build::HAND).to(Destination::Hand(Who::Demander)),
            ],
        },
        Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![build::draw(1)],
        },
    ];
    let payment = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
    let draw = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    activate(&mut game, source);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(moved(&events[0]).card, payment);
    assert_eq!(moved(&events[0]).executor, B);
    assert_eq!(moved(&events[0]).destination, Zone::Hand(A));
    assert_eq!(moved(&events[1]).card, draw);
    assert_eq!(moved(&events[1]).executor, A);
    assert_eq!(events.len(), 2);
}

#[test]
fn activators_first_can_remove_ptolemy_without_cancelling_admitted_foreign_program() {
    let mut f = Fixture::default();
    let ptolemy = f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    // Opponent Homer must not respond to somebody else's Dogma action.
    f.figure(Figure::Homer, Zone::Board(B, Color::Purple));
    let force = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(force.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Board(B, Color::Blue),
        vec![build::draw(1)],
    );
    let first = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let draw = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    activate(&mut game, source);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [first, first, ptolemy, draw]
    );
    assert!(
        events[..3]
            .iter()
            .all(|e| moved(e).inside_karma && moved(e).executor == A)
    );
    assert_eq!(moved(&events[3]).executor, A);
    assert!(!moved(&events[3]).inside_karma);
    assert_eq!(game.position().dogma_source_owner(A, source), None);
}

#[test]
fn foreign_source_removal_does_not_erase_later_effects_or_change_execution_order() {
    let mut f = Fixture::default();
    f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Board(B, Color::Blue),
        vec![
            build::verb(Verb::Score, Selector::TopOf(Who::Either, Color::Blue))
                .to(Destination::Score(Who::Executor)),
        ],
    );
    f.cards[usize::from(source.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![build::draw(1)],
    });
    let own = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let shared = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let reward = f.figure(Figure::Ptolemy, Zone::Deck(Deck::figures(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    activate(&mut game, source);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(moved(&events[0]).card, source);
    assert_eq!(moved(&events[0]).source, Zone::Board(B, Color::Blue));
    assert_eq!(moved(&events[0]).destination, Zone::Score(B));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [source, shared, own, reward]
    );
    assert_eq!(game.position().cards(Zone::Score(B)), &[source]);
    assert!(game.position().check_conservation());
}

#[test]
fn own_tops_stay_legal_but_foreign_nonblue_buried_and_figure_tops_are_not_dogma() {
    let mut f = Fixture::default();
    let ptolemy = f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
    let own = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let foreign = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let buried = f.writing(Zone::Board(B, Color::Blue));
    let figure = f.figure(Figure::Daedalus, Zone::Board(B, Color::Blue));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().dogma_source_owner(A, own), Some(A));
    for card in [ptolemy, foreign, buried, figure] {
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
    activate(&mut game, own);
    finish(&mut game, &mut Vec::new());
}

#[test]
fn borrowed_source_self_execution_still_skips_demands_and_uses_executor() {
    let mut f = Fixture::default();
    f.figure(Figure::Ptolemy, Zone::Board(A, Color::Green));
    let source = f.writing(Zone::Board(B, Color::Blue));
    f.cards[usize::from(source.0)].effects.insert(
        0,
        Effect {
            kind: EffectKind::Demand,
            instructions: vec![
                build::verb(Verb::Transfer, build::HAND).to(Destination::Hand(Who::Demander)),
            ],
        },
    );
    let kept = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let draw = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let mut context = Context::new(A);
    context.source_owner = B;
    game.self_execute(source, context).unwrap();
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, draw);
    assert_eq!(moved(&events[0]).executor, A);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[kept, draw]);
}
