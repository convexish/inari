use inno_engine::*;

mod common;
use common::*;

fn name(code: &str) -> &'static FigureName {
    FIGURE_NAMES.iter().find(|n| n.code == code).unwrap()
}

fn choose_deck(game: &mut Game, events: &mut Vec<Event>, age: u8) -> Decision {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Deck(Deck::figures(age)))
        .unwrap();
    game.choose(index).unwrap();
    decision(game, events)
}

fn choose_name(game: &mut Game, events: &mut Vec<Event>, code: Option<&str>) {
    let menu = decision(game, events);
    let option = code.map_or(Alternative::Pass, |code| {
        Alternative::NameFigure(name(code))
    });
    game.choose(menu.options.iter().position(|o| *o == option).unwrap())
        .unwrap();
}

fn draw(game: &mut Game, player: Player, set: Option<CardSet>, value: u8) {
    game.start(Operation::Draw { set, value }, Context::new(player))
        .unwrap();
}

#[test]
fn naming_the_card_that_would_have_been_drawn_still_draws_the_next_card() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let next = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    let named = f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    let base = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, A, Some(CardSet::Figures), 7);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 7);
    choose_name(&mut game, &mut events, Some("F069"));
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(next),
            value: Some(7),
            short: false,
        }))
    );
    assert_eq!(events.len(), 4);
    assert_eq!(moved(&events[1]).card, named);
    assert_eq!(moved(&events[1]).verb, Some(Verb::Transfer));
    assert!(matches!(events[2], Event::Shuffled { .. }));
    assert_eq!(moved(&events[3]).card, next);
    assert_eq!(moved(&events[3]).verb, None);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[named, next]);
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::figures(7)))
            .is_empty()
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), &[base]);
    assert!(game.position().check_conservation());
}

#[test]
fn the_original_draw_takes_the_post_shuffle_top_card() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    let nightingale = f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 0).unwrap();
    let mut events = Vec::new();
    // The first seed-0 shuffle preserves order, the second swaps two cards.
    draw(&mut game, A, None, 1);
    choose_deck(&mut game, &mut events, 7);
    choose_name(&mut game, &mut events, None);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(7))),
        &[meiji, nightingale]
    );
    events.clear();
    draw(&mut game, A, Some(CardSet::Figures), 7);
    choose_deck(&mut game, &mut events, 7);
    choose_name(&mut game, &mut events, None);
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(meiji),
            value: Some(7),
            short: false,
        }))
    );
    assert_eq!(events.len(), 3);
    assert!(matches!(events[1], Event::Shuffled { .. }));
    assert_eq!(moved(&events[2]).card, meiji);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(7))),
        &[nightingale]
    );
    assert!(game.position().check_conservation());
}

#[test]
fn deck_then_name_uses_original_rosters_including_empty_decks_and_public_figures() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    draw(&mut game, A, None, 1);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(
        menu.options,
        (1..=11)
            .map(|age| Alternative::Deck(Deck::figures(age)))
            .collect::<Vec<_>>()
    );
    assert!(events.is_empty());
    let mut all_names = Vec::new();
    for age in 1..=11 {
        let mut fork = game.clone();
        let names = choose_deck(&mut fork, &mut events, age);
        assert_eq!(names.player, A);
        assert_eq!(names.options.len(), if age == 1 { 16 } else { 11 });
        assert_eq!(names.options.last(), Some(&Alternative::Pass));
        for option in &names.options[..names.options.len() - 1] {
            let Alternative::NameFigure(figure) = option else {
                panic!("name expected")
            };
            assert_eq!(figure.age, age);
            all_names.push(figure.code);
        }
        if age == 10 {
            // Trebek himself is on the board, yet his name remains available.
            assert!(
                names
                    .options
                    .contains(&Alternative::NameFigure(name("F098")))
            );
        }
        let before = fork.clone();
        assert_eq!(fork.choose(names.options.len()), Err(Error::InvalidChoice));
        assert_eq!(fork, before);
        assert!(events.is_empty());
    }
    all_names.sort_unstable();
    all_names.dedup();
    assert_eq!(all_names.len(), 115);
    assert_eq!(all_names.first(), Some(&"F001"));
    assert_eq!(all_names.last(), Some(&"F115"));
}

#[test]
fn hit_transfers_the_exact_buried_figure_then_shuffles_then_resumes_the_draw() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    let nightingale = f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    let drawn = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    draw(&mut game, A, None, 2);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 7);
    choose_name(&mut game, &mut events, Some("F074"));
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(drawn),
            value: Some(2),
            short: false,
        }))
    );
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[0],
        Event::FigureSearch {
            executor: A,
            deck: Deck::figures(7),
            name: Some(name("F074")),
            found: Some(meiji)
        }
    );
    assert_eq!(
        moved(&events[1]),
        &Movement {
            verb: Some(Verb::Transfer),
            card: meiji,
            source: Zone::Deck(Deck::figures(7)),
            destination: Zone::Hand(A),
            executor: A,
            inside_karma: true
        }
    );
    assert_eq!(
        events[2],
        Event::Shuffled {
            executor: A,
            deck: Deck::figures(7)
        }
    );
    assert_eq!(moved(&events[3]).card, drawn);
    assert!(!moved(&events[3]).inside_karma);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[meiji, drawn]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(7))),
        &[nightingale]
    );
    assert!(game.position().check_conservation());
}

#[test]
fn hidden_worlds_have_identical_deck_and_name_menus_for_hits_and_misses() {
    let make = |present| {
        let mut f = Fixture::default();
        f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
        let meiji = f.figure(
            Figure::EmperorMeiji,
            if present {
                Zone::Deck(Deck::figures(7))
            } else {
                Zone::Hand(B)
            },
        );
        f.figure(
            Figure::Nightingale,
            if present {
                Zone::Hand(B)
            } else {
                Zone::Deck(Deck::figures(7))
            },
        );
        f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
        (Game::new(f.position(), Expansions::FIGURES).unwrap(), meiji)
    };
    let mut menus = Vec::new();
    for present in [false, true] {
        let (mut game, meiji) = make(present);
        draw(&mut game, A, None, 2);
        let mut events = Vec::new();
        let decks = decision(&mut game, &mut events);
        let names = choose_deck(&mut game, &mut events, 7);
        menus.push((decks, names));
        assert!(events.is_empty());
        choose_name(&mut game, &mut events, Some("F074"));
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(
            events[0],
            Event::FigureSearch {
                executor: A,
                deck: Deck::figures(7),
                name: Some(name("F074")),
                found: present.then_some(meiji)
            }
        );
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&meiji),
            present
        );
        if !present {
            assert!(game.position().cards(Zone::Hand(B)).contains(&meiji));
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Shuffled { .. }))
                .count(),
            1
        );
        assert!(game.position().check_conservation());
    }
    assert_eq!(menus[0], menus[1]);
}

#[test]
fn intentional_whiff_still_shuffles_and_uses_the_persistent_frozen_rng_stream() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    let nightingale = f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 0).unwrap();
    let mut events = Vec::new();
    // Frozen seed-0 outputs end in 535 (odd), then 700 (even): a two-card
    // Fisher-Yates shuffle first preserves order, then swaps it.
    for expected in [[meiji, nightingale], [nightingale, meiji]] {
        draw(&mut game, A, None, 1);
        choose_deck(&mut game, &mut events, 7);
        choose_name(&mut game, &mut events, None);
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(7))),
            &expected
        );
    }
    assert_eq!(events.len(), 6);
    for batch in events.as_chunks::<3>().0 {
        assert_eq!(
            batch[0],
            Event::FigureSearch {
                executor: A,
                deck: Deck::figures(7),
                name: None,
                found: None
            }
        );
        assert_eq!(
            batch[1],
            Event::Shuffled {
                executor: A,
                deck: Deck::figures(7)
            }
        );
        assert_eq!(moved(&batch[2]).verb, None);
    }
    assert!(
        game.position().cards(Zone::Hand(A)).iter().all(|&c| game
            .position()
            .card(c)
            .figure
            .is_none())
    );
}

#[test]
fn absent_name_and_pass_leave_the_same_position_and_rng_but_distinct_search_facts() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 2).unwrap();
    draw(&mut game, A, None, 1);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 7);
    let mut whiff = game.clone();
    let mut whiff_events = Vec::new();
    choose_name(&mut game, &mut events, Some("F066"));
    choose_name(&mut whiff, &mut whiff_events, None);
    assert_eq!(
        settle(&mut game, &mut events),
        settle(&mut whiff, &mut whiff_events)
    );
    assert_eq!(game, whiff);
    assert_ne!(events[0], whiff_events[0]);
    assert_eq!(events[1..], whiff_events[1..]);
}

#[test]
fn empty_deck_still_allows_names_or_whiff_and_does_not_consume_rng() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let position = f.position();
    let mut game = Game::seeded(position.clone(), Expansions::FIGURES, 0).unwrap();
    let mut expected = Game::seeded(position, Expansions::FIGURES, 0).unwrap();
    draw(&mut game, A, None, 1);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 11);
    choose_name(&mut game, &mut events, Some("F115"));
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    // A different empty deck has the same RNG effect.
    draw(&mut expected, A, None, 1);
    choose_deck(&mut expected, &mut Vec::new(), 1);
    choose_name(&mut expected, &mut Vec::new(), None);
    settle(&mut expected, &mut Vec::new());
    assert_eq!(game, expected);
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[1],
        Event::Shuffled {
            executor: A,
            deck: Deck::figures(11)
        }
    );
    game.start(
        Operation::Shuffle {
            deck: Deck::figures(7),
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    // The first seed-0 shuffle must still preserve this two-card order.
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(7))),
        expected.position().cards(Zone::Deck(Deck::figures(7)))
    );
}

#[test]
fn original_figures_draw_falls_back_to_base_after_the_search_empties_its_deck() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    let meiji = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    let kalashnikov = f.figure(Figure::Kalashnikov, Zone::Deck(Deck::figures(9)));
    let base = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, A, Some(CardSet::Figures), 7);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 7);
    choose_name(&mut game, &mut events, Some("F074"));
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(base),
            value: Some(7),
            short: false,
        }))
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[meiji, base]);
    assert!(game.position().cards(Zone::Deck(Deck::base(7))).is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(9))),
        &[kalashnikov]
    );
    assert_eq!(
        moved(events.last().unwrap()).source,
        Zone::Deck(Deck::base(7))
    );
}

#[test]
fn search_transfer_shuffle_and_draw_resume_identically_at_every_work_boundary() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    draw(&mut game, A, None, 1);
    let mut events = Vec::new();
    choose_deck(&mut game, &mut events, 7);
    choose_name(&mut game, &mut events, Some("F074"));
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = settle(&mut expected, &mut expected_events);
    let mut complete = false;
    for _ in 0..100 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(settle(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        let status = game.advance(1, &mut events).unwrap();
        assert!(game.position().check_conservation());
        if status != Status::WorkLimit {
            assert_eq!(status, result);
            complete = true;
            break;
        }
    }
    assert!(complete);
    assert_eq!(game, expected);
}

#[test]
fn trebek_uses_logical_hand_membership_and_the_correct_executor() {
    for (turn_owner, executor, own_figure, covered) in [
        (A, A, true, false),
        (A, B, false, false),
        (A, A, false, true),
        (A, A, false, false),
        (B, A, false, false),
    ] {
        let mut f = Fixture::default();
        f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        f.figure(
            Figure::Nightingale,
            Zone::Hand(if own_figure { A } else { B }),
        );
        if covered {
            f.tech(Color::Yellow, 11, Zone::Board(A, Color::Yellow), vec![]);
        }
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_turn(turn_owner).unwrap();
        draw(&mut game, executor, None, 1);
        let status = settle(&mut game, &mut Vec::new());
        let should_trigger = turn_owner == B && executor == A && !own_figure && !covered;
        assert_eq!(matches!(status, Status::Decision(_)), should_trigger);
        if !should_trigger {
            assert!(matches!(status, Status::Complete(_)));
        }
    }
}

#[test]
fn trebek_is_suppressed_inside_kalashnikovs_foreign_self_execution() {
    let mut f = Fixture::default();
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.tech(
        Color::Red,
        9,
        Zone::Board(B, Color::Red),
        vec![build::draw(1)],
    );
    let incoming = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(incoming, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, drawn);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(moved(&events[1]).card, incoming);
}

#[test]
fn competing_hook_is_chosen_by_turn_owner_but_trebek_choices_belong_to_its_owner() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::JackieChan, Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_turn(B).unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    game.start(
        Operation::Draw {
            set: None,
            value: 1,
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    let hooks = decision(&mut game, &mut events);
    assert_eq!(hooks.player, B);
    assert_eq!(hooks.options.len(), 2);
    let index = hooks
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Karma {
                    figure: Figure::AlexTrebek,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(index).unwrap();
    assert_eq!(decision(&mut game, &mut events).player, A);
    assert_eq!(choose_deck(&mut game, &mut events, 1).player, A);
    choose_name(&mut game, &mut events, None);
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 3);
    assert_eq!(moved(&events[2]).executor, A);
}

#[test]
fn physical_decks_reject_cards_from_the_wrong_expansion() {
    let mut f = Fixture::default();
    let card = f.tech(Color::Blue, 7, Zone::Deck(Deck::figures(7)), vec![]);
    assert!(matches!(
        Position::new(f.cards, &[(card, Zone::Deck(Deck::figures(7)))], A),
        Err(Error::InvalidSetup(_))
    ));
    let mut f = Fixture::default();
    let card = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::base(7)));
    assert!(matches!(
        Position::new(f.cards, &[(card, Zone::Deck(Deck::base(7)))], A),
        Err(Error::InvalidSetup(_))
    ));
}
