use super::common::*;
use super::*;

fn start_effect(game: &mut Game, card: CardId, effect: u16, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
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
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                verb: Verb::Return,
                target: TargetDescriptor::Known { card, .. },
            } => {
                let deck = game.position.card(*card).origin_deck();
                Some(((deck.age, deck.set as u8, *card), index))
            }
            _ => None,
        })
        .min_by_key(|(key, _)| *key)
        .map(|(_, index)| index)
        .or_else(|| {
            menu.options.iter().position(|option| match option {
                Alternative::Move {
                    verb: Verb::Junk,
                    target: TargetDescriptor::Hidden { source, slot, .. },
                } if matches!(source, Zone::Deck(_)) => {
                    *slot + 1 == game.position.cards(*source).len()
                }
                _ => false,
            })
        })
        .expect("only canonical returns or physical-top junk choices are expected")
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> (Status, usize) {
    let mut decisions = 0;
    for _ in 0..32 {
        match settle(game, events) {
            Status::Decision(menu) => {
                decisions += 1;
                game.choose(canonical(game, &menu)).unwrap();
            }
            status => return (status, decisions),
        }
    }
    panic!("von Neumann invocation did not finish");
}

fn assert_parity(reduced: &Game, mut manual: Game) {
    manual.shortcuts_disabled = false;
    assert_eq!(*reduced, manual); // Includes continuation state, results and RNG.
    assert!(reduced.position.check_conservation());
}

fn sweep_fixture() -> (Fixture, Vec<CardId>, CardId) {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let cards: Vec<_> = [Color::Blue, Color::Green, Color::Yellow]
        .into_iter()
        .map(|color| f.tech(color, 8, Zone::Deck(Deck::base(8)), vec![]))
        .collect();
    // Distinguish physical-top traversal from identity sorting in either direction.
    f.placements
        .swap(usize::from(cards[0].0), usize::from(cards[1].0));
    let reward = f.tech(Color::Purple, 9, Zone::Deck(Deck::base(9)), vec![]);
    f.figure(Figure::HedyLamarr, Zone::Deck(Deck::figures(9)));
    f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    f.tech(Color::Yellow, 1, Zone::Junk, vec![]);
    (f, cards, reward)
}

fn start_draw(game: &mut Game) {
    game.start(
        Operation::Draw {
            value: 7,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
}

/// The reward drawn after the [8] deck was junked: a [9], short of the
/// requested age.
fn drawn(card: CardId) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(9),
        short: true,
    }))
}

#[test]
fn base_eight_sweep_reroutes_the_original_draw_with_exact_canonical_machine_parity() {
    let (f, cards, reward) = sweep_fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let initial_rng = reduced.rng.clone();
    start_draw(&mut reduced);
    start_draw(&mut manual);
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    assert_eq!(finish(&mut reduced, &mut events), (drawn(reward), 0));
    assert_eq!(finish(&mut manual, &mut manual_events), (drawn(reward), 2));
    assert_eq!(events, manual_events);
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [cards[2], cards[0], cards[1], reward]
    );
    for event in &events[..3] {
        let movement = moved(event);
        assert_eq!(movement.verb, Some(Verb::Junk));
        assert_eq!(movement.source, Zone::Deck(Deck::base(8)));
        assert_eq!(movement.destination, Zone::Junk);
        assert_eq!(movement.executor, A);
        assert!(movement.inside_karma);
    }
    let draw = moved(&events[3]);
    assert_eq!(draw.verb, None);
    assert_eq!(draw.source, Zone::Deck(Deck::base(9)));
    assert_eq!(draw.destination, Zone::Hand(A));
    assert_eq!(draw.executor, A);
    assert!(!draw.inside_karma);
    assert!(reduced.position.cards(Zone::Deck(Deck::base(8))).is_empty());
    assert_eq!(reduced.position.cards(Zone::Hand(A)), &[reward]);
    assert_eq!(reduced.rng, initial_rng);
    assert_parity(&reduced, manual);
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Game, Status)> {
    assert!(depth < 4, "three-card sweep exceeded its choice bound");
    let mut events = Vec::new();
    match settle(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            // Only hidden, unordered Junk is quotiented. Decks, hands, boards,
            // machine state, result and RNG retain their exact representation.
            game.position.junk.sort_unstable();
            game.shortcuts_disabled = false;
            vec![(game, status)]
        }
    }
}

#[test]
fn all_six_unrestricted_sweep_orders_match_the_single_reduced_outcome() {
    let (f, _, reward) = sweep_fixture();
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start_draw(&mut reduced);
    start_draw(&mut manual);
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert_eq!(reduced.len(), 1);
    assert_eq!(manual.len(), 6);
    assert_eq!(reduced[0].1, drawn(reward));
    for outcome in manual {
        assert_eq!(outcome, reduced[0]);
    }
}

fn prosperity(f: &mut Fixture, value: u8) -> CardId {
    let card = f.tech(Color::Blue, value, Zone::Hand(B), vec![]);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
    card
}

#[test]
fn oars_keeps_live_hazard_choices_before_age_eight_is_reached_but_reduces_inactive_sources() {
    for state in 0..6 {
        let mut f = Fixture::default();
        let oars = f.oars(Zone::Board(A, Color::Red));
        let source = match state {
            0 => Zone::Board(A, Color::Red),
            1 | 2 => Zone::Board(B, Color::Red),
            3 => Zone::Hand(B),
            4 => Zone::Score(B),
            5 => Zone::Junk,
            _ => unreachable!(),
        };
        let figure = f.figure(Figure::JohnVonNeumann, source);
        if state == 2 {
            f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
        }
        let high = prosperity(&mut f, 7);
        let low = prosperity(&mut f, 2);
        let first = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        let eight = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
        let nine = f.tech(Color::Purple, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        let mut context = Context::new(B);
        context.demander = A;
        context.source_owner = A;
        let mut all_events = Vec::new();
        for (game, unrestricted) in [(&mut reduced, false), (&mut manual, true)] {
            start_effect(game, oars, 0, context);
            let mut events = Vec::new();
            if state < 2 || unrestricted {
                let menu = decision(game, &mut events);
                assert_eq!(menu.player, B);
                assert_eq!(menu.options.len(), 2);
                assert!(events.is_empty());
                assert!(menu.options.iter().all(|option| matches!(
                    option,
                    Alternative::Move {
                        verb: Verb::Transfer,
                        ..
                    }
                )));
                // The higher transfer is genuinely selectable with either
                // player's active source, not merely listed in a stale menu.
                choose_card(game, &mut events, if state < 2 { high } else { low });
            }
            assert_eq!(
                settle(game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(
                game.position.cards(Zone::Score(A)),
                if state < 2 {
                    vec![high, low]
                } else {
                    vec![low, high]
                }
            );
            // First draw is age 1. Only exhausting that deck exposes the
            // age-8 draw, so a guard examining just the first draw is unsound.
            assert_eq!(moved(&events[1]).card, first);
            assert_eq!(moved(&events[1]).verb, None);
            if state == 1 {
                assert_eq!(events.len(), 7);
                assert_eq!(moved(&events[3]).card, eight);
                assert_eq!(moved(&events[3]).verb, Some(Verb::Junk));
                assert!(moved(&events[3]).inside_karma);
                assert_eq!(game.position.cards(Zone::Hand(B)), &[first, nine]);
            } else {
                assert_eq!(events.len(), if state == 3 { 7 } else { 6 });
                let expected_hand = if state == 3 {
                    vec![figure, first, eight]
                } else {
                    vec![first, eight]
                };
                assert_eq!(game.position.cards(Zone::Hand(B)), expected_hand);
            }
            assert_eq!(
                events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Revealed {
                            card,
                            source: Zone::Hand(B),
                            ..
                        } => Some(*card),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                game.position.cards(Zone::Hand(B)),
            );
            all_events.push(events);
        }
        assert_eq!(all_events[0], all_events[1]);
        assert_parity(&reduced, manual);
    }
}

#[test]
fn tools_reduces_independent_returns_before_a_real_meld_and_printed_self_execution() {
    let mut f = Fixture::default();
    let tools = f.tools_first(Zone::Board(A, Color::Blue));
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let hand: Vec<_> = [7, 1, 2]
        .into_iter()
        .map(|age| f.tech(Color::Green, age, Zone::Hand(A), vec![]))
        .collect();
    let experimentation = f.experimentation(Zone::Deck(Deck::base(4)));
    let five = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    for game in [&mut reduced, &mut manual] {
        start_effect(game, tools, 0, Context::new(A));
    }
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    let commitment = decision(&mut reduced, &mut events);
    assert_eq!(commitment.options.len(), 2);
    assert!(commitment.options.contains(&Alternative::Decline));
    assert!(events.is_empty());
    assert_eq!(
        finish(&mut reduced, &mut events),
        (Status::Complete(ReturnValue::Unit), 1)
    );
    assert_eq!(
        finish(&mut manual, &mut manual_events),
        (Status::Complete(ReturnValue::Unit), 2)
    );
    assert_eq!(events, manual_events);
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [
            hand[1],
            hand[2],
            hand[0],
            experimentation,
            experimentation,
            five,
            five
        ]
    );
    for event in &events[..3] {
        assert_eq!(moved(event).verb, Some(Verb::Return));
        assert!(!moved(event).inside_karma);
    }
    assert_eq!(moved(&events[3]).verb, None);
    assert!(!moved(&events[3]).inside_karma);
    assert_eq!(moved(&events[4]).verb, Some(Verb::Meld));
    assert_eq!(moved(&events[5]).verb, None);
    assert_eq!(moved(&events[6]).verb, Some(Verb::Meld));
    for event in &events[4..] {
        assert!(moved(event).inside_karma);
        assert_eq!(moved(event).executor, A);
    }
    assert_eq!(reduced.position.top(A, Color::Blue), Some(experimentation));
    assert_eq!(reduced.position.top(A, Color::Yellow), Some(five));
    assert!(reduced.position.cards(Zone::Hand(A)).is_empty());
    assert_parity(&reduced, manual);
}

#[test]
fn ecology_keeps_its_reduced_junk_phase_with_active_von_neumann() {
    let mut f = Fixture::default();
    let ecology = f.ecology(Zone::Board(A, Color::Yellow));
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let cards: Vec<_> = [Color::Blue, Color::Green, Color::Purple]
        .into_iter()
        .map(|color| f.tech(color, 10, Zone::Deck(Deck::base(10)), vec![]))
        .collect();
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    for game in [&mut reduced, &mut manual] {
        start_effect(game, ecology, 1, Context::new(A));
    }
    let mut events = Vec::new();
    let mut manual_events = Vec::new();
    let commitment = decision(&mut reduced, &mut events);
    assert_eq!(commitment.options.len(), 2);
    assert!(commitment.options.contains(&Alternative::Decline));
    assert!(events.is_empty());
    assert_eq!(
        finish(&mut reduced, &mut events),
        (Status::Complete(ReturnValue::Unit), 1)
    );
    assert_eq!(
        finish(&mut manual, &mut manual_events),
        (Status::Complete(ReturnValue::Unit), 2)
    );
    assert_eq!(events, manual_events);
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        cards.into_iter().rev().collect::<Vec<_>>()
    );
    assert!(
        events
            .iter()
            .all(|event| { moved(event).verb == Some(Verb::Junk) && !moved(event).inside_karma })
    );
    assert_parity(&reduced, manual);
}

#[test]
fn original_invocation_cannot_borrow_the_replacement_childs_success_for_if_you_do() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let source = f.tech(
        Color::Green,
        3,
        Zone::Board(A, Color::Green),
        vec![
            build::draw(4),
            build::verb(Verb::Meld, build::LAST).to(Destination::Board(Who::Executor)),
            build::if_(build::DID, 1),
            build::draw(6),
        ],
    );
    let experimentation = f.experimentation(Zone::Deck(Deck::base(4)));
    let five = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let forbidden_reward = f.tech(Color::Purple, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start_effect(&mut game, source, 0, Context::new(A));
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [experimentation, experimentation, five, five]
    );
    assert_eq!(game.position.top(A, Color::Blue), Some(experimentation));
    assert_eq!(game.position.top(A, Color::Yellow), Some(five));
    assert_eq!(
        game.position.cards(Zone::Deck(Deck::base(6))),
        &[forbidden_reward]
    );
    assert!(game.position.cards(Zone::Hand(A)).is_empty());
    assert!(game.position.check_conservation());
}
