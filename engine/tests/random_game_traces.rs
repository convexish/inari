//! Seeded uniformly random Base games, digested decision by decision. The
//! fixture pins every offered option, choice, action and internal event, so
//! an interpreter or card-language refactor must reproduce play exactly.
//! A deliberate rules change regenerates it: `UPDATE_TRACES=1 cargo test`.
//! `TRACE_SEEDS`/`TRACE_FIXTURE` select a larger out-of-tree comparison, and
//! `TRACE_FIGURES` digests full Base + Figures deals instead. A second
//! fixture pins Base + Cities and Base + Figures + Cities deals.
use std::fmt::Write as _;

use inno_engine::*;

const FIXTURE: &str = "tests/fixtures/random_base_traces.txt";
const SEEDS: u64 = 300;
const CITIES_FIXTURE: &str = "tests/fixtures/random_cities_traces.txt";
const CITIES_SEEDS: u64 = 100;

fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

struct Digest(u64);

impl Digest {
    fn add(&mut self, value: &impl std::fmt::Debug) {
        let mut text = String::new();
        write!(text, "{value:?};").unwrap();
        self.text(&text);
    }

    fn text(&mut self, text: &str) {
        for byte in text.bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn status(&mut self, status: &Result<Status, Error>, frozen: bool, game: &Game) {
        let mut status = status.clone();
        if let Ok(Status::Decision(decision)) = &mut status
            && decision
                .source_card(decision.player)
                .is_some_and(|source| matches!(game.position().card(source).code, "B090" | "Z006"))
        {
            // The frozen digest names revealed packet cards by physical slot.
            for option in &mut decision.options {
                if let Alternative::Select {
                    target: TargetDescriptor::Known { card, source },
                } = option
                {
                    let historical = game.position().descriptor(
                        Target {
                            card: *card,
                            physical: *source,
                            logical: LogicalZone::Physical(*source),
                        },
                        decision.player,
                    );
                    if matches!(historical, TargetDescriptor::Hidden { .. }) {
                        *option = Alternative::Select { target: historical };
                    }
                }
            }
        }
        let status = &status;
        let old_splay = |text: String| {
            text.replace("Splay { player: One, ", "Splay { ")
                .replace("Splay { player: Two, ", "Splay { ")
        };
        // The non-Echoes digest omits the target player; Splayed events retain it.
        if !frozen {
            self.text(&old_splay(format!("{status:?};")));
        } else if let Ok(Status::Decision(d)) = status {
            // Hash only the frozen gameplay projection.
            self.text(&frozen_backs(&old_splay(format!(
                "Ok(Decision(Decision {{ player: {:?}, options: {:?} }}));",
                d.player, d.options,
            ))));
        } else {
            self.add(status);
        }
    }
}

/// The identity step replaced hidden descriptors' `value, figure` pair with
/// the card's back. Render the back in the frozen fields: same information,
/// so the recorded digests still pin gameplay exactly.
fn frozen_backs(text: &str) -> String {
    const OPEN: &str = "back: Deck { set: ";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];
        let (set, after) = after.split_once(", age: ").expect("back set");
        let (age, after) = after.split_once(" }").expect("back age");
        assert!(set == "Base" || set == "Figures", "{set}");
        write!(out, "value: {age}, figure: {}", set == "Figures").unwrap();
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Digest one seeded random game. `frozen` keeps the Base/Figures
/// fixture's historical projection of decisions.
fn trace(seed: u64, expansions: Expansions, frozen: bool) -> (u64, u64) {
    let mut game = Game::new_game(seed, expansions).unwrap();
    let mut policy = Rng::new(seed ^ 0x0005_EED0_FA11);
    let mut digest = Digest(0xcbf2_9ce4_8422_2325);
    let mut events = Vec::new();
    let mut steps = 0;
    loop {
        events.clear();
        let status = game.advance(1 << 20, &mut events);
        digest.add(&events);
        digest.status(&status, frozen, &game);
        match status {
            Ok(Status::Decision(decision)) => {
                let index = (policy.next_u64() % decision.options.len() as u64) as usize;
                digest.add(&index);
                game.choose(index).unwrap();
            }
            Ok(Status::Complete(_)) => {
                let actions = game.legal_actions();
                let action = actions[(policy.next_u64() % actions.len() as u64) as usize];
                digest.add(&actions);
                digest.add(&action);
                game.take_action(action, ActionKind::Regular).unwrap();
            }
            _ => break,
        }
        steps += 1;
    }
    (digest.0, steps)
}

#[test]
fn seeded_random_base_games_match_recorded_traces() {
    let seeds = setting("TRACE_SEEDS").map_or(SEEDS, |n| n.parse().unwrap());
    let fixture = setting("TRACE_FIXTURE").unwrap_or_else(|| FIXTURE.to_owned());
    let expansions = if setting("TRACE_FIGURES").is_some() {
        Expansions::FIGURES
    } else {
        Expansions::BASE
    };
    let actual: String = (0..seeds)
        .map(|seed| {
            let (digest, steps) = trace(seed, expansions, true);
            format!("{seed} {digest:016x} {steps}\n")
        })
        .collect();
    if std::env::var_os("UPDATE_TRACES").is_some() {
        std::fs::write(&fixture, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&fixture).expect("recorded trace fixture");
    for (want, got) in expected.lines().zip(actual.lines()) {
        assert_eq!(want, got, "first diverging seed");
    }
    assert_eq!(expected.lines().count(), actual.lines().count());
}

#[test]
fn seeded_random_cities_games_match_recorded_traces() {
    let modes = [
        ("cities", Expansions::CITIES),
        (
            "figures+cities",
            Expansions::FIGURES.union(Expansions::CITIES),
        ),
    ];
    let actual: String = modes
        .into_iter()
        .flat_map(|(name, expansions)| {
            (0..CITIES_SEEDS).map(move |seed| {
                let (digest, steps) = trace(seed, expansions, false);
                format!("{name} {seed} {digest:016x} {steps}\n")
            })
        })
        .collect();
    if std::env::var_os("UPDATE_TRACES").is_some() {
        std::fs::write(CITIES_FIXTURE, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(CITIES_FIXTURE).expect("recorded trace fixture");
    for (want, got) in expected.lines().zip(actual.lines()) {
        assert_eq!(want, got, "first diverging seed");
    }
    assert_eq!(expected.lines().count(), actual.lines().count());
}
