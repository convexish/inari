//! Small smoke test: information-set search against uniform random play.
//! `cargo run --release --example search_smoke -- <games> <iterations>
//! [rollout_actions] [full] [cities] [threads=N]`. `full` adds Figures and
//! `cities` adds Cities. Seats alternate; reports wins and
//! time per searched move. Not a strength benchmark.
use std::time::Instant;

use inno_engine::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let games: u64 = args.next().map_or(10, |a| a.parse().expect("games"));
    let iterations: u32 = args.next().map_or(200, |a| a.parse().expect("iterations"));
    let rollout_actions: u32 = args.next().map_or(4, |a| a.parse().expect("rollout"));
    let rest: Vec<String> = args.collect();
    let full = rest.iter().any(|a| a == "full");
    let cities = rest.iter().any(|a| a == "cities");
    let mut expansions = Expansions::BASE;
    if full {
        expansions = expansions.union(Expansions::FIGURES);
    }
    if cities {
        expansions = expansions.union(Expansions::CITIES);
    }
    let threads: u32 = rest
        .iter()
        .find_map(|a| a.strip_prefix("threads="))
        .map_or(1, |n| n.parse().expect("threads"));
    let (mut wins, mut losses, mut draws, mut searched, mut refused) = (0, 0, 0, 0u64, 0u64);
    let mut seconds = 0.0;
    for seed in 0..games {
        let searcher = if seed % 2 == 0 {
            Player::One
        } else {
            Player::Two
        };
        let game = Game::new_game(seed, expansions);
        let mut game = game.unwrap().with_tracking().unwrap();
        let mut rng = Rng::new(seed ^ 0xABCD);
        let outcome = loop {
            let status = game.advance(1 << 20, &mut Vec::new()).unwrap();
            let (actor, count) = match &status {
                Status::Decision(menu) => (menu.player, menu.options.len()),
                Status::Complete(_) => (game.position().turn_owner(), game.legal_actions().len()),
                Status::Winner(player) => break Some(*player),
                Status::Draw(_) => break None,
                Status::WorkLimit => panic!("work limit"),
            };
            let step = if actor == searcher {
                let start = Instant::now();
                let result = game.search(SearchConfig {
                    iterations,
                    rollout_actions,
                    seed: rng.next_u64(),
                    threads,
                    ..SearchConfig::default()
                });
                seconds += start.elapsed().as_secs_f64();
                searched += 1;
                match result {
                    Ok(result) => Some(result.step),
                    Err(_) => {
                        refused += 1;
                        None
                    }
                }
            } else {
                None
            };
            let step = step.unwrap_or_else(|| {
                let index = (rng.next_u64() % count as u64) as usize;
                match status {
                    Status::Decision(_) => Step::Choice(index),
                    _ => Step::Action(game.legal_actions()[index]),
                }
            });
            match step {
                Step::Choice(index) => game.choose(index).unwrap(),
                Step::Action(action) => game.take_action(action, ActionKind::Regular).unwrap(),
            }
        };
        match outcome {
            Some(winner) if winner == searcher => wins += 1,
            Some(_) => losses += 1,
            None => draws += 1,
        }
    }
    println!(
        "search wins {wins} losses {losses} draws {draws}; {searched} searched moves \
         ({refused} refused), {:.2} ms per searched move",
        1000.0 * seconds / searched.max(1) as f64
    );
}
