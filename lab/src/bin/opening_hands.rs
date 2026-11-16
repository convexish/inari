//! Which Base opening hands and starting melds win. Self-play on random
//! Base deals where each seat's starting meld is a coin flip, so every hand
//! is seen with both melds; the rest of each game is played by `play=` for
//! both seats.
//!
//! ```sh
//! pixi run lab opening_hands run > lab/runs/x.csv
//! pixi run lab opening_hands summary lab/runs/x.csv
//! ```
//!
//! Options: `games=N` (default 200,000) from deal seed `first=S` (default
//! 2,000,000), `play=` (a player, `elo:N`, `t:X` or `sims:N`; default
//! elo:850, the strongest policy play that still samples: the greedy policy
//! is deterministic and some of its games loop to the decision cap),
//! `workers=N`, `weights=PATH`.
//!
//! One row per game, hands in deal order:
//! `seed,p1a,p1b,p1meld,p2a,p2b,p2meld,first,winner,decisions` with `first`
//! the seat that took the first turn and `winner` 1, 2 or 0 (a draw or a
//! capped game).
//!
//! `summary` fits each hand's score as a base plus a value for each of its
//! two cards plus one term for hands of a single color, and reports each
//! card's value and how much better it does melded than kept in hand.
use std::collections::BTreeMap;

use inno_engine::*;
use lab::*;

const OPTIONS: [&str; 5] = ["games", "first", "play", "workers", "weights"];

fn main() {
    let args = Args::from_env(&OPTIONS);
    match args.positional.first().map(String::as_str) {
        Some("run") => run(&args),
        Some("summary") => summary(args.positional.get(1).expect("summary <file>")),
        _ => panic!("usage: opening_hands run [options] | opening_hands summary <file>"),
    }
}

fn run(args: &Args) {
    let games: u64 = args.get("games", 200_000);
    let first: u64 = args.get("first", 2_000_000);
    let ai: Ai = args.get("play", Ai::elo(850));
    let weights = args.weights_path();
    let net = load_net(&weights);
    let seeds: Vec<u64> = (first..first + games).collect();
    println!("{}", header("opening_hands", args, &weights));
    println!("# play={ai}");
    println!("columns seed,p1a,p1b,p1meld,p2a,p2b,p2meld,first,winner,decisions");
    run_ordered(&seeds, args.workers(), |&seed| {
        let mut m = Match::new(Expansions::BASE, seed).expect("a Base deal");
        let mut rng = Rng::new(seed ^ 0x09E1_1265);
        let mut cols = vec![seed.to_string()];
        play_opening(&mut m, |m, seat| {
            let hand = m.game().position().cards(Zone::Hand(seat));
            let pick = (rng.next_u64() & 1) as usize;
            cols.extend([hand[0], hand[1], hand[pick]].map(|c| card_title(c).to_string()));
            pick
        });
        let starter = m.to_move().expect("a first turn").index() + 1;
        let end = play(&mut m, [ai; 2], &net, &mut rng);
        let winner = end.winner.map_or(0, |seat| seat.index() + 1);
        cols.extend([
            starter.to_string(),
            winner.to_string(),
            (end.decisions + 2).to_string(),
        ]);
        cols.join(",")
    });
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// The standard error of a mean score `s` over `n` games.
fn se(s: f64, n: usize) -> f64 {
    (s * (1.0 - s) / n as f64).sqrt()
}

fn same_color(a: &str, b: &str) -> bool {
    let color = |t: &str| {
        BASE_METADATA
            .iter()
            .find(|m| m.title == t)
            .expect("a Base title")
            .color
    };
    color(a) == color(b)
}

fn summary(path: &str) {
    // Every seat's score by hand (its two titles sorted), and by card
    // melded or kept.
    let mut by_hand: BTreeMap<[String; 2], Vec<f64>> = BTreeMap::new();
    let mut melded: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut kept: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut first_turn = Vec::new();
    let lines = data_lines(path);
    for line in &lines {
        let f: Vec<&str> = line.split(',').collect();
        let (first, winner): (usize, usize) = (f[7].parse().unwrap(), f[8].parse().unwrap());
        for seat in 1..=2 {
            let c = &f[3 * seat - 2..3 * seat + 1];
            let score = match winner {
                0 => 0.5,
                w if w == seat => 1.0,
                _ => 0.0,
            };
            let mut cards = [c[0].to_string(), c[1].to_string()];
            cards.sort_unstable();
            for card in &cards {
                let side = if card == c[2] { &mut melded } else { &mut kept };
                side.entry(card.clone()).or_default().push(score);
            }
            by_hand.entry(cards).or_default().push(score);
            if first == seat {
                first_turn.push(score);
            }
        }
    }
    let hands: Vec<(&[String; 2], f64, f64)> = by_hand
        .iter()
        .map(|(cards, s)| (cards, mean(s), se(mean(s), s.len())))
        .collect();
    let cards: Vec<&String> = melded.keys().collect();

    // Least squares for score = base + value[a] + value[b] + mono (when a
    // and b share a color), by cycling through the terms until they settle.
    let mut base = mean(&hands.iter().map(|h| h.1).collect::<Vec<_>>());
    let mut value: BTreeMap<&str, f64> = cards.iter().map(|c| (c.as_str(), 0.0)).collect();
    let mut mono = 0.0;
    let fit = |cards: &[String; 2], base: f64, value: &BTreeMap<&str, f64>, mono: f64| {
        let m = if same_color(&cards[0], &cards[1]) {
            mono
        } else {
            0.0
        };
        base + value[cards[0].as_str()] + value[cards[1].as_str()] + m
    };
    for _ in 0..200 {
        for card in &cards {
            let residuals: Vec<f64> = hands
                .iter()
                .filter(|h| h.0.contains(card))
                .map(|h| h.1 - fit(h.0, base, &value, mono))
                .collect();
            *value.get_mut(card.as_str()).unwrap() += mean(&residuals);
        }
        let residuals: Vec<f64> = hands
            .iter()
            .filter(|h| same_color(&h.0[0], &h.0[1]))
            .map(|h| h.1 - fit(h.0, base, &value, mono))
            .collect();
        mono += mean(&residuals);
        let shift = mean(&value.values().copied().collect::<Vec<_>>());
        value.values_mut().for_each(|v| *v -= shift);
        base += 2.0 * shift;
        let residuals: Vec<f64> = hands
            .iter()
            .map(|h| h.1 - fit(h.0, base, &value, mono))
            .collect();
        base += mean(&residuals);
    }
    let z: Vec<f64> = hands
        .iter()
        .map(|h| (h.1 - fit(h.0, base, &value, mono)) / h.2)
        .collect();
    let spread =
        (z.iter().map(|z| z * z).sum::<f64>() / (hands.len() - cards.len() - 1) as f64).sqrt();
    let hand_se = mean(&hands.iter().map(|h| h.2).collect::<Vec<_>>());
    let n = cards.len() as f64;

    println!(
        "{} games; the seat taking the first turn scores {:.1}%",
        lines.len(),
        mean(&first_turn) * 100.0
    );
    println!(
        "\nA hand scores {:.1}% plus its two cards' values, {:+.1} (±{:.1}) if they share a color.",
        base * 100.0,
        mono * 100.0,
        hand_se / n.sqrt() * 100.0
    );
    println!(
        "Hands are measured to ±{:.1}; they sit {spread:.1} times that far from this fit.",
        hand_se * 100.0
    );
    println!(
        "\nCard            value (±{:.1})   melded minus kept (blank: within 3 standard errors)",
        hand_se * (n - 1.0).sqrt() / (n - 2.0) * 100.0
    );
    let mut order = cards.clone();
    order.sort_by(|a, b| value[b.as_str()].total_cmp(&value[a.as_str()]));
    for card in order {
        let (m, k) = (&melded[card], &kept[card]);
        let gap = mean(m) - mean(k);
        let gap_se = (se(mean(m), m.len()).powi(2) + se(mean(k), k.len()).powi(2)).sqrt();
        let meld = if gap.abs() > 3.0 * gap_se {
            format!("{:+5.1} ±{:.1}", gap * 100.0, gap_se * 100.0)
        } else {
            String::new()
        };
        println!(
            "{card:15} {:+5.1}          {meld}",
            value[card.as_str()] * 100.0
        );
    }
    let mut ranked = hands.clone();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("\nBest and worst hands");
    for h in ranked
        .iter()
        .take(3)
        .chain(ranked.iter().rev().take(3).rev())
    {
        println!(
            "  {:.1}% ±{:.1}  {} + {}",
            h.1 * 100.0,
            h.2 * 100.0,
            h.0[0],
            h.0[1]
        );
    }
}
