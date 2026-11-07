use inno_engine::*;
mod common;
use common::*;

#[test]
fn wheel_draws_two_cards_with_boxes_adjusted_but_not_the_quantity() {
    let mut f = Fixture::default();
    let wheel = f.add(base_card("B009").unwrap(), Zone::Board(A, Color::Green));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let bottom = f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]);
    let top = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let untouched = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: wheel,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [top, bottom]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[top, bottom]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        &[untouched]
    );
}

#[test]
fn mathematics_uses_the_successful_return_value_not_a_boxed_draw_literal() {
    let mut f = Fixture::default();
    let math = f.add(base_card("B023").unwrap(), Zone::Board(A, Color::Blue));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let payment = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let drawn = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let untouched = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(math, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, payment);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [payment, drawn, drawn]
    );
    assert_eq!(game.position().top(A, Color::Green), Some(drawn));
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(3))),
        &[untouched]
    );
}

#[test]
fn quantum_theory_keeps_every_optional_stop_and_scores_only_the_second_reward() {
    for count in 0..=2 {
        let mut f = Fixture::default();
        let quantum = f.add(base_card("B082").unwrap(), Zone::Board(A, Color::Blue));
        let payments = [
            f.tech(Color::Red, 1, Zone::Hand(A), vec![]),
            f.tech(Color::Green, 2, Zone::Hand(A), vec![]),
        ];
        let scored = f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
        let held = f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        game.self_execute(quantum, Context::new(A)).unwrap();
        let mut events = Vec::new();
        for &payment in &payments[..count] {
            choose_card(&mut game, &mut events, payment);
        }
        if count < 2 {
            let menu = decision(&mut game, &mut events);
            game.choose(
                menu.options
                    .iter()
                    .position(|o| *o == Alternative::Decline)
                    .unwrap(),
            )
            .unwrap();
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        let mut expected = payments[..count].to_vec();
        if count == 2 {
            expected.extend([held, scored, scored]);
            assert_eq!(game.position().cards(Zone::Hand(A)), &[held]);
            assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
        } else {
            assert_eq!(game.position().cards(Zone::Hand(A)), &payments[count..]);
            assert_eq!(
                game.position().cards(Zone::Deck(Deck::base(10))),
                &[scored, held]
            );
            assert!(game.position().cards(Zone::Score(A)).is_empty());
        }
        assert_eq!(
            events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
            expected
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn quantum_theory_requires_two_original_returns_not_two_attempts() {
    let mut f = Fixture::default();
    let quantum = f.add(base_card("B082").unwrap(), Zone::Board(A, Color::Blue));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let replaced = f.figure(Figure::Archimedes, Zone::Hand(A));
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let reward = f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(quantum, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, replaced);
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).verb).collect::<Vec<_>>(),
        [Some(Verb::Tuck), Some(Verb::Return)]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[reward]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn quantum_theory_second_draw_exhaustion_does_not_score_the_first_draw() {
    let mut f = Fixture::default();
    let quantum = f.add(base_card("B082").unwrap(), Zone::Board(A, Color::Blue));
    let one = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let two = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let last = f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.self_execute(quantum, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, one);
    choose_card(&mut game, &mut events, two);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [one, two, last]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[last]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}
