//! How games end. Self-play on random deals of one expansion combination,
//! recording each game's ending and, for a win by card effect, the card.
//!
//! ```sh
//! pixi run lab endings run expansions=artifacts > lab/runs/x.csv
//! pixi run lab endings summary lab/runs/x.csv
//! ```
//!
//! Options: `expansions=NAME` (an `Expansions::name`: `base`, `artifacts`,
//! `figures+cities`, ...; default `base`), `play=` (a player, `elo:N`,
//! `t:X` or `sims:N`; default elo:800), `games=N` (default 50,000) from
//! deal seed `first=S` (default 3,000,000), `workers=N`, `weights=PATH`.
//!
//! One row per game: `seed,winner,ending,card,set,decisions`. `winner` is 1, 2
//! or 0. `ending` is `achievements`, `score` (the decks ran out), `effect`,
//! `karma` or `elimination` for a win; `draw-score`, `draw-effect` or
//! `draw-loop` for a draw; `capped` for a game stopped at the decision cap.
//! `card` and `set` are the title and set of the card whose effect or karma
//! ended the game.
use std::collections::BTreeMap;

use inno_engine::*;
use lab::*;

const OPTIONS: [&str; 6] = ["expansions", "play", "games", "first", "workers", "weights"];

fn main() {
    let args = Args::from_env(&OPTIONS);
    match args.positional.first().map(String::as_str) {
        Some("run") => run(&args),
        Some("summary") => summary(args.positional.get(1).expect("summary <file>")),
        _ => panic!("usage: endings run [options] | endings summary <file>"),
    }
}

fn run(args: &Args) {
    let name: String = args.get("expansions", "base".to_string());
    let expansions = Expansions::from_name(&name)
        .unwrap_or_else(|| panic!("expansions={name}: not an expansion combination"));
    let ai: Ai = args.get("play", Ai::elo(800));
    let games: u64 = args.get("games", 50_000);
    let first: u64 = args.get("first", 3_000_000);
    let weights = args.weights_path();
    let net = load_net(&weights);
    let seeds: Vec<u64> = (first..first + games).collect();
    println!("{}", header("endings", args, &weights));
    println!("# expansions={name} play={ai}");
    println!("columns seed,winner,ending,card,set,decisions");
    run_ordered(&seeds, args.workers(), |&seed| {
        let mut m = Match::new(expansions, seed).expect("a deal");
        let mut rng = Rng::new(seed ^ 0x0E4D_1265);
        let end = play(&mut m, [ai; 2], &net, &mut rng);
        // The deciding card's title and set, as `title,set`.
        let card = |victory: &Victory, winner: Player| {
            victory
                .source_card(winner)
                .or_else(|| victory.source_card(winner.opponent()))
                .map(|id| m.game().position().card(id))
                .map_or(",".to_string(), |card| {
                    format!("{},{:?}", title(card.code).unwrap_or(card.code), card.set)
                })
        };
        let (winner, ending, card) = match m.prompt() {
            Prompt::Over(MatchOutcome::Winner { player, victory }) => {
                let (ending, named) = match victory.reason {
                    WinReason::Achievements => ("achievements", false),
                    WinReason::Exhaustion => ("score", false),
                    WinReason::Effect => ("effect", true),
                    WinReason::Karma => ("karma", true),
                    WinReason::Elimination { .. } => ("elimination", true),
                };
                let card = if named {
                    card(victory, *player)
                } else {
                    ",".to_string()
                };
                (player.index() + 1, ending, card)
            }
            Prompt::Over(MatchOutcome::Draw(reason)) => {
                let ending = match reason {
                    DrawReason::Exhaustion => "draw-score",
                    DrawReason::Effect => "draw-effect",
                    DrawReason::Loop => "draw-loop",
                };
                (0, ending, ",".to_string())
            }
            _ => (0, "capped", ",".to_string()),
        };
        format!("{seed},{winner},{ending},{card},{}", end.decisions)
    });
}

fn summary(path: &str) {
    let mut endings: BTreeMap<String, u64> = BTreeMap::new();
    let mut cards: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut sets: BTreeMap<String, u64> = BTreeMap::new();
    let lines = data_lines(path);
    for line in &lines {
        let f: Vec<&str> = line.split(',').collect();
        *endings.entry(f[2].to_string()).or_default() += 1;
        if !f[3].is_empty() {
            *cards
                .entry((f[3].to_string(), f[4].to_string()))
                .or_default() += 1;
            *sets.entry(f[4].to_string()).or_default() += 1;
        }
    }
    let n = lines.len() as f64;
    let percent = |count: u64| count as f64 / n * 100.0;
    let by_count = |map: BTreeMap<String, u64>| {
        let mut rows: Vec<_> = map.into_iter().collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.1));
        rows
    };
    println!("{} games", lines.len());
    for (ending, count) in by_count(endings) {
        println!("  {ending:13} {:5.1}%  {count:6}", percent(count));
    }
    println!("\nGames ended by a card (effect, karma or elimination), by set");
    for (set, count) in by_count(sets) {
        println!("  {set:13} {:5.1}%", percent(count));
    }
    println!("\nand by card");
    let mut cards: Vec<_> = cards.into_iter().collect();
    cards.sort_by_key(|row| std::cmp::Reverse(row.1));
    for ((card, set), count) in cards {
        println!("  {:5.2}%  {count:6}  {card} ({set})", percent(count));
    }
}
