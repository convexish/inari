//! Exact groups, complete preselection and every printed clause of the
//! Scissors, Magnifying Glass and Loom witnesses.
use super::cards::{card, deck_card, in_hand, offered};
use super::*;

fn start(layout: Layout, code: &str, effect: u16) -> Game {
    let mut layout = layout;
    let source = layout.add(card(code), Zone::Junk);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    game
}

/// Every operation boundary, including half-selected groups and returns,
/// resumes identically from a clone with a one-unit work budget.
fn advance(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..10000 {
        let mut clone = game.clone();
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        let status = game.advance(1, &mut actual).unwrap();
        assert_eq!(status, clone.advance(1, &mut expected).unwrap());
        assert_eq!(actual, expected);
        assert_eq!(*game, clone);
        events.extend(actual);
        if status != Status::WorkLimit {
            return status;
        }
    }
    panic!("group did not settle");
}

fn menu(game: &mut Game, events: &mut Vec<Event>) -> Decision {
    let Status::Decision(menu) = advance(game, events) else {
        panic!("expected group decision")
    };
    menu
}

fn pick_card(game: &mut Game, menu: &Decision, card: CardId) {
    let index = menu
        .options
        .iter()
        .position(|option| {
            matches!(option,
        Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
        | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
        if *c == card)
        })
        .unwrap();
    game.choose(index).unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) {
    loop {
        match advance(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => break,
            other => panic!("unexpected terminal result: {other:?}"),
        }
    }
    assert!(game.position().check_conservation());
}

#[test]
fn scissors_selects_the_whole_set_before_choosing_live_meld_score_order() {
    let mut layout = Layout::default();
    let first = in_hand(&mut layout, A, Color::Red, 1);
    let second = in_hand(&mut layout, A, Color::Blue, 2);
    let unchosen = in_hand(&mut layout, A, Color::Yellow, 3);
    let mut game = start(layout, "E021", 0);
    let mut events = Vec::new();
    let m = menu(&mut game, &mut events);
    assert_eq!(offered(&m), [first, second, unchosen]);
    pick_card(&mut game, &m, first);
    let m = menu(&mut game, &mut events);
    assert_eq!(offered(&m), [second, unchosen]);
    assert!(
        events.is_empty(),
        "nothing moves before both cards are chosen"
    );
    pick_card(&mut game, &m, second);
    let m = menu(&mut game, &mut events);
    assert_eq!(offered(&m), [first, second]);
    assert!(events.is_empty());
    pick_card(&mut game, &m, second);
    let m = menu(&mut game, &mut events);
    assert_eq!(m.options, [Alternative::Branch(0), Alternative::Branch(1)]);
    game.choose(1).unwrap(); // Score second, then meld first.
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [second]);
    assert_eq!(game.position().top(A, Color::Red), Some(first));
    assert_eq!(game.position().cards(Zone::Hand(A)), [unchosen]);

    for chosen in 0..=1 {
        let mut layout = Layout::default();
        let first = in_hand(&mut layout, A, Color::Red, 1);
        in_hand(&mut layout, A, Color::Blue, 2);
        let mut game = start(layout, "E021", 0);
        let mut events = Vec::new();
        let mut m = menu(&mut game, &mut events);
        if chosen == 1 {
            pick_card(&mut game, &m, first);
            m = menu(&mut game, &mut events);
        }
        game.choose(
            m.options
                .iter()
                .position(|o| *o == Alternative::Decline)
                .unwrap(),
        )
        .unwrap();
        finish(&mut game, &mut events);
        assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2 - chosen);
        assert_eq!(
            game.position().top(A, Color::Red),
            (chosen == 1).then_some(first)
        );
    }
}

#[test]
fn scissors_echo_scores_the_bottom_yellow_and_second_dogma_scores_only_top_paper() {
    let mut layout = Layout::default();
    let bottom = layout.add(
        printed(Color::Yellow, [Symbol::Blank; 4]),
        Zone::Board(A, Color::Yellow),
    );
    let top = layout.add(
        printed(Color::Yellow, [Symbol::Blank; 4]),
        Zone::Board(A, Color::Yellow),
    );
    let mut game = start(layout, "E021", Card::ECHO);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().cards(Zone::Score(A)), [bottom]);
    assert_eq!(game.position().top(A, Color::Yellow), Some(top));
    for owner in [A, B] {
        for covered in [false, true] {
            let mut layout = Layout::default();
            let paper = layout.add(base_card("B031").unwrap(), Zone::Board(owner, Color::Green));
            if covered {
                layout.pile(owner, Color::Green, 1, [Symbol::Blank; 4]);
            }
            let mut game = start(layout, "E021", 1);
            finish(&mut game, &mut Vec::new());
            assert_eq!(
                game.position().cards(Zone::Score(A)).contains(&paper),
                !covered
            );
        }
    }
}

#[test]
fn magnifying_glass_requires_three_equal_cards_then_returns_only_the_committed_three() {
    for ages in [&[2, 2][..], &[1, 2, 2][..]] {
        let mut layout = Layout::default();
        for &age in ages {
            in_hand(&mut layout, A, Color::Red, age);
        }
        let mut game = start(layout, "E033", 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events);
        assert!(events.is_empty());
        assert_eq!(game.position().cards(Zone::Hand(A)).len(), ages.len());
    }
    let mut layout = Layout::default();
    let cards: Vec<_> = (0..4)
        .map(|_| in_hand(&mut layout, A, Color::Red, 2))
        .collect();
    let reward = deck_card(&mut layout, CardSet::Base, Color::Green, 4);
    let mut game = start(layout, "E033", 0);
    let mut events = Vec::new();
    for &selected in &cards[..3] {
        let m = menu(&mut game, &mut events);
        assert!(events.is_empty(), "the whole group precedes every return");
        pick_card(&mut game, &m, selected);
    }
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))).len(), 3);
    let hand = game.position().cards(Zone::Hand(A));
    assert_eq!(hand.len(), 2);
    assert!(hand.contains(&cards[3]) && hand.contains(&reward));
}

#[test]
fn magnifying_glass_echo_draws_before_return_and_splays_yellow_or_blue_optionally() {
    let mut layout = Layout::default();
    let drawn = deck_card(&mut layout, CardSet::Base, Color::Red, 4);
    let mut game = start(layout, "E033", Card::ECHO);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    let moves: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.card, m.verb)),
            _ => None,
        })
        .collect();
    assert_eq!(moves, [(drawn, None), (drawn, Some(Verb::Return))]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(4))), [drawn]);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    for color in [Color::Yellow, Color::Blue] {
        let mut layout = Layout::default();
        for color in [Color::Yellow, Color::Blue, Color::Red] {
            layout.pile(A, color, 2, [Symbol::Blank; 4]);
        }
        let mut game = start(layout, "E033", 1);
        let m = menu(&mut game, &mut Vec::new());
        assert_eq!(m.options.len(), 3);
        assert!(m.options.contains(&Alternative::Decline));
        game.choose(
            m.options
                .iter()
                .position(|o| {
                    *o == Alternative::Splay {
                        player: A,
                        color,
                        direction: Splay::Left,
                    }
                })
                .unwrap(),
        )
        .unwrap();
        finish(&mut game, &mut Vec::new());
        assert_eq!(game.position().splay(A, color), Splay::Left);
        assert_eq!(game.position().splay(A, Color::Red), Splay::None);
    }
}

#[test]
fn loom_requires_distinct_values_and_interleaves_three_draws_and_tucks() {
    for different in [false, true] {
        let mut layout = Layout::default();
        for age in [1, if different { 2 } else { 1 }] {
            let mut scored = printed(Color::Green, [Symbol::Blank; 4]);
            scored.value = age;
            layout.add(scored, Zone::Score(A));
        }
        let draws: Vec<_> = [Color::Green, Color::Blue, Color::Yellow]
            .into_iter()
            .map(|color| deck_card(&mut layout, CardSet::Base, color, 6))
            .collect();
        let mut game = start(layout, "E056", 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events);
        assert_eq!(
            game.position().cards(Zone::Score(A)).len(),
            if different { 0 } else { 2 }
        );
        let movements: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) if draws.contains(&m.card) => Some((m.card, m.verb)),
                _ => None,
            })
            .collect();
        let expected: Vec<_> = if different {
            draws
                .iter()
                .rev()
                .flat_map(|&c| [(c, None), (c, Some(Verb::Tuck))])
                .collect()
        } else {
            vec![]
        };
        assert_eq!(movements, expected);
    }
}

#[test]
fn loom_echo_scores_a_lowest_top_and_heritage_counts_one_colors_visible_images() {
    let mut layout = Layout::default();
    let low = layout.add(
        printed(Color::Red, [Symbol::Blank; 4]),
        Zone::Board(A, Color::Red),
    );
    let mut high = printed(Color::Blue, [Symbol::Blank; 4]);
    high.value = 2;
    let high = layout.add(high, Zone::Board(A, Color::Blue));
    let mut game = start(layout, "E056", Card::ECHO);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().cards(Zone::Score(A)), [low]);
    assert_eq!(game.position().top(A, Color::Blue), Some(high));
    for in_one_color in [0, 4, 5] {
        let mut layout = Layout::default();
        if in_one_color > 0 {
            layout.pile(
                A,
                Color::Red,
                in_one_color,
                [Symbol::Blank, Symbol::Blank, Symbol::Blank, Symbol::Image],
            );
            layout.splay(A, Color::Red, Splay::Up);
        } else {
            for color in Color::ALL {
                layout.pile(
                    A,
                    color,
                    1,
                    [Symbol::Image, Symbol::Blank, Symbol::Blank, Symbol::Blank],
                );
            }
        }
        let mut game = start(layout, "E056", 1);
        finish(&mut game, &mut Vec::new());
        assert_eq!(
            game.position().special_location(Special::Heritage),
            if in_one_color >= 5 {
                SpecialLocation::Owned(A)
            } else {
                SpecialLocation::Available
            }
        );
    }
}
