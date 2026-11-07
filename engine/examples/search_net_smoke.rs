//! Learned search against the same network's greedy play, or against another
//! search setting, on duplicate deals with seats swapped. `cargo run --release
//! --example search_net_smoke -- <weights.bin> <deals> <iterations>
//! [workers=N] [first=K] [c=X] [temperature=X] [leaf_batch=N]
//! [vs_iterations=N] [vs_c=X] [vs_temperature=X] [vs_vloss=X] [vs_weights=PATH]
//! [rule=visits|value|shrunk|netq|prior] [sample=T] [shrink=K] [vloss=X] [policy=on|off]
//! [vs_policy=on|off] [vs_beta=X] [subgames=on|off]`. A positive `vs_iterations` makes the
//! opponent search too (its `c` and temperature default to the searcher's),
//! and `vs_weights` gives the opponent its own network, so two checkpoints can
//! meet. `vs_beta` instead has the opponent play its policy output without
//! search, sampled with probability proportional to `exp(X * logit)` (`inf`
//! for its top move). A network with a policy output takes its search priors from it
//! unless that side's `policy` is off; priors are then `softmax(policy / T)`. `rule` and `sample` choose the move from the root statistics (see
//! [`pick`]); the default plays the most-visited move as the engine's own
//! recommendation does. Deals `first..first + deals` play in
//! `workers` threads; deal
//! `k` uses training mode `k % 32`. Each search runs one tree as training
//! does, with up to `leaf_batch` leaves waiting at once, and plays its
//! most-visited move; a refused root plays greedily. `subgames=on` plays
//! Martian Internet's printed subgame instead of the draw variant (the search
//! still values the effect as a draw and plays a subgame to win it); the
//! per-deal line then counts the deal's subgames and the summary says how they
//! ended. Reports the searcher's
//! score with its standard error over deals, wins, losses, draws (games past
//! 3,000 decisions are capped draws) and time per searched move. A smoke test,
//! not a gate.
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use inno_engine::*;

const DECISION_CAP: usize = 3000;

/// One side's network, and whether its search priors come from the network's
/// policy output (when it has one) rather than from Q.
#[derive(Clone, Copy)]
struct Side<'a> {
    net: &'a Net,
    policy: bool,
}

#[derive(Default)]
struct Totals {
    /// The searcher's mean score over its two seats, per deal.
    pairs: Vec<f64>,
    wins: u32,
    losses: u32,
    draws: u32,
    capped: u32,
    refused: u64,
    searched: u64,
    seconds: f64,
    /// The searcher's simulations: count, summed depth, deepest, how many
    /// answered an opponent prompt, and a histogram of depths 1..=5 and 6+.
    leaves: u64,
    /// Network calls (leaf batches) the searcher's trees made.
    batches: u64,
    depth_sum: u64,
    depth_max: u32,
    past_opponent: u64,
    depth_hist: [u64; 6],
    /// Roots with two or more distinct moves, and the summed share of their
    /// visits on the most-visited move.
    roots: u64,
    share_sum: f64,
    /// Martian Internet subgames started, and how the finished ones ended:
    /// won by the searcher, won by the opponent, drawn.
    subgames: u64,
    subgames_won: u64,
    subgames_lost: u64,
    subgames_drawn: u64,
}

/// Per-search diagnostics the searcher's trees report.
#[derive(Default)]
struct Diag {
    depths: Vec<(u32, bool)>,
    /// Network calls made.
    batches: u64,
    /// Share of root visits on the most-visited move, for roots with two or
    /// more distinct moves.
    shares: Vec<f64>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("weights path");
    let net = Net::load(std::path::Path::new(&path)).expect("next-1 weights");
    let deals: u64 = args.next().map_or(4, |a| a.parse().expect("deals"));
    let iterations: u32 = args.next().map_or(128, |a| a.parse().expect("iterations"));
    let rest: Vec<String> = args.collect();
    let option = |name: &str| rest.iter().find_map(|a| a.strip_prefix(name));
    let workers: u64 = option("workers=").map_or(1, |n| n.parse().expect("workers"));
    let first: u64 = option("first=").map_or(0, |n| n.parse().expect("first"));
    let leaf_batch: usize = option("leaf_batch=").map_or(1, |n| n.parse().expect("leaf_batch"));
    let rule = match option("rule=").unwrap_or("visits") {
        "visits" => Rule::Visits,
        "value" => Rule::Value,
        "shrunk" => Rule::Shrunk,
        "netq" => Rule::NetQ,
        "prior" => Rule::Prior,
        other => panic!("rule={other}: visits, value, shrunk, netq or prior"),
    };
    let sample: f64 = option("sample=").map_or(0.0, |t| t.parse().expect("sample"));
    let shrink: f64 = option("shrink=").map_or(4.0, |k| k.parse().expect("shrink"));
    let mut config = SearchConfig::learned(iterations);
    if let Some(c) = option("c=") {
        config.exploration = c.parse().expect("c");
    }
    if let Some(t) = option("temperature=") {
        config.prior_temperature = t.parse().expect("temperature");
    }
    if let Some(v) = option("vloss=") {
        config.virtual_loss = v.parse().expect("vloss");
    }
    let vs_iterations: u32 =
        option("vs_iterations=").map_or(0, |n| n.parse().expect("vs_iterations"));
    let opponent = (vs_iterations > 0).then(|| SearchConfig {
        iterations: vs_iterations,
        exploration: option("vs_c=").map_or(config.exploration, |c| c.parse().expect("vs_c")),
        prior_temperature: option("vs_temperature=").map_or(config.prior_temperature, |t| {
            t.parse().expect("vs_temperature")
        }),
        virtual_loss: option("vs_vloss=")
            .map_or(config.virtual_loss, |v| v.parse().expect("vs_vloss")),
        ..config
    });
    let vs_beta: Option<f64> = option("vs_beta=").map(|b| b.parse().expect("vs_beta"));
    let opponent_net = option("vs_weights=")
        .map(|p| Net::load(std::path::Path::new(p)).expect("opponent weights"));
    let switch = |name: &str| match option(name) {
        None | Some("on") => true,
        Some("off") => false,
        Some(other) => panic!("{name}{other}: on or off"),
    };
    let mine = Side {
        net: &net,
        policy: switch("policy=") && net.has_policy(),
    };
    let other_net = opponent_net.as_ref().unwrap_or(&net);
    assert!(
        vs_beta.is_none() || other_net.has_policy(),
        "vs_beta needs an opponent network with a policy output"
    );
    let other = Side {
        net: other_net,
        policy: switch("vs_policy=") && other_net.has_policy(),
    };
    let subgames = match option("subgames=") {
        None | Some("off") => false,
        Some("on") => true,
        Some(other) => panic!("subgames={other}: on or off"),
    };
    let next = AtomicU64::new(first);
    let totals = Mutex::new(Totals::default());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let deal = next.fetch_add(1, Ordering::Relaxed);
                    if deal >= first + deals {
                        break;
                    }
                    play_deal(
                        mine, other, deal, config, opponent, vs_beta, leaf_batch, rule, sample,
                        shrink, subgames, &totals,
                    );
                }
            });
        }
    });
    let t = totals.into_inner().unwrap();
    let n = t.pairs.len() as f64;
    let mean = t.pairs.iter().sum::<f64>() / n;
    let variance = t.pairs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let against = opponent.map_or("greedy".to_string(), |o| {
        format!(
            "search {} c={} temperature={} vloss={}",
            o.iterations, o.exploration, o.prior_temperature, o.virtual_loss
        )
    });
    let against = match option("vs_weights=") {
        Some(path) => format!("{against} with {path}"),
        None => against,
    };
    let against = match vs_beta {
        Some(beta) => format!("policy sampled at beta {beta}"),
        None => format!("{against} policy={}", other.policy),
    };
    println!(
        "search {iterations} c={} temperature={} leaf_batch={leaf_batch} vloss={} rule={rule:?} sample={sample} \
         shrink={shrink} policy={} subgames={subgames} against {against}: \
         score {mean:.4} ± {:.4} \
         over {} deals; {} wins, {} losses, {} draws ({} capped); {} refused roots; \
         {:.1} ms per searched move",
        config.exploration,
        config.prior_temperature,
        config.virtual_loss,
        mine.policy,
        (variance / n).sqrt(),
        t.pairs.len(),
        t.wins,
        t.losses,
        t.draws,
        t.capped,
        t.refused,
        t.seconds * 1e3 / t.searched.max(1) as f64
    );
    let n = t.leaves.max(1) as f64;
    println!(
        "leaf depth mean {:.2}, max {}; shares at depth 1/2/3/4/5/6+: {}; {:.1}% of simulations answered an \
         opponent prompt; {:.2} leaves per network call",
        t.depth_sum as f64 / n,
        t.depth_max,
        t.depth_hist
            .iter()
            .map(|&c| format!("{:.2}", c as f64 / n))
            .collect::<Vec<_>>()
            .join("/"),
        100.0 * t.past_opponent as f64 / n,
        n / t.batches.max(1) as f64
    );
    println!(
        "mean share of root visits on the most-visited move: {:.3} over {} roots with two or more moves",
        t.share_sum / t.roots.max(1) as f64,
        t.roots
    );
    if subgames {
        println!(
            "{} Martian Internet subgames started: {} won by the searcher, {} by the opponent, {} drawn",
            t.subgames, t.subgames_won, t.subgames_lost, t.subgames_drawn
        );
    }
}

/// Play one deal from both seats and add it to the totals; the opponent
/// plays with `other`.
#[allow(clippy::too_many_arguments)]
fn play_deal(
    net: Side,
    other: Side,
    deal: u64,
    config: SearchConfig,
    opponent: Option<SearchConfig>,
    vs_beta: Option<f64>,
    leaf_batch: usize,
    rule: Rule,
    sample: f64,
    shrink: f64,
    subgames: bool,
    totals: &Mutex<Totals>,
) {
    let expansions = Expansions::from_bits((deal % 32) as u8).unwrap();
    let mut local = Totals::default();
    let mut diag = Diag::default();
    let mut pair = 0.0;
    for searcher in [Player::One, Player::Two] {
        let mut m = Match::with_subgames(expansions, deal, subgames).unwrap();
        let mut rng = Rng::new(deal ^ 0xD0E1);
        let mut opponent_rng = Rng::new(deal ^ 0x0BB0);
        let mut decisions = 0;
        while let Some(player) = m.to_move() {
            decisions += 1;
            if decisions > DECISION_CAP {
                break;
            }
            let step = if player == searcher {
                let start = Instant::now();
                let config = SearchConfig {
                    seed: rng.next_u64(),
                    ..config
                };
                let step = search(
                    &m, net, config, leaf_batch, rule, sample, shrink, &mut rng, &mut diag,
                );
                local.seconds += start.elapsed().as_secs_f64();
                local.searched += 1;
                local.refused += u64::from(step.is_none());
                step
            } else if let Some(beta) = vs_beta {
                policy_step(&m, other.net, beta, &mut opponent_rng)
            } else {
                opponent.and_then(|config| {
                    let config = SearchConfig {
                        seed: opponent_rng.next_u64(),
                        ..config
                    };
                    search(
                        &m,
                        other,
                        config,
                        leaf_batch,
                        rule,
                        sample,
                        shrink,
                        &mut opponent_rng,
                        &mut Diag::default(),
                    )
                })
            };
            let fallback = if player == searcher { net } else { other };
            let step = step
                .or_else(|| greedy(&m, fallback.net))
                .expect("a legal step");
            m.apply(step).unwrap();
        }
        let score = match m.prompt() {
            Prompt::Over(MatchOutcome::Winner { player, .. }) if *player == searcher => {
                local.wins += 1;
                1.0
            }
            Prompt::Over(MatchOutcome::Winner { .. }) => {
                local.losses += 1;
                0.0
            }
            Prompt::Over(_) => {
                local.draws += 1;
                0.5
            }
            _ => {
                local.draws += 1;
                local.capped += 1;
                0.5
            }
        };
        pair += score / 2.0;
        for (_, event) in m.events() {
            match event {
                Event::SubgameStarted { .. } => local.subgames += 1,
                Event::SubgameEnded { winner, .. } => match winner {
                    Some(player) if *player == searcher => local.subgames_won += 1,
                    Some(_) => local.subgames_lost += 1,
                    None => local.subgames_drawn += 1,
                },
                _ => {}
            }
        }
    }
    let played = if local.subgames > 0 {
        format!("; {} subgames", local.subgames)
    } else {
        String::new()
    };
    println!(
        "deal {deal} ({}): searcher scored {pair}{played}",
        TRAINING_MODES[(deal % 32) as usize]
    );
    local.roots += diag.shares.len() as u64;
    local.batches += diag.batches;
    local.share_sum += diag.shares.iter().sum::<f64>();
    for &(depth, crossed) in &diag.depths {
        local.leaves += 1;
        local.depth_sum += u64::from(depth);
        local.depth_max = local.depth_max.max(depth);
        local.past_opponent += u64::from(crossed);
        local.depth_hist[(depth.clamp(1, 6) - 1) as usize] += 1;
    }
    let mut t = totals.lock().unwrap();
    t.pairs.push(pair);
    t.leaves += local.leaves;
    t.batches += local.batches;
    t.depth_sum += local.depth_sum;
    t.depth_max = t.depth_max.max(local.depth_max);
    t.past_opponent += local.past_opponent;
    t.roots += local.roots;
    t.share_sum += local.share_sum;
    for (total, part) in t.depth_hist.iter_mut().zip(local.depth_hist) {
        *total += part;
    }
    t.wins += local.wins;
    t.losses += local.losses;
    t.draws += local.draws;
    t.capped += local.capped;
    t.refused += local.refused;
    t.searched += local.searched;
    t.seconds += local.seconds;
    t.subgames += local.subgames;
    t.subgames_won += local.subgames_won;
    t.subgames_lost += local.subgames_lost;
    t.subgames_drawn += local.subgames_drawn;
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Rule {
    Visits,
    Value,
    Shrunk,
    NetQ,
    Prior,
}

/// The move from the root statistics. `Visits`: the most-visited move (ties
/// by value, then prior), or with `sample` > 0 a draw with weights
/// visits^(1/sample), as training samples. `Value`: the best mean value among
/// moves visited at least twice (every visited move when none was).
/// `Shrunk`: every move scored by its mean pulled toward its network Q with
/// `shrink` pseudo-visits, `(n · mean + shrink · Q) / (n + shrink)`. `NetQ`:
/// the network's root Q alone, as reflex play. `Prior`: the network's root
/// prior alone (its policy when it has one), as greedy policy play, or with
/// `sample` > 0 a draw with weights prior^(1/sample), so `temperature=T
/// sample=1` plays `softmax(policy / T)` as a difficulty setting would. For
/// `Value`, `Shrunk` and `NetQ`, `sample` > 0 draws with weights
/// exp(score / sample).
fn pick(moves: &[RootMove], rule: Rule, sample: f64, shrink: f64, rng: &mut Rng) -> Option<Step> {
    let visited: Vec<&RootMove> = moves.iter().filter(|m| m.visits > 0).collect();
    let pool: Vec<&RootMove> = match rule {
        Rule::Visits => visited,
        Rule::Value => {
            let twice: Vec<&RootMove> = visited.iter().copied().filter(|m| m.visits >= 2).collect();
            if twice.is_empty() { visited } else { twice }
        }
        Rule::Shrunk | Rule::NetQ | Rule::Prior => moves.iter().collect(),
    };
    if pool.is_empty() {
        return moves.first().map(|m| m.step);
    }
    let score = |m: &RootMove| -> f64 {
        match (rule, m.q) {
            (Rule::Visits, _) => f64::from(m.visits),
            (Rule::Value, _) | (Rule::Shrunk | Rule::NetQ, None) => m.value,
            (Rule::Shrunk, Some(q)) => {
                (f64::from(m.visits) * m.value + shrink * q) / (f64::from(m.visits) + shrink)
            }
            (Rule::NetQ, Some(q)) => q,
            (Rule::Prior, _) => m.prior,
        }
    };
    if sample > 0.0 {
        let logs: Vec<f64> = pool
            .iter()
            .map(|m| match rule {
                Rule::Visits | Rule::Prior => score(m).ln() / sample,
                _ => score(m) / sample,
            })
            .collect();
        let top = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let weights: Vec<f64> = logs.iter().map(|l| (l - top).exp()).collect();
        let total: f64 = weights.iter().sum();
        let mut u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * total;
        for (m, w) in pool.iter().zip(&weights) {
            if u < *w {
                return Some(m.step);
            }
            u -= w;
        }
        return pool.last().map(|m| m.step);
    }
    pool.iter()
        .max_by(|a, b| {
            (score(a), a.visits, a.value, a.prior)
                .partial_cmp(&(score(b), b.visits, b.value, b.prior))
                .expect("finite statistics")
        })
        .map(|m| m.step)
}

/// One training-style tree: up to `leaf_batch` leaves wait for the network
/// at once. None when tracking refuses the root or a later world. Appends the
/// tree's diagnostics to `diag`.
#[allow(clippy::too_many_arguments)]
fn search(
    m: &Match,
    side: Side,
    config: SearchConfig,
    leaf_batch: usize,
    rule: Rule,
    sample: f64,
    shrink: f64,
    rng: &mut Rng,
    diag: &mut Diag,
) -> Option<Step> {
    let mut tree = SearchTree::new(m.game(), config, true).ok()?;
    while !tree.done() {
        if tree.select(leaf_batch).ok()? == 0 {
            continue;
        }
        let pending: Vec<_> = tree.pending().collect();
        diag.batches += 1;
        if side.policy {
            let (q, policy): (Vec<Vec<f32>>, Vec<Vec<f32>>) = pending
                .into_iter()
                .map(|observation| {
                    let (q, policy) = side.net.evaluate(observation).ok()?;
                    Some((q, policy?))
                })
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .unzip();
            tree.resolve_with_policy(&q, &policy).ok()?;
        } else {
            let q: Vec<Vec<f32>> = pending
                .into_iter()
                .map(|observation| side.net.q(observation))
                .collect::<Result<_, _>>()
                .ok()?;
            tree.resolve(&q).ok()?;
        }
    }
    diag.depths.extend_from_slice(tree.leaf_depths());
    let result = tree.result().ok()?;
    if result.moves.len() >= 2 {
        let total: u32 = result.moves.iter().map(|m| m.visits).sum();
        if let Some(top) = result.moves.iter().map(|m| m.visits).max()
            && total > 0
        {
            diag.shares.push(f64::from(top) / f64::from(total));
        }
    }
    pick(&result.moves, rule, sample, shrink, rng)
}

fn policy_step(m: &Match, net: &Net, beta: f64, rng: &mut Rng) -> Option<Step> {
    let player = m.to_move()?;
    let logits = net.policy(&m.game().observe(player).ok()?).ok()??;
    let index = inno_engine::net::sample_inverse_temperature(&logits, beta, rng)?;
    Some(match m.prompt() {
        Prompt::Decision(_) => Step::Choice(index),
        Prompt::Turn { actions, .. } => Step::Action(actions[index]),
        Prompt::Over(_) => return None,
    })
}

fn greedy(m: &Match, net: &Net) -> Option<Step> {
    let player = m.to_move()?;
    let index = net.greedy(&m.game().observe(player).ok()?)?;
    Some(match m.prompt() {
        Prompt::Decision(_) => Step::Choice(index),
        Prompt::Turn { actions, .. } => Step::Action(actions[index]),
        Prompt::Over(_) => return None,
    })
}
