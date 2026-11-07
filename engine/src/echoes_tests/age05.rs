//! Age-five clauses, successful original references, and shortcut oracles.
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
    tech(f, other, 5, Zone::Board(A, other));
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
    panic!("age-five program did not settle");
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
    assert!(depth < 24, "bounded age-five oracle");
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

#[test]
fn age_five_catalog_has_every_printed_clause() {
    for (code, echo, clauses, demand) in [
        (46, true, 2, false),
        (47, false, 2, true),
        (48, false, 2, false),
        (49, true, 2, false),
        (50, false, 3, false),
        (51, false, 2, true),
        (52, true, 3, true),
        (53, true, 1, false),
        (54, true, 1, false),
        (55, true, 1, false),
    ] {
        let c = echoes_card(&format!("E{code:03}")).unwrap();
        assert_eq!(
            (c.echo.is_some(), c.effects.len(), c.effects[0].demand()),
            (echo, clauses, demand)
        );
    }
}

#[test]
fn coke_echo_tucks_four_and_dogma_reveals_each_draw_before_branching() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E046");
    let echo = draw_card(&mut f, Color::Yellow, 4);
    let mut game = start(f, card, Card::ECHO, E);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.locate(echo),
        Some(Zone::Board(A, Color::Yellow))
    );
    for full in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E046");
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Green, 7, Zone::Forecast(A));
            }
        }
        let stop = draw_card(&mut f, Color::Yellow, 6);
        let repeat = draw_card(&mut f, Color::Green, 6);
        f.cards[repeat.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(reveals(&events), [repeat, stop]);
        assert_eq!(
            game.position.locate(repeat),
            Some(Zone::Board(A, Color::Green))
        );
        assert_eq!(
            game.position.locate(stop),
            Some(if full {
                Zone::Hand(A)
            } else {
                Zone::Forecast(A)
            })
        );
        for c in [repeat, stop] {
            let reveal = events
                .iter()
                .position(|e| matches!(e,Event::Revealed{card,..} if *card==c))
                .unwrap();
            if let Some(moved) = events
                .iter()
                .position(|e| matches!(e,Event::Moved(m) if m.card==c && m.verb.is_some()))
            {
                assert!(reveal < moved);
            }
        }
    }
}

#[test]
fn coke_repeats_even_when_napoleon_replaces_the_industry_meld() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E046");
    let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
    let stop = draw_card(&mut f, Color::Purple, 6);
    let original = draw_card(&mut f, Color::Green, 6);
    f.cards[original.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    finish(&mut game, &mut events, |m| {
        m.options.iter().position(|a|matches!(a,Alternative::Move{verb:Verb::Score,target:TargetDescriptor::Known{card,..},..} if *card==napoleon)).unwrap_or(0)
    });
    assert_eq!(drawn(&events), [original, stop]);
    assert_eq!(game.position.locate(stop), Some(Zone::Forecast(A)));
    assert!(events.iter().any(|e|matches!(e,Event::Moved(m) if m.card==original && m.verb==Some(Verb::Meld) && m.inside_karma)));
}

#[test]
fn stove_echo_keeps_the_identity_choice_and_excludes_industry() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E049");
    let excluded = tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    f.cards[excluded.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
    let chosen = tech(&mut f, Color::Green, 2, Zone::Board(A, Color::Green));
    let mut game = start(f, card, Card::ECHO, E);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    assert!(offered(&m).contains(&chosen));
    assert!(!offered(&m).contains(&excluded));
    assert!(!offered(&m).contains(&card));
    pick_card(&mut game, &m, chosen);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(chosen), Some(Zone::Score(A)));
    assert_eq!(
        game.position.locate(excluded),
        Some(Zone::Board(A, Color::Red))
    );
}

#[test]
fn stove_checks_the_successful_tucks_live_top_and_plain_four_threshold() {
    for (top, archimedes) in [(0, false), (3, false), (4, false), (3, true), (4, true)] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E049");
        if top > 0 {
            tech(&mut f, Color::Green, top, Zone::Board(A, Color::Green));
        }
        if archimedes {
            figure(&mut f, Figure::Archimedes, A);
        }
        let age = if archimedes { 5 } else { 4 };
        let scored = draw_card(&mut f, Color::Purple, age);
        let tucked = draw_card(&mut f, Color::Green, age);
        let mut game = start(f, card, 0, if archimedes { EF } else { E });
        finish(&mut game, &mut vec![], |_| 0);
        assert_eq!(
            game.position.locate(tucked),
            Some(Zone::Board(A, Color::Green))
        );
        assert_eq!(
            game.position.locate(scored) == Some(Zone::Score(A)),
            top > 0 && top < 4
        );
    }
}

#[test]
fn stove_and_lightning_rod_do_not_use_a_replacement_tucks_color() {
    for code in ["E049", "E052"] {
        let mut f = Fixture::default();
        let card = source(&mut f, code);
        let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
        let top = tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green));
        let age = if code == "E049" { 4 } else { 5 };
        let spare = draw_card(&mut f, Color::Purple, age);
        let tucked = draw_card(&mut f, Color::Green, age);
        let mut game = start(f, card, 0, EF);
        let mut events = vec![];
        finish(&mut game, &mut events, |m| {
            m.options.iter().position(|a|matches!(a,Alternative::Move{verb:Verb::Score,target:TargetDescriptor::Known{card,..},..} if *card==napoleon)).unwrap_or(0)
        });
        assert_eq!(
            game.position.locate(spare),
            Some(Zone::Deck(Deck::base(age)))
        );
        assert_eq!(
            game.position.locate(top),
            Some(Zone::Board(A, Color::Green))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Return)))
        );
        assert!(events.iter().any(|e|matches!(e,Event::Moved(m) if m.card==tucked && m.verb==Some(Verb::Meld) && m.inside_karma)));
    }
}

#[test]
fn stove_green_splay_is_optional_and_keeps_other_colors() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E049");
        for _ in 0..2 {
            tech(&mut f, Color::Green, 2, Zone::Board(A, Color::Green));
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
            game.position.splay(A, Color::Green),
            if accept { Splay::Right } else { Splay::None }
        );
        assert_eq!(game.position.splay(A, Color::Yellow), Splay::None);
    }
}

#[test]
fn lightning_rod_echo_and_demand_use_the_successful_tuck_and_return_the_live_top() {
    for (effect, old) in [(Card::ECHO, 0), (0, 0), (0, 3)] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E052");
        let top = (old > 0).then(|| tech(&mut f, Color::Green, old, Zone::Board(A, Color::Green)));
        let tucked = draw_card(&mut f, Color::Green, 5);
        let mut game = start(f, card, effect, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        let returned = if effect == 0 {
            top.or(Some(tucked))
        } else {
            None
        };
        if let Some(c) = returned {
            assert_eq!(
                game.position.locate(c),
                Some(Zone::Deck(Deck::base(game.position.card(c).value)))
            );
        }
        assert_eq!(
            game.position.locate(tucked) == Some(Zone::Board(A, Color::Green)),
            effect == Card::ECHO || old > 0
        );
    }
}

#[test]
fn lightning_rods_independent_return_offers_tops_and_runs_peter_first() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E052");
    figure(&mut f, Figure::PeterTheGreat, A);
    let bottom = tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
    let top = tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green));
    let mut game = start(f, card, 1, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    assert!(offered(&m).contains(&top));
    assert!(!offered(&m).contains(&bottom));
    pick_card(&mut game, &m, top);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(bottom), Some(Zone::Score(A)));
    assert_eq!(game.position.locate(top), Some(Zone::Deck(Deck::base(3))));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==bottom && m.inside_karma))
    );
}

#[test]
fn coke_foreseen_scores_one_non_red_top_per_color_and_keeps_exposed_cards() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let card = f.add(
            echoes_card("E046").unwrap(),
            if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Board(A, Color::Red)
            },
        );
        let low = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        let top = tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
        let green = tech(&mut f, Color::Green, 2, Zone::Board(A, Color::Green));
        let promotion = tech(&mut f, Color::Red, 6, Zone::Hand(A));
        draw_card(&mut f, Color::Red, 6);
        draw_card(&mut f, Color::Red, 4);
        let mut game = if foreseen {
            promote(f, promotion, E)
        } else {
            start(f, card, 1, E)
        };
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        for c in [top, green] {
            assert_eq!(
                game.position.locate(c) == Some(Zone::Score(A)),
                foreseen,
                "{events:#?}"
            );
        }
        assert_eq!(game.position.locate(low), Some(Zone::Board(A, Color::Blue)));
        assert_eq!(game.position.locate(card), Some(Zone::Board(A, Color::Red)));
    }
}

#[test]
fn coke_foreseen_score_oracle_preserves_every_path_and_exposed_shen_kuo() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        f.add(echoes_card("E046").unwrap(), Zone::Forecast(A));
        let promotion = tech(&mut f, Color::Red, 6, Zone::Hand(A));
        draw_card(&mut f, Color::Red, 6);
        draw_card(&mut f, Color::Red, 4);
        for age in [1, 2] {
            tech(&mut f, Color::Blue, age, Zone::Board(A, Color::Blue));
        }
        if reacting {
            figure(&mut f, Figure::ShenKuo, A);
        } else {
            tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green));
        }
        tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
        let leaves = oracle(
            promote(f, promotion, if reacting { EF } else { E }),
            !reacting,
        );
        if reacting {
            assert!(
                leaves
                    .iter()
                    .any(|leaf| leaf.chronological.iter().any(|e| matches!(
                        e,
                        Event::Splayed {
                            to: Splay::Right,
                            ..
                        }
                    )))
            );
        }
    }
}

#[test]
fn octant_transfers_either_icon_once_per_non_red_color_then_draws() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E047");
    let low = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
    f.cards[low.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let mut expected = vec![];
    for (color, icons) in [
        (Color::Blue, vec![Icon::Health]),
        (Color::Green, vec![Icon::Industry]),
        (Color::Purple, vec![Icon::Health, Icon::Industry]),
    ] {
        let c = tech(&mut f, color, 3, Zone::Board(A, color));
        for (i, icon) in icons.into_iter().enumerate() {
            f.cards[c.0 as usize].symbols[i] = Symbol::Icon(icon);
        }
        expected.push(c);
    }
    let red = tech(&mut f, Color::Red, 4, Zone::Board(A, Color::Red));
    f.cards[red.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
    let yellow = tech(&mut f, Color::Yellow, 2, Zone::Board(A, Color::Yellow));
    let draw = draw_card(&mut f, Color::Blue, 6);
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    for c in expected {
        assert_eq!(
            game.position.locate(c),
            Some(Zone::Board(B, game.position.card(c).color))
        );
    }
    for c in [low, red, yellow] {
        assert_eq!(
            game.position.locate(c),
            Some(Zone::Board(A, game.position.card(c).color))
        );
    }
    assert_eq!(game.position.locate(draw), Some(Zone::Forecast(A)));
    assert!(reveals(&events).is_empty());
    let draw_index = events
        .iter()
        .position(|e| matches!(e,Event::Moved(m) if m.card==draw))
        .unwrap();
    assert_eq!(
        events[..draw_index]
            .iter()
            .filter(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Transfer)))
            .count(),
        3
    );
}

#[test]
fn octant_foreseen_suppresses_only_the_victims_followup_and_forecast_capacity_is_live() {
    for (foreseen, eligible, full) in [
        (false, false, false),
        (false, true, false),
        (false, true, true),
        (true, true, false),
    ] {
        let mut f = Fixture::default();
        let card = f.add(
            echoes_card("E047").unwrap(),
            if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Board(A, Color::Red)
            },
        );
        let victim = if foreseen { B } else { A };
        if eligible {
            let c = tech(&mut f, Color::Blue, 3, Zone::Board(victim, Color::Blue));
            f.cards[c.0 as usize].symbols[0] = Symbol::Icon(Icon::Industry);
        }
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Purple, 7, Zone::Forecast(victim));
            }
        }
        let draw = draw_card(&mut f, Color::Green, 6);
        let promotion = tech(&mut f, Color::Red, 6, Zone::Hand(A));
        let mut game = if foreseen {
            promote(f, promotion, E)
        } else {
            start(f, card, 0, E)
        };
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(drawn(&events), if eligible { vec![draw] } else { vec![] });
        if foreseen {
            assert!(game.position.cards(Zone::Forecast(B)).is_empty());
            assert_eq!(
                game.position.locate(draw),
                Some(Zone::Forecast(A)),
                "the independent D1 still runs"
            );
        } else if eligible {
            assert_eq!(
                game.position.locate(draw),
                Some(if full {
                    Zone::Hand(A)
                } else {
                    Zone::Forecast(A)
                })
            );
        }
    }
}

#[test]
fn octant_transfer_oracle_preserves_every_path_and_nobels_actual_first_junk() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E047");
        for color in [Color::Blue, Color::Green] {
            let c = tech(&mut f, color, 3, Zone::Board(A, color));
            f.cards[c.0 as usize].symbols[0] = Symbol::Icon(Icon::Health);
        }
        draw_card(&mut f, Color::Purple, 6);
        let extra = if reacting {
            figure(&mut f, Figure::AlfredNobel, B);
            Some(draw_card(&mut f, Color::Yellow, 7))
        } else {
            None
        };
        let leaves = oracle(start(f, card, 0, if reacting { EF } else { E }), !reacting);
        if let Some(extra) = extra {
            assert!(leaves.iter().all(|leaf|leaf.chronological.iter().any(|e|matches!(e,Event::Moved(m) if m.card==extra && m.verb==Some(Verb::Junk) && m.inside_karma))));
        }
    }
}

#[test]
fn seed_drill_demand_uses_plain_three_and_preserves_the_top_choice() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E051");
    let one = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let two = tech(&mut f, Color::Yellow, 2, Zone::Board(A, Color::Yellow));
    let three = tech(&mut f, Color::Green, 3, Zone::Board(A, Color::Green));
    let archimedes = figure(&mut f, Figure::Archimedes, A);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    assert_eq!(offered(&m), [one, archimedes, two]);
    assert!(!offered(&m).contains(&three));
    pick_card(&mut game, &m, two);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(two), Some(Zone::Deck(Deck::base(2))));
}

#[test]
fn seed_drill_keeps_deck_choice_and_requires_real_junk_before_highest_eligibility() {
    for (age, accept, present, highest) in [
        (3, false, true, 6),
        (3, true, false, 6),
        (3, true, true, 6),
        (4, true, true, 6),
        (5, true, true, 6),
        (3, true, true, 9),
    ] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E051");
        tech(&mut f, Color::Blue, 6, Zone::Board(A, Color::Blue));
        for _ in 0..3 {
            tech(&mut f, Color::Purple, 10, Zone::Score(A));
        }
        let high = tech(&mut f, Color::Red, highest, Zone::Junk);
        let low = tech(&mut f, Color::Yellow, 1, Zone::Junk);
        let swept = present.then(|| draw_card(&mut f, Color::Green, age));
        let echo = tech(
            &mut f,
            Color::Purple,
            age,
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age,
            }),
        );
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        let mut game = start(f, card, 1, E);
        let mut events = vec![];
        if present {
            // The two empty decks are one option beside the real one.
            let m = menu(&mut game, &mut events);
            assert_eq!(m.options.len(), 2);
            let index = m
                .options
                .iter()
                .position(|option| *option == Alternative::Value(age))
                .unwrap();
            game.choose(index).unwrap();
            let m = menu(&mut game, &mut events);
            if accept {
                game.choose(0).unwrap();
            } else {
                decline(&mut game, &m);
            }
        }
        // With every deck empty the value choice resolves alone.
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(high) == Some(Zone::Achievements(A)),
            present && accept && highest < 9
        );
        assert_eq!(game.position.locate(low), Some(Zone::Junk));
        assert_eq!(
            game.position.locate(echo),
            Some(Zone::Deck(Deck {
                set: CardSet::Echoes,
                age
            }))
        );
        if let Some(c) = swept {
            assert_eq!(
                game.position.locate(c),
                Some(if accept {
                    Zone::Junk
                } else {
                    Zone::Deck(Deck::base(age))
                })
            );
        }
        assert!(reveals(&events).is_empty());
    }
}

#[test]
fn seed_drill_deck_top_oracle_preserves_every_path_and_rod_serlings_real_junk_reaction() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E051");
        tech(&mut f, Color::Blue, 6, Zone::Board(A, Color::Blue));
        for _ in 0..3 {
            tech(&mut f, Color::Purple, 10, Zone::Score(A));
        }
        let highest = tech(&mut f, Color::Yellow, 6, Zone::Junk);
        for color in [Color::Red, Color::Green] {
            draw_card(&mut f, color, 3);
        }
        // Every offered deck holds a card, so the empty-deck alias leaves
        // the value menu whole and only the junk order is at stake.
        draw_card(&mut f, Color::Yellow, 4);
        draw_card(&mut f, Color::Blue, 5);
        if reacting {
            figure(&mut f, Figure::RodSerling, B);
        }
        let leaves = oracle(start(f, card, 1, if reacting { EF } else { E }), !reacting);
        if reacting {
            assert!(
                leaves
                    .iter()
                    .any(|leaf| leaf.outcome.status == Status::Winner(B))
            );
        } else {
            assert!(
                leaves.iter().any(
                    |leaf| leaf.outcome.position.locate(highest) == Some(Zone::Achievements(A))
                )
            );
        }
    }
}

#[test]
fn seed_drill_archimedes_adjusts_all_deck_choices_but_does_not_reveal_junk_faces() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E051");
    figure(&mut f, Figure::Archimedes, A);
    let swept = draw_card(&mut f, Color::Red, 4);
    let unchanged = draw_card(&mut f, Color::Green, 3);
    let mut game = start(f, card, 1, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    // Archimedes shifts the offered values; the empty [5] and [6] are one.
    assert_eq!(m.options, [Alternative::Value(4), Alternative::Value(5)]);
    game.choose(0).unwrap();
    let m = menu(&mut game, &mut events);
    assert!(matches!(
        m.options[0],
        Alternative::Move {
            target: TargetDescriptor::Hidden { .. },
            ..
        }
    ));
    game.choose(0).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(swept), Some(Zone::Junk));
    assert_eq!(
        game.position.locate(unchanged),
        Some(Zone::Deck(Deck::base(3)))
    );
    assert!(reveals(&events).is_empty());
}

#[test]
fn lightning_rod_deck_top_oracle_is_base_only_and_archimedes_really_shifts_the_deck() {
    for archimedes in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E052");
        if archimedes {
            figure(&mut f, Figure::Archimedes, A);
        }
        let age = if archimedes { 7 } else { 6 };
        let junked: Vec<_> = [Color::Red, Color::Green]
            .map(|color| draw_card(&mut f, color, age))
            .into();
        let untouched = draw_card(&mut f, Color::Yellow, if archimedes { 6 } else { 7 });
        let echo = tech(
            &mut f,
            Color::Purple,
            age,
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age,
            }),
        );
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        let leaves = oracle(start(f, card, 2, if archimedes { EF } else { E }), true);
        for leaf in leaves {
            for c in &junked {
                assert_eq!(leaf.outcome.position.locate(*c), Some(Zone::Junk));
            }
            assert!(matches!(
                leaf.outcome.position.locate(untouched),
                Some(Zone::Deck(_))
            ));
            assert_eq!(
                leaf.outcome.position.locate(echo),
                Some(Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age
                }))
            );
            assert!(reveals(&leaf.chronological).is_empty());
        }
    }
    let mut f = Fixture::default();
    let card = source(&mut f, "E052");
    let next = draw_card(&mut f, Color::Red, 7);
    let mut game = start(f, card, 2, E);
    finish(&mut game, &mut vec![], |_| {
        panic!("empty physical deck has no choice")
    });
    assert_eq!(game.position.locate(next), Some(Zone::Deck(Deck::base(7))));
}

#[test]
fn thermometer_echo_reorders_the_green_bottom_without_losing_splay() {
    for count in 0..=3 {
        for direction in [Splay::Right, Splay::Up] {
            let mut f = Fixture::default();
            let card = f.add(echoes_card("E053").unwrap(), Zone::Board(A, Color::Blue));
            let greens: Vec<_> = (0..count)
                .map(|i| tech(&mut f, Color::Green, i + 1, Zone::Board(A, Color::Green)))
                .collect();
            let mut game = start(f, card, Card::ECHO, E);
            if count > 1 {
                game.position.players[A.index()].splays[Color::Green as usize] = direction;
            }
            finish(&mut game, &mut vec![], |_| 0);
            let mut expected = greens.clone();
            if count > 1 {
                expected.rotate_left(1);
            }
            assert_eq!(game.position.cards(Zone::Board(A, Color::Green)), expected);
            assert_eq!(
                game.position.splay(A, Color::Green),
                if count > 1 { direction } else { Splay::None }
            );
        }
    }
}

#[test]
fn thermometer_recomputes_yellow_value_and_computed_draws_ignore_archimedes() {
    for archimedes in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E053");
        if archimedes {
            figure(&mut f, Figure::Archimedes, A);
        }
        let unused = draw_card(&mut f, Color::Purple, 1);
        let first = draw_card(&mut f, Color::Yellow, 1);
        let second = draw_card(&mut f, Color::Yellow, 2);
        let last = draw_card(&mut f, Color::Purple, 3);
        let mut game = start(f, card, 0, if archimedes { EF } else { E });
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(drawn(&events), [first, second, last]);
        assert_eq!(
            game.position.locate(unused),
            Some(Zone::Deck(Deck::base(1)))
        );
        assert_eq!(
            game.position.cards(Zone::Board(A, Color::Yellow)),
            [first, second]
        );
        assert_eq!(
            game.position.locate(last),
            Some(Zone::Board(A, Color::Purple))
        );
    }
}

#[test]
fn thermometer_repeats_red_or_purple_only_when_foreseen_and_stops_after_replaced_meld() {
    for color in [Color::Red, Color::Purple] {
        for foreseen in [false, true] {
            let mut f = Fixture::default();
            let card = f.add(
                echoes_card("E053").unwrap(),
                if foreseen {
                    Zone::Forecast(A)
                } else {
                    Zone::Board(A, Color::Blue)
                },
            );
            let stop = draw_card(&mut f, Color::Green, 1);
            let first = draw_card(&mut f, color, 1);
            let promotion = tech(&mut f, Color::Blue, 6, Zone::Hand(A));
            let mut game = if foreseen {
                promote(f, promotion, E)
            } else {
                start(f, card, 0, E)
            };
            let mut events = vec![];
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(
                drawn(&events),
                if foreseen {
                    vec![first, stop]
                } else {
                    vec![first]
                }
            );
        }
    }
    let mut f = Fixture::default();
    let card = source(&mut f, "E053");
    let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
    let original = draw_card(&mut f, Color::Yellow, 1);
    let second = draw_card(&mut f, Color::Green, 2);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    finish(&mut game, &mut events, |m| {
        m.options.iter().position(|a|matches!(a,Alternative::Move{verb:Verb::Score,target:TargetDescriptor::Known{card,..},..} if *card==napoleon)).unwrap_or(0)
    });
    assert_eq!(drawn(&events), [original]);
    assert_eq!(
        game.position.locate(second),
        Some(Zone::Deck(Deck::base(2)))
    );
    assert!(events.iter().any(|e|matches!(e,Event::Moved(m) if m.card==original && m.verb==Some(Verb::Meld) && m.inside_karma)));
}

#[test]
fn piano_and_tuning_fork_echoes_choose_distinct_values_from_both_players_backs_or_draw_a_zero() {
    for (code, is_hand) in [("E054", true), ("E055", false)] {
        let mut f = Fixture::default();
        let card = source(&mut f, code);
        for (p, age, set) in [
            (A, 2, CardSet::Base),
            (A, 2, CardSet::Echoes),
            (B, 4, CardSet::Base),
        ] {
            let zone = if is_hand {
                Zone::Hand(p)
            } else {
                Zone::Score(p)
            };
            let c = tech(&mut f, Color::Red, age, zone);
            f.cards[c.0 as usize].set = set;
        }
        let chosen = draw_card(&mut f, Color::Green, 4);
        let mut game = start(f, card, Card::ECHO, E);
        let mut events = vec![];
        let m = menu(&mut game, &mut events);
        assert_eq!(m.options, [Alternative::Value(2), Alternative::Value(4)]);
        game.choose(1).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.locate(chosen), Some(Zone::Hand(A)));
        assert!(reveals(&events).is_empty());
        let mut f = Fixture::default();
        let card = source(&mut f, code);
        // No value to choose: draw a 0, here a [1].
        let one = draw_card(&mut f, Color::Yellow, 1);
        let mut game = start(f, card, Card::ECHO, E);
        finish(&mut game, &mut vec![], |_| {
            panic!("no value in either empty pile")
        });
        assert_eq!(game.position.locate(one), Some(Zone::Hand(A)));
    }
}

fn piano_tops(f: &mut Fixture) -> CardId {
    let card = source(f, "E054");
    for (color, age) in [
        (Color::Blue, 1),
        (Color::Green, 2),
        (Color::Red, 3),
        (Color::Yellow, 4),
    ] {
        tech(f, color, age, Zone::Board(A, color));
    }
    card
}

#[test]
fn piano_requires_five_distinct_tops_but_draws_after_short_or_empty_returns() {
    for score_count in [0_usize, 2, 5, 6] {
        let mut f = Fixture::default();
        let card = piano_tops(&mut f);
        for _ in 0..score_count {
            tech(&mut f, Color::Red, 8, Zone::Score(A));
        }
        let drawn_cards: Vec<_> = (1..=5)
            .map(|age| draw_card(&mut f, Color::Green, age))
            .collect();
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(drawn(&events), drawn_cards);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Return)))
                .count(),
            score_count.min(5)
        );
        assert_eq!(
            game.position.cards(Zone::Score(A)).len(),
            5 + score_count.saturating_sub(5)
        );
    }
    for distinct in [false, true] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E054");
        for (color, age) in [(Color::Green, 2), (Color::Red, 3), (Color::Yellow, 4)] {
            if distinct && color == Color::Yellow {
                continue;
            }
            tech(&mut f, color, age, Zone::Board(A, color));
        }
        // The source helper's blue5 duplicates Piano5; the other case has only four colors.
        let draw = draw_card(&mut f, Color::Green, 1);
        let score = tech(&mut f, Color::Red, 8, Zone::Score(A));
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| panic!("failed guard"));
        assert!(
            !events.iter().any(|e| matches!(e, Event::Moved(_))),
            "a failed guard does no return/draw/score work"
        );
        assert_eq!(game.position.locate(draw), Some(Zone::Deck(Deck::base(1))));
        assert_eq!(game.position.locate(score), Some(Zone::Score(A)));
    }
}

#[test]
fn piano_freezes_values_after_peter_returns_and_draws_in_ascending_order() {
    let mut f = Fixture::default();
    let card = piano_tops(&mut f);
    figure(&mut f, Figure::PeterTheGreat, A);
    tech(&mut f, Color::Purple, 3, Zone::Board(A, Color::Purple));
    // Peter the Great occupies red5, with a purple3 covering Piano; returning the single score card makes green disappear
    // before the value loop is captured. Short returns still authorize every draw.
    let initial_score = tech(&mut f, Color::Red, 8, Zone::Score(A));
    let undrawn = draw_card(&mut f, Color::Blue, 2);
    let second = draw_card(&mut f, Color::Blue, 3);
    let third = draw_card(&mut f, Color::Blue, 4);
    let fourth = draw_card(&mut f, Color::Blue, 5);
    let mut game = start(f, card, 0, EF);
    let green = game.position.cards(Zone::Board(A, Color::Green))[0];
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(drawn(&events), [undrawn, second, third, fourth]);
    assert_eq!(game.position.locate(green), Some(Zone::Deck(Deck::base(2))));
    assert_eq!(
        game.position.locate(initial_score),
        Some(Zone::Deck(Deck::base(8)))
    );
    assert!(game.position.cards(Zone::Board(A, Color::Green)).is_empty());
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.inside_karma && m.verb==Some(Verb::Score)))
    );
}

#[test]
fn piano_return_quota_oracle_preserves_subsets_same_deck_order_and_peters_live_arrival() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let card = piano_tops(&mut f);
        if reacting {
            figure(&mut f, Figure::PeterTheGreat, A);
            tech(&mut f, Color::Purple, 3, Zone::Board(A, Color::Purple));
        }
        tech(&mut f, Color::Red, 8, Zone::Score(A));
        tech(&mut f, Color::Yellow, 8, Zone::Score(A));
        let echo = tech(&mut f, Color::Purple, 8, Zone::Score(A));
        f.cards[echo.0 as usize].set = CardSet::Echoes;
        for age in 1..=5 {
            draw_card(&mut f, Color::Blue, age);
        }
        let leaves = oracle(start(f, card, 0, if reacting { EF } else { E }), !reacting);
        if reacting {
            assert!(leaves.iter().all(|leaf| leaf.chronological.iter().any(
                |e| matches!(e,Event::Moved(m) if m.inside_karma && m.verb==Some(Verb::Score))
            )));
        }
    }
    let mut f = Fixture::default();
    let card = piano_tops(&mut f);
    for color in [
        Color::Red,
        Color::Yellow,
        Color::Purple,
        Color::Blue,
        Color::Green,
        Color::Red,
    ] {
        tech(&mut f, color, 8, Zone::Score(A));
    }
    for age in 1..=5 {
        draw_card(&mut f, Color::Blue, age);
    }
    assert_eq!(
        oracle(start(f, card, 0, E), false).len(),
        720,
        "all six subsets and every same-deck return order survive"
    );
}

#[test]
fn tuning_fork_full_forecast_ends_before_draw_return_or_repeat() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E055");
    for _ in 0..5 {
        tech(&mut f, Color::Yellow, 2, Zone::Forecast(A));
    }
    let kept = tech(&mut f, Color::Green, 3, Zone::Hand(A));
    let undrawn = draw_card(&mut f, Color::Red, 3);
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| {
        panic!("refused admission is not a choice")
    });
    assert_eq!(game.position.locate(kept), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(undrawn),
        Some(Zone::Deck(Deck::base(3)))
    );
    assert!(reveals(&events).is_empty());
    assert!(events.iter().all(|e| !matches!(e, Event::Moved(_))));
}

#[test]
fn tuning_fork_reveals_the_actual_draw_and_compares_strictly_with_its_live_color_top() {
    for (top, draw_age) in [(0, 2), (1, 2), (2, 2), (3, 2), (2, 3)] {
        let mut f = Fixture::default();
        let card = source(&mut f, "E055");
        if top > 0 {
            tech(&mut f, Color::Green, top, Zone::Board(A, Color::Green));
        }
        let foreshadowed = tech(&mut f, Color::Red, 2, Zone::Hand(A));
        let original = draw_card(&mut f, Color::Green, draw_age);
        let mut game = start(f, card, 0, E);
        let mut events = vec![];
        finish(&mut game, &mut events, |m| {
            m.options
                .iter()
                .position(|a| *a == Alternative::Decline)
                .unwrap_or(0)
        });
        assert_eq!(game.position.locate(foreshadowed), Some(Zone::Forecast(A)));
        assert_eq!(reveals(&events), [original]);
        assert_eq!(
            game.position.locate(original),
            Some(if draw_age > top {
                Zone::Board(A, Color::Green)
            } else {
                Zone::Deck(Deck::base(draw_age))
            })
        );
        let reveal = events
            .iter()
            .position(|e| matches!(e,Event::Revealed{card,..} if *card==original))
            .unwrap();
        assert!(
            events[reveal + 1..]
                .iter()
                .any(|e| matches!(e,Event::Moved(m) if m.card==original))
        );
    }
}

#[test]
fn tuning_fork_optional_repeat_starts_with_another_forecast_choice() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E055");
    tech(&mut f, Color::Green, 4, Zone::Board(A, Color::Green));
    let first = tech(&mut f, Color::Red, 2, Zone::Hand(A));
    let second = tech(&mut f, Color::Yellow, 3, Zone::Hand(A));
    let returned = draw_card(&mut f, Color::Green, 2);
    let melded = draw_card(&mut f, Color::Red, 3);
    let mut game = start(f, card, 0, E);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, first);
    let repeat = menu(&mut game, &mut events);
    assert_eq!(
        repeat.options,
        [Alternative::Branch(0), Alternative::Decline]
    );
    game.choose(0).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.cards(Zone::Forecast(A)), [first, second]);
    assert_eq!(
        game.position.locate(returned),
        Some(Zone::Deck(Deck::base(2)))
    );
    assert_eq!(
        game.position.locate(melded),
        Some(Zone::Board(A, Color::Red))
    );
    assert_eq!(reveals(&events), [returned, melded]);
}

#[test]
fn tuning_fork_replaced_meld_does_not_return_the_original_from_its_new_board() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E055");
    let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
    let foreshadowed = tech(&mut f, Color::Red, 2, Zone::Hand(A));
    let original = draw_card(&mut f, Color::Green, 2);
    let mut game = start(f, card, 0, EF);
    let mut events = vec![];
    let m = menu(&mut game, &mut events);
    pick_card(&mut game, &m, napoleon);
    let m = menu(&mut game, &mut events);
    assert_eq!(m.options, [Alternative::Branch(0), Alternative::Decline]);
    assert_eq!(game.position.locate(foreshadowed), Some(Zone::Forecast(A)));
    assert_eq!(
        game.position.locate(original),
        Some(Zone::Board(A, Color::Green))
    );
    for viewer in [A] {
        for seed in 0..8 {
            let mut sampled = game.determinize(viewer, seed).unwrap();
            assert_eq!(
                sampled.observe(viewer).unwrap(),
                game.observe(viewer).unwrap()
            );
            assert_eq!(menu(&mut sampled, &mut vec![]), m);
        }
    }
    decline(&mut game, &m);
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(reveals(&events), [original]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==original && m.verb==Some(Verb::Return)))
    );
}

#[test]
fn seed_drill_junk_achievement_ties_stay_private_through_hedy_and_sampling() {
    let mut f = Fixture::default();
    let card = source(&mut f, "E051");
    figure(&mut f, Figure::HedyLamarr, A);
    for _ in 0..2 {
        tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue));
    }
    for _ in 0..2 {
        tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
    }
    for _ in 0..2 {
        tech(&mut f, Color::Yellow, 10, Zone::Score(A));
    }
    for color in [Color::Blue, Color::Red] {
        tech(&mut f, color, 4, Zone::Junk);
    }
    let echo = tech(&mut f, Color::Blue, 4, Zone::Junk);
    f.cards[echo.0 as usize].set = CardSet::Echoes;
    draw_card(&mut f, Color::Purple, 3);
    let mut game = start(f, card, 1, EF);
    let mut events = vec![];
    let values = menu(&mut game, &mut events);
    // The empty [4] and [5] decks are one option.
    assert_eq!(
        values.options,
        [Alternative::Value(3), Alternative::Value(4)]
    );
    game.choose(0).unwrap();
    menu(&mut game, &mut events);
    game.choose(0).unwrap();
    let m = menu(&mut game, &mut events);
    assert_eq!(m.options.len(), 2);
    assert!(m.options.iter().all(|a| matches!(
        a,
        Alternative::BlindMove {
            verb: Verb::Achieve,
            source: Zone::Junk,
            ..
        }
    )));
    let index = m
        .options
        .iter()
        .position(|a| matches!(a,Alternative::BlindMove{back, ..} if *back==Deck::base(4)))
        .unwrap();
    let mut rng = game.rng.clone();
    rng.next_u64();
    game.choose(index).unwrap();
    let pending_menu = menu(&mut game, &mut events);
    assert_eq!(game.rng, rng);
    assert!(
        pending_menu
            .options
            .iter()
            .all(|a| matches!(a, Alternative::Splay { .. }))
    );
    assert!(reveals(&events).is_empty());
    for player in [A] {
        for seed in 0..12 {
            for mut world in [
                game.determinize(player, seed).unwrap(),
                game.sample_world(player, seed).unwrap().world,
            ] {
                assert_eq!(menu(&mut world, &mut vec![]), pending_menu);
                let selected = world
                    .frames
                    .iter()
                    .find_map(|frame| match frame {
                        Frame::Operation {
                            request: Operation::Claim { target, .. },
                            ..
                        } => Some(target.card),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(world.position.card(selected).origin_deck(), Deck::base(4));
                assert_eq!(world.position.locate(selected), Some(Zone::Junk));
                let mut events = vec![];
                world.choose(0).unwrap();
                finish(&mut world, &mut events, |_| 0);
                assert_eq!(world.position.locate(selected), Some(Zone::Achievements(A)));
                assert_eq!(world.position.locate(echo), Some(Zone::Junk));
                assert!(reveals(&events).is_empty());
                assert!(
                    events.iter().any(|e| matches!(e, Event::Splayed { .. })),
                    "Hedy actually changes a splay"
                );
            }
        }
    }
}
