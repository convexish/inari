//! Search refusals diagnosed in the seeded observation audit, at the
//! positions the current seeded games still reach. When a rules or menu
//! change moves the games, `examples/find_search_roots.rs` finds a seed
//! and step for each case again.
//! Replay the actual shared effects and nested reactions, with no fixture
//! mutations, and require search at every formerly refused decision.
use inno_engine::*;

type ExpectedRoots = &'static [(usize, &'static str)];

#[test]
fn public_verification_makes_the_diagnosed_roots_searchable() {
    let echoes_cities = Expansions::ECHOES.union(Expansions::CITIES);
    let cases: &[(Expansions, u64, ExpectedRoots)] = &[
        (Expansions::FIGURES, 251, &[(147, "B060")]),
        (
            Expansions::ECHOES,
            1,
            &[(101, "E038"), (102, "E038"), (103, "E038"), (104, "E038")],
        ),
        (echoes_cities, 2, &[(114, "E041"), (115, "E041")]),
        (echoes_cities, 1, &[(138, "E038")]),
        (echoes_cities, 17, &[(154, "E006")]),
        (
            echoes_cities.union(Expansions::FIGURES),
            12,
            &[(98, "E038"), (99, "E038")],
        ),
        // Live unbounded batches over a hidden hand, interrupted by a
        // reaction that asks the other player: Yi Sun-sin replacing
        // Classification's transfers, Taqiyya's scores, and Classification's
        // own decision during a transfer.
        (Expansions::FIGURES, 2684, &[(79, "F037")]),
        (
            Expansions::FIGURES.union(Expansions::ECHOES),
            9,
            &[(106, "F037")],
        ),
        (
            Expansions::FIGURES.union(Expansions::UNSEEN),
            2215,
            &[(55, "U035")],
        ),
        (
            Expansions::FIGURES.union(Expansions::ARTIFACTS),
            604,
            &[(196, "B060"), (197, "B060")],
        ),
        // Silk's per-color scores reveal each card, so a reaction during
        // them (Edward Jenner, Emmy Noether) can still be searched.
        (
            Expansions::FIGURES.union(Expansions::UNSEEN),
            1414,
            &[(31, "U009"), (32, "U009")],
        ),
        (
            Expansions::FIGURES.union(Expansions::UNSEEN),
            0,
            &[(259, "F081")],
        ),
        // Dragon's Lair freezes every deck top, more than the pick pool
        // holds; a deck position resolves by place in a sampled world.
        (
            Expansions::FIGURES
                .union(Expansions::CITIES)
                .union(Expansions::ECHOES)
                .union(Expansions::ARTIFACTS)
                .union(Expansions::UNSEEN),
            171,
            &[(372, "A105")],
        ),
        // Boerhaave Microscope sums a returned hidden hand card and a named
        // top card; their distinctness is public, so Emmy Noether's
        // reaction to its score can still be searched.
        (
            Expansions::FIGURES.union(Expansions::ARTIFACTS),
            23,
            &[(149, "F081")],
        ),
        (
            Expansions::FIGURES
                .union(Expansions::CITIES)
                .union(Expansions::ECHOES)
                .union(Expansions::ARTIFACTS)
                .union(Expansions::UNSEEN),
            178,
            &[(314, "F081")],
        ),
        // Yi Sun-sin replaces an exchange of a score pile with the junk:
        // the exchanged cards are revealed, then tucked, so ordering the
        // tuck needs no pick per hidden card.
        (
            Expansions::FIGURES
                .union(Expansions::CITIES)
                .union(Expansions::UNSEEN),
            13,
            &[(178, "F037")],
        ),
    ];
    let mut checked = 0;
    for &(expansions, seed, roots) in cases {
        let mut game = Game::new_game(seed, expansions)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut rng = Rng::new(seed ^ 0x0B5E);
        for step in 0..=roots.last().unwrap().0 {
            let status = game.advance(1 << 16, &mut Vec::new()).unwrap();
            if let Some(&(_, code)) = roots.iter().find(|&&(root, _)| root == step) {
                let Status::Decision(menu) = &status else {
                    panic!("expected {code} decision")
                };
                assert_eq!(
                    game.position()
                        .card(menu.source_card(menu.player).unwrap())
                        .code,
                    code,
                    "{expansions:?} seed={seed} step={step}"
                );
                let before = game.clone();
                for sample in 0..4 {
                    let world = game
                        .determinize(menu.player, sample)
                        .unwrap_or_else(|error| {
                            panic!("{expansions:?} seed={seed} step={step} {code}: {error:?}")
                        });
                    assert_eq!(
                        world.observe(menu.player).unwrap(),
                        game.observe(menu.player).unwrap()
                    );
                    assert!(world.position().check_conservation());
                    assert!(
                        world
                            .tracker(menu.player)
                            .unwrap()
                            .check_truth(world.position())
                    );
                }
                let result = game
                    .search(SearchConfig {
                        iterations: 4,
                        rollout_actions: 0,
                        seed: 71,
                        ..SearchConfig::default()
                    })
                    .unwrap();
                assert_eq!(result.iterations, 4);
                assert!(matches!(result.step, Step::Choice(index) if index < menu.options.len()));
                assert_eq!(game, before);
                checked += 1;
            }
            let options = match &status {
                Status::Decision(menu) => menu.options.len(),
                Status::Complete(_) => game.legal_actions().len(),
                _ => panic!("{expansions:?} seed={seed}: replay ended early at step {step}"),
            };
            let index = (rng.next_u64() % options as u64) as usize;
            match status {
                Status::Decision(_) => game.choose(index).unwrap(),
                _ => game
                    .take_action(game.legal_actions()[index], ActionKind::Regular)
                    .unwrap(),
            }
        }
    }
    assert_eq!(checked, 23);
}

/// Jenner junks the achievement Buried Treasure was trying to score. The failed
/// score proves both picks named the same card, so sampled worlds must remove it
/// from the available achievements too.
#[test]
fn a_failed_attempt_keeps_sampled_worlds_consistent() {
    let expansions = Expansions::FIGURES
        .union(Expansions::ECHOES)
        .union(Expansions::UNSEEN);
    let (seed, root) = (28675, 355);
    let mut game = Game::new_game(seed, expansions)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut rng = Rng::new(seed ^ 0x0B5E);
    for step in 0..=root {
        let status = game.advance(1 << 16, &mut Vec::new()).unwrap();
        if step == root {
            let Status::Decision(menu) = &status else {
                panic!("expected Buried Treasure's decision")
            };
            let source = menu.source_card(menu.player).unwrap();
            assert_eq!(game.position().card(source).code, "U050");
            let real = game.observe(menu.player).unwrap();
            for sample in 0..20 {
                let world = game.determinize(menu.player, sample).unwrap();
                assert_eq!(world.observe(menu.player).unwrap(), real, "sample {sample}");
            }
            return;
        }
        let options = match &status {
            Status::Decision(menu) => menu.options.len(),
            Status::Complete(_) => game.legal_actions().len(),
            _ => panic!("replay ended early"),
        };
        let index = (rng.next_u64() % options as u64) as usize;
        match status {
            Status::Decision(_) => game.choose(index).unwrap(),
            _ => game
                .take_action(game.legal_actions()[index], ActionKind::Regular)
                .unwrap(),
        }
    }
}
