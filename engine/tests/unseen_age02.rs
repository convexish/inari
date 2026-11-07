//! Unseen Age 2: one expected-outcome scenario per card.

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

/// A full Dogma of the demand by A against B, so the owner's effect reads
/// the demand's results.
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
fn padlock_takes_a_secret_or_lets_the_owner_score_three_distinct_values() {
    for has_secret in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U016", Zone::Board(A, Color::Red));
        let secret = has_secret.then(|| f.tech(Color::Blue, 3, Zone::Safe(B), vec![]));
        let one = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
        let two = f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
        let two_b = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        dogma(&mut game, card);
        let mut events = Vec::new();
        if has_secret {
            complete(run(&mut game, &mut events, &[]));
            assert_eq!(
                game.position().cards(Zone::AvailableAchievements),
                &[secret.unwrap()]
            );
            assert_eq!(game.position().cards(Zone::Score(A)).len(), 0);
        } else {
            // Subsets are picked in identity order: the 1, then a 2; the
            // other 2 then cannot join and the group ends by itself.
            let first = decision(&mut game, &mut events);
            assert_eq!(first.options.len(), 4);
            assert!(first.options.contains(&Alternative::Decline));
            game.choose(pick_known(&first, one).unwrap()).unwrap();
            let second = decision(&mut game, &mut events);
            assert!(second.options.contains(&Alternative::Decline));
            game.choose(pick_known(&second, two).unwrap()).unwrap();
            complete(run(&mut game, &mut events, &[]));
            let mut scored = game.position().cards(Zone::Score(A)).to_vec();
            scored.sort();
            assert_eq!(scored, vec![one, two]);
            assert_eq!(game.position().cards(Zone::Hand(A)), &[two_b]);
        }
    }
}

#[test]
fn password_scores_the_draw_after_a_safeguard_else_returns_the_rest_of_the_hand() {
    for safeguard in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U017", Zone::Board(A, Color::Red));
        let drawn = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
        let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        // Only the other blue card qualifies, plus the decline.
        assert!(pick_known(&menu, blue).is_some());
        assert!(pick_known(&menu, drawn).is_none() && pick_known(&menu, red).is_none());
        let choice = if safeguard {
            pick_known(&menu, blue).unwrap()
        } else {
            menu.options.len() - 1
        };
        game.choose(choice).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if safeguard {
            assert_eq!(position.cards(Zone::Safe(A)), &[blue]);
            assert_eq!(position.cards(Zone::Score(A)), &[drawn]);
            assert_eq!(position.cards(Zone::Hand(A)), &[red]);
        } else {
            assert_eq!(position.cards(Zone::Hand(A)), &[drawn]);
            assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&blue));
            assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&red));
        }
    }
}

#[test]
fn exile_returns_a_top_and_its_value_from_score_then_leaves_after_exactly_one() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U018", Zone::Board(A, Color::Yellow));
    let top = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
    let three = f.tech(Color::Green, 3, Zone::Score(B), vec![]);
    let four = f.tech(Color::Red, 4, Zone::Score(B), vec![]);
    let drawn = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    dogma(&mut game, card);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    // Two cards were returned by the demand: Exile stays, nothing drawn.
    assert!(position.cards(Zone::Deck(Deck::base(3))).contains(&top));
    assert!(position.cards(Zone::Deck(Deck::base(3))).contains(&three));
    assert_eq!(position.cards(Zone::Score(B)), &[four]);
    assert_eq!(position.top(A, Color::Yellow), Some(card));
    assert!(position.cards(Zone::Deck(Deck::base(3))).contains(&drawn));

    // Exactly one: Exile goes back to its deck and a [3] is drawn.
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U018", Zone::Board(A, Color::Yellow));
    f.tech(Color::Blue, 5, Zone::Board(B, Color::Blue), vec![]);
    let drawn = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    dogma(&mut game, card);
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert!(position.cards(Zone::Board(A, Color::Yellow)).is_empty());
    assert_eq!(position.cards(Zone::Hand(A)), &[drawn]);
}

#[test]
fn fingerprints_safeguards_and_transfers_by_the_splayed_color_count() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U019", Zone::Board(A, Color::Yellow));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let one = f.tech(Color::Blue, 1, Zone::AvailableAchievements, vec![]);
    let hand_one = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let hand_two = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    // Splay red left: one splayed color.
    let splay = decision(&mut game, &mut events);
    assert_eq!(splay.options.len(), 2);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
    effect(&mut game, card, 1, A, A);
    // Safeguard the 1, then transfer the hand 1 to the other board.
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 2);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[one]);
    assert_eq!(position.top(B, Color::Green), Some(hand_one));
    assert_eq!(position.cards(Zone::Hand(A)), &[hand_two]);
    // No hand card of value 1 is left: no board is asked.
    effect(&mut game, card, 1, A, A);
    complete(settle(&mut game, &mut events));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[hand_two]);
}

#[test]
fn cipher_draws_one_above_the_highest_of_two_or_more_returns() {
    for count in [1, 2] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U020", Zone::Board(A, Color::Green));
        f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
        if count == 2 {
            f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        }
        let four = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let hand = game.position().cards(Zone::Hand(A));
        if count == 2 {
            assert_eq!(hand, &[four]);
        } else {
            assert!(hand.is_empty());
        }
    }
}

#[test]
fn counterfeiting_scores_new_values_until_none_remain() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U021", Zone::Board(A, Color::Green));
    let red = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    let yellow = f.tech(Color::Yellow, 5, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Purple, 2, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[red, yellow]));
    let position = game.position();
    // After the red 3, the blue 3 no longer qualifies; Counterfeiting (2)
    // never did; the 5 scores on the second pass.
    assert_eq!(position.cards(Zone::Score(A)).len(), 3);
    assert_eq!(position.top(A, Color::Blue), Some(blue));
    assert_eq!(position.top(A, Color::Green), Some(card));
}

#[test]
fn astrology_melds_by_visible_purple_count_and_tucks_without_prosperity() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U022", Zone::Board(A, Color::Blue));
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Purple, 2, Zone::Board(A, Color::Purple), vec![]);
    let two = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let position = f
        .position()
        .with_splay(A, Color::Purple, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    // Two visible purple cards: a [2] is drawn, melded and (no Prosperity)
    // tucked, so it stays alone in its pile.
    assert_eq!(game.position().cards(Zone::Board(A, Color::Green)), &[two]);
    let moved: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => m.verb,
            _ => None,
        })
        .collect();
    assert_eq!(moved, vec![Verb::Meld, Verb::Tuck]);
}

#[test]
fn meteorology_branches_on_the_drawn_icons_and_claims_zen_without_authority() {
    for (health, prosperity, expect) in [
        (true, true, "score"),
        (false, true, "return"),
        (false, false, "tuck"),
    ] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U023", Zone::Board(A, Color::Blue));
        // The extras lie under the drawn card, which is the deck's top.
        let extra = [
            f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]),
            f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]),
        ];
        let drawn = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut shown = Vec::new();
        if health {
            shown.push(Icon::Health);
        }
        if prosperity {
            shown.push(Icon::Prosperity);
        }
        icons(&mut f, drawn, &shown);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        match expect {
            "score" => assert_eq!(position.cards(Zone::Score(A)), &[drawn]),
            "return" => {
                assert!(position.cards(Zone::Deck(Deck::base(3))).contains(&drawn));
                let mut hand = position.cards(Zone::Hand(A)).to_vec();
                hand.sort();
                assert_eq!(hand, extra.to_vec());
            }
            _ => assert_eq!(position.cards(Zone::Board(A, Color::Green)), &[drawn]),
        }
    }
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U023", Zone::Board(A, Color::Blue));
    f.tech(Color::Red, 6, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(
        game.position().special_location(Special::Zen),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn propaganda_melds_the_demanders_color_and_takes_the_card_beneath() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U024", Zone::Board(A, Color::Purple));
    let under = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let top = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let red = f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    let color = decision(&mut game, &mut events);
    assert_eq!(color.player, A);
    let index = color
        .options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << Color::Red as u8))
        .unwrap();
    game.choose(index).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(B, Color::Red), Some(red));
    // The card directly beneath the meld (the old top) moved, not the one
    // beneath that.
    assert_eq!(position.top(A, Color::Red), Some(top));
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), &[under, red]);
    assert_eq!(position.cards(Zone::Hand(B)), &[blue]);
}

#[test]
fn steganography_safeguards_by_the_splayed_colors_size_or_draws_and_tucks() {
    for splay in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U025", Zone::Board(A, Color::Purple));
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        let red = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        icons(&mut f, red, &[Icon::Concept]);
        let two = f.tech(Color::Blue, 2, Zone::AvailableAchievements, vec![]);
        let three = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        // The red top shows Concept: red is the one color offered.
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options,
            vec![
                Alternative::Colors(1 << Color::Red as u8),
                Alternative::Decline
            ]
        );
        game.choose(if splay { 0 } else { 1 }).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if splay {
            assert_eq!(position.splay(A, Color::Red), Splay::Left);
            assert_eq!(position.cards(Zone::Safe(A)), &[two]);
            assert!(position.cards(Zone::Board(A, Color::Green)).is_empty());
        } else {
            assert_eq!(position.cards(Zone::Board(A, Color::Green)), &[three]);
        }
    }
}
