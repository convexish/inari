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

/// A blank Base card of this value.
fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, vec![])
}

fn pile(f: &mut Fixture, player: Player, color: Color, values: &[u8]) -> Vec<CardId> {
    values
        .iter()
        .map(|&value| blank(f, color, value, Zone::Board(player, color)))
        .collect()
}

fn cities() -> Expansions {
    Expansions::CITIES
}

fn with_figures() -> Expansions {
    Expansions::FIGURES.union(Expansions::CITIES)
}

fn splay(player: Player, color: Color, direction: Splay) -> Operation {
    Operation::Splay {
        player,
        color,
        direction,
    }
}

/// Start an operation and settle it, taking the only option of any forced
/// choice (a single karma) and failing on a real decision.
fn run(game: &mut Game, request: Operation, executor: Player) -> (Status, Vec<Event>) {
    game.start(request, Context::new(executor)).unwrap();
    let mut events = Vec::new();
    loop {
        match settle(game, &mut events) {
            Status::Decision(decision) if decision.options.len() == 1 => game.choose(0).unwrap(),
            status => {
                assert!(game.position().check_conservation());
                return (status, events);
            }
        }
    }
}

fn performed(card: Option<CardId>, value: Option<u8>) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card,
        value,
        short: false,
    }))
}

fn inapplicable() -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Inapplicable,
        card: None,
        value: None,
        short: false,
    }))
}

fn drawn(events: &[Event]) -> Vec<(CardId, Zone, Zone)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(Movement {
                verb: None,
                card,
                source,
                destination,
                ..
            }) => Some((*card, *source, *destination)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_splay_draws_a_city_for_its_executor_and_returns_the_splay() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Red, &[1, 3]);
    pile(&mut f, A, Color::Blue, &[2]);
    blank(&mut f, Color::Green, 3, Zone::Deck(Deck::base(3)));
    let under = city(&mut f, "C029", Zone::Deck(Deck::cities(3)));
    let top = city(&mut f, "C027", Zone::Deck(Deck::cities(3)));
    let mut game = Game::new(f.position(), cities()).unwrap();

    let (status, events) = run(&mut game, splay(A, Color::Red, Splay::Left), A);
    // The caller receives the splay's own result, not the draw's.
    assert_eq!(status, performed(None, None));
    assert!(matches!(events[0], Event::Splayed { .. }));
    assert_eq!(
        drawn(&events),
        [(top, Zone::Deck(Deck::cities(3)), Zone::Hand(A))]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), [top]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::cities(3))), [under]);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
}

#[test]
fn a_foreign_splay_draws_for_the_executor_at_their_own_highest_top() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Blue, &[2]);
    pile(&mut f, B, Color::Red, &[4, 5]);
    blank(&mut f, Color::Green, 2, Zone::Deck(Deck::base(2)));
    blank(&mut f, Color::Green, 5, Zone::Deck(Deck::base(5)));
    let two = city(&mut f, "C016", Zone::Deck(Deck::cities(2)));
    city(&mut f, "C046", Zone::Deck(Deck::cities(5)));
    let mut game = Game::new(f.position(), cities()).unwrap();

    let (status, _) = run(&mut game, splay(B, Color::Red, Splay::Right), A);
    assert_eq!(status, performed(None, None));
    assert_eq!(game.position().cards(Zone::Hand(A)), [two]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
}

#[test]
fn unsplays_repeats_and_single_cards_draw_nothing() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Red, &[1, 1]);
    pile(&mut f, A, Color::Blue, &[1]);
    pile(&mut f, B, Color::Red, &[1, 1]);
    city(&mut f, "C001", Zone::Deck(Deck::cities(1)));
    blank(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)));
    let position = f
        .position()
        .with_splay(A, Color::Red, Splay::Left)
        .unwrap()
        .with_splay(B, Color::Red, Splay::Up)
        .unwrap();
    let mut game = Game::new(position, cities()).unwrap();

    let (status, events) = run(&mut game, splay(B, Color::Red, Splay::None), A);
    assert_eq!(status, performed(None, None));
    assert!(drawn(&events).is_empty());
    for request in [
        splay(A, Color::Red, Splay::Left),
        splay(A, Color::Blue, Splay::Right),
    ] {
        let (status, events) = run(&mut game, request, A);
        assert_eq!(status, inapplicable());
        assert!(drawn(&events).is_empty());
    }
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn a_held_city_skips_the_draw_and_a_base_fallback_does_not() {
    // Holding a City: no draw.
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Red, &[1, 1]);
    city(&mut f, "C002", Zone::Hand(A));
    city(&mut f, "C001", Zone::Deck(Deck::cities(1)));
    blank(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    let (_, events) = run(&mut game, splay(A, Color::Red, Splay::Left), A);
    assert!(drawn(&events).is_empty());

    // An empty Cities deck falls back to Base, which puts no City in hand,
    // so the next splay draws again.
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Red, &[1, 1]);
    pile(&mut f, A, Color::Blue, &[1, 1]);
    let first = blank(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)));
    let second = blank(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    run(&mut game, splay(A, Color::Red, Splay::Left), A);
    run(&mut game, splay(A, Color::Blue, Splay::Left), A);
    assert_eq!(game.position().cards(Zone::Hand(A)), [second, first]);
}

#[test]
fn city_draws_skip_empty_base_ages_before_choosing_the_cities_deck() {
    for cities_four in [true, false] {
        let mut f = Fixture::default();
        pile(&mut f, A, Color::Red, &[1, 3]);
        // Base 3 is empty, so the request escalates to 4 even though a
        // Cities 3 card remains.
        city(&mut f, "C027", Zone::Deck(Deck::cities(3)));
        let base = blank(&mut f, Color::Green, 4, Zone::Deck(Deck::base(4)));
        let four = cities_four.then(|| city(&mut f, "C036", Zone::Deck(Deck::cities(4))));
        let mut game = Game::new(f.position(), cities()).unwrap();
        run(&mut game, splay(A, Color::Red, Splay::Right), A);
        assert_eq!(game.position().cards(Zone::Hand(A)), [four.unwrap_or(base)]);
    }
}

#[test]
fn an_exhausting_city_draw_ends_the_game_after_the_splay() {
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Red, &[9, 10]);
    blank(&mut f, Color::Green, 9, Zone::Deck(Deck::base(9)));
    city(&mut f, "C096", Zone::Deck(Deck::cities(10)));
    blank(&mut f, Color::Yellow, 5, Zone::Score(A));
    let mut game = Game::new(f.position(), cities()).unwrap();
    let (status, _) = run(&mut game, splay(A, Color::Red, Splay::Up), A);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.victory().unwrap().reason, WinReason::Exhaustion);
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn melding_an_arrow_city_onto_its_splay_claims_tradition_repute_or_fame() {
    for (code, direction, special) in [
        ("C027", Splay::Left, Special::Tradition),
        ("C057", Splay::Right, Special::Repute),
        ("C086", Splay::Up, Special::Fame),
    ] {
        let mut f = Fixture::default();
        pile(&mut f, A, Color::Red, &[1, 1]);
        let card = city(&mut f, code, Zone::Hand(A));
        let position = f.position().with_splay(A, Color::Red, direction).unwrap();
        let mut game = Game::new(position, cities()).unwrap();
        let value = game.position().card(card).value;
        let (status, events) = run(&mut game, meld(card, Zone::Hand(A), A, Color::Red), A);
        // The effect meld's own result returns after the claim.
        assert_eq!(status, performed(Some(card), Some(value)));
        assert!(matches!(events[0], Event::Moved(_)));
        assert!(matches!(
            events[1],
            Event::SpecialMoved { special: s, destination: SpecialLocation::Owned(A), .. } if s == special
        ));
        assert_eq!(
            game.position().special_location(special),
            SpecialLocation::Owned(A)
        );
    }
}

#[test]
fn a_mismatched_or_later_splay_earns_no_arrow_achievement() {
    // Delhi bears Left: Right, Aslant and unsplayed piles do not qualify.
    for direction in [Splay::Right, Splay::Aslant, Splay::None] {
        let mut f = Fixture::default();
        pile(&mut f, A, Color::Red, &[1, 1]);
        let delhi = city(&mut f, "C027", Zone::Hand(A));
        blank(&mut f, Color::Green, 3, Zone::Deck(Deck::base(3)));
        let position = f.position().with_splay(A, Color::Red, direction).unwrap();
        let mut game = Game::new(position, cities()).unwrap();
        run(&mut game, meld(delhi, Zone::Hand(A), A, Color::Red), A);
        // Splaying the pile Left afterwards is not the meld event.
        run(&mut game, splay(A, Color::Red, Splay::Left), A);
        assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
        assert!(
            Special::CITIES
                .into_iter()
                .all(|s| game.position().special_location(s) == SpecialLocation::Available),
            "{direction:?}"
        );
    }
    // Aslant is not Up for Fame.
    let mut f = Fixture::default();
    pile(&mut f, A, Color::Red, &[1, 1]);
    let dallas = city(&mut f, "C086", Zone::Hand(A));
    let position = f
        .position()
        .with_splay(A, Color::Red, Splay::Aslant)
        .unwrap();
    let mut game = Game::new(position, cities()).unwrap();
    run(&mut game, meld(dallas, Zone::Hand(A), A, Color::Red), A);
    assert_eq!(
        game.position().special_location(Special::Fame),
        SpecialLocation::Available
    );
}

fn junk(card: CardId, physical: Zone, logical: LogicalZone) -> Operation {
    Operation::Move {
        verb: Verb::Junk,
        target: Target {
            card,
            physical,
            logical,
        },
        destination: Zone::Junk,
    }
}

#[test]
fn junking_a_flag_or_fountain_city_from_your_hand_claims_glory_or_victory() {
    for (code, special) in [("C046", Special::Glory), ("C076", Special::Victory)] {
        let mut f = Fixture::default();
        let card = city(&mut f, code, Zone::Hand(A));
        let mut game = Game::new(f.position(), cities()).unwrap();
        run(
            &mut game,
            junk(card, Zone::Hand(A), LogicalZone::Hand(A)),
            A,
        );
        assert_eq!(
            game.position().special_location(special),
            SpecialLocation::Owned(A)
        );
    }
}

#[test]
fn junking_someone_elses_hand_or_another_zone_earns_nothing() {
    let mut f = Fixture::default();
    let theirs = city(&mut f, "C046", Zone::Hand(B));
    let board = city_on_board(&mut f, "C051", A);
    let scored = city(&mut f, "C054", Zone::Score(A));
    let mut game = Game::new(f.position(), cities()).unwrap();
    for request in [
        junk(theirs, Zone::Hand(B), LogicalZone::Physical(Zone::Hand(B))),
        junk(
            board,
            Zone::Board(A, Color::Green),
            LogicalZone::Physical(Zone::Board(A, Color::Green)),
        ),
        junk(scored, Zone::Score(A), LogicalZone::Score(A)),
    ] {
        let (status, _) = run(&mut game, request, A);
        assert!(matches!(status, Status::Complete(_)));
    }
    assert_eq!(game.position().cards(Zone::Junk).len(), 3);
    assert_eq!(
        game.position().special_location(Special::Glory),
        SpecialLocation::Available
    );
}

#[test]
fn an_owned_token_is_not_claimed_again() {
    let mut f = Fixture::default();
    let card = city(&mut f, "C046", Zone::Hand(A));
    let mut game = Game::new(f.position(), cities()).unwrap();
    run(
        &mut game,
        Operation::ClaimSpecial {
            special: Special::Glory,
            check_eligibility: false,
        },
        B,
    );
    let (status, events) = run(
        &mut game,
        junk(card, Zone::Hand(A), LogicalZone::Hand(A)),
        A,
    );
    assert!(matches!(status, Status::Complete(_)));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::SpecialMoved { .. }))
    );
    assert_eq!(
        game.position().special_location(Special::Glory),
        SpecialLocation::Owned(B)
    );
}

#[test]
fn meiji_borrowed_hands_count_for_glory_and_the_held_city_check() {
    let meiji = || figure_card(Figure::EmperorMeiji.code()).unwrap();
    // The executor junks a Flag City from the opponent's hand through Meiji.
    let mut f = Fixture::default();
    let card = meiji();
    let zone = Zone::Board(A, card.color);
    f.add(card, zone);
    let theirs = city(&mut f, "C046", Zone::Hand(B));
    let mut game = Game::new(f.position(), with_figures()).unwrap();
    assert!(game.position().meiji_active(A));
    run(
        &mut game,
        junk(theirs, Zone::Hand(B), LogicalZone::Hand(A)),
        A,
    );
    assert_eq!(
        game.position().special_location(Special::Glory),
        SpecialLocation::Owned(A)
    );

    // A City in the borrowed hand is in the executor's hand: no City draw.
    let mut f = Fixture::default();
    let card = meiji();
    let color = card.color;
    f.add(card, Zone::Board(A, color));
    let other = Color::ALL.into_iter().find(|&c| c != color).unwrap();
    pile(&mut f, A, other, &[1, 1]);
    city(&mut f, "C002", Zone::Hand(B));
    blank(&mut f, Color::Green, 4, Zone::Deck(Deck::base(4)));
    city(&mut f, "C036", Zone::Deck(Deck::cities(4)));
    let mut game = Game::new(f.position(), with_figures()).unwrap();
    let (_, events) = run(&mut game, splay(A, other, Splay::Left), A);
    assert!(drawn(&events).is_empty());
}

#[test]
fn a_replaced_meld_earns_nothing() {
    let mut f = Fixture::default();
    let card = figure_card(Figure::YunaKim.code()).unwrap();
    let zone = Zone::Board(B, card.color);
    f.add(card, zone);
    pile(&mut f, A, Color::Red, &[1, 1]);
    let delhi = city(&mut f, "C027", Zone::Hand(A));
    let position = f.position().with_splay(A, Color::Red, Splay::Left).unwrap();
    let mut game = Game::new(position, with_figures()).unwrap();
    game.start(meld(delhi, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    // Any return order: none of them is a meld.
    let status = loop {
        match settle(&mut game, &mut Vec::new()) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => break status,
        }
    };
    assert!(matches!(
        status,
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Replaced,
            ..
        }))
    ));
    assert_eq!(
        game.position().special_location(Special::Tradition),
        SpecialLocation::Available
    );
}

/// Hedy Lamarr on A's board, a red pile splayed left and Delhi in hand.
fn hedy_meld(base_nine: usize) -> (Game, CardId) {
    let mut f = Fixture::default();
    let hedy = figure_card(Figure::HedyLamarr.code()).unwrap();
    assert_eq!(hedy.color, Color::Green);
    f.add(hedy, Zone::Board(A, Color::Green));
    pile(&mut f, A, Color::Red, &[1, 1]);
    let delhi = city(&mut f, "C027", Zone::Hand(A));
    for _ in 0..base_nine {
        blank(&mut f, Color::Yellow, 9, Zone::Deck(Deck::base(9)));
    }
    city(&mut f, "C091", Zone::Deck(Deck::cities(9)));
    blank(&mut f, Color::Yellow, 5, Zone::Score(A));
    let position = f.position().with_splay(A, Color::Red, Splay::Left).unwrap();
    (Game::new(position, with_figures()).unwrap(), delhi)
}

/// Settle, answering Hedy's "splay a color up" with red.
fn splay_red_for_hedy(game: &mut Game, events: &mut Vec<Event>) -> Status {
    loop {
        match settle(game, events) {
            Status::Decision(decision) => {
                let option = decision
                    .options
                    .iter()
                    .position(|option| {
                        matches!(
                            option,
                            Alternative::Splay {
                                color: Color::Red,
                                ..
                            }
                        )
                    })
                    .or((decision.options.len() == 1).then_some(0))
                    .expect("Hedy's splay or a forced karma");
                game.choose(option).unwrap();
            }
            status => return status,
        }
    }
}

#[test]
fn hedys_first_splay_cannot_cancel_an_earned_claim() {
    let (mut game, delhi) = hedy_meld(2);
    game.start(meld(delhi, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    let status = splay_red_for_hedy(&mut game, &mut events);
    assert!(matches!(status, Status::Complete(_)));
    // Hedy splayed the qualifying pile up, and that splay drew a City inside
    // her karma; the meld still earned Tradition.
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
    let hand = game.position().cards(Zone::Hand(A));
    assert_eq!(hand.len(), 1);
    assert_eq!(game.position().card(hand[0]).set, CardSet::Cities);
    assert_eq!(
        game.position().special_location(Special::Tradition),
        SpecialLocation::Owned(A)
    );
    let splayed = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::Splayed {
                    inside_karma: true,
                    ..
                }
            )
        })
        .unwrap();
    let claimed = events
        .iter()
        .position(|e| matches!(e, Event::SpecialMoved { .. }))
        .unwrap();
    assert!(splayed < claimed);
}

#[test]
fn an_exhausting_draw_inside_hedy_ends_the_game_before_the_claim() {
    let (mut game, delhi) = hedy_meld(0);
    game.start(meld(delhi, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    let status = splay_red_for_hedy(&mut game, &mut Vec::new());
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.victory().unwrap().reason, WinReason::Exhaustion);
    assert_eq!(
        game.position().special_location(Special::Tradition),
        SpecialLocation::Available
    );
}

const SPLAY_ALL_LEFT: Instruction = Instruction::Splay(SplayStep {
    who: Who::Executor,
    choices: SplayChoices::Fixed {
        colors: ColorSet::Only(&Color::ALL),
        direction: Splay::Left,
        from: None,
    },
    targets: SplayTargets::Changes,
    optional: false,
    all: true,
});

/// Two unsplayed piles; the red one hides a 5 bonus that Left reveals.
fn splay_all(base_twos: usize) -> (Game, CardId) {
    let mut f = Fixture::default();
    let bonus = blank(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    f.cards[usize::from(bonus.0)].symbols[3] = Symbol::Bonus(5);
    blank(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    pile(&mut f, A, Color::Blue, &[1, 2]);
    for _ in 0..base_twos {
        blank(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2)));
    }
    blank(&mut f, Color::Yellow, 3, Zone::Score(A));
    blank(&mut f, Color::Yellow, 6, Zone::Score(B));
    let source = blank(&mut f, Color::Purple, 1, Zone::Hand(A));
    f.cards[usize::from(source.0)].effects[0].instructions = vec![SPLAY_ALL_LEFT];
    (Game::new(f.position(), cities()).unwrap(), source)
}

fn execute(card: CardId) -> Operation {
    Operation::ExecuteEffect {
        card,
        effect: 0,
        boxed: BoxedValues::default(),
    }
}

#[test]
fn a_splay_batch_keeps_its_order_choice_only_when_a_city_draw_could_end_the_game() {
    // Enough Base cards for both draws: one canonical order, no menu.
    let (mut game, source) = splay_all(2);
    let (status, events) = run(&mut game, execute(source), A);
    assert!(matches!(status, Status::Complete(_)));
    assert_eq!(drawn(&events).len(), 2);

    // The first draw exhausts the game, so the pile splayed first decides the
    // winner: red reveals the 5 bonus (A: 3 + 5 > 6), blue does not.
    for (color, winner) in [(Color::Red, A), (Color::Blue, B)] {
        let (mut game, source) = splay_all(0);
        game.start(execute(source), Context::new(A)).unwrap();
        let menu = decision(&mut game, &mut Vec::new());
        assert_eq!(menu.options.len(), 2);
        let option = menu
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Splay { color: c, .. } if *c == color))
            .unwrap();
        game.choose(option).unwrap();
        assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(winner));
    }
}

#[test]
fn a_cities_deal_keeps_the_base_deal_and_shuffles_cities_like_production() {
    for seed in [0, 7, 1234] {
        let base = Game::new_base(seed).unwrap();
        let game = Game::new_game(seed, Expansions::CITIES).unwrap();
        let (b, c) = (base.position(), game.position());
        // Base identities come first in every catalog, so the deal compares
        // by id: hands, supply and every Base deck.
        for zone in [Zone::Hand(A), Zone::Hand(B), Zone::AvailableAchievements]
            .into_iter()
            .chain((1..=11).map(|age| Zone::Deck(Deck::base(age))))
        {
            assert_eq!(b.cards(zone), c.cards(zone), "{zone:?}");
        }
        // Shuffle each Cities deck after the Base deal, ascending
        // ages, from cards in publisher-code order (its frozen Fisher-Yates).
        fn shuffle<T>(rng: &mut Rng, cards: &mut [T]) {
            for i in (1..cards.len()).rev() {
                let j = (rng.next_u64() % (i + 1) as u64) as usize;
                cards.swap(i, j);
            }
        }
        let mut rng = Rng::new(seed);
        let mut base_decks: Vec<Vec<u16>> = (1..=11)
            .map(|age| vec![0; if age == 1 { 15 } else { 10 }])
            .collect();
        for deck in &mut base_decks {
            shuffle(&mut rng, deck);
        }
        for age in 1..=11u8 {
            let mut codes: Vec<&str> = CITY_METADATA
                .iter()
                .filter(|m| m.value == age)
                .map(|m| m.code)
                .collect();
            shuffle(&mut rng, &mut codes);
            let dealt: Vec<&str> = c
                .cards(Zone::Deck(Deck::cities(age)))
                .iter()
                .map(|&id| c.card(id).code)
                .collect();
            assert_eq!(dealt, codes, "seed {seed} age {age}");
        }
        assert_eq!(c.specials().count(), 10);
        assert!(
            c.cards(Zone::AvailableAchievements)
                .iter()
                .all(|&id| c.card(id).set == CardSet::Base)
        );
    }
}

#[test]
fn every_supported_combination_deals_and_disabled_sets_stay_out() {
    let both = Expansions::FIGURES.union(Expansions::CITIES);
    for expansions in [
        Expansions::BASE,
        Expansions::FIGURES,
        Expansions::CITIES,
        both,
    ] {
        let game = Game::new_game(3, expansions).unwrap();
        let position = game.position();
        for set in [CardSet::Figures, CardSet::Cities] {
            let dealt: usize = (0..=11)
                .map(|age| position.cards(Zone::Deck(Deck { set, age })).len())
                .sum();
            let expected = if expansions.includes(set) { 115 } else { 0 };
            assert_eq!(dealt, expected, "{expansions:?} {set:?}");
        }
        assert!(position.check_conservation());
    }
}

#[test]
fn self_executing_a_city_inside_a_self_execution_earns_no_chain() {
    // Escapism, self-executed: "Reveal and junk a card in your hand. Return from your
    // hand all cards of value equal to the value of the junked card. Draw three cards
    // of that value. Self-execute the junked card." The junked City has no dogma
    // effects: it executes nothing and the keyword earns no Chain.
    let mut f = Fixture::default();
    let escapism = f.add(base_card("B114").unwrap(), Zone::Board(A, Color::Purple));
    city(&mut f, "C086", Zone::Hand(A));
    for _ in 0..3 {
        blank(&mut f, Color::Green, 9, Zone::Deck(Deck::base(9)));
    }
    let eleven = blank(&mut f, Color::Green, 11, Zone::Deck(Deck::base(11)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    game.self_execute(escapism, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(11))), [eleven]);
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 3);
}
