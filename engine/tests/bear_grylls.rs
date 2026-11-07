use inno_engine::*;

mod common;
use common::*;

fn zero(f: &mut Fixture, color: Color, zone: Zone) -> CardId {
    let card = f.tech(color, 0, zone, vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Food);
    card
}

fn pick(menu: &Decision, bear: CardId) -> usize {
    menu.options
        .iter()
        .position(|o| {
            matches!(o, Alternative::Move {
        target: TargetDescriptor::Known { card, .. }, ..
    } if *card == bear)
        })
        .unwrap_or(0)
}

fn finish(game: &mut Game, events: &mut Vec<Event>, bear: CardId) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(pick(&menu, bear)).unwrap(),
            status => return status,
        }
    }
    panic!("Bear Grylls did not finish");
}

#[test]
fn bear_junks_himself_and_the_caller_but_finishes_introduction_draw_and_parent_program() {
    let mut f = Fixture::default();
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
            build::draw(2),
        ],
    );
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let nightingale = f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    let michelangelo = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let scored = f.tech(Color::Blue, 5, Zone::Score(A), vec![]);
    let hidden = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let buried = f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    // The source remains executable even below another card and after junking.
    let achievement = f.tech(Color::Blue, 9, Zone::Achievements(A), vec![]);
    let available = f.tech(Color::Red, 8, Zone::AvailableAchievements, vec![]);
    let old_junk = f.tech(Color::Green, 3, Zone::Junk, vec![]);
    let deck = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let parent_draw = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut zero_order = [Color::Red, Color::Blue, Color::Green]
        .map(|c| zero(&mut f, c, Zone::AgeZeroReserve))
        .to_vec();
    // Frozen seed-42 outputs select j=1 for i=2, then j=1 for i=1.
    zero_order.swap(2, 1);
    let expected_draw = zero_order.pop().unwrap();
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.self_execute(caller, Context::new(A)).unwrap();
    let mut events = Vec::new();
    // The junk sweep goes to the unordered junk in one canonical order.
    assert_eq!(
        finish(&mut game, &mut events, bear),
        Status::Complete(ReturnValue::Unit)
    );
    let junked = [
        bear,
        caller,
        buried,
        nightingale,
        michelangelo,
        scored,
        hidden,
    ];
    for card in junked {
        assert!(game.position().cards(Zone::Junk).contains(&card));
        assert_eq!(events.iter().filter(|e| matches!(e, Event::Moved(m) if m.card == card && m.verb == Some(Verb::Junk) && m.inside_karma)).count(), 1);
    }
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[achievement]);
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        &[available]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(1))), &[deck]);
    assert!(game.position().cards(Zone::Junk).contains(&old_junk));
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[expected_draw, parent_draw]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(0))), zero_order);
    assert!(game.position().cards(Zone::AgeZeroReserve).is_empty());
    assert!(game.position().age_zero_introduced());
    assert_eq!(
        events[events.len() - 3],
        Event::AgeZeroIntroduced {
            executor: A,
            count: 3
        }
    );
    assert!(moved(&events[events.len() - 2]).inside_karma);
    assert!(!moved(events.last().unwrap()).inside_karma);
    for player in [A, B] {
        for color in Color::ALL {
            assert!(game.position().cards(Zone::Board(player, color)).is_empty());
        }
    }
    assert!(game.position().check_conservation());
    // Food is a real printed symbol on the introduced Base card.
    let color = game.position().card(expected_draw).color;
    game.start(
        meld(expected_draw, Zone::Hand(A), A, color),
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().basic_icon_count(A, Icon::Food), 1);
    assert!(!game.position().eligible_for_special(A, Special::Empire));
}

/// Secrets leave both Safes for the unordered junk with the same outcome in any order,
/// so the sweep asks for none.
#[test]
fn bear_junks_both_safes_without_asking_for_an_order() {
    let mut f = Fixture::default();
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor))],
    );
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let secrets = [
        f.tech(Color::Red, 2, Zone::Safe(A), vec![]),
        f.tech(Color::Blue, 2, Zone::Safe(A), vec![]),
        f.tech(Color::Green, 3, Zone::Safe(B), vec![]),
    ];
    let scored = f.tech(Color::Yellow, 4, Zone::Score(B), vec![]);
    zero(&mut f, Color::Red, Zone::AgeZeroReserve);
    let mut game = Game::seeded(
        f.position(),
        Expansions::FIGURES.union(Expansions::UNSEEN),
        42,
    )
    .unwrap();
    game.self_execute(caller, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    for card in secrets.into_iter().chain([bear, caller, scored]) {
        assert!(game.position().cards(Zone::Junk).contains(&card));
    }
    for player in [A, B] {
        assert!(game.position().cards(Zone::Safe(player)).is_empty());
    }
}

/// When/Would karmas cannot trigger while a When/Would karma resolves (pp28-29),
/// so a junk-reactive Figure that surfaces during Bear's sweep cannot react
/// and the sweep still asks for no order.
#[test]
fn a_buried_junk_karma_cannot_react_inside_bears_sweep() {
    let mut f = Fixture::default();
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor))],
    );
    f.figure(Figure::BearGrylls, Zone::Hand(A));
    // Junking the card above Rod Serling makes him a top card.
    let serling = f.figure(Figure::RodSerling, Zone::Board(B, Color::Purple));
    f.tech(Color::Purple, 5, Zone::Board(B, Color::Purple), vec![]);
    f.tech(Color::Red, 2, Zone::Safe(A), vec![]);
    zero(&mut f, Color::Red, Zone::AgeZeroReserve);
    let mut game = Game::seeded(
        f.position(),
        Expansions::FIGURES.union(Expansions::UNSEEN),
        42,
    )
    .unwrap();
    game.self_execute(caller, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    for card in [caller, serling] {
        assert!(game.position().cards(Zone::Junk).contains(&card));
    }
}

#[test]
fn dormant_cards_cannot_be_moved_or_revealed_and_base_rejects_age_zero_positions() {
    let mut f = Fixture::default();
    let card = zero(&mut f, Color::Red, Zone::AgeZeroReserve);
    let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let position = f.position();
    assert!(matches!(
        Game::new(position.clone(), Expansions::BASE),
        Err(Error::InvalidSetup(_))
    ));
    let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    let mut events = Vec::new();
    for request in [
        Operation::Reveal {
            target: target(card, Zone::AgeZeroReserve),
        },
        Operation::Move {
            verb: Verb::Junk,
            target: target(card, Zone::AgeZeroReserve),
            destination: Zone::Junk,
        },
    ] {
        game.start(request, Context::new(A)).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Inapplicable,
                card: None,
                value: None,
                short: false,
            }))
        );
        assert_eq!(game.position(), &position);
        assert!(events.is_empty());
    }
    game.start(
        Operation::Draw {
            value: 0,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[one]);
    assert_eq!(game.position().cards(Zone::AgeZeroReserve), &[card]);
    assert!(!game.position().age_zero_introduced());
}

#[test]
fn age_zero_introduction_is_idempotent_and_does_not_consume_rng_twice() {
    let mut f = Fixture::default();
    for color in Color::ALL {
        zero(&mut f, color, Zone::AgeZeroReserve);
    }
    for color in Color::ALL {
        f.tech(color, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 17).unwrap();
    let mut events = Vec::new();
    game.start(Operation::IntroduceAgeZero, Context::new(A))
        .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(
        events,
        [Event::AgeZeroIntroduced {
            executor: A,
            count: 5
        }]
    );
    let mut fork = game.clone();
    game.start(Operation::IntroduceAgeZero, Context::new(A))
        .unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    assert_eq!(game.position(), fork.position());
    assert_eq!(events.len(), 1);
    let shuffle = Operation::Shuffle {
        deck: Deck::base(1),
    };
    game.start(shuffle, Context::new(A)).unwrap();
    fork.start(shuffle, Context::new(A)).unwrap();
    settle(&mut game, &mut events);
    settle(&mut fork, &mut Vec::new());
    assert_eq!(game, fork);
}

#[test]
fn remelding_bear_neither_restores_consumed_age_zero_cards_nor_reshuffles_the_deck() {
    let mut f = Fixture::default();
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let last = zero(&mut f, Color::Red, Zone::Deck(Deck::base(0)));
    let first = zero(&mut f, Color::Blue, Zone::Deck(Deck::base(0)));
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 9).unwrap();
    let mut events = Vec::new();
    for drawn in [first, last] {
        game.start(meld(bear, Zone::Hand(A), A, Color::Green), Context::new(A))
            .unwrap();
        assert!(matches!(
            finish(&mut game, &mut events, bear),
            Status::Complete(_)
        ));
        assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
        game.start(
            Operation::Move {
                verb: Verb::Transfer,
                target: target(bear, Zone::Junk),
                destination: Zone::Hand(A),
            },
            Context::new(A),
        )
        .unwrap();
        settle(&mut game, &mut events);
    }
    assert!(game.position().cards(Zone::Junk).contains(&first));
    assert!(game.position().cards(Zone::Deck(Deck::base(0))).is_empty());
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::AgeZeroIntroduced { .. } | Event::Shuffled { .. }))
    );
    assert!(game.position().check_conservation());
}

#[test]
fn exhaustion_in_bears_when_unwinds_the_original_meld_and_its_parent_program() {
    let mut f = Fixture::default();
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    zero(&mut f, Color::Red, Zone::Junk); // Age 0 was already introduced and consumed.
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            Instruction::Win(WinRule::Executor), // must not run after the tied exhaustion draw
        ],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(caller, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events, bear),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(
        game.start(
            Operation::ProposeWin {
                winner: A,
                reason: WinReason::Effect
            },
            Context::new(A)
        ),
        Err(Error::Busy)
    );
    assert!(game.position().cards(Zone::Junk).contains(&bear));
    assert!(game.position().cards(Zone::Junk).contains(&caller));
    assert_eq!(events.len(), 3); // meld, junk Bear, junk caller; no resurrected deck
    assert!(game.position().check_conservation());
}

#[test]
fn hidden_junking_offers_no_menu_over_opponents_cards() {
    for reverse in [false, true] {
        let mut f = Fixture::default();
        let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
        let first = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
        let second = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
        zero(&mut f, Color::Yellow, Zone::AgeZeroReserve);
        if reverse {
            f.placements
                .swap(usize::from(first.0), usize::from(second.0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(meld(bear, Zone::Hand(A), A, Color::Green), Context::new(A))
            .unwrap();
        // Everything goes to the unordered junk: no choice reads a front.
        let status = settle(&mut game, &mut Vec::new());
        assert!(!matches!(status, Status::Decision(_)), "{status:?}");
        let junk = game.position().cards(Zone::Junk);
        assert!(junk.contains(&first) && junk.contains(&second));
    }
}

/// Bear Grylls junks every non-achievement card outside the decks, like
/// Fission: Forecasts, Safes and Artifacts displays too.
#[test]
fn bear_junks_forecasts_safes_and_displays() {
    let mut f = Fixture::default();
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let forecast = f.tech(Color::Red, 2, Zone::Forecast(B), vec![]);
    let safe = f.tech(Color::Blue, 3, Zone::Safe(A), vec![]);
    let display = f.tech(Color::Green, 4, Zone::Display(B), vec![]);
    zero(&mut f, Color::Yellow, Zone::AgeZeroReserve);
    let expansions = Expansions::FIGURES
        .union(Expansions::ECHOES)
        .union(Expansions::UNSEEN)
        .union(Expansions::ARTIFACTS);
    let mut game = Game::new(f.position(), expansions).unwrap();
    game.start(meld(bear, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    let status = finish(&mut game, &mut events, bear);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    let junk = game.position().cards(Zone::Junk);
    for card in [bear, forecast, safe, display] {
        assert!(junk.contains(&card), "{card:?}");
    }
}

#[test]
fn cloning_at_every_bear_boundary_preserves_global_progress_and_shuffled_draw() {
    let mut f = Fixture::default();
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    f.figure(Figure::Nightingale, Zone::Board(A, Color::Yellow));
    f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    for color in Color::ALL {
        zero(&mut f, color, Zone::AgeZeroReserve);
    }
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 93).unwrap();
    game.start(meld(bear, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events, bear);
    let mut events = Vec::new();
    for _ in 0..200 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix, bear), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => game.choose(pick(&menu, bear)).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("Bear resolution did not terminate");
}

#[test]
fn an_omitted_age_zero_roster_faults_instead_of_silently_skipping_bears_last_instructions() {
    let mut f = Fixture::default();
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(bear, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    let error = Error::Unsupported("Age 0 roster was not supplied");
    assert_eq!(game.advance(100, &mut events), Err(error.clone()));
    assert_eq!(events.len(), 2); // the completed meld/junk are not replayed
    assert_eq!(game.advance(100, &mut events), Err(error));
    assert_eq!(events.len(), 2);
    assert!(game.position().check_conservation());
}

#[test]
fn cards_cannot_be_moved_back_out_of_the_game_to_the_age_zero_reserve() {
    let mut f = Fixture::default();
    let card = zero(&mut f, Color::Red, Zone::Hand(A));
    let position = f.position();
    let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(card, Zone::Hand(A)),
            destination: Zone::AgeZeroReserve,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(100, &mut events),
        Err(Error::Unsupported("invalid movement destination"))
    );
    assert_eq!(game.position(), &position);
    assert!(game.position().age_zero_introduced());
    assert!(events.is_empty());
}

#[test]
fn mixed_dormant_and_introduced_age_zero_setup_is_rejected() {
    let mut f = Fixture::default();
    zero(&mut f, Color::Red, Zone::AgeZeroReserve);
    zero(&mut f, Color::Blue, Zone::Hand(A));
    assert_eq!(
        Position::new(f.cards, &f.placements, A),
        Err(Error::InvalidSetup(
            "mixed introduced and dormant Age 0 cards"
        ))
    );
}

#[test]
fn bear_sweeps_without_choices_past_claim_karmas_it_suppresses() {
    // With Cities a junk can claim a special achievement, and Zhang Heng
    // (on top) and Darwin (buried) react to claims. Inside Bear's When
    // karma no Would/When can fire, and Leeuwenhoek's claim cost (buried)
    // never applies to Cities' tokens, so the sweep still needs no order.
    let mut f = Fixture::default();
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor))],
    );
    let bear = f.figure(Figure::BearGrylls, Zone::Hand(A));
    let color = |figure: Figure| figure_card(figure.code()).unwrap().color;
    let zhang = color(Figure::ZhangHeng);
    f.figure(Figure::ZhangHeng, Zone::Board(B, zhang));
    let darwin = color(Figure::CharlesDarwin);
    f.figure(Figure::CharlesDarwin, Zone::Board(A, darwin));
    let covering = f.tech(darwin, 4, Zone::Board(A, darwin), vec![]);
    // A buried claim modifier: Cities' tokens have no cost it could change.
    let leeuwenhoek = color(Figure::AntonieVanLeeuwenhoek);
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(B, leeuwenhoek));
    f.tech(leeuwenhoek, 5, Zone::Board(B, leeuwenhoek), vec![]);
    let hands = [
        f.tech(Color::Blue, 2, Zone::Hand(B), vec![]),
        f.tech(Color::Yellow, 3, Zone::Hand(B), vec![]),
    ];
    let scores = [
        f.tech(Color::Red, 5, Zone::Score(A), vec![]),
        f.tech(Color::Blue, 6, Zone::Score(B), vec![]),
    ];
    for c in [Color::Red, Color::Blue, Color::Green] {
        zero(&mut f, c, Zone::AgeZeroReserve);
    }
    let mut game = Game::seeded(
        f.position(),
        Expansions::FIGURES.union(Expansions::CITIES),
        42,
    )
    .unwrap();
    game.self_execute(caller, Context::new(A)).unwrap();
    let mut events = Vec::new();
    match settle(&mut game, &mut events) {
        Status::Decision(menu) => panic!("no choice is needed: {menu:?}"),
        status => assert_eq!(status, Status::Complete(ReturnValue::Unit)),
    }
    let junk = game.position().cards(Zone::Junk);
    for card in [bear, caller, covering]
        .into_iter()
        .chain(hands)
        .chain(scores)
    {
        assert!(junk.contains(&card), "{card:?} was not junked");
    }
}
