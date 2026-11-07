use inno_engine::*;

mod common;
use common::*;

fn authority(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    let card = f.tech(color, value, zone, vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    card
}

fn replaced() -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Replaced,
        card: None,
        value: None,
        short: false,
    }))
}

/// The physical moves among the events (exchanged cards are revealed first).
fn moves(events: &[Event]) -> Vec<&Movement> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(movement) => Some(movement),
            _ => None,
        })
        .collect()
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

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        let status = settle(game, events);
        if matches!(status, Status::Decision(_)) {
            game.choose(0).unwrap();
        } else {
            return status;
        }
    }
    panic!("resolution did not terminate");
}

#[test]
fn transfer_replacement_runs_as_yi_owner_and_does_not_reach_original_destination() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let original = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let scored = authority(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(original, Zone::Hand(B)),
            destination: Zone::Score(B),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), replaced());
    assert_eq!(events.len(), 4);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [original, drawn, drawn, scored]
    );
    assert_eq!(
        events.iter().map(|e| moved(e).verb).collect::<Vec<_>>(),
        [Some(Verb::Tuck), None, Some(Verb::Tuck), Some(Verb::Score)]
    );
    assert!(
        events
            .iter()
            .all(|e| moved(e).executor == A && moved(e).inside_karma)
    );
    assert_eq!(game.position().top(A, Color::Green), Some(original));
    assert_eq!(game.position().top(A, Color::Yellow), Some(drawn));
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
    assert!(game.position().cards(Zone::Score(B)).is_empty());
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn entire_exchange_is_replaced_once_with_a_choice_of_tuck_order() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    let first = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let last = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    let middle = f.tech(Color::Green, 5, Zone::Score(B), vec![]);
    let drawn = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let scored = authority(&mut f, Color::Purple, 1, Zone::Board(B, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExchangeAll {
            left: LogicalZone::Hand(B),
            right: LogicalZone::Score(B),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    let choices = decision(&mut game, &mut events);
    assert_eq!(choices.player, A);
    // Every exchanged card is revealed, then tucked, so the tuck order is chosen among
    // known cards, one colour at a time, lowest first: the two blues are offered, the
    // green follows alone.
    assert_eq!(choices.options.len(), 2);
    let mut shown = revealed(&events);
    shown.sort();
    let mut exchanged = vec![first, last, middle];
    exchanged.sort();
    assert_eq!(shown, exchanged);
    assert!(moves(&events).is_empty());
    assert_eq!(game.position().cards(Zone::Hand(B)), &[first, last]);
    assert_eq!(game.position().cards(Zone::Score(B)), &[middle]);
    choose_card(&mut game, &mut events, first);
    // The remaining blue and then the green are the only options left,
    // so they resolve without a menu.
    assert_eq!(settle(&mut game, &mut events), replaced());
    let moves = moves(&events);
    assert_eq!(
        moves.iter().map(|m| m.card).collect::<Vec<_>>(),
        [first, last, middle, drawn, drawn, scored]
    );
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        &[last, first]
    );
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert!(game.position().cards(Zone::Score(B)).is_empty());
    assert_eq!(moves.iter().filter(|m| m.verb.is_none()).count(), 1);
    assert!(moves.iter().all(|m| m.inside_karma));
    assert!(game.position().check_conservation());
}

#[test]
fn replacement_cannot_satisfy_the_exchange_callers_if_performed() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let program = f.tech(
        Color::Blue,
        5,
        Zone::Board(A, Color::Blue),
        vec![
            Instruction::Exchange(ExchangeStep {
                left: build::HAND,
                right: build::SCORE,
                counts: &[None, None],
                optional: false,
            }),
            build::if_(build::DID, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(program, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(revealed(&events).len(), 1);
    assert_eq!(moves(&events).len(), 3);
    assert!(moves(&events).iter().all(|m| m.inside_karma));
}

#[test]
fn a_single_nonempty_exchange_side_is_still_replaced_in_either_orientation() {
    for reverse in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        let original = f.tech(Color::Blue, 2, Zone::Score(B), vec![]);
        f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let (left, right) = if reverse {
            (LogicalZone::Score(B), LogicalZone::Hand(B))
        } else {
            (LogicalZone::Hand(B), LogicalZone::Score(B))
        };
        game.start(Operation::ExchangeAll { left, right }, Context::new(B))
            .unwrap();
        assert_eq!(settle(&mut game, &mut Vec::new()), replaced());
        assert_eq!(game.position().top(A, Color::Blue), Some(original));
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert!(game.position().cards(Zone::Score(B)).is_empty());
    }
}

#[test]
fn exchanged_cards_are_revealed_before_the_tuck_order_menu() {
    // When Yi Sun-sin replaces an exchange, all the exchanged cards are revealed,
    // then tucked, so the order is chosen among known cards and the real deal decides
    // nothing hidden. Two cards of one colour: tucks of different colours would run one
    // colour at a time.
    let make = |reverse| {
        let mut f = Fixture::default();
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let green = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
        if reverse {
            f.placements.swap(usize::from(blue.0), usize::from(green.0));
        }
        f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::ExchangeAll {
                left: LogicalZone::Hand(B),
                right: LogicalZone::Score(B),
            },
            Context::new(B),
        )
        .unwrap();
        (game, [blue, green])
    };
    for reverse in [false, true] {
        let (mut game, cards) = make(reverse);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        let mut shown = revealed(&events);
        shown.sort();
        let mut expected = cards.to_vec();
        expected.sort();
        assert_eq!(shown, expected);
        assert!(moves(&events).is_empty());
        let mut named: Vec<_> = menu
            .options
            .iter()
            .map(|o| match o {
                Alternative::Move {
                    verb: Verb::Tuck,
                    target: TargetDescriptor::Known { card, .. },
                } => *card,
                other => panic!("a known card, not {other:?}"),
            })
            .collect();
        named.sort();
        assert_eq!(named, expected);
        let cloned = game.clone();
        assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
        assert_eq!(game, cloned);
    }
}

#[test]
fn yi_resolution_and_suppression_survive_tucking_yi_below_his_own_stack() {
    let mut f = Fixture::default();
    let exposed = authority(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    let yi = f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let drawn = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Transfer,
            target: target(yi, Zone::Board(A, Color::Red)),
            destination: Zone::Hand(B),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert!(!game.position().has_figure(A, Figure::YiSunSin));
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        &[yi, exposed]
    );
    // Trebek is active, but does not trigger on the draw inside Yi's karma.
    assert_eq!(settle(&mut game, &mut events), replaced());
    assert_eq!(events.len(), 4);
    assert_eq!(moved(&events[1]).card, drawn);
    assert_eq!(game.position().cards(Zone::Score(A)), &[exposed]);
    assert!(game.position().has_figure(A, Figure::YiSunSin));
}

#[test]
fn trebek_search_transfer_does_not_trigger_yi() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            set: None,
            value: 1,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let decks = decision(&mut game, &mut events);
    game.choose(
        decks
            .options
            .iter()
            .position(|o| *o == Alternative::Deck(Deck::figures(7)))
            .unwrap(),
    )
    .unwrap();
    let names = decision(&mut game, &mut events);
    game.choose(
        names
            .options
            .iter()
            .position(|o| matches!(o, Alternative::NameFigure(n) if n.code == "F074"))
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[meiji, drawn]);
    assert_eq!(events.len(), 4);
    assert_eq!(moved(&events[1]).verb, Some(Verb::Transfer));
}

#[test]
fn current_player_selects_one_competing_replacement() {
    for choose_yi in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        f.figure(Figure::Nightingale, Zone::Board(B, Color::Yellow));
        let original = f.tech(Color::Green, 2, Zone::Score(B), vec![]);
        f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_turn(B).unwrap();
        game.start(
            Operation::Move {
                verb: Verb::Transfer,
                target: target(original, Zone::Score(B)),
                destination: Zone::Hand(A),
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        assert_eq!(menu.options.len(), 2);
        let wanted = if choose_yi {
            Figure::YiSunSin
        } else {
            Figure::Nightingale
        };
        game.choose(
            menu.options
                .iter()
                .position(|o| matches!(o, Alternative::Karma { figure, .. } if *figure == wanted))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(settle(&mut game, &mut events), replaced());
        assert_eq!(events.len(), if choose_yi { 3 } else { 0 });
        assert_eq!(
            game.position().top(A, Color::Green),
            choose_yi.then_some(original)
        );
        assert_eq!(game.position().cards(Zone::Score(B)).is_empty(), choose_yi);
    }
}

#[test]
fn clone_and_resume_preserve_selected_groups_at_every_machine_boundary() {
    let mut f = Fixture::default();
    f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
    f.tech(Color::Green, 5, Zone::Score(B), vec![]);
    f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    authority(&mut f, Color::Purple, 1, Zone::Board(B, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExchangeAll {
            left: LogicalZone::Hand(B),
            right: LogicalZone::Score(B),
        },
        Context::new(B),
    )
    .unwrap();
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    assert_eq!(finish(&mut expected, &mut expected_events), replaced());
    let mut events = Vec::new();
    for _ in 0..100 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), replaced());
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, replaced());
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("resolution did not terminate");
}

#[test]
fn michelangelo_overlap_is_removed_before_yi_selects_cards_to_tuck() {
    for reverse in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
        let frozen = f.tech(Color::Green, 5, Zone::Hand(A), vec![]);
        let first = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
        let second = f.tech(Color::Blue, 3, Zone::Score(A), vec![]);
        let drawn = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let (left, right) = if reverse {
            (LogicalZone::Score(A), LogicalZone::Hand(A))
        } else {
            (LogicalZone::Hand(A), LogicalZone::Score(A))
        };
        game.start(Operation::ExchangeAll { left, right }, Context::new(A))
            .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 2);
        assert_eq!(menu.player, A);
        assert!(menu.options.iter().all(|option| matches!(option,
            Alternative::Move { verb: Verb::Tuck, target: TargetDescriptor::Known { card, .. } }
            if *card == first || *card == second)));
        choose_card(&mut game, &mut events, second);
        assert_eq!(settle(&mut game, &mut events), replaced());
        assert_eq!(
            moves(&events).iter().map(|m| m.card).collect::<Vec<_>>(),
            [second, first, drawn, drawn]
        );
        assert_eq!(game.position().cards(Zone::Hand(A)), &[frozen]);
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        assert_eq!(
            game.position().cards(Zone::Board(A, Color::Blue)),
            &[first, second]
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn meiji_membership_is_resolved_before_yis_whole_exchange_replacement() {
    for active in [false, true] {
        for reverse in [false, true] {
            let mut f = Fixture::default();
            f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
            f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
            let mine = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
            let theirs = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
            let drawn = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            if !active {
                game.begin_action(ActionKind::Regular).unwrap();
                game.begin_action(ActionKind::Regular).unwrap();
            }
            let (left, right) = if reverse {
                (LogicalZone::Hand(B), LogicalZone::Hand(A))
            } else {
                (LogicalZone::Hand(A), LogicalZone::Hand(B))
            };
            game.start(Operation::ExchangeAll { left, right }, Context::new(B))
                .unwrap();
            let mut events = Vec::new();
            assert_eq!(finish(&mut game, &mut events), replaced());
            assert!(game.position().cards(Zone::Hand(A)).is_empty());
            let retained = [theirs];
            assert_eq!(
                game.position().cards(Zone::Hand(B)),
                if active { &retained[..] } else { &[] }
            );
            assert_eq!(game.position().top(A, Color::Blue), Some(mine));
            assert_eq!(
                game.position().top(A, Color::Green),
                (!active).then_some(theirs)
            );
            assert_eq!(moves(&events).len(), if active { 3 } else { 4 });
            assert_eq!(
                moves(&events).iter().filter(|m| m.card == theirs).count(),
                usize::from(!active)
            );
            assert_eq!(game.position().top(A, Color::Yellow), Some(drawn));
            assert!(game.position().check_conservation());
        }
    }
}

#[test]
fn an_exchange_without_physical_movers_does_not_trigger_yi() {
    for modifier in [None, Some(Figure::Michelangelo), Some(Figure::EmperorMeiji)] {
        let mut f = Fixture::default();
        f.figure(Figure::YiSunSin, Zone::Board(A, Color::Red));
        let right = match modifier {
            Some(Figure::Michelangelo) => {
                f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
                f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
                LogicalZone::Score(A)
            }
            Some(Figure::EmperorMeiji) => {
                f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
                f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
                LogicalZone::Hand(B)
            }
            None => LogicalZone::Score(A),
            _ => unreachable!(),
        };
        f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
        let position = f.position();
        let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::ExchangeAll {
                left: LogicalZone::Hand(A),
                right,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Inapplicable,
                card: None,
                value: None,
                short: false,
            }))
        );
        assert!(events.is_empty());
        assert_eq!(game.position(), &position);
    }
}
