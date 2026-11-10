//! Seeded uniformly random games in every expansion combination, digested
//! boundary by boundary. The fixture pins every offered option, choice,
//! action, internal event and resulting position, so an interpreter or
//! card-language refactor must reproduce play exactly. A deliberate rules
//! change regenerates it: `UPDATE_TRACES=1 cargo test --test random_game_traces`.
//!
//! Each game also checks conservation, zone membership and tracker truth at
//! every boundary, and each mode must witness its sets' own mechanics.
//! `TRACE_SEEDS` and `TRACE_FIXTURE` select a larger out-of-tree comparison.
use std::fmt::Write as _;

use inno_engine::*;

const FIXTURE: &str = "tests/fixtures/random_game_traces.txt";
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
        self.add(&p.available_museums());
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
        zones.extend([
            Zone::Display(player),
            Zone::Museum(player),
            Zone::Safe(player),
        ]);
        zones.extend(Color::ALL.map(|color| Zone::Board(player, color)));
    }
    for set in CardSet::ALL {
        zones.extend((0..=11).map(|age| Zone::Deck(Deck { set, age })));
    }
    zones
}

fn invariant(game: &Game, context: &str) {
    let p = game.position();
    assert!(p.check_conservation(), "{context}");
    for zone in zones() {
        for &id in p.cards(zone) {
            let card = p.card(id);
            assert!(p.expansions().includes(card.set), "{context}");
            match zone {
                Zone::Deck(deck) => {
                    assert_eq!((card.set, card.value), (deck.set, deck.age), "{context}");
                }
                Zone::Board(_, color) => assert_eq!(card.color, color, "{context}"),
                Zone::AgeZeroReserve => {
                    assert_eq!((card.set, card.value), (CardSet::Base, 0), "{context}");
                }
                Zone::Display(_) | Zone::Museum(_) => {
                    assert_eq!(card.set, CardSet::Artifacts, "{context}");
                }
                _ => {}
            }
        }
        if let Zone::Display(_) = zone {
            assert!(p.cards(zone).len() <= 1, "{context} display");
        }
    }
    for tracker in Player::ALL
        .into_iter()
        .filter_map(|p| game.tracker(p))
        .chain(game.public_tracker())
    {
        assert!(tracker.check_truth(p), "{context}");
    }
}

/// How often a mode's games exercised each set's own mechanics.
#[derive(Debug, Default)]
struct Witness {
    figure_reactions: usize,
    city_draws: usize,
    echo_draws: usize,
    forecast_moves: usize,
    promotions: usize,
    top_echo_dogmas: usize,
    artifact_draws: usize,
    digs: usize,
    seizures: usize,
    rotations: usize,
    museum_claims: usize,
    unseen_draws: usize,
    safeguards: usize,
    secrets_out: usize,
    unseen_claims: usize,
    flips: usize,
}

impl Witness {
    fn events(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::Moved(m) => {
                    let drawn_from = |set| {
                        m.verb.is_none()
                            && matches!(m.source, Zone::Deck(Deck { set: s, .. }) if s == set)
                    };
                    self.figure_reactions += usize::from(m.inside_karma);
                    self.city_draws += usize::from(drawn_from(CardSet::Cities));
                    self.echo_draws += usize::from(drawn_from(CardSet::Echoes));
                    self.forecast_moves += usize::from(matches!(m.destination, Zone::Forecast(_)));
                    self.promotions += usize::from(
                        m.verb == Some(Verb::Meld) && matches!(m.source, Zone::Forecast(_)),
                    );
                    self.artifact_draws += usize::from(drawn_from(CardSet::Artifacts));
                    self.digs += usize::from(m.verb == Some(Verb::Dig));
                    self.seizures += usize::from(m.verb == Some(Verb::Seize));
                    self.rotations += usize::from(m.verb == Some(Verb::Rotate));
                    self.unseen_draws += usize::from(drawn_from(CardSet::Unseen));
                    self.safeguards += usize::from(m.verb == Some(Verb::Safeguard));
                    self.secrets_out += usize::from(matches!(m.source, Zone::Safe(_)));
                }
                Event::MuseumClaimed { .. } => self.museum_claims += 1,
                Event::SpecialMoved {
                    special,
                    destination: SpecialLocation::Owned(_),
                    ..
                } if Special::UNSEEN.contains(special) => self.unseen_claims += 1,
                Event::Flipped { .. } => self.flips += 1,
                _ => {}
            }
        }
    }

    /// The mechanics a mode's enabled sets must have shown.
    fn missing(&self, expansions: Expansions) -> Vec<&'static str> {
        [
            (
                expansions.figures(),
                "figure_reactions",
                self.figure_reactions,
            ),
            (expansions.cities(), "city_draws", self.city_draws),
            (expansions.echoes(), "echo_draws", self.echo_draws),
            (expansions.echoes(), "forecast_moves", self.forecast_moves),
            (expansions.echoes(), "promotions", self.promotions),
            (expansions.echoes(), "top_echo_dogmas", self.top_echo_dogmas),
            (expansions.artifacts(), "digs", self.digs),
            (expansions.artifacts(), "seizures", self.seizures),
            (expansions.artifacts(), "rotations", self.rotations),
            (expansions.artifacts(), "museum_claims", self.museum_claims),
            (expansions.unseen(), "unseen_draws", self.unseen_draws),
            (expansions.unseen(), "safeguards", self.safeguards),
            (expansions.unseen(), "secrets_out", self.secrets_out),
            (expansions.unseen(), "unseen_claims", self.unseen_claims),
        ]
        .into_iter()
        .filter(|&(enabled, _, count)| enabled && count == 0)
        .map(|(_, name, _)| name)
        .collect()
    }
}

/// Play one seeded random game to its end: its digest and boundary count.
fn trace(seed: u64, expansions: Expansions, witness: &mut Witness) -> (u64, usize) {
    let mode = expansions.name();
    let mut game = Game::new_game(seed, expansions).unwrap();
    // One fully tracked game in each mode supplements the dedicated
    // observation and search tests.
    if seed == 0 {
        game = game.with_tracking().unwrap();
    }
    let mut policy = Rng::new(seed ^ 0x0005_EED0_FA11);
    let mut digest = Digest(0xcbf2_9ce4_8422_2325);
    digest.state(game.position());
    for boundary in 0..MAX_BOUNDARIES {
        let context = format!("{mode} seed {seed} boundary {boundary}");
        let mut clone = (boundary % 64 == 0).then(|| game.clone());
        let mut events = Vec::new();
        let status = game
            .advance(WORK, &mut events)
            .unwrap_or_else(|error| panic!("{context}: {error:?}"));
        if let Some(clone) = clone.as_mut() {
            let mut cloned_events = Vec::new();
            assert_eq!(clone.advance(WORK, &mut cloned_events).unwrap(), status);
            assert_eq!(cloned_events, events, "{context}");
            assert_eq!(*clone, game, "{context}");
        }
        invariant(&game, &context);
        witness.events(&events);
        digest.add(&events);
        digest.add(&status);
        digest.state(game.position());
        match status {
            Status::Decision(decision) => {
                assert!(!decision.options.is_empty(), "{context}: empty decision");
                let index = (policy.next_u64() % decision.options.len() as u64) as usize;
                digest.add(&index);
                game.choose(index).unwrap();
            }
            Status::Complete(_) => {
                let actions = game.legal_actions();
                assert!(!actions.is_empty(), "{context}: nonterminal dead end");
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
                return (digest.0, boundary + 1);
            }
            Status::WorkLimit => {}
        }
        invariant(&game, &context);
    }
    panic!("{mode} seed {seed} did not end within {MAX_BOUNDARIES} bounded advances");
}

/// One mode's fixture lines: a digest per seed, then what its games showed.
fn recorded(expansions: Expansions, seeds: u64) -> String {
    let mode = expansions.name();
    let mut witness = Witness::default();
    let mut lines = String::new();
    for seed in 0..seeds {
        let (digest, boundaries) = trace(seed, expansions, &mut witness);
        writeln!(lines, "{mode} {seed} {digest:016x} {boundaries}").unwrap();
    }
    let missing = witness.missing(expansions);
    assert!(missing.is_empty(), "{mode} never showed {missing:?}");
    writeln!(lines, "# {mode} {witness:?}").unwrap();
    lines
}

#[test]
fn seeded_random_games_in_every_mode_finish_and_match_recorded_traces() {
    let setting = |name: &str| std::env::var(name).ok();
    let seeds = setting("TRACE_SEEDS").map_or(SEEDS, |n| n.parse().unwrap());
    let fixture = setting("TRACE_FIXTURE").unwrap_or_else(|| FIXTURE.to_owned());
    let actual: String = std::thread::scope(|scope| {
        let modes =
            Expansions::ALL.map(|expansions| scope.spawn(move || recorded(expansions, seeds)));
        modes
            .into_iter()
            .map(|mode| {
                mode.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    if setting("UPDATE_TRACES").is_some() {
        std::fs::write(&fixture, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&fixture).expect("recorded trace fixture");
    for (want, got) in expected.lines().zip(actual.lines()) {
        assert_eq!(want, got, "first diverging seed or witnessed behavior");
    }
    assert_eq!(expected.lines().count(), actual.lines().count());
}
