use std::fmt::Write as _;

use inno_engine::*;

const SEEDS: u64 = 25;
const MAX_BOUNDARIES: usize = 20_000;
const WORK: usize = 4_096;

struct Digest(u64);

impl Digest {
    fn add(&mut self, value: &impl std::fmt::Debug) {
        for byte in format!("{value:?};").bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn state(&mut self, p: &Position) {
        self.add(&p.turn_owner());
        self.add(&p.regular_actions_started());
        self.add(&p.endorsement_used());
        self.add(&p.age_zero_introduced());
        for zone in zones() {
            self.add(&p.cards(zone));
        }
        for player in Player::ALL {
            self.add(&p.score(player));
            self.add(&p.effective_achievement_count(player));
            self.add(&p.forecast_limit(player));
            for color in Color::ALL {
                self.add(&p.splay(player, color));
            }
        }
        for special in p.specials() {
            self.add(&p.special_location(special));
        }
        for decree in Decree::ALL {
            self.add(&p.decree_location(decree));
        }
    }
}

fn zones() -> Vec<Zone> {
    let mut zones = vec![
        Zone::Junk,
        Zone::AvailableAchievements,
        Zone::AgeZeroReserve,
    ];
    for player in Player::ALL {
        zones.extend([
            Zone::Hand(player),
            Zone::Score(player),
            Zone::Achievements(player),
            Zone::Forecast(player),
        ]);
        zones.extend(Color::ALL.map(|color| Zone::Board(player, color)));
    }
    for set in CardSet::ALL {
        zones.extend((0..=11).map(|age| Zone::Deck(Deck { set, age })));
    }
    zones
}

fn invariant(game: &Game, mode: &str, seed: u64, boundary: usize) {
    let p = game.position();
    assert!(
        p.check_conservation(),
        "{mode} seed {seed} boundary {boundary}"
    );
    for zone in zones() {
        for &id in p.cards(zone) {
            let card = p.card(id);
            assert!(p.expansions().includes(card.set));
            match zone {
                Zone::Deck(deck) => {
                    assert_eq!((card.set, card.value), (deck.set, deck.age));
                }
                Zone::Board(_, color) => assert_eq!(card.color, color),
                Zone::AgeZeroReserve => assert_eq!((card.set, card.value), (CardSet::Base, 0)),
                _ => {}
            }
        }
    }
    for tracker in Player::ALL
        .into_iter()
        .filter_map(|p| game.tracker(p))
        .chain(game.public_tracker())
    {
        assert!(
            tracker.check_truth(p),
            "{mode} seed {seed} boundary {boundary}"
        );
    }
}

#[derive(Debug, Default)]
struct Witness {
    echo_draws: usize,
    forecast_moves: usize,
    promotions: usize,
    top_echo_dogmas: usize,
    figure_reactions: usize,
    city_draws: usize,
}

impl Witness {
    fn events(&mut self, events: &[Event]) {
        for event in events {
            if let Event::Moved(m) = event {
                self.echo_draws += usize::from(
                    m.verb.is_none()
                        && matches!(
                            m.source,
                            Zone::Deck(Deck {
                                set: CardSet::Echoes,
                                ..
                            })
                        ),
                );
                self.city_draws += usize::from(
                    m.verb.is_none()
                        && matches!(
                            m.source,
                            Zone::Deck(Deck {
                                set: CardSet::Cities,
                                ..
                            })
                        ),
                );
                self.forecast_moves += usize::from(matches!(m.destination, Zone::Forecast(_)));
                self.promotions += usize::from(
                    m.verb == Some(Verb::Meld) && matches!(m.source, Zone::Forecast(_)),
                );
                self.figure_reactions += usize::from(m.inside_karma);
            }
        }
    }
}

fn trace(seed: u64, expansions: Expansions, mode: &str, witness: &mut Witness) -> String {
    let mut game = Game::new_game(seed, expansions).unwrap();
    // One fully tracked seeded game in each mode supplements the dedicated
    // observation/search acceptance. All games check physical invariants.
    if seed == 0 {
        game = game.with_tracking().unwrap();
    }
    let mut policy = Rng::new(seed ^ 0x0005_EED0_FA11);
    let mut digest = Digest(0xcbf2_9ce4_8422_2325);
    digest.state(game.position());
    for boundary in 0..MAX_BOUNDARIES {
        let mut clone = (boundary % 64 == 0).then(|| game.clone());
        let mut events = Vec::new();
        let status = game.advance(WORK, &mut events).unwrap_or_else(|error| {
            panic!("{mode} seed {seed} boundary {boundary}: {error:?}");
        });
        if let Some(clone) = clone.as_mut() {
            let mut cloned_events = Vec::new();
            assert_eq!(clone.advance(WORK, &mut cloned_events).unwrap(), status);
            assert_eq!(cloned_events, events);
            assert_eq!(*clone, game);
        }
        invariant(&game, mode, seed, boundary);
        witness.events(&events);
        digest.add(&events);
        digest.add(&status);
        digest.state(game.position());
        match status {
            Status::Decision(decision) => {
                assert!(
                    !decision.options.is_empty(),
                    "{mode} seed {seed} empty decision"
                );
                let index = (policy.next_u64() % decision.options.len() as u64) as usize;
                digest.add(&index);
                game.choose(index).unwrap();
            }
            Status::Complete(_) => {
                let actions = game.legal_actions();
                assert!(
                    !actions.is_empty(),
                    "{mode} seed {seed} nonterminal dead end"
                );
                let action = actions[(policy.next_u64() % actions.len() as u64) as usize];
                if let Action::Dogma { card, .. } = action {
                    witness.top_echo_dogmas +=
                        usize::from(game.position().card(card).echo.is_some());
                }
                digest.add(&actions);
                digest.add(&action);
                game.take_action(action, ActionKind::Regular).unwrap();
            }
            Status::Winner(_) | Status::Draw(_) => {
                digest.add(&game.victory());
                return format!("{seed} {:016x} {}\n", digest.0, boundary + 1);
            }
            Status::WorkLimit => {}
        }
        invariant(&game, mode, seed, boundary);
    }
    panic!(
        "{mode} seed {seed} did not reach a terminal result within {MAX_BOUNDARIES} bounded advances"
    );
}

fn recorded(mode: &str, expansions: Expansions) {
    let mut witness = Witness::default();
    let mut actual = String::new();
    for seed in 0..SEEDS {
        actual.push_str(&trace(seed, expansions, mode, &mut witness));
    }
    assert!(
        witness.echo_draws > 0
            && witness.forecast_moves > 0
            && witness.promotions > 0
            && witness.top_echo_dogmas > 0,
        "{mode}: {witness:?}"
    );
    if expansions.figures() {
        assert!(witness.figure_reactions > 0, "{mode}: {witness:?}");
    }
    if expansions.cities() {
        assert!(witness.city_draws > 0, "{mode}: {witness:?}");
    }
    writeln!(actual, "# {witness:?}").unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/random_{mode}_traces.txt"));
    if std::env::var_os("UPDATE_ECHOES_TRACES").is_some() {
        std::fs::write(path, actual).unwrap();
    } else {
        let expected = std::fs::read_to_string(path).expect("frozen Echoes traces");
        for (want, got) in expected.lines().zip(actual.lines()) {
            assert_eq!(
                want, got,
                "{mode}: first diverging seed or witnessed behavior"
            );
        }
        assert_eq!(expected.lines().count(), actual.lines().count());
    }
}

#[test]
fn seeded_echoes_games_finish_and_match_native_traces() {
    recorded("echoes", Expansions::ECHOES);
}

#[test]
fn seeded_echoes_figures_games_finish_and_match_native_traces() {
    recorded(
        "echoes_figures",
        Expansions::ECHOES.union(Expansions::FIGURES),
    );
}

#[test]
fn seeded_echoes_cities_games_finish_and_match_native_traces() {
    recorded(
        "echoes_cities",
        Expansions::ECHOES.union(Expansions::CITIES),
    );
}

#[test]
fn seeded_echoes_figures_cities_games_finish_and_match_native_traces() {
    recorded(
        "echoes_figures_cities",
        Expansions::ECHOES
            .union(Expansions::FIGURES)
            .union(Expansions::CITIES),
    );
}
