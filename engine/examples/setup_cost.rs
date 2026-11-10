//! Small setup/layout probe with initialized catalogs. Compare release builds
//! at identified revisions; full-game timing lives in `random_games`.
use inno_engine::*;
use std::hint::black_box;
use std::time::Instant;

fn main() {
    println!(
        "Position {} Game {} Card {}",
        std::mem::size_of::<Position>(),
        std::mem::size_of::<Game>(),
        std::mem::size_of::<Card>()
    );
    for expansions in [Expansions::BASE, Expansions::FIGURES] {
        let new = |seed| Game::new_game(seed, expansions);
        black_box(new(0).unwrap());
        let start = Instant::now();
        for seed in 0..50_000 {
            black_box(new(black_box(seed)).unwrap());
        }
        println!(
            "{} setup {:.0} ns/game",
            expansions.name(),
            start.elapsed().as_nanos() as f64 / 50_000.0
        );
    }
}
