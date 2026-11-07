//! Age-eleven frozen cohorts and live achievement choices.
use super::*;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn card(f: &mut Fixture, code: &str, zone: Option<Zone>) -> CardId {
    let printed = echoes_card(code).unwrap();
    let color = printed.color;
    f.add(printed, zone.unwrap_or(Zone::Board(A, color)))
}
fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, Vec::new())
}
fn figure(f: &mut Fixture, figure: Figure, owner: Player) -> CardId {
    let printed = figure_card(figure.code()).unwrap();
    let color = printed.color;
    f.add(printed, Zone::Board(owner, color))
}
fn deck(f: &mut Fixture, set: CardSet, age: u8, color: Color) -> CardId {
    let id = tech(f, color, age, Zone::Deck(Deck { set, age }));
    f.cards[usize::from(id.0)].set = set;
    id
}
fn start(game: &mut Game, card: CardId, effect: u16, context: Context, increase: u8) {
    let mut boxed = BoxedValues::ZERO;
    boxed.increase(increase).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed,
        },
        context,
    )
    .unwrap();
}
fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut choose: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..150 {
        let mut clone = game.clone();
        let mut next = Vec::new();
        let status = game.advance(1000, &mut next).unwrap();
        let mut expected = Vec::new();
        assert_eq!(clone.advance(1000, &mut expected).unwrap(), status);
        assert_eq!(clone, *game);
        assert_eq!(expected, next);
        events.extend(next);
        match status {
            Status::Decision(menu) => game.choose(choose(&menu)).unwrap(),
            status => {
                assert!(game.position.check_conservation());
                for viewer in Player::ALL {
                    if let Some(tracker) = game.tracker(viewer) {
                        assert!(tracker.check_truth(&game.position));
                    }
                }
                return status;
            }
        }
    }
    panic!("age-eleven card failed to finish");
}
fn pick(menu: &Decision, card: CardId) -> usize {
    menu.options.iter().position(|option| matches!(option, Alternative::Move { target: TargetDescriptor::Known { card: target, .. }, .. } if *target == card)).unwrap()
}
fn decline(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Decline)
        .unwrap_or(0)
}
fn returned(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(Verb::Return) && !m.inside_karma => Some(m.card),
            _ => None,
        })
        .collect()
}

#[test]
fn drone_reveals_original_draw_before_color_branch_and_splays_only_its_own_board() {
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
    for count in [0, 1, 2, 5] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E106", None);
        for _ in 0..count {
            tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        }
        for _ in 0..6 {
            tech(&mut f, Color::Green, 1, Zone::Board(B, Color::Green));
        }
        let drawn = deck(&mut f, CardSet::Base, 10, Color::Green);
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Hand(A)), [drawn]);
        assert_eq!(
            game.position.splay(A, Color::Green),
            if count >= 2 {
                Splay::Aslant
            } else {
                Splay::None
            }
        );
        assert_eq!(game.position.splay(B, Color::Green), Splay::None);
        assert!(returned(&events).is_empty());
        let reveal = events
            .iter()
            .position(|e| matches!(e,Event::Revealed{card,..} if *card==drawn))
            .unwrap();
        if count >= 2 {
            assert!(
                events[reveal + 1..]
                    .iter()
                    .any(|e| matches!(e, Event::Splayed { .. }))
            );
        }
        assert!(
            game.tracker(B)
                .unwrap()
                .belief(Zone::Hand(A))
                .iter()
                .any(|slot| slot.candidates == [drawn])
        );
    }
}

fn drone_setup(other: u8) -> (Fixture, CardId, Vec<CardId>, Vec<CardId>, CardId, CardId) {
    let mut f = Fixture::default();
    let source = card(&mut f, "E106", None);
    let own = (1..=6)
        .map(|n| tech(&mut f, Color::Green, n, Zone::Board(A, Color::Green)))
        .collect();
    let theirs = (1..=other)
        .map(|n| tech(&mut f, Color::Green, n, Zone::Board(B, Color::Green)))
        .collect();
    let second = deck(&mut f, CardSet::Base, 10, Color::Purple);
    let first = deck(&mut f, CardSet::Base, 10, Color::Green);
    (f, source, own, theirs, first, second)
}
#[test]
fn drone_requires_five_successful_originals_from_each_board_before_repeating() {
    for other in [0, 4, 5, 6] {
        let (f, source, own, theirs, first, second) = drone_setup(other);
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        let returns = returned(&events);
        assert_eq!(returns.len(), 5 + usize::from(other.min(5)));
        assert!(own[..5].iter().all(|c| returns.contains(c)));
        assert!(
            theirs[..usize::from(other.min(5))]
                .iter()
                .all(|c| returns.contains(c))
        );
        assert_eq!(game.position.cards(Zone::Board(A, Color::Green)), [own[5]]);
        assert_eq!(
            game.position.cards(Zone::Board(B, Color::Green)),
            &theirs[usize::from(other.min(5))..]
        );
        assert!(game.position.cards(Zone::Hand(A)).contains(&first));
        assert_eq!(
            game.position.cards(Zone::Hand(A)).contains(&second),
            other >= 5
        );
    }
}
#[test]
fn frozen_originals_survive_sneferus_first_tuck_changing_bottom_five_membership() {
    use crate::program::build::*;
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Red,
        11,
        Zone::Board(A, Color::Red),
        vec![
            meld(Selector::Board(
                Who::Executor,
                ColorSet::Only(&[Color::Green]),
                BoardRegion::BottomN(5),
            ))
            .all()
            .frozen(),
        ],
    );
    let originals: Vec<_> = (1..=6)
        .map(|v| tech(&mut f, Color::Green, v, Zone::Board(A, Color::Green)))
        .collect();
    figure(&mut f, Figure::Sneferu, A);
    let incoming: Vec<_> = (0..5)
        .map(|_| {
            let c = deck(&mut f, CardSet::Base, 2, Color::Green);
            f.cards[usize::from(c.0)].symbols[0] = Symbol::Icon(Icon::Authority);
            c
        })
        .collect();
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let sample = game.determinize(menu.player, 71).unwrap();
    assert_eq!(
        sample.observe(menu.player).unwrap(),
        game.observe(menu.player).unwrap()
    );
    game.choose(pick(&menu, originals[0])).unwrap();
    finish(&mut game, &mut events, |_| 0);
    let melded: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb == Some(Verb::Meld) && !m.inside_karma => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(melded.len(), 5);
    assert!(originals[..5].iter().all(|c| melded.contains(c)));
    assert!(incoming.iter().all(|c| !melded.contains(c)));
    assert_eq!(game.position.cards(Zone::Board(A, Color::Green)).len(), 11);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,Event::Moved(m) if m.verb==Some(Verb::Tuck)&&m.inside_karma))
            .count(),
        5
    );
}
#[test]
fn drone_replacement_and_departed_originals_cannot_satisfy_repeat() {
    for napoleon in [false, true] {
        let (mut f, source, own, _, _, second) = drone_setup(5);
        figure(
            &mut f,
            if napoleon {
                Figure::NapoleonBonaparte
            } else {
                Figure::PeterTheGreat
            },
            A,
        );
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert!(!game.position.cards(Zone::Hand(A)).contains(&second));
        assert!(returned(&events).len() < 10);
        assert!(
            events
                .iter()
                .any(|e| matches!(e,Event::Moved(m) if own.contains(&m.card)&&m.inside_karma))
        );
    }
}
#[test]
fn frozen_hidden_membership_refuses_foreign_samples_without_exposing_a_face() {
    use crate::program::build::*;
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let source = f.tech(
            Color::Red,
            11,
            Zone::Board(A, Color::Red),
            vec![
                achieve(filtered(&HAND, &[Filter::Has(Test::Color(Color::Blue))]))
                    .all()
                    .frozen()
                    .optional(),
            ],
        );
        let selected = tech(&mut f, Color::Blue, 2, Zone::Hand(A));
        tech(&mut f, Color::Green, 2, Zone::Hand(A));
        figure(&mut f, Figure::HedyLamarr, A);
        figure(&mut f, Figure::ZhangHeng, B);
        tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        deck(&mut f, CardSet::Base, 3, Color::Yellow);
        let mut position = f.position();
        position.turn_owner = B;
        let mut game = Game::new(position, BOTH).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert!(menu.options.contains(&Alternative::Decline));
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
        if tracked {
            assert_eq!(
                game.determinize(A, 7).unwrap().observe(A).unwrap(),
                game.observe(A).unwrap()
            );
        }
        game.choose(pick(&menu, selected)).unwrap();
        let first = decision(&mut game, &mut events);
        assert_eq!(first.player, B);
        assert!(
            first
                .options
                .iter()
                .all(|o| matches!(o, Alternative::Karma { .. }))
        );
        if tracked {
            for seed in 0..8 {
                assert!(matches!(
                    game.determinize(B, seed),
                    Err(Error::Unsupported(
                        "a frozen selection contains an unnamed original"
                    ))
                ));
            }
        }
        game.choose(0).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert!(
            game.position
                .cards(Zone::Achievements(A))
                .contains(&selected)
        );
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
    }
}
#[test]
fn exoskeleton_demand_preserves_all_lowest_ties_across_sets_and_empty_hands() {
    for values in [vec![], vec![2], vec![2, 2], vec![2, 2, 3, 4]] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E107", Some(Zone::Board(B, Color::Red)));
        let cards: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let c = tech(&mut f, Color::Purple, v, Zone::Hand(A));
                if i == 1 {
                    f.cards[usize::from(c.0)].set = CardSet::Echoes;
                }
                c
            })
            .collect();
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        let mut context = Context::new(A);
        context.demander = B;
        start(&mut game, source, 0, context, 0);
        finish(&mut game, &mut Vec::new(), |_| 0);
        for (c, v) in cards.iter().zip(&values) {
            assert_eq!(
                game.position.locate(*c),
                Some(if *v == 2 {
                    Zone::Hand(A)
                } else {
                    Zone::Score(B)
                })
            );
        }
    }
}
#[test]
fn exoskeleton_claims_either_hand_only_when_eligible_and_may_decline() {
    for decline_claim in [false, true] {
        for owner in Player::ALL {
            let mut f = Fixture::default();
            let source = card(&mut f, "E107", None);
            tech(&mut f, Color::Yellow, 10, Zone::Score(A));
            let eligible = tech(&mut f, Color::Blue, 2, Zone::Hand(owner));
            let ineligible = tech(&mut f, Color::Green, 3, Zone::Hand(owner));
            let mut game = Game::new(f.position(), Expansions::ECHOES)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, source, 1, Context::new(A), 0);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options.len(), 2);
            let option = menu
                .options
                .iter()
                .find(|o| !matches!(o, Alternative::Decline))
                .unwrap();
            if owner == B {
                assert!(matches!(option,Alternative::BlindMove{back,..} if back.age==2));
            }
            game.choose(if decline_claim {
                decline(&menu)
            } else {
                menu.options
                    .iter()
                    .position(|o| !matches!(o, Alternative::Decline))
                    .unwrap()
            })
            .unwrap();
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(
                game.position.locate(eligible),
                Some(if decline_claim {
                    Zone::Hand(owner)
                } else {
                    Zone::Achievements(A)
                })
            );
            assert_eq!(game.position.locate(ineligible), Some(Zone::Hand(owner)));
        }
    }
}
#[test]
fn exoskeleton_foreseen_repeats_successful_claims_and_rechecks_duplicate_cost() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let source = card(
            &mut f,
            "E107",
            Some(if foreseen {
                Zone::Forecast(A)
            } else {
                Zone::Board(A, Color::Red)
            }),
        );
        tech(&mut f, Color::Purple, 10, Zone::Score(A));
        let one = tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        let two = tech(&mut f, Color::Yellow, 2, Zone::Hand(A));
        let duplicate = tech(&mut f, Color::Green, 2, Zone::Hand(A));
        let promotion = tech(&mut f, Color::Purple, 11, Zone::Hand(A));
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        if foreseen {
            game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, 1, Context::new(A), 0);
        }
        finish(&mut game, &mut Vec::new(), |menu| {
            menu.options
                .iter()
                .position(|o| !matches!(o, Alternative::Decline))
                .unwrap_or(0)
        });
        let achieved = game.position.cards(Zone::Achievements(A));
        assert_eq!(achieved.len(), if foreseen { 2 } else { 1 });
        assert!(achieved.contains(&one));
        assert_eq!(
            usize::from(achieved.contains(&two)) + usize::from(achieved.contains(&duplicate)),
            usize::from(foreseen)
        );
    }
}

#[derive(Clone)]
struct End {
    game: Game,
    status: Status,
    events: Vec<Event>,
}
fn paths(mut game: Game, mut events: Vec<Event>, decisions: usize) -> Vec<End> {
    assert!(decisions < 12, "bounded oracle");
    let status = game.advance(10000, &mut events).unwrap();
    if let Status::Decision(menu) = status {
        (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                paths(fork, events.clone(), decisions + 1)
            })
            .collect()
    } else {
        assert!(
            matches!(
                status,
                Status::Complete(_) | Status::Winner(_) | Status::Draw(_)
            ),
            "{status:?}"
        );
        assert!(game.position.check_conservation());
        for viewer in Player::ALL {
            if let Some(tracker) = game.tracker(viewer) {
                assert!(tracker.check_truth(&game.position));
            }
        }
        if let Some(tracker) = game.public_tracker() {
            assert!(tracker.check_truth(&game.position));
        }
        vec![End {
            game,
            status,
            events,
        }]
    }
}
#[derive(PartialEq, Eq)]
struct Outcome {
    position: Position,
    rng: Rng,
    status: Status,
    beliefs: [Vec<Vec<crate::tracking::BeliefSlot>>; 3],
    views: [Vec<u32>; 2],
}
fn outcomes(ends: &[End]) -> Vec<Outcome> {
    let mut result = Vec::new();
    for end in ends {
        let mut position = end.game.position.clone();
        position.sort_unordered_piles();
        let trackers = &end
            .game
            .knowledge
            .as_ref()
            .expect("tracked oracle")
            .trackers;
        let beliefs = std::array::from_fn(|observer| {
            crate::tracking::zones()
                .into_iter()
                .map(|zone| trackers[observer].belief(zone))
                .collect()
        });
        let item = Outcome {
            position,
            rng: end.game.rng.clone(),
            status: end.status.clone(),
            beliefs,
            views: Player::ALL.map(|p| {
                end.game
                    .observe(p)
                    .unwrap()
                    .row()
                    .into_iter()
                    .map(f32::to_bits)
                    .collect()
            }),
        };
        if !result.contains(&item) {
            result.push(item);
        }
    }
    result
}
fn oracle(game: Game) -> (Vec<End>, Vec<End>) {
    assert!(
        game.knowledge.is_some(),
        "attach tracking before the oracle invocation"
    );
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    let reduced = paths(game, Vec::new(), 0);
    let unrestricted = paths(manual, Vec::new(), 0);
    let a = outcomes(&reduced);
    let b = outcomes(&unrestricted);
    assert_eq!(a.len(), b.len());
    assert!(a.iter().all(|item| b.contains(item)));
    let mut traces = Vec::new();
    for end in &unrestricted {
        if !traces.contains(&end.events.as_slice()) {
            traces.push(end.events.as_slice());
        }
    }
    assert!(
        reduced
            .iter()
            .all(|end| traces.contains(&end.events.as_slice())),
        "every canonical execution preserves an actual unrestricted event trace"
    );
    (reduced, unrestricted)
}

#[test]
fn drone_origin_shortcut_oracle_covers_actual_maxwell_first_draws() {
    for reacting in [false, true] {
        let (mut f, source, _, _, _, _) = drone_setup(0);
        if reacting {
            figure(&mut f, Figure::JamesClerkMaxwell, A);
            for _ in 0..5 {
                deck(&mut f, CardSet::Base, 8, Color::Purple);
            }
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let (reduced, manual) = oracle(game);
        assert_eq!(manual.len(), 120);
        if reacting {
            assert!(reduced.iter().all(|end|end.events.iter().any(|e|matches!(e,Event::Moved(m) if m.verb.is_none() && m.inside_karma && m.destination==Zone::Hand(A)))));
        } else {
            assert!(reduced.len() < manual.len());
        }
    }
}
#[test]
fn exoskeleton_any_order_oracle_covers_actual_nobel_first_junk() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E107", Some(Zone::Board(B, Color::Red)));
        tech(&mut f, Color::Blue, 1, Zone::Hand(A));
        tech(&mut f, Color::Green, 2, Zone::Hand(A));
        tech(&mut f, Color::Yellow, 3, Zone::Hand(A));
        let junked = deck(&mut f, CardSet::Base, 7, Color::Purple);
        if reacting {
            figure(&mut f, Figure::AlfredNobel, B);
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut context = Context::new(A);
        context.demander = B;
        start(&mut game, source, 0, context, 0);
        let (reduced, manual) = oracle(game);
        assert_eq!(manual.len(), 2);
        if reacting {
            assert!(reduced.iter().all(|end| {
                end.events
                    .iter()
                    .any(|e| matches!(e,Event::Moved(m) if m.card==junked&&m.inside_karma))
            }));
        } else {
            assert_eq!(reduced.len(), 1);
        }
    }
}

fn pending_claim(game: &Game) -> (CardId, OperandKnowledge) {
    game.frames
        .iter()
        .find_map(|frame| match frame {
            Frame::Operation {
                request:
                    Operation::Claim {
                        target,
                        eligibility: ClaimEligibility::Cost { score_factor: 1 },
                    },
                knowledge: Some(knowledge),
                ..
            } => Some((target.card, *knowledge)),
            _ => None,
        })
        .expect("original checked claim retained through First")
}
#[test]
fn exoskeleton_blind_hand_claim_is_sampled_once_and_stays_private_through_real_firsts() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let source = card(&mut f, "E107", None);
        tech(&mut f, Color::Purple, 10, Zone::Score(A));
        let hidden = [
            tech(&mut f, Color::Blue, 2, Zone::Hand(B)),
            tech(&mut f, Color::Yellow, 2, Zone::Hand(B)),
        ];
        figure(&mut f, Figure::HedyLamarr, A);
        figure(&mut f, Figure::ZhangHeng, B);
        tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        deck(&mut f, CardSet::Base, 3, Color::Purple);
        let mut game = Game::seeded(f.position(), BOTH, 93).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        start(&mut game, source, 1, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(
            menu.options,
            [
                Alternative::BlindMove {
                    verb: Verb::Achieve,
                    source: Zone::Hand(B),
                    back: Deck::base(2)
                },
                Alternative::Decline
            ]
        );
        let mut expected_rng = game.rng.clone();
        let chosen = hidden[(expected_rng.next_u64() % 2) as usize];
        game.choose(0).unwrap();
        assert_eq!(game.rng, expected_rng);
        let first = decision(&mut game, &mut events);
        assert_eq!(first.player, A);
        assert_eq!(first.options.len(), 2);
        assert!(
            first
                .options
                .iter()
                .all(|o| matches!(o, Alternative::Karma { .. }))
        );
        let (actual, knowledge) = pending_claim(&game);
        assert_eq!(actual, chosen);
        assert!(!knowledge.evidence.known(A));
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed { .. })));
        assert_eq!(game.rng, expected_rng);
        let mut completed = game.clone();
        completed.choose(0).unwrap();
        finish(&mut completed, &mut Vec::new(), |_| 0);
        let final_rng = completed.rng.clone();
        if tracked {
            let mut seen = Vec::new();
            for seed in 0..32 {
                let sampled = game.determinize(A, seed).unwrap();
                assert_eq!(sampled.observe(A).unwrap(), game.observe(A).unwrap());
                for mut sampled in [sampled, game.sample_world(A, seed).unwrap().world] {
                    let (target, evidence) = pending_claim(&sampled);
                    assert!(!evidence.evidence.known(A));
                    assert!(hidden.contains(&target));
                    if !seen.contains(&target) {
                        seen.push(target);
                    }
                    let sampled_rng = sampled.rng.clone();
                    sampled.choose(0).unwrap();
                    finish(&mut sampled, &mut Vec::new(), |_| 0);
                    assert_eq!(sampled.position.cards(Zone::Achievements(A)), [target]);
                    assert_eq!(sampled.rng, sampled_rng);
                }
            }
            assert_eq!(
                seen.len(),
                2,
                "both anonymous original identities remain possible"
            );
        }
        game.choose(0).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Achievements(A)), [chosen]);
        assert_eq!(game.rng, final_rng);
    }
}
#[test]
fn exoskeleton_foreseen_failed_original_claim_stops_despite_other_eligible_hand() {
    let mut f = Fixture::default();
    card(&mut f, "E107", Some(Zone::Forecast(A)));
    figure(&mut f, Figure::RowlandHill, A);
    tech(&mut f, Color::Purple, 10, Zone::Score(A));
    let own: Vec<_> = (1..=3)
        .map(|v| tech(&mut f, Color::Blue, v, Zone::Hand(A)))
        .collect();
    let other = tech(&mut f, Color::Yellow, 2, Zone::Hand(B));
    let promotion = tech(&mut f, Color::Purple, 11, Zone::Hand(A));
    let mut game = Game::new(f.position(), BOTH).unwrap();
    game.take_action(Action::Meld { card: promotion }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    game.choose(pick(&menu, own[0])).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert!(game.position.cards(Zone::Achievements(A)).is_empty());
    assert_eq!(game.position.locate(other), Some(Zone::Hand(B)));
    assert!(own.iter().all(|c|events.iter().any(|e|matches!(e,Event::Moved(m) if m.card==*c && m.verb==Some(Verb::Return) && m.inside_karma))));
}

#[test]
fn frozen_originals_retain_source_after_yi_tucks_push_them_beyond_bottom_five() {
    use crate::program::build::*;
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Green,
        11,
        Zone::Board(A, Color::Green),
        vec![
            transfer(
                Selector::Board(
                    Who::Executor,
                    ColorSet::Only(&[Color::Blue]),
                    BoardRegion::BottomN(5),
                ),
                Destination::Score(Who::Executor),
            )
            .all()
            .frozen(),
        ],
    );
    let originals: Vec<_> = (1..=6)
        .map(|v| tech(&mut f, Color::Blue, v, Zone::Board(A, Color::Blue)))
        .collect();
    figure(&mut f, Figure::YiSunSin, A);
    for _ in 0..5 {
        deck(&mut f, CardSet::Base, 4, Color::Blue);
    }
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    let tucked: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m)
                if m.verb == Some(Verb::Tuck) && m.inside_karma && originals.contains(&m.card) =>
            {
                Some(m.card)
            }
            _ => None,
        })
        .collect();
    assert_eq!(tucked.len(), 5);
    assert!(originals[..5].iter().all(|c| tucked.contains(c)));
    assert!(!tucked.contains(&originals[5]));
    assert_eq!(game.position.cards(Zone::Board(A, Color::Blue)).len(), 11);
    assert!(game.position.cards(Zone::Score(A)).is_empty());
}

#[test]
fn frozen_per_color_ignores_unneeded_peers_without_failing_success() {
    use crate::program::build::*;
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Red,
        11,
        Zone::Board(A, Color::Red),
        vec![return_(HAND).per_color(None).frozen(), if_(DID, 1), draw(3)],
    );
    tech(&mut f, Color::Blue, 1, Zone::Hand(A));
    tech(&mut f, Color::Blue, 2, Zone::Hand(A));
    tech(&mut f, Color::Green, 1, Zone::Hand(A));
    let reward = deck(&mut f, CardSet::Base, 3, Color::Yellow);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    start(&mut game, source, 0, Context::new(A), 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(returned(&events).len(), 2);
    assert!(game.position.cards(Zone::Hand(A)).contains(&reward));
}

#[test]
fn frozen_per_color_defers_lost_peer_until_peters_remaining_original_can_return() {
    use crate::program::build::*;
    for (green_count, lose_chosen) in [(1, false), (3, false), (3, true)] {
        let mut f = Fixture::default();
        let source = f.tech(
            Color::Purple,
            11,
            Zone::Board(A, Color::Purple),
            vec![
                return_(Selector::Board(
                    Who::Executor,
                    ColorSet::Only(&[Color::Blue, Color::Green]),
                    BoardRegion::BelowTop(0),
                ))
                .per_color(None)
                .frozen(),
                if_(DID, 1),
                draw(8),
            ],
        );
        let blue = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        let green: Vec<_> = (0..green_count)
            .map(|_| tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green)))
            .collect();
        figure(&mut f, Figure::PeterTheGreat, A);
        let reward = deck(&mut f, CardSet::Base, 8, Color::Yellow);
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        game.choose(pick(&menu, blue)).unwrap();
        finish(&mut game, &mut events, |menu| {
            pick(
                menu,
                if lose_chosen {
                    green[1]
                } else {
                    *green.last().unwrap()
                },
            )
        });
        assert_eq!(
            returned(&events),
            if green_count == 3 && !lose_chosen {
                vec![blue, green[2]]
            } else {
                vec![blue]
            }
        );
        assert_eq!(
            game.position.cards(Zone::Hand(A)).contains(&reward),
            green_count == 3 && !lose_chosen
        );
        assert_eq!(
            game.position.cards(Zone::Score(A)),
            &green[..green_count.min(2)]
        );
    }
}

#[test]
fn frozen_quota_defers_unselected_losses_but_preserves_a_real_deficit() {
    use crate::program::build::*;
    for (green_count, lose_chosen) in [(1, false), (3, false), (3, true)] {
        let mut f = Fixture::default();
        let source = f.tech(
            Color::Purple,
            11,
            Zone::Board(A, Color::Purple),
            vec![
                return_(Selector::Board(
                    Who::Executor,
                    ColorSet::Only(&[Color::Blue, Color::Green]),
                    BoardRegion::BelowTop(0),
                ))
                .exactly(Expr::Const(2))
                .frozen(),
                if_(DID, 1),
                draw(8),
            ],
        );
        let blue = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        let green: Vec<_> = (0..green_count)
            .map(|_| tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green)))
            .collect();
        figure(&mut f, Figure::PeterTheGreat, A);
        let reward = deck(&mut f, CardSet::Base, 8, Color::Yellow);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        game.choose(pick(&menu, blue)).unwrap();
        finish(&mut game, &mut events, |menu| {
            pick(
                menu,
                if lose_chosen {
                    green[1]
                } else {
                    *green.last().unwrap()
                },
            )
        });
        assert_eq!(
            returned(&events),
            if green_count == 3 && !lose_chosen {
                vec![blue, green[2]]
            } else {
                vec![blue]
            }
        );
        assert_eq!(
            game.position.cards(Zone::Hand(A)).contains(&reward),
            green_count == 3 && !lose_chosen
        );
    }
}

#[test]
fn frozen_up_to_has_no_debt_for_unselected_lost_members_or_zero_choice() {
    use crate::program::build::*;
    for decline_all in [false, true] {
        let mut f = Fixture::default();
        let source = f.tech(
            Color::Purple,
            11,
            Zone::Board(A, Color::Purple),
            vec![
                return_(Selector::Board(
                    Who::Executor,
                    ColorSet::Only(&[Color::Blue, Color::Green]),
                    BoardRegion::BelowTop(0),
                ))
                .up_to(None)
                .frozen(),
                if_(DID, 1),
                draw(8),
            ],
        );
        let blue = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue));
        let green = tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
        figure(&mut f, Figure::PeterTheGreat, A);
        let reward = deck(&mut f, CardSet::Base, 8, Color::Yellow);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 0, Context::new(A), 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        game.choose(if decline_all {
            decline(&menu)
        } else {
            pick(&menu, blue)
        })
        .unwrap();
        finish(&mut game, &mut events, decline);
        assert_eq!(
            returned(&events),
            if decline_all { vec![] } else { vec![blue] }
        );
        assert_eq!(
            game.position.cards(Zone::Hand(A)).contains(&reward),
            !decline_all
        );
        assert_eq!(
            game.position.locate(green),
            Some(if decline_all {
                Zone::Board(A, Color::Green)
            } else {
                Zone::Score(A)
            })
        );
    }
}
