//! Seeded uniformly random matches through the session layer, with tracking,
//! in every training mode: reports each step the engine offered and then
//! rejected, and each stuck match. `cargo run --release --example fault_hunt
//! -- <games per mode> [first_seed] [threads] [subgames]`. Each fault line
//! carries the mode, seed, step and the replay record.
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use inno_engine::*;

const MAX_STEPS: usize = 3000;

fn main() {
    let mut args = std::env::args().skip(1);
    let games: u64 = args
        .next()
        .map_or(100, |a| a.parse().expect("games per mode"));
    let first: u64 = args.next().map_or(0, |a| a.parse().expect("first seed"));
    let threads: usize = args.next().map_or(8, |a| a.parse().expect("threads"));
    let subgames = args.next().is_some_and(|a| a == "subgames");
    let jobs: Vec<(u8, u64)> = (0..TRAINING_MODES.len() as u8)
        .flat_map(|mode| (first..first + games).map(move |seed| (mode, seed)))
        .collect();
    let next = AtomicUsize::new(0);
    let faults = Mutex::new(Vec::new());
    let (finished, capped, steps) = (
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    );
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(mode, seed)) = jobs.get(index) else {
                        break;
                    };
                    let expansions = mode_expansions(mode).expect("training mode");
                    let mut m = match Match::with_subgames(expansions, seed, subgames) {
                        Ok(m) => m,
                        Err(error) => {
                            faults.lock().unwrap().push(format!(
                                "mode {} seed {seed} setup: {error:?}",
                                TRAINING_MODES[mode as usize]
                            ));
                            continue;
                        }
                    };
                    let mut policy = Rng::new(seed ^ 0xFA17_0000 ^ u64::from(mode) << 40);
                    let mut done = false;
                    for step_index in 0..MAX_STEPS {
                        steps.fetch_add(1, Ordering::Relaxed);
                        let Some(step) = m.random_step(policy.next_u64()) else {
                            done = true;
                            break;
                        };
                        if let Err(error) = m.apply(step) {
                            let record = m.record().to_text().unwrap_or_default();
                            faults.lock().unwrap().push(format!(
                                "mode {} seed {seed} step {step_index}: {error:?} applying {step:?}\nprompt {:?}\nrecord {record:?}",
                                TRAINING_MODES[mode as usize],
                                m.prompt()
                            ));
                            done = true;
                            break;
                        }
                    }
                    if done {
                        finished.fetch_add(1, Ordering::Relaxed);
                    } else {
                        capped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    let faults = faults.into_inner().unwrap();
    for fault in &faults {
        println!("FAULT {fault}");
    }
    println!(
        "{} matches, {:.0} steps each: {} ended or faulted, {} past {MAX_STEPS} steps, {} faults",
        jobs.len(),
        steps.load(Ordering::Relaxed) as f64 / jobs.len() as f64,
        finished.load(Ordering::Relaxed),
        capped.load(Ordering::Relaxed),
        faults.len()
    );
}
