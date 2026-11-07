use inno_engine::*;

mod common;
use common::*;

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("Emma did not terminate");
}

#[test]
fn emma_requires_six_actual_returns_not_merely_all_available_returns() {
    for count in 0..=7 {
        let mut f = Fixture::default();
        f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
        let own_blue =
            [1, 2].map(|age| f.tech(Color::Blue, age, Zone::Board(A, Color::Blue), vec![]));
        let mut returned_stack = Vec::new();
        for _ in 0..count {
            returned_stack.push(f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]));
        }
        let selected = f.tech(Color::Blue, 3, Zone::Hand(B), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(selected, Zone::Hand(B), B, Color::Blue),
            Context::new(B),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Performed,
                card: Some(selected),
                value: Some(3),
                short: false,
            }))
        );
        let returns = count.min(6);
        assert_eq!(events.len(), returns + if count < 6 { 2 } else { 0 } + 1);
        for (event, &card) in events[..returns].iter().zip(returned_stack.iter().rev()) {
            assert_eq!(moved(event).card, card);
            assert_eq!(moved(event).verb, Some(Verb::Return));
            assert_eq!(moved(event).executor, A); // Emma owns the return instruction.
            assert!(moved(event).inside_karma);
        }
        if count < 6 {
            assert_eq!(
                game.position().cards(Zone::Score(B)),
                &[own_blue[1], own_blue[0]]
            );
            assert!(
                game.position()
                    .cards(Zone::Board(A, Color::Blue))
                    .is_empty()
            );
            assert!(
                events[returns..returns + 2]
                    .iter()
                    .all(|e| moved(e).executor == B && moved(e).inside_karma)
            );
        } else {
            assert_eq!(game.position().cards(Zone::Board(A, Color::Blue)), own_blue);
            assert!(game.position().cards(Zone::Score(B)).is_empty());
        }
        assert_eq!(
            game.position().cards(Zone::Board(B, Color::Blue)).len(),
            count.saturating_sub(6) + 1
        );
        assert_eq!(game.position().top(B, Color::Blue), Some(selected));
        assert!(!moved(events.last().unwrap()).inside_karma);
        assert!(game.position().check_conservation());
    }
}

#[test]
fn partial_fallback_scores_as_the_melder_and_suppresses_maxwell_and_shen_kuo() {
    let mut f = Fixture::default();
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    let below = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let maxwell = f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    f.figure(Figure::ShenKuo, Zone::Board(B, Color::Green));
    for age in 1..=5 {
        f.tech(Color::Blue, age, Zone::Board(B, Color::Blue), vec![]);
    }
    let selected = f.tech(Color::Blue, 4, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(selected, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 8); // Five returns, two scores, the original meld.
    assert_eq!(game.position().cards(Zone::Score(B)), &[maxwell, below]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert!(events[..7].iter().all(|e| moved(e).inside_karma));
    assert!(!moved(&events[7]).inside_karma);
    // No Base 8 was supplied: an unsuppressed Maxwell draw would have ended the game.
    assert_eq!(game.position().top(B, Color::Blue), Some(selected));
}

#[test]
fn emma_can_return_herself_and_finish_all_six_returns_after_losing_the_source() {
    let mut f = Fixture::default();
    for _ in 0..5 {
        f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    }
    let emma = f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    let opposing = f.tech(Color::Purple, 3, Zone::Board(B, Color::Purple), vec![]);
    let selected = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(selected, Zone::Hand(A), A, Color::Purple),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(11))),
        &[emma]
    );
    assert!(!game.position().has_figure(A, Figure::EmmaWatson));
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 7);
    assert_eq!(game.position().top(B, Color::Purple), Some(opposing));
    assert_eq!(game.position().top(A, Color::Purple), Some(selected));
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn fallback_may_score_the_selected_foreign_meld_card_but_the_exact_meld_still_resumes() {
    let mut f = Fixture::default();
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    let selected = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(selected, Zone::Board(A, Color::Blue), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(game.position().cards(Zone::Score(B)), &[selected]);
    for viewer in [A, B] {
        assert_eq!(
            game.pending_melds(viewer),
            [PendingMeld {
                player: B,
                card: CardView::Known(selected)
            }]
        );
    }
    let mut fork = game.clone();
    let mut suffix = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        finish(&mut fork, &mut suffix)
    );
    assert_eq!(game, fork);
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[1]).card, selected);
    assert_eq!(moved(&events[1]).source, Zone::Score(B));
    assert_eq!(game.position().top(B, Color::Blue), Some(selected));
    assert!(game.position().cards(Zone::Score(B)).is_empty());
}

#[test]
fn a_hidden_selected_meld_stays_private_until_its_commit_after_the_reaction() {
    let mut f = Fixture::default();
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let selected = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(selected, Zone::Hand(B), B, Color::Blue),
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(
        game.pending_melds(A),
        [PendingMeld {
            player: B,
            card: CardView::Back(Deck::base(2))
        }]
    );
    assert_eq!(
        game.pending_melds(B),
        [PendingMeld {
            player: B,
            card: CardView::Known(selected)
        }]
    );
    assert_eq!(game.position().cards(Zone::Hand(B)), &[selected]);
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(moved(&events[1]).card, selected);
}

#[test]
fn remelding_emma_runs_first_then_commits_the_retained_card_then_runs_when() {
    let mut f = Fixture::default();
    for _ in 0..5 {
        f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    }
    let emma = f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    for player in [A, B] {
        for _ in 0..2 {
            f.tech(Color::Blue, 1, Zone::Board(player, Color::Blue), vec![]);
        }
    }
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for player in [A, B] {
        game.start(
            Operation::Splay {
                player,
                color: Color::Blue,
                direction: Splay::Up,
            },
            Context::new(player),
        )
        .unwrap();
        finish(&mut game, &mut Vec::new());
    }
    game.start(
        meld(emma, Zone::Board(A, Color::Purple), A, Color::Purple),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(emma),
            value: Some(11),
            short: false,
        }))
    );
    assert_eq!(events.len(), 9);
    assert_eq!(moved(&events[0]).card, emma);
    assert_eq!(moved(&events[6]).card, emma);
    assert_eq!(moved(&events[6]).source, Zone::Deck(Deck::figures(11)));
    assert!(!moved(&events[6]).inside_karma);
    assert!(events[7..].iter().all(|e| matches!(
        e,
        Event::Splayed {
            to: Splay::None,
            inside_karma: true,
            ..
        }
    )));
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Purple)),
        &[emma]
    );
}

#[test]
fn emma_does_not_trigger_inside_another_reaction_or_after_losing_competition() {
    let mut f = Fixture::default();
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    f.figure(Figure::Kalashnikov, Zone::Board(A, Color::Red));
    let own = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let foreign = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let inner = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let program = f.tech(
        Color::Red,
        1,
        Zone::Board(B, Color::Red),
        vec![build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor))],
    );
    let outer = f.tech(Color::Green, 2, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(outer, Zone::Hand(B), B, Color::Green), Context::new(B))
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    let choice = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Karma {
                    figure: Figure::Kalashnikov,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(choice).unwrap();
    let menu = decision(&mut game, &mut events);
    let choice = menu
        .options
        .iter()
        .position(|o| *o == Alternative::SelfExecute(program))
        .unwrap();
    game.choose(choice).unwrap();
    assert!(matches!(
        finish(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, inner);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        &[own, inner]
    );
    assert_eq!(
        game.position().cards(Zone::Board(B, Color::Blue)),
        &[foreign]
    );
    assert_eq!(game.position().top(B, Color::Green), Some(outer));
}

#[test]
fn cloning_every_six_return_boundary_preserves_exact_quantity_and_meld_progress() {
    for count in [5, 6, 7] {
        let mut f = Fixture::default();
        f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
        f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
        for _ in 0..count {
            f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
        }
        let selected = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            meld(selected, Zone::Hand(B), B, Color::Blue),
            Context::new(B),
        )
        .unwrap();
        let mut expected = game.clone();
        let mut expected_events = Vec::new();
        let result = finish(&mut expected, &mut expected_events);
        let mut events = Vec::new();
        for step in 0..150 {
            let mut fork = game.clone();
            let mut suffix = Vec::new();
            assert_eq!(finish(&mut fork, &mut suffix), result);
            assert_eq!(fork, expected);
            assert_eq!(
                events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
                expected_events
            );
            match game.advance(1, &mut events).unwrap() {
                Status::WorkLimit => {}
                Status::Decision(_) => game.choose(0).unwrap(),
                status => {
                    assert_eq!(status, result);
                    assert_eq!(game, expected);
                    break;
                }
            }
            assert!(game.position().check_conservation());
            assert!(step < 149, "Emma did not terminate");
        }
    }
}
