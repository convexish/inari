//! A good opening hand against a bad one, between two AI settings. Finds
//! the Base deals where one seat is dealt `good` and the other `bad`, and
//! plays each twice: player A holding the good hand, then A holding the bad
//! one. Unless A and B are the same setting, random `controls` deals,
//! played once from each seat, give A's baseline.
//!
//! ```sh
//! pixi run lab hand_matchup run a=elo:1200 b=elo:600 > lab/runs/x.csv
//! pixi run lab hand_matchup summary lab/runs/x.csv
//! ```
//!
//! Options: `a=` and `b=` (players, `elo:N`, `t:X` or `sims:N`), `good=` and
//! `bad=` (`+`-separated Age 1 titles, default Clothing+Metalworking and
//! Tools+Writing), `scan=N` (deal seeds `0..N` searched, default 3,000,000:
//! about 730 deals), `controls=N` (default 734, or 0 when `a` is `b`) from `control_from=S`
//! (default 5,000,000), `workers=N`, `weights=PATH`.
//!
//! One row per game: `kind,seed,a_seat,winner,decisions` where `kind` is
//! `good` or `bad` (A's hand) or `control`, and `winner` is `a`, `b` or
//! `draw` (capped games are draws).
use std::collections::BTreeMap;

use inno_engine::*;
use lab::*;

const OPTIONS: [&str; 9] = [
    "a",
    "b",
    "good",
    "bad",
    "scan",
    "controls",
    "control_from",
    "workers",
    "weights",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Good,
    Bad,
    Control,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Good => "good",
            Kind::Bad => "bad",
            Kind::Control => "control",
        }
    }
}

struct Job {
    kind: Kind,
    seed: u64,
    a_seat: Player,
}

fn main() {
    let args = Args::from_env(&OPTIONS);
    match args.positional.first().map(String::as_str) {
        Some("run") => run(&args),
        Some("summary") => summary(args.positional.get(1).expect("summary <file>")),
        _ => panic!("usage: hand_matchup run [options] | hand_matchup summary <file>"),
    }
}

fn run(args: &Args) {
    let a: Ai = args.get("a", Ai::elo(1200));
    let b: Ai = args.get("b", Ai::elo(600));
    let good = parse_hand(&args.get("good", "Clothing+Metalworking".to_string())).unwrap();
    let bad = parse_hand(&args.get("bad", "Tools+Writing".to_string())).unwrap();
    let scan: u64 = args.get("scan", 3_000_000);
    // Equal players score 50% on random deals by construction.
    let controls: u64 = args.get("controls", if a == b { 0 } else { 734 });
    let control_from: u64 = args.get("control_from", 5_000_000);
    let weights = args.weights_path();
    let net = load_net(&weights);

    let mut jobs = Vec::new();
    for seed in 0..scan {
        let game = Game::new_game(seed, Expansions::BASE).expect("a Base deal");
        let hands = [hand(&game, Player::One), hand(&game, Player::Two)];
        let good_seat = if hands == [good.clone(), bad.clone()] {
            Player::One
        } else if hands == [bad.clone(), good.clone()] {
            Player::Two
        } else {
            continue;
        };
        let bad_seat = good_seat.opponent();
        jobs.push(Job {
            kind: Kind::Good,
            seed,
            a_seat: good_seat,
        });
        jobs.push(Job {
            kind: Kind::Bad,
            seed,
            a_seat: bad_seat,
        });
    }
    for seed in control_from..control_from + controls {
        for a_seat in Player::ALL {
            jobs.push(Job {
                kind: Kind::Control,
                seed,
                a_seat,
            });
        }
    }
    println!("{}", header("hand_matchup", args, &weights));
    println!(
        "# a={a} b={b} good={} bad={}",
        good.join("+"),
        bad.join("+")
    );
    println!("columns kind,seed,a_seat,winner,decisions");
    run_ordered(&jobs, args.workers(), |job| {
        let mut m = Match::new(Expansions::BASE, job.seed).expect("a Base deal");
        let mut rng =
            Rng::new(job.seed ^ (job.kind as u64) << 48 ^ (job.a_seat.index() as u64) << 56);
        let mut players = [b; 2];
        players[job.a_seat.index()] = a;
        let end = play(&mut m, players, &net, &mut rng);
        let winner = match end.winner {
            Some(seat) if seat == job.a_seat => "a",
            Some(_) => "b",
            None => "draw",
        };
        format!(
            "{},{},{},{winner},{}",
            job.kind.name(),
            job.seed,
            job.a_seat.index() + 1,
            end.decisions,
        )
    });
}

fn summary(path: &str) {
    let mut tally: BTreeMap<String, [u64; 3]> = BTreeMap::new();
    let lines = data_lines(path);
    for line in &lines {
        let f: Vec<&str> = line.split(',').collect();
        let t = tally.entry(f[0].to_string()).or_default();
        t[["a", "b", "draw"].iter().position(|w| *w == f[3]).unwrap()] += 1;
    }
    // The run's `# a=X b=Y good=... bad=...` line names the players.
    let text = std::fs::read_to_string(path).expect("the run file");
    let players = text
        .lines()
        .find_map(|l| l.strip_prefix("# a="))
        .expect("the run's `# a=` line");
    let (a, rest) = players.split_once(" b=").expect("b=");
    let b = rest.split(' ').next().expect("b's setting");
    let [gw, gl, gd] = tally["good"];
    let [bw, bl, bd] = tally["bad"];
    let line = |label: &str, (s, se): (f64, f64)| {
        println!(
            "  {label:22} {:5.1}% ±{:.1}  {:+.0} Elo",
            s * 100.0,
            se * 100.0,
            elo_gap(s)
        );
    };
    if a == b {
        // The same matchup whichever player is called A: pool both.
        let pooled = score(gw + bl, gl + bw, gd + bd);
        println!("Equal players ({a}), {} games", lines.len());
        println!(
            "  the good hand scores {:.1}% ±{:.1}: worth {:.0} ±{:.0} Elo",
            pooled.0 * 100.0,
            pooled.1 * 100.0,
            elo_gap(pooled.0),
            elo_se(pooled)
        );
        return;
    }
    let (good, bad) = (score(gw, gl, gd), score(bw, bl, bd));
    println!("A ({a}) against B ({b}), {} games", lines.len());
    if let Some(&[w, l, d]) = tally.get("control") {
        line("A on random deals", score(w, l, d));
    }
    line("A with the good hand", good);
    line("A with the bad hand", bad);
    let worth = (elo_gap(good.0) - elo_gap(bad.0)) / 2.0;
    if worth.is_finite() {
        println!(
            "  the hand is worth half the gap between the last two: {worth:.0} ±{:.0} Elo",
            (elo_se(good).powi(2) + elo_se(bad).powi(2)).sqrt() / 2.0
        );
    } else {
        println!("  a score of 0% or 100% leaves the hand's worth unmeasured");
    }
}
