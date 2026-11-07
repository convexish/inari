//! Random-play sweeps over batch menus: which unannotated batches still ask
//! for an order (the candidates for a shortcut), and whether every reduced
//! batch reaches exactly the unrestricted oracle's outcomes from the
//! positions seeded games actually reach.
use super::*;
use std::collections::BTreeMap;

const MAX_STEPS: usize = 1500;
/// Enumeration caps for the oracle comparison from one decision.
const MAX_LEAVES: usize = 4000;
const MAX_DEPTH: usize = 12;

/// `SWEEP_ONLY=mode,seed` narrows the oracle to one game, for a diagnosis.
fn only_game() -> Option<(u8, u64)> {
    let only = std::env::var("SWEEP_ONLY").ok()?;
    let (mode, seed) = only.split_once(',')?;
    Some((mode.parse().ok()?, seed.parse().ok()?))
}

fn games_per_mode() -> u64 {
    std::env::var("SWEEP_GAMES")
        .ok()
        .and_then(|games| games.parse().ok())
        .unwrap_or(6)
}

/// The innermost program step running at this decision.
fn program_of(game: &Game) -> String {
    let program = game.frames.iter().rev().find_map(|frame| match frame {
        Frame::Program { source, pc, .. } => Some(match source {
            Source::Effect { card, effect } => {
                format!("{} e{effect} pc{pc}", game.position.card(*card).code)
            }
            other => format!("{other:?} pc{pc}"),
        }),
        _ => None,
    });
    program.unwrap_or_else(|| "procedure".into())
}

/// The batch asking at this decision, with the program step that ran it.
fn asking_batch(game: &Game) -> Option<(&MoveAll, String)> {
    let [.., Frame::MoveAll(batch), Frame::Choose { .. }] = game.frames.as_slice() else {
        return None;
    };
    Some((batch, program_of(game)))
}

/// A hand-built list of cards asking for its next move with at least two
/// still to go. Each remaining group commutes, so every menu is compared.
fn asking_selected(game: &Game) -> Option<String> {
    let [
        ..,
        Frame::MoveSelected { verb, targets, .. },
        Frame::Choose { .. },
    ] = game.frames.as_slice()
    else {
        return None;
    };
    (targets.len() > 1).then(|| {
        let karma = game.frames.iter().rev().find_map(|frame| match frame {
            Frame::Karma { hook, .. } => Some(game.position.card(hook.source).code.to_string()),
            _ => None,
        });
        format!(
            "{} selected {verb:?} x{}",
            karma.unwrap_or_else(|| program_of(game)),
            targets.len()
        )
    })
}

fn describe(game: &Game, batch: &MoveAll, program: &str) -> String {
    format!(
        "{program}: {:?} {:?} -> {:?}{}{}{}",
        batch.verb,
        batch.from,
        batch.to,
        batch
            .limit
            .map_or(String::new(), |limit| format!(" limit {limit}")),
        if batch.stop.any_time() {
            " optional"
        } else {
            ""
        },
        if batch.frozen.is_some() {
            " frozen"
        } else {
            ""
        },
    )
    .replace(&format!("{:?}", game.position.expansions()), "")
}

/// Every reachable end of the current action from this decision, as
/// normalized positions with the RNG state, or None past the caps.
/// Advances to the next status, with the choices to enumerate at a menu
/// when a shortcut policy restricts them: a deck junked top-down takes the
/// physical top (the last junk option) on the unrestricted side as well,
/// keeping any Decline; Oars' lowest-first ends the comparison (None).
fn advance_policed(game: &mut Game) -> Option<(Status, Option<Vec<usize>>)> {
    let status = game.advance(1 << 16, &mut Vec::new()).ok()?;
    let mut allowed = None;
    if let Status::Decision(menu) = &status
        && let Some((batch, _)) = asking_batch(game)
    {
        match batch.shortcut {
            Some(Shortcut::LowestFirst) => return None,
            Some(Shortcut::DeckTop) if game.shortcuts_disabled => {
                let top = menu
                    .options
                    .iter()
                    .rposition(|option| matches!(option, Alternative::Move { .. }));
                allowed = Some(
                    (0..menu.options.len())
                        .filter(|&index| {
                            Some(index) == top || menu.options[index] == Alternative::Decline
                        })
                        .collect(),
                );
            }
            _ => {}
        }
    }
    Some((status, allowed))
}

fn leaves(mut game: Game, depth: usize, budget: &mut usize) -> Option<Vec<(Position, Rng)>> {
    if depth > MAX_DEPTH || *budget == 0 {
        return None;
    }
    let (status, allowed) = advance_policed(&mut game)?;
    match status {
        Status::Decision(menu) => {
            // Blind picks commute only in distribution: another group order
            // hands the sampler's words to other groups, so exact outcomes
            // differ while every identity stays equally likely. Any such
            // menu downstream makes the exact comparison meaningless.
            if menu.options.iter().any(|option| {
                matches!(
                    option,
                    Alternative::BlindMove { .. } | Alternative::BlindSelect { .. }
                )
            }) {
                return None;
            }
            let mut out = Vec::new();
            for choice in allowed.unwrap_or_else(|| (0..menu.options.len()).collect()) {
                let mut fork = game.clone();
                fork.choose(choice).ok()?;
                out.extend(leaves(fork, depth + 1, budget)?);
            }
            Some(out)
        }
        Status::Complete(_) | Status::Draw(_) | Status::Winner(_) => {
            *budget -= 1;
            let mut position = game.position.clone();
            position.sort_unordered_piles();
            Some(vec![(position, game.rng)])
        }
        Status::WorkLimit => None,
    }
}

/// The choice paths to every leaf, for diagnosing a mismatch.
fn paths(mut game: Game, depth: usize, trail: Vec<String>) -> Vec<((Position, Rng), Vec<String>)> {
    if depth > MAX_DEPTH {
        return Vec::new();
    }
    let Some((status, allowed)) = advance_policed(&mut game) else {
        return Vec::new();
    };
    match status {
        Status::Decision(menu) => allowed
            .unwrap_or_else(|| (0..menu.options.len()).collect())
            .into_iter()
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                let mut trail = trail.clone();
                trail.push(format!(
                    "P{:?} {} of {:?}",
                    menu.player,
                    choice,
                    menu.options
                        .iter()
                        .map(|option| format!("{option:?}").chars().take(60).collect::<String>())
                        .collect::<Vec<_>>()
                ));
                paths(fork, depth + 1, trail)
            })
            .collect(),
        _ => {
            let mut position = game.position.clone();
            position.sort_unordered_piles();
            vec![((position, game.rng), trail)]
        }
    }
}

/// Plays one seeded game, calling back at every decision with the game as
/// it stood right after the previous step (before that decision's menu was
/// built), so an oracle can rebuild the menu its own way.
fn play_random<F: FnMut(&Game, &Decision, &Game)>(
    expansions: Expansions,
    seed: u64,
    mut at_decision: F,
) {
    let Ok(mut game) = Game::new_game(seed, expansions) else {
        return;
    };
    let mut rng = Rng::new(seed ^ 0x5EE9);
    let mut before = game.clone();
    for _ in 0..MAX_STEPS {
        let status = game.advance(1 << 16, &mut Vec::new()).unwrap();
        let options = match &status {
            Status::Decision(menu) => {
                at_decision(&game, menu, &before);
                menu.options.len()
            }
            Status::Complete(_) => game.legal_actions().len(),
            _ => return,
        };
        let index = (rng.next_u64() % options as u64) as usize;
        match status {
            Status::Decision(_) => game.choose(index).unwrap(),
            _ => game
                .take_action(game.legal_actions()[index], ActionKind::Regular)
                .unwrap(),
        }
        before = game.clone();
    }
}

/// A pending menu an alias or no-op splay merge shaped, named by the step
/// that asked it.
fn asking_alias(game: &Game) -> Option<String> {
    let [.., Frame::Choose { options, .. }] = game.frames.as_slice() else {
        return None;
    };
    if !options.aliased {
        return None;
    }
    let choice = match game.frames.as_slice() {
        [.., Frame::Program { source, pc, .. }, Frame::Choose { .. }] => pc
            .checked_sub(1)
            .and_then(|pc| game.instruction(*source, pc))
            .map(|instruction| match instruction {
                crate::program::Instruction::Choose { choice, .. } => format!("{choice:?}"),
                other => format!("{other:?}").chars().take(80).collect(),
            }),
        _ => None,
    };
    Some(format!(
        "{}: alias {}",
        program_of(game),
        choice.unwrap_or_else(|| "splay".into())
    ))
}

/// Diagnostic: `SWEEP_GAMES=40 cargo test --release batch_menus_without -- --ignored --nocapture`
/// lists the unannotated batches that asked for an order, most frequent first.
#[test]
#[ignore = "diagnostic sweep"]
fn batch_menus_without_shortcuts_in_random_play() {
    let mut asked: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for mode in 0..Expansions::COMBINATIONS as u8 {
        let expansions = crate::mode_expansions(mode).unwrap();
        for seed in 0..games_per_mode() {
            play_random(expansions, seed, |game, menu, _| {
                if let Some((batch, program)) = asking_batch(game)
                    && batch.shortcut.is_none()
                {
                    let entry = asked.entry(describe(game, batch, &program)).or_default();
                    entry.0 += 1;
                    entry.1 = entry.1.max(menu.options.len());
                }
                // Value and colour choices without an alias, for review.
                if let [.., Frame::Program { source, pc, .. }, Frame::Choose { .. }] =
                    game.frames.as_slice()
                    && let Some(crate::program::Instruction::Choose {
                        choice,
                        alias: None,
                        ..
                    }) = pc
                        .checked_sub(1)
                        .and_then(|pc| game.instruction(*source, pc))
                    && matches!(
                        choice,
                        Choice::Value(_) | Choice::Color(_) | Choice::Colors(_)
                    )
                    && menu.options.len() > 2
                {
                    let entry = asked
                        .entry(format!("{}: choose {choice:?}", program_of(game)))
                        .or_default();
                    entry.0 += 1;
                    entry.1 = entry.1.max(menu.options.len());
                }
            });
        }
    }
    let mut rows: Vec<_> = asked.into_iter().collect();
    rows.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
    for (key, (count, widest)) in rows {
        println!("{count:5} (up to {widest} options)  {key}");
    }
}

/// Every reduced batch menu reached in seeded random play leads to exactly
/// the outcomes unrestricted play reaches from the same position.
#[test]
fn reduced_batches_match_the_unrestricted_oracle_in_random_play() {
    let mut compared = 0;
    let mut skipped = 0;
    let mut witnesses: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let only = only_game();
    for mode in 0..Expansions::COMBINATIONS as u8 {
        let expansions = crate::mode_expansions(mode).unwrap();
        for seed in 0..games_per_mode() {
            if only.is_some_and(|(m, s)| (m, s) != (mode, seed)) {
                continue;
            }
            play_random(expansions, seed, |game, _, before| {
                let (start, key) = if let Some((batch, program)) = asking_batch(game) {
                    // Compare from a batch's first menu: a later menu's floor
                    // already encodes the order chosen so far, which the
                    // unrestricted continuation would not have to honour.
                    if batch.shortcut.is_none() || !batch.visited.is_empty() {
                        return;
                    }
                    // Policies, not equivalences: Oars' lowest-first and a
                    // deck junked top-down before a blind highest-junk claim.
                    if matches!(
                        batch.shortcut,
                        Some(Shortcut::DeckTop | Shortcut::LowestFirst)
                    ) {
                        return;
                    }
                    (game, describe(game, batch, &program))
                } else if let Some(key) = asking_selected(game) {
                    (game, key)
                } else if let Some(key) = asking_alias(game) {
                    // The alias shaped this very menu: start from the step
                    // before it, so the oracle builds the menu unrestricted.
                    (before, key)
                } else {
                    return;
                };
                let mut manual = start.clone();
                manual.shortcuts_disabled = true;
                let (mut budget_a, mut budget_b) = (MAX_LEAVES, MAX_LEAVES);
                let (Some(reduced), Some(manual)) = (
                    leaves(start.clone(), 0, &mut budget_a),
                    leaves(manual, 0, &mut budget_b),
                ) else {
                    skipped += 1;
                    return;
                };
                for (left, right, what) in [
                    (&manual, &reduced, "missing"),
                    (&reduced, &manual, "invented"),
                ] {
                    for outcome in left {
                        if !right.contains(outcome) {
                            let summary = |(position, rng): &(Position, Rng)| {
                                let cards = |zone| {
                                    position
                                        .cards(zone)
                                        .iter()
                                        .map(|&card| position.card(card).code.to_string())
                                        .collect::<Vec<_>>()
                                };
                                format!(
                                    "rng {rng:?} hands {:?}/{:?} score {:?}/{:?} decks {:?}",
                                    cards(Zone::Hand(Player::One)),
                                    cards(Zone::Hand(Player::Two)),
                                    cards(Zone::Score(Player::One)),
                                    cards(Zone::Score(Player::Two)),
                                    (1..=3)
                                        .map(|age| cards(Zone::Deck(Deck::base(age))))
                                        .collect::<Vec<_>>(),
                                )
                            };
                            let mut source = start.clone();
                            source.shortcuts_disabled = what == "missing";
                            let trail = paths(source, 0, Vec::new())
                                .into_iter()
                                .find(|(leaf, _)| leaf == outcome)
                                .map(|(_, trail)| trail.join("\n    "))
                                .unwrap_or_default();
                            let mut other = start.clone();
                            other.shortcuts_disabled = what != "missing";
                            let other_trails: Vec<_> = paths(other, 0, Vec::new())
                                .into_iter()
                                .map(|(_, trail)| trail.join(" | "))
                                .collect();
                            panic!(
                                "{key}: {what} outcome in {expansions:?} seed {seed}\n  {}\n  via\n    {trail}\nother side's paths:\n  {}",
                                summary(outcome),
                                other_trails.join("\n  ")
                            );
                        }
                    }
                }
                compared += 1;
                let entry = witnesses.entry(key).or_default();
                entry.0 += 1;
                if reduced.len() < manual.len() {
                    entry.1 += 1;
                }
            });
        }
    }
    if std::env::var_os("SWEEP_REPORT").is_some() {
        println!("compared {compared}, skipped {skipped} past the caps");
        for (key, (seen, reduced)) in witnesses {
            println!("{seen:5} seen, {reduced:5} reduced  {key}");
        }
    }
    assert!(compared > 0);
}

/// Diagnostic: the first Currency menu in Base seed 1 and what arrives after
/// its first return.
#[test]
#[ignore = "diagnostic"]
fn currency_seed_one_trace() {
    let mut hit = false;
    play_random(Expansions::BASE, 1, |game, menu, _| {
        if hit {
            return;
        }
        let Some((batch, program)) = asking_batch(game) else {
            return;
        };
        if !program.starts_with("B020") {
            return;
        }
        hit = true;
        println!("menu {:?}", menu.options);
        println!("hand {:?}", game.position.cards(Zone::Hand(menu.player)));
        println!("visited {:?} seen {:?}", batch.visited, batch.seen);
        for frame in game.frames.iter().rev().take(6) {
            println!(
                "frame {}",
                format!("{frame:?}").chars().take(300).collect::<String>()
            );
        }
        let mut fork = game.clone();
        fork.choose(0).unwrap();
        let mut events = Vec::new();
        let status = fork.advance(1 << 16, &mut events).unwrap();
        for event in &events {
            println!(
                "event {}",
                format!("{event:?}").chars().take(200).collect::<String>()
            );
        }
        println!("then {:?}", status);
        println!("hand {:?}", fork.position.cards(Zone::Hand(menu.player)));
    });
}
