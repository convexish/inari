use inno_engine::*;

mod common;
use common::*;

/// Place a printed card on its own color's pile.
fn on_board(f: &mut Fixture, card: Card, player: Player) -> CardId {
    let color = card.color;
    f.add(card, Zone::Board(player, color))
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

/// Play to the end, preferring these cards and otherwise the first option.
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

fn revealed(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

fn scored(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Score)))
        .count()
}

fn icon(f: &mut Fixture, card: CardId, icon: Icon) {
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(icon);
}

#[test]
fn machinery_proves_its_authority_score_and_its_failure() {
    // Success: the scored card is shown; the other stays private.
    let mut f = Fixture::default();
    let card = on_board(&mut f, base_card("B028").unwrap(), A);
    let crown = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    icon(&mut f, crown, Icon::Authority);
    let plain = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[crown]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[crown]);
    assert_eq!(revealed(&events), [crown]);
    assert!(!revealed(&events).contains(&plain));

    // Failure: the hand proves there is no Authority card.
    let mut f = Fixture::default();
    let card = on_board(&mut f, base_card("B028").unwrap(), A);
    let one = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let two = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(revealed(&events), [one, two]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn feudalism_reveals_the_victims_hand_when_nothing_qualifies() {
    let mut f = Fixture::default();
    let card = on_board(&mut f, base_card("B035").unwrap(), A);
    let one = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let two = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(revealed(&events), [one, two]);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[one, two]);
}

#[test]
fn clothing_proves_an_impossible_meld_unless_the_board_already_does() {
    // Hand colors all on the board: the hand is the proof.
    let mut f = Fixture::default();
    let card = on_board(&mut f, base_card("B007").unwrap(), A);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let green = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(revealed(&events), [red, green]);

    // All five colors on the board prove it publicly: nothing is revealed.
    let mut f = Fixture::default();
    let card = on_board(&mut f, base_card("B007").unwrap(), A);
    for color in [Color::Red, Color::Blue, Color::Yellow, Color::Purple] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
    }
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert!(revealed(&events).is_empty());
}

#[test]
fn ninja_and_triad_reveal_the_card_whose_color_decides_the_rest() {
    // Ninja: the demanded color's card is shown as it is returned.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U036").unwrap(), A);
    let red = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert!(revealed(&events).contains(&red));

    // Triad: the returned card's color picks the splay.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U065").unwrap(), A);
    let returned = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[returned]);
    assert!(revealed(&events).contains(&returned));
}

#[test]
fn quackery_returns_nothing_without_two_cards() {
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U042").unwrap(), A);
    let only = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[only]);
}

#[test]
fn myth_and_handshake_prove_what_stayed_in_hand() {
    // Myth: no same-color pair among two cards shows the hand.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U013").unwrap(), A);
    let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(revealed(&events), [red, blue]);

    // Myth with one card proves nothing.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U013").unwrap(), A);
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert!(revealed(&events).is_empty());

    // Handshake: the cards left in the victim's hand are shown.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U004").unwrap(), A);
    f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let kept = f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert!(game.position().cards(Zone::Hand(B)).contains(&kept));
    assert!(revealed(&events).contains(&kept));
}

#[test]
fn value_only_and_board_proven_failures_reveal_nothing() {
    // Sunglasses' echo with no splayed color: the board is the proof.
    let mut f = Fixture::default();
    let card = on_board(&mut f, echoes_card("E035").unwrap(), A);
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
    effect(&mut game, card, Card::ECHO, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert!(revealed(&events).is_empty());

    // Velcro Shoes: card backs prove there is no [9].
    let mut f = Fixture::default();
    let card = on_board(&mut f, artifacts_card("A087").unwrap(), A);
    f.tech(Color::Green, 8, Zone::Hand(B), vec![]);
    f.tech(Color::Yellow, 7, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    let status = run(&mut game, &mut events, &[]);
    assert_eq!(status, Status::Winner(A));
    assert!(revealed(&events).is_empty());
}

#[test]
fn colt_paterson_reveals_the_drawn_seven() {
    let mut f = Fixture::default();
    let card = on_board(&mut f, artifacts_card("A068").unwrap(), A);
    f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let seven = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert!(revealed(&events).contains(&seven));
}

#[test]
fn draws_of_a_missing_value_draw_a_zero() {
    // Popular Science with no top green card, Attic with an empty score pile
    // and Karaoke's echo with an empty hand each draw a [1] (no Age 0).
    for (code, index, set) in [
        ("U052", 0, Expansions::UNSEEN),
        ("U058", 2, Expansions::UNSEEN),
        ("E094", Card::ECHO, Expansions::ECHOES),
    ] {
        let mut f = Fixture::default();
        let printed = if code.starts_with('U') {
            unseen_card(code).unwrap()
        } else {
            echoes_card(code).unwrap()
        };
        let card = on_board(&mut f, printed, A);
        let one = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), set).unwrap();
        effect(&mut game, card, index, A, A);
        let mut events = Vec::new();
        run(&mut game, &mut events, &[]);
        assert!(
            !game
                .position()
                .cards(Zone::Deck(Deck::base(1)))
                .contains(&one),
            "{code}"
        );
    }
}

#[test]
fn top_card_of_each_color_moves_one_card_per_color() {
    // Metaverse: three splayed piles score three tops and survive, even
    // when lower cards lie beneath.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U115").unwrap(), A);
    for v in [1, 3, 5] {
        f.tech(Color::Red, v, Zone::Board(A, Color::Red), vec![]);
    }
    for v in [2, 4] {
        f.tech(Color::Yellow, v, Zone::Board(A, Color::Yellow), vec![]);
    }
    for v in [2, 6] {
        f.tech(Color::Blue, v, Zone::Board(A, Color::Blue), vec![]);
    }
    let mut position = f.position();
    for color in [Color::Red, Color::Yellow, Color::Blue] {
        position = position.with_splay(A, color, Splay::Up).unwrap();
    }
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let status = run(&mut game, &mut events, &[]);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    assert_eq!(scored(&events), 3);

    // Hunt-Lenox Globe: a higher card beneath is not returned too.
    let mut f = Fixture::default();
    let card = on_board(&mut f, artifacts_card("A041").unwrap(), A);
    let r5 = f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Yellow, 4, Zone::Board(A, Color::Yellow), vec![]);
    for _ in 0..4 {
        f.tech(Color::Purple, 5, Zone::Deck(Deck::base(5)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    effect(&mut game, card, 0, A, A);
    run(&mut game, &mut Vec::new(), &[]);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), &[r5]);
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
}

#[test]
fn iron_curtain_unsplays_every_color_before_safeguarding() {
    // Two secrets, red and blue splayed up (capacity 2). Unsplaying both
    // first restores capacity 5, so both safeguards succeed.
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U086").unwrap(), A);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    for v in [1, 2] {
        f.tech(Color::Blue, v, Zone::Board(A, Color::Blue), vec![]);
    }
    for v in [1, 2] {
        f.tech(Color::Green, v, Zone::Safe(A), vec![]);
    }
    for v in [3, 4] {
        f.tech(Color::Yellow, v, Zone::AvailableAchievements, vec![]);
    }
    f.tech(Color::Purple, 9, Zone::Deck(Deck::base(9)), vec![]);
    f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut position = f.position();
    for color in [Color::Red, Color::Blue] {
        position = position.with_splay(A, color, Splay::Up).unwrap();
    }
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(game.position().cards(Zone::Safe(A)).len(), 4);
}

#[test]
fn red_herring_treats_unsplayed_as_no_direction() {
    let mut f = Fixture::default();
    let card = on_board(&mut f, unseen_card("U056").unwrap(), A);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    run(&mut game, &mut Vec::new(), &[]);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Blue)).len(), 2);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::None);
}

#[test]
fn along_the_river_stops_after_a_yellow_tuck() {
    let mut f = Fixture::default();
    let card = on_board(&mut f, artifacts_card("A029").unwrap(), A);
    let red = f.tech(Color::Red, 4, Zone::Deck(Deck::base(4)), vec![]);
    let yellow = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS).unwrap();
    effect(&mut game, card, 0, A, A);
    run(&mut game, &mut Vec::new(), &[]);
    // The yellow (deck top) is tucked; the red below is never drawn.
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Yellow)),
        &[yellow, card]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), &[red]);
}

#[test]
fn vermeer_scores_a_color_present_on_the_board() {
    let mut f = Fixture::default();
    let vermeer = figure_card(Figure::JohannesVermeer.code()).unwrap().color;
    f.figure(Figure::JohannesVermeer, Zone::Board(A, vermeer));
    let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let melded = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(melded, Zone::Hand(A), A, Color::Blue), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    // Only red and the Figure's own color are on the board.
    assert_eq!(menu.options.len(), 2, "{:?}", menu.options);
    run(&mut game, &mut events, &[]);
    assert!(scored(&events) >= 1);
    let _ = red;
}

#[test]
fn glory_reveals_a_city_junked_from_hand() {
    let mut f = Fixture::default();
    let junker = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![build::verb(Verb::Junk, build::HAND).to(Destination::Junk)],
    );
    let city = f.add(city_card("C001").unwrap(), Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::CITIES).unwrap();
    effect(&mut game, junker, 0, A, A);
    let mut events = Vec::new();
    run(&mut game, &mut events, &[]);
    assert_eq!(game.position().cards(Zone::Junk), &[city]);
    assert!(revealed(&events).contains(&city));
}

#[test]
fn dubai_draws_one_city_plus_eleven() {
    let card = city_card("C099").unwrap();
    let pluses = card
        .symbols
        .iter()
        .filter(|s| **s == Symbol::City(CityIcon::Plus))
        .count();
    assert_eq!(pluses, 1);
    assert_eq!(card.symbols[2], Symbol::Icon(Icon::Efficiency));
}

/// Under Blackmail's "must", Placebo repeats at least once, then may stop.
#[test]
fn placebo_repeats_at_least_once_under_blackmail_then_may_stop() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let placebo = f.add(unseen_card("U062").unwrap(), Zone::Hand(B));
    f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let red_top = f.tech(Color::Red, 3, Zone::Board(B, Color::Red), vec![]);
    for _ in 0..3 {
        f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    let mut events = Vec::new();
    let mut offered_stop = false;
    for _ in 0..100 {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                let index = if menu.options.contains(&Alternative::Branch(1)) {
                    offered_stop = true;
                    menu.options
                        .iter()
                        .position(|o| *o == Alternative::Branch(1))
                        .unwrap()
                } else {
                    [red_top, placebo]
                        .iter()
                        .find_map(|&card| pick_known(&menu, card))
                        .unwrap_or(0)
                };
                game.choose(index).unwrap();
            }
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                break;
            }
        }
    }
    assert!(offered_stop);
    // The first return, one forced repeat, then a stop.
    assert_eq!(game.position().cards(Zone::Board(B, Color::Red)).len(), 1);
}

/// A blackmailed Sunglasses chooses its alternative, and one that splays nothing is a
/// legal choice.
#[test]
fn blackmailed_sunglasses_may_choose_the_alternative_that_splays_nothing() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    f.add(echoes_card("E035").unwrap(), Zone::Hand(B));
    f.tech(Color::Purple, 1, Zone::Board(B, Color::Purple), vec![]);
    f.tech(Color::Purple, 2, Zone::Board(B, Color::Purple), vec![]);
    f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
    let position = f
        .position()
        .with_splay(B, Color::Purple, Splay::Left)
        .unwrap();
    let both = Expansions::UNSEEN.union(Expansions::ECHOES);
    let mut game = Game::new(position, both).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    let mut events = Vec::new();
    let mut offered = false;
    for _ in 0..20 {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                // No other color is splayed, so "splay purple like another
                // color" splays nothing; it is still offered and chosen.
                let index = match menu
                    .options
                    .iter()
                    .position(|o| *o == Alternative::Branch(0))
                {
                    Some(i) if menu.options.contains(&Alternative::Branch(1)) => {
                        offered = true;
                        i
                    }
                    _ => 0,
                };
                game.choose(index).unwrap();
            }
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                break;
            }
        }
    }
    assert!(offered);
    assert_eq!(game.position().splay(B, Color::Red), Splay::None);
}

/// A blackmailed Red Envelope with exactly two cards in hand must score both.
#[test]
fn red_envelope_scores_two_under_blackmail_with_two_in_hand() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let envelope = f.add(unseen_card("U027").unwrap(), Zone::Hand(B));
    f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    let mut events = Vec::new();
    for _ in 0..100 {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                assert!(!menu.options.contains(&Alternative::Branch(1)));
                game.choose(pick_known(&menu, envelope).unwrap_or(0))
                    .unwrap();
            }
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                break;
            }
        }
    }
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 2);
}

/// Natural reading: a blackmailed "for each color, you may tuck" tucks one
/// card of every color in hand.
#[test]
fn freemasons_tucks_one_of_each_color_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let freemasons = f.add(unseen_card("U028").unwrap(), Zone::Hand(B));
    let red = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let blue = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    let mut events = Vec::new();
    for _ in 0..100 {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                let index = [freemasons, red]
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                break;
            }
        }
    }
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), &[red]);
    assert_eq!(position.cards(Zone::Board(B, Color::Blue)), &[blue]);
    assert_eq!(position.cards(Zone::Hand(B)).len(), 1);
}

/// The same ruling for Currency: a blackmailed "may return any number"
/// returns at least one card.
#[test]
fn currency_returns_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let currency = f.add(base_card("B020").unwrap(), Zone::Hand(B));
    f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    for _ in 0..3 {
        f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    let mut events = Vec::new();
    let mut first_return = true;
    for _ in 0..100 {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                let decline = menu.options.iter().position(|o| *o == Alternative::Decline);
                let returning = menu.options.iter().any(|o| {
                    matches!(
                        o,
                        Alternative::Move {
                            verb: Verb::Return,
                            ..
                        }
                    )
                });
                let index = if returning {
                    if first_return {
                        assert_eq!(decline, None, "the first return is required");
                        first_return = false;
                        0
                    } else {
                        decline.expect("stopping after one remains")
                    }
                } else {
                    pick_known(&menu, currency).unwrap_or(0)
                };
                game.choose(index).unwrap();
            }
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                break;
            }
        }
    }
    assert!(!first_return);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
}

/// Play a blackmailed card's effects for B: a menu moving cards with `verb`
/// must offer no decline while `done` is false, then stopping is taken.
fn blackmailed_moves_at_least_one(
    game: &mut Game,
    card: CardId,
    verb: Verb,
    color: Option<Color>,
    done: impl Fn(&Position) -> bool,
) {
    let mut events = Vec::new();
    let mut moved = false;
    for _ in 0..100 {
        match settle(game, &mut events) {
            Status::Decision(menu) => {
                let decline = menu.options.iter().position(|o| *o == Alternative::Decline);
                let moving = menu
                    .options
                    .iter()
                    .position(|o| matches!(o, Alternative::Move { verb: v, .. } if *v == verb));
                let chosen = color.and_then(|color| {
                    let bit = Alternative::Colors(1 << color as u8);
                    menu.options.iter().position(|o| *o == bit)
                });
                let index = match (moving, chosen) {
                    (Some(index), _) if !done(game.position()) => {
                        assert_eq!(decline, None, "the first {verb:?} is required");
                        index
                    }
                    (Some(_), _) => {
                        moved = true;
                        decline.expect("stopping after one remains")
                    }
                    (None, Some(index)) => index,
                    (None, None) => pick_known(&menu, card).unwrap_or(0),
                };
                game.choose(index).unwrap();
            }
            _ => break,
        }
    }
    assert!(moved, "a second {verb:?} was offered and declined");
}

/// The same ruling for Reformation: a blackmailed "may tuck a card for every
/// splayed color" tucks at least one card, then may stop.
#[test]
fn reformation_tucks_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let reformation = f.add(base_card("B045").unwrap(), Zone::Hand(B));
    f.tech(Color::Green, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Yellow, 1, Zone::Hand(B), vec![]);
    for color in [Color::Red, Color::Blue] {
        f.tech(color, 1, Zone::Board(B, color), vec![]);
        f.tech(color, 2, Zone::Board(B, color), vec![]);
    }
    let mut position = f.position();
    for color in [Color::Red, Color::Blue] {
        position = position.with_splay(B, color, Splay::Left).unwrap();
    }
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    blackmailed_moves_at_least_one(&mut game, reformation, Verb::Tuck, None, |position| {
        position.cards(Zone::Hand(B)).len() < 2
    });
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
}

/// The same ruling for Puzzle Cube: a blackmailed "may score the bottom
/// card or two bottom cards of one color" scores at least one card.
#[test]
fn puzzle_cube_scores_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let cube = f.add(echoes_card("E104").unwrap(), Zone::Hand(B));
    f.tech(Color::Purple, 1, Zone::Board(B, Color::Purple), vec![]);
    f.tech(Color::Purple, 2, Zone::Board(B, Color::Purple), vec![]);
    f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    // Purple stays splayed, so its visible count never matches red's single
    // card and Puzzle Cube's win check fails.
    let position = f
        .position()
        .with_splay(B, Color::Purple, Splay::Left)
        .unwrap();
    let both = Expansions::UNSEEN.union(Expansions::ECHOES);
    let mut game = Game::new(position, both).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    blackmailed_moves_at_least_one(
        &mut game,
        cube,
        Verb::Score,
        Some(Color::Purple),
        |position| !position.cards(Zone::Score(B)).is_empty(),
    );
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 1);
}

/// Play to the end, preferring these cards and otherwise accepting.
fn accept(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = preferred
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .or_else(|| menu.options.iter().position(|o| *o != Alternative::Decline))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                return;
            }
        }
    }
    panic!("no end within 100 decisions");
}

fn melds(events: &[Event], card: CardId) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::Moved(m) if m.card == card && m.verb == Some(Verb::Meld)))
        .count()
}

/// "All cards" move one card at a time while any remain, each considered once. A card
/// Nikola Tesla scores during Translation's meld-all joins it and is melded.
#[test]
fn translation_melds_a_card_scored_while_it_melds_all() {
    let mut f = Fixture::default();
    let translation = on_board(&mut f, base_card("B033").unwrap(), A);
    on_board(&mut f, figure_card(Figure::NikolaTesla.code()).unwrap(), A);
    let scored = f.tech(Color::Yellow, 5, Zone::Score(A), vec![]);
    let taken = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, translation, 0, A, A);
    let mut events = Vec::new();
    accept(&mut game, &mut events, &[]);
    let position = game.position();
    assert_eq!(position.top(A, Color::Yellow), Some(scored));
    assert_eq!(position.top(A, Color::Green), Some(taken));
    assert!(position.cards(Zone::Score(A)).is_empty());
    assert_eq!(melds(&events, taken), 1);
}

/// The same rule's other half: a card already melded that comes back to the
/// score pile is not melded again. Napoleon melds instead, then scores the
/// melded 5 straight back; Translation stops rather than looping.
#[test]
fn translation_melds_each_card_once_when_one_is_scored_back() {
    let mut f = Fixture::default();
    let translation = on_board(&mut f, base_card("B033").unwrap(), A);
    on_board(
        &mut f,
        figure_card(Figure::NapoleonBonaparte.code()).unwrap(),
        A,
    );
    let five = f.tech(Color::Yellow, 5, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    effect(&mut game, translation, 0, A, A);
    let mut events = Vec::new();
    accept(&mut game, &mut events, &[five]);
    assert_eq!(melds(&events, five), 1);
    assert_eq!(game.position().cards(Zone::Score(A)), &[five]);
}

/// Yuna Kim's "if your top card of the same color has [Authority]" reads the scored
/// card's color, so a hidden scored card is revealed first, whichever way it decides.
#[test]
fn yuna_kim_reveals_the_scored_card_before_testing_its_color() {
    for (top_authority, wins) in [(true, true), (false, false)] {
        let mut f = Fixture::default();
        let yuna = figure_card(Figure::YunaKim.code()).unwrap().color;
        f.figure(Figure::YunaKim, Zone::Board(A, yuna));
        let top = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        if top_authority {
            icon(&mut f, top, Icon::Authority);
        }
        let scored = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Move {
                verb: Verb::Score,
                target: target(scored, Zone::Hand(A)),
                destination: Zone::Score(A),
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        assert_eq!(status == Status::Winner(A), wins, "{status:?}");
        assert!(revealed(&events).contains(&scored));
    }
}

/// A Cities top shows six symbols but is one visible card.
fn city(f: &mut Fixture, color: Color, player: Player) -> CardId {
    f.add(
        Card {
            code: "synthetic-city",
            set: CardSet::Cities,
            color,
            value: 1,
            figure: None,
            echo: None,
            symbols: [Symbol::Icon(Icon::Authority); SLOTS],
            featured: None,
            effects: vec![],
        },
        Zone::Board(player, color),
    )
}

/// Pride and Prejudice compares visible cards, not visible symbols: a lone
/// City (one card, six symbols) has fewer visible cards than a blue pile of
/// three splayed left (six symbols too), so a drawn blue card stays melded
/// while a drawn red card on the City is scored.
#[test]
fn pride_and_prejudice_compares_visible_cards_not_symbols() {
    for drawn_color in [Color::Blue, Color::Red] {
        let mut f = Fixture::default();
        let pride = f.add(artifacts_card("A059").unwrap(), Zone::Hand(A));
        city(&mut f, Color::Red, A);
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        let drawn = f.tech(drawn_color, 6, Zone::Deck(Deck::base(6)), vec![]);
        let position = f
            .position()
            .with_splay(A, Color::Blue, Splay::Left)
            .unwrap();
        let expansions = Expansions::ARTIFACTS.union(Expansions::CITIES);
        let mut game = Game::new(position, expansions).unwrap();
        effect(&mut game, pride, 0, A, A);
        let mut events = Vec::new();
        run(&mut game, &mut events, &[]);
        let scored_drawn = game.position().cards(Zone::Score(A)).contains(&drawn);
        assert_eq!(scored_drawn, drawn_color == Color::Red, "{drawn_color:?}");
    }
}
