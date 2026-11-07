use super::*;
use crate::net::{Dimensions, synthetic_bytes};

fn play(game: &mut Game, step: Step) {
    match step {
        Step::Choice(index) => game.choose(index).unwrap(),
        Step::Action(action) => game
            .take_action(action, crate::ActionKind::Regular)
            .unwrap(),
    }
}

/// Settle and report whether someone must move.
fn live(game: &mut Game) -> bool {
    mover(&settle(game)).is_some()
}

fn net() -> Net {
    Net::from_bytes(&synthetic_bytes(Dimensions::TINY, 3, 0.2)).unwrap()
}

#[test]
fn prepared_sampling_preserves_worlds_and_beliefs_across_all_modes() {
    for mode in 0..32 {
        let mut game = Game::new_game(17, crate::Expansions::from_bits(mode).unwrap())
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut rng = Rng::new(9);
        for _ in 0..16 {
            let prompt = settle(&mut game);
            let Some(viewer) = mover(&prompt) else { break };
            let before = game.clone();
            let plan = game.prepare_sampling(viewer).unwrap();
            // Reusing the plan in another order cannot carry sampled state
            // or RNG forward, and keeps all three observers' constraints.
            for seed in [7, 3, 7] {
                let cached = game.determinize_prepared(&plan, seed).unwrap();
                assert_eq!(cached, game.determinize(viewer, seed).unwrap());
                assert_eq!(
                    cached.observe(viewer).unwrap(),
                    game.observe(viewer).unwrap()
                );
                assert!(
                    cached
                        .public_tracker()
                        .unwrap()
                        .check_truth(cached.position())
                );
                for player in Player::ALL {
                    assert!(
                        cached
                            .tracker(player)
                            .unwrap()
                            .check_truth(cached.position())
                    );
                }
            }
            assert_eq!(game, before);
            let choices = steps(&mut game);
            play(&mut game, choices[rng.next_u64() as usize % choices.len()]);
        }
    }
}

/// The root's legal steps, in option order.
fn steps(game: &mut Game) -> Vec<Step> {
    match settle(game) {
        Prompt::Decision(_, options) => (0..options.len()).map(Step::Choice).collect(),
        Prompt::Turn(_, actions) => actions.into_iter().map(Step::Action).collect(),
        Prompt::Over(_) => Vec::new(),
    }
}

fn decide(game: &mut Game) -> Option<Step> {
    if !live(game) {
        return None;
    }
    let result = game
        .search(SearchConfig {
            iterations: 64,
            rollout_actions: 2,
            ..SearchConfig::default()
        })
        .unwrap();
    if result.moves.len() > 1 {
        assert!(result.moves.iter().all(|m| m.visits > 0));
        assert_eq!(
            result.moves.iter().map(|m| m.visits).sum::<u32>(),
            result.iterations
        );
    }
    Some(result.step)
}

#[test]
fn search_picks_a_legal_step_and_leaves_the_game_unchanged() {
    for figures in [false, true] {
        let game = if figures {
            Game::new_figures(3)
        } else {
            Game::new_base(3)
        };
        let mut game = game.unwrap().with_tracking().unwrap();
        for _ in 0..12 {
            if !live(&mut game) {
                break;
            }
            let before = game.clone();
            let step = decide(&mut game).unwrap();
            assert_eq!(game, before, "search leaves the real game unchanged");
            play(&mut game, step);
        }
    }
}

#[test]
fn parallel_trees_merge_their_root_statistics() {
    let mut game = Game::new_figures(4).unwrap().with_tracking().unwrap();
    assert!(live(&mut game));
    let config = SearchConfig {
        iterations: 48,
        rollout_actions: 1,
        threads: 3,
        ..SearchConfig::default()
    };
    let result = game.search(config).unwrap();
    assert!(result.moves.len() > 1);
    assert_eq!(result.iterations, 144);
    assert_eq!(result.moves.iter().map(|m| m.visits).sum::<u32>(), 144);
    assert_eq!(
        game.search(config).unwrap(),
        result,
        "deterministic per seed"
    );
}

#[test]
fn evaluation_is_antisymmetric_and_bounded() {
    let game = Game::new_base(5).unwrap();
    let value = evaluate(&game);
    assert!((-1.0..=1.0).contains(&value));
    assert!(value.abs() < 1e-12, "a symmetric opening deal scores level");
}

#[test]
fn learned_search_reports_root_options_and_leaves_the_game_unchanged() {
    let net = net();
    for expansions in [0u8, 0b11111] {
        let expansions = crate::Expansions::from_bits(expansions).unwrap();
        let mut game = Game::new_game(9, expansions)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut rng = Rng::new(9);
        for turn in 0..16 {
            if !live(&mut game) {
                break;
            }
            let legal = steps(&mut game);
            let before = game.clone();
            let config = SearchConfig {
                seed: rng.next_u64(),
                ..SearchConfig::learned(24)
            };
            let result = game.search_net(&net, config, 8).unwrap();
            assert_eq!(game, before, "search leaves the real game unchanged");
            for m in &result.moves {
                assert_eq!(legal[m.option], m.step);
                assert!((-1.0..=1.0).contains(&m.value));
            }
            if result.moves.len() > 1 {
                // The first simulation scores the root itself.
                assert_eq!(result.iterations, 24);
                assert_eq!(
                    result.moves.iter().map(|m| m.visits).sum::<u32>(),
                    result.iterations - 1
                );
                let prior: f64 = result.moves.iter().map(|m| m.prior).sum();
                assert!((prior - 1.0).abs() < 1e-9, "priors sum to one: {prior}");
                if turn == 0 {
                    assert_eq!(game.search_net(&net, config, 8).unwrap(), result);
                }
            }
            assert!(legal.contains(&result.step));
            play(&mut game, result.step);
        }
    }
}

#[test]
fn learned_trees_batch_leaves_and_follow_the_q_values() {
    let mut game = Game::new_game(21, crate::Expansions::from_bits(0b101).unwrap())
        .unwrap()
        .with_tracking()
        .unwrap();
    // Find a root with several distinct moves.
    let mut rng = Rng::new(5);
    loop {
        assert!(live(&mut game));
        let tree = Tree::new(&game, SearchConfig::learned(8), true).unwrap();
        if tree.root_keys.len() > 2 && !tree.forced {
            break;
        }
        let legal = steps(&mut game);
        play(&mut game, legal[below(&mut rng, legal.len())]);
    }
    let favorite = 1;
    let mut tree = Tree::new(&game, SearchConfig::learned(200), true).unwrap();
    // The root goes alone, then batches of up to eight leaves.
    assert_eq!(tree.select(8).unwrap(), 1);
    let mut widest = 0;
    let mut root = true;
    while !tree.done() {
        let waiting = tree.select(8).unwrap();
        widest = widest.max(waiting);
        let q: Vec<Vec<f32>> = tree
            .pending()
            .map(|observation| {
                // The root's favorite option is clearly best; elsewhere every
                // move looks even.
                (0..observation.options.len())
                    .map(|i| if root && i == favorite { 0.8 } else { 0.0 })
                    .collect()
            })
            .collect();
        assert!(
            tree.resolve(&q[..q.len().saturating_sub(1)]).is_err() || q.is_empty(),
            "a short batch is refused"
        );
        tree.resolve(&q).unwrap();
        root = false;
    }
    assert!(widest > 1, "virtual loss lets leaves batch");
    let result = tree.result().unwrap();
    assert_eq!(result.iterations, 200);
    let best = result.moves.iter().max_by_key(|m| m.visits).unwrap();
    assert_eq!(best.option, favorite);
    assert_eq!(result.step, best.step);
    for edge in tree.nodes.iter().flat_map(|node| &node.edges) {
        let mean = edge.value / f64::from(edge.visits.max(1));
        assert!((-1.0..=1.0).contains(&mean), "no virtual loss remains");
    }
}

#[test]
fn a_waiting_node_is_queued_once_and_ends_the_batch() {
    let mut game = Game::new_game(21, crate::Expansions::from_bits(0b101).unwrap())
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut rng = Rng::new(5);
    loop {
        assert!(live(&mut game));
        let tree = Tree::new(&game, SearchConfig::learned(8), true).unwrap();
        if tree.root_keys.len() > 2 && !tree.forced {
            break;
        }
        let legal = steps(&mut game);
        play(&mut game, legal[below(&mut rng, legal.len())]);
    }
    let mut tree = Tree::new(&game, SearchConfig::learned(300), true).unwrap();
    let root_options = tree.root_keys.len();
    // The root waits alone: every other simulation would reach it again.
    assert_eq!(tree.select(64).unwrap(), 1);
    assert_eq!(tree.started, 1, "a discarded simulation is not counted");
    let mut short_batches = 0;
    while !tree.done() {
        let waiting = tree.select(64).unwrap();
        let mut nodes: Vec<usize> = tree.leaves.iter().map(|leaf| leaf.node).collect();
        nodes.sort_unstable();
        nodes.dedup();
        assert_eq!(nodes.len(), waiting, "each waiting node has one leaf");
        if waiting < 64 && !tree.done() {
            short_batches += 1;
        }
        let q: Vec<Vec<f32>> = tree
            .pending()
            .map(|observation| vec![0.0; observation.options.len()])
            .collect();
        tree.resolve(&q).unwrap();
    }
    // The second batch can hold at most one leaf per root move, so the tree
    // (with far fewer than 64 moves) stopped short to let the next
    // simulations descend with the network's answers.
    assert!(root_options < 64 && short_batches > 0);
    let result = tree.result().unwrap();
    assert_eq!(result.iterations, 300);
    assert_eq!(tree.started, tree.finished);
    for node in &tree.nodes {
        assert!(!node.pending);
        for edge in &node.edges {
            assert!(
                edge.available >= edge.visits,
                "discards take back availability"
            );
        }
    }
}

#[test]
fn search_net_searches_the_same_with_and_without_a_policy_output() {
    let hl = Dimensions {
        headnorm: true,
        bins: 5,
        ..Dimensions::TINY
    };
    let hl_net = Net::from_bytes(&synthetic_bytes(hl, 4, 0.5)).unwrap();
    let policy_net =
        Net::from_bytes(&synthetic_bytes(Dimensions { policy: true, ..hl }, 4, 0.5)).unwrap();
    assert!(policy_net.has_policy() && !hl_net.has_policy());
    let mut game = Game::new_game(21, crate::Expansions::from_bits(0b101).unwrap())
        .unwrap()
        .with_tracking()
        .unwrap();
    assert!(live(&mut game));
    let player = mover(&settle(&mut game)).unwrap();
    let observation = game.observe(player).unwrap();
    assert_eq!(
        hl_net.q(&observation).unwrap(),
        policy_net.q(&observation).unwrap()
    );
    let config = SearchConfig {
        seed: 3,
        ..SearchConfig::learned(48)
    };
    let with = game.search_net(&policy_net, config, 4).unwrap();
    let without = game.search_net(&hl_net, config, 4).unwrap();
    assert!(with.moves.len() >= 2);
    for (a, b) in with.moves.iter().zip(&without.moves) {
        assert_eq!(
            (a.step, a.visits, a.prior, a.q),
            (b.step, b.visits, b.prior, b.q)
        );
    }
}

#[test]
fn policy_logits_set_the_priors_in_place_of_q() {
    let mut game = Game::new_game(21, crate::Expansions::from_bits(0b101).unwrap())
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut rng = Rng::new(5);
    loop {
        assert!(live(&mut game));
        let tree = Tree::new(&game, SearchConfig::learned(8), true).unwrap();
        if tree.root_keys.len() > 2 && !tree.forced {
            break;
        }
        let legal = steps(&mut game);
        play(&mut game, legal[below(&mut rng, legal.len())]);
    }
    let favorite = 1;
    // Every Q is even; only the root's policy prefers the favorite.
    let run = |policy: bool| {
        let mut tree = Tree::new(&game, SearchConfig::learned(64), true).unwrap();
        let mut root = true;
        while !tree.done() {
            tree.select(8).unwrap();
            let q: Vec<Vec<f32>> = tree
                .pending()
                .map(|observation| vec![0.0; observation.options.len()])
                .collect();
            let logits: Vec<Vec<f32>> = tree
                .pending()
                .map(|observation| {
                    (0..observation.options.len())
                        .map(|i| if root && i == favorite { 3.0 } else { 0.0 })
                        .collect()
                })
                .collect();
            if policy {
                assert!(
                    tree.resolve_with_policy(&q, &logits[..logits.len().saturating_sub(1)])
                        .is_err()
                        || q.is_empty()
                );
                tree.resolve_with_policy(&q, &logits).unwrap();
            } else {
                tree.resolve(&q).unwrap();
            }
            root = false;
        }
        tree.result().unwrap()
    };
    let with = run(true);
    let top = with
        .moves
        .iter()
        .max_by(|a, b| a.prior.total_cmp(&b.prior))
        .unwrap();
    assert_eq!(top.option, favorite);
    assert_eq!(
        with.moves.iter().max_by_key(|m| m.visits).unwrap().option,
        favorite
    );
    let without = run(false);
    let priors: Vec<f64> = without.moves.iter().map(|m| m.prior).collect();
    assert!(
        priors.iter().all(|p| (p - priors[0]).abs() < 1e-9),
        "{priors:?}"
    );
}

#[test]
fn root_noise_keeps_priors_a_distribution() {
    let net = net();
    let mut game = Game::new_base(2).unwrap().with_tracking().unwrap();
    assert!(live(&mut game));
    let plain = game.search_net(&net, SearchConfig::learned(16), 1).unwrap();
    let noisy = game
        .search_net(
            &net,
            SearchConfig {
                root_noise: Some(RootNoise {
                    alpha: 0.3,
                    fraction: 0.25,
                }),
                ..SearchConfig::learned(16)
            },
            1,
        )
        .unwrap();
    let total: f64 = noisy.moves.iter().map(|m| m.prior).sum();
    assert!((total - 1.0).abs() < 1e-9);
    // Reported priors are the network's, before noise.
    assert_eq!(
        plain.moves.iter().map(|m| m.prior).collect::<Vec<_>>(),
        noisy.moves.iter().map(|m| m.prior).collect::<Vec<_>>()
    );
}

#[test]
fn gamma_samples_have_the_right_mean() {
    let mut rng = Rng::new(1);
    for alpha in [0.3, 1.0, 2.5] {
        let n = 20_000;
        let mean = (0..n).map(|_| gamma(&mut rng, alpha)).sum::<f64>() / f64::from(n);
        assert!(
            (mean - alpha).abs() < 0.05 * alpha.max(1.0),
            "{alpha}: {mean}"
        );
    }
}

#[test]
fn a_root_only_search_follows_the_network() {
    let net = net();
    let mut game = Game::new_game(13, crate::Expansions::from_bits(0b100).unwrap())
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut rng = Rng::new(13);
    for _ in 0..30 {
        if !live(&mut game) {
            break;
        }
        let result = game.search_net(&net, SearchConfig::learned(1), 1).unwrap();
        assert!(result.moves.iter().all(|m| m.visits == 0));
        let player = mover(&settle(&mut game)).unwrap();
        let observation = game.observe(player).unwrap();
        let q = net.q(&observation).unwrap();
        let best = result
            .moves
            .iter()
            .max_by(|a, b| a.prior.total_cmp(&b.prior))
            .unwrap();
        if result.moves.len() > 1 {
            assert_eq!(result.step, best.step);
            // Without alike options, the highest prior is the highest Q.
            if result.moves.len() == q.len() {
                let top = q.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                assert_eq!(q[best.option], top);
            }
        }
        let legal = steps(&mut game);
        play(&mut game, legal[below(&mut rng, legal.len())]);
    }
}
