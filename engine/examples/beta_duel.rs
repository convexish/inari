//! Policy-head play at two inverse temperatures on duplicate deals.
//! `cargo run --release --example beta_duel -- <weights.bin> [workers=N]`
//! reads jobs from stdin, one per line: `mode x y seed`, where `mode` is a
//! training-mode index (0..32), `x` and `y` are inverse temperatures (`inf`
//! and `-inf` allowed) and `seed` is the deal. Each side samples its policy
//! with probability proportional to `exp(beta * logit)`. Both seats are
//! played on the deal and one line is printed per job:
//! `mode x y seed score decisions capped`, with `score` the x side's mean
//! over the two seats (draws and games past 3,000 decisions count 1/2).
//! At the end, stderr gets one `spread` line per mode: positions with two or
//! more options, and the mean over them of the policy logits' standard
//! deviation and of the gap between the two largest logits.
use std::io::BufRead;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use inno_engine::net::sample_inverse_temperature;
use inno_engine::*;

const DECISION_CAP: usize = 3000;

struct Job {
    mode: u8,
    x: f64,
    y: f64,
    seed: u64,
}

#[derive(Clone, Copy, Default)]
struct Spread {
    positions: u64,
    std_sum: f64,
    gap_sum: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let net =
        Net::load(std::path::Path::new(args.first().expect("weights path"))).expect("weights");
    assert!(net.has_policy(), "the network needs a policy output");
    let workers: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("workers="))
        .map_or(8, |w| w.parse().expect("workers"));
    let jobs: Vec<Job> = std::io::stdin()
        .lock()
        .lines()
        .map(|line| line.expect("stdin"))
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(f.len(), 4, "job line `mode x y seed`: {line}");
            Job {
                mode: f[0].parse().expect("mode"),
                x: f[1].parse().expect("x"),
                y: f[2].parse().expect("y"),
                seed: f[3].parse().expect("seed"),
            }
        })
        .collect();
    let next = AtomicUsize::new(0);
    let spreads = Mutex::new([Spread::default(); Expansions::COMBINATIONS]);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let mut spread = Spread::default();
                    let mut score = 0.0;
                    let mut decisions = 0;
                    let mut capped = 0;
                    for x_seat in [Player::One, Player::Two] {
                        let (s, d) = play(&net, job, x_seat, &mut spread);
                        score += s / 2.0;
                        decisions += d;
                        capped += usize::from(d > DECISION_CAP);
                    }
                    println!(
                        "{} {} {} {} {score} {decisions} {capped}",
                        job.mode, job.x, job.y, job.seed
                    );
                    let mut all = spreads.lock().unwrap();
                    let total = &mut all[usize::from(job.mode)];
                    total.positions += spread.positions;
                    total.std_sum += spread.std_sum;
                    total.gap_sum += spread.gap_sum;
                }
            });
        }
    });
    for (mode, s) in spreads.into_inner().unwrap().iter().enumerate() {
        if s.positions > 0 {
            let n = s.positions as f64;
            eprintln!(
                "spread {mode} {} {} {:.4} {:.4}",
                TRAINING_MODES[mode],
                s.positions,
                s.std_sum / n,
                s.gap_sum / n
            );
        }
    }
}

/// One game with the x side in `x_seat`: the x side's score and the number
/// of decisions (more than the cap when the game was cut off).
fn play(net: &Net, job: &Job, x_seat: Player, spread: &mut Spread) -> (f64, usize) {
    let expansions = Expansions::from_bits(job.mode).expect("mode");
    let mut m = Match::new(expansions, job.seed).unwrap();
    let salt = if x_seat == Player::One {
        0xBE7A_0001
    } else {
        0xBE7A_0002
    };
    let mut rng = Rng::new(job.seed ^ salt);
    let mut decisions = 0;
    while let Some(player) = m.to_move() {
        decisions += 1;
        if decisions > DECISION_CAP {
            return (0.5, decisions);
        }
        let observation = m.game().observe(player).unwrap();
        let logits = net.policy(&observation).unwrap().expect("policy");
        let finite: Vec<f64> = logits
            .iter()
            .filter(|v| v.is_finite())
            .map(|&v| f64::from(v))
            .collect();
        if finite.len() >= 2 {
            let n = finite.len() as f64;
            let mean = finite.iter().sum::<f64>() / n;
            let var = finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
            let mut sorted = finite;
            sorted.sort_by(|a, b| b.total_cmp(a));
            spread.positions += 1;
            spread.std_sum += var.sqrt();
            spread.gap_sum += sorted[0] - sorted[1];
        }
        let beta = if player == x_seat { job.x } else { job.y };
        let index = sample_inverse_temperature(&logits, beta, &mut rng).expect("an option");
        let step = match m.prompt() {
            Prompt::Decision(_) => Step::Choice(index),
            Prompt::Turn { actions, .. } => Step::Action(actions[index]),
            Prompt::Over(_) => unreachable!(),
        };
        m.apply(step).unwrap();
    }
    let score = match m.prompt() {
        Prompt::Over(MatchOutcome::Winner { player, .. }) if *player == x_seat => 1.0,
        Prompt::Over(MatchOutcome::Winner { .. }) => 0.0,
        _ => 0.5,
    };
    (score, decisions)
}
