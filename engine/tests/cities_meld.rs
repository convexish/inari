use inno_engine::*;

mod common;
use common::*;

fn city(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(city_card(code).unwrap(), zone)
}

fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, vec![])
}

fn figure(f: &mut Fixture, figure: Figure, player: Player) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let zone = Zone::Board(player, card.color);
    f.add(card, zone)
}

fn cities() -> Expansions {
    Expansions::CITIES
}

fn with_figures() -> Expansions {
    Expansions::FIGURES.union(Expansions::CITIES)
}

/// Settle, answering each decision with `decide`; fails on a decision
/// `decide` declines.
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

fn meld_action(game: &mut Game, card: CardId) {
    game.take_action(Action::Meld { card }, ActionKind::Regular)
        .unwrap();
}

/// Each physical move as (verb, card, source, destination), with reveals as
/// `Reveal` entries and splays as `Splay` entries, in event order.
#[derive(Debug, PartialEq)]
enum Step {
    Move(Option<Verb>, CardId, Zone, Zone),
    Reveal(CardId),
    Splay(Player, Color, Splay),
    Special(Special),
}

fn steps(events: &[Event]) -> Vec<Step> {
    events
        .iter()
        .filter_map(|event| match *event {
            Event::Moved(Movement {
                verb,
                card,
                source,
                destination,
                ..
            }) => Some(Step::Move(verb, card, source, destination)),
            Event::Revealed { card, .. } => Some(Step::Reveal(card)),
            Event::Splayed {
                player, color, to, ..
            } => Some(Step::Splay(player, color, to)),
            Event::SpecialMoved { special, .. } => Some(Step::Special(special)),
            _ => None,
        })
        .collect()
}

#[test]
fn milan_runs_plus_then_uplift_after_the_meld() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let milan = city(&mut f, "C036", Zone::Hand(A));
    let under = blank(&mut f, Color::Green, 5, Zone::Deck(Deck::base(5)));
    let five = blank(&mut f, Color::Green, 5, Zone::Deck(Deck::base(5)));
    let six = blank(&mut f, Color::Green, 6, Zone::Deck(Deck::base(6)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    meld_action(&mut game, milan);
    let (status, events) = play(&mut game, |_| None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let deck = |age| Zone::Deck(Deck::base(age));
    assert_eq!(
        steps(&events),
        [
            Step::Move(
                Some(Verb::Meld),
                milan,
                Zone::Hand(A),
                Zone::Board(A, Color::Red)
            ),
            // Plus (top-center): draw a 5.
            Step::Move(None, five, deck(5), Zone::Hand(A)),
            // Uplift (bottom-center): junk the 5 deck, then draw a 6.
            Step::Move(Some(Verb::Junk), under, deck(5), Zone::Junk),
            Step::Move(None, six, deck(6), Zone::Hand(A)),
        ]
    );
}

#[test]
fn lisbon_draws_twice_and_the_second_draw_routes_again() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
    let lisbon = city(&mut f, "C039", Zone::Hand(A));
    let five = blank(&mut f, Color::Green, 5, Zone::Deck(Deck::base(5)));
    let six = blank(&mut f, Color::Green, 6, Zone::Deck(Deck::base(6)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    meld_action(&mut game, lisbon);
    play(&mut game, |_| None);
    // The second Plus finds the 5 deck empty and escalates.
    assert_eq!(game.position().cards(Zone::Hand(A)), [five, six]);
}

#[test]
fn bangalore_unsplays_the_opponent_before_its_up_splay_draws_a_city() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
    let bangalore = city(&mut f, "C103", Zone::Hand(A));
    blank(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
    blank(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
    blank(&mut f, Color::Green, 10, Zone::Deck(Deck::base(10)));
    let drawn = city(&mut f, "C096", Zone::Deck(Deck::cities(10)));
    let position = f
        .position()
        .with_splay(B, Color::Blue, Splay::Right)
        .unwrap();
    let mut game = Game::new(position, cities()).unwrap();
    meld_action(&mut game, bangalore);
    let (_, events) = play(&mut game, |_| None);
    assert_eq!(
        steps(&events)[1..],
        [
            Step::Splay(B, Color::Blue, Splay::None),
            Step::Splay(A, Color::Blue, Splay::Up),
            Step::Move(None, drawn, Zone::Deck(Deck::cities(10)), Zone::Hand(A)),
        ]
    );
}

#[test]
fn effect_melds_run_no_city_icons_and_no_new_color_draw() {
    let mut f = Fixture::default();
    let milan = city(&mut f, "C036", Zone::Hand(A));
    blank(&mut f, Color::Green, 5, Zone::Deck(Deck::base(5)));
    city(&mut f, "C039", Zone::Deck(Deck::cities(4)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    game.start(meld(milan, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    let (_, events) = play(&mut game, |_| None);
    assert_eq!(steps(&events).len(), 1);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn a_meld_action_starting_a_color_draws_a_city_for_any_set() {
    for (existing, draws) in [(false, true), (true, false)] {
        let mut f = Fixture::default();
        blank(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
        if existing {
            blank(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        }
        let card = blank(&mut f, Color::Green, 1, Zone::Hand(A));
        blank(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2)));
        let two = city(&mut f, "C016", Zone::Deck(Deck::cities(2)));
        let mut game = Game::new(f.position(), cities()).unwrap();
        meld_action(&mut game, card);
        play(&mut game, |_| None);
        let expected: Vec<CardId> = if draws { vec![two] } else { Vec::new() };
        assert_eq!(game.position().cards(Zone::Hand(A)), expected, "{existing}");
    }
}

/// Mecca (purple 3) searches for Health; the 3 deck holds, bottom to top,
/// an unrevealed card, P1, P2 (Health) and P3.
fn mecca(expansions: Expansions, hawking: bool) -> (Game, CardId, [CardId; 4]) {
    let mut f = Fixture::default();
    blank(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
    if hawking {
        figure(&mut f, Figure::StephenHawking, A);
    }
    let mecca = city(&mut f, "C035", Zone::Hand(A));
    let deck = Zone::Deck(Deck::base(3));
    let rest = blank(&mut f, Color::Red, 3, deck);
    let p1 = blank(&mut f, Color::Red, 3, deck);
    let p2 = blank(&mut f, Color::Red, 3, deck);
    let p3 = blank(&mut f, Color::Red, 3, deck);
    f.cards[usize::from(p2.0)].symbols[0] = Symbol::Icon(Icon::Health);
    // A Figure-free Health elsewhere on the board does not make P1 match.
    let mut game = Game::new(f.position(), expansions).unwrap();
    if hawking {
        game = game.with_tracking().unwrap();
    }
    (game, mecca, [rest, p1, p2, p3])
}

fn return_first(card: CardId) -> impl FnMut(&Decision) -> Option<usize> {
    move |decision| {
        decision.options.iter().position(|option| {
            matches!(option, Alternative::Move {
                verb: Verb::Return,
                target: TargetDescriptor::Known { card: c, .. },
            } if *c == card)
        })
    }
}

#[test]
fn mecca_reveals_its_packet_takes_matches_and_returns_the_rest_in_order() {
    for (first, order) in [(1, [3, 1, 0]), (3, [1, 3, 0])] {
        let (mut game, mecca, cards) = mecca(cities(), false);
        meld_action(&mut game, mecca);
        let (status, events) = play(&mut game, return_first(cards[first]));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let deck = Zone::Deck(Deck::base(3));
        let steps = steps(&events);
        // Every reveal precedes the take, in top order.
        assert_eq!(
            steps[1..5],
            [
                Step::Reveal(cards[3]),
                Step::Reveal(cards[2]),
                Step::Reveal(cards[1]),
                Step::Move(Some(Verb::Transfer), cards[2], deck, Zone::Hand(A)),
            ]
        );
        assert_eq!(game.position().cards(Zone::Hand(A)), [cards[2]]);
        // Returns go to the bottom one after another: the second returned
        // card ends up lowest.
        assert_eq!(
            game.position().cards(deck),
            order.map(|i| cards[i]),
            "{first}"
        );
    }
}

#[test]
fn search_takes_no_karma_and_ordinary_draws_still_do() {
    // Hawking: "If you would draw a card, first draw and tuck a card of the
    // same value." Search arrivals are not draws.
    let (mut game, mecca, cards) = mecca(with_figures(), true);
    meld_action(&mut game, mecca);
    let (_, events) = play(&mut game, return_first(cards[1]));
    assert!(
        !steps(&events)
            .iter()
            .any(|step| matches!(step, Step::Move(Some(Verb::Tuck), ..)))
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), [cards[2]]);
}

#[test]
fn a_short_or_empty_search_deck_does_not_escalate() {
    for size in [0, 1] {
        let mut f = Fixture::default();
        blank(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
        let mecca = city(&mut f, "C035", Zone::Hand(A));
        let mut packet = Vec::new();
        for _ in 0..size {
            packet.push(blank(&mut f, Color::Red, 3, Zone::Deck(Deck::base(3))));
        }
        let four = blank(&mut f, Color::Red, 4, Zone::Deck(Deck::base(4)));
        let mut game = Game::new(f.position(), cities()).unwrap();
        meld_action(&mut game, mecca);
        let (_, events) = play(&mut game, |_| None);
        let reveals = steps(&events)
            .into_iter()
            .filter(|step| matches!(step, Step::Reveal(_)))
            .count();
        assert_eq!(reveals, size);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), packet);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), [four]);
    }
}

#[test]
fn a_search_return_choice_resumes_identically_after_clone_and_sampling() {
    let (mut game, mecca, cards) = mecca(with_figures(), true);
    meld_action(&mut game, mecca);
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.options.len(), 2);
    let deck = Zone::Deck(Deck::base(3));
    let mut world = game.determinize(A, 11).unwrap();
    assert_eq!(decision(&mut world, &mut Vec::new()), menu);
    let mut clone = game.clone();
    for game in [&mut game, &mut world, &mut clone] {
        play(game, return_first(cards[3]));
    }
    assert_eq!(game, clone);
    // The returned packet is public: both worlds put it at the bottom alike.
    for game in [&game, &world] {
        assert_eq!(game.position().cards(deck)[..2], [cards[1], cards[3]]);
    }
}

#[test]
fn zhang_heng_scoring_dublin_on_its_claim_skips_its_icons() {
    // Yucata thread: melding Dublin claimed Repute; Zhang Heng's "would
    // achieve" karma scored Dublin, so its icons were skipped.
    let mut f = Fixture::default();
    figure(&mut f, Figure::ZhangHeng, A);
    blank(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
    blank(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
    let dublin = city(&mut f, "C058", Zone::Hand(A));
    let tucked = blank(&mut f, Color::Yellow, 3, Zone::Deck(Deck::base(3)));
    let six = blank(&mut f, Color::Green, 6, Zone::AvailableAchievements);
    let position = f
        .position()
        .with_splay(A, Color::Yellow, Splay::Right)
        .unwrap();
    let mut game = Game::new(position, with_figures()).unwrap();
    meld_action(&mut game, dublin);
    play(&mut game, |_| None);
    assert_eq!(
        game.position().special_location(Special::Repute),
        SpecialLocation::Owned(A)
    );
    assert!(game.position().cards(Zone::Score(A)).contains(&dublin));
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Yellow)),
        [tucked]
    );
    // Dublin's Junk icon never ran.
    assert_eq!(game.position().cards(Zone::AvailableAchievements), [six]);
}

#[test]
fn a_melded_figures_when_follows_the_new_color_city_draw() {
    let mut f = Fixture::default();
    let tigernmas = figure(&mut f, Figure::Tigernmas, B);
    let alexander = figure_card(Figure::AlexanderTheGreat.code()).unwrap();
    let alexander = f.add(alexander, Zone::Hand(A));
    blank(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2)));
    let drawn = city(&mut f, "C016", Zone::Deck(Deck::cities(2)));
    let mut game = Game::new(f.position(), with_figures()).unwrap();
    meld_action(&mut game, alexander);
    let (_, events) = play(&mut game, |_| None);
    let steps = steps(&events);
    let draw = steps
        .iter()
        .position(|s| matches!(s, Step::Move(None, c, ..) if *c == drawn))
        .unwrap();
    let scored = steps
        .iter()
        .position(|s| matches!(s, Step::Move(Some(Verb::Score), c, ..) if *c == tigernmas))
        .unwrap();
    assert!(draw < scored, "{steps:?}");
}

#[test]
fn montreal_junks_an_achievement_of_its_value_then_draws() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let montreal = city(&mut f, "C067", Zone::Hand(A));
    let six = blank(&mut f, Color::Green, 6, Zone::AvailableAchievements);
    let seven = blank(&mut f, Color::Green, 7, Zone::AvailableAchievements);
    let eight = blank(&mut f, Color::Green, 8, Zone::Deck(Deck::base(8)));
    let mut game = Game::new(f.position(), cities()).unwrap();
    meld_action(&mut game, montreal);
    play(&mut game, |_| None);
    assert_eq!(game.position().cards(Zone::Junk), [seven]);
    assert_eq!(game.position().cards(Zone::AvailableAchievements), [six]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [eight]);
}

/// The Junk icon chooses among available achievements by back: one option per set of
/// the City's value, and the chosen set's card is junked.
#[test]
fn a_junk_icon_offers_one_achievement_per_back_and_junks_the_chosen_set() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let sparta = city(&mut f, "C016", Zone::Hand(A));
    let base = f.add(base_card("B016").unwrap(), Zone::AvailableAchievements);
    let echoes = f.add(echoes_card("E016").unwrap(), Zone::AvailableAchievements);
    let cities_two = city(&mut f, "C017", Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), cities().union(Expansions::ECHOES)).unwrap();
    meld_action(&mut game, sparta);
    let mut offered = Vec::new();
    play(&mut game, |decision| {
        offered = decision
            .options
            .iter()
            .filter_map(|option| match option {
                Alternative::BlindMove {
                    verb: Verb::Junk,
                    back,
                    ..
                } => Some(back.set),
                _ => None,
            })
            .collect();
        offered.iter().position(|&set| set == CardSet::Cities)
    });
    offered.sort_by_key(|set| set.index());
    assert_eq!(offered, [CardSet::Base, CardSet::Echoes, CardSet::Cities]);
    assert_eq!(game.position().cards(Zone::Junk), [cities_two]);
    let mut left = game.position().cards(Zone::AvailableAchievements).to_vec();
    left.sort();
    assert_eq!(left, [base, echoes]);
}

#[test]
fn an_exhausting_icon_draw_ends_the_game_before_later_steps() {
    // Singapore (red 10) draws an 11 with its Plus; no 11 remains. It starts
    // a new color, but the game ends before the City draw.
    let mut f = Fixture::default();
    let singapore = city(&mut f, "C097", Zone::Hand(A));
    city(&mut f, "C096", Zone::Deck(Deck::cities(10)));
    blank(&mut f, Color::Green, 10, Zone::Deck(Deck::base(10)));
    blank(&mut f, Color::Green, 3, Zone::Score(B));
    let mut game = Game::new(f.position(), cities()).unwrap();
    meld_action(&mut game, singapore);
    let (status, events) = play(&mut game, |_| None);
    // A's 10 bonus outscores B's 3.
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.victory().unwrap().reason, WinReason::Exhaustion);
    assert_eq!(steps(&events).len(), 1);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}
