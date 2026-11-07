use super::*;
use crate::execution::shortcuts::common::*;

fn start(f: Fixture, code: &str, figure: Option<Figure>) -> Game {
    let mut f = f;
    if let Some(figure) = figure {
        let card = figure_card(figure.code()).unwrap();
        let color = card.color;
        f.add(card, Zone::Board(A, color));
    }
    let source = f.add(echoes_card(code).unwrap(), Zone::Junk);
    let mut game = Game::seeded(
        f.position(),
        Expansions::ECHOES.union(Expansions::FIGURES),
        17,
    )
    .unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    game
}

/// Exhaust every decision path, including optional subsets, every within-
/// deck order and actual Figure reactions. Preserve full final state/RNG;
/// semantic events commute only as bags, since return order across decks is
/// the reduction under proof.
fn outcomes(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<(Game, Vec<String>)> {
    assert!(depth < 30);
    match settle(&mut game, &mut events) {
        Status::Decision(m) => (0..m.options.len())
            .flat_map(|index| {
                let mut branch = game.clone();
                branch.choose(index).unwrap();
                outcomes(branch, events.clone(), depth + 1)
            })
            .collect(),
        Status::Complete(_) => {
            assert!(game.position.check_conservation());
            game.position.sort_unordered_piles();
            game.shortcuts_disabled = false;
            let mut events: Vec<_> = events
                .into_iter()
                .map(|event| format!("{event:?}"))
                .collect();
            events.sort();
            vec![(game, events)]
        }
        other => panic!("unexpected {other:?}"),
    }
}

fn fixture(code: &str) -> Fixture {
    let mut f = Fixture::default();
    match code {
        "E033" => {
            for set in [
                CardSet::Base,
                CardSet::Base,
                CardSet::Echoes,
                CardSet::Echoes,
            ] {
                let id = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
                f.cards[usize::from(id.0)].set = set;
            }
            f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
        }
        "E056" => {
            for (value, set) in [(1, CardSet::Base), (2, CardSet::Base), (2, CardSet::Echoes)] {
                let id = f.tech(Color::Red, value, Zone::Score(A), vec![]);
                f.cards[usize::from(id.0)].set = set;
            }
            for color in [Color::Green, Color::Purple, Color::Yellow] {
                f.tech(color, 6, Zone::Deck(Deck::base(6)), vec![]);
            }
        }
        "E021" => {
            for _ in 0..2 {
                f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
            }
            f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
            f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        }
        _ => unreachable!(),
    }
    for _ in 0..4 {
        f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    }
    f
}

#[test]
fn all_group_shortcut_paths_match_the_oracle_and_reacting_figures_keep_real_orders() {
    for code in ["E033", "E056", "E021"] {
        for figure in [
            None,
            Some(Figure::Daedalus),
            Some(if code == "E021" {
                Figure::CaiLun
            } else {
                Figure::JamesClerkMaxwell
            }),
        ] {
            let reduced = start(fixture(code), code, figure);
            let mut manual = reduced.clone();
            manual.shortcuts_disabled = true;
            let reduced = outcomes(reduced, vec![], 0);
            let manual = outcomes(manual, vec![], 0);
            assert!(
                manual.iter().all(|o| reduced.contains(o)),
                "{code} {figure:?}: missing outcome"
            );
            assert!(
                reduced.iter().all(|o| manual.contains(o)),
                "{code} {figure:?}: extra outcome"
            );
            if code != "E021" && figure != Some(Figure::JamesClerkMaxwell) {
                assert!(
                    reduced.len() < manual.len(),
                    "{code} {figure:?}: no reduction"
                );
            } else {
                assert_eq!(
                    reduced.len(),
                    manual.len(),
                    "{code} {figure:?}: removed a guarded order"
                );
            }
            if figure == Some(Figure::JamesClerkMaxwell) {
                assert!(
                    manual.iter().any(|(game, events)| {
                        game.position.cards(Zone::Deck(Deck::base(8))).len() < 4
                            && events
                                .iter()
                                .any(|event| event.contains("inside_karma: true"))
                    }),
                    "Maxwell must actually draw before a return"
                );
            }
            if figure == Some(Figure::CaiLun) {
                assert!(
                    manual
                        .iter()
                        .any(|(game, _)| game.position.splay(A, Color::Red) == Splay::Left)
                );
            }
        }
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn committed_group_rechecks_originals_and_never_uses_a_newly_eligible_substitute() {
    let mut f = Fixture::default();
    let cards: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Red, 2, Zone::Hand(A), vec![]))
        .collect();
    let substitute = f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let reward = f.tech(Color::Green, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = start(f, "E033", None);
    let mut events = Vec::new();
    // Exact admission needs a choice to opt in; subsequent singleton picks
    // auto-complete. Then choose the first actual return.
    assert!(
        decision(&mut game, &mut events)
            .options
            .contains(&Alternative::Decline)
    );
    game.choose(0).unwrap();
    let m = decision(&mut game, &mut events);
    assert!(m.options.iter().all(|o| matches!(
        o,
        Alternative::Move {
            verb: Verb::Return,
            ..
        }
    )));
    game.choose(0).unwrap();
    while !events
        .iter()
        .any(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Return)))
    {
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    // Model a First reaction's post-state: an original leaves the hand,
    // while an equally eligible new card arrives. This is a frozen cohort.
    let missing = cards[1];
    game.position.players[A.index()]
        .hand
        .retain(|&c| c != missing);
    game.position.players[A.index()]
        .score
        .retain(|&c| c != substitute);
    game.position.players[A.index()].hand.push(substitute);
    game.position.players[A.index()].score.push(missing);
    game.position.refresh_boards();
    while let Status::Decision(_) = settle(&mut game, &mut events) {
        game.choose(0).unwrap();
    }
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(2))).len(), 2);
    assert_eq!(game.position.locate(substitute), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(reward),
        Some(Zone::Deck(Deck::base(4))),
        "partial success earns no draw"
    );
    assert!(game.position.check_conservation());

    // An actual Instead reaction replaces the first return, then Napoleon
    // scores himself (the only top 5/6), so the other two return normally.
    // The replacement's meld still cannot earn Magnifying Glass's draw.
    let mut f = Fixture::default();
    for _ in 0..3 {
        f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    }
    let reward = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = start(f, "E033", Some(Figure::NapoleonBonaparte));
    let mut events = Vec::new();
    while let Status::Decision(_) = settle(&mut game, &mut events) {
        game.choose(0).unwrap();
    }
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(2))).len(), 2);
    assert_eq!(game.position.cards(Zone::Board(A, Color::Green)).len(), 1);
    assert_eq!(
        game.position.locate(reward),
        Some(Zone::Deck(Deck::base(4)))
    );
}

#[test]
fn hidden_group_selection_and_frozen_block_order_use_only_backs_and_native_sampling() {
    let mut f = Fixture::default();
    let first = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let second = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
    let source = f.add(echoes_card("E021").unwrap(), Zone::Junk);
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 17)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    let mut events = Vec::new();
    let m = decision(&mut game, &mut events);
    assert_eq!(
        m.options,
        [
            Alternative::BlindSelect {
                source: Zone::Hand(A),
                back: Deck::base(1)
            },
            Alternative::BlindSelect {
                source: Zone::Hand(A),
                back: Deck::base(2)
            },
            Alternative::Decline,
        ]
    );
    let mut expected = game.rng.clone();
    let selected = [first, second][(expected.next_u64() % 2) as usize];
    game.choose(0).unwrap();
    assert_eq!(game.rng, expected);
    let m = decision(&mut game, &mut events);
    assert_eq!(m.options.len(), 3);
    assert!(events.is_empty());
    for seed in 0..8 {
        let world = game.determinize(B, seed).unwrap();
        assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
        assert!(world.position.check_conservation());
    }
    let mut same_back = game.clone();
    same_back.choose(0).unwrap();
    let m = decision(&mut same_back, &mut Vec::new());
    assert_eq!(m.options, [Alternative::Branch(0), Alternative::Branch(1)]);
    let mut expected_same_back = expected.clone();
    expected_same_back.next_u64(); // Native random order within the one back.
    assert_eq!(same_back.rng, expected_same_back);
    // Select the distinct back. Processing then asks only which back first;
    // it must not expose the sampled hidden card's original hand slot.
    game.choose(1).unwrap();
    let m = decision(&mut game, &mut events);
    assert_eq!(
        m.options,
        [
            Alternative::BlindSelect {
                source: Zone::Hand(A),
                back: Deck::base(1)
            },
            Alternative::BlindSelect {
                source: Zone::Hand(A),
                back: Deck::base(2)
            },
        ]
    );
    game.choose(0).unwrap();
    let m = decision(&mut game, &mut events);
    assert_eq!(m.options, [Alternative::Branch(0), Alternative::Branch(1)]);
    game.choose(1).unwrap();
    while let Status::Decision(_) = settle(&mut game, &mut events) {
        game.choose(0).unwrap();
    }
    assert_eq!(game.position.cards(Zone::Score(A)), [selected]);
    assert_eq!(
        game.rng, expected,
        "singleton hidden-back picks consume no RNG"
    );
}

fn tracked_scissors(controller: Player, figure: bool, reverse: bool, batch: bool) -> Game {
    let mut f = Fixture::default();
    for code in ["B001", "B002", "B003"] {
        f.add(base_card(code).unwrap(), Zone::Hand(A));
    }
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    if figure {
        let card = figure_card(Figure::Kalashnikov.code()).unwrap();
        let color = card.color;
        f.add(card, Zone::Board(B, color));
    }
    let mut scissors = echoes_card("E021").unwrap();
    if batch {
        // The same one-item work expressed as a generic slot-filtered batch
        // exercises the reference lookup with MoveAll above its program.
        for instruction in &mut scissors.effects[0].instructions {
            if matches!(instruction, Instruction::Move(_)) {
                *instruction = instruction.all();
            }
        }
    }
    let source = f.add(scissors, Zone::Junk);
    let mut position = f.position();
    if reverse {
        position.players[A.index()].hand.rotate_left(1);
    }
    let mut game = Game::seeded(position, Expansions::ECHOES.union(Expansions::FIGURES), 17)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.controller = controller;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}

fn scissors_slot(game: &Game, index: usize) -> &[HeldCard] {
    game.frames
        .iter()
        .find_map(|frame| match frame {
            Frame::Locals(locals) => match &locals.slots[index] {
                Some(slots::Held::Cards(cards)) => Some(cards.as_slice()),
                _ => None,
            },
            _ => None,
        })
        .expect("Scissors records its selected cards")
}

#[test]
fn sampled_group_members_stay_distinct_through_menus_slots_and_nested_worlds() {
    let mut roots = Vec::new();
    for reverse in [false, true] {
        let mut game = tracked_scissors(B, false, reverse, false);
        decision(&mut game, &mut vec![]);
        game.choose(0).unwrap();
        let menu = decision(&mut game, &mut vec![]);
        assert_eq!(menu.options.len(), 2); // same-back second choice or stop
        roots.push(game);
    }
    assert_eq!(roots[0].observe(B).unwrap(), roots[1].observe(B).unwrap());
    let original = |root: &Game| match &root.frames[root.frames.len() - 2] {
        Frame::SelectGroup(group) => group.selected[0].card,
        _ => unreachable!(),
    };
    assert_ne!(original(&roots[0]), original(&roots[1]));
    let mut sampled_first = std::collections::BTreeSet::new();
    for seed in 0..64 {
        let mut cohorts = Vec::new();
        for root in &roots {
            let observe::PromptView::Decision(root_menu) = root.prompt_view() else {
                panic!("partial group decision");
            };
            let mut search = root.sample_world(B, seed).unwrap().world;
            assert!(search.knowledge.is_none());
            let search_menu = decision(&mut search, &mut vec![]);
            assert_eq!(search_menu.options, root_menu.options);
            assert_eq!(search_menu.source_card(B), root_menu.source_card(B));
            search.choose(0).unwrap();
            let search_branch = decision(&mut search, &mut vec![]);
            let mut world = root.clone().determinize(B, seed).unwrap();
            assert_eq!(world.observe(B).unwrap(), root.observe(B).unwrap());
            let first = match &world.frames[world.frames.len() - 2] {
                Frame::SelectGroup(group) => group.selected[0].card,
                _ => panic!("partial group"),
            };
            sampled_first.insert(first);
            world.choose(0).unwrap();
            let menu = decision(&mut world, &mut vec![]);
            assert_eq!(
                menu.options,
                [Alternative::Branch(0), Alternative::Branch(1)]
            );
            assert!(
                menu.source_card(B).is_some(),
                "the tracker names the source pick"
            );
            assert!(menu.option_label(0, B).is_some());
            assert!(menu.option_label(1, B).is_some());
            assert_eq!(search_branch.options, menu.options);
            assert_eq!(search_branch.source_card(B), menu.source_card(B));
            for option in 0..2 {
                assert_eq!(
                    search_branch.option_label(option, B),
                    menu.option_label(option, B)
                );
            }
            let cards = scissors_slot(&world, 0);
            assert_eq!(cards.len(), 2);
            assert_ne!(
                cards[0].inner, cards[1].inner,
                "seed {seed}: remaining menu duplicated a member"
            );
            cohorts.push(cards.iter().map(|c| c.inner).collect::<Vec<_>>());
            for nested_seed in 0..8 {
                let mut nested = world
                    .clone()
                    .determinize(B, seed * 8 + nested_seed)
                    .unwrap();
                let nested_menu = decision(&mut nested, &mut vec![]);
                assert_eq!(nested_menu.source_card(B), menu.source_card(B));
                for option in 0..2 {
                    assert_eq!(
                        nested_menu.option_label(option, B),
                        menu.option_label(option, B)
                    );
                }
                assert_eq!(nested.observe(B).unwrap(), world.observe(B).unwrap());
                let cards = scissors_slot(&nested, 0);
                assert_ne!(cards[0].inner, cards[1].inner, "completed cohort collapsed");
                assert!(cards.contains(&scissors_slot(&nested, 1)[0]));
                let chosen = cards.iter().map(|c| c.inner).collect::<Vec<_>>();
                nested.choose(1).unwrap(); // score this item
                let menu = decision(&mut nested, &mut vec![]);
                assert_eq!(
                    menu.options,
                    [Alternative::Branch(0), Alternative::Branch(1)]
                );
                nested.choose(1).unwrap();
                assert!(matches!(
                    settle(&mut nested, &mut vec![]),
                    Status::Complete(_)
                ));
                let mut scored = nested.position.cards(Zone::Score(A)).to_vec();
                let mut chosen = chosen;
                chosen.sort();
                scored.sort();
                assert_eq!(scored, chosen);
                assert!(nested.position.check_conservation());
            }
        }
        assert_eq!(
            cohorts[0], cohorts[1],
            "hidden physical order affects sampled group"
        );
    }
    assert_eq!(
        sampled_first.len(),
        3,
        "sampling must not pin the real first selection"
    );

    // Retired cohort picks may be reused without their old inequalities.
    let mut knowledge = roots[0].knowledge.clone().unwrap();
    let a = knowledge.allocate().unwrap();
    let b = knowledge.allocate().unwrap();
    knowledge.distinct[a.index()] |= 1 << b.index();
    knowledge.distinct[b.index()] |= 1 << a.index();
    knowledge.retain_picks(1 << b.index());
    assert!(knowledge.distinct.iter().all(|&peers| peers == 0));
    let reused = knowledge.allocate().unwrap();
    assert_eq!(reused.index(), 0);
    assert_eq!(knowledge.distinct[reused.index()], 0);
}

#[test]
fn scissors_named_item_keeps_one_pick_through_an_opponents_first_reaction() {
    for batch in [false, true] {
        let mut game = tracked_scissors(A, true, false, batch);
        decision(&mut game, &mut vec![]);
        game.choose(0).unwrap();
        let menu = decision(&mut game, &mut vec![]);
        game.choose(menu.options.len() - 1).unwrap(); // select only this card
        let menu = decision(&mut game, &mut vec![]);
        assert_eq!(
            menu.options,
            [Alternative::Branch(0), Alternative::Branch(1)]
        );
        game.choose(0).unwrap(); // meld; Kalashnikov interrupts before the move
        let menu = decision(&mut game, &mut vec![]);
        assert_eq!(menu.player, B);
        assert!(matches!(
            menu.options.as_slice(),
            [
                Alternative::Move {
                    verb: Verb::Score,
                    ..
                },
                Alternative::SelfExecute(_)
            ]
        ));
        let pending = |world: &Game| {
            world
                .frames
                .iter()
                .find_map(|frame| match frame {
                    Frame::Operation {
                        request:
                            Operation::Move {
                                verb: Verb::Meld,
                                target,
                                ..
                            },
                        knowledge: Some(knowledge),
                        ..
                    } => Some(Captured::new(target.card, knowledge.evidence)),
                    _ => None,
                })
                .expect("suspended meld with retained evidence")
        };
        let mut identities = std::collections::BTreeSet::new();
        for seed in 0..64 {
            let mut world = game.clone().determinize(B, seed).unwrap();
            assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
            let item = scissors_slot(&world, 1)[0];
            let operand = pending(&world);
            assert_eq!(
                item, operand,
                "slot and suspended move must be the same reference"
            );
            assert_eq!(scissors_slot(&world, 0), [item]);
            identities.insert(item.inner);
            let nested = world.determinize(B, seed + 70).unwrap();
            assert_eq!(scissors_slot(&nested, 1)[0], pending(&nested));
            world.choose(0).unwrap(); // Kalashnikov actually scores the red top
            let mut events = vec![];
            assert!(matches!(
                settle(&mut world, &mut events),
                Status::Complete(_)
            ));
            assert_eq!(world.position.cards(Zone::Score(B)).len(), 1);
            assert!(events.iter().any(|event| matches!(event, Event::Moved(m) if m.card == item.inner && m.verb == Some(Verb::Meld))));
            assert!(world.position.check_conservation());
        }
        assert_eq!(
            identities.len(),
            3,
            "the private selected face must remain unpinned"
        );
    }
}

#[test]
fn scissors_branch_observation_names_the_owners_current_item_and_hides_it_from_opponent() {
    let mut game = tracked_scissors(A, false, false, false);
    decision(&mut game, &mut vec![]);
    game.choose(0).unwrap();
    decision(&mut game, &mut vec![]);
    game.choose(0).unwrap();
    let menu = decision(&mut game, &mut vec![]);
    assert_eq!(menu.options.len(), 2); // choose the first item to process
    let mut branches = Vec::new();
    for index in 0..2 {
        let mut world = game.clone();
        world.choose(index).unwrap();
        let menu = decision(&mut world, &mut vec![]);
        assert_eq!(
            menu.options,
            [Alternative::Branch(0), Alternative::Branch(1)]
        );
        let item = scissors_slot(&world, 1)[0].inner;
        assert!(
            world.context_refs(A).iter().any(
                |r| r.role == observe::Role::BlockItem && r.sight == observe::Sight::Card(item)
            )
        );
        assert!(
            world
                .context_refs(B)
                .iter()
                .filter(|r| matches!(r.role, observe::Role::Slot | observe::Role::BlockItem))
                .all(|r| r.sight == observe::Sight::Back(Deck::base(1)))
        );
        branches.push(world);
    }
    assert_ne!(
        branches[0].observe(A).unwrap(),
        branches[1].observe(A).unwrap()
    );
    assert_eq!(
        branches[0].observe(B).unwrap(),
        branches[1].observe(B).unwrap()
    );
}

#[test]
fn verified_group_discloses_only_its_committed_hidden_subset() {
    let mut f = Fixture::default();
    for value in 1..=3 {
        for color in [Color::Red, Color::Green, Color::Yellow] {
            f.tech(color, value, Zone::Hand(A), vec![]);
        }
    }
    let source = f.tech(
        Color::Blue,
        5,
        Zone::Board(A, Color::Blue),
        vec![
            crate::program::build::return_(Selector::Hand(Who::Executor))
                .group(2, GroupConstraint::DifferentValues)
                .verify()
                .shortcut(Shortcut::ByOrigin {
                    descending: false,
                    quota: false,
                }),
        ],
    );
    let mut game = Game::seeded(f.position(), Expansions::BASE, 31)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    let mut events = vec![];
    let first = decision(&mut game, &mut events);
    assert_eq!(
        first.options,
        (1..=3)
            .map(|age| Alternative::BlindSelect {
                source: Zone::Hand(A),
                back: Deck::base(age),
            })
            .collect::<Vec<_>>()
    );
    game.choose(0).unwrap();
    let second = decision(&mut game, &mut events);
    assert_eq!(
        second.options,
        (2..=3)
            .map(|age| Alternative::BlindSelect {
                source: Zone::Hand(A),
                back: Deck::base(age),
            })
            .collect::<Vec<_>>()
    );
    assert!(
        events.is_empty(),
        "selecting part of the group neither reveals nor moves anything"
    );
    assert_eq!(game.position.cards(Zone::Hand(A)).len(), 9);

    let mut worlds = vec![game.clone()];
    for seed in 0..16 {
        let sampled = game.determinize(B, seed).unwrap();
        assert_eq!(sampled.observe(B).unwrap(), game.observe(B).unwrap());
        worlds.push(sampled);
    }
    for mut world in worlds {
        world.choose(0).unwrap();
        let chosen: Vec<_> = world
            .frames
            .iter()
            .find_map(|frame| match frame {
                Frame::SelectGroup(group) => {
                    Some(group.selected.iter().map(|held| held.card).collect())
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(chosen.len(), 2);
        assert_ne!(chosen[0], chosen[1]);
        let mut events = vec![];
        let mut completed = false;
        for _ in 0..150 {
            let mut clone = world.clone();
            let mut clone_events = vec![];
            let mut step_events = vec![];
            let status = world.advance(1, &mut step_events).unwrap();
            let cloned_status = clone.advance(1, &mut clone_events).unwrap();
            assert_eq!(status, cloned_status);
            assert_eq!(step_events, clone_events);
            assert_eq!(world, clone);
            for event in &step_events {
                match event {
                    Event::Revealed { card, .. } => assert!(
                        chosen.contains(card),
                        "an eligible but unselected card must not be disclosed"
                    ),
                    Event::Moved(m) if m.verb == Some(Verb::Return) => {
                        assert!(chosen.contains(&m.card))
                    }
                    _ => {}
                }
            }
            events.extend(step_events);
            match status {
                Status::WorkLimit => {}
                Status::Decision(_) => world.choose(0).unwrap(),
                Status::Complete(_) => {
                    completed = true;
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(completed);
        let revealed: std::collections::BTreeSet<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(revealed, chosen.iter().copied().collect());
        for card in chosen {
            let deck = world.position.card(card).origin_deck();
            assert_eq!(world.position.locate(card), Some(Zone::Deck(deck)));
            assert_eq!(
                world.tracker(B).unwrap().belief(Zone::Deck(deck))[0].known(),
                Some(card)
            );
        }
        assert_eq!(world.position.cards(Zone::Hand(A)).len(), 7);
        assert!(
            world
                .tracker(B)
                .unwrap()
                .belief(Zone::Hand(A))
                .iter()
                .all(|slot| slot.known().is_none()),
            "unselected same-back cards remain anonymous"
        );
        assert!(world.position.check_conservation());
    }
}

#[test]
fn verified_group_forecast_orders_stay_blind_until_each_card_is_admitted() {
    let mut f = Fixture::default();
    for value in 1..=3 {
        for color in [Color::Red, Color::Green, Color::Yellow] {
            f.tech(color, value, Zone::Hand(A), vec![]);
        }
    }
    for color in [Color::Red, Color::Green, Color::Yellow, Color::Purple] {
        f.tech(color, 4, Zone::Forecast(A), vec![]);
    }
    let source = f.tech(
        Color::Blue,
        5,
        Zone::Board(A, Color::Blue),
        vec![
            crate::program::build::foreshadow(Selector::Hand(Who::Executor))
                .group(2, GroupConstraint::DifferentValues)
                .verify(),
        ],
    );
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 31)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.controller = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    let mut events = vec![];
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    let order = decision(&mut game, &mut events);
    let expected_options: Vec<_> = (1..=2)
        .map(|age| Alternative::BlindMove {
            verb: Verb::Foreshadow,
            source: Zone::Hand(A),
            back: Deck::base(age),
        })
        .collect();
    assert_eq!(
        order.options, expected_options,
        "both meaningful orders use public backs only"
    );
    assert!(
        events.is_empty(),
        "the group must not expose either face before an individual admission"
    );

    let mut worlds = vec![game.clone()];
    for seed in 0..16 {
        let mut sampled = game.determinize(B, seed).unwrap();
        assert_eq!(sampled.observe(B).unwrap(), game.observe(B).unwrap());
        assert_eq!(
            decision(&mut sampled, &mut vec![]).options,
            expected_options
        );
        worlds.push(sampled);
    }
    for world in worlds {
        let selected: Vec<_> = world
            .frames
            .iter()
            .find_map(|frame| match frame {
                Frame::MoveAll(batch) => batch
                    .frozen
                    .as_ref()
                    .map(|cards| cards.iter().map(|held| held.card).collect()),
                _ => None,
            })
            .unwrap();
        for first in 0..2 {
            let mut branch = world.clone();
            let admitted = *selected
                .iter()
                .find(|card| branch.position.card(**card).value == first as u8 + 1)
                .unwrap();
            let refused = *selected.iter().find(|card| **card != admitted).unwrap();
            branch.choose(first).unwrap();
            let mut events = vec![];
            let mut completed = false;
            for _ in 0..100 {
                let mut cloned = branch.clone();
                let mut clone_events = vec![];
                let mut step_events = vec![];
                let status = branch.advance(1, &mut step_events).unwrap();
                assert_eq!(status, cloned.advance(1, &mut clone_events).unwrap());
                assert_eq!(step_events, clone_events);
                assert_eq!(branch, cloned);
                events.extend(step_events);
                match status {
                    Status::WorkLimit => {}
                    Status::Decision(_) => branch.choose(0).unwrap(),
                    Status::Complete(_) => {
                        completed = true;
                        break;
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(completed);
            let revealed: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed { card, .. } => Some(*card),
                    _ => None,
                })
                .collect();
            assert_eq!(
                revealed,
                [admitted],
                "capacity refusal must never reveal the second selected face"
            );
            assert_eq!(branch.position.locate(admitted), Some(Zone::Forecast(A)));
            assert_eq!(branch.position.locate(refused), Some(Zone::Hand(A)));
            assert_eq!(branch.position.cards(Zone::Forecast(A)).len(), 5);
            assert_eq!(branch.position.cards(Zone::Hand(A)).len(), 8);
            assert!(
                branch
                    .tracker(B)
                    .unwrap()
                    .belief(Zone::Forecast(A))
                    .iter()
                    .any(|slot| slot.known() == Some(admitted))
            );
            assert!(
                branch
                    .tracker(B)
                    .unwrap()
                    .belief(Zone::Hand(A))
                    .iter()
                    .all(|slot| slot.known().is_none()),
                "the refused card and unselected hand remain anonymous"
            );
            assert!(branch.position.check_conservation());
        }
    }
}

#[test]
fn verified_group_same_area_transfers_disclose_nothing() {
    let mut f = Fixture::default();
    for value in 1..=3 {
        for color in [Color::Red, Color::Green, Color::Yellow] {
            f.tech(color, value, Zone::Hand(A), vec![]);
        }
    }
    let source = f.tech(
        Color::Blue,
        5,
        Zone::Board(A, Color::Blue),
        vec![
            crate::program::build::transfer(
                Selector::Hand(Who::Executor),
                Destination::Hand(Who::Executor),
            )
            .group(2, GroupConstraint::DifferentValues)
            .verify(),
        ],
    );
    let mut game = Game::seeded(f.position(), Expansions::BASE, 31)
        .unwrap()
        .with_tracking()
        .unwrap();
    let initial = game.position.clone();
    let mut context = Context::new(A);
    context.controller = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    let mut events = vec![];
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    decision(&mut game, &mut events);
    assert!(events.is_empty());
    let mut worlds = vec![game.clone()];
    for seed in 0..8 {
        let world = game.determinize(B, seed).unwrap();
        assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
        worlds.push(world);
    }
    for mut world in worlds {
        let before = world.position.clone();
        world.choose(0).unwrap();
        let mut events = vec![];
        let mut clone = world.clone();
        let mut cloned_events = vec![];
        let status = settle(&mut world, &mut events);
        assert!(matches!(status, Status::Complete(_)));
        assert_eq!(status, settle(&mut clone, &mut cloned_events));
        assert_eq!(events, cloned_events);
        assert_eq!(world, clone);
        assert!(
            events.is_empty(),
            "same-area transfers are refused before verification and create no move"
        );
        assert_eq!(world.position, before);
        assert!(
            world
                .tracker(B)
                .unwrap()
                .belief(Zone::Hand(A))
                .iter()
                .all(|slot| slot.known().is_none()),
            "neither selected nor unrelated hand cards become known"
        );
    }
    assert_eq!(game.position, initial);
}
