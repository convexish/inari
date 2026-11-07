use inno_engine::{Action, CardSet, Expansions, Match, Prompt, Record, SessionError, Step};

const MODES: [(&str, Expansions); 4] = [
    ("echoes", Expansions::ECHOES),
    (
        "figures+echoes",
        Expansions::ECHOES.union(Expansions::FIGURES),
    ),
    (
        "cities+echoes",
        Expansions::ECHOES.union(Expansions::CITIES),
    ),
    (
        "figures+cities+echoes",
        Expansions::ECHOES
            .union(Expansions::FIGURES)
            .union(Expansions::CITIES),
    ),
];

#[test]
fn echoes_records_round_trip_publisher_codes_in_every_enabled_catalog() {
    for (name, expansions) in MODES {
        let mut text = format!(
            "inari-next-record 1\nmode {name}\nseed 18446744073709551615\n\
             choice 0\nchoice 1\ndraw\nmeld E001\ndogma E070\nachieve E096\n"
        );
        if expansions.figures() {
            text.push_str("meld F052\ndogma Z001\n");
        }
        if expansions.cities() {
            text.push_str("meld C035\n");
        }
        let record = Record::from_text(&text).unwrap();
        assert_eq!(record.expansions, expansions);
        assert_eq!(record.to_text().unwrap(), text);
        let session = Match::new(expansions, record.seed).unwrap();
        for (index, code) in [(3, "E001"), (4, "E070"), (5, "E096")] {
            let card = match record.steps[index] {
                Step::Action(Action::Meld { card })
                | Step::Action(Action::Dogma { card, .. })
                | Step::Action(Action::Achieve { card }) => card,
                other => panic!("unexpected decoded step {other:?}"),
            };
            assert_eq!(session.game().position().card(card).code, code);
        }
        for forbidden in [
            (!expansions.figures()).then_some("F052"),
            (!expansions.cities()).then_some("C035"),
            Some("A001"),
            Some("U001"),
        ]
        .into_iter()
        .flatten()
        {
            let invalid = format!("inari-next-record 1\nmode {name}\nseed 7\nmeld {forbidden}\n");
            assert!(matches!(
                Record::from_text(&invalid),
                Err(SessionError::InvalidRecord { line: 4, .. })
            ));
        }
    }
    assert!(Record::from_text("inari-next-record 1\nmode base\nseed 7\nmeld E001\n").is_err());
}

#[test]
fn echoes_native_histories_replay_undo_and_reject_invalid_inputs_atomically() {
    for (_, expansions) in MODES {
        let mut session = Match::new(expansions, 7).unwrap();
        let mut actions_started = 0;
        let mut echoes_actions = 0;
        for index in 0..36 {
            let step = match session.prompt() {
                Prompt::Decision(_) => Step::Choice(0),
                Prompt::Turn { actions, .. } => {
                    let preferred = actions.iter().find(|action| match action {
                        Action::Meld { card } if actions_started % 3 == 1 => {
                            session.game().position().card(*card).set == CardSet::Echoes
                        }
                        Action::Dogma { card, .. } if actions_started % 3 == 2 => {
                            session.game().position().card(*card).set == CardSet::Echoes
                        }
                        _ => false,
                    });
                    let action = preferred.copied().unwrap_or(Action::Draw);
                    if let Action::Meld { .. } | Action::Dogma { .. } = action {
                        echoes_actions += 1;
                    }
                    actions_started += 1;
                    Step::Action(action)
                }
                Prompt::Over(_) => break,
            };
            session.apply(step).unwrap();
            if index % 8 == 7 {
                let decoded = Record::from_text(&session.record().to_text().unwrap()).unwrap();
                assert_eq!(Match::from_record(&decoded).unwrap(), session);
            }
        }
        assert!(echoes_actions > 0, "must record actual Echoes card actions");
        let decoded = Record::from_text(&session.record().to_text().unwrap()).unwrap();
        assert_eq!(Match::from_record(&decoded).unwrap(), session);
        let before = session.clone();
        assert_eq!(
            session.apply(Step::Choice(usize::MAX)),
            Err(SessionError::InvalidStep)
        );
        assert_eq!(session, before);
        let steps = session.record().steps.len() / 2;
        let mut prefix = session.record().clone();
        prefix.steps.truncate(steps);
        session.undo_to(steps).unwrap();
        assert_eq!(session, Match::from_record(&prefix).unwrap());
    }
}
