use inno_engine::*;

mod common;
use common::*;

fn draw(value: u8) -> Instruction {
    build::draw(value)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("replacement action did not terminate");
}

fn source(f: &mut Fixture, instructions: Vec<Instruction>) -> CardId {
    let card = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), instructions);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    card
}

fn dogma(game: &mut Game, card: CardId) {
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
fn kepler_counts_distinct_own_top_values_above_three_once() {
    let mut f = Fixture::default();
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    let card = source(&mut f, vec![draw(1)]);
    f.cards[usize::from(card.0)].value = 6;
    f.tech(Color::Green, 6, Zone::Board(A, Color::Green), vec![]);
    f.tech(Color::Yellow, 3, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Purple, 8, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Green, 11, Zone::Board(B, Color::Green), vec![]);
    let drawn = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, card); // {4,6,8}: boxed 1 becomes 4, not 5 or 6.
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(events.len(), 1);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().regular_actions_started(), 1);
}

fn removal_fixture() -> (Game, CardId, CardId, CardId, [CardId; 2]) {
    let mut f = Fixture::default();
    let kepler = f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.tech(Color::Green, 7, Zone::Board(A, Color::Green), vec![]);
    let card = source(
        &mut f,
        vec![
            build::verb(Verb::Score, Selector::TopOf(Who::Executor, Color::Blue))
                .to(Destination::Score(Who::Executor)),
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            draw(1),
        ],
    );
    f.cards[usize::from(card.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![draw(2)],
    });
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let draws = [3, 4].map(|age| f.tech(Color::Yellow, age, Zone::Deck(Deck::base(age)), vec![]));
    (
        Game::new(f.position(), Expansions::FIGURES).unwrap(),
        card,
        kepler,
        bear,
        draws,
    )
}

#[test]
fn whole_action_adjustment_survives_source_removal_and_changed_values_and_suppresses_when() {
    let (mut game, card, kepler, bear, draws) = removal_fixture();
    dogma(&mut game, card);
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert!(!game.position().has_figure(A, Figure::JohannesKepler));
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[kepler]);
    assert_eq!(game.position().cards(Zone::Hand(A)), draws);
    assert_eq!(game.position().top(A, Color::Green), Some(bear));
    assert_eq!(game.position().top(A, Color::Red), Some(card));
    assert_eq!(events.len(), 4);
    assert!(events.iter().all(|e| moved(e).inside_karma));
    assert!(!game.position().age_zero_introduced()); // Bear's When never ran; no roster was supplied.
}

#[test]
fn replacement_shared_effects_earn_their_own_reward_without_triggering_opponents_archimedes() {
    let mut f = Fixture::default();
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::Archimedes, Zone::Board(B, Color::Blue));
    let card = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![draw(1)]); // Authority tie at zero.
    let a_draw = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let b_draw = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let untouched = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let reward = f.figure(Figure::Michelangelo, Zone::Deck(Deck::figures(4)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    dogma(&mut game, card);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[b_draw]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[a_draw, reward]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(3))),
        &[untouched]
    );
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|e| moved(e).inside_karma));
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(game.position().regular_actions_started(), 0);
}

#[test]
fn replacement_fades_both_players_nonactive_first_and_duke_cannot_replace_it() {
    let mut f = Fixture::default();
    let kepler = f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::DukeEllington, Zone::Board(A, Color::Purple));
    let other = f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let card = source(&mut f, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, card);
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, B);
    choose_card(&mut game, &mut events, other);
    assert_eq!(decision(&mut game, &mut events).player, A); // Duke is suppressed, not a no-op fade.
    choose_card(&mut game, &mut events, kepler);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| moved(e).inside_karma));
    assert_eq!(game.position().cards(Zone::Score(B)), &[other]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[kepler]);
    assert_eq!(game.position().top_figures(A).count(), 1);
    assert_eq!(game.position().top_figures(B).count(), 1);
}

#[test]
fn free_reward_uses_post_fading_top_value_and_remains_suppressed_after_kepler_is_gone() {
    let mut f = Fixture::default();
    let kepler = f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let card = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![draw(1)]);
    let a_draw = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let reward = f.tech(Color::Blue, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, card);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, kepler); // No Trebek search during the body draws.
    assert_eq!(events.len(), 2);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4); // Nor during the free reward draw after source removal.
    assert_eq!(moved(&events[2]).card, kepler);
    assert_eq!(moved(&events[3]).source, Zone::Deck(Deck::base(10)));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[a_draw, reward]);
    assert!(events.iter().all(|e| moved(e).inside_karma));
}

#[test]
fn achievement_victory_inside_replacement_action_end_does_not_trigger_jackie() {
    let mut f = Fixture::default();
    let kepler = f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
    for _ in 0..7 {
        f.tech(Color::Green, 1, Zone::Achievements(A), vec![]);
    }
    let card = source(&mut f, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    dogma(&mut game, card);
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(A));
    assert!(events.is_empty());
    assert_eq!(game.position().top(A, Color::Blue), Some(kepler));
    assert_eq!(game.position().turn_owner(), A); // Terminal child prevents the outer turn advance.
    assert!(game.position().cards(Zone::Score(B)).is_empty());
}

#[test]
fn exhaustion_in_a_modified_effect_stays_inside_the_replacement_scope() {
    let mut f = Fixture::default();
    let kepler = f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    f.figure(Figure::JackieChan, Zone::Board(B, Color::Red));
    f.tech(Color::Green, 1, Zone::Score(A), vec![]);
    let card = source(
        &mut f,
        vec![
            draw(11),
            build::verb(Verb::Junk, Selector::InPlay)
                .to(Destination::Junk)
                .all(),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, card);
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(A));
    assert!(events.is_empty());
    assert_eq!(game.position().top(A, Color::Blue), Some(kepler));
    assert!(game.position().cards(Zone::Junk).is_empty());
}

#[test]
fn kepler_does_not_change_agricultures_computed_age_or_ordinary_self_execution() {
    let mut f = Fixture::default();
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    let agriculture = f.agriculture(Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let scored = f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]);
    let untouched = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, agriculture);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(4))),
        &[untouched]
    );
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|e| moved(e).inside_karma));
    let mut f = Fixture::default();
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    let card = source(&mut f, vec![draw(1)]);
    let plain = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[plain]);
    assert!(!moved(&events[0]).inside_karma);
}

#[test]
fn replacement_retains_the_original_auspice_selection() {
    for auspice in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
        let card = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![draw(1)]);
        f.cards[usize::from(card.0)].featured = Some(Icon::Concept);
        f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        let b = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
        f.cards[usize::from(b.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
        for _ in 0..2 {
            f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
        }
        f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(Action::Dogma { card, auspice }, ActionKind::Regular)
            .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), if auspice { 1 } else { 3 });
        assert!(events.iter().all(|e| moved(e).inside_karma));
    }
}

#[test]
fn selecting_kepler_ignores_competing_homer_and_original_dogma_returns_replaced() {
    let mut f = Fixture::default();
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    let homer = f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let card = source(&mut f, vec![draw(1)]);
    let drawn = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Dogma {
            card,
            icon: Icon::Authority,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    let index = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Karma {
                    figure: Figure::JohannesKepler,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(index).unwrap();
    choose_card(&mut game, &mut events, homer); // Replacement's action-end fading.
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Replaced,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(events.len(), 2); // No Homer draw/tuck before the replacement.
}

#[test]
fn replacement_consumes_no_extra_ordinal_and_only_the_original_regular_action_advances_turn() {
    for kind in [ActionKind::Regular, ActionKind::Free] {
        let mut f = Fixture::default();
        f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
        let card = source(&mut f, vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_action(ActionKind::Regular).unwrap();
        game.take_action(
            Action::Dogma {
                card,
                auspice: false,
            },
            kind,
        )
        .unwrap();
        assert_eq!(
            game.position().regular_actions_started(),
            if kind == ActionKind::Regular { 2 } else { 1 }
        );
        assert_eq!(
            finish(&mut game, &mut Vec::new()),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position().turn_owner(),
            if kind == ActionKind::Regular { B } else { A }
        );
        assert_eq!(
            game.position().regular_actions_started(),
            if kind == ActionKind::Regular { 0 } else { 1 }
        );
    }
}

#[test]
fn cloning_every_replacement_boundary_retains_action_scope_after_source_removal() {
    let (mut game, card, _, _, _) = removal_fixture();
    game.begin_opening_turn(A).unwrap();
    dogma(&mut game, card);
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..200 {
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
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("replacement action did not terminate");
}
