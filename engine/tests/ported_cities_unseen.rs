use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
}

fn city(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(city_card(code).unwrap(), zone)
}

fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, vec![])
}

fn echoes_cities() -> Expansions {
    Expansions::ECHOES.union(Expansions::CITIES)
}

fn unseen_cities() -> Expansions {
    Expansions::UNSEEN.union(Expansions::CITIES)
}

/// Start `card`'s effect `index` for `executor`, as `demander`'s Dogma.
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

fn decline(menu: &Decision) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Decline)
}

/// Settle, answering each decision with `decide` or else its first option.
fn play(
    game: &mut Game,
    mut decide: impl FnMut(&Decision) -> Option<usize>,
) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    for _ in 0..300 {
        let status = settle(game, &mut events);
        assert!(game.position().check_conservation());
        match status {
            Status::Decision(menu) => {
                let option = decide(&menu).unwrap_or(0);
                game.choose(option).unwrap();
            }
            status => return (status, events),
        }
    }
    panic!("did not terminate");
}

fn complete(status: &Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
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

fn meld_action(game: &mut Game, card: CardId) {
    game.take_action(Action::Meld { card }, ActionKind::Regular)
        .unwrap();
}

// ------------------------------------------------------------ Blackmail

/// Blackmail's `must` makes Padlock's "you may score up to three cards from your hand
/// of different values" score at least one, after which stopping is allowed.
#[test]
fn blackmailed_padlock_scores_at_least_one_card_then_may_stop() {
    let mut f = Fixture::default();
    let blackmail = unseen(&mut f, "U040", Zone::Board(A, Color::Green));
    let padlock = unseen(&mut f, "U016", Zone::Hand(B));
    let melded = blank(&mut f, Color::Blue, 1, Zone::Hand(B));
    let three = blank(&mut f, Color::Yellow, 3, Zone::Hand(B));
    let four = blank(&mut f, Color::Purple, 4, Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    let mut events = Vec::new();
    // A melds the blue card, then has B self-execute Padlock.
    let meld = decision(&mut game, &mut events);
    game.choose(pick_known(&meld, melded).unwrap()).unwrap();
    let execute = decision(&mut game, &mut events);
    game.choose(pick_known(&execute, padlock).unwrap()).unwrap();
    // A has no secret, so Padlock's score clause runs, under must.
    let first = decision(&mut game, &mut events);
    assert_eq!(first.player, B);
    assert!(pick_known(&first, three).is_some());
    assert_eq!(decline(&first), None, "must scores at least one card");
    game.choose(pick_known(&first, three).unwrap()).unwrap();
    let second = decision(&mut game, &mut events);
    game.choose(decline(&second).expect("may stop after one card"))
        .unwrap();
    complete(&settle(&mut game, &mut events));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(B)), [three]);
    let mut hand = position.cards(Zone::Hand(B)).to_vec();
    hand.sort();
    assert_eq!(hand, [padlock, four]);
}

// -------------------------------------------- cards that execute nothing

/// Robocar: a melded City executes nothing, and the effect still repeats.
#[test]
fn robocar_repeats_after_melding_a_city() {
    let mut f = Fixture::default();
    let robocar = f.add(echoes_card("E111").unwrap(), Zone::Board(A, Color::Green));
    let athens = city(&mut f, "C001", Zone::Hand(A));
    let other = blank(&mut f, Color::Blue, 1, Zone::Hand(A));
    let mut game = Game::new(f.position(), echoes_cities()).unwrap();
    effect(&mut game, robocar, 0, A, A);
    let mut events = Vec::new();
    // B picks the unrevealed City first, by its back.
    let first = decision(&mut game, &mut events);
    assert_eq!(first.player, B);
    let pick = first
        .options
        .iter()
        .position(|option| match option {
            Alternative::BlindSelect { back, .. } => back.set == CardSet::Cities,
            _ => false,
        })
        .unwrap_or_else(|| pick_known(&first, athens).unwrap());
    game.choose(pick).unwrap();
    let (status, events) = play(&mut game, |_| None);
    complete(&status);
    let melds: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Meld) => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(melds, [athens, other], "the City first, then the repeat");
    let position = game.position();
    assert_eq!(position.top(A, Color::Red), Some(athens));
    assert_eq!(position.top(A, Color::Blue), Some(other));
    assert!(position.cards(Zone::Hand(A)).is_empty());
}

/// Legend: a top City executes nothing and is still scored, and the repeat goes on to
/// the card beneath it.
#[test]
fn legend_scores_a_top_city_after_its_empty_self_execution() {
    let mut f = Fixture::default();
    let legend = unseen(&mut f, "U045", Zone::Board(A, Color::Purple));
    let nine = blank(&mut f, Color::Red, 9, Zone::Board(A, Color::Red));
    let athens = city(&mut f, "C001", Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), unseen_cities()).unwrap();
    effect(&mut game, legend, 0, A, A);
    let red = Alternative::Colors(1 << Color::Red as u8);
    let (status, _) = play(&mut game, |menu| {
        menu.options.iter().position(|option| *option == red)
    });
    complete(&status);
    // 1 + 9 points: the repeat after the City scores the 9 and stops.
    assert_eq!(game.position().cards(Zone::Score(A)), [athens, nine]);
}

// ---------------------------------------- Meld action order and promotion

/// A promoted City gets no free Dogma, so no achievement check follows its promotion
/// meld: a condition that meld completes waits for the next action's end. An ordinary
/// promoted card is checked after its Dogma. Here the promoted card is the fifth top of
/// value 8 or more (Universe).
#[test]
fn a_promoted_city_takes_no_dogma_and_no_second_achievement_check() {
    for promote_city in [true, false] {
        let mut f = Fixture::default();
        for color in [Color::Red, Color::Blue, Color::Green, Color::Yellow] {
            blank(&mut f, color, 8, Zone::Board(A, color));
        }
        let melded = blank(&mut f, Color::Yellow, 9, Zone::Hand(A));
        // Vancouver's two Plus icons would draw from the empty decks if a
        // promotion meld ran City icons.
        let promoted = if promote_city {
            city(&mut f, "C085", Zone::Forecast(A))
        } else {
            blank(&mut f, Color::Purple, 8, Zone::Forecast(A))
        };
        let mut game = Game::new(f.position(), echoes_cities()).unwrap();
        meld_action(&mut game, melded);
        let (status, _) = play(&mut game, |_| None);
        complete(&status);
        let position = game.position();
        assert_eq!(position.top(A, Color::Purple), Some(promoted));
        assert!(position.cards(Zone::Forecast(A)).is_empty());
        assert!(position.cards(Zone::Hand(A)).is_empty());
        let expected = if promote_city {
            SpecialLocation::Available
        } else {
            SpecialLocation::Owned(A)
        };
        assert_eq!(
            position.special_location(Special::Universe),
            expected,
            "city={promote_city}"
        );
        assert_eq!(
            position.eligible_for_special(A, Special::Universe),
            promote_city
        );
    }
}

/// Mecca's Search, a City icon of the Meld action, finishes before promotion: its
/// returns are chosen before the Forecast choice.
#[test]
fn a_citys_search_returns_precede_the_promotion_choice() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
    let mecca = city(&mut f, "C035", Zone::Hand(A));
    let deck = Zone::Deck(Deck::base(3));
    let rest = blank(&mut f, Color::Red, 3, deck);
    let p1 = blank(&mut f, Color::Red, 3, deck);
    let p2 = blank(&mut f, Color::Red, 3, deck);
    let p3 = blank(&mut f, Color::Red, 3, deck);
    f.cards[usize::from(p2.0)].symbols[0] = Symbol::Icon(Icon::Health);
    let blue = blank(&mut f, Color::Blue, 1, Zone::Forecast(A));
    let green = blank(&mut f, Color::Green, 2, Zone::Forecast(A));
    let mut game = Game::new(f.position(), echoes_cities()).unwrap();
    meld_action(&mut game, mecca);
    let mut events = Vec::new();
    let returns = decision(&mut game, &mut events);
    assert_eq!(returns.options.len(), 2);
    assert!(returns.options.iter().all(|option| matches!(
        option,
        Alternative::Move {
            verb: Verb::Return,
            ..
        }
    )));
    game.choose(pick_known(&returns, p3).unwrap()).unwrap();
    // The last return, then the Forecast choice.
    let mut menu = decision(&mut game, &mut events);
    if pick_known(&menu, p1).is_some() {
        game.choose(pick_known(&menu, p1).unwrap()).unwrap();
        menu = decision(&mut game, &mut events);
    }
    assert_eq!(game.position().cards(Zone::Hand(A)), [p2]);
    assert_eq!(game.position().cards(deck), [p1, p3, rest]);
    assert_eq!(game.position().top(A, Color::Purple), Some(mecca));
    assert_eq!(menu.options.len(), 2, "promote either Forecast card");
    for card in [blue, green] {
        assert!(menu.options.iter().any(|option| matches!(
            option,
            Alternative::Move {
                verb: Verb::Meld,
                target: TargetDescriptor::Known { card: c, source: Zone::Forecast(_) },
            } if *c == card
        )));
    }
    game.choose(pick_known(&menu, green).unwrap()).unwrap();
    let (status, _) = play(&mut game, |_| None);
    complete(&status);
    assert_eq!(game.position().top(A, Color::Green), Some(green));
}

/// Atlanta's Plus draws an [11]; with the [11] deck empty the game ends before
/// promotion, so the Forecast card stays and Atlanta stays on top.
#[test]
fn a_terminal_city_plus_draw_prevents_promotion() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Blue, 9, Zone::Board(A, Color::Blue));
    let atlanta = city(&mut f, "C102", Zone::Hand(A));
    let waiting = blank(&mut f, Color::Blue, 1, Zone::Forecast(A));
    let mut game = Game::new(f.position(), echoes_cities()).unwrap();
    meld_action(&mut game, atlanta);
    let (status, _) = play(&mut game, |_| None);
    assert!(
        matches!(status, Status::Winner(_) | Status::Draw(_)),
        "{status:?}"
    );
    let position = game.position();
    assert_eq!(position.cards(Zone::Forecast(A)), [waiting]);
    assert_eq!(position.top(A, Color::Blue), Some(atlanta));
}

/// A promoted card's free Dogma can be endorsed: Athens, a top City printing Writing's
/// featured Concept, lets A junk a [1] from hand and run Writing twice.
#[test]
fn a_promoted_free_dogma_can_be_endorsed() {
    let mut f = Fixture::default();
    city(&mut f, "C001", Zone::Board(A, Color::Red));
    blank(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
    let melded = blank(&mut f, Color::Blue, 1, Zone::Hand(A));
    let payment = blank(&mut f, Color::Green, 1, Zone::Hand(A));
    let writing = f.add(base_card("B012").unwrap(), Zone::Forecast(A));
    let twos: Vec<_> = (0..3)
        .map(|_| blank(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2))))
        .collect();
    let mut game = Game::new(f.position(), echoes_cities()).unwrap();
    meld_action(&mut game, melded);
    let mut offers = 0;
    let (status, events) = play(&mut game, |menu| {
        let endorse = menu
            .options
            .iter()
            .position(|option| *option == Alternative::Branch(0))
            .filter(|_| menu.options.contains(&Alternative::Branch(1)));
        offers += usize::from(endorse.is_some());
        endorse
    });
    complete(&status);
    assert_eq!(offers, 1);
    let position = game.position();
    assert_eq!(position.top(A, Color::Blue), Some(writing));
    assert!(position.cards(Zone::Junk).contains(&payment));
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Endorsed { player: A, card } if *card == writing
    )));
    // Writing ran twice: two [2] drawn.
    let mut hand = position.cards(Zone::Hand(A)).to_vec();
    hand.sort();
    assert_eq!(hand, [twos[1], twos[2]]);
}

// ------------------------------------------------------- Unseen readings

/// Sniping's demander chooses among the colors the victim has cards of.
#[test]
fn sniping_offers_only_colors_on_the_victims_board() {
    let mut f = Fixture::default();
    let sniping = unseen(&mut f, "U057", Zone::Board(A, Color::Red));
    blank(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
    blank(&mut f, Color::Red, 2, Zone::Board(B, Color::Red));
    let one = blank(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
    let three = blank(&mut f, Color::Blue, 3, Zone::Board(B, Color::Blue));
    let five = blank(&mut f, Color::Blue, 5, Zone::Board(B, Color::Blue));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, sniping, 0, B, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    let red = Alternative::Colors(1 << Color::Red as u8);
    let blue = Alternative::Colors(1 << Color::Blue as u8);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.contains(&red));
    let pick = menu.options.iter().position(|o| *o == blue).unwrap();
    game.choose(pick).unwrap();
    let (status, _) = play(&mut game, |_| None);
    complete(&status);
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Blue)), [five, one]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), [three]);
}

/// Spanish Inquisition returns from the hand first, then from the score pile, so a hand
/// card and a score card bound for the same deck keep that order, with no choice.
#[test]
fn spanish_inquisition_returns_the_hand_card_before_the_score_card() {
    let mut f = Fixture::default();
    let inquisition = unseen(&mut f, "U037", Zone::Board(A, Color::Red));
    let deck = Zone::Deck(Deck::base(2));
    let existing = blank(&mut f, Color::Green, 2, deck);
    blank(&mut f, Color::Blue, 5, Zone::Hand(B));
    let hand_low = blank(&mut f, Color::Yellow, 2, Zone::Hand(B));
    blank(&mut f, Color::Purple, 4, Zone::Score(B));
    let score_low = blank(&mut f, Color::Red, 2, Zone::Score(B));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, inquisition, 0, B, A);
    let (status, _) = play(&mut game, |menu| panic!("no choice expected: {menu:?}"));
    complete(&status);
    // Returns go to the bottom one after another: the later one is lowest.
    assert_eq!(game.position().cards(deck), [score_low, hand_low, existing]);
}

/// Pressure Cooker draws, then junks an achievement of each bonus's value, in the
/// player's order: an earlier draw can inform a later junk among achievements with
/// different backs, so both the order and the back stay choices.
#[test]
fn pressure_cooker_keeps_its_order_and_back_choices_with_several_backs() {
    let mut f = Fixture::default();
    let cooker = f.add(echoes_card("E048").unwrap(), Zone::Hand(A));
    let two = blank(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    f.cards[usize::from(two.0)].symbols[0] = Symbol::Bonus(2);
    let five = blank(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
    f.cards[usize::from(five.0)].symbols[0] = Symbol::Bonus(5);
    let base_two = blank(&mut f, Color::Green, 2, Zone::AvailableAchievements);
    let city_two = city(&mut f, "C017", Zone::AvailableAchievements);
    let base_five = blank(&mut f, Color::Green, 5, Zone::AvailableAchievements);
    blank(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2)));
    blank(&mut f, Color::Yellow, 5, Zone::Deck(Deck::base(5)));
    let mut game = Game::new(f.position(), echoes_cities()).unwrap();
    effect(&mut game, cooker, 1, A, A);
    let mut events = Vec::new();
    let order = decision(&mut game, &mut events);
    assert_eq!(order.options.len(), 2);
    game.choose(pick_known(&order, two).unwrap()).unwrap();
    let junk = decision(&mut game, &mut events);
    let backs: Vec<CardSet> = junk
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::BlindMove { back, .. } => Some(back.set),
            _ => None,
        })
        .collect();
    assert_eq!(junk.options.len(), 2, "{:?}", junk.options);
    assert!(backs.contains(&CardSet::Base) && backs.contains(&CardSet::Cities));
    let cities = junk
        .options
        .iter()
        .position(|option| {
            matches!(option, Alternative::BlindMove { back, .. } if back.set == CardSet::Cities)
        })
        .unwrap();
    game.choose(cities).unwrap();
    let (status, _) = play(&mut game, |_| None);
    complete(&status);
    let position = game.position();
    assert_eq!(position.cards(Zone::AvailableAchievements), [base_two]);
    for card in [city_two, base_five] {
        assert!(position.cards(Zone::Junk).contains(&card));
    }
}

/// Maze: a victim who scores only some of the matching colors exchanges the rest of
/// their hand with the demander's score pile; the scored card stays scored.
#[test]
fn maze_scores_a_partial_match_then_exchanges_the_remaining_hand() {
    let mut f = Fixture::default();
    let maze = unseen(&mut f, "U002", Zone::Board(A, Color::Red));
    let demanding: Vec<_> = (0..2)
        .map(|_| blank(&mut f, Color::Yellow, 1, Zone::Hand(A)))
        .collect();
    let matching = blank(&mut f, Color::Yellow, 2, Zone::Hand(B));
    let red = blank(&mut f, Color::Red, 3, Zone::Hand(B));
    let blue = blank(&mut f, Color::Blue, 4, Zone::Hand(B));
    let scored: Vec<_> = (5..7)
        .map(|value| blank(&mut f, Color::Green, value, Zone::Score(A)))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, maze, 0, B, A);
    let (status, events) = play(&mut game, |_| None);
    complete(&status);
    let position = game.position();
    let sorted = |zone| {
        let mut cards = position.cards(zone).to_vec();
        cards.sort();
        cards
    };
    assert_eq!(sorted(Zone::Score(B)), [matching]);
    assert_eq!(sorted(Zone::Hand(B)), scored);
    assert_eq!(sorted(Zone::Score(A)), [red, blue]);
    assert_eq!(sorted(Zone::Hand(A)), demanding);
    // The demanding hand set the quota, so it was shown first.
    assert_eq!(revealed(&events)[..2], demanding);
}

/// Maze with an empty demanding hand: no quota, no exchange, no reveal.
#[test]
fn maze_with_an_empty_demanding_hand_does_nothing() {
    let mut f = Fixture::default();
    let maze = unseen(&mut f, "U002", Zone::Board(A, Color::Red));
    let held = blank(&mut f, Color::Red, 1, Zone::Hand(B));
    let scored = blank(&mut f, Color::Green, 2, Zone::Score(A));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, maze, 0, B, A);
    let (status, events) = play(&mut game, |_| None);
    complete(&status);
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(B)), [held]);
    assert!(position.cards(Zone::Score(B)).is_empty());
    assert_eq!(position.cards(Zone::Score(A)), [scored]);
    assert!(revealed(&events).is_empty());
}

/// Password with a full Safe: the safeguard fails, so the drawn card is not scored and
/// the rest of the hand returns; the card offered for the safeguard is never revealed.
#[test]
fn password_with_a_full_safe_returns_the_rest_of_the_hand_unrevealed() {
    let mut f = Fixture::default();
    let password = unseen(&mut f, "U017", Zone::Board(A, Color::Red));
    let safe: Vec<_> = (1..=5)
        .map(|value| blank(&mut f, Color::Purple, value, Zone::Safe(A)))
        .collect();
    let matching = blank(&mut f, Color::Blue, 1, Zone::Hand(A));
    let other = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    let drawn = blank(&mut f, Color::Blue, 2, Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    effect(&mut game, password, 0, A, A);
    let mut attempted = false;
    let (status, events) = play(&mut game, |menu| {
        let pick = pick_known(menu, matching);
        attempted |= pick.is_some();
        pick
    });
    complete(&status);
    assert!(attempted, "the safeguard of the matching card is attempted");
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), [drawn]);
    assert_eq!(position.cards(Zone::Safe(A)), safe);
    assert!(position.cards(Zone::Score(A)).is_empty());
    let one = position.cards(Zone::Deck(Deck::base(1)));
    assert!(one.contains(&matching) && one.contains(&other));
    assert_eq!(revealed(&events), [drawn]);
}
