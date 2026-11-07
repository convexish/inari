//! Seeded random games with tracking. At every decision and action boundary,
//! determinize for the acting observer and count refusals by reason. With
//! `play`, also play each sampled world forward randomly for a few actions,
//! checking tracker truth and card conservation after every step.
//! With `observe`, also encode every root for its actor and time it.
//! With `nested`, also sample each sampled world again for every observer
//! that may sample it (the actor at a decision; both at a boundary).
//! `cargo run --release --example tracking_probe -- <games> [first_seed]
//! [full] [cities] [echoes] [play] [observe] [nested]`; `full` deals Base +
//! Figures games, `cities` adds Cities and `echoes` adds Echoes (in any
//! combination).
use std::collections::BTreeMap;

use inno_engine::*;

const WORK: usize = 1 << 20;

fn truth(game: &Game) -> bool {
    game.public_tracker().unwrap().check_truth(game.position())
        && Player::ALL
            .into_iter()
            .all(|viewer| game.tracker(viewer).unwrap().check_truth(game.position()))
}

/// Play a sampled world for up to `actions` actions; Err describes a failure.
fn play_forward(mut world: Game, rng: &mut Rng, actions: u32) -> Result<(), String> {
    let mut taken = 0;
    for _ in 0..2_000 {
        let status = world
            .advance(WORK, &mut Vec::new())
            .map_err(|e| format!("advance: {e:?}"))?;
        if !truth(&world) {
            return Err("tracker truth".into());
        }
        match status {
            Status::Decision(menu) => {
                let i = (rng.next_u64() % menu.options.len() as u64) as usize;
                world.choose(i).map_err(|e| format!("choose: {e:?}"))?;
            }
            Status::Complete(_) => {
                if taken == actions {
                    return Ok(());
                }
                taken += 1;
                let legal = world.legal_actions();
                let action = legal[(rng.next_u64() % legal.len() as u64) as usize];
                world
                    .take_action(action, ActionKind::Regular)
                    .map_err(|e| format!("action: {e:?}"))?;
            }
            Status::Winner(_) | Status::Draw(_) => return Ok(()),
            Status::WorkLimit => return Err("work limit".into()),
        }
    }
    Err("no progress".into())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let games: u64 = args.next().map_or(100, |a| a.parse().expect("game count"));
    let first: u64 = args.next().map_or(0, |a| a.parse().expect("first seed"));
    let flags: Vec<String> = args.collect();
    let mut expansions = Expansions::BASE;
    for (name, expansion) in [
        ("full", Expansions::FIGURES),
        ("cities", Expansions::CITIES),
        ("echoes", Expansions::ECHOES),
    ] {
        if flags.iter().any(|f| f == name) {
            expansions = expansions.union(expansion);
        }
    }
    let play = flags.iter().any(|f| f == "play");
    let observe = flags.iter().any(|f| f == "observe");
    let nested = flags.iter().any(|f| f == "nested");
    let (mut encoded, mut encode_seconds, mut row_values) = (0u64, 0.0, 0usize);
    let mut sampled = 0u64;
    let mut played = 0u64;
    let mut failures: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for seed in first..first + games {
        let game = Game::new_game(seed, expansions);
        let mut game = game.unwrap().with_tracking().unwrap();
        let mut policy = Rng::new(seed ^ 0x7EAC);
        let mut events = Vec::new();
        for _ in 0..20_000 {
            events.clear();
            let status = game.advance(WORK, &mut events).unwrap();
            let viewer = match &status {
                Status::Decision(menu) => Some(menu.player),
                Status::Complete(_) => Some(game.position().turn_owner()),
                _ => None,
            };
            if let Some(viewer) = viewer
                && observe
            {
                let start = std::time::Instant::now();
                let row = game.observe(viewer).unwrap().row();
                encode_seconds += start.elapsed().as_secs_f64();
                encoded += 1;
                row_values += row.len();
            }
            if let Some(viewer) = viewer {
                match game.determinize(viewer, policy.next_u64()) {
                    Ok(world) => {
                        sampled += 1;
                        if nested {
                            let again: &[Player] = if matches!(status, Status::Decision(_)) {
                                &[viewer]
                            } else {
                                &Player::ALL
                            };
                            for &again in again {
                                if let Err(error) = world.determinize(again, policy.next_u64()) {
                                    failures
                                        .entry(format!("nested: {error:?}"))
                                        .or_insert((0, seed))
                                        .0 += 1;
                                }
                            }
                        }
                        if play {
                            let mut rng = Rng::new(policy.next_u64());
                            match play_forward(world, &mut rng, 2) {
                                Ok(()) => played += 1,
                                Err(why) => {
                                    failures
                                        .entry(format!("sampled world: {why}"))
                                        .or_insert((0, seed))
                                        .0 += 1
                                }
                            }
                        }
                    }
                    Err(error) => {
                        failures.entry(format!("{error:?}")).or_insert((0, seed)).0 += 1;
                    }
                }
            }
            match status {
                Status::Decision(menu) => {
                    let i = (policy.next_u64() % menu.options.len() as u64) as usize;
                    game.choose(i).unwrap();
                }
                Status::Complete(_) => {
                    let actions = game.legal_actions();
                    let a = actions[(policy.next_u64() % actions.len() as u64) as usize];
                    game.take_action(a, ActionKind::Regular).unwrap();
                }
                _ => break,
            }
        }
    }
    let refused: u64 = failures
        .iter()
        .filter(|(why, _)| !why.starts_with("sampled world"))
        .map(|(_, (n, _))| n)
        .sum();
    println!(
        "roots {} sampled {} refused {} ({:.3}%) played forward {}",
        sampled + refused,
        sampled,
        refused,
        100.0 * refused as f64 / (sampled + refused) as f64,
        played
    );
    if encoded > 0 {
        println!(
            "observed {encoded} roots: {:.1} us each, {:.0} values per row",
            1e6 * encode_seconds / encoded as f64,
            row_values as f64 / encoded as f64
        );
    }
    for (why, (n, seed)) in &failures {
        println!("  {n:>7} {why} (first seed {seed})");
    }
}
