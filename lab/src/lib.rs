//! Shared pieces of the experiments in `src/bin/`: players named by Elo or
//! setting ([`Ai`]), whole games ([`play`]), a seeded parallel runner
//! whose output is the same on every run ([`run_ordered`]), a provenance
//! header for raw output ([`header`]) and `key=value` options ([`Args`]).
//! See `README.md` for the conventions and the index of reports.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use inno_engine::*;

/// Games past this many decisions stop and count as draws.
pub const DECISION_CAP: usize = 3000;
/// The app's learned-search leaf batch.
const LEAF_BATCH: usize = 8;

/// One side's play with the shipped network.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ai {
    /// The policy output sampled at `softmax(policy / temperature)`: 0 is
    /// its best option, infinity uniform random.
    Policy { temperature: f64 },
    /// Learned search of `sims` simulations, playing the most-visited move.
    Search { sims: u32 },
}

impl Ai {
    /// What the app plays at `elo` ([`inno_engine::strength::setting`]).
    pub fn elo(elo: u32) -> Self {
        match strength::setting(elo) {
            (0, temperature) => Self::Policy { temperature },
            (sims, _) => Self::Search {
                sims: u32::try_from(sims).expect("simulations fit u32"),
            },
        }
    }

    /// The step this player takes at `m`'s prompt. A refused search or
    /// policy falls back to the greedy Q, then to a random step, as the app
    /// does.
    pub fn step(&self, m: &Match, net: &Net, rng: &mut Rng) -> Step {
        let player = m.to_move().expect("a player to move");
        let chosen = match *self {
            Self::Policy { temperature } => m
                .game()
                .observe(player)
                .ok()
                .and_then(|o| net.sample_policy(&o, temperature, rng))
                .and_then(|i| option_step(m, i)),
            Self::Search { sims } => {
                let config = SearchConfig {
                    seed: rng.next_u64(),
                    ..SearchConfig::learned(sims)
                };
                m.search_net_step(net, config, LEAF_BATCH).ok()
            }
        };
        chosen
            .or_else(|| {
                let i = net.greedy(&m.game().observe(player).ok()?)?;
                option_step(m, i)
            })
            .or_else(|| m.random_step(rng.next_u64()))
            .expect("a legal step")
    }
}

/// `elo:N` (the app's setting), `t:X` (policy at temperature X, `inf`
/// allowed) or `sims:N` (search).
impl FromStr for Ai {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("player `{s}`: elo:N, t:X or sims:N");
        let (kind, value) = s.split_once(':').ok_or_else(bad)?;
        match kind {
            "elo" => Ok(Self::elo(value.parse().map_err(|_| bad())?)),
            "t" => Ok(Self::Policy {
                temperature: value.parse().map_err(|_| bad())?,
            }),
            "sims" => Ok(Self::Search {
                sims: value.parse().map_err(|_| bad())?,
            }),
            _ => Err(bad()),
        }
    }
}

impl fmt::Display for Ai {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy { temperature } => write!(f, "t:{temperature}"),
            Self::Search { sims } => write!(f, "sims:{sims}"),
        }
    }
}

/// The step for option `index` of `m`'s prompt.
pub fn option_step(m: &Match, index: usize) -> Option<Step> {
    match m.prompt() {
        Prompt::Decision(d) => (index < d.options.len()).then_some(Step::Choice(index)),
        Prompt::Turn { actions, .. } => actions.get(index).map(|&a| Step::Action(a)),
        Prompt::Over(_) => None,
    }
}

/// How a game ended: the winner (None for a draw or a capped game) and the
/// decisions played.
#[derive(Clone, Copy, Debug)]
pub struct Finished {
    pub winner: Option<Player>,
    pub decisions: usize,
}

/// Make both starting melds: `pick(m, seat)` names the index, in hand
/// order, of the card `seat` melds. Returns each seat's melded card.
pub fn play_opening(m: &mut Match, mut pick: impl FnMut(&Match, Player) -> usize) -> [CardId; 2] {
    let mut melds = [CardId(0); 2];
    for _ in 0..2 {
        let seat = m.to_move().expect("an opening meld to choose");
        assert!(
            matches!(m.prompt(), Prompt::Decision(_)),
            "an opening decision"
        );
        let index = pick(m, seat);
        melds[seat.index()] = m.game().position().cards(Zone::Hand(seat))[index];
        m.apply(Step::Choice(index)).expect("a legal opening meld");
    }
    melds
}

/// Play `m` to the end, `players[i]` choosing for seat `i`, every random
/// choice drawn from `rng`.
pub fn play(m: &mut Match, players: [Ai; 2], net: &Net, rng: &mut Rng) -> Finished {
    let mut decisions = 0;
    while let Some(seat) = m.to_move() {
        decisions += 1;
        if decisions > DECISION_CAP {
            return Finished {
                winner: None,
                decisions,
            };
        }
        let step = players[seat.index()].step(m, net, rng);
        m.apply(step).expect("a legal step");
    }
    let winner = match m.prompt() {
        Prompt::Over(MatchOutcome::Winner { player, .. }) => Some(*player),
        _ => None,
    };
    Finished { winner, decisions }
}

/// The printed title of a Base card.
pub fn card_title(card: CardId) -> &'static str {
    BASE_METADATA[usize::from(card.0)].title
}

/// A seat's hand as sorted titles.
pub fn hand(game: &Game, seat: Player) -> Vec<&'static str> {
    let mut h: Vec<_> = game
        .position()
        .cards(Zone::Hand(seat))
        .iter()
        .map(|&c| card_title(c))
        .collect();
    h.sort_unstable();
    h
}

/// A `+`-separated list of Age 1 Base titles, any case (`clothing+metalworking`),
/// as sorted printed titles.
pub fn parse_hand(text: &str) -> Result<Vec<&'static str>, String> {
    let mut cards = text
        .split('+')
        .map(|name| {
            BASE_METADATA
                .iter()
                .find(|m| m.value == 1 && m.title.eq_ignore_ascii_case(name.trim()))
                .map(|m| m.title)
                .ok_or_else(|| format!("`{name}` is not an Age 1 Base card"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    cards.sort_unstable();
    Ok(cards)
}

/// Score from wins, losses and draws (draws count 1/2), with its standard
/// error.
pub fn score(wins: u64, losses: u64, draws: u64) -> (f64, f64) {
    let n = (wins + losses + draws) as f64;
    let s = (wins as f64 + draws as f64 / 2.0) / n;
    (s, (s * (1.0 - s) / n).sqrt())
}

/// The Elo difference a score implies (infinite at 0 or 1).
pub fn elo_gap(score: f64) -> f64 {
    400.0 * (score / (1.0 - score)).log10()
}

/// The standard error, in Elo, of the gap a `(score, standard error)` implies.
pub fn elo_se((score, se): (f64, f64)) -> f64 {
    400.0 / std::f64::consts::LN_10 / (score * (1.0 - score)) * se
}

/// `key=value` options after optional positional arguments. Every option
/// read must be named in `known`, so a mistyped one fails loudly.
pub struct Args {
    pub positional: Vec<String>,
    options: BTreeMap<String, String>,
}

impl Args {
    pub fn from_env(known: &[&str]) -> Self {
        let mut positional = Vec::new();
        let mut options = BTreeMap::new();
        for arg in std::env::args().skip(1) {
            match arg.split_once('=') {
                Some((k, v)) => {
                    assert!(
                        known.contains(&k),
                        "unknown option `{k}`; known: {}",
                        known.join(", ")
                    );
                    options.insert(k.to_string(), v.to_string());
                }
                None => positional.push(arg),
            }
        }
        Self {
            positional,
            options,
        }
    }

    /// The option's value, or `default`.
    pub fn get<T: FromStr>(&self, key: &str, default: T) -> T
    where
        T::Err: fmt::Display,
    {
        self.options.get(key).map_or(default, |v| {
            v.parse()
                .unwrap_or_else(|e| panic!("option {key}={v}: {e}"))
        })
    }

    /// Every option given, as `key=value`, for the provenance header.
    pub fn describe(&self) -> String {
        self.options
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The `weights=` option, or the shipped network.
    pub fn weights_path(&self) -> PathBuf {
        self.options.get("weights").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../weights/inari_v1.0.bin"),
            PathBuf::from,
        )
    }

    /// The `workers=` option, or all but two cores.
    pub fn workers(&self) -> usize {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        self.get("workers", cores.saturating_sub(2).max(1))
    }
}

/// `#` lines that start every raw output: the experiment and its options,
/// the source revision and the weights' digest. Readers skip `#` lines.
pub fn header(experiment: &str, args: &Args, weights: &Path) -> String {
    let revision = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=12"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or("unknown".into(), |o| {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        });
    let bytes = std::fs::read(weights).expect("weights file");
    let weights = weights.canonicalize().expect("weights path");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.canonicalize().ok());
    let weights = root
        .and_then(|r| weights.strip_prefix(r).ok().map(Path::to_path_buf))
        .unwrap_or(weights);
    format!(
        "# lab {experiment} {}\n# revision {revision}\n# weights {} fnv1a64 {:016x}",
        args.describe(),
        weights.display(),
        fnv1a64(&bytes)
    )
}

/// Load the network from `path`.
pub fn load_net(path: &Path) -> Net {
    Net::load(path).unwrap_or_else(|e| panic!("weights {}: {e}", path.display()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Run `f` over `jobs` in `workers` threads and print each result line to
/// stdout in job order, so the output does not depend on thread timing.
/// Progress goes to stderr.
pub fn run_ordered<J: Sync>(jobs: &[J], workers: usize, f: impl Fn(&J) -> String + Sync) {
    run_ordered_to(jobs, workers, f, &mut std::io::stdout());
    eprintln!();
}

/// [`run_ordered`] writing to `out`.
fn run_ordered_to<J: Sync>(
    jobs: &[J],
    workers: usize,
    f: impl Fn(&J) -> String + Sync,
    out: &mut (impl Write + Send),
) {
    let next = AtomicUsize::new(0);
    let done = Mutex::new((0, BTreeMap::new(), out));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let line = f(job);
                    let mut guard = done.lock().unwrap();
                    let (printed, pending, out) = &mut *guard;
                    pending.insert(i, line);
                    while let Some(line) = pending.remove(printed) {
                        writeln!(out, "{line}").expect("output");
                        *printed += 1;
                        if *printed % 100 == 0 || *printed == jobs.len() {
                            eprint!("\r{}/{}", printed, jobs.len());
                        }
                    }
                }
            });
        }
    });
}

/// The data lines of a raw output file: `#` lines and the column line
/// (starting `columns`) skipped.
pub fn data_lines(path: &str) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("columns") && !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_parse_as_the_app_plays_them() {
        assert_eq!("elo:1200".parse(), Ok(Ai::Search { sims: 128 }));
        assert_eq!("elo:860".parse(), Ok(Ai::Policy { temperature: 0.0 }));
        assert_eq!("sims:13".parse(), Ok(Ai::Search { sims: 13 }));
        assert_eq!(
            "t:inf".parse(),
            Ok(Ai::Policy {
                temperature: f64::INFINITY
            })
        );
        assert!("elo".parse::<Ai>().is_err() && "beta:1".parse::<Ai>().is_err());
        assert_eq!(Ai::Search { sims: 128 }.to_string(), "sims:128");
    }

    #[test]
    fn hands_name_age_one_base_cards_in_any_case() {
        assert_eq!(
            parse_hand("metalworking+Clothing"),
            Ok(vec!["CLOTHING", "METALWORKING"])
        );
        assert!(parse_hand("Clothing+Calendar").is_err());
    }

    #[test]
    fn results_print_in_job_order_and_scores_convert_to_elo() {
        // Later jobs finish first, yet lines come out in job order.
        let jobs: Vec<u64> = (0..64).collect();
        let mut out = Vec::new();
        run_ordered_to(
            &jobs,
            8,
            |&j| {
                std::thread::sleep(std::time::Duration::from_micros(64 - j));
                j.to_string()
            },
            &mut out,
        );
        let lines: Vec<u64> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| l.parse().unwrap())
            .collect();
        assert_eq!(lines, jobs);
        assert!(elo_gap(0.5).abs() < 1e-9);
        assert!((elo_gap(0.76) - 200.0).abs() < 1.0);
        let (s, se) = score(3, 1, 0);
        assert!((s - 0.75).abs() < 1e-9 && se > 0.0);
    }
}
