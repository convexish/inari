//! Public set restrictions, physical deck sweeps and Dentures action memory.
use super::cards::{card, deck_card, dogma, in_hand, run_effect};
use super::*;

#[test]
fn flute_filters_public_sets_reveals_its_explicit_draw_and_splays() {
    let expansions = Expansions::ECHOES
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES);
    let mut layout = Layout::default();
    let source = layout.add(card("E014"), Zone::Board(A, Color::Purple));
    let base = in_hand(&mut layout, A, Color::Red, 1);
    let echo = layout.add(card("E001"), Zone::Hand(A));
    let figure = layout.add(figure_card("F006").unwrap(), Zone::Hand(A));
    let mut menus = Vec::new();
    let game = run_effect(layout, expansions, source, 0, |menu| {
        menus.push(cards::offered(menu));
        0
    });
    assert_eq!(menus, [vec![echo, figure]]);
    assert_eq!(game.position().locate(base), Some(Zone::Hand(A)));
    assert_eq!(
        game.position().locate(echo),
        Some(Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 1
        }))
    );
    assert!(!program::Test::Set(CardSet::Echoes).reads_front());

    for bonus in [false, true] {
        let mut layout = Layout::default();
        let source = layout.add(card("E014"), Zone::Board(A, Color::Purple));
        top(&mut layout, A, Color::Green, 1); // tie prevents ordinary Echoes routing
        deck_card(&mut layout, CardSet::Base, Color::Blue, 2); // Base age1 is empty
        let explicit = deck_card(&mut layout, CardSet::Echoes, Color::Red, 2);
        if bonus {
            layout.cards[usize::from(explicit.0)].symbols[0] = Symbol::Bonus(2);
        }
        let untouched = deck_card(&mut layout, CardSet::Echoes, Color::Yellow, 1);
        let mut game = Game::new(layout.position(expansions), expansions).unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 1,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            game.advance(1000, &mut events).unwrap(),
            Status::Complete(_)
        ));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::Revealed {card, ..} if *card == explicit))
        );
        assert_eq!(
            game.position().cards(Zone::Hand(A)).len(),
            if bonus { 2 } else { 1 }
        );
        assert_eq!(
            game.position().locate(untouched),
            Some(Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 1
            }))
        );
    }
    let mut layout = Layout::default();
    let source = layout.add(card("E014"), Zone::Board(A, Color::Purple));
    layout.pile(A, Color::Blue, 2, [Symbol::Blank; 4]);
    let game = run_effect(layout, expansions, source, Card::ECHO, |_| 0);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Left);
}

#[test]
fn crossbow_transfers_expansion_hand_card_and_gives_any_hand_card_to_opponent_board() {
    let expansions = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    let source = layout.add(card("E016"), Zone::Board(B, Color::Red));
    let base = in_hand(&mut layout, A, Color::Green, 2);
    let expansion = layout.add(figure_card("F006").unwrap(), Zone::Hand(A));
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Score(B)), [expansion]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [base]);
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 1,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()).unwrap(),
        Status::Complete(_)
    ));
    assert_eq!(game.position().top(B, Color::Green), Some(base));
    assert!(game.position().check_conservation());
}

#[test]
fn watermill_draws_its_bonus_then_foreseen_sweeps_the_exact_base_deck_top_down() {
    let mut layout = Layout::default();
    let source = layout.add(card("E019"), Zone::Board(A, Color::Yellow));
    let figure = layout.figure(Figure::NapoleonBonaparte, A); // shows a 6
    let age6 = deck_card(&mut layout, CardSet::Base, Color::Blue, 6);
    let game = run_effect(
        layout,
        Expansions::ECHOES.union(Expansions::FIGURES),
        source,
        0,
        |_| 0,
    );
    assert_eq!(game.position().locate(age6), Some(Zone::Hand(A)));
    assert!(game.position().locate(figure).is_some());

    let mut layout = Layout::default();
    let melded = layout.add(dogma_card(Color::Red, 5, vec![Vec::new()]), Zone::Hand(A));
    layout.add(card("E019"), Zone::Forecast(A));
    let tucked = in_hand(&mut layout, A, Color::Green, 4);
    let base: Vec<_> = (0..3)
        .map(|_| deck_card(&mut layout, CardSet::Base, Color::Green, 4))
        .collect();
    let echo = deck_card(&mut layout, CardSet::Echoes, Color::Blue, 4);
    let (game, _) = meld_action(layout, Expansions::ECHOES, melded);
    let mut expected = base;
    expected.push(tucked);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Green)),
        expected
    );
    assert!(game.position().cards(Zone::Deck(Deck::base(4))).is_empty());
    assert_eq!(
        game.position().locate(echo),
        Some(Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 4
        }))
    );
}

#[test]
fn watermill_keeps_known_deck_positions_through_napoleons_replacement() {
    let expansions = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    let driver = layout.add(
        dogma_card(Color::Purple, 7, vec![Vec::new()]),
        Zone::Hand(A),
    );
    layout.add(card("E019"), Zone::Forecast(A));
    let napoleon = layout.add(figure_card("F056").unwrap(), Zone::Hand(A));
    deck_card(&mut layout, CardSet::Base, Color::Blue, 2);
    let first = deck_card(&mut layout, CardSet::Base, Color::Green, 6);
    let returned: Vec<_> = [Color::Yellow, Color::Blue, Color::Red]
        .into_iter()
        .map(|color| in_hand(&mut layout, A, color, 6))
        .collect();
    top(&mut layout, B, Color::Blue, 5);
    let mut game = Game::new(layout.position(expansions), expansions)
        .unwrap()
        .with_tracking()
        .unwrap();
    for &card in &returned {
        let target = game
            .position()
            .targets(LogicalZone::Physical(Zone::Hand(A)))
            .find(|target| target.card == card)
            .unwrap();
        game.start(
            Operation::Move {
                verb: Verb::Return,
                target,
                destination: Zone::Deck(Deck::base(6)),
            },
            Context::new(A),
        )
        .unwrap();
        cards::settle(&mut game, |_| 0);
    }
    let deck = Zone::Deck(Deck::base(6));
    let expected: Vec<_> = returned.iter().rev().copied().collect();
    assert_eq!(
        game.tracker(A)
            .unwrap()
            .belief(deck)
            .iter()
            .map(|slot| slot.known())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .copied()
            .map(Some)
            .chain([Some(first)])
            .collect::<Vec<_>>()
    );
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    loop {
        let Status::Decision(menu) = game.advance(1000, &mut Vec::new()).unwrap() else {
            panic!("Watermill must pause in Napoleon's score choice");
        };
        if game.position().top(A, Color::Green) == Some(first) {
            assert!(menu.options.iter().all(|option| matches!(
                option,
                Alternative::Move {
                    verb: Verb::Score,
                    ..
                }
            )));
            break;
        }
        let pick = menu.options.iter().position(|option| matches!(option,
            Alternative::Move { verb: Verb::Tuck, target: TargetDescriptor::Known { card, .. }, .. } if *card == napoleon
        )).unwrap_or(0);
        game.choose(pick).unwrap();
    }
    assert_eq!(game.position().cards(deck), expected);
    assert_eq!(
        game.tracker(A)
            .unwrap()
            .belief(deck)
            .iter()
            .map(|slot| slot.known())
            .collect::<Vec<_>>(),
        expected.iter().copied().map(Some).collect::<Vec<_>>()
    );
    for seed in 0..32 {
        let sampled = game.determinize(A, seed).unwrap();
        assert_eq!(sampled.position().cards(deck), expected);
        assert_eq!(sampled.observe(A).unwrap(), game.observe(A).unwrap());
        assert!(sampled.tracker(A).unwrap().check_truth(sampled.position()));
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
}

#[test]
fn dentures_remembers_its_original_tuck_and_scores_only_top_nonbottom_cards() {
    for existing in [0, 1, 3] {
        let mut layout = Layout::default();
        let source = layout.add(card("E058"), Zone::Board(A, Color::Yellow));
        let old: Vec<_> = (0..existing)
            .map(|i| top(&mut layout, A, Color::Red, i + 1))
            .collect();
        // The first original tuck may create a singleton: the dogma must
        // draw/tuck again, updating its action record before repeating.
        let second = deck_card(&mut layout, CardSet::Base, Color::Red, 6);
        let first = deck_card(&mut layout, CardSet::Base, Color::Red, 6);
        let (game, _) = dogma(layout, Expansions::ECHOES, source, |_| 0);
        let score = game.position().cards(Zone::Score(A));
        match existing {
            0 => {
                assert_eq!(score, [first]);
                assert_eq!(game.position().top(A, Color::Red), Some(second));
            }
            1 => {
                assert_eq!(score, old);
                assert_eq!(game.position().top(A, Color::Red), Some(first));
            }
            _ => {
                assert_eq!(score, [old[2], old[1]]);
                assert_eq!(
                    game.position().cards(Zone::Board(A, Color::Red)),
                    [first, old[0]]
                );
            }
        }
        assert!(game.position().check_conservation());
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
}

#[test]
fn dentures_keeps_the_last_original_tuck_when_a_later_copy_is_replaced() {
    use program::build;
    let expansions = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    let dentures = layout.add(card("E058"), Zone::Board(A, Color::Yellow));
    let driver = layout.add(
        dogma_card(
            Color::Green,
            8,
            vec![vec![
                build::bind(Selector::TopOf(Who::Executor, Color::Yellow)),
                crate::build::execute(ExecutionMode::SelfExecute, Who::Executor),
                build::meld(build::HAND),
                build::bind(Selector::TopOf(Who::Executor, Color::Yellow)),
                crate::build::execute(ExecutionMode::SelfExecute, Who::Executor),
            ]],
        ),
        Zone::Board(A, Color::Green),
    );
    let napoleon = layout.add(figure_card("F056").unwrap(), Zone::Hand(A));
    let red: Vec<_> = (1..=3)
        .map(|age| top(&mut layout, A, Color::Red, age))
        .collect();
    let replaced = deck_card(&mut layout, CardSet::Base, Color::Blue, 6);
    let original = deck_card(&mut layout, CardSet::Base, Color::Red, 6);
    let (game, won) = dogma(layout, expansions, driver, |menu| {
        // A later optional blue splay is irrelevant to the remembered tuck.
        menu.options
            .iter()
            .position(|o| *o == Alternative::Decline)
            .unwrap_or(0)
    });
    assert!(!won);
    assert_eq!(
        game.position().cards(Zone::Score(A)),
        [red[2], red[1], napoleon, red[0]]
    );
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [original]
    );
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        [replaced]
    );
    assert_eq!(game.position().top(A, Color::Yellow), Some(dentures));
    assert!(game.position().check_conservation());
}
