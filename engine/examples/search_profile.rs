//! Replay roots from lines `mode seed choice...`, then measure learned tree
//! overhead with fixed synthetic Q values. No network or training updates.
use inno_engine::{Expansions, Match, Prompt, SearchConfig, SearchTree, Step};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let input = std::fs::read_to_string(args.next().expect("root corpus")).unwrap();
    let repeats: usize = args.next().map_or(1, |s| s.parse().unwrap());
    let mut roots = Vec::new();
    for line in input.lines() {
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
        roots.push(game);
    }
    let start = Instant::now();
    let mut digest = 0u64;
    let mut leaves = 0usize;
    let mut refused = 0;
    for _ in 0..repeats {
        for root in &roots {
            let mut tree = match SearchTree::new(root.game(), SearchConfig::learned(64), true) {
                Ok(tree) => tree,
                Err(_) => {
                    refused += 1;
                    continue;
                }
            };
            while !tree.done() {
                tree.select(8).unwrap();
                let q: Vec<Vec<f32>> = tree
                    .pending()
                    .map(|obs| {
                        leaves += 1;
                        for value in obs.row() {
                            digest = digest
                                .wrapping_mul(31)
                                .wrapping_add(u64::from(value.to_bits()));
                        }
                        (0..obs.options.len())
                            .map(|i| (i % 3) as f32 * 0.1 - 0.1)
                            .collect()
                    })
                    .collect();
                tree.resolve(&q).unwrap();
            }
            for m in tree.result().unwrap().moves {
                digest = digest.wrapping_mul(31).wrapping_add(u64::from(m.visits));
            }
        }
    }
    println!(
        "roots={} repeats={repeats} leaves={leaves} refused={refused} digest={digest:016x} seconds={:.6}",
        roots.len(),
        start.elapsed().as_secs_f64()
    );
}
