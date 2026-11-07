use inno_engine::*;

mod common;
use common::*;

fn city(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(city_card(code).unwrap(), zone)
}

fn city_on_board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = city_card(code).unwrap();
    let zone = Zone::Board(player, card.color);
    f.add(card, zone)
}

fn base_on_board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let zone = Zone::Board(player, card.color);
    f.add(card, zone)
}

fn figure(f: &mut Fixture, figure: Figure, zone: Option<Zone>) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let zone = zone.unwrap_or(Zone::Board(A, card.color));
    f.add(card, zone)
}

fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, vec![])
}

fn deck(f: &mut Fixture, value: u8, count: usize) -> Vec<CardId> {
    (0..count)
        .map(|_| blank(f, Color::Green, value, Zone::Deck(Deck::base(value))))
        .collect()
}

fn cities() -> Expansions {
    Expansions::CITIES
}

fn with_figures() -> Expansions {
    Expansions::FIGURES.union(Expansions::CITIES)
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

fn play(
    game: &mut Game,
    mut decide: impl FnMut(&Decision) -> Option<usize>,
) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    loop {
        let status = settle(game, &mut events);
        assert!(game.position().check_conservation());
        match status {
            Status::Decision(decision) => {
                let option = decide(&decision)
                    .or((decision.options.len() == 1).then_some(0))
                    .unwrap_or_else(|| panic!("unexpected decision {decision:?}"));
                game.choose(option).unwrap();
            }
            status => return (status, events),
        }
    }
}

/// Answer the endorsement offer, and the first option of any other choice
/// (a payment, an achievement to junk).
fn endorse(answer: bool) -> impl FnMut(&Decision) -> Option<usize> {
    move |decision| {
        let offer = decision.options == [Alternative::Branch(0), Alternative::Branch(1)];
        Some(if offer && !answer { 1 } else { 0 })
    }
}

fn endorsed(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::Endorsed { .. }))
}

/// Writing (blue, Concept: "Draw a [2].") with Babylon, a Concept City of
/// value 1, on A's board and one value-1 card in hand.
fn writing() -> (Fixture, CardId, CardId) {
    let mut f = Fixture::default();
    let writing = base_on_board(&mut f, "B012", A);
    city_on_board(&mut f, "C007", A);
    let payment = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    deck(&mut f, 1, 2);
    deck(&mut f, 2, 6);
    (f, writing, payment)
}

#[test]
fn an_endorsed_writing_pays_a_hand_card_and_runs_twice() {
    let (f, writing, payment) = writing();
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, writing);
    let offer = decision(&mut game, &mut Vec::new());
    assert_eq!(
        (offer.option_label(0, A), offer.option_label(1, A)),
        (Some("Endorse this Dogma action"), Some("Do not endorse"))
    );
    let (status, events) = play(&mut game, endorse(true));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(endorsed(&events));
    assert!(game.position().endorsement_used());
    assert_eq!(game.position().cards(Zone::Junk), [payment]);
    let hand = game.position().cards(Zone::Hand(A));
    assert_eq!(hand.len(), 2);
    assert!(hand.iter().all(|&c| game.position().card(c).value == 2));
}

#[test]
fn declining_keeps_the_turns_endorsement_and_it_works_once() {
    let (mut f, writing, _) = writing();
    blank(&mut f, Color::Red, 1, Zone::Hand(A));
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, writing);
    let (_, events) = play(&mut game, endorse(false));
    assert!(!endorsed(&events));
    assert!(!game.position().endorsement_used());
    // One draw; then the second action may still endorse.
    dogma(&mut game, writing);
    let (_, events) = play(&mut game, endorse(true));
    assert!(endorsed(&events));
    // Next turn: the allowance resets for B, and A's third action is gone.
    assert_eq!(game.position().turn_owner(), B);
    assert!(!game.position().endorsement_used());
}

#[test]
fn a_second_dogma_in_the_turn_gets_no_offer() {
    let (mut f, writing, _) = writing();
    blank(&mut f, Color::Red, 1, Zone::Hand(A));
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, writing);
    play(&mut game, endorse(true));
    let before = game.position().cards(Zone::Hand(A)).len();
    dogma(&mut game, writing);
    // No offer: the only decision would be a failure in `play`.
    let (_, events) = play(&mut game, |_| None);
    assert!(!endorsed(&events));
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), before + 1);
}

#[test]
fn a_sharer_runs_once_then_the_owner_twice_with_one_sharing_draw() {
    let (mut f, writing, _) = writing();
    // B shows more Concept than A, so B shares.
    let concept = blank(&mut f, Color::Yellow, 1, Zone::Board(B, Color::Yellow));
    f.cards[usize::from(concept.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
    let concept = blank(&mut f, Color::Purple, 1, Zone::Board(B, Color::Purple));
    f.cards[usize::from(concept.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, writing);
    play(&mut game, endorse(true));
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
    // Two 2s for A, then one sharing draw (A's highest top is 1).
    let values: Vec<u8> = game
        .position()
        .cards(Zone::Hand(A))
        .iter()
        .map(|&c| game.position().card(c).value)
        .collect();
    assert_eq!(values, [2, 2, 1]);
}

/// Archery (red, Authority: "I demand you draw a [1], then transfer the
/// highest card in your hand to my hand! / Junk an available achievement of
/// value 1 or 2.") with Damascus, whose only Authority is its Search icon.
fn archery() -> (Fixture, CardId) {
    let mut f = Fixture::default();
    let archery = base_on_board(&mut f, "B001", A);
    city_on_board(&mut f, "C004", A);
    blank(&mut f, Color::Red, 1, Zone::Hand(A));
    deck(&mut f, 1, 4);
    deck(&mut f, 2, 2);
    for value in [1, 2, 2] {
        blank(&mut f, Color::Yellow, value, Zone::AvailableAchievements);
    }
    (f, archery)
}

#[test]
fn an_endorsed_demand_hits_a_vulnerable_opponent_twice() {
    let (f, archery) = archery();
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, archery);
    play(&mut game, endorse(true));
    // Two demand executions: B drew and passed two cards to A.
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    // The owner's effect ran twice: two achievements junked (plus payment).
    assert_eq!(game.position().cards(Zone::AvailableAchievements).len(), 1);
}

#[test]
fn without_a_vulnerable_opponent_an_endorsed_demand_runs_zero_times() {
    let (mut f, archery) = archery();
    for color in [Color::Blue, Color::Green] {
        let card = blank(&mut f, color, 1, Zone::Board(B, color));
        f.cards[usize::from(card.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    }
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, archery);
    play(&mut game, endorse(true));
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    // B shares the second effect once, A runs it twice: three junked.
    assert!(
        game.position()
            .cards(Zone::AvailableAchievements)
            .is_empty()
    );
}

#[test]
fn oars_sees_its_first_pass_transfer_after_an_empty_repeat() {
    // Oars: "I demand you transfer a card with [Prosperity] from your hand to
    // my score pile! If you do, draw a [1] and repeat! / If no cards were
    // transferred due to the demand, draw a [1]."
    for transfers in [true, false] {
        let mut f = Fixture::default();
        let oars = base_on_board(&mut f, "B003", A);
        city_on_board(&mut f, "C004", A);
        blank(&mut f, Color::Red, 1, Zone::Hand(A));
        if transfers {
            let card = blank(&mut f, Color::Yellow, 1, Zone::Hand(B));
            f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
        }
        deck(&mut f, 1, 4);
        let mut game = Game::new(f.position(), cities()).unwrap();
        dogma(&mut game, oars);
        play(&mut game, endorse(true));
        // The second demand pass transfers nothing, yet the first pass's
        // transfer still answers "if no cards were transferred".
        let draws = game.position().cards(Zone::Hand(A)).len();
        assert_eq!(draws, if transfers { 0 } else { 2 }, "{transfers}");
        assert_eq!(
            game.position().cards(Zone::Score(A)).len(),
            usize::from(transfers)
        );
    }
}

#[test]
fn no_offer_without_a_qualifying_top_city_or_a_payable_card() {
    for case in 0..4 {
        let mut f = Fixture::default();
        let writing = base_on_board(&mut f, "B012", A);
        match case {
            // No City at all.
            0 => {}
            // A covered Concept City.
            1 => {
                city_on_board(&mut f, "C007", A);
                blank(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
            }
            // A top City without Concept (Damascus).
            2 => {
                city_on_board(&mut f, "C004", A);
            }
            // Only hand cards above the City's value.
            _ => {
                city_on_board(&mut f, "C007", A);
            }
        }
        let value = if case == 3 { 3 } else { 1 };
        blank(&mut f, Color::Red, value, Zone::Hand(A));
        deck(&mut f, 2, 2);
        let mut game = Game::new(f.position(), cities()).unwrap();
        dogma(&mut game, writing);
        let (_, events) = play(&mut game, |_| None);
        assert!(!endorsed(&events), "{case}");
        assert!(!game.position().endorsement_used());
    }
}

#[test]
fn paying_with_a_flag_city_claims_glory() {
    let mut f = Fixture::default();
    let writing = base_on_board(&mut f, "B012", A);
    city_on_board(&mut f, "C065", A);
    let stockholm = city(&mut f, "C051", Zone::Hand(A));
    deck(&mut f, 2, 2);
    let mut game = Game::new(f.position(), cities()).unwrap();
    dogma(&mut game, writing);
    play(&mut game, endorse(true));
    assert_eq!(game.position().cards(Zone::Junk), [stockholm]);
    assert_eq!(
        game.position().special_location(Special::Glory),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn auspice_is_chosen_after_the_endorsement() {
    let (mut f, writing, _) = writing();
    // Hammurabi prints Concept: Auspice could use Avatar instead, which
    // matters because the opponent shares Concept but not Avatar.
    figure(&mut f, Figure::Hammurabi, None);
    for color in [Color::Yellow, Color::Purple] {
        let rival = blank(&mut f, color, 1, Zone::Board(B, color));
        f.cards[usize::from(rival.0)].symbols = [Symbol::Icon(Icon::Concept); SLOTS];
    }
    let mut game = Game::new(f.position(), with_figures()).unwrap();
    let position = game.position();
    assert!(position.icon_count(B, Icon::Concept) >= position.icon_count(A, Icon::Concept));
    assert!(position.icon_count(B, Icon::Avatar) < position.icon_count(A, Icon::Avatar));
    let actions = game.legal_actions();
    assert!(actions.contains(&Action::Dogma {
        card: writing,
        auspice: false
    }));
    assert!(!actions.contains(&Action::Dogma {
        card: writing,
        auspice: true
    }));
    assert_eq!(
        game.take_action(
            Action::Dogma {
                card: writing,
                auspice: true
            },
            ActionKind::Regular
        ),
        Err(Error::InvalidAction)
    );
    dogma(&mut game, writing);
    let mut auspice = None;
    play(&mut game, |decision| {
        if decision.options == [Alternative::Branch(0), Alternative::Branch(1)] {
            return Some(0);
        }
        auspice = Some(decision.options.clone());
        decision
            .options
            .iter()
            .position(|o| *o == Alternative::Icon(Icon::Avatar))
    });
    assert_eq!(
        auspice,
        Some(vec![
            Alternative::Icon(Icon::Concept),
            Alternative::Icon(Icon::Avatar)
        ])
    );
}

#[test]
fn kepler_replacing_an_endorsed_dogma_is_not_doubled() {
    let (mut f, archery) = archery();
    figure(&mut f, Figure::JohannesKepler, None);
    deck(&mut f, 3, 2);
    let mut game = Game::new(f.position(), with_figures()).unwrap();
    dogma(&mut game, archery);
    let (_, events) = play(&mut game, endorse(true));
    assert!(endorsed(&events));
    assert!(game.position().endorsement_used());
    // One demand execution only: a single card reached A's hand.
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 1);
}

#[test]
fn homer_tucking_the_payment_still_endorses() {
    let mut f = Fixture::default();
    let writing = base_on_board(&mut f, "B012", A);
    city_on_board(&mut f, "C007", A);
    figure(&mut f, Figure::Homer, None);
    let tigernmas = figure(&mut f, Figure::Tigernmas, Some(Zone::Hand(A)));
    deck(&mut f, 2, 4);
    let mut game = Game::new(f.position(), with_figures()).unwrap();
    dogma(&mut game, writing);
    let (_, events) = play(&mut game, endorse(true));
    assert!(endorsed(&events));
    // Homer tucked the Figure instead of junking it; Writing still ran twice.
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Moved(Movement { verb: Some(Verb::Tuck), card, .. }) if *card == tigernmas
    )));
    assert!(game.position().cards(Zone::Junk).is_empty());
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
}

#[test]
fn the_offer_resumes_identically_after_clone_and_sampling() {
    let (f, writing, _) = writing();
    let mut game = Game::new(f.position(), cities())
        .unwrap()
        .with_tracking()
        .unwrap();
    dogma(&mut game, writing);
    let offer = decision(&mut game, &mut Vec::new());
    let mut world = game.determinize(A, 3).unwrap();
    assert_eq!(decision(&mut world, &mut Vec::new()), offer);
    let mut clone = game.clone();
    for game in [&mut game, &mut world, &mut clone] {
        play(game, endorse(true));
        assert!(game.position().endorsement_used());
    }
    assert_eq!(game, clone);
}
