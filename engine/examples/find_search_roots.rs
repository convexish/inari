//! Find seeded random games for `tests/search_verification.rs`: the first
//! seed from `first_seed` whose uniformly random replay (the test's own
//! choice stream) reaches a decision sourced by `code`, where determinizing
//! and searching that decision succeed. `cargo run --release --example
//! find_search_roots -- <sets> <code> <first_seed> <seeds> [jenner]`, with
//! `sets` letters among F, C, E, A, U. `jenner` keeps only decisions
//! reached right after a karma junked an available achievement while
//! Edward Jenner tops a board (the failed-attempt shape).
use inno_engine::*;

const MAX_STEPS: usize = 1200;

fn expansions(letters: &str) -> Expansions {
    letters.chars().fold(Expansions::BASE, |acc, letter| {
        acc.union(match letter {
            'F' => Expansions::FIGURES,
            'C' => Expansions::CITIES,
            'E' => Expansions::ECHOES,
            'A' => Expansions::ARTIFACTS,
            'U' => Expansions::UNSEEN,
            other => panic!("unknown set letter {other}"),
        })
    })
}

fn jenner_tops(game: &Game) -> bool {
    Player::ALL.into_iter().any(|player| {
        Color::ALL.into_iter().any(|color| {
            game.position()
                .cards(Zone::Board(player, color))
                .last()
                .is_some_and(|&card| {
                    game.position().card(card).figure == Some(Figure::EdwardJenner)
                })
        })
    })
}

fn verified(game: &Game, menu: &Decision) -> bool {
    for sample in 0..4 {
        let Ok(world) = game.determinize(menu.player, sample) else {
            return false;
        };
        if world.observe(menu.player).unwrap() != game.observe(menu.player).unwrap()
            || !world.position().check_conservation()
            || !world
                .tracker(menu.player)
                .unwrap()
                .check_truth(world.position())
        {
            return false;
        }
    }
    let Ok(result) = game.search(SearchConfig {
        iterations: 4,
        rollout_actions: 0,
        seed: 71,
        ..SearchConfig::default()
    }) else {
        return false;
    };
    result.iterations == 4
        && matches!(result.step, Step::Choice(index) if index < menu.options.len())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sets = expansions(&args[0]);
    let code = args[1].as_str();
    let first_seed: u64 = args[2].parse().unwrap();
    let seeds: u64 = args[3].parse().unwrap();
    let jenner = args.get(4).is_some_and(|flag| flag == "jenner");
    for seed in first_seed..first_seed + seeds {
        let mut game = Game::new_game(seed, sets).unwrap().with_tracking().unwrap();
        let mut rng = Rng::new(seed ^ 0x0B5E);
        let mut hits = Vec::new();
        for step in 0..MAX_STEPS {
            let mut events = Vec::new();
            let status = game.advance(1 << 16, &mut events).unwrap();
            if let Status::Decision(menu) = &status
                && menu
                    .source_card(menu.player)
                    .is_some_and(|card| game.position().card(card).code == code)
            {
                let shape = !jenner
                    || (jenner_tops(&game)
                        && events.iter().any(|event| {
                            matches!(
                                event,
                                Event::Moved(Movement {
                                    verb: Some(Verb::Junk),
                                    source: Zone::AvailableAchievements,
                                    inside_karma: true,
                                    ..
                                })
                            )
                        }));
                if shape {
                    let before = game.clone();
                    let ok = verified(&game, menu);
                    assert_eq!(game, before);
                    hits.push((step, ok));
                }
            }
            let options = match &status {
                Status::Decision(menu) => menu.options.len(),
                Status::Complete(_) => game.legal_actions().len(),
                _ => break,
            };
            let index = (rng.next_u64() % options as u64) as usize;
            match status {
                Status::Decision(_) => game.choose(index).unwrap(),
                _ => game
                    .take_action(game.legal_actions()[index], ActionKind::Regular)
                    .unwrap(),
            }
        }
        if !hits.is_empty() {
            println!("{} {code} seed={seed} {hits:?}", args[0]);
            if hits.iter().any(|&(_, ok)| ok) {
                return;
            }
        }
    }
    println!(
        "{} {code}: no seed in {first_seed}..{}",
        args[0],
        first_seed + seeds
    );
}
