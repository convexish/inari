//! The Unseen language hard cases run on the shared API: Heirloom,
//! Blackmail, Legend, Chartreuse, Iron Curtain and 3D Printing.

use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
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

/// Choose the preferred known card where offered, else the first option.
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

fn elevens(f: &mut Fixture, count: usize) {
    for i in 0..count {
        f.tech(Color::ALL[i % 5], 11, Zone::Deck(Deck::base(11)), vec![]);
    }
}

/// Heirloom: a secret goes to the supply and a card one higher is drawn. With no secret
/// the player draws a 0. Unless the drawn card is exactly one higher than the
/// transferred one, the player safeguards the achievement of the top red card's value.
#[test]
fn heirloom_transfers_a_secret_or_safeguards_by_top_red_value() {
    // (has a secret, a [4] to draw, safeguards)
    for (has_secret, four_in_deck, safeguards) in [
        (true, true, false),
        (false, true, true),
        (true, false, true),
    ] {
        let mut f = Fixture::default();
        let heirloom = unseen(&mut f, "U039", Zone::Board(A, Color::Yellow));
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        let secret = has_secret.then(|| f.tech(Color::Blue, 3, Zone::Safe(A), vec![]));
        // A drawn 0 skips the empty [1]-[3] decks.
        let drawn = if four_in_deck {
            f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![])
        } else {
            f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![])
        };
        let two = f.tech(Color::Purple, 2, Zone::AvailableAchievements, vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: heirloom,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            run(&mut game, &mut events, &[]),
            Status::Complete(_)
        ));
        let position = game.position();
        assert_eq!(position.cards(Zone::Hand(A)), &[drawn]);
        let mut supply = vec![two];
        supply.extend(secret);
        if safeguards {
            supply.retain(|&c| c != two);
            assert_eq!(position.cards(Zone::Safe(A)), &[two]);
        } else {
            assert!(position.cards(Zone::Safe(A)).is_empty());
        }
        assert_eq!(position.cards(Zone::AvailableAchievements), supply);
    }
}

/// Blackmail: the demander picks the meld from the revealed hand and the
/// card to self-execute among everything revealed; the victim runs it with
/// "may" as "must".
#[test]
fn blackmail_melds_and_executes_the_demanders_choices_under_must() {
    let mut f = Fixture::default();
    elevens(&mut f, 1);
    let blackmail = unseen(&mut f, "U040", Zone::Board(A, Color::Green));
    // The victim's red card: "you may return a card from your hand".
    let red = f.tech(
        Color::Red,
        1,
        Zone::Hand(B),
        vec![build::return_(build::HAND).optional()],
    );
    let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let scored = f.tech(Color::Yellow, 3, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    game.start(
        Operation::ExecuteEffect {
            card: blackmail,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    // The hand is revealed, then A picks the meld among known fronts.
    let meld = decision(&mut game, &mut events);
    assert_eq!(meld.player, A);
    assert_eq!(meld.options.len(), 2);
    game.choose(pick_known(&meld, red).unwrap()).unwrap();
    // The score pile is revealed; A picks among all three revealed cards.
    let execute = decision(&mut game, &mut events);
    assert_eq!(execute.player, A);
    for card in [red, blue, scored] {
        assert!(pick_known(&execute, card).is_some(), "{card:?}");
    }
    game.choose(pick_known(&execute, red).unwrap()).unwrap();
    // B self-executes the melded red card: its "may" return is forced and,
    // with one hand card, asks nothing.
    let status = run(&mut game, &mut events, &[]);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, vec![red, blue, scored]);
    let position = game.position();
    assert_eq!(position.top(B, Color::Red), Some(red));
    assert!(position.cards(Zone::Hand(B)).is_empty());
    assert_eq!(position.cards(Zone::Deck(Deck::base(2))), &[blue]);
    assert_eq!(position.cards(Zone::Score(B)), &[scored]);
}

/// Blackmail over Self Service: the victim's self-execution under the demand earns no
/// Chain (the demander is executing), but Self Service's own "self-execute" is the
/// victim's non-demand keyword and earns the victim Chain.
#[test]
fn blackmailed_self_service_earns_its_victim_chain_for_its_own_keyword() {
    let mut f = Fixture::default();
    elevens(&mut f, 2);
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    let blackmail = unseen(&mut f, "U040", Zone::Board(A, Color::Green));
    f.add(base_card("B101").unwrap(), Zone::Hand(B));
    f.add(base_card("B005").unwrap(), Zone::Board(B, Color::Yellow));
    f.tech(Color::Blue, 1, Zone::Achievements(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    game.start(
        Operation::ExecuteEffect {
            card: blackmail,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    let status = run(&mut game, &mut events, &[]);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    let position = game.position();
    let achieved = position.cards(Zone::Achievements(B));
    assert_eq!(achieved.len(), 1, "{achieved:?}");
    assert_eq!(position.card(achieved[0]).value, 11);
}

/// Legend: the chosen color is kept across repeats, each repeat
/// self-executes and scores the live top, and the loop ends once nine
/// points were scored due to Legend.
#[test]
fn legend_repeats_with_its_color_until_nine_points() {
    let mut f = Fixture::default();
    let legend = unseen(&mut f, "U045", Zone::Board(A, Color::Purple));
    let bottom = f.tech(Color::Red, 6, Zone::Board(A, Color::Red), vec![]);
    let middle = f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
    let top = f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: legend,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let color = decision(&mut game, &mut events);
    // Red has a top card; the three empty colours are one option.
    assert_eq!(color.options.len(), 2);
    let red = color
        .options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << Color::Red as u8))
        .unwrap();
    game.choose(red).unwrap();
    assert!(matches!(
        run(&mut game, &mut events, &[]),
        Status::Complete(_)
    ));
    let position = game.position();
    // 4 + 5 = 9: the 6 stays.
    assert_eq!(position.cards(Zone::Score(A)), &[top, middle]);
    assert_eq!(position.top(A, Color::Red), Some(bottom));
    assert!(position.check_conservation());
}

/// Legend: a card scored again counts again. The middle card melds the
/// scored 4 back, so Legend scores it three times: 4, 8, then 12 stops.
#[test]
fn legend_counts_a_card_scored_again() {
    let mut f = Fixture::default();
    let legend = unseen(&mut f, "U045", Zone::Board(A, Color::Purple));
    let bottom = f.tech(Color::Red, 6, Zone::Board(A, Color::Red), vec![]);
    let middle = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![program::build::meld(Selector::Score(Who::Executor))],
    );
    let top = f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: legend,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let color = decision(&mut game, &mut events);
    let red = color
        .options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << Color::Red as u8))
        .unwrap();
    game.choose(red).unwrap();
    assert!(matches!(
        run(&mut game, &mut events, &[]),
        Status::Complete(_)
    ));
    let position = game.position();
    let scores = events
        .iter()
        .filter(
            |e| matches!(e, Event::Moved(m) if m.card == top && m.destination == Zone::Score(A)),
        )
        .count();
    assert_eq!(scores, 3);
    assert_eq!(position.cards(Zone::Score(A)), &[top]);
    assert_eq!(position.top(A, Color::Red), Some(middle));
    assert_eq!(position.cards(Zone::Board(A, Color::Red))[0], bottom);
}

/// Chartreuse: four draws are revealed, the green and yellow ones meld,
/// the others return; the second effect may splay green or yellow right.
#[test]
fn chartreuse_melds_green_and_yellow_draws_and_returns_the_rest() {
    let mut f = Fixture::default();
    let chartreuse = unseen(&mut f, "U048", Zone::Board(A, Color::Yellow));
    let three = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let four = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let five = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let six = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: chartreuse,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        run(&mut game, &mut events, &[]),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.top(A, Color::Green), Some(three));
    assert_eq!(position.top(A, Color::Yellow), Some(five));
    assert_eq!(position.cards(Zone::Deck(Deck::base(4))), &[four]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(6))), &[six]);
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Revealed { .. }))
            .count(),
        4
    );
    // The optional splay offers the two-card yellow pile (Chartreuse and
    // the melded 5) and a decline; the single green card cannot splay.
    game.start(
        Operation::ExecuteEffect {
            card: chartreuse,
            effect: 1,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let _ = chartreuse;
    let splay = decision(&mut game, &mut events);
    assert_eq!(
        splay.options,
        vec![
            Alternative::Splay {
                player: A,
                color: Color::Yellow,
                direction: Splay::Right,
            },
            Alternative::Decline,
        ]
    );
}

/// Iron Curtain: each splayed color is unsplayed, then its top card returned
/// and a standard achievement safeguarded; unsplayed colors are untouched.
#[test]
fn iron_curtain_unsplays_then_returns_and_safeguards_per_color() {
    let mut f = Fixture::default();
    let iron = unseen(&mut f, "U086", Zone::Board(A, Color::Red));
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let blue_under = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let blue_top = f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    let green_under = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    let green_top = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    let one = f.tech(Color::Yellow, 1, Zone::AvailableAchievements, vec![]);
    let two = f.tech(Color::Purple, 2, Zone::AvailableAchievements, vec![]);
    let position = f
        .position()
        .with_splay(A, Color::Red, Splay::Up)
        .unwrap()
        .with_splay(A, Color::Blue, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: iron,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        run(&mut game, &mut events, &[]),
        Status::Complete(_)
    ));
    let position = game.position();
    for color in Color::ALL {
        assert_eq!(position.splay(A, color), Splay::None);
    }
    // Red's top (Iron Curtain itself) and blue's top returned.
    assert_eq!(position.cards(Zone::Deck(Deck::base(3))), &[blue_top]);
    assert_eq!(position.top(A, Color::Blue), Some(blue_under));
    assert_eq!(position.top(A, Color::Green), Some(green_top));
    assert_eq!(position.cards(Zone::Board(A, Color::Green)).len(), 2);
    let _ = green_under;
    assert!(position.cards(Zone::Board(A, Color::Red)).len() <= 1);
    let mut safe = position.cards(Zone::Safe(A)).to_vec();
    safe.sort();
    assert_eq!(safe, vec![one, two]);
}

/// 3D Printing: return a top or bottom card, achieve a secret of its value
/// regardless of eligibility, safeguard a standard achievement, and repeat
/// while both happen.
#[test]
fn three_d_printing_achieves_matching_secrets_and_repeats_while_both_happen() {
    let mut f = Fixture::default();
    let printing = unseen(&mut f, "U104", Zone::Board(A, Color::Purple));
    let five = f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
    let seven = f.tech(Color::Red, 7, Zone::Board(A, Color::Red), vec![]);
    let secret_seven = f.tech(Color::Blue, 7, Zone::Safe(A), vec![]);
    let secret_five = f.tech(Color::Green, 5, Zone::Safe(A), vec![]);
    let one = f.tech(Color::Yellow, 1, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: printing,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    // Return the red 7 first (a top), then the red 5 (top and bottom).
    assert!(matches!(
        run(&mut game, &mut events, &[seven, five]),
        Status::Complete(_)
    ));
    let position = game.position();
    assert_eq!(position.cards(Zone::Deck(Deck::base(7))), &[seven]);
    // The second pass achieved the 5 but found no standard achievement to
    // safeguard, so it did not repeat; the 5 was still returned.
    assert_eq!(position.cards(Zone::Deck(Deck::base(5))), &[five]);
    assert_eq!(
        position.cards(Zone::Achievements(A)),
        &[secret_seven, secret_five]
    );
    assert_eq!(position.cards(Zone::Safe(A)), &[one]);
    assert_eq!(position.top(A, Color::Purple), Some(printing));
}
