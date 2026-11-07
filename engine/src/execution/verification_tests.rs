use super::*;
use crate::execution::shortcuts::common::*;
use crate::program::build::{self, HAND};
use std::sync::Arc;

#[test]
fn oars_failure_proof_adds_only_reveals_to_seeded_games_without_changing_actions_or_rng() {
    let mut proofs = 0;
    for mode in 0..8 {
        let mut expansions = Expansions::BASE;
        for (bit, expansion) in [
            (1, Expansions::ECHOES),
            (2, Expansions::FIGURES),
            (4, Expansions::CITIES),
        ] {
            if mode & bit != 0 {
                expansions = expansions.union(expansion);
            }
        }
        for seed in 0..16 {
            let mut game = Game::new_game(seed, expansions).unwrap();
            let mut historical = game.clone();
            let cards = Arc::make_mut(&mut historical.position.catalog);
            let oars = cards.iter_mut().find(|card| card.code == "B003").unwrap();
            let Instruction::Move(step) = &mut oars.effects[0].instructions[0] else {
                panic!("Oars starts with its mandatory transfer");
            };
            assert!(step.reveal_if_empty.take().is_some());
            let mut policy = Rng::new(seed ^ 0x0005_EED0_FA11);
            let mut ended = false;
            for _ in 0..20_000 {
                let mut events = Vec::new();
                let mut historical_events = Vec::new();
                let status = game.advance(1 << 20, &mut events).unwrap();
                assert_eq!(
                    historical.advance(1 << 20, &mut historical_events).unwrap(),
                    status,
                    "mode {mode}, seed {seed}: choices/outcome must not change"
                );
                let mut expected = historical_events.iter().peekable();
                for event in &events {
                    if expected.peek().is_some_and(|&old| old == event) {
                        expected.next();
                    } else {
                        let Event::Revealed {
                            card,
                            source,
                            executor,
                            ..
                        } = event
                        else {
                            panic!("mode {mode}, seed {seed}: unexpected new event {event:?}");
                        };
                        assert!(
                            matches!(*source, Zone::Hand(owner) | Zone::Score(owner) if owner == *executor)
                        );
                        assert!(
                            !game
                                .position
                                .card(*card)
                                .symbols
                                .contains(&Symbol::Icon(Icon::Prosperity))
                        );
                        proofs += 1;
                    }
                }
                assert!(
                    expected.next().is_none(),
                    "historical events must all remain in order"
                );
                assert_eq!(game.rng, historical.rng);
                // Compare every state field using the same immutable rules;
                // the sole declaration difference above is the proof itself.
                let mut expected_position = historical.position.clone();
                expected_position.catalog = game.position.catalog.clone();
                assert_eq!(game.position, expected_position);
                match status {
                    Status::Decision(menu) => {
                        let choice = (policy.next_u64() % menu.options.len() as u64) as usize;
                        game.choose(choice).unwrap();
                        historical.choose(choice).unwrap();
                    }
                    Status::Complete(_) => {
                        let actions = game.legal_actions();
                        assert_eq!(historical.legal_actions(), actions);
                        let action = actions[(policy.next_u64() % actions.len() as u64) as usize];
                        game.take_action(action, ActionKind::Regular).unwrap();
                        historical.take_action(action, ActionKind::Regular).unwrap();
                    }
                    Status::Winner(_) | Status::Draw(_) => {
                        ended = true;
                        break;
                    }
                    other => panic!("mode {mode}, seed {seed}: {other:?}"),
                }
            }
            assert!(ended, "mode {mode}, seed {seed}: bounded game must finish");
        }
    }
    assert!(
        proofs > 0,
        "the seeded audit must actually exercise Oars' new proof"
    );
}

fn start(mut f: Fixture, code: &str, effect: u16, specials: &[Special], tracked: bool) -> Game {
    let source = f.add(echoes_card(code).unwrap(), Zone::Junk);
    let mut game = Game::seeded(
        f.position(),
        Expansions::ECHOES
            .union(Expansions::FIGURES)
            .union(Expansions::CITIES),
        17,
    )
    .unwrap();
    for special in game.position.specials().collect::<Vec<_>>() {
        game.position.specials[special.index()] = if specials.contains(&special) {
            SpecialLocation::Available
        } else {
            SpecialLocation::Junk
        };
    }
    if tracked {
        game = game.with_tracking().unwrap();
    }
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}

fn icon(f: &mut Fixture, color: Color, age: u8, zone: Zone, icon: Icon) -> CardId {
    let card = f.tech(color, age, zone, vec![]);
    f.cards[usize::from(card.0)].featured = Some(icon);
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(icon);
    card
}

fn finish(game: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => {
                assert!(game.position.check_conservation());
                return;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("effect did not finish")
}

#[test]
fn bandage_public_verification_allows_the_other_player_to_search() {
    for echo in [false, true] {
        for matching in [false, true] {
            let mut f = Fixture::default();
            let zone = if echo { Zone::Hand(A) } else { Zone::Score(A) };
            let required = if echo { Icon::Health } else { Icon::Efficiency };
            let selected = icon(
                &mut f,
                Color::Yellow,
                4,
                zone,
                if matching { required } else { Icon::Authority },
            );
            let hidden = icon(&mut f, Color::Green, 4, zone, Icon::Authority);
            icon(
                &mut f,
                Color::Purple,
                4,
                Zone::Deck(Deck::base(4)),
                Icon::Authority,
            );
            icon(
                &mut f,
                Color::Blue,
                4,
                Zone::Board(A, Color::Blue),
                Icon::Efficiency,
            );
            let mut source = echoes_card("E076").unwrap();
            let program = if echo {
                &mut source.echo.as_mut().unwrap().instructions
            } else {
                &mut source.effects[0].instructions
            };
            // A subsequent foreign decision must not inherit stale private flow.
            program.push(build::choose(
                Who::Opponent,
                Choice::Value(ValueChoices::Boxed(&[1, 2])),
            ));
            let source = f.add(source, Zone::Junk);
            let mut game = Game::seeded(f.position(), Expansions::ECHOES, 17)
                .unwrap()
                .with_tracking()
                .unwrap();
            let mut context = Context::new(A);
            context.demander = B;
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: if echo { Card::ECHO } else { 0 },
                    boxed: BoxedValues::ZERO,
                },
                context,
            )
            .unwrap();
            let mut events = vec![];
            loop {
                let menu = decision(&mut game, &mut events);
                if menu.player == B {
                    assert_eq!(
                        menu.options,
                        vec![Alternative::Value(1), Alternative::Value(2)]
                    );
                    break;
                }
                game.choose(0).unwrap();
            }
            assert!(
                events.iter().any(|event| matches!(event,
                Event::Revealed { card, .. } if *card == selected))
                    || (echo && matching)
            );
            assert_eq!(
                events.iter().any(|event| matches!(event,
                Event::Revealed { card, .. } if *card == hidden)),
                !matching
            );
            let before = game.clone();
            let mut sampled_piles = std::collections::BTreeSet::new();
            for seed in 0..8 {
                let world = game.determinize(B, seed).unwrap_or_else(|error| {
                    panic!("Bandage echo={echo} matching={matching}: {error:?}")
                });
                assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
                let mut pile = world.position.cards(zone).to_vec();
                pile.sort_unstable();
                sampled_piles.insert(pile);
            }
            if matching {
                assert!(
                    sampled_piles.len() > 1,
                    "a public witness must not pin the unchosen hidden card"
                );
            } else {
                assert_eq!(
                    sampled_piles.len(),
                    1,
                    "the failure proof revealed the whole pile"
                );
            }
            let result = game
                .search(crate::SearchConfig {
                    iterations: 4,
                    rollout_actions: 0,
                    seed: 3,
                    ..crate::SearchConfig::default()
                })
                .unwrap();
            assert!(matches!(result.step, crate::Step::Choice(0 | 1)));
            assert_eq!(game, before);
        }
    }
}

#[test]
fn empty_selection_proof_stays_private_until_complete_and_records_no_move() {
    const EMPTY: Slot = Slot::effect(0);
    const EFFICIENT: Selector =
        build::filtered(&HAND, &[Filter::Has(Test::Icon(Icon::Efficiency))]);
    let mut f = Fixture::default();
    let hidden = [Color::Green, Color::Yellow]
        .map(|color| icon(&mut f, color, 4, Zone::Hand(A), Icon::Authority));
    icon(
        &mut f,
        Color::Purple,
        4,
        Zone::Deck(Deck::base(4)),
        Icon::Efficiency,
    );
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Junk,
        vec![
            build::return_(EFFICIENT).reveal_if_empty(&HAND).keep(EMPTY),
            build::choose(Who::Opponent, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
            build::if_(&Cond::Not(&Cond::Empty(EMPTY)), 1),
            build::draw(4),
        ],
    );
    let mut game = Game::seeded(f.position(), Expansions::BASE, 3)
        .unwrap()
        .with_tracking()
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
    let mut events = vec![];
    let mut guarded = 0;
    for _ in 0..100 {
        let mut clone = game.clone();
        let mut clone_events = vec![];
        let mut step_events = vec![];
        let status = game.advance(1, &mut step_events).unwrap();
        assert_eq!(clone.advance(1, &mut clone_events).unwrap(), status);
        assert_eq!(clone_events, step_events);
        assert_eq!(clone, game);
        events.extend(step_events);
        if game.frames.iter().any(|frame| {
            matches!(frame,
            Frame::RevealAll(packet) if packet.empty_selection.is_some())
        }) {
            guarded += 1;
            let before = game.clone();
            for result in [
                game.determinize(B, 7).map(|_| ()),
                game.sample_world(B, 7).map(|_| ()),
            ] {
                assert!(
                    matches!(result, Err(Error::Busy)),
                    "unfinished work is not a search root"
                );
            }
            for frame in &game.frames {
                if let Frame::RevealAll(packet) = frame {
                    let mut stable = vec![false; game.position.catalog.len()];
                    stable[usize::from(source.0)] = true;
                    let scan = tracking::remap::Remap::new(
                        tracking::remap::Mode::Scan,
                        B,
                        &game.position,
                        stable,
                    );
                    packet.clone().remap(&scan);
                    assert_eq!(
                        scan.opaque.get(),
                        Some("a retained scalar depends on private information")
                    );
                }
            }
            assert_eq!(
                game.context_records(B)[0].kind,
                crate::observation::context::RecordKind::Opaque
            );
            assert_eq!(game, before);
        }
        if let Status::Decision(menu) = status {
            assert_eq!(menu.player, B);
            assert_eq!(
                menu.options,
                vec![Alternative::Value(1), Alternative::Value(2)]
            );
            assert!(guarded > 1);
            for card in hidden {
                assert!(events.iter().any(|event| matches!(event,
                    Event::Revealed { card: revealed, .. } if *revealed == card)));
                assert_eq!(game.position.locate(card), Some(Zone::Hand(A)));
            }
            let sampled = game.determinize(B, 7).unwrap();
            assert_eq!(sampled.observe(B).unwrap(), game.observe(B).unwrap());
            game.choose(0).unwrap();
            finish(&mut game, &mut events);
            assert_eq!(game.position.cards(Zone::Hand(A)), hidden);
            assert!(
                !events.iter().any(|event| matches!(event, Event::Moved(_))),
                "proof reveals cannot populate the successful-move slot"
            );
            return;
        }
    }
    panic!("proof did not reach the foreign decision");
}

#[test]
fn verification_admission_precedes_reveal_and_preserves_layout_budgets() {
    let mut f = Fixture::default();
    let selected = f.add(base_card("B001").unwrap(), Zone::Hand(A));
    f.add(base_card("B002").unwrap(), Zone::Hand(A));
    for _ in 0..5 {
        f.tech(Color::Yellow, 1, Zone::Forecast(A), vec![]);
    }
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Junk,
        vec![build::foreshadow(HAND).verify()],
    );
    let mut game = Game::new(f.position(), Expansions::ECHOES)
        .unwrap()
        .with_tracking()
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
    let mut events = vec![];
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    finish(&mut game, &mut events);
    assert!(
        events.is_empty(),
        "a full Forecast refuses admission before revealing anything"
    );
    assert_eq!(game.position.locate(selected), Some(Zone::Hand(A)));
    assert!(
        game.tracker(B)
            .unwrap()
            .belief(Zone::Hand(A))
            .iter()
            .all(|slot| slot.known().is_none())
    );
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn digital_pet_live_returns_and_final_proof_include_cards_added_by_peter_the_great() {
    for color in [Color::Red, Color::Green] {
        let mut f = Fixture::default();
        let incoming = f.tech(Color::Green, 4, Zone::Board(A, Color::Green), vec![]);
        let figure = figure_card(Figure::PeterTheGreat.code()).unwrap();
        let figure_color = figure.color;
        f.add(figure, Zone::Board(A, figure_color));
        let original = f.tech(color, 2, Zone::Score(A), vec![]);
        f.tech(color, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = start(f, "E108", 0, &[], true);
        let mut events = vec![];
        finish(&mut game, &mut events);
        assert!(events.iter().any(|e| matches!(e, Event::Moved(m) if m.card == incoming && m.verb == Some(Verb::Score) && m.inside_karma)));
        assert_eq!(
            game.position.locate(original),
            Some(Zone::Deck(Deck::base(2)))
        );
        if color == Color::Green {
            assert_eq!(
                game.position.locate(incoming),
                Some(Zone::Deck(Deck::base(4))),
                "new matching score cards join the live return batch"
            );
        } else {
            assert_eq!(game.position.locate(incoming), Some(Zone::Score(A)));
            assert!(events.iter().any(|e| matches!(e, Event::Revealed { card, source: Zone::Score(A), .. } if *card == incoming)), "final completeness proof includes the new nonmatching score card");
        }
    }
}

#[test]
fn algocracy_freezes_the_leader_and_recorded_packet_before_transfer_reactions() {
    let mut f = Fixture::default();
    let figure = figure_card(Figure::AlfredNobel.code()).unwrap();
    let figure_color = figure.color;
    f.add(figure, Zone::Board(B, figure_color));
    let leader = icon(
        &mut f,
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        Icon::Health,
    );
    let other = f.tech(Color::Purple, 2, Zone::Board(B, Color::Purple), vec![]);
    let moving: Vec<_> = [Zone::Hand(B), Zone::Hand(B), Zone::Score(A), Zone::Score(B)]
        .into_iter()
        .map(|z| icon(&mut f, Color::Green, 3, z, Icon::Health))
        .collect();
    let new_match = icon(
        &mut f,
        Color::Yellow,
        3,
        Zone::Deck(Deck::base(3)),
        Icon::Health,
    );
    f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = start(f, "E110", 0, &[], true);
    let mut events = vec![];
    let menu = decision(&mut game, &mut events);
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Icon(Icon::Health))
            .unwrap(),
    )
    .unwrap();
    let menu = decision(&mut game, &mut events);
    assert!(
        menu.options.iter().all(|o| matches!(
            o,
            Alternative::Move {
                target: TargetDescriptor::Known { .. },
                ..
            }
        )),
        "revealed packet choices name their identities"
    );
    assert_eq!(
        menu.options.len(),
        4,
        "Nobel must guard the meaningful transfer order"
    );
    for seed in 0..32 {
        let world = game.determinize(A, seed).unwrap();
        assert_eq!(world.observe(A).unwrap(), game.observe(A).unwrap());
        let observe::PromptView::Decision(sampled) = world.prompt_view() else {
            panic!("sampled named packet menu")
        };
        assert_eq!(
            sampled.options, menu.options,
            "sampled physical order cannot relabel named packet choices"
        );
    }
    game.choose(0).unwrap();
    while !events
        .iter()
        .any(|e| matches!(e, Event::Moved(m) if m.inside_karma && m.verb == Some(Verb::Junk)))
    {
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    // At the First boundary, model a board-changing child and a newly
    // eligible arrival. Original target and cohort remain frozen.
    game.position.players[A.index()].stacks[Color::Blue as usize].clear();
    game.position.players[B.index()].stacks[Color::Blue as usize].push(leader);
    game.position
        .zone_mut(Zone::Deck(Deck::base(3)))
        .retain(|&card| card != new_match);
    game.position.players[B.index()].score.push(new_match);
    // A committed packet member which left the named source zones is not
    // taken back off a board just because the slot still names it.
    game.position.players[B.index()]
        .score
        .retain(|&card| card != moving[3]);
    game.position.players[B.index()].stacks[Color::Green as usize].push(moving[3]);
    game.position.refresh_boards();
    game.knowledge = None; // The synthetic boundary has no observation history.
    finish(&mut game, &mut events);
    assert_eq!(game.position.locate(moving[0]), Some(Zone::Hand(A)));
    assert_eq!(game.position.locate(moving[1]), Some(Zone::Hand(A)));
    assert_eq!(game.position.locate(moving[2]), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(moving[3]),
        Some(Zone::Board(B, Color::Green))
    );
    assert_eq!(game.position.locate(new_match), Some(Zone::Score(B)));
    assert_eq!(
        game.position.locate(other),
        Some(Zone::Board(B, Color::Purple))
    );
}

#[test]
fn parachute_foreseen_junks_covered_cards_on_both_boards() {
    for promoted in [false, true] {
        let mut f = Fixture::default();
        let boards: Vec<_> = [A, B]
            .into_iter()
            .flat_map(|p| [Color::Blue, Color::Green].into_iter().map(move |c| (p, c)))
            .map(|(p, c)| f.tech(c, 1, Zone::Board(p, c), vec![]))
            .collect();
        let buried = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        let parachute = f.add(
            echoes_card("E077").unwrap(),
            if promoted {
                Zone::Forecast(A)
            } else {
                Zone::Junk
            },
        );
        let melding = f.tech(Color::Purple, 8, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        if promoted {
            game.take_action(Action::Meld { card: melding }, ActionKind::Regular)
                .unwrap();
        } else {
            game.start(
                Operation::ExecuteEffect {
                    card: parachute,
                    effect: 1,
                    boxed: BoxedValues::ZERO,
                },
                Context::new(A),
            )
            .unwrap();
        }
        finish(&mut game, &mut vec![]);
        for card in boards.into_iter().chain([buried]) {
            assert_eq!(game.position.locate(card) == Some(Zone::Junk), promoted);
        }
        if promoted {
            assert_eq!(game.position.locate(parachute), Some(Zone::Junk));
        }
    }
}

fn outcomes(
    mut game: Game,
    mut events: Vec<Event>,
    depth: usize,
) -> Vec<(Position, Rng, Vec<String>)> {
    assert!(depth < 25);
    match settle(&mut game, &mut events) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut branch = game.clone();
                branch.choose(index).unwrap();
                outcomes(branch, events.clone(), depth + 1)
            })
            .collect(),
        Status::Complete(_) => {
            assert!(game.position.check_conservation());
            game.position.sort_unordered_piles();
            let mut events: Vec<_> = events.into_iter().map(|e| format!("{e:?}")).collect();
            events.sort();
            vec![(game.position, game.rng, events)]
        }
        other => panic!("unexpected {other:?}"),
    }
}

fn oracle_fixture(code: &str, reacting: bool) -> Game {
    let mut f = Fixture::default();
    match code {
        "E066" => {
            for color in [Color::Blue, Color::Green] {
                let c = f.tech(color, 2, Zone::Board(A, color), vec![]);
                f.cards[usize::from(c.0)].symbols[0] = Symbol::Bonus(2);
            }
            for _ in 0..2 {
                f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
            }
            f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
            f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
        }
        "E076" => {
            for _ in 0..2 {
                icon(&mut f, Color::Red, 3, Zone::Score(A), Icon::Efficiency);
            }
            icon(
                &mut f,
                Color::Blue,
                4,
                Zone::Board(A, Color::Blue),
                Icon::Efficiency,
            );
            for _ in 0..3 {
                f.tech(Color::Green, 1, Zone::Achievements(A), vec![]);
            }
            for _ in 0..2 {
                f.tech(Color::Purple, 2, Zone::AvailableAchievements, vec![]);
            }
        }
        "E077" => {
            for color in [Color::Blue, Color::Green] {
                f.tech(color, 2, Zone::Hand(A), vec![]);
            }
            icon(&mut f, Color::Red, 2, Zone::Hand(A), Icon::Efficiency);
            f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
        }
        "E108" => {
            f.tech(Color::Red, 2, Zone::Score(A), vec![]);
            f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
            f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
            f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
        }
        "E110" => {
            icon(
                &mut f,
                Color::Blue,
                1,
                Zone::Board(A, Color::Blue),
                Icon::Health,
            );
            icon(&mut f, Color::Green, 2, Zone::Hand(B), Icon::Health);
            icon(&mut f, Color::Green, 2, Zone::Score(A), Icon::Health);
            f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
        }
        _ => unreachable!(),
    }
    if reacting {
        let figure = if matches!(code, "E076" | "E108") {
            Figure::JamesClerkMaxwell
        } else {
            Figure::AlfredNobel
        };
        let figure = figure_card(figure.code()).unwrap();
        let color = figure.color;
        f.add(figure, Zone::Board(B, color));
        for _ in 0..3 {
            f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
        }
    }
    start(f, code, 0, &[Special::Empire], false)
}

#[test]
fn verification_card_shortcuts_match_every_manual_path_with_reacting_figures() {
    for code in ["E066", "E076", "E077", "E108", "E110"] {
        for reacting in [false, true] {
            let reduced = oracle_fixture(code, reacting);
            let mut manual = reduced.clone();
            manual.shortcuts_disabled = true;
            let reduced = outcomes(reduced, vec![], 0);
            let manual = outcomes(manual, vec![], 0);
            assert!(
                manual.iter().all(|o| reduced.contains(o)),
                "{code} {reacting}: missing outcome"
            );
            assert!(
                reduced.iter().all(|o| manual.contains(o)),
                "{code} {reacting}: extra outcome"
            );
            if reacting {
                assert!(
                    manual.iter().any(|(_, _, events)| events
                        .iter()
                        .any(|e| e.contains("inside_karma: true"))),
                    "{code}: Figure must really react"
                );
            } else {
                assert!(reduced.len() < manual.len(), "{code}: expected a reduction");
            }
        }
    }
}

#[test]
fn mixed_junk_quota_keeps_alias_order_when_junking_cai_lun_can_remove_other_targets() {
    let mut f = Fixture::default();
    let figure = figure_card(Figure::CaiLun.code()).unwrap();
    let color = figure.color;
    let cai = f.add(figure, Zone::Board(A, color));
    let other_color = Color::ALL.into_iter().find(|&c| c != color).unwrap();
    let alias = f.tech(other_color, 2, Zone::Board(A, other_color), vec![]);
    let supply = f.tech(Color::Red, 1, Zone::AvailableAchievements, vec![]);
    let source = f.tech(
        Color::Blue,
        1,
        Zone::Junk,
        vec![Instruction::Special(SpecialStep::JunkAvailable {
            count: &Expr::Const(3),
            shortcut: Some(Shortcut::AnyOrder),
        })],
    );
    let mut game = Game::new(f.position(), Expansions::ECHOES.union(Expansions::FIGURES)).unwrap();
    for special in game.position.specials().collect::<Vec<_>>() {
        game.position.specials[special.index()] = SpecialLocation::Junk;
    }
    let mut context = Context::new(A);
    context.set_suppress_karma(true); // Each remains active.
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    let menu = decision(&mut game, &mut vec![]);
    assert_eq!(
        menu.options.len(),
        3,
        "the full quota cannot erase changing alias availability"
    );
    assert!(menu.options.iter().any(|o|matches!(o,Alternative::Move {target:TargetDescriptor::Known {card,..},..} if *card==cai)));
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let reduced = outcomes(game, vec![], 0);
    let manual = outcomes(manual, vec![], 0);
    assert_eq!(reduced.len(), manual.len());
    assert!(
        manual.iter().all(|o| reduced.contains(o)) && reduced.iter().all(|o| manual.contains(o))
    );
    assert!(
        reduced
            .iter()
            .any(|(p, _, _)| p.locate(alias) == Some(Zone::Board(A, other_color)))
    );
    assert!(
        reduced
            .iter()
            .any(|(p, _, _)| p.locate(alias) == Some(Zone::Junk))
    );
    assert!(
        reduced
            .iter()
            .all(|(p, _, _)| p.locate(supply) == Some(Zone::Junk))
    );
}

#[test]
fn bandage_verification_witness_allows_search_before_first_reactions() {
    let mut f = Fixture::default();
    let mut selected = base_card("B001").unwrap();
    selected.symbols[0] = Symbol::Icon(Icon::Efficiency);
    let selected = f.add(selected, Zone::Score(A));
    let mut other = base_card("B002").unwrap();
    other.symbols[0] = Symbol::Icon(Icon::Efficiency);
    f.add(other, Zone::Score(A));
    for (figure, owner) in [(Figure::JamesClerkMaxwell, B), (Figure::PeterTheGreat, A)] {
        let card = figure_card(figure.code()).unwrap();
        let color = card.color;
        f.add(card, Zone::Board(owner, color));
    }
    f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = start(f, "E076", 0, &[], true);
    game.position.turn_owner = B;
    let mut events = vec![];
    let menu = decision(&mut game, &mut events);
    game.choose(menu.options.iter().position(|o|matches!(o,Alternative::Move {target:TargetDescriptor::Known {card,..},..} if *card==selected)).unwrap()).unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B);
    assert_eq!(menu.options.len(), 2, "both First reactions are offered");
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Revealed {card,..} if *card==selected))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.card==selected))
    );
    let pending = |world: &Game| {
        world
            .frames
            .iter()
            .find_map(|frame| match frame {
                Frame::Operation {
                    request:
                        Operation::Move {
                            verb: Verb::Return,
                            target,
                            ..
                        },
                    knowledge: Some(known),
                    ..
                } => Some((target.card, known.evidence)),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(pending(&game).0, selected);
    assert!(pending(&game).1.known(B));
    // The revealed return is public proof that an eligible card existed.
    // B can search the reaction order without learning the unchosen score card.
    let before = game.clone();
    for seed in 0..4 {
        let world = game.determinize(B, seed).unwrap();
        assert_eq!(pending(&world).0, selected);
        assert_eq!(world.observe(B).unwrap(), game.observe(B).unwrap());
        assert_eq!(
            pending(&game.sample_world(B, seed).unwrap().world).0,
            selected
        );
    }
    let result = game
        .search(crate::SearchConfig {
            iterations: 4,
            rollout_actions: 0,
            seed: 3,
            ..crate::SearchConfig::default()
        })
        .unwrap();
    assert!(matches!(result.step, crate::Step::Choice(0 | 1)));
    assert_eq!(game, before);
    game.choose(
        menu.options
            .iter()
            .position(|o| {
                matches!(
                    o,
                    Alternative::Karma {
                        figure: Figure::JamesClerkMaxwell,
                        ..
                    }
                )
            })
            .unwrap(),
    )
    .unwrap();
    finish(&mut game, &mut events);
    assert_eq!(
        game.position.locate(selected),
        Some(Zone::Deck(Deck::base(1)))
    );
    assert_eq!(
        game.tracker(B).unwrap().belief(Zone::Deck(Deck::base(1)))[0].known(),
        Some(selected)
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Moved(m) if m.inside_karma && m.verb.is_none()))
    );
}

#[test]
fn bandage_verifies_only_uninspectable_logical_score_members_before_choosing() {
    assert!(!Test::Inspectable(Who::Executor).reads_front());
    for wernher in [false, true] {
        let mut f = Fixture::default();
        let own = icon(&mut f, Color::Green, 2, Zone::Score(A), Icon::Efficiency);
        let other = icon(&mut f, Color::Blue, 2, Zone::Score(A), Icon::Health);
        let junk = icon(&mut f, Color::Red, 2, Zone::Junk, Icon::Efficiency);
        let junk_other = icon(&mut f, Color::Yellow, 2, Zone::Junk, Icon::Efficiency);
        let private = icon(&mut f, Color::Purple, 2, Zone::Hand(B), Icon::Efficiency);
        if wernher {
            let figure = figure_card(Figure::WernherVonBraun.code()).unwrap();
            let color = figure.color;
            f.add(figure, Zone::Board(A, color));
        }
        let mut game = start(f, "E076", 0, &[], true);
        let mut events = vec![];
        if wernher {
            let menu = decision(&mut game, &mut events);
            for card in [own, junk, junk_other] {
                assert!(
                    menu.options.iter().any(|o| matches!(o, Alternative::Move {
                    target: TargetDescriptor::Known { card: found, .. }, ..
                } if *found == card)),
                    "every inspectable or explicitly revealed member is named"
                );
            }
            for seed in 0..32 {
                let mut world = game.determinize(A, seed).unwrap();
                let sampled = decision(&mut world, &mut vec![]);
                assert_eq!(sampled.options, menu.options);
                assert_eq!(world.observe(A).unwrap(), game.observe(A).unwrap());
            }
            game.choose(
                menu.options
                    .iter()
                    .position(|o| {
                        matches!(o, Alternative::Move {
                target: TargetDescriptor::Known { card, .. }, ..
            } if *card == junk)
                    })
                    .unwrap(),
            )
            .unwrap();
        }
        finish(&mut game, &mut events);
        let revealed = |card| {
            events
                .iter()
                .any(|e| matches!(e, Event::Revealed { card: found, .. } if *found == card))
        };
        // Wernher's owner may look at the junk, so no proof precedes the
        // choice; only the returned junk card is verified.
        assert_eq!(revealed(junk), wernher);
        assert!(!revealed(junk_other), "an unchosen junk card stays hidden");
        assert!(
            !revealed(other),
            "an ordinary unchosen score card stays hidden"
        );
        assert!(
            !revealed(private),
            "unrelated opponent cards are never inspected"
        );
        assert_eq!(
            revealed(own),
            !wernher,
            "only the selected ordinary score card is a witness"
        );
        assert_eq!(
            game.position.locate(if wernher { junk } else { own }),
            Some(Zone::Deck(Deck::base(2)))
        );
    }
}

#[test]
fn digital_pet_retains_both_same_deck_orders_for_revealed_score_aliases() {
    let mut f = Fixture::default();
    let first = f.add(base_card("B001").unwrap(), Zone::Junk);
    let second = f.add(base_card("B002").unwrap(), Zone::Junk);
    let own = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    let figure = figure_card(Figure::WernherVonBraun.code()).unwrap();
    let color = figure.color;
    f.add(figure, Zone::Board(A, color));
    f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = start(f, "E108", 0, &[], true);
    let mut events = vec![];
    let menu = decision(&mut game, &mut events);
    for card in [first, second, own] {
        assert!(
            menu.options.iter().any(|o| matches!(o, Alternative::Move {
            target: TargetDescriptor::Known { card: found, .. }, ..
        } if *found == card)),
            "the live return menu preserves each known member"
        );
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == first))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == second))
    );
    let original = game.clone();
    let mut orders = vec![];
    for selected in [first, second] {
        let mut branch = original.clone();
        let mut events = events.clone();
        branch
            .choose(
                menu.options
                    .iter()
                    .position(|o| {
                        matches!(o, Alternative::Move {
            target: TargetDescriptor::Known { card, .. }, ..
        } if *card == selected)
                    })
                    .unwrap(),
            )
            .unwrap();
        finish(&mut branch, &mut events);
        let order = branch.position.cards(Zone::Deck(Deck::base(1))).to_vec();
        assert_eq!(order.len(), 2);
        assert_eq!(
            order[1], selected,
            "the first return lies above the later bottom return"
        );
        assert_eq!(branch.position.locate(own), Some(Zone::Deck(Deck::base(3))));
        orders.push(order);
    }
    assert_ne!(
        orders[0], orders[1],
        "both same-deck orders remain reachable"
    );
    let mut manual = original.clone();
    manual.shortcuts_disabled = true;
    let reduced = outcomes(original, vec![], 0);
    let manual = outcomes(manual, vec![], 0);
    assert!(
        manual.iter().all(|o| reduced.contains(o)) && reduced.iter().all(|o| manual.contains(o))
    );
}

#[test]
fn digital_pet_new_junk_preserves_order_choices_among_already_revealed_members() {
    for incoming_color in [Color::Red, Color::Green] {
        let mut f = Fixture::default();
        let first = f.add(base_card("B001").unwrap(), Zone::Junk);
        let second = f.add(base_card("B002").unwrap(), Zone::Junk);
        let initial = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
        let incoming = f.tech(incoming_color, 1, Zone::Deck(Deck::base(1)), vec![]);
        let figure = figure_card(Figure::WernherVonBraun.code()).unwrap();
        let color = figure.color;
        f.add(figure, Zone::Board(A, color));
        f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
        let reactor = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
        let reactor_color = reactor.color;
        f.add(reactor, Zone::Board(B, reactor_color));
        for _ in 0..5 {
            f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
        }
        let mut game = start(f, "E108", 0, &[], true);
        let mut events = vec![];
        let menu = decision(&mut game, &mut events);
        game.choose(
            menu.options
                .iter()
                .position(|o| {
                    matches!(o, Alternative::Move {
            target: TargetDescriptor::Known { card, .. }, ..
        } if *card == initial)
                })
                .unwrap(),
        )
        .unwrap();
        // A later logical score arrival: only the initial reveal packet is
        // named. Model a reaction boundary without giving the arrival a face.
        game.position.zone_mut(Zone::Deck(Deck::base(1))).clear();
        game.position.zone_mut(Zone::Junk).push(incoming);
        game.position.refresh_boards();
        game.knowledge = None;
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options.len(),
            if incoming_color == Color::Red { 3 } else { 2 },
            "new matching members join every already-known order choice"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Moved(m) if m.inside_karma && m.verb.is_none())),
            "Maxwell actually reacts before the next live selection"
        );
        for card in [first, second] {
            assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move {
                target: TargetDescriptor::Known { card: found, .. }, ..
            } if *found == card)));
        }
        assert_eq!(game.position.locate(incoming), Some(Zone::Junk));
        let performed = game
            .frames
            .iter()
            .find_map(|frame| match frame {
                Frame::MoveAll(batch) => Some(batch.outcome.performed),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            performed, 1,
            "verification disclosures are not performed original returns"
        );
        if incoming_color == Color::Red {
            assert!(menu.options.iter().any(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == incoming)));
        }
        let mut manual = game.clone();
        manual.shortcuts_disabled = true;
        let reduced = outcomes(game.clone(), vec![], 0);
        let manual = outcomes(manual, vec![], 0);
        assert!(
            manual.iter().all(|o| reduced.contains(o))
                && reduced.iter().all(|o| manual.contains(o))
        );
        let orders: std::collections::BTreeSet<_> = reduced
            .iter()
            .map(|(position, _, _)| position.cards(Zone::Deck(Deck::base(1))).to_vec())
            .collect();
        assert_eq!(
            orders.len(),
            if incoming_color == Color::Red { 6 } else { 2 },
            "every interleaving of the new witness with the old same-back members remains reachable"
        );
        game.choose(0).unwrap();
        finish(&mut game, &mut events);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Revealed { card, .. } if *card == incoming)),
            "new matching cards get a witness, new nonmatches a final completeness proof"
        );
    }
}

#[test]
fn verified_batch_strengthens_anonymous_retained_reference_without_recapturing_it() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let selected = f.add(base_card("B001").unwrap(), Zone::AvailableAchievements);
        f.add(base_card("B002").unwrap(), Zone::Junk);
        let source = f.tech(
            Color::Blue,
            2,
            Zone::Board(A, Color::Blue),
            vec![
                build::bind(Selector::AvailableAchievements),
                build::return_(Selector::Reg(Reg::Bound)).all().verify(),
            ],
        );
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = vec![];
        for _ in 0..40 {
            let _ = game.advance(1, &mut events).unwrap();
            if game
                .frames
                .iter()
                .any(|f| matches!(f, Frame::MoveAll(batch) if batch.proving))
            {
                break;
            }
        }
        let operands = |game: &Game| {
            let bound = game
                .frames
                .iter()
                .find_map(|f| match f {
                    Frame::Program {
                        retained: Some(bound),
                        ..
                    } => Some(*bound),
                    _ => None,
                })
                .unwrap();
            let (target, known) = game
                .frames
                .iter()
                .find_map(|f| match f {
                    Frame::Operation {
                        request: Operation::Reveal { target },
                        knowledge: Some(known),
                        ..
                    } => Some((*target, *known)),
                    _ => None,
                })
                .unwrap();
            assert_eq!(bound.inner, target.card);
            assert_eq!(
                bound.evidence.pick(),
                known.evidence.pick(),
                "the disclosure names the original retained pick"
            );
            target.card
        };
        assert_eq!(operands(&game), selected);
        if tracked {
            // A harmless synthetic choice makes the pre-reveal boundary a
            // supported sampling root, without changing either operand.
            game.frames.push(Frame::Choose {
                player: A,
                options: DecisionOptions::new(
                    vec![Alternative::Colors(1), Alternative::Colors(2)],
                    Context::new(A),
                ),
                tasks: vec![Task::Decline, Task::Decline],
            });
            for seed in 0..16 {
                let mut world = game.determinize(A, seed).unwrap();
                let sampled = operands(&world);
                world.frames.pop();
                let mut sampled_events = vec![];
                finish(&mut world, &mut sampled_events);
                assert!(sampled_events.iter().any(|e| matches!(e, Event::Moved(m) if m.card == sampled && m.verb == Some(Verb::Return))));
                assert_eq!(
                    world.position.locate(sampled),
                    Some(Zone::Deck(Deck::base(1)))
                );
            }
            game.frames.pop();
        }
        finish(&mut game, &mut events);
        assert_eq!(
            game.position.locate(selected),
            Some(Zone::Deck(Deck::base(1)))
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Return)))
                .count(),
            1
        );
    }
}

#[test]
fn verified_per_value_returns_reveal_only_the_admitted_quota_selection() {
    for (quota, per_color) in [(false, false), (true, false), (true, true)] {
        let mut f = Fixture::default();
        let first = f.add(base_card("B001").unwrap(), Zone::Junk);
        let second = f.add(base_card("B002").unwrap(), Zone::Junk);
        let excluded = f.tech(Color::Red, 2, Zone::Junk, vec![]);
        let figure = figure_card(Figure::WernherVonBraun.code()).unwrap();
        let color = figure.color;
        f.add(figure, Zone::Board(A, color));
        if quota {
            let bonus = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
            f.cards[usize::from(bonus.0)].symbols[0] = Symbol::Bonus(1);
        }
        let returns = build::return_(Selector::Score(Who::Executor));
        let admitted = if per_color {
            returns.per_color(None)
        } else {
            returns.per_value(&PerValue {
                source: ValueSource::Bonuses(Bonuses {
                    on: Who::Executor,
                    only: BonusValues::All,
                }),
                multiplicity: Multiplicity::Distinct,
            })
        };
        let source = f.tech(
            Color::Blue,
            3,
            Zone::Board(B, Color::Blue),
            vec![admitted.verify()],
        );
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, 7)
            .unwrap()
            .with_tracking()
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
        let mut events = vec![];
        finish(&mut game, &mut events);
        let revealed: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(revealed.len(), usize::from(quota));
        assert!(
            !revealed.contains(&excluded),
            "a zero cap grants no disclosure"
        );
        assert_eq!(game.position.locate(excluded), Some(Zone::Junk));
        if quota {
            assert!([first, second].contains(&revealed[0]));
            assert_eq!(
                game.position.locate(revealed[0]),
                Some(Zone::Deck(Deck::base(1)))
            );
            let other = if revealed[0] == first { second } else { first };
            assert_eq!(game.position.locate(other), Some(Zone::Junk));
        } else {
            assert!(events.is_empty());
            assert_eq!(game.position.locate(first), Some(Zone::Junk));
            assert_eq!(game.position.locate(second), Some(Zone::Junk));
        }
    }
}
