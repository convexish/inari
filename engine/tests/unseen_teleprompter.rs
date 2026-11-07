use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
}

fn unseen_on_board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = unseen_card(code).unwrap();
    let zone = Zone::Board(player, card.color);
    f.add(card, zone)
}

fn teleprompter(f: &mut Fixture) -> CardId {
    unseen_on_board(f, "U091", A)
}

fn unseen_deck(age: u8) -> Zone {
    Zone::Deck(Deck {
        set: CardSet::Unseen,
        age,
    })
}

fn effect(game: &mut Game, card: CardId, index: u16, executor: Player) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        Context::new(executor),
    )
    .unwrap();
}

fn known(option: &Alternative) -> Option<CardId> {
    match option {
        Alternative::Select {
            target: TargetDescriptor::Known { card, .. },
        }
        | Alternative::Move {
            target: TargetDescriptor::Known { card, .. },
            ..
        }
        | Alternative::Reveal {
            target: TargetDescriptor::Known { card, .. },
        } => Some(*card),
        _ => None,
    }
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| known(option) == Some(card))
}

/// A hidden card offered by its back: the first option showing `age`.
fn pick_back(menu: &Decision, age: u8) -> Option<usize> {
    menu.options.iter().position(|option| match option {
        Alternative::Select {
            target: TargetDescriptor::Hidden { back, .. },
        }
        | Alternative::Move {
            target: TargetDescriptor::Hidden { back, .. },
            ..
        }
        | Alternative::Reveal {
            target: TargetDescriptor::Hidden { back, .. },
        }
        | Alternative::BlindMove { back, .. }
        | Alternative::BlindSelect { back, .. } => back.age == age,
        _ => false,
    })
}

fn decline(menu: &Decision) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Decline)
}

fn play(
    game: &mut Game,
    events: &mut Vec<Event>,
    deciders: &mut Vec<Player>,
    mut decide: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                deciders.push(menu.player);
                let option = decide(&menu);
                game.choose(option).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("did not terminate");
}

fn complete(status: Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

fn elevens(f: &mut Fixture, count: usize) -> Vec<CardId> {
    (0..count)
        .map(|i| f.tech(Color::ALL[i % 5], 11, Zone::Deck(Deck::base(11)), vec![]))
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

fn moved_cards(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some(m.card),
            _ => None,
        })
        .collect()
}

fn non_demand(instructions: Vec<Instruction>) -> Effect {
    Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }
}

fn demand(instructions: Vec<Instruction>) -> Effect {
    Effect {
        kind: EffectKind::Demand,
        instructions,
    }
}

fn keyword(color: Color, mode: ExecutionMode) -> Vec<Instruction> {
    vec![
        build::bind(Selector::TopOf(Who::Executor, color)),
        build::execute(mode, Who::Executor),
    ]
}

/// A no-op copy (Agriculture over an empty hand) and a reveal-only copy
/// (Illuminati revealing Writing with nothing to splay or safeguard)
/// neither return the revealed card nor repeat.
#[test]
fn a_no_op_or_reveal_only_copy_neither_returns_nor_repeats() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let agriculture = f.agriculture(Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        decline(menu).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        &[agriculture]
    );
    assert_eq!(revealed(&events), [agriculture]);
    assert!(moved_cards(&events).is_empty());

    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let illuminati = unseen(&mut f, "U064", unseen_deck(6));
    let writing = f.writing(Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        panic!("{:?}", menu.options)
    }));
    let position = game.position();
    assert_eq!(position.cards(unseen_deck(6)), &[illuminati]);
    assert_eq!(position.cards(Zone::Hand(A)), &[writing]);
    assert_eq!(revealed(&events), [illuminati, writing]);
    assert!(moved_cards(&events).is_empty());
}

/// Returning one card to Tools' "three" is activity although its reward
/// fails: Tools returns beneath the returned card and the loop repeats. The
/// next top, Oars, has no second non-demand effect, so the loop ends with
/// Oars still on top.
#[test]
fn a_partial_tools_return_counts_and_a_card_without_a_next_effect_stops_the_loop() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let tools = f.tools(Zone::Deck(Deck::base(1)));
    let oars = f.oars(Zone::Hand(A));
    let three = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        if let Some(index) = pick_known(menu, oars) {
            assert!(decline(menu).is_some(), "{:?}", menu.options);
            return index;
        }
        pick_back(menu, 1).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    let position = game.position();
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[tools, oars]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(3))), &[three]);
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(revealed(&events), [tools, oars]);
    assert_eq!(moved_cards(&events), [oars, tools]);
}

/// A value held only by the Artifacts deck is offered; its top is revealed
/// in place and returns to that deck.
#[test]
fn a_value_held_only_by_the_artifacts_deck_is_offered_and_revealed() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let artifact = f.add(
        Card {
            code: "synthetic-artifact",
            set: CardSet::Artifacts,
            color: Color::Red,
            value: 8,
            figure: None,
            echo: None,
            symbols: [Symbol::Blank; SLOTS],
            featured: Some(Icon::Authority),
            effects: vec![non_demand(vec![build::draw(1)])],
        },
        Zone::Deck(Deck {
            set: CardSet::Artifacts,
            age: 8,
        }),
    );
    let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(
        f.position(),
        Expansions::UNSEEN.union(Expansions::ARTIFACTS),
    )
    .unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        assert_eq!(menu.options.len(), 2, "{:?}", menu.options);
        pick_back(menu, 8).unwrap()
    }));
    assert_eq!(deciders, [A]);
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), &[one]);
    assert_eq!(
        position.cards(Zone::Deck(Deck {
            set: CardSet::Artifacts,
            age: 8
        })),
        &[artifact]
    );
    assert_eq!(revealed(&events), [artifact, artifact]);
}

/// Cosmology places Fight Club on top of its own deck, above itself. The
/// remembered card returns from beneath it, so Fight Club keeps the top and
/// both players know the deck's ends.
#[test]
fn the_remembered_card_returns_from_beneath_the_card_cosmology_placed_above_it() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let fight_club = unseen_on_board(&mut f, "U096", A);
    let filler = f.add(
        Card {
            code: "synthetic-unseen",
            set: CardSet::Unseen,
            color: Color::Blue,
            value: 10,
            figure: None,
            echo: None,
            symbols: [Symbol::Blank; SLOTS],
            featured: Some(Icon::Authority),
            effects: vec![],
        },
        unseen_deck(10),
    );
    let cosmology = unseen(&mut f, "U103", unseen_deck(10));
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_known(menu, fight_club)
            .or(decline(menu))
            .unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    let position = game.position();
    assert_eq!(
        position.cards(unseen_deck(10)),
        &[cosmology, filler, fight_club]
    );
    assert_eq!(revealed(&events), [cosmology, fight_club]);
    assert_eq!(moved_cards(&events), [fight_club, cosmology]);
    for viewer in Player::ALL {
        let tracker = game.tracker(viewer).unwrap();
        assert!(tracker.check_truth(position));
        for seed in 0..8 {
            let world = game.determinize(viewer, seed).unwrap();
            let deck = world.position().cards(unseen_deck(10)).to_vec();
            assert_eq!(
                (deck.first(), deck.last()),
                (Some(&cosmology), Some(&fight_club))
            );
        }
    }
}

/// A one-effect copy starts with fresh demand memory: Padlock's "if no card
/// was transferred" holds, Exile's "exactly one returned" does not, and
/// Oars' "if no cards were transferred" draws.
#[test]
fn a_one_effect_copy_sees_no_prior_demand_moves() {
    // Padlock on top of Exile: the copy offers the scoring, Padlock returns
    // beneath Exile, and Exile's second non-demand effect is absent.
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let exile = unseen(&mut f, "U018", unseen_deck(2));
    let padlock = unseen(&mut f, "U016", unseen_deck(2));
    let card = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let secret = f.tech(Color::Blue, 4, Zone::Safe(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        assert!(decline(menu).is_some(), "{:?}", menu.options);
        menu.options
            .iter()
            .position(|option| *option != Alternative::Decline)
            .unwrap()
    }));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(A)), &[card]);
    assert_eq!(position.cards(Zone::Safe(B)), &[secret]);
    assert_eq!(position.cards(unseen_deck(2)), &[padlock, exile]);
    assert_eq!(revealed(&events), [padlock, exile]);

    // Exile on top: no demand returned anything, so nothing happens.
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let padlock = unseen(&mut f, "U016", unseen_deck(2));
    let exile = unseen(&mut f, "U018", unseen_deck(2));
    let top = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let three = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_back(menu, 2).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    let position = game.position();
    assert_eq!(position.cards(unseen_deck(2)), &[padlock, exile]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(3))), &[three]);
    assert_eq!(position.top(A, Color::Red), Some(top));
    assert_eq!(revealed(&events), [exile]);
    assert!(moved_cards(&events).is_empty());

    // Oars: the copied non-demand effect draws because no demand ran. The
    // draw takes the revealed Oars itself, which then returns from the hand.
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let oars = f.oars(Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        panic!("{:?}", menu.options)
    }));
    let position = game.position();
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[oars]);
    assert_eq!(revealed(&events), [oars, oars]);
    let moves: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.card, m.source, m.destination)),
            _ => None,
        })
        .collect();
    let deck = Zone::Deck(Deck::base(1));
    assert_eq!(
        moves,
        [(oars, deck, Zone::Hand(A)), (oars, Zone::Hand(A), deck)]
    );
}

/// A win or loss inside the copied effect ends the game there: the revealed
/// card is not returned and the loop does not continue.
#[test]
fn a_terminal_result_inside_the_copy_ends_the_game_before_the_return() {
    for losing in [false, true] {
        let mut f = Fixture::default();
        let tele = teleprompter(&mut f);
        let rule = if losing {
            WinRule::Loses(Who::Executor)
        } else {
            WinRule::Executor
        };
        let x = f.tech(
            Color::Red,
            1,
            Zone::Deck(Deck::base(1)),
            vec![Instruction::Win(rule), build::draw(2)],
        );
        let two = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, tele, 0, A);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        let status = play(&mut game, &mut events, &mut deciders, |menu| {
            pick_back(menu, 1).unwrap_or_else(|| panic!("{:?}", menu.options))
        });
        assert_eq!(status, Status::Winner(if losing { B } else { A }));
        let position = game.position();
        assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[x]);
        assert_eq!(position.cards(Zone::Deck(Deck::base(2))), &[two]);
        assert_eq!(revealed(&events), [x]);
    }
}

/// Teleprompter donated through Hitchhiking: the controller picks the deck
/// and decides inside the copied effect, seeing the executor's hand; the
/// executor performs the moves and keeps the melded card.
#[test]
fn a_controller_decides_the_deck_and_the_copied_effects_choices_for_the_executor() {
    let mut f = Fixture::default();
    let hitchhiking = unseen_on_board(&mut f, "U081", A);
    let tele = unseen(&mut f, "U091", Zone::Hand(B));
    let other = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let x = f.tech(
        Color::Red,
        1,
        Zone::Deck(Deck::base(1)),
        vec![build::meld(build::HAND)],
    );
    let y = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let low = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let high = f.tech(Color::Yellow, 5, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    effect(&mut game, hitchhiking, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        if let Some(index) = pick_known(menu, low) {
            assert!(menu.options.iter().all(|option| known(option).is_some()));
            return index;
        }
        pick_known(menu, tele)
            .or(pick_back(menu, 1))
            .unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    assert_eq!(deciders, [B, B, B, B]);
    let position = game.position();
    assert_eq!(position.top(A, Color::Yellow), Some(low));
    assert_eq!(position.cards(Zone::Hand(A)), &[high]);
    assert_eq!(position.cards(Zone::Hand(B)), &[other]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[x]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(2))), &[y]);
    assert_eq!(revealed(&events), [x, x]);
    assert!(events.iter().all(|event| match event {
        Event::Moved(m) => m.executor == if m.card == tele { B } else { A },
        _ => true,
    }));
    assert!(position.cards(Zone::Achievements(A)).is_empty());
}

/// The copy itself earns no Chain, but a keyword inside the copied effect
/// does, also when that effect follows a demand on its card.
#[test]
fn a_keyword_inside_the_copied_effect_earns_chain_at_any_effect_index() {
    for demand_first in [false, true] {
        let mut f = Fixture::default();
        let tele = teleprompter(&mut f);
        f.tech(
            Color::Red,
            1,
            Zone::Board(A, Color::Red),
            vec![build::draw(1)],
        );
        let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let five = f.tech(Color::Blue, 5, Zone::Deck(Deck::base(5)), vec![]);
        let x = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
        let mut effects = vec![non_demand(keyword(Color::Red, ExecutionMode::SelfExecute))];
        if demand_first {
            effects.insert(0, demand(vec![build::draw(5)]));
        }
        f.cards[usize::from(x.0)].effects = effects;
        let eleven = elevens(&mut f, 1)[0];
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, tele, 0, A);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        complete(play(&mut game, &mut events, &mut deciders, |menu| {
            pick_back(menu, 4).unwrap_or_else(|| panic!("{:?}", menu.options))
        }));
        let position = game.position();
        let label = format!("demand_first={demand_first}");
        assert_eq!(position.cards(Zone::Achievements(A)), &[eleven], "{label}");
        assert_eq!(position.cards(Zone::Hand(A)), &[one], "{label}");
        assert!(position.cards(Zone::Hand(B)).is_empty(), "{label}");
        assert_eq!(
            position.cards(Zone::Deck(Deck::base(5))),
            &[five],
            "{label}"
        );
        assert_eq!(position.cards(Zone::Deck(Deck::base(4))), &[x], "{label}");
        assert_eq!(revealed(&events), [x, x], "{label}");
    }
}

/// Teleprompter inside a self-execution (Hitchhiking's): a plain copied
/// effect still earns no Chain; a keyword nested in it earns one.
#[test]
fn inside_a_self_execution_only_a_nested_keyword_earns_chain() {
    for nested_keyword in [false, true] {
        let mut f = Fixture::default();
        let hitchhiking = unseen_on_board(&mut f, "U081", A);
        let tele = unseen(&mut f, "U091", Zone::Hand(B));
        f.tech(
            Color::Red,
            1,
            Zone::Board(A, Color::Red),
            vec![build::draw(1)],
        );
        let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let program = if nested_keyword {
            keyword(Color::Red, ExecutionMode::SelfExecute)
        } else {
            vec![build::draw(1)]
        };
        let x = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), program);
        let eleven = elevens(&mut f, 1)[0];
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, hitchhiking, 0, A);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        complete(play(&mut game, &mut events, &mut deciders, |menu| {
            pick_known(menu, tele)
                .or(pick_back(menu, 4))
                .unwrap_or_else(|| panic!("{:?}", menu.options))
        }));
        let position = game.position();
        let label = format!("nested_keyword={nested_keyword}");
        let (achievements, deck): (&[CardId], &[CardId]) = if nested_keyword {
            (&[eleven], &[])
        } else {
            (&[], &[eleven])
        };
        assert_eq!(
            position.cards(Zone::Achievements(A)),
            achievements,
            "{label}"
        );
        assert_eq!(position.cards(Zone::Deck(Deck::base(11))), deck, "{label}");
        assert_eq!(position.cards(Zone::Hand(A)), &[one], "{label}");
        assert_eq!(position.cards(Zone::Deck(Deck::base(4))), &[x], "{label}");
        assert_eq!(revealed(&events), [x, x], "{label}");
    }
}

/// A copy inside the copy: the inner copied effect's keyword earns Chain
/// once; neither copy earns any.
#[test]
fn a_keyword_inside_a_copy_inside_the_copy_earns_chain_once() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![build::draw(1)],
    );
    f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        keyword(Color::Blue, ExecutionMode::SelfExecute),
    );
    let one = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let y = f.tech(
        Color::Purple,
        4,
        Zone::Deck(Deck::base(4)),
        keyword(Color::Red, ExecutionMode::Effect(0)),
    );
    let elevens = elevens(&mut f, 2);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_back(menu, 4).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    let position = game.position();
    assert_eq!(position.cards(Zone::Achievements(A)), &elevens[1..]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(11))), &elevens[..1]);
    assert_eq!(position.cards(Zone::Hand(A)), &[one]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(4))), &[y]);
    assert_eq!(revealed(&events), [y, y]);
}

/// The copied effect super-executes a card whose demand self-executes: the
/// super-execution keyword earns the executor's Chain, the demand's keyword
/// earns the victim none (the copied-demand boundary).
#[test]
fn a_keyword_inside_a_super_executed_demand_in_the_copy_earns_no_chain() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.cards[usize::from(red.0)].effects = vec![
        demand(vec![
            Instruction::Bind(Binding::Card(build::HAND)),
            build::execute(ExecutionMode::SelfExecute, Who::Executor),
        ]),
        non_demand(vec![build::draw(1)]),
    ];
    let child = f.tech(Color::Green, 1, Zone::Hand(B), vec![build::draw(2)]);
    let one = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let two = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let z = f.tech(
        Color::Purple,
        4,
        Zone::Deck(Deck::base(4)),
        keyword(Color::Red, ExecutionMode::SuperExecute),
    );
    let elevens = elevens(&mut f, 2);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_back(menu, 4).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    let position = game.position();
    assert_eq!(position.cards(Zone::Achievements(A)), &elevens[1..]);
    assert!(position.cards(Zone::Achievements(B)).is_empty());
    assert_eq!(position.cards(Zone::Deck(Deck::base(11))), &elevens[..1]);
    assert_eq!(position.cards(Zone::Hand(A)), &[one]);
    assert_eq!(position.cards(Zone::Hand(B)), &[child, two]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(4))), &[z]);
    assert_eq!(revealed(&events), [z, z]);
}

/// Open question 2, current reading: a Figure replacing the copied effect's only
/// operation still moves cards inside the copy, so the copy "did something" and the
/// loop continues.
#[test]
fn a_replacement_reactions_movement_inside_the_copy_counts_as_activity() {
    let mut f = Fixture::default();
    let tele = teleprompter(&mut f);
    let sejong = figure_card(Figure::SejongTheGreat.code()).unwrap();
    let sejong_color = sejong.color;
    f.add(sejong, Zone::Board(A, sejong_color));
    let red = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let equal = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let three = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
    let x = f.tech(
        Color::Purple,
        1,
        Zone::Deck(Deck::base(1)),
        vec![build::meld(build::HAND)],
    );
    let mut game = Game::new(f.position(), Expansions::UNSEEN.union(Expansions::FIGURES)).unwrap();
    effect(&mut game, tele, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_back(menu, 1).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    let position = game.position();
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(position.cards(Zone::Deck(Deck::base(2))), &[equal]);
    assert_eq!(position.top(A, Color::Yellow), Some(three));
    assert_eq!(position.top(A, Color::Red), Some(red));
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[x]);
    assert_eq!(revealed(&events), [x, x]);
    assert_eq!(moved_cards(&events), [equal, three, three, x]);
}
