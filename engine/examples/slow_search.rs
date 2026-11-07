//! Replay games from lines `mode seed choice...` and time every search step
//! at every decision with fixed synthetic Q values: tree construction, each
//! `select` call and each applied step. Prints the steps slower than a
//! threshold, then a summary. Usage: slow_search GAMES SIMS THREADS MS
use inno_engine::{Expansions, Match, Prompt, SearchConfig, SearchTree, Step};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let input = std::fs::read_to_string(args.next().expect("game lines")).unwrap();
    let sims: u32 = args.next().map_or(64, |s| s.parse().unwrap());
    let threads: usize = args.next().map_or(4, |s| s.parse().unwrap());
    let threshold: f64 = args.next().map_or(100.0, |s| s.parse().unwrap());
    let games: Vec<(u8, u64, Vec<usize>)> = input
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let mode = fields.next().unwrap().parse().unwrap();
            let seed = fields.next().unwrap().parse().unwrap();
            (mode, seed, fields.map(|c| c.parse().unwrap()).collect())
        })
        .collect();
    let next = AtomicUsize::new(0);
    let totals = Mutex::new((0usize, 0usize, 0f64, Vec::<f64>::new()));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let g = next.fetch_add(1, Ordering::Relaxed);
                    let Some((mode, seed, choices)) = games.get(g) else {
                        break;
                    };
                    let mut game = Match::new(Expansions::from_bits(*mode).unwrap(), *seed).unwrap();
                    let (mut decisions, mut refused, mut seconds, mut worst) = (0, 0, 0.0, Vec::new());
                    for (d, &choice) in choices.iter().enumerate() {
                        let start = Instant::now();
                        let config = SearchConfig {
                            seed: *seed ^ d as u64,
                            virtual_loss: 1.0,
                            ..SearchConfig::learned(sims)
                        };
                        let built = SearchTree::new(game.game(), config, true);
                        let new_ms = start.elapsed().as_secs_f64() * 1e3;
                        let mut slowest = (0.0, 0usize);
                        let mut calls = 0;
                        match built {
                            Err(_) => refused += 1,
                            Ok(mut tree) => {
                                while !tree.done() {
                                    let call = Instant::now();
                                    if tree.select(8).is_err() {
                                        refused += 1;
                                        break;
                                    }
                                    let ms = call.elapsed().as_secs_f64() * 1e3;
                                    if ms > slowest.0 {
                                        slowest = (ms, calls);
                                    }
                                    calls += 1;
                                    let q: Vec<Vec<f32>> = tree
                                        .pending()
                                        .map(|obs| {
                                            (0..obs.options.len())
                                                .map(|i| (i % 3) as f32 * 0.1 - 0.1)
                                                .collect()
                                        })
                                        .collect();
                                    tree.resolve(&q).unwrap();
                                }
                            }
                        }
                        let step = match game.prompt() {
                            Prompt::Decision(_) => Step::Choice(choice),
                            Prompt::Turn { actions, .. } => Step::Action(actions[choice]),
                            Prompt::Over(_) => panic!("replay ended early"),
                        };
                        let apply = Instant::now();
                        game.apply(step).unwrap();
                        let apply_ms = apply.elapsed().as_secs_f64() * 1e3;
                        let total = start.elapsed().as_secs_f64() * 1e3;
                        if new_ms > threshold || slowest.0 > threshold || apply_ms > threshold {
                            println!(
                                "SLOW mode {mode} seed {seed} decision {d} new {new_ms:.1} ms, slowest select {:.1} ms (call {} of {calls}), apply {apply_ms:.1} ms, total {total:.1} ms",
                                slowest.0, slowest.1
                            );
                        }
                        decisions += 1;
                        seconds += total / 1e3;
                        worst.push(slowest.0.max(new_ms));
                    }
                    let mut totals = totals.lock().unwrap();
                    totals.0 += decisions;
                    totals.1 += refused;
                    totals.2 += seconds;
                    totals.3.extend(worst);
                }
            });
        }
    });
    let (decisions, refused, seconds, mut worst) = totals.into_inner().unwrap();
    worst.sort_by(f64::total_cmp);
    let pct = |p: f64| worst[((worst.len() - 1) as f64 * p) as usize];
    println!(
        "games={} decisions={decisions} refused={refused} search_seconds={seconds:.1} slowest-call ms: p50 {:.2} p99 {:.1} p99.9 {:.1} max {:.1}",
        games.len(),
        pct(0.5),
        pct(0.99),
        pct(0.999),
        worst.last().copied().unwrap_or(0.0)
    );
}
