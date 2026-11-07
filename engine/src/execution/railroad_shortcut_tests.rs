use super::common::*;
use super::*;

fn fixture(ages: &[u8]) -> (Fixture, CardId, Vec<CardId>) {
    let mut f = Fixture::default();
    let railroad = f.add(base_card("B075").unwrap(), Zone::Board(A, Color::Purple));
    let hand = ages
        .iter()
        .map(|&age| f.tech(Color::Red, age, Zone::Hand(A), vec![]))
        .collect();
    (f, railroad, hand)
}

fn start(game: &mut Game, card: CardId, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            if let Alternative::Move {
                verb: Verb::Return,
                target: TargetDescriptor::Known { card, .. },
            } = option
            {
                let deck = game.position.card(*card).origin_deck();
                Some(((deck.age, deck.set as u8, *card), index))
            } else {
                None
            }
        })
        .min_by_key(|(key, _)| *key)
        .map_or(0, |(_, index)| index)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut decisions = 0;
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                decisions += 1;
                game.choose(canonical(game, &menu)).unwrap();
            }
            status => return (status, decisions),
        }
    }
    panic!("Railroad return phase did not finish");
}

fn oracle<const F: bool>(position: Position, card: CardId) -> (Game, Vec<Event>, usize) {
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, card, Context::new(A));
    start(&mut manual, card, Context::new(A));
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    let (status, decisions) = finish(&mut reduced, &mut events);
    let (manual_status, manual_decisions) = finish(&mut manual, &mut manual_events);
    assert_eq!(status, manual_status);
    assert_eq!(events, manual_events);
    manual.shortcuts_disabled = false;
    assert_eq!(reduced, manual); // Includes RNG and physical deck order.
    assert!(decisions <= manual_decisions);
    assert!(reduced.position.check_conservation());
    (reduced, events, decisions)
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Status)> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => {
            assert!(!menu.options.contains(&Alternative::Decline));
            (0..menu.options.len())
                .flat_map(|choice| {
                    let mut fork = game.clone();
                    fork.choose(choice).unwrap();
                    outcomes(fork, depth + 1)
                })
                .collect()
        }
        status => vec![(game.position.clone(), status)],
    }
}

#[test]
fn all_distinguishable_outcomes_survive_but_only_cross_deck_interleavings_disappear() {
    for (ages, manual_paths, reduced_paths) in [
        (vec![], 1, 1),
        (vec![1], 1, 1),
        (vec![1, 2, 4], 6, 1),
        (vec![1, 1, 2], 6, 2),
        (vec![1, 1, 1], 6, 6),
        (vec![1, 2, 1, 2], 24, 4),
        (vec![1, 2, 3, 4], 24, 1),
    ] {
        let (f, card, _) = fixture(&ages);
        let position = f.position();
        let mut reduced = Game::new(position.clone(), Expansions::BASE).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, card, Context::new(A));
        start(&mut manual, card, Context::new(A));
        let reduced = outcomes(reduced, 0);
        let manual = outcomes(manual, 0);
        assert_eq!(manual.len(), manual_paths, "{ages:?}");
        assert_eq!(reduced.len(), reduced_paths, "{ages:?}");
        assert!(
            manual.iter().all(|outcome| reduced.contains(outcome)),
            "{ages:?}"
        );
        assert!(
            reduced.iter().all(|outcome| manual.contains(outcome)),
            "{ages:?}"
        );
        oracle::<false>(position, card);
    }
}

#[test]
fn whole_hand_has_no_quota_and_base_and_figure_origin_decks_are_separate() {
    let (mut f, card, hand) = fixture(&[1, 1, 2, 3, 4]);
    let figure = f.figure(Figure::Gilgamesh, Zone::Hand(A));
    let (game, events, decisions) = oracle::<true>(f.position(), card);
    assert_eq!(decisions, 1); // Only the two same-deck Base cards need ordering.
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [hand[0], hand[1], figure, hand[2], hand[3], hand[4]]
    );
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(
        game.position.cards(Zone::Deck(Deck::base(1))),
        &[hand[1], hand[0]]
    );
    assert_eq!(game.position.cards(Zone::Deck(Deck::figures(1))), &[figure]);
}

#[test]
fn relevant_return_hazards_block_but_unrelated_later_draw_and_meld_hooks_do_not() {
    for (figure, color, blocked) in [
        (Figure::Homer, Color::Purple, true),
        (Figure::JamesClerkMaxwell, Color::Blue, true),
        (Figure::Nightingale, Color::Yellow, true),
        (Figure::AlexTrebek, Color::Yellow, false),
        (Figure::GalileoGalilei, Color::Green, false),
        (Figure::JohnVonNeumann, Color::Red, false),
        (Figure::Kalashnikov, Color::Red, false),
    ] {
        let (mut f, card, _) = fixture(&[1, 2, 4]);
        f.figure(figure, Zone::Board(A, color));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, card, Context::new(A));
        let mut events = Vec::new();
        if blocked {
            assert_eq!(
                decision(&mut game, &mut events).options.len(),
                3,
                "{figure:?}"
            );
            assert!(events.is_empty());
        } else {
            assert_eq!(finish(&mut game, &mut events).1, 0, "{figure:?}");
            assert_eq!(events.len(), 3);
        }
    }
}

#[test]
fn meiji_logical_hand_is_mandatory_and_timing_guard_is_live() {
    let (mut f, card, hand) = fixture(&[2, 4]);
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let borrowed = f.tech(Color::Yellow, 1, Zone::Hand(B), vec![]);
    let position = f.position();
    let (game, events, _) = oracle::<true>(position.clone(), card);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [borrowed, hand[0], hand[1]]
    );
    assert_eq!(moved(&events[0]).source, Zone::Hand(B));
    assert!(game.position.cards(Zone::Hand(B)).is_empty());
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.position.regular_actions_started = 2;
    start(&mut game, card, Context::new(A));
    assert_eq!(finish(&mut game, &mut Vec::new()).1, 0);
    assert_eq!(game.position.cards(Zone::Hand(B)), &[borrowed]);
}

#[test]
fn batch_resumption_rechecks_newly_exposed_homer_and_new_hand_membership() {
    for meiji in [false, true] {
        let (mut f, card, hand) = fixture(&[1, 1, 2, 4]);
        let figure = if meiji {
            Figure::EmperorMeiji
        } else {
            Figure::Homer
        };
        f.figure(figure, Zone::Board(A, Color::Purple));
        let cover = f.tech(Color::Purple, 8, Zone::Board(A, Color::Purple), vec![]);
        let borrowed = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, card, Context::new(A));
        let mut events = Vec::new();
        assert_eq!(decision(&mut game, &mut events).options.len(), 2);
        choose_card(&mut game, &mut events, hand[0]);
        // Expose a live hazard between batch resumptions; leave the captured
        // declaration untouched. This is a direct guard-boundary probe.
        assert_eq!(
            game.position.zone_mut(Zone::Board(A, Color::Purple)).pop(),
            Some(cover)
        );
        game.position.zone_mut(Zone::Junk).push(cover);
        game.position.refresh_boards();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), if meiji { 4 } else { 3 });
        assert!(!menu.options.contains(&Alternative::Decline));
        if meiji {
            assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == borrowed)));
        }
    }
}

#[test]
fn foreign_controller_cannot_distinguish_hidden_front_permutations() {
    let make = |reverse| {
        let (mut f, card, hand) = fixture(&[1, 1, 2]);
        if reverse {
            f.placements
                .swap(usize::from(hand[0].0), usize::from(hand[1].0));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        start(&mut game, card, context);
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let menu = decision(&mut first, &mut Vec::new());
    assert_eq!(menu, decision(&mut second, &mut Vec::new()));
    assert_eq!(menu.player, B);
    assert!(menu.options.iter().all(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Return,
            source: Zone::Hand(A),
            ..
        }
    )));
}

#[test]
fn canonical_events_rng_and_clones_match_at_every_return_boundary() {
    let (f, card, _) = fixture(&[1, 1, 2, 3]);
    let mut game = Game::seeded(f.position(), Expansions::BASE, 42).unwrap();
    start(&mut game, card, Context::new(A));
    let mut expected = game.clone();
    let mut all = Vec::new();
    let status = finish(&mut expected, &mut all).0;
    let mut prefix = Vec::new();
    for _ in 0..200 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix).0, status);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => game.choose(canonical(&game, &menu)).unwrap(),
            result => {
                assert_eq!(result, status);
                assert_eq!(game, expected);
                return;
            }
        }
    }
    panic!("Railroad did not finish");
}

#[test]
fn splay_program_retains_karma_suppression() {
    for suppressed in [false, true] {
        let mut f = Fixture::default();
        let card = f.add(base_card("B025").unwrap(), Zone::Board(A, Color::Purple));
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.set_suppress_karma(suppressed);
        start(&mut game, card, context);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        let index = menu
            .options
            .iter()
            .position(|o| {
                *o == Alternative::Splay {
                    player: A,
                    color: Color::Red,
                    direction: Splay::Left,
                }
            })
            .unwrap();
        game.choose(index).unwrap();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            game.position.splay(A, Color::Red),
            if suppressed {
                Splay::Left
            } else {
                Splay::Right
            }
        );
    }
}

#[test]
fn homer_replacement_does_not_stop_the_mandatory_return_phase() {
    let (mut f, card, hand) = fixture(&[2, 4]);
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let returned_figure = f.figure(Figure::Archimedes, Zone::Hand(A));
    let (game, events, _) = oracle::<true>(f.position(), card);
    assert_eq!(events.len(), 3);
    assert_eq!(
        events
            .iter()
            .filter(|e| moved(e).verb == Some(Verb::Return))
            .count(),
        hand.len()
    );
    assert!(events.iter().any(|e| moved(e).card == returned_figure
        && moved(e).verb == Some(Verb::Tuck)
        && moved(e).inside_karma));
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
}

#[test]
fn printed_branch_rechecks_the_live_board_at_its_program_boundary() {
    for (code, color, direction) in [
        ("B061", Color::Green, Splay::Right),
        ("B076", Color::Red, Splay::Up),
    ] {
        for initially_splayed in [false, true] {
            let mut f = Fixture::default();
            f.tech(color, 1, Zone::Board(A, color), vec![]);
            let card = f.add(base_card(code).unwrap(), Zone::Board(A, color));
            f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
            f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
            let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
            if initially_splayed {
                game.start(
                    Operation::Splay {
                        player: A,
                        color,
                        direction,
                    },
                    Context::new(A),
                )
                .unwrap();
                settle(&mut game, &mut Vec::new());
            }
            start(&mut game, card, Context::new(A));
            let mut events = Vec::new();
            for _ in 0..100 {
                if matches!(game.frames.last(), Some(Frame::Program { pc: 0, .. })) {
                    break;
                }
                assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
            }
            assert!(matches!(
                game.frames.last(),
                Some(Frame::Program { pc: 0, .. })
            ));
            // Insert an ordinary operation at the suspended instruction
            // boundary, as an intervening reaction would do.
            game.frames.push(Frame::operation(
                Operation::Splay {
                    player: A,
                    color,
                    direction: if initially_splayed {
                        Splay::None
                    } else {
                        direction
                    },
                },
                Context::new(A),
            ));
            if initially_splayed {
                assert_eq!(
                    settle(&mut game, &mut events),
                    Status::Complete(ReturnValue::Unit)
                );
            } else {
                assert_eq!(
                    decision(&mut game, &mut events).options,
                    [
                        Alternative::Splay {
                            player: A,
                            color: Color::Blue,
                            direction
                        },
                        Alternative::Decline,
                    ]
                );
            }
        }
    }
}

#[test]
fn full_railroad_completes_grouped_returns_before_trebeks_real_draw_reaction() {
    let (mut f, card, hand) = fixture(&[1, 2, 4]);
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(
        menu.options
            .iter()
            .all(|option| matches!(option, Alternative::Deck(_)))
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        hand
    );
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
}
