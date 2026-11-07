//! Bounded CPU search scaling on replayed training positions, without a network.
//! Usage: search_workers ROOTS WORKERS_PER_POOL POOLS REPEATS [SIMULATIONS]
//! ROOTS contains `mode seed choice...` lines. Independent pools search the
//! same roots with synthetic Q values; matching counts/digests verify equal
//! work across worker counts. Replay and thread creation precede the timer.
#![forbid(unsafe_code)]

use inno_engine::{Expansions, Match, Prompt, SearchConfig, SearchTree, Step};
use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Default)]
struct Totals {
    roots: usize,
    leaves: usize,
    refused: usize,
    digest: u64,
}

impl Totals {
    fn add(&mut self, other: Self) {
        self.roots += other.roots;
        self.leaves += other.leaves;
        self.refused += other.refused;
        self.digest = self.digest.wrapping_add(other.digest);
    }
}

fn search(root: &Match, seed: u64, simulations: u32) -> Totals {
    let mut total = Totals {
        roots: 1,
        ..Totals::default()
    };
    let config = SearchConfig {
        seed,
        exploration: 0.25,
        prior_temperature: 0.3,
        virtual_loss: 1.0,
        ..SearchConfig::learned(simulations)
    };
    let Ok(mut tree) = SearchTree::new(root.game(), config, true) else {
        total.refused = 1;
        return total;
    };
    while !tree.done() {
        if tree.select(8).is_err() {
            total.refused = 1;
            return total;
        }
        let q: Vec<Vec<f32>> = tree
            .pending()
            .map(|obs| {
                total.leaves += 1;
                // Include the row serialization the live collector performs,
                // without making a full-row checksum the benchmark's hot loop.
                let row = std::hint::black_box(obs.row());
                total.digest = total.digest.wrapping_mul(31).wrapping_add(row.len() as u64);
                (0..obs.options.len())
                    .map(|i| (i % 3) as f32 * 0.1 - 0.1)
                    .collect()
            })
            .collect();
        tree.resolve(&q).unwrap();
    }
    for m in tree.result().unwrap().moves {
        total.digest = total
            .digest
            .wrapping_mul(31)
            .wrapping_add(u64::from(m.visits))
            .wrapping_mul(31)
            .wrapping_add(u64::from((m.value as f32).to_bits()));
    }
    total
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = std::fs::read_to_string(args.next().expect("root corpus")).unwrap();
    let workers: usize = args.next().expect("workers per pool").parse().unwrap();
    let pools: usize = args.next().expect("pools").parse().unwrap();
    let repeats: usize = args.next().expect("repeats").parse().unwrap();
    let simulations: u32 = args.next().map_or(256, |s| s.parse().unwrap());
    assert!(workers > 0 && pools > 0 && repeats > 0 && simulations > 0);
    let roots: Vec<Match> = input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split_whitespace();
            let mode = fields.next().unwrap().parse().unwrap();
            let seed = fields.next().unwrap().parse().unwrap();
            let mut game = Match::new(Expansions::from_bits(mode).unwrap(), seed).unwrap();
            for choice in fields {
                let choice: usize = choice.parse().unwrap();
                let step = match game.prompt() {
                    Prompt::Decision(_) => Step::Choice(choice),
                    Prompt::Turn { actions, .. } => Step::Action(actions[choice]),
                    Prompt::Over(_) => panic!("replay ended before root"),
                };
                game.apply(step).unwrap();
            }
            game
        })
        .collect();
    assert!(!roots.is_empty());
    let counters: Vec<AtomicUsize> = (0..pools).map(|_| AtomicUsize::new(0)).collect();
    let barrier = Barrier::new(workers * pools + 1);
    let (total, seconds) = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for counter in &counters {
            for _ in 0..workers {
                let (roots, barrier) = (&roots, &barrier);
                handles.push(scope.spawn(move || {
                    let mut total = Totals::default();
                    barrier.wait();
                    loop {
                        let index = counter.fetch_add(1, Ordering::Relaxed);
                        if index >= roots.len() * repeats {
                            break;
                        }
                        let root = index % roots.len();
                        total.add(search(&roots[root], root as u64 ^ 0x5EA2, simulations));
                    }
                    total
                }));
            }
        }
        let start = Instant::now();
        barrier.wait();
        let mut total = Totals::default();
        for handle in handles {
            total.add(handle.join().unwrap());
        }
        (total, start.elapsed().as_secs_f64())
    });
    assert_eq!(total.roots, roots.len() * repeats * pools);
    println!(
        "roots={} workers={workers} pools={pools} repeats={repeats} simulations={simulations} \
         completed={} leaves={} refused={} digest={:016x} seconds={seconds:.6}",
        roots.len(),
        total.roots,
        total.leaves,
        total.refused,
        total.digest
    );
}
