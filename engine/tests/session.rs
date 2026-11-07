use inno_engine::{
    Action, Alternative, CardId, DogmaKind, Event, Expansions, Match, Player, Prompt, Record, Rng,
    SessionError, Step, Verb, Zone,
};

/// Every supported deal, as the record names it.
const MODES: [(&str, Expansions); 4] = [
    ("base", Expansions::BASE),
    ("figures", Expansions::FIGURES),
    ("cities", Expansions::CITIES),
    (
        "figures+cities",
        Expansions::FIGURES.union(Expansions::CITIES),
    ),
];

fn assert_same(actual: &Match, expected: &Match) {
    assert_eq!(actual.game(), expected.game());
    assert_eq!(actual.prompt(), expected.prompt());
    assert_eq!(actual.record(), expected.record());
    assert_eq!(actual.events(), expected.events());
    assert_eq!(actual.to_move(), expected.to_move());
}

fn prefix(record: &Record, steps: usize) -> Record {
    Record {
        expansions: record.expansions,
        seed: record.seed,
        subgames: record.subgames,
        steps: record.steps[..steps].to_vec(),
    }
}

#[test]
fn text_round_trip_all_step_kinds_and_catalogs() {
    for (mode, expansions) in MODES {
        let mut extra = String::new();
        if expansions.figures() {
            extra += "meld F052\ndogma Z001\n";
        }
        if expansions.cities() {
            extra += "meld C035\n";
        }
        let text = format!(
            "inari-next-record 1\nmode {mode}\nseed 18446744073709551615\n\
             choice 0\nchoice 42\ndraw\nmeld B004\nachieve B005\ndogma B006\n\
             dogma-auspice B007\ndecree\n{extra}"
        );
        let record = Record::from_text(&text).unwrap();
        assert_eq!(record.expansions, expansions);
        assert_eq!(record.seed, u64::MAX);
        assert_eq!(record.steps[0], Step::Choice(0));
        assert_eq!(record.steps[1], Step::Choice(42));
        assert_eq!(record.steps[2], Step::Action(Action::Draw));
        assert_eq!(record.steps[7], Step::Action(Action::Decree));
        let game = Match::new(record.expansions, record.seed).unwrap();
        for (index, code) in [(3, "B004"), (4, "B005"), (5, "B006"), (6, "B007")] {
            let card = match record.steps[index] {
                Step::Action(Action::Meld { card }) if index == 3 => card,
                Step::Action(Action::Achieve { card }) if index == 4 => card,
                Step::Action(Action::Dogma { card, auspice }) if index == 5 || index == 6 => {
                    assert_eq!(auspice, index == 6);
                    card
                }
                _ => panic!("wrong decoded step"),
            };
            assert_eq!(game.game().position().card(card).code, code);
        }
        assert_eq!(record.to_text().unwrap(), text);
        assert_eq!(
            Record::from_text(&record.to_text().unwrap()).unwrap(),
            record
        );
        assert_eq!(Record::from_text(text.trim_end()).unwrap(), record);
        assert_eq!(
            Record::from_text(&text.replace('\n', "\r\n")).unwrap(),
            record
        );
    }
    // Every combination saves and loads, under a distinct name.
    let mut names = Vec::new();
    for bits in 0..Expansions::COMBINATIONS as u8 {
        let expansions = Expansions::from_bits(bits).unwrap();
        let record = Match::new(expansions, 0).unwrap().record().clone();
        let text = record.to_text().unwrap();
        assert_eq!(Record::from_text(&text).unwrap(), record);
        let name = text.lines().nth(1).unwrap().to_owned();
        assert_eq!(name.contains("unseen"), expansions.unseen(), "{name}");
        names.push(name);
    }
    assert!(names.contains(&"mode figures+cities+echoes+artifacts+unseen".to_owned()));
    names.sort();
    names.dedup();
    assert_eq!(names.len(), Expansions::COMBINATIONS);
}

#[test]
fn malformed_text_and_unknown_or_wrong_mode_codes_are_errors() {
    let header = "inari-next-record 1\nmode base\nseed 0\n";
    for text in [
        "".to_owned(),
        header.replace("record 1", "record 2"),
        header.replace("mode base", "mode unseen+base"),
        header.replace("mode base", "mode echoes+figures"),
        header.replace("seed 0", "seed -1"),
        header.replace("seed 0", "seed 18446744073709551616"),
        "inari-next-record 1\nmode base\n".to_owned(),
    ] {
        assert!(matches!(
            Record::from_text(&text),
            Err(SessionError::InvalidRecord { .. })
        ));
    }
    for line in [
        "meld UNKNOWN",
        "meld 4",
        "meld F052",
        "dogma Z001",
        "meld C035",
        "choice -1",
        "choice 18446744073709551616",
        "choice",
        "draw extra",
        "decree extra",
        "meld",
        "meld B004 extra",
        "dogma-auspice",
        "unknown",
        "mode figures",
        "",
    ] {
        assert!(
            matches!(
                Record::from_text(&format!("{header}{line}\n")),
                Err(SessionError::InvalidRecord { line: 4, .. })
            ),
            "{line:?}"
        );
    }
    let record = Record {
        expansions: Expansions::BASE,
        seed: 0,
        subgames: false,
        steps: vec![Step::Action(Action::Meld {
            card: CardId(u16::MAX),
        })],
    };
    assert!(matches!(
        record.to_text(),
        Err(SessionError::InvalidRecord { line: 4, .. })
    ));
}

#[test]
fn random_replays_preserve_game_prompt_and_tagged_events() {
    for (_, expansions) in MODES {
        for seed in 0..6 {
            let mut session = Match::new(expansions, seed).unwrap();
            let mut policy = Rng::new(seed ^ 0x5EED);
            for _ in 0..160 {
                let Some(step) = session.random_step(policy.next_u64()) else {
                    break;
                };
                let previous_events = session.events().len();
                session.apply(step).unwrap();
                let tag = session.record().steps.len();
                assert!(
                    session.events()[previous_events..]
                        .iter()
                        .all(|(index, _)| *index == tag)
                );
                match session.prompt() {
                    Prompt::Decision(decision) => {
                        assert_eq!(session.to_move(), Some(decision.player))
                    }
                    Prompt::Turn { player, .. } => {
                        assert_eq!(*player, session.game().position().turn_owner());
                        assert_eq!(session.to_move(), Some(*player));
                    }
                    Prompt::Over(_) => assert_eq!(session.to_move(), None),
                }
            }
            let replay = Match::from_record(session.record()).unwrap();
            assert_same(&session, &replay);
            let decoded = Record::from_text(&session.record().to_text().unwrap()).unwrap();
            assert_same(&session, &Match::from_record(&decoded).unwrap());
        }
    }
}

#[test]
fn undo_replays_opening_and_later_decisions_and_allows_branching() {
    for (_, expansions) in MODES {
        let mut session = Match::new(expansions, 7).unwrap();
        assert_eq!(session.to_move(), Some(Player::One));
        session.apply(Step::Choice(0)).unwrap();
        assert_eq!(session.to_move(), Some(Player::Two));
        let after_first = session.clone();
        session.apply(Step::Choice(0)).unwrap();
        assert!(matches!(session.prompt(), Prompt::Turn { .. }));
        assert!(!session.events().is_empty());
        assert!(session.events().iter().all(|(tag, _)| *tag == 2));
        let after_opening = session.clone();
        let mut after_decision = None;
        let mut policy = Rng::new(88);
        for _ in 0..400 {
            let Some(step) = session.random_step(policy.next_u64()) else {
                break;
            };
            let decision = matches!(session.prompt(), Prompt::Decision(_));
            session.apply(step).unwrap();
            if decision && after_decision.is_none() {
                after_decision = Some(session.clone());
            }
        }
        let after_decision = after_decision.expect("encounter a post-opening decision");
        let record = session.record().clone();
        for k in [
            record.steps.len(),
            after_decision.record().steps.len(),
            2,
            1,
            0,
        ] {
            session.undo_to(k).unwrap();
            assert_same(&session, &Match::from_record(&prefix(&record, k)).unwrap());
            if k == 1 {
                assert_same(&session, &after_first);
            }
            if k == 2 {
                assert_same(&session, &after_opening);
            }
            if k == after_decision.record().steps.len() {
                assert_same(&session, &after_decision);
            }
        }
        session.apply(Step::Choice(1)).unwrap();
        assert_eq!(session.record().steps, vec![Step::Choice(1)]);
        assert_same(&session, &Match::from_record(session.record()).unwrap());
    }
}

fn reject_unchanged(session: &mut Match, step: Step) {
    let before = session.clone();
    assert_eq!(session.apply(step), Err(SessionError::InvalidStep));
    assert_same(session, &before);
    assert_eq!(*session, before);
}

#[test]
fn invalid_inputs_and_undo_are_atomic_and_replay_validates_every_step() {
    for (_, expansions) in MODES {
        let mut session = Match::new(expansions, 0).unwrap();
        reject_unchanged(&mut session, Step::Action(Action::Draw));
        let Prompt::Decision(decision) = session.prompt() else {
            panic!("opening choice")
        };
        let out_of_range = decision.options.len();
        reject_unchanged(&mut session, Step::Choice(out_of_range));
        reject_unchanged(&mut session, Step::Choice(usize::MAX));
        session.apply(Step::Choice(0)).unwrap();
        session.apply(Step::Choice(0)).unwrap();
        reject_unchanged(&mut session, Step::Choice(0));
        for action in [
            Action::Meld {
                card: CardId(u16::MAX),
            },
            Action::Achieve {
                card: CardId(u16::MAX),
            },
            Action::Dogma {
                card: CardId(u16::MAX),
                auspice: false,
            },
            Action::Decree,
        ] {
            reject_unchanged(&mut session, Step::Action(action));
        }
        let before = session.clone();
        assert_eq!(session.undo_to(3), Err(SessionError::InvalidUndo));
        assert_eq!(session, before);
        let mut invalid = session.record().clone();
        invalid.steps.push(Step::Action(Action::Draw)); // Valid third input.
        invalid.steps.push(Step::Choice(usize::MAX));
        assert_eq!(Match::from_record(&invalid), Err(SessionError::InvalidStep));
    }
}

#[test]
fn random_step_is_legal_repeatable_and_read_only() {
    let mut session = Match::new(Expansions::FIGURES, 42).unwrap();
    for _ in 0..100 {
        let before = session.clone();
        let step = session.random_step(37).unwrap();
        assert_eq!(session.random_step(37), Some(step));
        assert_eq!(session, before);
        session.apply(step).unwrap();
        if matches!(session.prompt(), Prompt::Over(_)) {
            break;
        }
    }
}

#[test]
fn one_hundred_complete_random_games() {
    // Each game runs on its own thread.
    std::thread::scope(|scope| {
        for (_, expansions) in MODES {
            for seed in 0..50 {
                scope.spawn(move || complete_random_game(expansions, seed));
            }
        }
    });
}

fn complete_random_game(expansions: Expansions, seed: u64) {
    let mut session = Match::new(expansions, seed).unwrap();
    let mut policy = Rng::new(seed ^ 0x5EED_FA11);
    for _ in 0..20_000 {
        let Some(step) = session.random_step(policy.next_u64()) else {
            break;
        };
        session.apply(step).unwrap_or_else(|error| {
            panic!(
                "{expansions:?} seed={seed} step={}: {error}",
                session.record().steps.len()
            );
        });
    }
    assert!(
        matches!(session.prompt(), Prompt::Over(_)),
        "{expansions:?} seed={seed}"
    );
    assert_eq!(session.to_move(), None);
    assert_eq!(session.random_step(0), None);
    reject_unchanged(&mut session, Step::Choice(0));
    reject_unchanged(&mut session, Step::Action(Action::Draw));
}

#[test]
fn finished_games_with_unseen_save_and_replay() {
    for bits in [16u8, 31] {
        let expansions = Expansions::from_bits(bits).unwrap();
        for seed in 0..3 {
            let mut session = Match::new(expansions, seed).unwrap();
            let mut policy = Rng::new(seed);
            while let Some(step) = session.random_step(policy.next_u64()) {
                session.apply(step).unwrap();
            }
            let text = session.record().to_text().unwrap();
            let replayed = Match::from_record(&Record::from_text(&text).unwrap()).unwrap();
            assert_same(&replayed, &session);
        }
    }
}

#[test]
fn turn_and_free_dogma_events_mark_turn_starts_promotions_and_showcases() {
    let (mut promoted, mut showcased) = (0, 0);
    for bits in [0b01100u8, 0b11111] {
        let expansions = Expansions::from_bits(bits).unwrap();
        for seed in 0..12 {
            let mut session = Match::new(expansions, seed).unwrap();
            let mut policy = Rng::new(seed);
            while let Some(step) = session.random_step(policy.next_u64()) {
                session.apply(step).unwrap();
            }
            let mut turn_owner = None;
            let mut forecast_meld = None;
            for (_, event) in session.events() {
                match event {
                    Event::TurnBegan { player } => {
                        assert_ne!(turn_owner, Some(*player), "turns alternate");
                        turn_owner = Some(*player);
                    }
                    // A showcase rotates the turn owner's display at turn
                    // start, after its turn began.
                    Event::Moved(mv) if mv.verb == Some(Verb::Rotate) => {
                        assert_eq!(Some(mv.executor), turn_owner);
                    }
                    Event::Moved(mv)
                        if mv.verb == Some(Verb::Meld)
                            && matches!(mv.source, Zone::Forecast(_)) =>
                    {
                        forecast_meld = Some(mv.card);
                    }
                    Event::DogmaStarted {
                        player, card, kind, ..
                    } => match kind {
                        DogmaKind::Promoted => {
                            assert_eq!(forecast_meld, Some(*card));
                            promoted += 1;
                        }
                        DogmaKind::Showcased => {
                            assert_eq!(Some(*player), turn_owner);
                            showcased += 1;
                        }
                        DogmaKind::Action | DogmaKind::Free => {}
                    },
                    _ => {}
                }
            }
            assert!(turn_owner.is_some());
        }
    }
    assert!(
        promoted > 0 && showcased > 0,
        "{promoted} promoted, {showcased} showcased"
    );
}

#[test]
fn public_choices_are_announced_and_card_picks_are_not() {
    let (mut colors, mut picks) = (0, 0);
    for seed in 0..40 {
        if colors >= 3 && picks >= 3 {
            break;
        }
        let mut m = Match::new(Expansions::from_bits(31).unwrap(), seed).unwrap();
        for _ in 0..600 {
            let Some(step) = m.random_step(seed) else {
                break;
            };
            let announced = match (m.prompt(), step) {
                (Prompt::Decision(decision), Step::Choice(index)) => {
                    let public = matches!(decision.options[index], Alternative::Colors(_));
                    let card = matches!(
                        decision.options[index],
                        Alternative::Move { .. } | Alternative::BlindMove { .. }
                    );
                    colors += usize::from(public);
                    picks += usize::from(card);
                    Some((public, card, decision.player))
                }
                _ => None,
            };
            m.apply(step).unwrap();
            let Some((public, card, player)) = announced else {
                continue;
            };
            let tag = m.record().steps.len();
            let chose: Vec<_> = m
                .events()
                .iter()
                .filter(|(t, e)| *t == tag && matches!(e, Event::Chose { .. }))
                .collect();
            if public {
                assert!(matches!(chose[..], [(_, Event::Chose { player: p, .. })] if *p == player));
            }
            if card {
                assert!(chose.is_empty());
            }
        }
    }
    assert!(
        colors > 0 && picks > 0,
        "{colors} color choices, {picks} card picks"
    );
}

#[test]
fn the_printed_martian_internet_rule_is_recorded_and_replayed() {
    let text = "inari-next-record 1\nmode artifacts\nseed 7\nmartian-internet printed\nchoice 0\nchoice 0\n";
    let record = Record::from_text(text).unwrap();
    assert!(record.subgames);
    assert_eq!(record.steps, [Step::Choice(0), Step::Choice(0)]);
    assert_eq!(record.to_text().unwrap(), text);
    let replayed = Match::from_record(&record).unwrap();
    assert!(replayed.game().subgames());
    assert_eq!(replayed.record(), &record);
    // The default, and a record without the line, keep the draw variant.
    let plain = Record::from_text("inari-next-record 1\nmode artifacts\nseed 7\n").unwrap();
    assert!(!plain.subgames);
    assert!(
        !Match::new(Expansions::ARTIFACTS, 7)
            .unwrap()
            .game()
            .subgames()
    );
    // The line belongs before the steps, once.
    assert!(matches!(
        Record::from_text(
            "inari-next-record 1\nmode artifacts\nseed 7\ndraw\nmartian-internet printed\n"
        ),
        Err(SessionError::InvalidRecord { line: 5, .. })
    ));
}

#[test]
fn a_match_with_subgames_tags_the_subgame_events_a_duel_counts() {
    // Random play on this seed starts and finishes one subgame (the app's
    // log test uses the same recipe); the events a duel harness counts are
    // in the match log, tagged with the steps that produced them.
    const SEED: u64 = 245;
    let mut m = Match::with_subgames(Expansions::ARTIFACTS, SEED, true).unwrap();
    let mut k = 0;
    while !matches!(m.prompt(), Prompt::Over(_)) {
        let step = m.random_step(SEED * 100_003 + k).unwrap();
        k += 1;
        m.apply(step).unwrap();
    }
    let started: Vec<_> = m
        .events()
        .iter()
        .filter_map(|(tag, event)| match event {
            Event::SubgameStarted { depth, .. } => Some((*tag, *depth)),
            _ => None,
        })
        .collect();
    let ended: Vec<_> = m
        .events()
        .iter()
        .filter_map(|(tag, event)| match event {
            Event::SubgameEnded { depth, .. } => Some((*tag, *depth)),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 1);
    assert_eq!(ended.len(), 1);
    assert_eq!(started[0].1, 1);
    assert_eq!(ended[0].1, 1);
    assert!(0 < started[0].0 && started[0].0 < ended[0].0 && ended[0].0 <= k as usize);
}
