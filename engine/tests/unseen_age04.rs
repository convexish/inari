//! Unseen Age 4: one expected-outcome scenario per card (Heirloom, Blackmail
//! and Legend live in `unseen_hard_cases.rs`).

use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
}

fn icons(f: &mut Fixture, card: CardId, icons: &[Icon]) {
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
    }
}

fn effect(game: &mut Game, card: CardId, index: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
            | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
            if *c == card
        )
    })
}

fn run(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = preferred
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("did not terminate");
}

fn complete(status: Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

fn color_option(menu: &Decision, color: Color) -> usize {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << color as u8))
        .unwrap()
}

#[test]
fn ninja_returns_the_chosen_color_and_takes_that_top_or_proves_absence() {
    for present in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U036", Zone::Board(A, Color::Red));
        let top = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
        let hand = f.tech(
            if present { Color::Blue } else { Color::Green },
            1,
            Zone::Hand(B),
            vec![],
        );
        let mut game = Game::new(f.position(), Expansions::UNSEEN)
            .unwrap()
            .with_tracking()
            .unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        let color = decision(&mut game, &mut events);
        assert_eq!(color.player, A);
        assert_eq!(color.options.len(), 5);
        game.choose(color_option(&color, Color::Blue)).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if present {
            assert_eq!(position.top(A, Color::Blue), Some(top));
            assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&hand));
        } else {
            assert_eq!(position.top(B, Color::Blue), Some(top));
            // The hand is shown to prove it holds no blue card.
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Revealed { card, .. } if *card == hand))
            );
        }
    }
}

#[test]
fn spanish_inquisition_keeps_only_the_highest_and_returns_its_red_stack() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U037", Zone::Board(A, Color::Red));
    let high_a = f.tech(Color::Blue, 5, Zone::Hand(B), vec![]);
    let high_b = f.tech(Color::Green, 5, Zone::Hand(B), vec![]);
    let low = f.tech(Color::Yellow, 2, Zone::Hand(B), vec![]);
    let top_score = f.tech(Color::Purple, 4, Zone::Score(B), vec![]);
    let low_score = f.tech(Color::Red, 1, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    let mut hand = position.cards(Zone::Hand(B)).to_vec();
    hand.sort();
    assert_eq!(hand, vec![high_a, high_b]);
    assert_eq!(position.cards(Zone::Score(B)), &[top_score]);
    assert!(position.cards(Zone::Deck(Deck::base(2))).contains(&low));
    assert!(
        position
            .cards(Zone::Deck(Deck::base(1)))
            .contains(&low_score)
    );
    // Second effect: Spanish Inquisition is red and on top, so its whole
    // stack returns; covered, nothing happens.
    for on_top in [true, false] {
        let mut f = Fixture::default();
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        let card = unseen(&mut f, "U037", Zone::Board(A, Color::Red));
        if !on_top {
            f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 1, A, A);
        complete(run(&mut game, &mut events, &[]));
        assert_eq!(
            game.position().cards(Zone::Board(A, Color::Red)).is_empty(),
            on_top
        );
    }
}

#[test]
fn april_fools_day_gives_a_card_or_claims_folklore_and_swaps_splays() {
    for has_card in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U038", Zone::Board(A, Color::Yellow));
        let given = has_card.then(|| f.tech(Color::Blue, 3, Zone::Score(A), vec![]));
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if let Some(given) = given {
            assert_eq!(position.top(B, Color::Blue), Some(given));
            assert_eq!(
                position.special_location(Special::Folklore),
                SpecialLocation::Available
            );
        } else {
            assert_eq!(
                position.special_location(Special::Folklore),
                SpecialLocation::Owned(A)
            );
        }
    }
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U038", Zone::Board(A, Color::Yellow));
    f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Purple, 2, Zone::Board(A, Color::Purple), vec![]);
    let position = f
        .position()
        .with_splay(A, Color::Purple, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 2);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::Right);
    assert_eq!(game.position().splay(A, Color::Purple), Splay::None);
}

#[test]
fn el_dorado_scores_the_five_deck_for_three_prosperity_and_splays_for_two() {
    for prosperous in [3, 2, 1] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U041", Zone::Board(A, Color::Green));
        f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        let drawn = [
            f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]),
            f.tech(Color::Yellow, 2, Zone::Deck(Deck::base(2)), vec![]),
            f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]),
        ];
        for &card in drawn.iter().take(prosperous) {
            icons(&mut f, card, &[Icon::Prosperity]);
        }
        let five = f.tech(Color::Red, 5, Zone::Deck(Deck::base(5)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        for card in drawn {
            assert_eq!(position.top(A, position.card(card).color), Some(card));
        }
        assert_eq!(
            position.cards(Zone::Score(A)).contains(&five),
            prosperous == 3,
            "{prosperous}"
        );
        let splayed = position.splay(A, Color::Green) == Splay::Right
            && position.splay(A, Color::Blue) == Splay::Right;
        assert_eq!(splayed, prosperous >= 2, "{prosperous}");
    }
}

#[test]
fn quackery_offers_score_or_draw_and_draws_by_returned_icons() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U042", Zone::Board(A, Color::Blue));
    let hand = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let four = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 2);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[hand, four]);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U042", Zone::Board(A, Color::Blue));
    let first = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    icons(&mut f, first, &[Icon::Health, Icon::Health]);
    let second = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    icons(&mut f, second, &[Icon::Concept, Icon::Authority]);
    let three = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    // Two Health and one Concept: a [3].
    assert_eq!(game.position().cards(Zone::Hand(A)), &[three]);
}

#[test]
fn prophecies_reveals_a_reference_secret_and_melds_another_before_safeguarding() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U043", Zone::Board(A, Color::Blue));
    let reference = f.tech(Color::Yellow, 3, Zone::Safe(A), vec![]);
    let other = f.tech(Color::Green, 2, Zone::Safe(A), vec![]);
    let four = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    game.choose(1).unwrap();
    // Choose the slot of the yellow 3 (a Base 3 back) as the reference.
    let secrets = decision(&mut game, &mut events);
    let slot = secrets
        .options
        .iter()
        .position(|option| {
            matches!(
                option,
                Alternative::Reveal {
                    target: TargetDescriptor::Hidden { slot: 0, .. }
                }
            )
        })
        .unwrap();
    game.choose(slot).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let _ = branch;
    let position = game.position();
    // The drawn red 4 was revealed, the other secret melded, the 4 safeguarded.
    assert_eq!(position.top(A, Color::Green), Some(other));
    assert_eq!(position.cards(Zone::Safe(A)), &[reference, four]);
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, vec![reference, four]);
    for viewer in Player::ALL {
        game.determinize(viewer, 2).unwrap();
    }
}

/// With no secret, the second choice draws and reveals a 0 (as Heirloom).
#[test]
fn prophecies_without_a_secret_draws_and_reveals_a_zero() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U043", Zone::Board(A, Color::Blue));
    // A drawn 0 skips the empty [1] deck.
    let two = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[two]);
    assert!(game.position().cards(Zone::Safe(A)).is_empty());
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Revealed { card, .. } if *card == two))
    );
}

#[test]
fn confession_returns_one_authority_top_per_color_or_melds_and_scores() {
    for authority in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U044", Zone::Board(A, Color::Purple));
        let under = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        icons(&mut f, under, &[Icon::Authority]);
        let top = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        if authority {
            icons(&mut f, top, &[Icon::Authority]);
        }
        let scored = f.tech(Color::Blue, 3, Zone::Score(A), vec![]);
        let four = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if authority {
            // Only the top at the time returns, not the newly exposed one.
            assert_eq!(position.top(A, Color::Red), Some(under));
            assert_eq!(position.cards(Zone::Score(A)), &[scored]);
        } else {
            assert_eq!(position.top(A, Color::Blue), Some(scored));
            assert_eq!(position.cards(Zone::Score(A)), &[four]);
        }
    }
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U044", Zone::Board(A, Color::Purple));
    f.tech(Color::Blue, 4, Zone::Score(A), vec![]);
    f.tech(Color::Red, 4, Zone::Score(A), vec![]);
    f.tech(Color::Green, 3, Zone::Score(A), vec![]);
    let fours = [
        f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]),
        f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]),
        f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[fours[2], fours[1]]);
}
