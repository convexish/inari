//! Seeded uniformly random Base games: a fault/invariant smoke test and a
//! fixed-work throughput probe. `cargo run --release --example random_games
//! -- <games> [first_seed] [flags...]`. Flags: `check` verifies card
//! conservation after every step (untimed runs only); `clone` also times
//! clones of every eighth sampled state; `coverage` lists rarely used cards;
//! `figures` plays the same Base deal under Figures rules (no Figures cards),
//! to measure their overhead;
//! `full` plays complete Base + Figures deals (`Game::new_figures`);
//! `cities` adds Cities, `echoes` adds Echoes and `artifacts` adds Artifacts
//! (`Game::new_game`), with or without each other and `full`;
//! `no-achieve` never takes Achieve actions, so `figures` then plays the very
//! same games as Base and isolates the cost of the Figures code.
use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

use inno_engine::*;

/// Bounded automatic work between decisions; a stuck loop is reported.
const WORK: usize = 1 << 20;
const MAX_DECISIONS: u64 = 20_000;

#[derive(Default)]
struct Totals {
    games: u64,
    decisions: u64,
    actions: u64,
    events: u64,
    winners: u64,
    draws: u64,
    /// Draws ending a mandatory loop (included in `draws`).
    loops: u64,
    stuck: u64,
    /// First few stuck games: seed and whether the work or decision cap hit.
    stuck_seeds: Vec<(u64, &'static str)>,
    faults: BTreeMap<String, (u64, u64)>,
    dogmas: BTreeMap<&'static str, u64>,
    /// Figures melded, karma-performed moves, Decree moves, Age 0 starts.
    figure_melds: BTreeMap<&'static str, u64>,
    karma_moves: u64,
    decree_moves: u64,
    age_zero: u64,
    /// City melds, Search reveals, endorsements, Cities tokens claimed.
    city_melds: u64,
    search_reveals: u64,
    endorsements: u64,
    city_claims: u64,
    /// Echoes cards melded, Forecast arrivals, promotions, Echoes tokens claimed.
    echoes_melds: u64,
    forecast_arrivals: u64,
    promotions: u64,
    echoes_claims: u64,
}

struct Options {
    check: bool,
    clone: bool,
    /// Never take Achieve actions: Base deals then play identically under
    /// Base and Figures rules (no Achieve draw; five specials win neither).
    no_achieve: bool,
}

fn play(
    mut game: Game,
    seed: u64,
    options: &Options,
    samples: &mut Vec<Game>,
    totals: &mut Totals,
) {
    let mut policy = Rng::new(seed ^ 0x0005_EED0_FA11);
    let mut events = Vec::new();
    let mut decisions = 0;
    loop {
        events.clear();
        let status = match game.advance(WORK, &mut events) {
            Ok(status) => status,
            Err(error) => {
                totals
                    .faults
                    .entry(format!("{error:?}"))
                    .or_insert((0, seed))
                    .0 += 1;
                break;
            }
        };
        totals.events += events.len() as u64;
        for event in &events {
            match event {
                Event::Moved(movement) => {
                    totals.karma_moves += u64::from(movement.inside_karma);
                    let card = game.position().card(movement.card);
                    if movement.verb == Some(Verb::Meld) && card.figure.is_some() {
                        *totals.figure_melds.entry(card.code).or_default() += 1;
                    }
                    if movement.verb == Some(Verb::Meld) && card.set == CardSet::Cities {
                        totals.city_melds += 1;
                    }
                    if movement.verb == Some(Verb::Meld) && card.set == CardSet::Echoes {
                        totals.echoes_melds += 1;
                    }
                    totals.forecast_arrivals +=
                        u64::from(matches!(movement.destination, Zone::Forecast(_)));
                    totals.promotions += u64::from(
                        movement.verb == Some(Verb::Meld)
                            && matches!(movement.source, Zone::Forecast(_)),
                    );
                }
                Event::Revealed { source, .. } => {
                    totals.search_reveals += u64::from(matches!(source, Zone::Deck(_)));
                }
                Event::Endorsed { .. } => totals.endorsements += 1,
                Event::SpecialMoved {
                    special,
                    destination: SpecialLocation::Owned(_),
                    ..
                } if Special::CITIES.contains(special) => totals.city_claims += 1,
                Event::SpecialMoved {
                    special,
                    destination: SpecialLocation::Owned(_),
                    ..
                } if Special::ECHOES.contains(special) => totals.echoes_claims += 1,
                Event::DecreeMoved { .. } => totals.decree_moves += 1,
                Event::AgeZeroIntroduced { .. } => totals.age_zero += 1,
                _ => {}
            }
        }
        if options.clone && (totals.actions + decisions).is_multiple_of(8) && samples.len() < 8192 {
            samples.push(game.clone());
        }
        if options.check {
            assert!(game.position().check_conservation(), "seed {seed}");
        }
        match status {
            Status::Decision(decision) => {
                decisions += 1;
                let index = (policy.next_u64() % decision.options.len() as u64) as usize;
                game.choose(index).expect("offered option");
            }
            Status::Complete(_) => {
                let mut actions = game.legal_actions();
                if options.no_achieve {
                    actions.retain(|action| !matches!(action, Action::Achieve { .. }));
                }
                let action = actions[(policy.next_u64() % actions.len() as u64) as usize];
                totals.actions += 1;
                if let Action::Dogma { card, .. } = action {
                    *totals
                        .dogmas
                        .entry(game.position().card(card).code)
                        .or_default() += 1;
                }
                game.take_action(action, ActionKind::Regular)
                    .expect("legal action");
            }
            Status::Winner(_) => {
                totals.winners += 1;
                break;
            }
            Status::Draw(reason) => {
                totals.draws += 1;
                totals.loops += u64::from(reason == DrawReason::Loop);
                break;
            }
            Status::WorkLimit => {
                totals.stuck += 1;
                totals.stuck_seeds.push((seed, "work"));
                break;
            }
        }
        if decisions >= MAX_DECISIONS {
            totals.stuck += 1;
            totals.stuck_seeds.push((seed, "decisions"));
            break;
        }
    }
    totals.games += 1;
    totals.decisions += decisions;
}

/// Mean nanoseconds to clone and drop each sampled state.
fn clone_cost(samples: &[Game]) -> f64 {
    let passes = 20;
    let start = Instant::now();
    for _ in 0..passes {
        for state in samples {
            black_box(black_box(state).clone());
        }
    }
    start.elapsed().as_secs_f64() * 1e9 / (passes * samples.len()) as f64
}

fn main() {
    let mut args = std::env::args().skip(1);
    let games: u64 = args.next().map_or(1000, |a| a.parse().expect("game count"));
    let first: u64 = args.next().map_or(0, |a| a.parse().expect("first seed"));
    let flags: Vec<String> = args.collect();
    let flag = |name: &str| flags.iter().any(|f| f == name);
    let options = Options {
        check: flag("check"),
        clone: flag("clone"),
        no_achieve: flag("no-achieve"),
    };
    let mut totals = Totals::default();
    let mut base_samples = Vec::new();
    let mut figures_samples = Vec::new();
    let start = Instant::now();
    let expanded = flag("cities") || flag("echoes") || flag("artifacts") || flag("unseen");
    for seed in first..first + games {
        if expanded {
            let mut expansions = Expansions::BASE;
            for (name, expansion) in [
                ("full", Expansions::FIGURES),
                ("cities", Expansions::CITIES),
                ("echoes", Expansions::ECHOES),
                ("artifacts", Expansions::ARTIFACTS),
                ("unseen", Expansions::UNSEEN),
            ] {
                if flag(name) {
                    expansions = expansions.union(expansion);
                }
            }
            let game = Game::new_game(seed, expansions).unwrap();
            play(game, seed, &options, &mut figures_samples, &mut totals);
        } else if flag("full") {
            let game = Game::new_figures(seed).unwrap();
            play(game, seed, &options, &mut figures_samples, &mut totals);
        } else if flag("figures") {
            let game = Game::base_deal_with_figures_rules(seed).unwrap();
            play(game, seed, &options, &mut figures_samples, &mut totals);
        } else {
            let game = Game::new_base(seed).unwrap();
            play(game, seed, &options, &mut base_samples, &mut totals);
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "games {} decisions {} actions {} events {} winners {} draws {} (loops {}) stuck {} faults {}",
        totals.games,
        totals.decisions,
        totals.actions,
        totals.events,
        totals.winners,
        totals.draws,
        totals.loops,
        totals.stuck,
        totals.faults.values().map(|(n, _)| n).sum::<u64>()
    );
    if flag("cities") {
        println!(
            "city melds {} search reveals {} endorsements {} cities tokens claimed {}",
            totals.city_melds, totals.search_reveals, totals.endorsements, totals.city_claims
        );
    }
    if flag("echoes") {
        let distinct = ECHOES_METADATA
            .iter()
            .filter(|m| totals.dogmas.contains_key(m.code))
            .count();
        println!(
            "echoes melds {} forecast arrivals {} promotions {} echoes tokens claimed {} \
             Echoes cards dogma'd {distinct}/{}",
            totals.echoes_melds,
            totals.forecast_arrivals,
            totals.promotions,
            totals.echoes_claims,
            ECHOES_METADATA.len()
        );
    }
    if flag("full") {
        println!(
            "figures melded {} (distinct {}) karma moves {} decree moves {} age 0 introduced {}",
            totals.figure_melds.values().sum::<u64>(),
            totals.figure_melds.len(),
            totals.karma_moves,
            totals.decree_moves,
            totals.age_zero
        );
        let age_zero: Vec<_> = AGE_ZERO_METADATA
            .iter()
            .map(|m| (m.code, totals.dogmas.get(m.code).copied().unwrap_or(0)))
            .collect();
        println!("Age 0 Dogma actions: {age_zero:?}");
    }
    if flag("coverage") {
        let rare: Vec<_> = BASE_METADATA
            .iter()
            .map(|m| (totals.dogmas.get(m.code).copied().unwrap_or(0), m.code))
            .filter(|&(n, _)| n < 20)
            .collect();
        println!("cards with fewer than 20 Dogma actions: {rare:?}");
    }
    if !totals.stuck_seeds.is_empty() {
        println!(
            "stuck: {:?}",
            &totals.stuck_seeds[..totals.stuck_seeds.len().min(10)]
        );
    }
    for (fault, (count, seed)) in &totals.faults {
        println!("  {count:6} first seed {seed:6}  {fault}");
    }
    if options.clone {
        let (cost, sampled) = if flag("figures") || flag("full") || expanded {
            (clone_cost(&figures_samples), figures_samples.len())
        } else {
            (clone_cost(&base_samples), base_samples.len())
        };
        println!("clone {cost:.0} ns over {sampled} sampled states");
    } else {
        println!(
            "{elapsed:.3}s  {:.1} games/s  {:.0} actions/s  {:.0} decisions/s",
            totals.games as f64 / elapsed,
            totals.actions as f64 / elapsed,
            totals.decisions as f64 / elapsed
        );
    }
}
