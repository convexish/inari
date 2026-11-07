use super::common::*;
use super::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn top(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}
fn start(game: &mut Game, source: CardId, effect: u16) {
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}
fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..1000 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("constrained program did not terminate");
}
fn originals(events: &[Event], verb: Verb) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(movement) if movement.verb == Some(verb) && !movement.inside_karma => {
                Some(movement.card)
            }
            _ => None,
        })
        .collect()
}
fn choose_pair(game: &mut Game, first: CardId, second: CardId, events: &mut Vec<Event>) {
    let menu = decision(game, events);
    let index = menu
        .options
        .iter()
        .position(|option| {
            matches!(option,
        Alternative::MatchingPair {
            first: TargetDescriptor::Known { card: a, .. },
            second: TargetDescriptor::Known { card: b, .. },
        } if *a == first && *b == second)
        })
        .unwrap();
    game.choose(index).unwrap();
}
fn choose_colors(game: &mut Game, colors: &[Color], events: &mut Vec<Event>) {
    let mask = colors
        .iter()
        .fold(0, |mask, &color| mask | (1 << color as u8));
    let menu = decision(game, events);
    assert!(
        menu.options
            .iter()
            .all(|option| matches!(option, Alternative::Colors(mask) if mask.count_ones() == 2))
    );
    game.choose(
        menu.options
            .iter()
            .position(|option| *option == Alternative::Colors(mask))
            .unwrap(),
    )
    .unwrap();
}

fn mobility<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B077", Zone::Junk);
    let buried = top(&mut f, "B010", A);
    let blue = top(&mut f, "B082", A);
    let green = top(&mut f, "B081", A);
    let purple = top(&mut f, "B025", A);
    let excluded = top(&mut f, "B079", A);
    let red = top(&mut f, "B076", A);
    let reward = base(&mut f, "B078", Zone::Deck(Deck::base(8)));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let eligible: Vec<_> = menu
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card, .. },
                ..
            } => Some(*card),
            _ => None,
        })
        .collect();
    assert!(eligible.contains(&blue) && eligible.contains(&green));
    assert!(
        !eligible.contains(&excluded) && !eligible.contains(&red) && !eligible.contains(&purple)
    );
    game.choose(
        menu.options
            .iter()
            .position(|option| {
                matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == blue)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(originals(&events, Verb::Transfer), [blue, green]);
    assert_eq!(game.position.top(A, Color::Blue), Some(buried));
    assert_eq!(game.position.top(A, Color::Purple), Some(purple));
    assert!(game.position.cards(Zone::Hand(A)).contains(&reward));
    assert!(game.position.check_conservation());
}
#[test]
fn mobility_keeps_live_highest_ties_and_consumes_colors_in_both_kernels() {
    mobility::<false>();
    mobility::<true>();
}

fn pair_boundaries<const F: bool>() {
    for matching in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B110", Zone::Junk);
        let red = top(&mut f, "B016", A);
        let green = top(&mut f, if matching { "B020" } else { "B008" }, A);
        let low = base(&mut f, "B001", Zone::Hand(A));
        let equal = base(&mut f, "B025", Zone::Score(A));
        let high = base(&mut f, "B081", Zone::Score(A));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        start(&mut game, source, 0);
        let mut events = Vec::new();
        if matching {
            choose_pair(&mut game, red, green, &mut events);
        }
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        if matching {
            let returned = originals(&events, Verb::Return);
            assert_eq!(&returned[..2], &[red, green]);
            assert!(returned.contains(&low) && returned.contains(&equal));
        } else {
            assert!(originals(&events, Verb::Return).is_empty());
            assert_eq!(game.position.top(A, Color::Red), Some(red));
        }
        assert!(game.position.cards(Zone::Score(A)).contains(&high));
    }
    let mut f = Fixture::default();
    let source = base(&mut f, "B110", Zone::Junk);
    top(&mut f, "B001", A);
    let upper = top(&mut f, "B002", A);
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    assert_eq!(game.position.top(A, Color::Red), Some(upper));
}
#[test]
fn hypersonics_requires_a_complete_distinct_color_pair_before_any_return() {
    pair_boundaries::<false>();
    pair_boundaries::<true>();
}

#[test]
fn hypersonics_revalidates_committed_second_identity_after_future_reaction_state_changes() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B110", Zone::Junk);
    let first = top(&mut f, "B001", A);
    let second = top(&mut f, "B008", A);
    let cover = base(&mut f, "B020", Zone::Hand(A));
    let low = base(&mut f, "B004", Zone::Score(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    choose_pair(&mut game, first, second, &mut events);
    while originals(&events, Verb::Return).is_empty() {
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    // Model the post-state of a future return reaction covering the second
    // original, without inventing a Figure or attributing a child result to the
    // pair. The authored native program and both selected identities are intact.
    game.position.players[A.index()]
        .hand
        .retain(|&card| card != cover);
    game.position.players[A.index()].stacks[Color::Green as usize].push(cover);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(originals(&events, Verb::Return), [first]);
    assert_eq!(game.position.top(A, Color::Green), Some(cover));
    assert!(game.position.cards(Zone::Score(A)).contains(&low));
    assert!(game.position.check_conservation());
}

fn empiricism<const F: bool>() {
    for matching in [false, true] {
        for stack_size in 0..=2 {
            let mut f = Fixture::default();
            let source = base(&mut f, "B084", Zone::Junk);
            for code in ["B011", "B012"].into_iter().take(stack_size) {
                top(&mut f, code, A);
            }
            let drawn = base(&mut f, "B093", Zone::Deck(Deck::base(9)));
            let mut position = f.position();
            if stack_size == 2 {
                position.players[A.index()].splays[Color::Blue as usize] = Splay::Right;
            }
            let mut game = Game::new(position, Expansions::with_figures(F)).unwrap();
            start(&mut game, source, 0);
            let mut events = Vec::new();
            choose_colors(
                &mut game,
                if matching {
                    &[Color::Blue, Color::Green]
                } else {
                    &[Color::Red, Color::Green]
                },
                &mut events,
            );
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                originals(&events, Verb::Meld),
                if matching { vec![drawn] } else { vec![] }
            );
            assert_eq!(
                game.position.splay(A, Color::Blue),
                if matching && stack_size > 0 {
                    Splay::Up
                } else {
                    Splay::None
                }
            );
            assert_eq!(
                game.position.cards(Zone::Hand(A)).contains(&drawn),
                !matching
            );
        }
    }
}
#[test]
fn empiricism_matches_exact_draw_and_splays_or_unsplays_empty_singleton_and_stacks() {
    empiricism::<false>();
    empiricism::<true>();
}

#[test]
fn empiricism_uses_original_draw_color_not_curie_child_color() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B084", Zone::Junk);
    f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    top(&mut f, "B008", A);
    let original = base(&mut f, "B091", Zone::Deck(Deck::base(9)));
    let child = base(&mut f, "B088", Zone::Deck(Deck::base(9)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    choose_colors(&mut game, &[Color::Green, Color::Red], &mut events);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(originals(&events, Verb::Meld), [original]);
    assert!(game.position.cards(Zone::Hand(A)).contains(&child));
    assert_eq!(game.position.splay(A, Color::Green), Splay::Up);
}

#[test]
fn empiricism_win_reads_effective_concepts_at_its_separate_instruction() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B084", Zone::Junk);
    for code in ["B011", "B012", "B023", "B025", "B093"] {
        top(&mut f, code, A);
    }
    f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    let mut position = f.position();
    position.players[A.index()].splays[Color::Blue as usize] = Splay::Up;
    position.players[A.index()].splays[Color::Purple as usize] = Splay::Up;
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    assert!(game.position.basic_icon_count(A, Icon::Concept) < 20);
    assert!(game.position.icon_count(A, Icon::Concept) >= 20);
    start(&mut game, source, 1);
    assert_eq!(finish(&mut game, &mut Vec::new()), Status::Winner(A));
}

fn climatology<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B112", Zone::Junk);
    let buried = top(&mut f, "B001", A);
    let upper = top(&mut f, "B002", A);
    let green = top(&mut f, "B009", A);
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    start(&mut game, source, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B);
    assert!(!menu.options.contains(&Alternative::Icon(Icon::Health)));
    assert!(menu.options.contains(&Alternative::Icon(Icon::Avatar)));
    assert!(!menu.options.contains(&Alternative::Icon(Icon::Food)));
    game.choose(
        menu.options
            .iter()
            .position(|option| *option == Alternative::Icon(Icon::Authority))
            .unwrap(),
    )
    .unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    game.choose(
        menu.options
            .iter()
            .position(|option| {
                matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == upper)
            })
            .unwrap(),
    )
    .unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    game.choose(
        menu.options
            .iter()
            .position(|option| {
                matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == buried)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(originals(&events, Verb::Return), [upper, buried]);
    assert_eq!(game.position.top(A, Color::Green), Some(green));

    let mut f = Fixture::default();
    let source = base(&mut f, "B112", Zone::Junk);
    let returned = top(&mut f, "B016", A);
    let low = base(&mut f, "B001", Zone::Score(A));
    let equal = base(&mut f, "B025", Zone::Score(A));
    let high = base(&mut f, "B081", Zone::Score(A));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    start(&mut game, source, 1);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    let cards = originals(&events, Verb::Return);
    assert_eq!(cards[0], returned);
    assert!(cards.contains(&equal) && cards.contains(&high) && !cards.contains(&low));
}
#[test]
fn climatology_chooser_does_not_take_control_and_newly_exposed_same_color_is_eligible() {
    climatology::<false>();
    climatology::<true>();
}

#[test]
fn hypersonics_union_deduplicates_aliases_and_uses_native_rng_for_foreign_hidden_cards() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B110", Zone::Junk);
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let first = top(&mut f, "B011", A);
    let second = top(&mut f, "B008", A);
    let owned = base(&mut f, "B004", Zone::Hand(B));
    let hidden_one = base(&mut f, "B001", Zone::Hand(A));
    let hidden_two = base(&mut f, "B002", Zone::Hand(A));
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 91).unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    choose_pair(&mut game, first, second, &mut events);
    let menu = decision(&mut game, &mut events);
    let blind = Alternative::BlindMove {
        verb: Verb::Return,
        source: Zone::Hand(A),
        back: Deck {
            set: CardSet::Base,
            age: 1,
        },
    };
    assert!(menu.options.contains(&blind));
    assert!(menu.options.iter().any(|option| matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == owned)));
    assert!(!menu.options.iter().any(|option| matches!(
        option,
        Alternative::Move {
            target: TargetDescriptor::Hidden { .. },
            ..
        }
    )));
    let mut expected_rng = game.rng.clone();
    let slot = (expected_rng.next_u64() % 2) as usize;
    game.choose(
        menu.options
            .iter()
            .position(|option| *option == blind)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(game.rng, expected_rng);
    let start = originals(&events, Verb::Return).len();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        originals(&events, Verb::Return)[start],
        [hidden_one, hidden_two][slot]
    );
    assert_eq!(game.rng, expected_rng); // singleton continuation consumes no RNG
    for card in [owned, hidden_one, hidden_two] {
        assert_eq!(
            originals(&events, Verb::Return)
                .iter()
                .filter(|&&id| id == card)
                .count(),
            1
        );
    }
    assert!(game.position.check_conservation());
}

fn oracle_fixture(code: &str) -> (Position, CardId) {
    let mut f = Fixture::default();
    let source = base(&mut f, code, Zone::Junk);
    match code {
        "B077" => {
            for code in ["B078", "B081", "B082"] {
                top(&mut f, code, A);
            }
            base(&mut f, "B085", Zone::Deck(Deck::base(8)));
        }
        "B084" => {
            top(&mut f, "B011", A);
            base(&mut f, "B093", Zone::Deck(Deck::base(9)));
        }
        "B110" => {
            for code in ["B001", "B004", "B008"] {
                top(&mut f, code, A);
            }
            base(&mut f, "B011", Zone::Hand(A));
            base(&mut f, "B013", Zone::Score(A));
        }
        "B112" => {
            for code in ["B001", "B006", "B009"] {
                top(&mut f, code, A);
            }
        }
        _ => unreachable!(),
    }
    (f.position(), source)
}
fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng, Status)> {
    assert!(depth < 12);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut fork = game.clone();
                fork.choose(index).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert!(game.position.check_conservation());
            // Score piles are unordered: only their contents matter.
            let mut position = game.position;
            position.sort_unordered_piles();
            vec![(position, game.rng, status)]
        }
    }
}
fn oracles<const F: bool>() {
    for code in ["B077", "B084", "B110", "B112"] {
        let (position, source) = oracle_fixture(code);
        let mut reduced = Game::seeded(position, Expansions::with_figures(F), 713).unwrap();
        start(&mut reduced, source, 0);
        let mut unrestricted = reduced.clone();
        unrestricted.shortcuts_disabled = true;
        // The same ends, reached once per chosen set under the reduction
        // and once per order without it.
        let (reduced_ends, manual_ends) = (
            outcomes(reduced.clone(), 0),
            outcomes(unrestricted.clone(), 0),
        );
        assert!(reduced_ends.len() <= manual_ends.len(), "{code}");
        for (left, right) in [(&reduced_ends, &manual_ends), (&manual_ends, &reduced_ends)] {
            assert!(left.iter().all(|end| right.contains(end)), "{code}");
        }
        let mut events = Vec::new();
        for _ in 0..1000 {
            let mut clone = reduced.clone();
            let mut one = Vec::new();
            let mut two = Vec::new();
            let status = reduced.advance(1, &mut one).unwrap();
            assert_eq!(status, clone.advance(1, &mut two).unwrap());
            assert_eq!(one, two);
            assert_eq!(reduced, clone);
            events.extend(one);
            match status {
                Status::WorkLimit => {}
                Status::Decision(_) => reduced.choose(0).unwrap(),
                Status::Complete(ReturnValue::Unit) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(reduced.frames.is_empty());
        let mut expected = Vec::new();
        assert_eq!(
            finish(&mut unrestricted, &mut expected),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events, expected);
        unrestricted.shortcuts_disabled = false;
        assert_eq!(reduced, unrestricted);
    }
}
#[test]
fn all_tied_subsets_pair_orders_canonical_events_rng_and_clones_match_unrestricted() {
    oracles::<false>();
    oracles::<true>();
}
