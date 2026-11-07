use inno_engine::*;

mod common;
use common::*;

fn effect(game: &mut Game, card: CardId, index: u16) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}

fn decline(game: &mut Game, events: &mut Vec<Event>) {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Decline)
        .unwrap();
    game.choose(index).unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..30 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("Ecology did not finish");
}

#[test]
fn complete_printed_ecology_returns_scores_draws_twice_then_junks_the_remaining_deck() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let scored = f.tech(Color::Green, 3, Zone::Hand(A), vec![]);
    let tens: Vec<_> = (0..5)
        .map(|_| f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let figure = f.figure(Figure::JackieChan, Zone::Deck(Deck::figures(10)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: ecology,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    // A mandatory singleton score is automatic. The next choice commits the deck.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert_eq!(menu.options[1], Alternative::Decline);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, scored, tens[4], tens[3]]
    );
    assert_eq!(moved(&events[0]).verb, Some(Verb::Return));
    assert_eq!(moved(&events[1]).verb, Some(Verb::Score));
    game.choose(0).unwrap();
    // No later stop/order choice survives after commitment.
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [
            returned, scored, tens[4], tens[3], tens[2], tens[1], tens[0]
        ]
    );
    assert!(
        events[4..]
            .iter()
            .all(|e| moved(e).verb == Some(Verb::Junk))
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[tens[4], tens[3]]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[returned]
    );
    assert_eq!(
        game.position().cards(Zone::Junk),
        &[tens[2], tens[1], tens[0]]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(10))),
        &[figure]
    );
    assert!(game.position().cards(Zone::Deck(Deck::base(10))).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn decline_or_empty_hand_does_not_pay_score_or_draw() {
    for has_hand in [false, true] {
        let mut f = Fixture::default();
        let ecology = f.ecology(Zone::Board(A, Color::Yellow));
        let hand = has_hand.then(|| f.tech(Color::Red, 1, Zone::Hand(A), vec![]));
        let ten = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        effect(&mut game, ecology, 0);
        let mut events = Vec::new();
        if has_hand {
            decline(&mut game, &mut events);
        }
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(events.is_empty());
        assert_eq!(game.position().cards(Zone::Hand(A)), hand.as_slice());
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[ten]);
    }
}

#[test]
fn returning_the_last_hand_card_still_draws_twice_when_scoring_is_impossible() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let bottom = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let top = f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, ecology, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, top, bottom]
    );
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[top, bottom]);
}

#[test]
fn homer_replacing_the_return_does_not_earn_scoring_or_draws() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let returned = f.figure(Figure::ShenKuo, Zone::Hand(A));
    let retained = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let ten = f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, ecology, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Tuck));
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().top(A, Color::Green), Some(returned));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[retained]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[ten]);
}

#[test]
fn shen_replacing_the_score_does_not_cancel_either_draw() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    // No Purple stack: the replacement splay cannot itself change the board.
    let retained = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
    let bottom = f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]);
    let top = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, ecology, 0);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events[1],
        Event::Revealed {
            card: retained,
            source: Zone::Hand(A),
            executor: A,
            inside_karma: true
        }
    );
    assert_eq!(
        events[..1]
            .iter()
            .chain(&events[2..])
            .map(|e| moved(e).card)
            .collect::<Vec<_>>(),
        [returned, top, bottom]
    );
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[retained, top, bottom]
    );
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(game.position().splay(A, Color::Purple), Splay::None);
}

#[test]
fn declining_the_deck_preserves_its_exact_hidden_order() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let deck: Vec<_> = [Color::Red, Color::Green, Color::Purple]
        .into_iter()
        .map(|color| f.tech(color, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    f.placements
        .swap(usize::from(deck[0].0), usize::from(deck[2].0));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let before = game.position().clone();
    effect(&mut game, ecology, 1);
    let mut events = Vec::new();
    decline(&mut game, &mut events);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position(), &before);
    assert!(events.is_empty());
}

#[test]
fn empty_requested_base_deck_never_skips_to_eleven_or_figures() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let eleven = f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let figure = f.figure(Figure::JackieChan, Zone::Deck(Deck::figures(10)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, ecology, 1);
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(11))), &[eleven]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(10))),
        &[figure]
    );
    assert!(game.position().cards(Zone::Junk).is_empty());
}

#[test]
fn archimedes_changes_both_draws_and_the_junked_deck_to_eleven() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let ten = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let elevens: Vec<_> = (0..4)
        .map(|_| f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(ecology, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(
        matches!(menu.options[0], Alternative::Move { verb: Verb::Junk, target: TargetDescriptor::Hidden { source: Zone::Deck(deck), back: Deck { set: CardSet::Base, age: 11 }, .. } } if deck == Deck::base(11))
    );
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, elevens[3], elevens[2], elevens[1], elevens[0]]
    );
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[elevens[3], elevens[2]]
    );
    assert_eq!(game.position().cards(Zone::Junk), &[elevens[1], elevens[0]]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[ten]);
}

#[test]
fn keplers_whole_action_queries_twelve_without_exhaustion_or_falling_back() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::JohannesKepler, Zone::Board(A, Color::Blue));
    let retained = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let ten = f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]);
    let eleven = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    // Distinct top values {4,9} raise both printed boxes by two.
    game.take_action(
        Action::Dogma {
            card: ecology,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    decline(&mut game, &mut events);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)), &[retained]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(10))), &[ten]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(11))), &[eleven]);
    assert!(game.position().cards(Zone::Junk).is_empty());
}

#[test]
fn second_draw_exhaustion_is_terminal_before_the_deck_junk_effect() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let ten = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(ecology, Context::new(A)).unwrap();
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, returned);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, ten]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[ten]);
    assert!(game.position().cards(Zone::Junk).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn shared_payment_result_does_not_make_the_activator_score_or_draw_after_declining() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let opposing = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    f.cards[usize::from(opposing.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
    let paid = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let retained = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let tens: Vec<_> = (0..4)
        .map(|_| f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let reward = f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    game.take_action(
        Action::Dogma {
            card: ecology,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).player, B);
    choose_card(&mut game, &mut events, paid);
    assert_eq!(decision(&mut game, &mut events).player, A);
    decline(&mut game, &mut events);
    assert_eq!(decision(&mut game, &mut events).player, B);
    decline(&mut game, &mut events);
    assert_eq!(decision(&mut game, &mut events).player, A);
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[tens[3], tens[2]]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[retained, reward]);
    assert_eq!(game.position().cards(Zone::Junk), &[tens[1], tens[0]]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert!(game.position().cards(Zone::Score(B)).is_empty());
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [paid, tens[3], tens[2], tens[1], tens[0], reward]
    );
    assert!(events[..3].iter().all(|e| moved(e).executor == B));
    assert!(events[3..].iter().all(|e| moved(e).executor == A));
}

#[test]
fn hidden_permutations_have_the_same_commitment_menu_but_junk_physical_top_first() {
    let make = |reverse| {
        let mut f = Fixture::default();
        let ecology = f.ecology(Zone::Board(A, Color::Yellow));
        let deck: Vec<_> = [Color::Red, Color::Green, Color::Purple]
            .into_iter()
            .map(|color| f.tech(color, 10, Zone::Deck(Deck::base(10)), vec![]))
            .collect();
        if reverse {
            f.placements
                .swap(usize::from(deck[0].0), usize::from(deck[2].0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        effect(&mut game, ecology, 1);
        (game, deck)
    };
    let (mut first, deck) = make(false);
    let (mut second, _) = make(true);
    let mut events = Vec::new();
    let mut other = Vec::new();
    let menu = decision(&mut first, &mut events);
    assert_eq!(decision(&mut second, &mut other), menu);
    assert_eq!(menu.options.len(), 2);
    assert!(
        matches!(menu.options[0], Alternative::Move { verb: Verb::Junk, target: TargetDescriptor::Hidden { source: Zone::Deck(deck), back: Deck { set: CardSet::Base, age: 10 }, .. } } if deck == Deck::base(10))
    );
    assert_eq!(menu.options[1], Alternative::Decline);
    assert!(events.is_empty() && other.is_empty());
    first.choose(0).unwrap();
    second.choose(0).unwrap();
    assert_eq!(
        settle(&mut first, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        settle(&mut second, &mut other),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [deck[2], deck[1], deck[0]]
    );
    assert_eq!(
        other.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        deck
    );
    assert_eq!(
        first.position().cards(Zone::Junk),
        &[deck[2], deck[1], deck[0]]
    );
    assert_eq!(second.position().cards(Zone::Junk), deck);
    assert!(first.position().check_conservation());
    assert!(second.position().check_conservation());
}

#[test]
fn clones_at_every_boundary_preserve_payments_boxed_scopes_draws_and_deck_commitment() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    for _ in 0..5 {
        f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    }
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.self_execute(ecology, Context::new(A)).unwrap();
    let mut expected = game.clone();
    let mut all = Vec::new();
    let result = finish(&mut expected, &mut all);
    assert_eq!(result, Status::Complete(ReturnValue::Unit));
    assert_eq!(all.len(), 7);
    assert_eq!(expected.position().cards(Zone::Hand(A)).len(), 2);
    assert_eq!(expected.position().cards(Zone::Score(A)).len(), 1);
    assert_eq!(expected.position().cards(Zone::Junk).len(), 3);
    let mut prefix = Vec::new();
    for _ in 0..400 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
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
    panic!("Ecology did not finish");
}

#[test]
fn successful_return_leaves_a_genuine_mandatory_score_choice() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let retained = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let scored = f.tech(Color::Purple, 3, Zone::Hand(A), vec![]);
    let bottom = f.tech(Color::Red, 10, Zone::Deck(Deck::base(10)), vec![]);
    let top = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, ecology, 0);
    let mut events = Vec::new();
    let payment = decision(&mut game, &mut events);
    assert_eq!(payment.options.len(), 4);
    assert_eq!(payment.options.last(), Some(&Alternative::Decline));
    choose_card(&mut game, &mut events, returned);
    let score = decision(&mut game, &mut events);
    assert_eq!(score.options.len(), 2);
    assert!(score.options.iter().all(|o| matches!(
        o,
        Alternative::Move {
            verb: Verb::Score,
            ..
        }
    )));
    assert_eq!(events.len(), 1);
    choose_card(&mut game, &mut events, scored);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [returned, scored, top, bottom]
    );
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[retained, top, bottom]
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
}
