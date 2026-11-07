use inno_engine::*;

mod common;
use common::*;

const ECHOES: Expansions = Expansions::ECHOES;

fn echoes(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(echoes_card(code).unwrap(), zone)
}

/// A printed Echoes card on top of its color's pile.
fn on_board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = echoes_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

/// A card whose own effect does nothing.
fn plain(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, Vec::new())
}

/// A plain card on top of its Base deck.
fn deck(f: &mut Fixture, color: Color, value: u8) -> CardId {
    plain(f, color, value, Zone::Deck(Deck::base(value)))
}

/// Blackmail (U040) on A's board, demanding of B.
fn add_blackmail(f: &mut Fixture) -> CardId {
    let card = unseen_card("U040").unwrap();
    let color = card.color;
    f.add(card, Zone::Board(A, color))
}

fn symbols(f: &mut Fixture, card: CardId, symbols: [Symbol; SLOTS]) {
    f.cards[usize::from(card.0)].symbols = symbols;
}

fn symbol(f: &mut Fixture, card: CardId, slot: usize, symbol: Symbol) {
    f.cards[usize::from(card.0)].symbols[slot] = symbol;
}

/// One effect (`Card::ECHO` for the echo), outside any Dogma action.
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

/// A Meld action of `driver`, which promotes a card from A's Forecast.
fn meld_action(game: &mut Game, driver: CardId) {
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
}

/// A card whose own effect is only `effect` of the printed card: no echo
/// and no other effects interfere with the scenario.
fn only_effect(code: &str, effect: usize) -> Card {
    let mut card = echoes_card(code).unwrap();
    card.echo = None;
    card.effects = vec![card.effects[effect].clone()];
    card
}

fn known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
            | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
            if *c == card
        )
    })
}

fn declined(menu: &Decision) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Decline)
}

/// Choose these cards first, in order, otherwise the first option.
fn prefer(cards: &[CardId]) -> impl FnMut(&Decision) -> usize + '_ {
    move |menu| {
        cards
            .iter()
            .find_map(|&card| known(menu, card))
            .unwrap_or(0)
    }
}

/// Decline whenever allowed, otherwise choose these cards first.
fn declining(cards: &[CardId]) -> impl FnMut(&Decision) -> usize + '_ {
    move |menu| {
        declined(menu)
            .or_else(|| cards.iter().find_map(|&card| known(menu, card)))
            .unwrap_or(0)
    }
}

/// Play to the next status other than a decision.
fn play(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut choose: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..300 {
        match game.advance(1000, events).unwrap() {
            Status::Decision(menu) => game.choose(choose(&menu)).unwrap(),
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("did not finish");
}

fn complete(game: &mut Game, choose: impl FnMut(&Decision) -> usize) {
    let status = play(game, &mut Vec::new(), choose);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

/// Where a card is, among the zones these scenarios use.
fn locate(position: &Position, card: CardId) -> Option<Zone> {
    let mut zones = vec![Zone::Junk, Zone::AvailableAchievements];
    for player in Player::ALL {
        zones.extend([
            Zone::Hand(player),
            Zone::Score(player),
            Zone::Forecast(player),
            Zone::Achievements(player),
        ]);
        zones.extend(Color::ALL.map(|color| Zone::Board(player, color)));
    }
    zones.extend((1..=11).map(|value| Zone::Deck(Deck::base(value))));
    zones
        .into_iter()
        .find(|&zone| position.cards(zone).contains(&card))
}

fn values(position: &Position, zone: Zone) -> Vec<u8> {
    let mut values: Vec<_> = position
        .cards(zone)
        .iter()
        .map(|&card| position.card(card).value)
        .collect();
    values.sort();
    values
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

/// Wristwatch visits "each value in ascending order": 1 to 11. A visible bonus 12
/// (Esports) is not a value, so no [12] is drawn and the game goes on.
#[test]
fn wristwatch_ignores_a_visible_bonus_twelve() {
    let mut f = Fixture::default();
    let watch = echoes(&mut f, "E089", Zone::Junk);
    on_board(&mut f, "E109", A);
    let two = plain(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    symbol(&mut f, two, 0, Symbol::Bonus(2));
    let drawn = deck(&mut f, Color::Blue, 2);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    effect(&mut game, watch, 1, A, A);
    complete(&mut game, |_| 0);
    assert_eq!(game.position().top(A, Color::Blue), Some(drawn));
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

/// Wristwatch's echo tucks a top card under its own pile. The card never leaves the
/// pile, so a two-card pile keeps its splay.
#[test]
fn wristwatch_echo_keeps_a_two_card_piles_splay() {
    let mut f = Fixture::default();
    let watch = echoes(&mut f, "E089", Zone::Junk);
    let bottom = plain(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let top = plain(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    let position = f.position().with_splay(A, Color::Red, Splay::Up).unwrap();
    let mut game = Game::new(position, ECHOES).unwrap();
    effect(&mut game, watch, Card::ECHO, A, A);
    complete(&mut game, |_| 0);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [top, bottom]
    );
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
}

/// Foreseen Rubber checks Forecast capacity before the top card leaves its pile: a full
/// Forecast refuses the top of the up-splayed pile that sets its limit, although
/// removing it would unsplay that pile and raise the limit.
#[test]
fn foreseen_rubber_cannot_foreshadow_the_top_that_sets_a_full_forecasts_limit() {
    for full in [false, true] {
        let mut f = Fixture::default();
        let rubber = f.add(only_effect("E067", 2), Zone::Forecast(A));
        let driver = plain(&mut f, Color::Purple, 8, Zone::Hand(A));
        plain(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        let blue = plain(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
        // The up splay limits the Forecast to two; promoting Rubber leaves
        // one card, or two when full.
        for _ in 0..if full { 2 } else { 1 } {
            plain(&mut f, Color::Green, 10, Zone::Forecast(A));
        }
        let position = f.position().with_splay(A, Color::Blue, Splay::Up).unwrap();
        let mut game = Game::new(position, ECHOES).unwrap();
        meld_action(&mut game, driver);
        complete(&mut game, prefer(&[blue, rubber]));
        let position = game.position();
        assert_eq!(
            locate(position, blue),
            Some(if full {
                Zone::Board(A, Color::Blue)
            } else {
                Zone::Forecast(A)
            })
        );
        assert_eq!(
            position.splay(A, Color::Blue),
            if full { Splay::Up } else { Splay::None }
        );
        assert_eq!(position.cards(Zone::Forecast(A)).len(), 2);
    }
}

/// Barcode: "You may splay any one color of your cards up. If Barcode was foreseen,
/// splay all your colors up." The foreseen splay follows even a declined optional one.
#[test]
fn barcode_splays_one_color_optionally_and_every_color_when_foreseen() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let barcode = echoes(&mut f, "E101", Zone::Junk);
        for color in [Color::Red, Color::Yellow] {
            for value in [1, 2] {
                plain(&mut f, color, value, Zone::Board(A, color));
            }
        }
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        effect(&mut game, barcode, 1, A, A);
        complete(&mut game, |menu| {
            if accept {
                menu.options
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
                    .unwrap()
            } else {
                declined(menu).unwrap()
            }
        });
        let position = game.position();
        assert_eq!(
            position.splay(A, Color::Red),
            if accept { Splay::Up } else { Splay::None }
        );
        assert_eq!(position.splay(A, Color::Yellow), Splay::None);
    }

    let mut f = Fixture::default();
    let barcode = f.add(only_effect("E101", 1), Zone::Forecast(A));
    let driver = plain(&mut f, Color::Purple, 10, Zone::Hand(A));
    for color in [Color::Red, Color::Yellow] {
        for value in [1, 2] {
            plain(&mut f, color, value, Zone::Board(A, color));
        }
    }
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    meld_action(&mut game, driver);
    complete(&mut game, declining(&[barcode]));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::Up);
}

/// Karaoke self-executes "the melded card" wherever it is: here its first effect has
/// already moved the card, a bottom card, to the hand. Without an available achievement
/// of the melded card's value nothing is claimed or executed.
#[test]
fn karaoke_self_executes_its_echo_meld_from_the_hand_only_after_a_claim() {
    for matching in [false, true] {
        let mut f = Fixture::default();
        plain(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
        let karaoke = on_board(&mut f, "E094", A);
        plain(&mut f, Color::Green, 2, Zone::Hand(A));
        let one = deck(&mut f, Color::Red, 1);
        let melded = f.tech(
            Color::Blue,
            2,
            Zone::Deck(Deck::base(2)),
            vec![build::draw(1)],
        );
        // A has no score: the claim ignores eligibility.
        let achievement = plain(
            &mut f,
            Color::Yellow,
            if matching { 2 } else { 3 },
            Zone::AvailableAchievements,
        );
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        dogma(&mut game, karaoke);
        complete(&mut game, |_| 0);
        let position = game.position();
        assert_eq!(locate(position, melded), Some(Zone::Hand(A)));
        assert_eq!(
            position.cards(Zone::Achievements(A)).contains(&achievement),
            matching
        );
        assert_eq!(
            locate(position, one),
            Some(if matching {
                Zone::Hand(A)
            } else {
                Zone::Deck(Deck::base(1))
            })
        );
    }
}

/// Stethoscope's "if you melded a blue card due to Stethoscope's echo effect" reads
/// each player's own echo meld: the sharer melded yellow and draws only a [7]; the
/// owner melded blue and also draws an [8].
#[test]
fn stethoscope_reads_each_sharing_players_own_echo_meld() {
    let mut f = Fixture::default();
    let stethoscope = on_board(&mut f, "E063", A);
    let sharer = plain(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
    symbols(&mut f, sharer, [Symbol::Icon(Icon::Concept); SLOTS]);
    let blue = plain(&mut f, Color::Blue, 1, Zone::Hand(A));
    let yellow = plain(&mut f, Color::Yellow, 1, Zone::Hand(B));
    // A's share draw, at A's new top blue value.
    deck(&mut f, Color::Green, 1);
    for _ in 0..2 {
        deck(&mut f, Color::Green, 7);
        deck(&mut f, Color::Green, 8);
    }
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    dogma(&mut game, stethoscope);
    complete(&mut game, |_| 0);
    let position = game.position();
    assert_eq!(position.top(A, Color::Blue), Some(blue));
    assert_eq!(position.top(B, Color::Yellow), Some(yellow));
    assert_eq!(values(position, Zone::Hand(B)), [7]);
    assert_eq!(values(position, Zone::Hand(A)), [1, 7, 8]);
}

/// Dentures scores in the color of "the last card you tucked due to Dentures": the
/// sharer and the owner each use their own echo's tuck.
#[test]
fn dentures_scores_each_sharing_players_own_tucked_color() {
    let mut f = Fixture::default();
    let dentures = on_board(&mut f, "E058", A);
    let green = [
        plain(&mut f, Color::Green, 1, Zone::Board(A, Color::Green)),
        plain(&mut f, Color::Green, 2, Zone::Board(A, Color::Green)),
    ];
    let red = [
        plain(&mut f, Color::Red, 1, Zone::Board(B, Color::Red)),
        plain(&mut f, Color::Red, 2, Zone::Board(B, Color::Red)),
    ];
    symbols(&mut f, red[1], [Symbol::Icon(Icon::Industry); SLOTS]);
    // Deck order: A's share draw, then A's tuck, then the sharer's on top.
    deck(&mut f, Color::Yellow, 6);
    let owners = deck(&mut f, Color::Green, 6);
    let sharers = deck(&mut f, Color::Red, 6);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    dogma(&mut game, dentures);
    complete(&mut game, |_| 0);
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(B)), [red[1], red[0]]);
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), [sharers]);
    assert_eq!(position.cards(Zone::Score(A)), [green[1], green[0]]);
    assert_eq!(position.cards(Zone::Board(A, Color::Green)), [owners]);
}

/// Karaoke claims by the value of the card each player's own echo melded, sharer and
/// owner alike.
#[test]
fn karaoke_claims_by_each_sharing_players_own_echo_meld() {
    let mut f = Fixture::default();
    let karaoke = on_board(&mut f, "E094", A);
    let sharer = plain(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
    symbols(&mut f, sharer, [Symbol::Icon(Icon::Health); SLOTS]);
    plain(&mut f, Color::Green, 1, Zone::Hand(B));
    plain(&mut f, Color::Green, 2, Zone::Hand(A));
    // A's share draw from an emptied board, then the sharer's [1].
    deck(&mut f, Color::Purple, 1);
    deck(&mut f, Color::Blue, 1);
    deck(&mut f, Color::Yellow, 2);
    let one = plain(&mut f, Color::Purple, 1, Zone::AvailableAchievements);
    let two = plain(&mut f, Color::Purple, 2, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    dogma(&mut game, karaoke);
    complete(&mut game, |_| 0);
    let position = game.position();
    assert_eq!(position.cards(Zone::Achievements(B)), [one]);
    assert_eq!(position.cards(Zone::Achievements(A)), [two]);
}

/// Jet's demand reads "the color I last melded due to Jet's echo effect during this
/// action": the record ends with its action, so a later Dogma whose echo melds nothing
/// returns nothing.
#[test]
fn jets_echo_meld_record_ends_with_its_action() {
    let mut f = Fixture::default();
    let jet = on_board(&mut f, "E087", A);
    plain(&mut f, Color::Purple, 3, Zone::Hand(A));
    let kept = plain(&mut f, Color::Purple, 1, Zone::Board(B, Color::Purple));
    let returned = plain(&mut f, Color::Purple, 2, Zone::Board(B, Color::Purple));
    for _ in 0..2 {
        deck(&mut f, Color::Green, 10);
    }
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    dogma(&mut game, jet);
    complete(&mut game, |_| 0);
    assert_eq!(
        locate(game.position(), returned),
        Some(Zone::Deck(Deck::base(2)))
    );
    // The hand is empty now: the second echo melds nothing.
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    dogma(&mut game, jet);
    complete(&mut game, |_| 0);
    // The second Dogma ran: it foreshadowed its own [10].
    assert_eq!(game.position().cards(Zone::Forecast(A)).len(), 2);
    assert_eq!(game.position().cards(Zone::Board(B, Color::Purple)), [kept]);
}

/// Jet's junk does not depend on its return: with no top of the melded color to return,
/// achievements of the melded card's value are still junked; with no echo meld, nothing
/// is returned or junked.
#[test]
fn jet_junks_the_melded_value_without_a_return_and_nothing_without_a_meld() {
    let mut f = Fixture::default();
    let jet = on_board(&mut f, "E087", A);
    plain(&mut f, Color::Purple, 4, Zone::Hand(A));
    let four = plain(&mut f, Color::Green, 4, Zone::AvailableAchievements);
    let five = plain(&mut f, Color::Green, 5, Zone::AvailableAchievements);
    deck(&mut f, Color::Green, 10);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    dogma(&mut game, jet);
    complete(&mut game, |_| 0);
    assert_eq!(locate(game.position(), four), Some(Zone::Junk));
    assert_eq!(
        locate(game.position(), five),
        Some(Zone::AvailableAchievements)
    );

    let mut f = Fixture::default();
    let jet = on_board(&mut f, "E087", A);
    let purple = plain(&mut f, Color::Purple, 2, Zone::Board(B, Color::Purple));
    let two = plain(&mut f, Color::Green, 2, Zone::AvailableAchievements);
    deck(&mut f, Color::Green, 10);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    dogma(&mut game, jet);
    complete(&mut game, |_| 0);
    assert_eq!(
        locate(game.position(), purple),
        Some(Zone::Board(B, Color::Purple))
    );
    assert_eq!(
        locate(game.position(), two),
        Some(Zone::AvailableAchievements)
    );
}

/// The Executor's blue pile, wherever Stethoscope lies in it.
const STETHOSCOPE: Selector = build::filtered(
    &Selector::Board(
        Who::Executor,
        ColorSet::Only(&[Color::Blue]),
        BoardRegion::BelowTop(0),
    ),
    &[Filter::Has(Test::Code("E063"))],
);

/// Only a blue card melded by Stethoscope's own echo earns the [8]. Another card melds
/// one card from hand, then self-executes Stethoscope, whose echo melds the other: a
/// blue meld by the other card in the same action does not count.
#[test]
fn stethoscope_ignores_a_blue_meld_by_another_card_in_the_same_action() {
    for other_melds_blue in [false, true] {
        let mut f = Fixture::default();
        on_board(&mut f, "E063", A);
        let driver = f.tech(
            Color::Green,
            1,
            Zone::Board(A, Color::Green),
            vec![
                build::meld(build::HAND),
                build::bind(STETHOSCOPE),
                build::execute(ExecutionMode::SelfExecute, Who::Executor),
            ],
        );
        // More Authority than B: B does not share.
        symbol(&mut f, driver, 0, Symbol::Icon(Icon::Authority));
        let blue = plain(&mut f, Color::Blue, 1, Zone::Hand(A));
        let yellow = plain(&mut f, Color::Yellow, 1, Zone::Hand(A));
        let seven = deck(&mut f, Color::Red, 7);
        let eight = deck(&mut f, Color::Red, 8);
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        dogma(&mut game, driver);
        complete(
            &mut game,
            prefer(&[if other_melds_blue { blue } else { yellow }]),
        );
        let position = game.position();
        assert_eq!(locate(position, seven), Some(Zone::Hand(A)));
        assert_eq!(
            locate(position, eight),
            Some(if other_melds_blue {
                Zone::Deck(Deck::base(8))
            } else {
                Zone::Hand(A)
            })
        );
    }
}

/// With no blue or yellow card in hand, Stethoscope's echo reveals the hand to prove
/// its meld impossible.
#[test]
fn stethoscope_echo_reveals_a_hand_without_blue_or_yellow() {
    let mut f = Fixture::default();
    let stethoscope = on_board(&mut f, "E063", A);
    let red = plain(&mut f, Color::Red, 1, Zone::Hand(A));
    let green = plain(&mut f, Color::Green, 2, Zone::Hand(A));
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    effect(&mut game, stethoscope, Card::ECHO, A, A);
    let mut events = Vec::new();
    let status = play(&mut game, &mut events, |_| 0);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    let mut shown = revealed(&events);
    shown.sort();
    assert_eq!(shown, [red, green]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [red, green]);
}

/// "For each odd bonus on your board, return the lowest card from your hand" finds the
/// lowest card anew for each bonus: three odd bonuses return both [1]s and then the
/// [2].
#[test]
fn crossword_returns_the_live_lowest_hand_card_for_each_odd_bonus() {
    let mut f = Fixture::default();
    let crossword = echoes(&mut f, "E084", Zone::Junk);
    for (color, bonus) in [(Color::Red, 1), (Color::Yellow, 3), (Color::Green, 5)] {
        let top = plain(&mut f, color, 1, Zone::Board(A, color));
        symbol(&mut f, top, 0, Symbol::Bonus(bonus));
    }
    let ones = [
        plain(&mut f, Color::Blue, 1, Zone::Hand(A)),
        plain(&mut f, Color::Purple, 1, Zone::Hand(A)),
    ];
    let two = plain(&mut f, Color::Blue, 2, Zone::Hand(A));
    let three = plain(&mut f, Color::Blue, 3, Zone::Hand(A));
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    effect(&mut game, crossword, 1, A, A);
    complete(&mut game, |_| 0);
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), [three]);
    assert_eq!(locate(position, two), Some(Zone::Deck(Deck::base(2))));
    for one in ones {
        assert_eq!(locate(position, one), Some(Zone::Deck(Deck::base(1))));
    }
}

/// Calculator draws and repeats only after scoring two cards totalling less than 12: a
/// single scored card, or a pair totalling 12, draws nothing and does not repeat.
#[test]
fn calculator_needs_two_scored_cards_below_twelve_to_draw() {
    // One non-blue color: a single card is scored.
    let mut f = Fixture::default();
    let calculator = echoes(&mut f, "E092", Zone::Junk);
    let red = plain(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    for value in [1, 2] {
        plain(&mut f, Color::Blue, value, Zone::Board(A, Color::Blue));
    }
    let one = deck(&mut f, Color::Green, 1);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    effect(&mut game, calculator, 0, A, A);
    complete(&mut game, |_| 0);
    assert_eq!(game.position().cards(Zone::Score(A)), [red]);
    assert_eq!(
        locate(game.position(), one),
        Some(Zone::Deck(Deck::base(1)))
    );

    // Two sixes total 12, not less: no draw and no second pair.
    let mut f = Fixture::default();
    let calculator = echoes(&mut f, "E092", Zone::Junk);
    let mut sixes = Vec::new();
    let mut ones = Vec::new();
    for color in [Color::Red, Color::Yellow] {
        sixes.push(plain(&mut f, color, 6, Zone::Board(A, color)));
        ones.push(plain(&mut f, color, 1, Zone::Board(A, color)));
    }
    deck(&mut f, Color::Green, 2);
    let mut game = Game::new(f.position(), ECHOES).unwrap();
    effect(&mut game, calculator, 0, A, A);
    complete(&mut game, |_| 0);
    let position = game.position();
    let mut scored = position.cards(Zone::Score(A)).to_vec();
    scored.sort();
    assert_eq!(scored, sixes);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), [ones[0]]);
    assert_eq!(position.cards(Zone::Board(A, Color::Yellow)), [ones[1]]);
    assert!(position.cards(Zone::Hand(A)).is_empty());
}

/// Morphine's echo scores an odd-valued hand card; even values never qualify.
#[test]
fn morphine_echo_scores_only_an_odd_hand_card() {
    for with_odd in [false, true] {
        let mut f = Fixture::default();
        let morphine = echoes(&mut f, "E059", Zone::Junk);
        let even = [
            plain(&mut f, Color::Red, 2, Zone::Hand(A)),
            plain(&mut f, Color::Blue, 4, Zone::Hand(A)),
        ];
        let odd = with_odd.then(|| plain(&mut f, Color::Green, 3, Zone::Hand(A)));
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        effect(&mut game, morphine, Card::ECHO, A, A);
        complete(&mut game, |_| 0);
        let position = game.position();
        assert_eq!(position.cards(Zone::Score(A)), Vec::from_iter(odd));
        assert_eq!(position.cards(Zone::Hand(A)), even);
    }
}

/// Cell Phone draws a [10] for every color on your board with a visible [EFFICIENCY],
/// including one a splay shows on a covered card, but not a covered one it hides.
#[test]
fn cell_phone_draws_a_ten_for_each_color_showing_efficiency() {
    let mut f = Fixture::default();
    let phone = echoes(&mut f, "E098", Zone::Junk);
    let red = plain(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    symbol(&mut f, red, 0, Symbol::Icon(Icon::Efficiency));
    let shown = plain(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
    symbol(&mut f, shown, 2, Symbol::Icon(Icon::Efficiency));
    plain(&mut f, Color::Green, 2, Zone::Board(A, Color::Green));
    let hidden = plain(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
    symbol(&mut f, hidden, 0, Symbol::Icon(Icon::Efficiency));
    plain(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
    plain(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
    for _ in 0..4 {
        deck(&mut f, Color::Purple, 10);
    }
    let position = f.position().with_splay(A, Color::Green, Splay::Up).unwrap();
    let mut game = Game::new(position, ECHOES).unwrap();
    effect(&mut game, phone, 0, A, A);
    complete(&mut game, |_| 0);
    assert_eq!(values(game.position(), Zone::Hand(A)), [10, 10]);
}

/// Sudoku's echo "You may tuck any number of cards from your hand" may tuck none, or
/// stop after any tuck.
#[test]
fn sudoku_echo_may_stop_after_any_number_of_tucks() {
    for tucks in [0, 1] {
        let mut f = Fixture::default();
        let sudoku = echoes(&mut f, "E105", Zone::Junk);
        let red = plain(&mut f, Color::Red, 1, Zone::Hand(A));
        let blue = plain(&mut f, Color::Blue, 2, Zone::Hand(A));
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        effect(&mut game, sudoku, Card::ECHO, A, A);
        let mut done = 0;
        complete(&mut game, |menu| {
            if done == tucks {
                declined(menu).unwrap()
            } else {
                done += 1;
                known(menu, red).unwrap()
            }
        });
        let position = game.position();
        assert_eq!(
            locate(position, red),
            Some(if tucks == 1 {
                Zone::Board(A, Color::Red)
            } else {
                Zone::Hand(A)
            })
        );
        assert_eq!(locate(position, blue), Some(Zone::Hand(A)));
    }
}

/// Under Blackmail's "must", "you may tuck any number" tucks at least one card:
/// Sudoku's echo and Cell Phone's third effect. Unblackmailed, the same player
/// declines.
#[test]
fn blackmailed_sudoku_and_cell_phone_tuck_at_least_one_card() {
    let both = Expansions::UNSEEN.union(ECHOES);

    // Blackmail melds Sudoku and self-executes it: its echo must tuck.
    let mut f = Fixture::default();
    let blackmail = add_blackmail(&mut f);
    let sudoku = echoes(&mut f, "E105", Zone::Hand(B));
    let tucked = plain(&mut f, Color::Red, 1, Zone::Hand(B));
    // Sudoku's dogma draws and melds a [1]; executing it earns Chain's [11].
    deck(&mut f, Color::Green, 1);
    deck(&mut f, Color::Green, 11);
    let mut game = Game::new(f.position(), both).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    // Sudoku's draw takes the [1].
    let mut choose = declining(std::slice::from_ref(&sudoku));
    complete(&mut game, |menu| {
        menu.options
            .iter()
            .position(|option| *option == Alternative::Value(1))
            .unwrap_or_else(|| choose(menu))
    });
    assert_eq!(
        locate(game.position(), tucked),
        Some(Zone::Board(B, Color::Red))
    );

    // Cell Phone: its third effect must tuck the one Efficiency card.
    for blackmailed in [false, true] {
        let mut f = Fixture::default();
        let blackmail = blackmailed.then(|| add_blackmail(&mut f));
        let phone = echoes(
            &mut f,
            "E098",
            if blackmailed {
                Zone::Hand(B)
            } else {
                Zone::Junk
            },
        );
        let efficient = plain(&mut f, Color::Red, 1, Zone::Hand(B));
        symbol(&mut f, efficient, 0, Symbol::Icon(Icon::Efficiency));
        // The echo's foreshadowed [11] and one [10] for yellow Cell Phone.
        deck(&mut f, Color::Green, 11);
        deck(&mut f, Color::Green, 10);
        let mut game = Game::new(f.position(), both).unwrap();
        match blackmail {
            Some(blackmail) => effect(&mut game, blackmail, 0, B, A),
            None => effect(&mut game, phone, 2, B, B),
        }
        complete(&mut game, declining(&[phone]));
        assert_eq!(
            locate(game.position(), efficient),
            Some(if blackmailed {
                Zone::Board(B, Color::Red)
            } else {
                Zone::Hand(B)
            })
        );
    }
}

/// Foreseen Credit Card repeats only after scoring a card of the transferred card's
/// value: an age-skipped draw is still scored but ends the repeats.
#[test]
fn foreseen_credit_card_stops_repeating_after_an_age_skipped_draw() {
    for skipped in [false, true] {
        let mut f = Fixture::default();
        let credit = f.add(only_effect("E090", 0), Zone::Forecast(A));
        let driver = plain(&mut f, Color::Green, 9, Zone::Hand(A));
        let red = plain(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        let yellow = plain(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
        if !skipped {
            for _ in 0..2 {
                deck(&mut f, Color::Blue, 1);
            }
        }
        for _ in 0..2 {
            deck(&mut f, Color::Blue, 2);
        }
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        meld_action(&mut game, driver);
        complete(&mut game, prefer(&[credit, red, yellow]));
        let position = game.position();
        assert_eq!(locate(position, red), Some(Zone::Hand(A)));
        assert_eq!(
            locate(position, yellow),
            Some(if skipped {
                Zone::Board(A, Color::Yellow)
            } else {
                Zone::Hand(A)
            })
        );
        assert_eq!(
            values(position, Zone::Score(A)),
            if skipped { vec![2] } else { vec![1, 1] }
        );
    }
}

/// Foreseen Esports wins only if every draw-and-score took the top card's value: an
/// age-skipped draw blocks the win.
#[test]
fn foreseen_esports_does_not_win_after_an_age_skipped_draw() {
    for skipped in [false, true] {
        let mut f = Fixture::default();
        let esports = echoes(&mut f, "E109", Zone::Forecast(A));
        let driver = plain(&mut f, Color::Yellow, 11, Zone::Hand(A));
        plain(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        plain(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
        if !skipped {
            deck(&mut f, Color::Green, 1);
        }
        for _ in 0..2 {
            deck(&mut f, Color::Green, 2);
        }
        let mut game = Game::new(f.position(), ECHOES).unwrap();
        meld_action(&mut game, driver);
        let status = play(&mut game, &mut Vec::new(), prefer(&[esports]));
        assert_eq!(
            matches!(status, Status::Winner(winner) if winner == A),
            !skipped,
            "{status:?}"
        );
        assert_eq!(
            values(game.position(), Zone::Score(A)),
            if skipped { vec![2, 2] } else { vec![1, 2] }
        );
    }
}
