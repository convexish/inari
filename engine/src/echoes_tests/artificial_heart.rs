//! Artificial Heart's live eligibility, Figure composition, and private claims.
use super::*;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn heart(f: &mut Fixture, owner: Player, foreseen: bool) -> CardId {
    f.add(
        echoes_card("E102").unwrap(),
        if foreseen {
            Zone::Forecast(owner)
        } else {
            Zone::Board(owner, Color::Blue)
        },
    )
}

fn card(f: &mut Fixture, value: u8, zone: Zone) -> CardId {
    f.tech(Color::Purple, value, zone, Vec::new())
}

fn start(game: &mut Game, source: CardId, executor: Player) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(executor),
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        let mut cloned = game.clone();
        let mut next = Vec::new();
        let status = game.advance(1000, &mut next).unwrap();
        let mut expected = Vec::new();
        assert_eq!(cloned.advance(1000, &mut expected).unwrap(), status);
        assert_eq!(cloned, *game);
        assert_eq!(expected, next);
        events.extend(next);
        if let Status::Decision(_) = status {
            game.choose(0).unwrap();
        } else {
            assert!(game.position.check_conservation());
            for player in Player::ALL {
                if let Some(tracker) = game.tracker(player) {
                    assert!(tracker.check_truth(&game.position));
                }
            }
            return status;
        }
    }
    panic!("claim failed to finish");
}

fn choose_age(game: &mut Game, events: &mut Vec<Event>, age: u8) {
    let menu = decision(game, events);
    game.choose(
        menu.options
            .iter()
            .position(|option| {
                matches!(option,
        Alternative::BlindMove { verb: Verb::Achieve, back, .. } if back.age == age)
            })
            .unwrap(),
    )
    .unwrap();
}

#[test]
fn artificial_heart_doubles_its_own_bonus_and_makes_one_non_foreseen_claim() {
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
    let mut f = Fixture::default();
    let source = heart(&mut f, A, false);
    let three = card(&mut f, 3, Zone::AvailableAchievements);
    let four = card(&mut f, 4, Zone::AvailableAchievements);
    let five = card(&mut f, 5, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
    assert_eq!(game.position.score(A), 11);
    start(&mut game, source, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(
        menu.options.as_slice(),
        [3, 4].map(|age| Alternative::BlindMove {
            verb: Verb::Achieve,
            source: Zone::AvailableAchievements,
            back: Deck::base(age)
        })
    );
    game.choose(1).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Achievements(A)), [four]);
    assert_eq!(
        game.position.cards(Zone::AvailableAchievements),
        [three, five]
    );
    assert_eq!(
        game.position.score(A),
        11,
        "doubling never changes the score"
    );
}

#[test]
fn artificial_heart_doubles_polhems_score_but_not_leeuwenhoeks_claim_only_points() {
    let mut f = Fixture::default();
    let source = heart(&mut f, A, false);
    f.figure(Figure::ChristopherPolhem, Zone::Board(A, Color::Yellow));
    card(&mut f, 1, Zone::Achievements(A));
    let achievement = card(&mut f, 5, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    assert_eq!(
        game.position.score(A),
        13,
        "Heart's 11 plus Polhem's two Industry"
    );
    start(&mut game, source, A);
    finish(&mut game, &mut Vec::new());
    assert!(
        game.position
            .cards(Zone::Achievements(A))
            .contains(&achievement)
    );
    assert_eq!(
        game.position.score(A),
        15,
        "the new achievement increases Polhem's score"
    );

    let mut f = Fixture::default();
    let source = heart(&mut f, A, false);
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    card(&mut f, 6, Zone::Hand(A));
    card(&mut f, 7, Zone::Hand(A));
    let six = card(&mut f, 6, Zone::AvailableAchievements);
    let seven = card(&mut f, 7, Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    assert_eq!(game.position.score(A), 11);
    start(&mut game, source, A);
    let mut events = Vec::new();
    assert_eq!(
        game.advance(1000, &mut events).unwrap(),
        Status::Complete(ReturnValue::Unit),
        "only age 6 qualifies, so its claim is forced"
    );
    finish(&mut game, &mut Vec::new());
    assert_eq!(
        game.position.cards(Zone::Achievements(A)),
        [six],
        "22 doubled score + 10 hand points covers 30, not 35"
    );
    assert_eq!(game.position.cards(Zone::AvailableAchievements), [seven]);
    assert_eq!(game.position.score(A), 11);
}

#[test]
fn artificial_heart_retains_doubling_after_hedy_removes_daedalus_allowance() {
    for age in [9, 10] {
        let mut f = Fixture::default();
        // The sharer executes a Heart physically owned by the opponent.
        let source = heart(&mut f, B, false);
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), Vec::new());
        f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
        f.tech(Color::Green, 1, Zone::Board(A, Color::Green), Vec::new());
        f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
        for value in [10, 10, 5] {
            card(&mut f, value, Zone::Score(A));
        }
        let achievement = card(&mut f, age, Zone::AvailableAchievements);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        game.position.players[A.index()].splays[Color::Green as usize] = Splay::Up;
        game.position.refresh_board(A);
        assert_eq!(game.position.score(A), 25);
        assert_eq!(game.position.highest_claim_value(A), 10);
        assert!(!game.position.eligible_for_achievement(A, achievement));
        start(&mut game, source, A);
        let mut events = Vec::new();
        finish(&mut game, &mut events);
        assert_eq!(game.position.highest_claim_value(A), 9);
        assert_eq!(game.position.splay(A, Color::Blue), Splay::Up);
        assert_eq!(
            game.position.locate(achievement),
            Some(if age == 9 {
                Zone::Achievements(A)
            } else {
                Zone::AvailableAchievements
            })
        );
        assert!(game.position.cards(Zone::Achievements(B)).is_empty());
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Splayed {
                color: Color::Blue,
                inside_karma: true,
                ..
            }
        )));
    }
}

#[test]
fn artificial_heart_foreseen_rechecks_score_and_duplicate_cost_each_repeat() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = heart(&mut f, A, foreseen);
        f.figure(Figure::ChristopherPolhem, Zone::Board(A, Color::Yellow));
        let first = card(&mut f, 4, Zone::AvailableAchievements);
        let second = card(&mut f, 5, Zone::AvailableAchievements);
        let third = card(&mut f, 5, Zone::AvailableAchievements);
        let promotion = f.tech(Color::Red, 11, Zone::Hand(A), Vec::new());
        let mut game = Game::new(f.position(), BOTH).unwrap();
        if foreseen {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, A);
        }
        finish(&mut game, &mut Vec::new());
        let achieved = game.position.cards(Zone::Achievements(A));
        assert!(achieved.contains(&first));
        assert_eq!(achieved.len(), if foreseen { 2 } else { 1 });
        assert_eq!(
            usize::from(achieved.contains(&second)) + usize::from(achieved.contains(&third)),
            usize::from(foreseen)
        );
        // Score 11 -> 13 makes age 5 newly affordable (26 >= 25); the
        // following duplicate costs 50 and stays despite score rising to 15.
        assert_eq!(game.position.score(A), if foreseen { 15 } else { 13 });
    }
}

#[test]
fn artificial_heart_failed_commit_stops_foreseen_repeat_after_hedy_lowers_score() {
    let mut f = Fixture::default();
    heart(&mut f, A, true);
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    let bonus = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), Vec::new());
    f.cards[usize::from(bonus.0)].set = CardSet::Echoes;
    f.cards[usize::from(bonus.0)].symbols[0] = Symbol::Bonus(11);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), Vec::new());
    card(&mut f, 1, Zone::Score(A));
    let cheaper = card(&mut f, 4, Zone::AvailableAchievements);
    let chosen = card(&mut f, 5, Zone::AvailableAchievements);
    let promotion = f.tech(Color::Yellow, 11, Zone::Hand(A), Vec::new());
    let mut game = Game::new(f.position(), BOTH).unwrap();
    game.position.players[A.index()].splays[Color::Red as usize] = Splay::Aslant;
    game.position.refresh_board(A);
    game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    choose_age(&mut game, &mut events, 5);
    assert_eq!(game.position.score(A), 13);
    finish(&mut game, &mut events);
    assert_eq!(game.position.score(A), 12);
    assert_eq!(game.position.splay(A, Color::Red), Splay::Up);
    assert!(game.position.cards(Zone::Achievements(A)).is_empty());
    assert_eq!(
        game.position.cards(Zone::AvailableAchievements),
        [cheaper, chosen]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Achieve)))
    );
}

#[test]
fn artificial_heart_blind_claims_sample_native_backs_and_preserve_sharing() {
    assert!(!Test::Eligible { score_factor: 2 }.reads_front());
    let mut f = Fixture::default();
    let source = heart(&mut f, A, false);
    let other = f.tech(Color::Green, 4, Zone::Board(B, Color::Green), Vec::new());
    f.cards[usize::from(other.0)].symbols[0] = Symbol::Icon(Icon::Health);
    f.cards[usize::from(other.0)].symbols[1] = Symbol::Icon(Icon::Health);
    card(&mut f, 10, Zone::Score(B));
    let base: Vec<_> = (0..2)
        .map(|_| card(&mut f, 4, Zone::AvailableAchievements))
        .collect();
    let echoes = card(&mut f, 4, Zone::AvailableAchievements);
    f.cards[usize::from(echoes.0)].set = CardSet::Echoes;
    let reward = card(&mut f, 11, Zone::Deck(Deck::base(11)));
    let mut game = Game::seeded(f.position(), Expansions::ECHOES, 71)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut expected_rng = game.rng.clone();
    let sampled = base[(expected_rng.next_u64() % 2) as usize];
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, B);
    assert_eq!(
        menu.options.as_slice(),
        [CardSet::Base, CardSet::Echoes].map(|set| Alternative::BlindMove {
            verb: Verb::Achieve,
            source: Zone::AvailableAchievements,
            back: Deck { set, age: 4 }
        })
    );
    let world = game.determinize(B, 123).unwrap();
    assert!(world.tracker(B).unwrap().check_truth(world.position()));
    game.choose(0).unwrap();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    let world = game.determinize(A, 124).unwrap();
    assert!(world.tracker(A).unwrap().check_truth(world.position()));
    game.choose(0).unwrap();
    finish(&mut game, &mut events);
    assert_eq!(game.rng, expected_rng);
    assert_eq!(game.position.cards(Zone::Achievements(B)), [sampled]);
    assert_eq!(game.position.cards(Zone::Achievements(A)).len(), 1);
    assert_eq!(game.position.cards(Zone::Hand(A)), [reward]);
    assert_eq!(game.position.cards(Zone::AvailableAchievements), [echoes]);
}

#[test]
fn artificial_heart_replaced_claim_does_not_repeat_or_record_an_achievement() {
    let mut f = Fixture::default();
    heart(&mut f, A, true);
    let three = card(&mut f, 3, Zone::AvailableAchievements);
    let four = card(&mut f, 4, Zone::AvailableAchievements);
    let promotion = f.tech(Color::Red, 11, Zone::Hand(A), Vec::new());
    let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
    // This fixture interrupts the original claim to simulate replacement;
    // keep its choice boundary despite the ordinary deterministic reduction.
    game.shortcuts_disabled = true;
    game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    choose_age(&mut game, &mut events, 4);
    // No current Figure has Instead on Claim. Exercise the existing replaced
    // operation continuation without inventing one for the catalog.
    let mut replaced = false;
    for _ in 0..30 {
        if let Some(Frame::Operation { phase, .. }) = game.frames.iter_mut().rev().find(|frame| {
            matches!(
                frame,
                Frame::Operation {
                    request: Operation::Claim { .. },
                    ..
                }
            )
        }) {
            *phase = Phase::Replaced;
            replaced = true;
            break;
        }
        assert_eq!(game.advance(1, &mut events).unwrap(), Status::WorkLimit);
    }
    assert!(replaced);
    finish(&mut game, &mut events);
    assert!(game.position.cards(Zone::Achievements(A)).is_empty());
    assert_eq!(
        game.position.cards(Zone::AvailableAchievements),
        [three, four]
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Achieve)))
    );
}

#[test]
fn artificial_heart_standard_supply_excludes_every_figure_board_alias() {
    for figure in [
        Figure::CaiLun,
        Figure::JohnLoudonMcAdam,
        Figure::AminaSukhera,
    ] {
        for supply in [false, true] {
            let mut f = Fixture::default();
            let source = heart(&mut f, A, false);
            let definition = figure_card(figure.code()).unwrap();
            let color = definition.color;
            f.add(definition, Zone::Board(A, color));
            let alias = card(&mut f, 1, Zone::Board(A, Color::Purple));
            f.cards[usize::from(alias.0)].symbols[0] = Symbol::Icon(Icon::Industry);
            f.cards[usize::from(alias.0)].symbols[1] = Symbol::Icon(Icon::Authority);
            let standard = supply.then(|| card(&mut f, 2, Zone::AvailableAchievements));
            let mut game = Game::new(f.position(), BOTH).unwrap();
            assert!(
                game.position
                    .targets(LogicalZone::AvailableAchievements(A))
                    .any(|target| target.card == alias)
            );
            assert!(game.position.eligible_for_achievement(A, alias));
            start(&mut game, source, A);
            finish(&mut game, &mut Vec::new());
            assert_eq!(
                game.position.locate(alias),
                Some(Zone::Board(A, Color::Purple))
            );
            assert_eq!(
                game.position.cards(Zone::Achievements(A)),
                standard.as_slice(),
                "{figure:?}, supply={supply}"
            );
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ClaimEnd {
    position: Position,
    rng: Rng,
    status: Status,
    events: Vec<Event>,
    independent_claims: Vec<Movement>,
}

fn claim_paths(
    mut game: Game,
    mut events: Vec<Event>,
    independent: bool,
    depth: usize,
) -> Vec<ClaimEnd> {
    assert!(depth < 20);
    match game.advance(1000, &mut events).unwrap() {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut next = game.clone();
                next.choose(choice).unwrap();
                claim_paths(next, events.clone(), independent, depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            for player in Player::ALL {
                assert!(game.tracker(player).unwrap().check_truth(&game.position));
            }
            game.position.sort_unordered_piles();
            for player in Player::ALL {
                game.position
                    .zone_mut(Zone::Achievements(player))
                    .sort_unstable();
            }
            game.position
                .zone_mut(Zone::AvailableAchievements)
                .sort_unstable();
            // Only the no-reaction proof permits claim event permutations.
            // The Hedy fallback compares the full interleaved event trace.
            let mut independent_claims = Vec::new();
            if independent {
                events.retain(|event| {
                    if let Event::Moved(movement) = event
                        && movement.verb == Some(Verb::Achieve)
                        && !movement.inside_karma
                    {
                        independent_claims.push(*movement);
                        false
                    } else {
                        true
                    }
                });
                independent_claims.sort_by_key(|movement| movement.card);
            }
            vec![ClaimEnd {
                position: game.position,
                rng: game.rng,
                status,
                events,
                independent_claims,
            }]
        }
    }
}

#[test]
fn artificial_heart_claim_order_shortcut_matches_every_path_and_reacting_figure() {
    // The simple score-11 / age-3-and-4 case must have no claim decision.
    // An unrelated live Figure does not prevent the same proof. Same-age
    // different-set choices remain, and duplicate backs conservatively defer.
    for case in 0..5 {
        let hedy = case == 4;
        let mut f = Fixture::default();
        heart(&mut f, A, true);
        if case == 1 {
            f.figure(Figure::Ptolemy, Zone::Board(B, Color::Green));
        }
        if hedy {
            f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
            let bonus = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), Vec::new());
            f.cards[usize::from(bonus.0)].set = CardSet::Echoes;
            f.cards[usize::from(bonus.0)].symbols[0] = Symbol::Bonus(11);
            f.tech(Color::Red, 2, Zone::Board(A, Color::Red), Vec::new());
            card(&mut f, 1, Zone::Score(A));
        }
        let ages: &[u8] = match case {
            2 => &[2, 2, 4],
            3 => &[3, 3, 4, 4],
            4 => &[4, 5],
            _ => &[3, 4],
        };
        for (index, &age) in ages.iter().enumerate() {
            let achievement = card(&mut f, age, Zone::AvailableAchievements);
            if case == 2 && index == 1 {
                f.cards[usize::from(achievement.0)].set = CardSet::Echoes;
            }
        }
        let promotion = f.tech(Color::Yellow, 11, Zone::Hand(A), Vec::new());
        let mut reduced = Game::seeded(f.position(), BOTH, 91)
            .unwrap()
            .with_tracking()
            .unwrap();
        if hedy {
            reduced.position.players[A.index()].splays[Color::Red as usize] = Splay::Aslant;
            reduced.position.refresh_board(A);
        }
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        for game in [&mut reduced, &mut manual] {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        }
        let reduced = claim_paths(reduced, Vec::new(), !hedy, 0);
        let manual = claim_paths(manual, Vec::new(), !hedy, 0);
        if case < 3 {
            assert!(reduced.len() < manual.len(), "case {case}");
            if case < 2 {
                assert_eq!(reduced.len(), 1);
            }
        } else {
            // Hedy blocks the claim-order shortcut, so the order stays a
            // choice; her no-op splay colours are one option on the reduced
            // side, so only the outcomes are compared, below.
            assert!(reduced.len() > 1, "fallback case {case}");
        }
        assert!(
            reduced.iter().all(|end| manual.contains(end)),
            "reduced case {case}"
        );
        assert!(
            manual.iter().all(|end| reduced.contains(end)),
            "manual case {case}"
        );
        // Hedy may also name a color its splay cannot change (a no-op
        // choice is allowed), so only some paths splay.
        if hedy {
            assert!(
                reduced
                    .iter()
                    .any(|end| end.events.iter().any(|event| matches!(
                        event,
                        Event::Splayed {
                            color: Color::Red,
                            inside_karma: true,
                            ..
                        }
                    )))
            );
            assert!(
                reduced
                    .iter()
                    .any(|end| end.position.cards(Zone::Achievements(A)).is_empty())
            );
            assert!(
                reduced
                    .iter()
                    .any(|end| end.position.cards(Zone::Achievements(A)).len() == 1)
            );
        }
    }
}

const CLAIM_PACKET: Slot = Slot::effect(0);
const NAMED_CLAIMS: Selector = Selector::Filtered(
    &Selector::StandardAchievements,
    &[Filter::Has(Test::InSlot(CLAIM_PACKET))],
);
const MIXED_CLAIMS: Selector = Selector::Union(&[NAMED_CLAIMS, Selector::StandardAchievements]);

fn packet_claim_fixture(tracked: bool, mixed: bool, forced: bool) -> (Game, [CardId; 2]) {
    packet_claim_fixture_evidence(tracked, mixed, forced, false)
}

fn packet_claim_fixture_evidence(
    tracked: bool,
    mixed: bool,
    forced: bool,
    private: bool,
) -> (Game, [CardId; 2]) {
    use crate::program::build::*;
    let mut f = Fixture::default();
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), Vec::new());
    let mut source = echoes_card("E102").unwrap();
    source.effects[0].instructions = vec![
        draw(1),
        reveal(LAST).keep(CLAIM_PACKET),
        transfer(LAST, Destination::Supply),
    ];
    if !forced {
        source.effects[0].instructions.extend([
            draw(1),
            reveal(LAST).add(CLAIM_PACKET),
            transfer(LAST, Destination::Supply),
        ]);
    }
    if private {
        source.effects[0].instructions = vec![
            draw(1).keep(CLAIM_PACKET),
            transfer(LAST, Destination::Supply),
            draw(1).add(CLAIM_PACKET),
            transfer(LAST, Destination::Supply),
        ];
    }
    source.effects[0].instructions.push(achieve(filtered(
        if mixed { &MIXED_CLAIMS } else { &NAMED_CLAIMS },
        &[Filter::Has(Test::Eligible { score_factor: 2 })],
    )));
    if mixed && !private {
        source.effects[0].instructions.push(choose(
            Who::Executor,
            Choice::Value(ValueChoices::Boxed(&[1, 2])),
        ));
    }
    let source = f.add(source, Zone::Board(A, Color::Blue));
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    // Two legal splays make First a real decision at which the captured
    // checked claim can be cloned or sampled before it commits.
    for _ in 0..2 {
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), Vec::new());
    }
    for code in ["B001", "B002"] {
        f.add(base_card(code).unwrap(), Zone::AvailableAchievements);
    }
    f.add(base_card("B003").unwrap(), Zone::Deck(Deck::base(1)));
    let revealed =
        ["B004", "B005"].map(|code| f.add(base_card(code).unwrap(), Zone::Deck(Deck::base(1))));
    if private {
        let figure = figure_card(Figure::ZhangHeng.code()).unwrap();
        f.add(figure.clone(), Zone::Board(B, figure.color));
    }
    let mut position = f.position();
    if private {
        position.turn_owner = B;
    }
    let mut game = Game::seeded(position, BOTH, 73).unwrap();
    if tracked {
        game = game.with_tracking().unwrap();
    }
    start(&mut game, source, A);
    (game, revealed)
}

fn named_claim(card: CardId) -> Alternative {
    Alternative::Move {
        verb: Verb::Achieve,
        target: TargetDescriptor::Known {
            card,
            source: Zone::AvailableAchievements,
        },
    }
}

fn pending_checked_claim(game: &Game) -> (CardId, Option<OperandKnowledge>) {
    game.frames
        .iter()
        .find_map(|frame| match frame {
            Frame::Operation {
                request:
                    Operation::Claim {
                        target,
                        eligibility: ClaimEligibility::Cost { score_factor: 2 },
                    },
                knowledge,
                ..
            } => Some((target.card, *knowledge)),
            _ => None,
        })
        .expect("a checked claim waits through First")
}

fn finish_packet_claim(mut game: Game, chosen: CardId) {
    let menu = decision(&mut game, &mut Vec::new());
    assert!(menu.options.contains(&Alternative::Splay {
        player: A,
        color: Color::Blue,
        direction: Splay::Up
    }));
    assert_eq!(pending_checked_claim(&game).0, chosen);
    let mut events = Vec::new();
    let before_rng = game.rng.clone();
    game.choose(0).unwrap();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Achievements(A)), [chosen]);
    assert_eq!(game.rng, before_rng);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Splayed {
            inside_karma: true,
            ..
        }
    )));
}

#[test]
fn checked_named_claims_keep_each_identity_through_first_clones_and_samples() {
    for tracked in [false, true] {
        for forced in [false, true] {
            let (mut game, revealed) = packet_claim_fixture(tracked, false, forced);
            let expected = [named_claim(revealed[1]), named_claim(revealed[0])];
            let first = decision(&mut game, &mut Vec::new());
            if !forced {
                assert_eq!(first.options, expected);
            }
            let mut roots = vec![game.clone()];
            if tracked {
                for seed in 0..16 {
                    roots.push(game.determinize(A, seed).unwrap());
                    roots.push(game.sample_world(A, seed).unwrap().world);
                }
            }
            for root in roots {
                for choice in 0..if forced { 1 } else { 2 } {
                    let mut world = root.clone();
                    if !forced {
                        assert_eq!(decision(&mut world, &mut Vec::new()).options, expected);
                        world.choose(choice).unwrap();
                    }
                    let chosen = revealed[1 - choice];
                    let before_rng = world.rng.clone();
                    decision(&mut world, &mut Vec::new());
                    let (actual, knowledge) = pending_checked_claim(&world);
                    assert_eq!(actual, chosen);
                    assert!(knowledge.expect("retained named claim").evidence.known(A));
                    assert_eq!(world.rng, before_rng, "named selection does not sample");
                    if world.tracker(A).is_some() {
                        for seed in 16..24 {
                            finish_packet_claim(world.determinize(A, seed).unwrap(), chosen);
                            finish_packet_claim(world.sample_world(A, seed).unwrap().world, chosen);
                        }
                    }
                    finish_packet_claim(world, chosen);
                }
            }
        }
    }
}

#[test]
fn mixed_checked_claims_rebuild_anonymous_remainder_after_sampling_named_supply() {
    for tracked in [false, true] {
        let (mut game, revealed) = packet_claim_fixture(tracked, true, false);
        let expected = [
            Alternative::BlindMove {
                verb: Verb::Achieve,
                source: Zone::AvailableAchievements,
                back: Deck::base(1),
            },
            named_claim(revealed[1]),
            named_claim(revealed[0]),
        ];
        assert_eq!(decision(&mut game, &mut Vec::new()).options, expected);
        let mut roots = vec![game.clone()];
        if tracked {
            let observation = game.observe(A).unwrap();
            let mut permuted = false;
            for seed in 0..24 {
                let mut world = game.determinize(A, seed).unwrap();
                permuted |= world.position.cards(Zone::AvailableAchievements)
                    != game.position.cards(Zone::AvailableAchievements);
                assert_eq!(world.observe(A).unwrap(), observation);
                assert_eq!(decision(&mut world, &mut Vec::new()).options, expected);
                roots.push(world);
                roots.push(game.sample_world(A, seed).unwrap().world);
            }
            assert!(
                permuted,
                "sample actually moves named cards around anonymous supply"
            );
        }
        for root in roots {
            let anonymous: Vec<_> = root
                .position
                .cards(Zone::AvailableAchievements)
                .iter()
                .copied()
                .filter(|card| !revealed.contains(card))
                .collect();
            assert_eq!(anonymous.len(), 2);
            let Some(Frame::Choose { tasks, .. }) = root.frames.last() else {
                panic!("claim menu")
            };
            let Task::BlindOperations(group) = &tasks[0] else {
                panic!("blind supply group")
            };
            let candidates: Vec<_> = group
                .operations
                .iter()
                .map(|(operation, reference)| {
                    assert!(
                        reference.is_none(),
                        "anonymous supply is not a retained named reference"
                    );
                    let Operation::Claim {
                        target,
                        eligibility: ClaimEligibility::Cost { score_factor: 2 },
                    } = operation
                    else {
                        panic!("checked operation")
                    };
                    target.card
                })
                .collect();
            assert_eq!(
                candidates, anonymous,
                "blind group is exactly the unnamed sampled remainder"
            );
            for choice in 0..3 {
                let mut world = root.clone();
                assert_eq!(decision(&mut world, &mut Vec::new()).options, expected);
                let mut expected_rng = world.rng.clone();
                let chosen = if choice == 0 {
                    anonymous[(expected_rng.next_u64() % anonymous.len() as u64) as usize]
                } else {
                    revealed[2 - choice]
                };
                world.choose(choice).unwrap();
                let mut events = Vec::new();
                decision(&mut world, &mut events);
                assert_eq!(world.rng, expected_rng);
                let (actual, knowledge) = pending_checked_claim(&world);
                assert_eq!(actual, chosen);
                assert_eq!(
                    knowledge.is_some_and(|knowledge| knowledge.evidence.known(A)),
                    choice != 0
                );
                assert!(
                    !events.iter().any(
                        |event| matches!(event, Event::Revealed { card, .. } if *card == chosen)
                    )
                );
                // Sampling an anonymous pending claim may change its identity;
                // naming a packet card must keep that exact identity instead.
                if world.tracker(A).is_some() {
                    // Broaden every positional candidate just as a shuffle
                    // does. Identity exclusions must survive that broadening.
                    for tracker in &mut world.knowledge.as_mut().unwrap().trackers {
                        tracker.shuffle(Zone::AvailableAchievements);
                    }
                    for seed in 32..40 {
                        for sampled in [
                            world.determinize(A, seed).unwrap(),
                            world.sample_world(A, seed).unwrap().world,
                        ] {
                            let target = pending_checked_claim(&sampled).0;
                            if choice == 0 {
                                assert!(!revealed.contains(&target));
                            } else {
                                assert_eq!(target, chosen);
                            }
                            finish_packet_claim(sampled, target);
                        }
                    }
                }
                // Complete Hedy, then sample the retained claim result at a
                // later program decision after its hidden departure committed.
                world.choose(0).unwrap();
                assert_eq!(
                    decision(&mut world, &mut Vec::new()).options,
                    [Alternative::Value(1), Alternative::Value(2)]
                );
                if world.tracker(A).is_some() {
                    for seed in 40..56 {
                        for mut sample in [
                            world.determinize(A, seed).unwrap(),
                            world.sample_world(A, seed).unwrap().world,
                        ] {
                            let claimed = sample.position.cards(Zone::Achievements(A))[0];
                            if choice == 0 {
                                assert!(!revealed.contains(&claimed));
                            } else {
                                assert_eq!(claimed, chosen);
                            }
                            sample.choose(0).unwrap();
                            finish(&mut sample, &mut Vec::new());
                        }
                    }
                }
                world.choose(0).unwrap();
                finish(&mut world, &mut Vec::new());
                assert_eq!(world.position.cards(Zone::Achievements(A)), [chosen]);
            }
        }
    }
}

#[test]
fn checked_claim_verification_reveals_only_admitted_cards_before_first() {
    use crate::program::build::*;
    for forced in [false, true] {
        for admitted in [false, true] {
            let mut f = Fixture::default();
            let selected = f.add(base_card("B001").unwrap(), Zone::Score(A));
            f.add(base_card("B002").unwrap(), Zone::Score(A));
            f.add(base_card("B003").unwrap(), Zone::Score(A));
            let instructions = if forced {
                vec![
                    bind(filtered(&SCORE, &[Filter::Has(Test::Code("B001"))])),
                    achieve(filtered(
                        &Selector::Reg(Reg::Bound),
                        &[Filter::Has(Test::Eligible { score_factor: 2 })],
                    ))
                    .verify(),
                ]
            } else {
                vec![
                    achieve(filtered(
                        &SCORE,
                        &[Filter::Has(Test::Eligible { score_factor: 2 })],
                    ))
                    .verify(),
                ]
            };
            f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), Vec::new());
            let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), instructions);
            for _ in 0..2 {
                f.tech(Color::Red, 1, Zone::Board(A, Color::Red), Vec::new());
            }
            f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, source, A);
            let mut events = Vec::new();
            if !forced {
                let menu = decision(&mut game, &mut events);
                let index = menu.options.iter().position(|option| matches!(option,
                    Alternative::Move { verb: Verb::Achieve, target: TargetDescriptor::Known { card, .. } } if *card == selected
                )).unwrap();
                game.choose(index).unwrap();
            }
            // Pause immediately before Discover to model a newly invalid
            // retained claim without altering any zone or tracker support.
            for _ in 0..30 {
                if matches!(
                    game.frames.last(),
                    Some(Frame::Operation {
                        request: Operation::Claim { .. },
                        phase: Phase::Discover,
                        ..
                    })
                ) {
                    break;
                }
                assert_eq!(
                    game.advance(1, &mut events).unwrap(),
                    Status::WorkLimit,
                    "forced={forced}, admitted={admitted}, frames={:?}",
                    game.frames
                );
            }
            let Some(Frame::Operation {
                request: Operation::Claim { eligibility, .. },
                knowledge,
                ..
            }) = game.frames.last_mut()
            else {
                panic!("checked claim ready")
            };
            assert!(knowledge.is_some_and(|knowledge| knowledge.verify));
            if !admitted {
                *eligibility = ClaimEligibility::Cost { score_factor: 1 };
            }
            if admitted {
                let first = decision(&mut game, &mut events);
                assert!(
                    first
                        .options
                        .iter()
                        .all(|option| matches!(option, Alternative::Splay { .. }))
                );
                assert!(events.iter().any(
                    |event| matches!(event, Event::Revealed { card, .. } if *card == selected)
                ));
                assert!(pending_checked_claim(&game).1.unwrap().evidence.known(B));
                game.choose(0).unwrap();
            }
            finish(&mut game, &mut events);
            let reveals: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed { card, .. } => Some(*card),
                    _ => None,
                })
                .collect();
            assert_eq!(reveals, if admitted { vec![selected] } else { vec![] });
            assert_eq!(
                game.position.locate(selected),
                Some(if admitted {
                    Zone::Achievements(A)
                } else {
                    Zone::Score(A)
                })
            );
            if admitted {
                let reveal = events
                    .iter()
                    .position(
                        |event| matches!(event, Event::Revealed { card, .. } if *card == selected),
                    )
                    .unwrap();
                let claim = events.iter().position(|event| matches!(event, Event::Moved(m) if m.card == selected && m.verb == Some(Verb::Achieve))).unwrap();
                assert!(reveal < claim);
            }
        }
    }
}

#[test]
fn private_named_claim_exclusions_never_reach_the_other_observer() {
    let (mut game, private_names) = packet_claim_fixture_evidence(true, true, false, true);
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    assert_eq!(
        menu.options,
        [
            Alternative::BlindMove {
                verb: Verb::Achieve,
                source: Zone::AvailableAchievements,
                back: Deck::base(1)
            },
            named_claim(private_names[1]),
            named_claim(private_names[0]),
        ]
    );
    game.choose(0).unwrap();
    let mut events = Vec::new();
    let first = decision(&mut game, &mut events);
    assert_eq!(
        first.player, B,
        "turn owner orders actual Hedy and Zhang Heng First hooks"
    );
    assert_eq!(first.options.len(), 2);
    assert!(
        first
            .options
            .iter()
            .all(|option| matches!(option, Alternative::Karma { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Revealed { .. }))
    );
    let observation = game.observe(B).unwrap();
    let (chosen, knowledge) = pending_checked_claim(&game);
    let pick = knowledge.unwrap().evidence.pick().unwrap();
    assert!(!private_names.contains(&chosen));
    for &named in &private_names {
        assert!(
            !game
                .tracker(A)
                .unwrap()
                .pick_holds(&game.position, named, pick)
        );
        assert!(
            game.tracker(B)
                .unwrap()
                .pick_holds(&game.position, named, pick)
        );
        assert!(
            game.public_tracker()
                .unwrap()
                .pick_holds(&game.position, named, pick)
        );
    }
    let mut retired = game.knowledge.as_ref().unwrap().clone();
    retired.retain_picks(0);
    let reused = (0..=pick.index())
        .map(|_| retired.allocate().unwrap())
        .last()
        .unwrap();
    assert_eq!(reused, pick);
    for tracker in &mut retired.trackers {
        tracker.mark(&game.position, private_names[0], reused);
        assert!(
            tracker.pick_holds(&game.position, private_names[0], reused),
            "retired pick exclusions cannot follow its reused ID"
        );
    }
    let mut included_private_name = false;
    let mut roots = Vec::new();
    for seed in 0..48 {
        let mut world = game.determinize(B, seed).unwrap();
        assert_eq!(world.observe(B).unwrap(), observation);
        assert_eq!(decision(&mut world, &mut Vec::new()), first);
        included_private_name |= private_names.contains(&pending_checked_claim(&world).0);
        assert!(world.position.check_conservation());
        for viewer in Player::ALL {
            assert!(world.tracker(viewer).unwrap().check_truth(&world.position));
        }
        roots.push(world);
    }
    assert!(
        included_private_name,
        "B's selected-pick domain still includes identities known only to A"
    );
    // Private actor memory differs between these worlds. B's new sample must
    // depend on B's evidence and the supplied seed, not that private memory.
    let expected = roots[0].sample_world(B, 411).unwrap().world;
    for root in roots {
        let sample = root.sample_world(B, 411).unwrap().world;
        assert_eq!(sample.position, expected.position);
        assert_eq!(
            pending_checked_claim(&sample).0,
            pending_checked_claim(&expected).0
        );
    }
}

#[test]
fn retained_blind_claim_domain_survives_ambiguous_arrival_and_departure() {
    use crate::program::build::*;
    const FIRST_CLAIM: Slot = Slot::effect(1);
    const SECOND_CLAIM: Slot = Slot::effect(2);
    fn first_reference(game: &Game) -> HeldCard {
        let mut view = game.clone();
        assert!(matches!(view.frames.pop(), Some(Frame::Choose { .. })));
        view.slot_register(
            FIRST_CLAIM,
            Context::new(A),
            Regs {
                locals: true,
                ..Regs::EMPTY
            },
        )
        .unwrap()
    }
    let (mut game, named) = packet_claim_fixture(true, true, false);
    let source = *game
        .position
        .cards(Zone::Board(A, Color::Blue))
        .last()
        .unwrap();
    let program = &mut std::sync::Arc::make_mut(&mut game.position.catalog)[usize::from(source.0)]
        .effects[0]
        .instructions;
    program[6] = program[6].keep(FIRST_CLAIM);
    program.extend([
        achieve(Selector::StandardAchievements).keep(SECOND_CLAIM),
        choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
        transfer(
            Selector::Achievements(Who::Executor),
            Destination::Hand(Who::Opponent),
        ),
        choose(Who::Executor, Choice::Value(ValueChoices::Boxed(&[1, 2]))),
    ]);
    decision(&mut game, &mut Vec::new());
    game.choose(0).unwrap();
    decision(&mut game, &mut Vec::new()); // Hedy's first splay.
    game.choose(0).unwrap();
    assert_eq!(
        decision(&mut game, &mut Vec::new()).options,
        [Alternative::Value(1), Alternative::Value(2)]
    );
    game.choose(0).unwrap();
    // Hedy's second splay: a no-op choice is still a choice.
    decision(&mut game, &mut Vec::new());
    game.choose(0).unwrap();
    assert_eq!(
        decision(&mut game, &mut Vec::new()).options,
        [Alternative::Value(1), Alternative::Value(2)]
    );
    assert_eq!(game.position.cards(Zone::Achievements(A)).len(), 2);
    for tracker in game.knowledge.as_ref().unwrap().trackers.iter() {
        assert!(
            tracker
                .belief(Zone::Achievements(A))
                .iter()
                .any(|slot| slot.candidates.iter().all(|card| !named.contains(card))),
            "the public first claim still certifies one achievement outside the named packet"
        );
    }
    let first_card = first_reference(&game);
    assert!(!named.contains(&first_card.inner));
    for seed in 0..64 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            let first_card = first_reference(&world);
            assert!(!named.contains(&first_card.inner));
            assert_eq!(
                world.position.locate(first_card.inner),
                Some(Zone::Achievements(A))
            );
            world.choose(0).unwrap();
            finish(&mut world, &mut Vec::new());
        }
    }

    game.choose(0).unwrap();
    let mut expected_rng = game.rng.clone();
    expected_rng.next_u64();
    // One anonymous back is forced: native chance picks which achievement
    // departs, then the next meaningful decision is the continuation value.
    assert_eq!(
        decision(&mut game, &mut Vec::new()).options,
        [Alternative::Value(1), Alternative::Value(2)]
    );
    assert_eq!(game.rng, expected_rng);
    let first_pick = first_reference(&game).evidence.pick().unwrap();
    assert_eq!(
        game.tracker(A)
            .unwrap()
            .pick_position(Zone::Achievements(A), first_pick),
        None
    );
    assert_eq!(
        game.tracker(A)
            .unwrap()
            .pick_position(Zone::Hand(B), first_pick),
        None
    );
    for seed in 0..64 {
        for mut world in [
            game.determinize(A, seed).unwrap(),
            game.sample_world(A, seed).unwrap().world,
        ] {
            let retained = first_reference(&world);
            assert!(!named.contains(&retained.inner));
            assert!(matches!(
                world.position.locate(retained.inner),
                Some(Zone::Achievements(A) | Zone::Hand(B))
            ));
            world.choose(0).unwrap();
            finish(&mut world, &mut Vec::new());
        }
    }
}
