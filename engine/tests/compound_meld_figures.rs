use inno_engine::*;

mod common;
use common::*;

const FIGURES: [Figure; 3] = [Figure::Sneferu, Figure::SuSong, Figure::SejongTheGreat];

fn native(f: &mut Fixture, figure: Figure, owner: Player) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}

fn performed(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: false,
    }))
}

fn empty(disposition: Disposition) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition,
        card: None,
        value: None,
        short: false,
    }))
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            result => return result,
        }
    }
    panic!("compound meld did not finish");
}

#[test]
fn first_returns_only_the_exact_child_and_resumes_the_original_for_either_executor() {
    for (figure, age, icon, verb) in [
        (Figure::Sneferu, 2, Icon::Authority, Verb::Tuck),
        (Figure::SuSong, 3, Icon::Health, Verb::Meld),
    ] {
        for owner in [A, B] {
            for keeps_child in [false, true] {
                let mut f = Fixture::default();
                native(&mut f, figure, owner);
                let old = f.tech(Color::Purple, 1, Zone::Board(owner, Color::Purple), vec![]);
                // Empty printed-age deck forces routing without changing the fixed box.
                let child = f.tech(
                    Color::Purple,
                    age + 1,
                    Zone::Deck(Deck::base(age + 1)),
                    vec![],
                );
                if keeps_child {
                    f.cards[usize::from(child.0)].symbols[3] = Symbol::Icon(icon);
                }
                let selected = f.tech(Color::Blue, 7, Zone::Hand(owner.opponent()), vec![]);
                let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
                let mut context = Context::new(owner);
                context.controller = owner.opponent();
                context.demander = owner.opponent();
                context.source_owner = owner.opponent();
                game.start(
                    meld(selected, Zone::Hand(owner.opponent()), owner, Color::Blue),
                    context,
                )
                .unwrap();
                let mut events = Vec::new();
                assert_eq!(settle(&mut game, &mut events), performed(selected, 7));
                assert_eq!(events.len(), if keeps_child { 3 } else { 4 });
                assert_eq!(moved(&events[0]).card, child);
                assert_eq!(moved(&events[0]).source, Zone::Deck(Deck::base(age + 1)));
                assert_eq!(moved(&events[1]).card, child);
                assert_eq!(moved(&events[1]).verb, Some(verb));
                if !keeps_child {
                    assert_eq!(moved(&events[2]).card, child);
                    assert_eq!(moved(&events[2]).source, Zone::Board(owner, Color::Purple));
                    assert_eq!(
                        moved(&events[2]).destination,
                        Zone::Deck(Deck::base(age + 1))
                    );
                    assert_eq!(
                        game.position().cards(Zone::Board(owner, Color::Purple)),
                        &[old]
                    );
                } else if figure == Figure::Sneferu {
                    assert_eq!(
                        game.position().cards(Zone::Board(owner, Color::Purple)),
                        &[child, old]
                    );
                } else {
                    assert_eq!(
                        game.position().cards(Zone::Board(owner, Color::Purple)),
                        &[old, child]
                    );
                }
                for event in &events[..events.len() - 1] {
                    assert!(moved(event).inside_karma);
                    assert_eq!(moved(event).executor, owner);
                }
                let original = moved(events.last().unwrap());
                assert_eq!(original.card, selected);
                assert_eq!(original.source, Zone::Hand(owner.opponent()));
                assert_eq!(original.executor, owner);
                assert!(!original.inside_karma);
                assert!(game.position().check_conservation());
            }
        }
    }
}

#[test]
fn printed_predicates_ignore_board_icons_and_each_additions() {
    for (figure, age, icon) in [
        (Figure::Sneferu, 2, Icon::Authority),
        (Figure::SuSong, 3, Icon::Health),
    ] {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        let other = f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
        f.cards[usize::from(other.0)].symbols = four_slots([Symbol::Icon(icon); 4]);
        native(&mut f, Figure::JamesClerkMaxwell, A);
        let selected = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let child = f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert!(game.position().icon_count(A, icon) > game.position().basic_icon_count(A, icon));
        game.start(
            meld(selected, Zone::Hand(A), A, Color::Red),
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), performed(selected, 1));
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(age))), &[child]);
        // Returning the child does not invoke Maxwell's draw-eight First.
        assert_eq!(events.len(), 4);
        assert_eq!(moved(&events[2]).verb, Some(Verb::Return));
    }
}

#[test]
fn su_song_can_cover_its_source_and_original_first_meld_retains_the_now_buried_source() {
    for returns_child in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, Figure::SuSong, A);
        let child = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        if !returns_child {
            f.cards[usize::from(child.0)].symbols[0] = Symbol::Icon(Icon::Health);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(source, Zone::Board(A, Color::Green), A, Color::Green),
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        advance_to_events(&mut game, &mut events, 2);
        assert!(!game.position().has_figure(A, Figure::SuSong));
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(settle(&mut fork, &mut suffix), performed(source, 3));
        assert_eq!(settle(&mut game, &mut events), performed(source, 3));
        assert_eq!(&events[2..], suffix);
        assert_eq!(game, fork);
        assert_eq!(game.position().top(A, Color::Green), Some(source));
        assert_eq!(
            moved(events.last().unwrap()).source,
            Zone::Board(A, Color::Green)
        );
        assert!(!moved(events.last().unwrap()).inside_karma);
        assert!(game.position().check_conservation());
    }
}

#[test]
fn covered_compound_sources_stop_triggering_on_the_next_independent_meld() {
    for figure in [Figure::Sneferu, Figure::SuSong] {
        let mut f = Fixture::default();
        let source = native(&mut f, figure, A);
        let color = f.cards[usize::from(source.0)].color;
        let age = if figure == Figure::Sneferu { 2 } else { 3 };
        let cover = f.tech(color, 5, Zone::Hand(A), vec![]);
        let next = f.tech(Color::Purple, 6, Zone::Hand(A), vec![]);
        let child = f.tech(Color::Red, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(meld(cover, Zone::Hand(A), A, color), Context::new(A))
            .unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), performed(cover, 5));
        assert!(!game.position().has_figure(A, figure));
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(age))), &[child]);
        events.clear();
        game.start(meld(next, Zone::Hand(A), A, Color::Purple), Context::new(A))
            .unwrap();
        assert_eq!(settle(&mut game, &mut events), performed(next, 6));
        assert_eq!(events.len(), 1);
    }
}

#[test]
fn sejong_matches_only_own_top_red_and_missing_red_means_zero() {
    for owner in [A, B] {
        for (own_red, selected_value, replaces) in [
            (None, 0, true),
            (None, 1, false),
            (Some(0), 0, true),
            (Some(2), 2, true),
            (Some(2), 3, false),
        ] {
            let mut f = Fixture::default();
            native(&mut f, Figure::SejongTheGreat, owner);
            // Neither a buried equal value nor the opponent's red satisfies the condition.
            if let Some(value) = own_red {
                f.tech(
                    Color::Red,
                    selected_value,
                    Zone::Board(owner, Color::Red),
                    vec![],
                );
                f.tech(Color::Red, value, Zone::Board(owner, Color::Red), vec![]);
            }
            f.tech(
                Color::Red,
                selected_value,
                Zone::Board(owner.opponent(), Color::Red),
                vec![],
            );
            let selected = f.tech(
                Color::Green,
                selected_value,
                Zone::Hand(owner.opponent()),
                vec![],
            );
            let child = f.tech(
                Color::Purple,
                selected_value + 1,
                Zone::Deck(Deck::base(selected_value + 1)),
                vec![],
            );
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let mut context = Context::new(owner);
            context.controller = owner.opponent();
            context.demander = owner.opponent();
            context.source_owner = owner.opponent();
            game.start(
                meld(selected, Zone::Hand(owner.opponent()), owner, Color::Green),
                context,
            )
            .unwrap();
            let mut events = Vec::new();
            assert_eq!(
                settle(&mut game, &mut events),
                if replaces {
                    empty(Disposition::Replaced)
                } else {
                    performed(selected, selected_value)
                }
            );
            if replaces {
                assert_eq!(
                    events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
                    [selected, child, child]
                );
                assert_eq!(moved(&events[0]).source, Zone::Hand(owner.opponent()));
                assert_eq!(
                    moved(&events[0]).destination,
                    Zone::Deck(Deck::base(selected_value))
                );
                assert!(
                    events
                        .iter()
                        .all(|e| moved(e).inside_karma && moved(e).executor == owner)
                );
                assert_eq!(game.position().top(owner, Color::Green), None);
                assert_eq!(game.position().top(owner, Color::Purple), Some(child));
            } else {
                assert_eq!(events.len(), 1);
                assert_eq!(
                    game.position()
                        .cards(Zone::Deck(Deck::base(selected_value + 1))),
                    &[child]
                );
            }
            assert!(game.position().check_conservation());
        }
    }
}

#[test]
fn sejong_returns_native_figures_to_their_origin_and_keeps_going_after_returning_itself() {
    for selected_is_source in [false, true] {
        let mut f = Fixture::default();
        let source = native(&mut f, Figure::SejongTheGreat, A);
        let selected = if selected_is_source {
            source
        } else {
            f.figure(Figure::SuSong, Zone::Hand(B))
        };
        let physical = if selected_is_source {
            Zone::Board(A, Color::Blue)
        } else {
            Zone::Hand(B)
        };
        let color = f.cards[usize::from(selected.0)].color;
        f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
        let child = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(meld(selected, physical, A, color), Context::new(A))
            .unwrap();
        let mut events = Vec::new();
        advance_to_events(&mut game, &mut events, 1);
        assert_eq!(moved(&events[0]).destination, Zone::Deck(Deck::figures(3)));
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(settle(&mut game, &mut events), empty(Disposition::Replaced));
        assert_eq!(settle(&mut fork, &mut suffix), empty(Disposition::Replaced));
        assert_eq!(&events[1..], suffix);
        assert_eq!(game, fork);
        assert_eq!(game.position().top(A, Color::Blue), Some(child));
        assert!(!game.position().has_figure(A, Figure::SejongTheGreat));
    }
}

#[test]
fn effective_borrowed_hand_selection_survives_the_child_covering_meiji() {
    for figure in [Figure::SuSong, Figure::SejongTheGreat] {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        native(&mut f, Figure::EmperorMeiji, A);
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        let selected = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
        let child = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
        f.cards[usize::from(child.0)].symbols[0] = Symbol::Icon(Icon::Health);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let borrowed = game
            .position()
            .targets(LogicalZone::Hand(A))
            .find(|t| t.card == selected)
            .unwrap();
        assert_eq!(borrowed.physical, Zone::Hand(B));
        game.start(
            Operation::Move {
                verb: Verb::Meld,
                target: borrowed,
                destination: Zone::Board(A, Color::Purple),
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let expected = if figure == Figure::SejongTheGreat {
            empty(Disposition::Replaced)
        } else {
            performed(selected, 2)
        };
        assert_eq!(settle(&mut game, &mut events), expected);
        let selected_move = events
            .iter()
            .map(moved)
            .find(|m| m.card == selected)
            .unwrap();
        assert_eq!(selected_move.source, Zone::Hand(B));
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert!(events.iter().any(|e| moved(e).card == child));
        assert!(!game.position().has_figure(A, Figure::EmperorMeiji));
    }
}

#[test]
fn competing_hooks_are_chosen_by_turn_owner_without_chaining_or_leaking_child_karma() {
    for figure in FIGURES {
        for choose_compound in [false, true] {
            let mut f = Fixture::default();
            let source = native(&mut f, figure, B);
            let kalashnikov = native(&mut f, Figure::Kalashnikov, A);
            let red = f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
            let selected = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
            let age = if figure == Figure::Sneferu { 2 } else { 3 };
            let child = f.tech(Color::Green, age, Zone::Deck(Deck::base(age)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let mut context = Context::new(B);
            context.controller = B;
            game.start(meld(selected, Zone::Hand(B), B, Color::Purple), context)
                .unwrap();
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, A);
            assert_eq!(menu.options.len(), 2);
            assert!(
                menu.options
                    .contains(&Alternative::Karma { source, figure })
            );
            assert!(menu.options.contains(&Alternative::Karma {
                source: kalashnikov,
                figure: Figure::Kalashnikov
            }));
            assert!(!menu.options.contains(&Alternative::Decline));
            let chosen = if choose_compound { source } else { kalashnikov };
            let index = menu
                .options
                .iter()
                .position(|o| matches!(o, Alternative::Karma { source, .. } if *source == chosen))
                .unwrap();
            let before = game.clone();
            assert!(game.choose(menu.options.len()).is_err());
            assert_eq!(game, before);
            game.choose(index).unwrap();
            let mut fork = game.clone();
            let mut suffix = Vec::new();
            let result = finish(&mut game, &mut events);
            assert_eq!(finish(&mut fork, &mut suffix), result);
            assert_eq!(events, suffix);
            assert_eq!(game, fork);
            assert_eq!(
                result,
                if choose_compound && figure == Figure::SejongTheGreat {
                    empty(Disposition::Replaced)
                } else {
                    performed(selected, 2)
                }
            );
            if choose_compound {
                assert_eq!(game.position().top(B, Color::Red), Some(red));
                assert!(game.position().cards(Zone::Score(A)).is_empty());
                assert!(events.iter().any(|e| moved(e).card == child));
            } else {
                assert_eq!(game.position().cards(Zone::Score(A)), &[red]);
                assert_eq!(game.position().cards(Zone::Deck(Deck::base(age))), &[child]);
            }
        }
    }
}

#[test]
fn fresh_original_when_runs_after_first_but_never_after_sejong_instead() {
    for figure in FIGURES {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
        let victim = native(&mut f, Figure::Michelangelo, B);
        let age = if figure == Figure::Sneferu { 2 } else { 3 };
        let child = f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(alexander, Zone::Hand(A), A, Color::Red),
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            if figure == Figure::SejongTheGreat {
                empty(Disposition::Replaced)
            } else {
                performed(alexander, 2)
            }
        );
        if figure == Figure::SejongTheGreat {
            assert_eq!(game.position().top(B, Color::Yellow), Some(victim));
            assert_eq!(game.position().top(A, Color::Purple), Some(child));
            assert_eq!(
                game.position().cards(Zone::Deck(Deck::figures(2))),
                &[alexander]
            );
        } else {
            assert_eq!(game.position().cards(Zone::Score(A)), &[victim]);
            assert_eq!(moved(&events[3]).card, alexander);
            assert!(!moved(&events[3]).inside_karma);
            assert_eq!(moved(&events[4]).card, victim);
            assert!(moved(&events[4]).inside_karma);
        }
    }
}

#[test]
fn child_results_never_replace_original_program_result_or_if_you_do_disposition() {
    for figure in FIGURES {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
        let program = f.tech(
            Color::Purple,
            1,
            Zone::Board(A, Color::Purple),
            vec![
                build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
                build::if_(build::DID, 1),
                build::draw_age(Expr::Add(&build::LAST_VALUE, 1)),
            ],
        );
        let selected = f.tech(Color::Green, 4, Zone::Hand(A), vec![]);
        let age = if figure == Figure::Sneferu {
            2
        } else if figure == Figure::SuSong {
            3
        } else {
            5
        };
        let child = f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
        let next = if age != 5 {
            Some(f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]))
        } else {
            None
        };
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(program, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if let Some(next) = next {
            assert_eq!(game.position().cards(Zone::Hand(A)), &[next]);
            assert_eq!(moved(&events[3]).card, selected);
            assert_eq!(moved(&events[4]).card, next);
        } else {
            assert!(game.position().cards(Zone::Hand(A)).is_empty());
            assert_eq!(events.len(), 3);
            assert_eq!(game.position().top(A, Color::Purple), Some(child));
        }
    }
}

#[test]
fn archimedes_changes_original_boxes_but_not_fixed_karma_boxes() {
    for (figure, age) in [(Figure::Sneferu, 2), (Figure::SuSong, 3)] {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        native(&mut f, Figure::Archimedes, A);
        let program = f.tech(
            Color::Purple,
            1,
            Zone::Board(B, Color::Purple),
            vec![
                build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
                build::draw(6),
            ],
        );
        let selected = f.tech(Color::Red, 5, Zone::Hand(A), vec![]);
        let child = f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
        let not_child = f.tech(
            Color::Purple,
            age + 1,
            Zone::Deck(Deck::base(age + 1)),
            vec![],
        );
        let reward = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.source_owner = B;
        context.demander = B;
        game.self_execute(program, context).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(
            events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
            [child, child, child, selected, reward]
        );
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(age + 1))),
            &[not_child]
        );
    }
}

#[test]
fn sejong_computed_increment_ignores_an_inherited_box_adjustment() {
    let mut f = Fixture::default();
    native(&mut f, Figure::SejongTheGreat, A);
    let archimedes = native(&mut f, Figure::Archimedes, A);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let program = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(Verb::Score, Selector::TopOf(Who::Executor, Color::Blue))
                .to(Destination::Score(Who::Executor)),
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            build::draw(6),
        ],
    );
    let selected = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let child = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let untouched = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let reward = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(program, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [archimedes, selected, child, child, reward]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(4))),
        &[untouched]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
}

#[test]
fn invalid_missing_buried_and_stale_public_targets_cannot_earn_compound_children() {
    for figure in FIGURES {
        for mode in 0..4 {
            let mut f = Fixture::default();
            native(&mut f, figure, A);
            f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
            let physical = if mode == 2 {
                Zone::Board(A, Color::Purple)
            } else {
                Zone::Hand(A)
            };
            let selected = f.tech(Color::Purple, 2, physical, vec![]);
            if mode == 2 {
                f.tech(Color::Purple, 3, physical, vec![]);
            }
            let child = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            if mode == 3 {
                game.start(
                    Operation::Move {
                        verb: Verb::Return,
                        target: target(selected, physical),
                        destination: Zone::Deck(Deck::base(2)),
                    },
                    Context::new(A),
                )
                .unwrap();
                assert_eq!(settle(&mut game, &mut Vec::new()), performed(selected, 2));
            }
            let mut invalid = target(selected, physical);
            if mode == 0 {
                invalid.card = CardId(u16::MAX);
            }
            if mode == 1 {
                invalid.logical = LogicalZone::Hand(B);
            }
            game.start(
                Operation::Move {
                    verb: Verb::Meld,
                    target: invalid,
                    destination: Zone::Board(A, Color::Purple),
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            assert_eq!(
                settle(&mut game, &mut events),
                empty(Disposition::Inapplicable)
            );
            assert!(events.is_empty());
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), &[child]);
            assert!(game.position().check_conservation());
        }
    }
}

#[test]
fn opponent_or_covered_sources_do_not_trigger_and_tucking_is_not_melding() {
    for figure in FIGURES {
        for mode in 0..3 {
            let mut f = Fixture::default();
            let owner = if mode == 0 { B } else { A };
            let source = native(&mut f, figure, owner);
            if mode == 1 {
                let color = f.cards[usize::from(source.0)].color;
                f.tech(color, 8, Zone::Board(A, color), vec![]);
            }
            f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
            let selected = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
            let child = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.start(
                Operation::Move {
                    verb: if mode == 2 { Verb::Tuck } else { Verb::Meld },
                    target: target(selected, Zone::Hand(A)),
                    destination: Zone::Board(A, Color::Purple),
                },
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            assert_eq!(settle(&mut game, &mut events), performed(selected, 2));
            assert_eq!(events.len(), 1);
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), &[child]);
        }
    }
}

#[test]
fn compound_terminal_draw_unwinds_without_resuming_the_interrupted_meld() {
    for figure in FIGURES {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Green, 9, Zone::Score(A), vec![]);
        let selected = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(selected, Zone::Hand(A), A, Color::Purple),
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
        assert_eq!(events.len(), usize::from(figure == Figure::SejongTheGreat));
        assert_eq!(game.position().top(A, Color::Purple), None);
        if figure == Figure::SejongTheGreat {
            assert_eq!(
                game.position().cards(Zone::Deck(Deck::base(2))),
                &[selected]
            );
        } else {
            assert_eq!(game.position().cards(Zone::Hand(A)), &[selected]);
        }
        let terminal = game.clone();
        assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
        assert_eq!(game, terminal);
        assert!(
            game.start(
                meld(selected, Zone::Hand(A), A, Color::Purple),
                Context::new(A)
            )
            .is_err()
        );
        assert_eq!(game, terminal);
        assert!(game.position().check_conservation());
    }
}

#[test]
fn first_clone_resume_preserves_every_child_boundary_and_original_result() {
    for (figure, age) in [(Figure::Sneferu, 2), (Figure::SuSong, 3)] {
        for checkpoint in 1..=3 {
            let mut f = Fixture::default();
            native(&mut f, figure, A);
            f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
            let selected = f.tech(Color::Red, 8, Zone::Hand(A), vec![]);
            f.tech(Color::Purple, age, Zone::Deck(Deck::base(age)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.start(
                meld(selected, Zone::Hand(A), A, Color::Red),
                Context::new(A),
            )
            .unwrap();
            let mut events = Vec::new();
            advance_to_events(&mut game, &mut events, checkpoint);
            let mut fork = game.clone();
            let mut suffix = Vec::new();
            assert_eq!(settle(&mut game, &mut events), performed(selected, 8));
            assert_eq!(settle(&mut fork, &mut suffix), performed(selected, 8));
            assert_eq!(&events[checkpoint..], suffix);
            assert_eq!(game, fork);
        }
    }
}

#[test]
fn any_two_permissions_admit_full_payment_and_native_decree_effects() {
    for (figure, decree, draw_age, draws) in [
        (Figure::Sneferu, Decree::Expansion, 0, 0),
        (Figure::SuSong, Decree::Trade, 4, 3),
        (Figure::SejongTheGreat, Decree::Advancement, 5, 1),
    ] {
        let mut f = Fixture::default();
        if figure == Figure::Sneferu {
            f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
            for _ in 0..2 {
                f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
            }
        }
        native(&mut f, figure, A);
        let first = f.figure(Figure::EmmaWatson, Zone::Hand(A));
        let second = f.figure(Figure::BearGrylls, Zone::Hand(A));
        let nonfigure = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let rewards: Vec<_> = (0..draws)
            .map(|_| {
                f.tech(
                    Color::Purple,
                    draw_age,
                    Zone::Deck(Deck::base(draw_age)),
                    vec![],
                )
            })
            .collect();
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_opening_turn(A).unwrap();
        game.take_action(Action::Decree, ActionKind::Regular)
            .unwrap();
        let mut events = Vec::new();
        // The whole hand goes to the unordered junk without an order choice.
        if decree == Decree::Expansion {
            let splays = decision(&mut game, &mut events);
            assert_eq!(events.len(), 3);
            assert_eq!(splays.options.len(), 2);
            let choice = splays
                .options
                .iter()
                .position(|option| {
                    *option
                        == Alternative::Splay {
                            player: A,
                            color: Color::Red,
                            direction: Splay::Up,
                        }
                })
                .unwrap();
            let mut fork = game.clone();
            game.choose(choice).unwrap();
            fork.choose(choice).unwrap();
            let mut suffix = Vec::new();
            assert_eq!(
                settle(&mut fork, &mut suffix),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(&events[3..], suffix);
            assert_eq!(game, fork);
            assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
            assert_eq!(game.position().splay(A, Color::Yellow), Splay::None);
        } else {
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                game.position().cards(Zone::Hand(A)),
                rewards.into_iter().rev().collect::<Vec<_>>()
            );
        }
        assert_eq!(
            game.position().cards(Zone::Junk),
            &[first, second, nonfigure]
        );
        for event in &events[..3] {
            assert_eq!(moved(event).verb, Some(Verb::Junk));
            assert!(!moved(event).inside_karma);
        }
        assert_eq!(
            game.position().decree_location(decree),
            DecreeLocation::Owned(A)
        );
        assert_eq!(game.position().turn_owner(), B);
        assert_eq!(game.position().regular_actions_started(), 0);
        assert!(game.position().check_conservation());
    }
}

#[test]
fn decree_permission_requires_own_exposed_source_and_two_figures_not_two_cards() {
    for figure in FIGURES {
        for mode in 0..3 {
            let mut f = Fixture::default();
            let source = native(&mut f, figure, if mode == 0 { B } else { A });
            if mode == 1 {
                let color = f.cards[usize::from(source.0)].color;
                f.tech(color, 5, Zone::Board(A, color), vec![]);
            }
            f.figure(Figure::EmmaWatson, Zone::Hand(A));
            if mode != 2 {
                f.figure(Figure::BearGrylls, Zone::Hand(A));
            }
            f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            let before = game.clone();
            for kind in [ActionKind::Regular, ActionKind::Free] {
                assert_eq!(
                    game.take_action(Action::Decree, kind),
                    Err(Error::InvalidAction)
                );
                assert_eq!(game, before);
            }
        }
    }
}

#[test]
fn interrupted_optional_meld_keeps_both_targets_and_decline_with_the_controller() {
    for figure in FIGURES {
        for selected_option in 0..3 {
            let mut f = Fixture::default();
            native(&mut f, figure, B);
            f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
            let program = f.tech(
                Color::Purple,
                1,
                Zone::Board(A, Color::Purple),
                vec![
                    build::verb(Verb::Meld, build::HAND)
                        .to(Destination::Board(Who::Executor))
                        .optional(),
                ],
            );
            let first = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
            let second = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
            let child = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
            let mut position = f.position();
            position.grant_hand_view(A, B);
            let mut game = Game::new(position, Expansions::FIGURES).unwrap();
            let mut context = Context::new(B);
            context.controller = A;
            context.demander = A;
            context.source_owner = A;
            game.self_execute(program, context).unwrap();
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, A);
            assert_eq!(menu.options.len(), 3);
            assert!(menu.options.contains(&Alternative::Decline));
            for selected in [first, second] {
                assert!(menu.options.iter().any(|option| matches!(
                    option,
                    Alternative::Move {
                        target: TargetDescriptor::Known { card, .. },
                        ..
                    } if *card == selected
                )));
            }
            let selected = [Some(first), Some(second), None][selected_option];
            let index = menu
                .options
                .iter()
                .position(|option| match (selected, option) {
                    (None, Alternative::Decline) => true,
                    (
                        Some(selected),
                        Alternative::Move {
                            target: TargetDescriptor::Known { card, .. },
                            ..
                        },
                    ) => selected == *card,
                    _ => false,
                })
                .unwrap();
            let mut fork = game.clone();
            game.choose(index).unwrap();
            fork.choose(index).unwrap();
            let mut suffix = Vec::new();
            assert_eq!(
                settle(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                settle(&mut fork, &mut suffix),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(game, fork);
            assert_eq!(events, suffix);
            if let Some(selected) = selected {
                assert!(events.iter().any(|event| moved(event).card == selected));
                assert!(events.iter().any(|event| moved(event).card == child));
                assert!(events.iter().all(|event| moved(event).executor == B));
            } else {
                assert!(events.is_empty());
                assert_eq!(game.position().cards(Zone::Hand(B)), &[first, second]);
            }
        }
    }
}

#[test]
fn a_meld_inside_another_karma_suppresses_compound_hooks_and_its_child_when() {
    for figure in FIGURES {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        let kalashnikov = native(&mut f, Figure::Kalashnikov, A);
        let program = f.tech(
            Color::Red,
            2,
            Zone::Board(B, Color::Red),
            vec![
                build::verb(Verb::Score, Selector::TopOf(Who::Executor, Color::Red))
                    .to(Destination::Score(Who::Executor)),
                build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            ],
        );
        let victim = native(&mut f, Figure::Michelangelo, B);
        let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
        let original = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
        // A child draw would have a real target; absence of child movements
        // therefore proves suppression rather than an exhausted deck.
        let child = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(original, Zone::Hand(B), B, Color::Purple),
            Context::new(B),
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        let execute = menu
            .options
            .iter()
            .position(|option| {
                matches!(
                    option,
                    Alternative::SelfExecute(card) if *card == program
                )
            })
            .unwrap();
        game.choose(execute).unwrap();
        assert_eq!(settle(&mut game, &mut events), performed(original, 2));
        assert_eq!(
            events
                .iter()
                .map(|event| moved(event).card)
                .collect::<Vec<_>>(),
            [kalashnikov, alexander, original]
        );
        assert!(moved(&events[0]).inside_karma);
        assert!(moved(&events[1]).inside_karma);
        assert!(!moved(&events[2]).inside_karma);
        assert_eq!(game.position().top(B, Color::Yellow), Some(victim));
        assert_eq!(game.position().cards(Zone::Score(A)), &[kalashnikov]);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), &[child]);
    }
}

#[test]
fn resumed_demand_keeps_demander_and_original_selected_identity() {
    for figure in FIGURES {
        let mut f = Fixture::default();
        native(&mut f, figure, B);
        f.tech(Color::Red, 2, Zone::Board(B, Color::Red), vec![]);
        let program = f.tech(
            Color::Purple,
            1,
            Zone::Board(A, Color::Purple),
            vec![
                build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
                build::verb(
                    Verb::Transfer,
                    build::filtered(
                        &Selector::TopOf(Who::Executor, Color::Purple),
                        &[Filter::Has(Test::Is(Reg::Last))],
                    ),
                )
                .to(Destination::Score(Who::Demander)),
            ],
        );
        f.cards[usize::from(program.0)].effects[0].kind = EffectKind::Demand;
        let selected = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
        f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(B);
        context.controller = A;
        context.demander = A;
        context.source_owner = A;
        game.start(
            Operation::ExecuteEffect {
                card: program,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if figure == Figure::SejongTheGreat {
            assert!(game.position().cards(Zone::Score(A)).is_empty());
            assert_eq!(events.len(), 3);
        } else {
            assert_eq!(game.position().cards(Zone::Score(A)), &[selected]);
            let transfer = moved(events.last().unwrap());
            assert_eq!(transfer.card, selected);
            assert_eq!(transfer.verb, Some(Verb::Transfer));
            assert_eq!(transfer.executor, B);
            assert!(!transfer.inside_karma);
        }
    }
}

#[test]
fn sejong_uses_selected_value_after_return_exposes_a_different_red_and_draw_reroutes() {
    let mut f = Fixture::default();
    native(&mut f, Figure::SejongTheGreat, A);
    let exposed = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let selected = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let lower = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let child = f.tech(Color::Blue, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(selected, Zone::Board(A, Color::Red), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 2);
    assert_eq!(game.position().top(A, Color::Red), Some(exposed));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[child]);
    let mut fork = game.clone();
    let mut suffix = Vec::new();
    assert_eq!(settle(&mut game, &mut events), empty(Disposition::Replaced));
    assert_eq!(settle(&mut fork, &mut suffix), empty(Disposition::Replaced));
    assert_eq!(&events[2..], suffix);
    assert_eq!(game, fork);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[selected, lower]
    );
    assert_eq!(game.position().top(A, Color::Blue), Some(child));
    assert!(events.iter().all(|event| moved(event).inside_karma));
}
