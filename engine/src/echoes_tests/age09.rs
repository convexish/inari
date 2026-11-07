//! Age-nine clauses, successful original references, and shortcut oracles.
use super::*;
use shortcuts::common::*;

const E: Expansions = Expansions::ECHOES;
const EF: Expansions = E.union(Expansions::FIGURES);

fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, vec![])
}
fn draw_card(f: &mut Fixture, color: Color, age: u8) -> CardId {
    tech(f, color, age, Zone::Deck(Deck::base(age)))
}
fn source(f: &mut Fixture, code: &str) -> CardId {
    let card = echoes_card(code).unwrap();
    let color = card.color;
    let other = if color == Color::Purple {
        Color::Blue
    } else {
        Color::Purple
    };
    tech(f, other, 9, Zone::Board(A, other));
    f.add(card, Zone::Board(A, color))
}
fn start(f: Fixture, card: CardId, effect: u16, expansions: Expansions) -> Game {
    let mut game = Game::seeded(f.position(), expansions, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}
fn advance(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..10000 {
        let mut clone = game.clone();
        let mut actual = vec![];
        let mut expected = vec![];
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(clone.advance(1, &mut expected).unwrap(), status);
        assert_eq!(actual, expected);
        assert_eq!(*game, clone);
        events.extend(actual);
        if status != Status::WorkLimit {
            return status;
        }
    }
    panic!("age-nine program did not settle");
}
fn menu(game: &mut Game, events: &mut Vec<Event>) -> Decision {
    let status = advance(game, events);
    let Status::Decision(menu) = status else {
        panic!("expected decision, got {status:?}")
    };
    menu
}
fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut pick: impl FnMut(&Decision) -> usize,
) -> Status {
    loop {
        match advance(game, events) {
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            status => {
                assert!(game.position.check_conservation());
                for p in Player::ALL {
                    if let Some(tracker) = game.tracker(p) {
                        assert!(tracker.check_truth(&game.position));
                    }
                }
                return status;
            }
        }
    }
}
fn offered(menu: &Decision) -> Vec<CardId> {
    menu.options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            }
            | Alternative::Select {
                target: TargetDescriptor::Known { card, .. },
            } => Some(*card),
            _ => None,
        })
        .collect()
}
fn pick_card(game: &mut Game, menu: &Decision, card: CardId) {
    let index = menu
        .options
        .iter()
        .position(|option| {
            matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
        | Alternative::Select { target: TargetDescriptor::Known { card: c, .. } } if *c == card)
        })
        .unwrap_or_else(|| panic!("card {card:?} absent from {menu:?}"));
    game.choose(index).unwrap();
}
fn decline(game: &mut Game, menu: &Decision) {
    game.choose(
        menu.options
            .iter()
            .position(|a| *a == Alternative::Decline)
            .unwrap(),
    )
    .unwrap();
}
fn reveals(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

fn figure(f: &mut Fixture, which: Figure, player: Player) -> CardId {
    let printed = figure_card(which.code()).unwrap();
    let color = printed.color;
    f.add(printed, Zone::Board(player, color))
}

#[derive(Debug, PartialEq)]
struct Outcome {
    position: Position,
    rng: Rng,
    status: Status,
    events: Vec<String>,
    views: [crate::observation::Observation; 2],
    beliefs: [Vec<Vec<crate::tracking::BeliefSlot>>; 3],
}
struct Path {
    outcome: Outcome,
    chronological: Vec<Event>,
}
fn paths(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<Path> {
    assert!(depth < 24, "bounded age-nine oracle");
    match advance(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut child = game.clone();
                child.choose(choice).unwrap();
                paths(child, events.clone(), depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            for p in Player::ALL {
                assert!(game.tracker(p).unwrap().check_truth(&game.position));
            }
            let views = Player::ALL.map(|player| game.observe(player).unwrap());
            let beliefs = [
                game.tracker(A).unwrap(),
                game.tracker(B).unwrap(),
                game.public_tracker().unwrap(),
            ]
            .map(|tracker| {
                crate::tracking::zones()
                    .into_iter()
                    .map(|zone| tracker.belief(zone))
                    .collect()
            });
            game.position.sort_unordered_piles();
            let mut bag: Vec<_> = events.iter().map(|e| format!("{e:?}")).collect();
            bag.sort();
            vec![Path {
                outcome: Outcome {
                    position: game.position,
                    rng: game.rng,
                    status,
                    events: bag,
                    views,
                    beliefs,
                },
                chronological: events,
            }]
        }
    }
}
fn oracle(game: Game, reduces: bool) -> Vec<Path> {
    compare_paths(game, Some(reduces))
}
fn compare_paths(game: Game, reduces: Option<bool>) -> Vec<Path> {
    let mut full = game.clone();
    full.shortcuts_disabled = true;
    let reduced = paths(game, vec![], 0);
    let full = paths(full, vec![], 0);
    if reduces == Some(true) {
        assert!(
            reduced.len() < full.len(),
            "expected reduction: {} versus {}",
            reduced.len(),
            full.len()
        );
    } else if reduces == Some(false) {
        assert_eq!(
            reduced.len(),
            full.len(),
            "reaction must preserve the order menu"
        );
    }
    for path in &full {
        assert!(
            reduced.iter().any(|r| r.outcome == path.outcome),
            "lost outcome {:#?}",
            path.outcome
        );
    }
    for path in &reduced {
        assert!(
            full.iter()
                .any(|f| f.outcome == path.outcome && f.chronological == path.chronological),
            "canonical path must occur unchanged in unrestricted play"
        );
    }
    reduced
}

fn promote(f: Fixture, promotion: CardId, expansions: Expansions) -> Game {
    let mut game = Game::seeded(f.position(), expansions, 53)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
        .unwrap();
    game
}
fn drawn(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb.is_none() => Some(m.card),
            _ => None,
        })
        .collect()
}

fn value(game: &mut Game, events: &mut Vec<Event>, wanted: u8) {
    let m = menu(game, events);
    game.choose(
        m.options
            .iter()
            .position(|a| *a == Alternative::Value(wanted))
            .unwrap(),
    )
    .unwrap();
}

#[test]
fn age_nine_catalog_has_every_printed_clause_and_stays_within_layout_budgets() {
    for (code, echo, clauses, demand) in [
        (86, false, 1, false),
        (87, true, 2, true),
        (88, true, 3, true),
        (89, true, 2, false),
        (90, true, 2, false),
        (91, true, 2, false),
        (92, false, 2, false),
        (93, false, 2, false),
        (94, true, 2, false),
        (95, false, 2, true),
    ] {
        let c = echoes_card(&format!("E{code:03}")).unwrap();
        assert_eq!(
            (c.echo.is_some(), c.effects.len(), c.effects[0].demand()),
            (echo, clauses, demand)
        );
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn shares_icon_reads_both_fronts_and_uses_the_common_icon_type_rules() {
    use crate::program::{
        Filter::Has,
        build::{HAND, filtered},
    };
    assert!(Test::SharesIcon(Reg::Last).reads_front());
    assert!(Cond::Card(Reg::Bound, Has(Test::SharesIcon(Reg::Last))).reads_card_front());
    assert!(Cond::Card(Reg::Last, Has(Test::SharesIcon(Reg::Bound))).reads_card_front());
    assert!(!Cond::Card(Reg::Last, Has(Test::SharesIcon(Reg::Chosen))).reads_card_front());
    const MATCHING: Selector = filtered(&HAND, &[Has(Test::SharesIcon(Reg::Bound))]);
    const PRIVATE: Selector = filtered(
        &Selector::Hand(Who::Opponent),
        &[Has(Test::SharesIcon(Reg::Bound))],
    );
    for (left, right, expected) in [
        (Symbol::Icon(Icon::Health), Symbol::Icon(Icon::Health), true),
        (
            Symbol::Icon(Icon::Health),
            Symbol::Icon(Icon::Industry),
            false,
        ),
        (Symbol::Bonus(1), Symbol::Bonus(12), true),
        (Symbol::Bonus(6), Symbol::Icon(Icon::Concept), false),
        (
            Symbol::City(CityIcon::Uplift),
            Symbol::City(CityIcon::Uplift),
            true,
        ),
        (
            Symbol::City(CityIcon::Uplift),
            Symbol::City(CityIcon::Plus),
            false,
        ),
        (
            Symbol::Search(Icon::Industry),
            Symbol::Icon(Icon::Industry),
            true,
        ),
        (
            Symbol::Search(Icon::Health),
            Symbol::Search(Icon::Industry),
            false,
        ),
        (Symbol::Image, Symbol::Image, false),
        (Symbol::Echo, Symbol::Echo, false),
    ] {
        let mut f = Fixture::default();
        let anchor = tech(&mut f, Color::Red, 5, Zone::Board(B, Color::Red));
        let candidate = tech(&mut f, Color::Blue, 4, Zone::Hand(A));
        let secret = tech(&mut f, Color::Green, 4, Zone::Hand(B));
        f.cards[anchor.0 as usize].symbols[0] = left;
        f.cards[candidate.0 as usize].symbols[0] = right;
        f.cards[secret.0 as usize].symbols[0] = right;
        let mut game = Game::seeded(f.position(), E.union(Expansions::CITIES), 11)
            .unwrap()
            .with_tracking()
            .unwrap();
        let regs = Regs {
            bound: Some(game.capture(anchor)),
            ..Regs::EMPTY
        };
        let selected = game
            .select(MATCHING, Context::new(A), regs, Excluded::default())
            .unwrap();
        assert_eq!(
            selected.iter().any(|target| target.card == candidate),
            expected,
            "{left:?} vs {right:?}"
        );
        assert!(
            matches!(
                game.select(PRIVATE, Context::new(A), regs, Excluded::default()),
                Err(Error::Unsupported(_))
            ),
            "front filter may not reveal a private opponent card"
        );
    }
}

#[test]
fn helicopter_returns_only_matching_hand_cards_reveals_before_return_and_repeats() {
    for owner in Player::ALL {
        let mut f = Fixture::default();
        let card = source(&mut f, "E086");
        tech(&mut f, Color::Red, 8, Zone::Board(B, Color::Red));
        let moved = tech(&mut f, Color::Green, 5, Zone::Board(owner, Color::Green));
        f.cards[moved.0 as usize].symbols[0] = Symbol::Bonus(5);
        let matching = tech(&mut f, Color::Blue, 3, Zone::Hand(A));
        f.cards[matching.0 as usize].symbols[0] = Symbol::Bonus(9);
        let wrong = tech(&mut f, Color::Yellow, 3, Zone::Hand(A));
        f.cards[wrong.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        let targets = menu(&mut game, &mut events);
        assert!(offered(&targets).contains(&moved));
        assert!(!offered(&targets).contains(&card));
        pick_card(&mut game, &targets, moved);
        let returns = menu(&mut game, &mut events);
        assert_eq!(offered(&returns), [matching]);
        assert_eq!(reveals(&events), []);
        assert_eq!(game.position.locate(moved), Some(Zone::Score(owner)));
        pick_card(&mut game, &returns, matching);
        let repeated = menu(&mut game, &mut events);
        assert!(offered(&repeated).iter().all(|c| {
            game.position
                .locate(*c)
                .is_some_and(|z| matches!(z, Zone::Board(..)))
        }));
        assert_eq!(reveals(&events), [matching]);
        assert_eq!(
            game.position.locate(matching),
            Some(Zone::Deck(Deck::base(3)))
        );
        assert_eq!(game.position.locate(wrong), Some(Zone::Hand(A)));
        let reveal = events
            .iter()
            .position(|e| matches!(e,Event::Revealed{card,..} if *card==matching))
            .unwrap();
        let returned = events
            .iter()
            .position(
                |e| matches!(e,Event::Moved(m) if m.card==matching && m.verb==Some(Verb::Return)),
            )
            .unwrap();
        assert!(reveal < returned);
    }
}

#[test]
fn helicopter_optional_decline_and_absent_other_tops_do_no_extra_work() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E086");
    let moved = tech(&mut f, Color::Blue, 3, Zone::Board(B, Color::Blue));
    f.cards[moved.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let matching = tech(&mut f, Color::Green, 2, Zone::Hand(A));
    f.cards[matching.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, moved);
    let m = menu(&mut game, &mut events);
    decline(&mut game, &m);
    finish(&mut game, &mut events, |_| {
        panic!("decline does not repeat")
    });
    assert_eq!(game.position.locate(matching), Some(Zone::Hand(A)));
    assert!(reveals(&events).is_empty());
    let mut f = Fixture::default();
    let card = f.add(echoes_card("E086").unwrap(), Zone::Board(A, Color::Red));
    let hand = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| panic!("no other top"));
    assert_eq!(game.position.locate(hand), Some(Zone::Hand(A)));
    assert!(events.is_empty());
}

#[test]
fn helicopter_replaced_transfer_has_no_anchor_and_replaced_return_does_not_repeat() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E086");
    figure(&mut f, Figure::YiSunSin, A);
    let moved = tech(&mut f, Color::Blue, 3, Zone::Board(B, Color::Blue));
    f.cards[moved.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let hand = tech(&mut f, Color::Green, 2, Zone::Hand(A));
    f.cards[hand.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    draw_card(&mut f, Color::Yellow, 4);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, moved);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(hand), Some(Zone::Hand(A)));
    assert!(reveals(&events).is_empty());
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==moved && m.inside_karma))
    );
    let mut f = Fixture::default();
    let card = source(&mut f, "E086");
    let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
    let moved = tech(&mut f, Color::Blue, 4, Zone::Board(B, Color::Blue));
    f.cards[moved.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let hand = tech(&mut f, Color::Red, 2, Zone::Hand(A));
    f.cards[hand.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, moved);
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, hand);
    finish(&mut game, &mut events, |_| {
        panic!("replaced return does not repeat")
    });
    assert_eq!(game.position.locate(napoleon), Some(Zone::Score(A)));
    assert_eq!(events.iter().filter(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Transfer) && !m.inside_karma)).count(), 1, "the replaced return must not start a second transfer");
    assert_eq!(reveals(&events), [hand]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==hand && m.verb==Some(Verb::Return)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==hand && m.inside_karma))
    );
}

#[test]
fn atm_echo_uses_any_plain_value_and_demand_filters_before_retaining_highest_ties() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E088");
    figure(&mut f, Figure::Archimedes, A);
    let drawn = draw_card(&mut f, Color::Green, 3);
    let wrong = draw_card(&mut f, Color::Blue, 4);
    let mut game = start(f, card, Card::ECHO, EF);
    let mut events = vec![];
    value(&mut game, &mut events, 3);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(drawn), Some(Zone::Score(A)));
    assert_eq!(game.position.locate(wrong), Some(Zone::Deck(Deck::base(4))));
    let mut f = Fixture::default();
    let card = source(&mut f, "E088");
    let excluded = tech(&mut f, Color::Purple, 10, Zone::Board(A, Color::Purple));
    f.cards[excluded.0 as usize].symbols[0] = Symbol::Icon(Icon::Prosperity);
    let left = tech(&mut f, Color::Blue, 4, Zone::Board(A, Color::Blue));
    let right = tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
    let low = tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    let mut offered = offered(&m);
    offered.sort();
    let mut expected = vec![left, right];
    expected.sort();
    assert_eq!(offered, expected);
    pick_card(&mut game, &m, right);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.locate(right),
        Some(Zone::Board(B, Color::Green))
    );
    for c in [excluded, left, low, card] {
        assert!(matches!(game.position.locate(c), Some(Zone::Board(A, _))));
    }
}

#[test]
fn atm_optional_purple_splay_keeps_decline_and_its_deck_sweep_uses_the_boxed_age() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E088");
        for _ in 0..2 {
            tech(&mut f, Color::Purple, 2, Zone::Board(A, Color::Purple));
        }
        let mut game = start(f, card, 1, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        if accept {
            game.choose(0).unwrap();
        } else {
            decline(&mut game, &m);
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.splay(A, Color::Purple),
            if accept { Splay::Up } else { Splay::None }
        );
    }
    for archimedes in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E088");
        if archimedes {
            figure(&mut f, Figure::Archimedes, A);
        }
        let age = if archimedes { 11 } else { 10 };
        let swept = draw_card(&mut f, Color::Red, age);
        let ignored = draw_card(&mut f, Color::Green, if archimedes { 10 } else { 11 });
        let echo = tech(
            &mut f,
            Color::Blue,
            age,
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age,
            }),
        );
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        let mut game = start(f, card, 2, if archimedes { EF } else { E });
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.locate(swept), Some(Zone::Junk));
        for c in [ignored, echo] {
            assert!(matches!(game.position.locate(c), Some(Zone::Deck(_))));
        }
        assert!(reveals(&events).is_empty());
    }
}

#[test]
fn atm_deck_top_oracle_matches_every_order_with_archimedes_actually_shifting_ten_to_eleven() {
    for archimedes in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E088");
        if archimedes {
            figure(&mut f, Figure::Archimedes, A);
        }
        let age = if archimedes { 11 } else { 10 };
        for color in [Color::Red, Color::Green] {
            draw_card(&mut f, color, age);
        }
        let leaves = oracle(start(f, card, 2, if archimedes { EF } else { E }), true);
        assert!(leaves.iter().all(|p| {
            p.chronological
                .iter()
                .filter(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Junk)))
                .count()
                == 2
        }));
    }
}

#[test]
fn email_echo_and_normal_dogma_draw_even_when_forecast_is_full() {
    for effect in [Card::ECHO, 0] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let card = source(&mut f, "E091");
            if full {
                for _ in 0..5 {
                    tech(&mut f, Color::Yellow, 11, Zone::Forecast(A));
                }
            }
            let age = if effect == Card::ECHO { 10 } else { 9 };
            let drawn = draw_card(&mut f, Color::Blue, age);
            let unused = draw_card(&mut f, Color::Red, 11);
            let mut game = start(f, card, effect, E);
            let mut events = vec![];
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(
                game.position.locate(drawn),
                Some(if full {
                    Zone::Hand(A)
                } else {
                    Zone::Forecast(A)
                })
            );
            assert_eq!(
                game.position.locate(unused),
                Some(Zone::Deck(Deck::base(11)))
            );
            assert!(reveals(&events).is_empty());
        }
    }
}

#[test]
fn email_foreseen_adds_eleven_after_nine_and_does_not_mark_its_self_execution_foreseen() {
    use crate::program::build;
    for full in [false, true] {
        let mut f = Fixture::default();
        f.add(echoes_card("E091").unwrap(), Zone::Forecast(A));
        if full {
            for _ in 0..4 {
                tech(&mut f, Color::Purple, 11, Zone::Forecast(A));
            }
        }
        let promotion = tech(&mut f, Color::Green, 10, Zone::Hand(A));
        f.tech(
            Color::Blue,
            2,
            Zone::Board(A, Color::Blue),
            vec![build::if_(&Cond::Foreseen, 1), build::score(build::HAND)],
        );
        let echo = draw_card(&mut f, Color::Red, 10);
        let normal = draw_card(&mut f, Color::Yellow, 9);
        let extra = draw_card(&mut f, Color::Purple, 11);
        let mut game = promote(f, promotion, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(drawn(&events), [echo, normal, extra]);
        assert_eq!(game.position.locate(echo), Some(Zone::Forecast(A)));
        for c in [normal, extra] {
            assert_eq!(
                game.position.locate(c),
                Some(if full {
                    Zone::Hand(A)
                } else {
                    Zone::Forecast(A)
                })
            );
        }
        assert!(
            game.position.cards(Zone::Score(A)).is_empty(),
            "the other card was self-executed, not foreseen"
        );
    }
}

#[test]
fn email_self_execution_keeps_lowest_ties_after_excluding_green() {
    use crate::program::build;
    for choose_blue in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E091");
        let green = f.tech(
            Color::Green,
            1,
            Zone::Board(A, Color::Green),
            vec![build::draw(11)],
        );
        let blue = f.tech(
            Color::Blue,
            2,
            Zone::Board(A, Color::Blue),
            vec![build::score(build::HAND)],
        );
        let red = f.tech(
            Color::Red,
            2,
            Zone::Board(A, Color::Red),
            vec![build::tuck(build::HAND)],
        );
        let hand = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
        let mut game = start(f, card, 1, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        let mut offered = offered(&m);
        offered.sort();
        let mut expected = vec![blue, red];
        expected.sort();
        assert_eq!(offered, expected);
        assert!(!offered.contains(&green));
        pick_card(&mut game, &m, if choose_blue { blue } else { red });
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(hand),
            Some(if choose_blue {
                Zone::Score(A)
            } else {
                Zone::Board(A, Color::Yellow)
            })
        );
        assert!(drawn(&events).is_empty());
    }
}

#[test]
fn laser_returns_up_to_four_score_cards_before_every_physical_standard_achievement() {
    for score_count in [0_usize, 2, 4, 5] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E093");
        let scores: Vec<_> = (0..score_count)
            .map(|_| tech(&mut f, Color::Red, 8, Zone::Score(A)))
            .collect();
        let base = tech(&mut f, Color::Blue, 2, Zone::AvailableAchievements);
        let echo = tech(&mut f, Color::Green, 3, Zone::AvailableAchievements);
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            scores
                .iter()
                .filter(|&&c| game.position.locate(c) == Some(Zone::Score(A)))
                .count(),
            score_count.saturating_sub(4)
        );
        assert_eq!(game.position.locate(base), Some(Zone::Deck(Deck::base(2))));
        assert_eq!(
            game.position.locate(echo),
            Some(Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 3
            }))
        );
        let returns: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) if m.verb == Some(Verb::Return) => Some(m),
                _ => None,
            })
            .collect();
        assert!(
            returns[..score_count.min(4)]
                .iter()
                .all(|m| m.source == Zone::Score(A))
        );
        assert!(
            returns[score_count.min(4)..]
                .iter()
                .all(|m| m.source == Zone::AvailableAchievements)
        );
        assert!(reveals(&events).is_empty());
    }
}

#[test]
fn laser_standard_supply_does_not_return_figure_granted_board_aliases() {
    for which in [
        Figure::CaiLun,
        Figure::JohnLoudonMcAdam,
        Figure::AminaSukhera,
    ] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E093");
        figure(&mut f, which, A);
        let alias = tech(&mut f, Color::Purple, 1, Zone::Board(A, Color::Purple));
        f.cards[alias.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
        f.cards[alias.0 as usize].symbols[1] = Symbol::Icon(Icon::Authority);
        let standard = tech(&mut f, Color::Green, 2, Zone::AvailableAchievements);
        let mut game = start(f, card, 0, EF);
        assert!(
            game.position
                .targets(LogicalZone::AvailableAchievements(A))
                .any(|target| target.card == alias)
        );
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(alias),
            Some(Zone::Board(A, Color::Purple))
        );
        assert_eq!(
            game.position.locate(standard),
            Some(Zone::Deck(Deck::base(2)))
        );
    }
}

#[test]
fn laser_score_and_supply_oracle_preserves_deck_order_and_maxwells_actual_draws() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E093");
        tech(&mut f, Color::Blue, 2, Zone::Score(A));
        tech(&mut f, Color::Green, 2, Zone::Score(A));
        let echo = tech(&mut f, Color::Yellow, 2, Zone::Score(A));
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        tech(&mut f, Color::Blue, 3, Zone::AvailableAchievements);
        let echo = tech(&mut f, Color::Green, 4, Zone::AvailableAchievements);
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        if reacting {
            figure(&mut f, Figure::JamesClerkMaxwell, B);
            for _ in 0..5 {
                draw_card(&mut f, Color::Red, 8);
            }
        }
        let leaves = oracle(start(f, card, 0, if reacting { EF } else { E }), !reacting);
        assert_eq!(leaves.len(), if reacting { 12 } else { 2 });
        if reacting {
            assert!(leaves.iter().all(|p| {
                p.chronological
                    .iter()
                    .filter(|e| matches!(e,Event::Moved(m) if m.verb.is_none() && m.inside_karma))
                    .count()
                    == 5
            }));
        }
    }
    let mut f = Fixture::default();
    let card = source(&mut f, "E093");
    for _ in 0..5 {
        tech(&mut f, Color::Red, 8, Zone::Score(A));
    }
    assert_eq!(
        oracle(start(f, card, 0, E), false).len(),
        120,
        "all five score subsets and their same-deck orders remain"
    );
}

#[test]
fn laser_blind_supply_duplicates_keep_native_rng_and_private_sampled_continuations() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E093");
        let candidates: Vec<_> = [Color::Red, Color::Blue]
            .map(|c| tech(&mut f, c, 3, Zone::AvailableAchievements))
            .into();
        let others: Vec<_> = [Color::Green, Color::Yellow]
            .map(|c| tech(&mut f, c, 4, Zone::AvailableAchievements))
            .into();
        if reacting {
            figure(&mut f, Figure::JamesClerkMaxwell, B);
            for _ in 0..4 {
                draw_card(&mut f, Color::Purple, 8);
            }
        }
        let mut game = start(f, card, 0, if reacting { EF } else { E });
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        assert_eq!(m.options.len(), 2);
        assert!(m.options.iter().all(|a| matches!(
            a,
            Alternative::BlindMove {
                verb: Verb::Return,
                source: Zone::AvailableAchievements,
                ..
            }
        )));
        for seed in 0..12 {
            for mut world in [
                game.determinize(A, seed).unwrap(),
                game.sample_world(A, seed).unwrap().world,
            ] {
                let m = menu(&mut world, &mut vec![]);
                let index = m.options.iter().position(|a| matches!(a, Alternative::BlindMove { back, .. } if *back == Deck::base(3))).unwrap();
                let mut expected = world.rng.clone();
                expected.next_u64();
                expected.next_u64();
                world.choose(index).unwrap();
                let mut events = vec![];
                finish(&mut world, &mut events, |_| 0);
                assert_eq!(world.rng, expected);
                for c in &candidates {
                    assert_eq!(world.position.locate(*c), Some(Zone::Deck(Deck::base(3))));
                }
                for c in &others {
                    assert_eq!(world.position.locate(*c), Some(Zone::Deck(Deck::base(4))));
                }
                assert!(reveals(&events).is_empty());
                assert_eq!(drawn(&events).len(), if reacting { 4 } else { 0 });
            }
        }
        // Once each remaining back is a singleton, cross-deck commutation can
        // resume. Both native random words must first stay with their cohort.
        compare_paths(game, reacting.then_some(false));
    }
}

#[test]
fn laser_draws_foreshadow_then_meld_in_printed_order_and_adds_only_foreseen_eleven() {
    for (foreseen, full) in [(false, false), (false, true), (true, false)] {
        let mut f = Fixture::default();
        let card = f.add(
            echoes_card("E093").unwrap(),
            if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Board(A, Color::Blue)
            },
        );
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Yellow, 9, Zone::Forecast(A));
            }
        }
        let extra = draw_card(&mut f, Color::Green, 11);
        let forecast = draw_card(&mut f, Color::Purple, 11);
        let melded = draw_card(&mut f, Color::Red, 10);
        let promotion = tech(&mut f, Color::Blue, 10, Zone::Hand(A));
        let mut game = if foreseen {
            promote(f, promotion, E)
        } else {
            start(f, card, 1, E)
        };
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            drawn(&events),
            if foreseen {
                vec![forecast, melded, extra]
            } else {
                vec![forecast, melded]
            }
        );
        assert_eq!(
            game.position.locate(forecast),
            Some(if full {
                Zone::Hand(A)
            } else {
                Zone::Forecast(A)
            })
        );
        assert_eq!(
            game.position.locate(melded),
            Some(Zone::Board(A, Color::Red))
        );
        assert_eq!(
            game.position.locate(extra),
            Some(if foreseen {
                Zone::Board(A, Color::Green)
            } else {
                Zone::Deck(Deck::base(11))
            })
        );
    }
}

#[test]
fn rock_demands_a_transfer_before_proposing_the_demanders_win() {
    for scissors in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E095");
        let bottom = if scissors {
            f.add(echoes_card("E021").unwrap(), Zone::Board(A, Color::Green))
        } else {
            tech(&mut f, Color::Green, 2, Zone::Board(A, Color::Green))
        };
        let top = tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        let status = finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.locate(top), Some(Zone::Hand(B)));
        assert_eq!(
            game.position.locate(bottom),
            Some(Zone::Board(A, Color::Green))
        );
        assert_eq!(status == Status::Winner(B), scissors);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Transfer)))
                .count(),
            1
        );
    }
    let mut f = Fixture::default();
    let card = source(&mut f, "E095");
    let mut game = start(f, card, 0, E);
    assert!(matches!(
        finish(&mut game, &mut vec![], |_| 0),
        Status::Complete(_)
    ));
}

#[test]
fn rocks_named_demander_win_runs_jackies_real_first_instead_of_eliminating_the_victim() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E095");
    figure(&mut f, Figure::JackieChan, A);
    let scored = figure(&mut f, Figure::NapoleonBonaparte, B);
    f.add(echoes_card("E021").unwrap(), Zone::Board(A, Color::Green));
    let transferred = tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    assert_eq!(finish(&mut game, &mut events, |_| 0), Status::Winner(A));
    assert_eq!(game.position.locate(transferred), Some(Zone::Hand(B)));
    assert_eq!(game.position.locate(scored), Some(Zone::Score(A)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==scored && m.inside_karma))
    );
}

#[test]
fn rock_checks_the_live_green_paper_even_when_the_optional_score_is_declined() {
    for (paper, cover, accept) in [
        (true, false, false),
        (true, true, true),
        (true, true, false),
        (false, false, false),
    ] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E095");
        let bottom = if paper {
            f.add(base_card("B031").unwrap(), Zone::Board(A, Color::Green))
        } else {
            tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green))
        };
        let top = cover.then(|| tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green)));
        let mut game = start(f, card, 1, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        if accept {
            pick_card(&mut game, &m, top.unwrap());
        } else {
            decline(&mut game, &m);
        }
        let status = finish(&mut game, &mut events, |_| 0);
        assert_eq!(status == Status::Winner(A), paper && (!cover || accept));
        assert_eq!(
            game.position.locate(bottom),
            Some(Zone::Board(A, Color::Green))
        );
        if accept {
            assert_eq!(game.position.locate(top.unwrap()), Some(Zone::Score(A)));
        }
    }
}

#[test]
fn rock_live_win_checks_survive_replacements_of_the_preceding_transfer_or_score() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E095");
    figure(&mut f, Figure::YiSunSin, A);
    let scissors = f.add(echoes_card("E021").unwrap(), Zone::Board(A, Color::Green));
    let tucked = tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green));
    draw_card(&mut f, Color::Yellow, 4);
    let alternative = tech(&mut f, Color::Blue, 5, Zone::Board(B, Color::Blue));
    f.cards[alternative.0 as usize].symbols[0] = Symbol::Icon(Icon::Authority);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    assert_eq!(
        finish(&mut game, &mut events, |menu| {
            menu.options.iter().position(|option| matches!(option, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == alternative)).unwrap_or(0)
        }),
        Status::Winner(B)
    );
    assert_eq!(game.position.locate(alternative), Some(Zone::Score(A)));
    assert_eq!(
        game.position.cards(Zone::Board(A, Color::Green)),
        [tucked, scissors]
    );
    assert!(game.position.cards(Zone::Hand(B)).is_empty());
    assert!(events.iter().any(|e|matches!(e,Event::Moved(m) if m.card==tucked && m.verb==Some(Verb::Tuck) && m.inside_karma)));
    let mut f = Fixture::default();
    let card = source(&mut f, "E095");
    let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
    let paper = f.add(base_card("B031").unwrap(), Zone::Board(A, Color::Green));
    let mut game = start(f, card, 1, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, paper);
    assert_eq!(finish(&mut game, &mut events, |_| 0), Status::Winner(A));
    assert_eq!(
        game.position.locate(paper),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(game.position.locate(napoleon), Some(Zone::Score(A)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==napoleon && m.inside_karma))
    );
}

#[test]
fn helicopter_matching_menu_keeps_the_public_transferred_reference_after_sampling_private_score() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E086");
    tech(&mut f, Color::Red, 8, Zone::Board(B, Color::Red));
    let anchor = tech(&mut f, Color::Blue, 4, Zone::Board(B, Color::Blue));
    f.cards[anchor.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let decoy = tech(&mut f, Color::Green, 4, Zone::Score(B));
    f.cards[decoy.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
    let hidden = draw_card(&mut f, Color::Purple, 4);
    f.cards[hidden.0 as usize].symbols[0] = Symbol::Icon(Icon::Concept);
    let matching = tech(&mut f, Color::Yellow, 3, Zone::Hand(A));
    f.cards[matching.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let wrong = tech(&mut f, Color::Red, 3, Zone::Hand(A));
    f.cards[wrong.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, anchor);
    let returns = menu(&mut game, &mut events);
    assert_eq!(offered(&returns), [matching]);
    assert!(reveals(&events).is_empty());
    for seed in 0..16 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            if world.tracker(A).is_some() {
                assert_eq!(world.observe(A).unwrap(), game.observe(A).unwrap());
            }
            let m = menu(&mut world, &mut vec![]);
            assert_eq!(m.player, returns.player);
            assert_eq!(m.options, returns.options);
            assert_eq!(m.source_card(A), returns.source_card(A));
            for option in 0..m.options.len() {
                assert_eq!(m.option_label(option, A), returns.option_label(option, A));
            }
            pick_card(&mut world, &m, matching);
            let mut events = vec![];
            menu(&mut world, &mut events);
            assert_eq!(reveals(&events), [matching]);
            assert_eq!(
                world.position.locate(matching),
                Some(Zone::Deck(Deck::base(3)))
            );
            assert_eq!(world.position.locate(wrong), Some(Zone::Hand(A)));
            assert_eq!(world.position.locate(anchor), Some(Zone::Score(B)));
        }
    }
}
