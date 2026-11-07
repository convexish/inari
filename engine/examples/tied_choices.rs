//! Play greedy self-play games with exported weights and print every
//! decision whose options all get the same Q (best minus worst below 1e-4),
//! with its source card and options, card ids shown as publisher codes.
//! Games cycle the 32 training modes. Usage: tied_choices WEIGHTS GAMES SEED THREADS
use inno_engine::{Game, Match, Net, Prompt, Step, TRAINING_MODES, mode_expansions};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const TIED_Q_SPREAD: f32 = 1e-4;

/// `text` with every `CardId(n)` replaced by that card's publisher code.
fn codes(game: &Game, text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("CardId(") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 7..];
        let end = tail.find(')').unwrap();
        let id = inno_engine::CardId(tail[..end].parse().unwrap());
        out.push_str(game.position().card(id).code);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let net = Net::load(std::path::Path::new(&args.next().expect("weights"))).unwrap();
    let games: usize = args.next().map_or(64, |s| s.parse().unwrap());
    let seed: u64 = args.next().map_or(1, |s| s.parse().unwrap());
    let threads: usize = args.next().map_or(4, |s| s.parse().unwrap());
    let next = AtomicUsize::new(0);
    let totals = Mutex::new((0usize, 0usize));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let g = next.fetch_add(1, Ordering::Relaxed);
                    if g >= games {
                        break;
                    }
                    let mode = (g % TRAINING_MODES.len()) as u8;
                    let seed = seed + g as u64;
                    let mut m = Match::new(mode_expansions(mode).unwrap(), seed).unwrap();
                    let (mut choices, mut tied, mut lines) = (0, 0, String::new());
                    for decision in 0.. {
                        let Some(player) = m.to_move() else { break };
                        if decision >= 3000 {
                            break;
                        }
                        let observation = m.game().observe(player).unwrap();
                        let q = net.q(&observation).unwrap();
                        let steps: Vec<Step> = match m.prompt() {
                            Prompt::Decision(d) => (0..d.options.len()).map(Step::Choice).collect(),
                            Prompt::Turn { actions, .. } => {
                                actions.iter().copied().map(Step::Action).collect()
                            }
                            Prompt::Over(_) => unreachable!(),
                        };
                        assert_eq!(q.len(), steps.len());
                        let best = q.iter().copied().fold(f32::MIN, f32::max);
                        let worst = q.iter().copied().fold(f32::MAX, f32::min);
                        if steps.len() > 1 {
                            choices += 1;
                            if best - worst < TIED_Q_SPREAD {
                                tied += 1;
                                let game = m.game();
                                let (kind, source, options) = match m.prompt() {
                                    Prompt::Decision(d) => (
                                        "decision",
                                        d.source_card(player)
                                            .map_or("hidden or none", |c| game.position().card(c).code),
                                        d.options.iter().map(|o| format!("{o:?}")).collect(),
                                    ),
                                    Prompt::Turn { actions, .. } => (
                                        "turn",
                                        "-",
                                        actions.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>(),
                                    ),
                                    Prompt::Over(_) => unreachable!(),
                                };
                                lines.push_str(&format!(
                                    "TIE mode {} seed {seed} decision {decision} {kind} source {source} spread {:.1e} q {best:+.3}\n",
                                    TRAINING_MODES[mode as usize],
                                    best - worst
                                ));
                                for (i, option) in options.iter().enumerate() {
                                    lines.push_str(&format!("  {i}: {}\n", codes(game, option)));
                                }
                            }
                        }
                        let pick = q
                            .iter()
                            .enumerate()
                            .max_by(|a, b| a.1.total_cmp(b.1))
                            .map_or(0, |(i, _)| i);
                        m.apply(steps[pick]).unwrap();
                    }
                    let mut totals = totals.lock().unwrap();
                    print!("{lines}");
                    totals.0 += choices;
                    totals.1 += tied;
                }
            });
        }
    });
    let (choices, tied) = totals.into_inner().unwrap();
    println!("games={games} choices={choices} tied={tied}");
}
