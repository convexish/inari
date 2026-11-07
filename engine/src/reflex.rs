//! Synchronous parallel game stepping for training on the replacement
//! engine.
//!
//! Workers own matches; the caller owns policy sampling and learning. Every
//! response is ordered by game index, independent of thread completion
//! order. Games are assigned to workers round-robin by index.

use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::Expansions;
use crate::session::{Error, Match, Prompt, Step};

/// One name per [`Expansions::index`]: the enabled sets joined with `+` in
/// bit order (figures, cities, echoes, artifacts, unseen), or `base`.
pub const TRAINING_MODES: [&str; Expansions::COMBINATIONS] = [
    "base",
    "figures",
    "cities",
    "figures+cities",
    "echoes",
    "figures+echoes",
    "cities+echoes",
    "figures+cities+echoes",
    "artifacts",
    "figures+artifacts",
    "cities+artifacts",
    "figures+cities+artifacts",
    "echoes+artifacts",
    "figures+echoes+artifacts",
    "cities+echoes+artifacts",
    "figures+cities+echoes+artifacts",
    "unseen",
    "figures+unseen",
    "cities+unseen",
    "figures+cities+unseen",
    "echoes+unseen",
    "figures+echoes+unseen",
    "cities+echoes+unseen",
    "figures+cities+echoes+unseen",
    "artifacts+unseen",
    "figures+artifacts+unseen",
    "cities+artifacts+unseen",
    "figures+cities+artifacts+unseen",
    "echoes+artifacts+unseen",
    "figures+echoes+artifacts+unseen",
    "cities+echoes+artifacts+unseen",
    "figures+cities+echoes+artifacts+unseen",
];

/// The expansions of training mode `index`, `None` past the 32 combinations.
pub fn mode_expansions(index: u8) -> Option<Expansions> {
    Expansions::from_bits(index)
}

/// A live game's prompt for the player to move.
pub struct Row {
    pub game: usize,
    pub player: usize,
    pub observation: Vec<f32>,
    /// The number of legal steps, equal to the observation's option count.
    pub options: usize,
}

/// A game that left the pool: finished, capped, or faulted (reported as
/// capped so training never panics on an engine error).
#[derive(Debug, PartialEq, Eq)]
pub struct Finished {
    pub game: usize,
    pub decisions: usize,
    pub capped: bool,
    /// The winner's seat index, or -1 for a draw or a capped game.
    pub winner: i32,
}

#[derive(Default)]
pub struct Batch {
    pub rows: Vec<Row>,
    pub finished: Vec<Finished>,
}

pub(crate) struct Game {
    pub(crate) index: usize,
    pub(crate) state: Result<Match, Error>,
    decisions: usize,
    cap: usize,
}

impl Game {
    pub(crate) fn new(index: usize, seed: u64, mode: u8, cap: usize, subgames: bool) -> Self {
        let state = match mode_expansions(mode) {
            Some(expansions) => Match::with_subgames(expansions, seed, subgames),
            None => Err(Error::Engine(crate::Error::Unsupported("training mode"))),
        };
        if let Err(error) = &state {
            log_fault(&format!(
                "WARNING engine_next_fault | game {index} | seed {seed} | mode {mode} | setup | {error}"
            ));
        }
        let mut game = Self {
            index,
            state,
            decisions: 0,
            cap,
        };
        game.check_prompt();
        game
    }

    fn capped(&self) -> bool {
        match &self.state {
            Ok(session) => {
                !matches!(session.prompt(), Prompt::Over(_))
                    && self.cap > 0
                    && self.decisions >= self.cap
            }
            Err(_) => true,
        }
    }

    pub(crate) fn finished(&self) -> Option<Finished> {
        let capped = self.capped();
        let winner = match &self.state {
            Ok(session) if !capped => match session.prompt() {
                Prompt::Over(crate::MatchOutcome::Winner { player, .. }) => player.index() as i32,
                // A mandatory-loop draw is a pathological game: report it as
                // capped so the collector keeps its history for diagnosis.
                Prompt::Over(crate::MatchOutcome::Draw(crate::DrawReason::Loop)) => {
                    return Some(Finished {
                        game: self.index,
                        decisions: self.decisions,
                        capped: true,
                        winner: -1,
                    });
                }
                Prompt::Over(crate::MatchOutcome::Draw(_)) => -1,
                _ => return None,
            },
            _ => -1,
        };
        Some(Finished {
            game: self.index,
            decisions: self.decisions,
            capped,
            winner,
        })
    }

    pub(crate) fn step(&mut self, choice: usize) {
        let Ok(session) = &mut self.state else {
            return;
        };
        let step = match session.prompt() {
            Prompt::Decision(_) => Step::Choice(choice),
            Prompt::Turn { actions, .. } => match actions.get(choice) {
                Some(&action) => Step::Action(action),
                None => {
                    self.fail(Error::InvalidStep, Some(choice));
                    return;
                }
            },
            Prompt::Over(_) => {
                self.fail(Error::InvalidStep, Some(choice));
                return;
            }
        };
        match session.apply(step) {
            Ok(()) => self.decisions += 1,
            Err(error) => self.fail(error, Some(choice)),
        }
        self.check_prompt();
    }

    fn check_prompt(&mut self) {
        if let Ok(session) = &self.state
            && let Err(error) = has_options(session.prompt())
        {
            // The step succeeded, so the record already includes the input
            // that produced the empty prompt.
            self.fail(error, None);
        }
    }

    fn fail(&mut self, error: Error, attempted_option: Option<usize>) {
        if let Ok(session) = &self.state {
            log_fault(&fault_report(self.index, session, &error, attempted_option));
        }
        self.state = Err(error);
    }
}

/// A prompt without options is an engine fault: the game leaves the pool as
/// capped, so the collector keeps its history instead of failing the batch.
fn has_options(prompt: &Prompt) -> Result<(), Error> {
    let empty = match prompt {
        Prompt::Decision(decision) => decision.options.is_empty(),
        Prompt::Turn { actions, .. } => actions.is_empty(),
        Prompt::Over(_) => false,
    };
    if empty {
        Err(Error::Engine(crate::Error::Unsupported(
            "a prompt without options",
        )))
    } else {
        Ok(())
    }
}

/// Keep the failed input separately: Match::apply is atomic, so its record
/// contains only successful steps. These are internal diagnostics, not a
/// viewer-safe projection. Formatting happens only when a game faults.
fn fault_report(
    index: usize,
    session: &Match,
    error: &Error,
    attempted_option: Option<usize>,
) -> String {
    let record = session.record();
    let mode = TRAINING_MODES[record.expansions.index()];
    let events = session.events();
    let recent = &events[events.len().saturating_sub(16)..];
    let replay = record.to_text().unwrap_or_else(|error| error.to_string());
    // One line per fault prevents different pool workers from interleaving
    // a replay. The quoted replay preserves actual newlines as escapes.
    format!(
        "WARNING engine_next_fault | game {index} | seed {} | mode {mode} | decisions {} \
         | error {error} | attempted_option {attempted_option:?} | prompt {:?} \
         | recent_events {recent:?} | replay {replay:?}",
        record.seed,
        record.steps.len(),
        session.prompt(),
    )
}

fn log_fault(report: &str) {
    // Diagnostics must not turn a broken log pipe into a worker panic.
    let _ = writeln!(std::io::stderr().lock(), "{report}");
}

enum Command {
    /// Games to start, their decision cap, and whether Martian Internet plays
    /// its printed subgame.
    Reset(Vec<(usize, u64, u8)>, usize, bool),
    /// Choices for this worker's live games, then new games to start before
    /// observing (the asynchronous collector refills finished slots).
    Step(Vec<(usize, usize)>, Vec<(usize, u64, u8)>, usize, bool),
    Stop,
}

struct Worker {
    send: Sender<Command>,
    receive: Receiver<Result<Batch, String>>,
    thread: JoinHandle<()>,
}

pub struct ReflexPool {
    workers: Vec<Worker>,
    /// Game index and option count of every live row, in game order.
    active: Vec<(usize, usize)>,
    failed: bool,
    /// The next game index; indices only grow, so a refilled slot never
    /// reuses a finished game's identity.
    next_index: usize,
    decision_cap: usize,
    /// Martian Internet plays its printed subgame ([`Match::with_subgames`])
    /// in every game started since the last reset.
    subgames: bool,
}

impl ReflexPool {
    pub fn new(workers: usize) -> Result<Self, String> {
        if !(1..=32).contains(&workers) {
            return Err("reflex worker count must be between 1 and 32".into());
        }
        let mut pool = Self {
            workers: Vec::new(),
            active: Vec::new(),
            failed: false,
            next_index: 0,
            decision_cap: 0,
            subgames: false,
        };
        for index in 0..workers {
            let (send, commands) = mpsc::channel();
            let (results, receive) = mpsc::channel();
            let thread = thread::Builder::new()
                .name(format!("reflex-next-{index}"))
                .spawn(move || worker_loop(commands, results))
                .map_err(|e| e.to_string())?;
            pool.workers.push(Worker {
                send,
                receive,
                thread,
            });
        }
        Ok(pool)
    }

    /// Start one match per seed in the given training modes under the Martian
    /// Internet draw variant; `decision_cap` of zero means uncapped.
    pub fn reset(
        &mut self,
        seeds: &[u64],
        modes: &[u8],
        decision_cap: usize,
    ) -> Result<Batch, String> {
        self.reset_with_subgames(seeds, modes, decision_cap, false)
    }

    /// `reset`; with `subgames`, Martian Internet plays its printed subgame
    /// in these games and in those `step_with` starts after them.
    pub fn reset_with_subgames(
        &mut self,
        seeds: &[u64],
        modes: &[u8],
        decision_cap: usize,
        subgames: bool,
    ) -> Result<Batch, String> {
        self.check_open()?;
        if seeds.is_empty()
            || seeds.len() != modes.len()
            || modes
                .iter()
                .any(|&mode| usize::from(mode) >= Expansions::COMBINATIONS)
        {
            return Err(
                "reset needs equally sized nonempty seeds/modes with modes in 0..32".into(),
            );
        }
        let mut assignments = vec![Vec::new(); self.workers.len()];
        for (index, (&seed, &mode)) in seeds.iter().zip(modes).enumerate() {
            assignments[index % self.workers.len()].push((index, seed, mode));
        }
        self.next_index = seeds.len();
        self.decision_cap = decision_cap;
        self.subgames = subgames;
        self.dispatch(
            assignments
                .into_iter()
                .map(|games| Command::Reset(games, decision_cap, subgames)),
        )
    }

    /// Apply one choice index per active row, in row order.
    pub fn step(&mut self, choices: &[usize]) -> Result<Batch, String> {
        self.step_with(choices, &[], &[])
    }

    /// `step`, then start one new match per seed (indices continue from the
    /// last started game) before observing. Finished games leave the pool in
    /// the same batch, so the asynchronous collector keeps its slot count.
    pub fn step_with(
        &mut self,
        choices: &[usize],
        seeds: &[u64],
        modes: &[u8],
    ) -> Result<Batch, String> {
        self.check_open()?;
        if choices.len() != self.active.len() || (choices.is_empty() && seeds.is_empty()) {
            return Err("one choice is required for every active game".into());
        }
        if seeds.len() != modes.len()
            || modes
                .iter()
                .any(|&mode| usize::from(mode) >= Expansions::COMBINATIONS)
        {
            return Err("new games need equally sized seeds/modes with modes in 0..32".into());
        }
        // Reject the entire request before any worker advances a match.
        for (&choice, &(game, options)) in choices.iter().zip(&self.active) {
            if choice >= options {
                return Err(format!(
                    "illegal reflex choice {choice} for game {game} with {options} options"
                ));
            }
        }
        let mut assignments = vec![Vec::new(); self.workers.len()];
        for (&choice, &(game, _)) in choices.iter().zip(&self.active) {
            assignments[game % self.workers.len()].push((game, choice));
        }
        let mut starts = vec![Vec::new(); self.workers.len()];
        for (&seed, &mode) in seeds.iter().zip(modes) {
            let index = self.next_index;
            self.next_index += 1;
            starts[index % self.workers.len()].push((index, seed, mode));
        }
        let (cap, subgames) = (self.decision_cap, self.subgames);
        self.dispatch(
            assignments
                .into_iter()
                .zip(starts)
                .map(|(choices, new)| Command::Step(choices, new, cap, subgames)),
        )
    }

    fn check_open(&self) -> Result<(), String> {
        if self.workers.is_empty() || self.failed {
            Err("reflex pool is closed or a worker failed".into())
        } else {
            Ok(())
        }
    }

    fn dispatch(&mut self, commands: impl Iterator<Item = Command>) -> Result<Batch, String> {
        self.failed = true;
        for (worker, command) in self.workers.iter().zip(commands) {
            worker
                .send
                .send(command)
                .map_err(|_| "reflex worker stopped")?;
        }
        let mut batch = Batch::default();
        for worker in &self.workers {
            let mut result = worker
                .receive
                .recv()
                .map_err(|_| "reflex worker stopped")??;
            batch.rows.append(&mut result.rows);
            batch.finished.append(&mut result.finished);
        }
        batch.rows.sort_unstable_by_key(|row| row.game);
        batch.finished.sort_unstable_by_key(|game| game.game);
        self.active = batch
            .rows
            .iter()
            .map(|row| (row.game, row.options))
            .collect();
        self.failed = false;
        Ok(batch)
    }

    pub fn close(&mut self) {
        for worker in &self.workers {
            let _ = worker.send.send(Command::Stop);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.thread.join();
        }
        self.active.clear();
    }
}

impl Drop for ReflexPool {
    fn drop(&mut self) {
        self.close();
    }
}

fn worker_loop(commands: Receiver<Command>, results: Sender<Result<Batch, String>>) {
    let mut games: Vec<Game> = Vec::new();
    while let Ok(command) = commands.recv() {
        match command {
            Command::Stop => break,
            Command::Reset(specs, cap, subgames) => {
                games = specs
                    .into_iter()
                    .map(|(index, seed, mode)| Game::new(index, seed, mode, cap, subgames))
                    .collect();
            }
            Command::Step(choices, new, cap, subgames) => {
                for (game, (index, choice)) in games.iter_mut().zip(choices) {
                    assert_eq!(game.index, index, "reflex step out of order");
                    game.step(choice);
                }
                games.extend(
                    new.into_iter()
                        .map(|(index, seed, mode)| Game::new(index, seed, mode, cap, subgames)),
                );
            }
        }
        let batch = observe(&games);
        games.retain(|game| game.finished().is_none());
        if results.send(batch).is_err() {
            break;
        }
    }
}

fn observe(games: &[Game]) -> Result<Batch, String> {
    let mut batch = Batch::default();
    for game in games {
        if let Some(finished) = game.finished() {
            batch.finished.push(finished);
            continue;
        }
        let session = game
            .state
            .as_ref()
            .map_err(|error| format!("live reflex game {} faulted: {error}", game.index))?;
        let (player, count) = match session.prompt() {
            Prompt::Decision(decision) => (decision.player, decision.options.len()),
            Prompt::Turn { player, actions } => (*player, actions.len()),
            Prompt::Over(_) => unreachable!("finished games were filtered"),
        };
        let observation = session
            .game()
            .observe(player)
            .map_err(|error| format!("observing reflex game {}: {error:?}", game.index))?;
        if observation.options.len() != count {
            return Err(format!(
                "reflex game {}: observation encodes {} options for a prompt with {count}",
                game.index,
                observation.options.len()
            ));
        }
        batch.rows.push(Row {
            game: game.index,
            player: player.index(),
            observation: observation.row(),
            options: count,
        });
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::HEADER;

    #[test]
    fn empty_decisions_and_turns_are_faults_but_terminal_prompts_are_not() {
        let session = Match::new(Expansions::BASE, 7).unwrap();
        let mut prompt = session.prompt().clone();
        assert!(has_options(&prompt).is_ok());
        let Prompt::Decision(decision) = &mut prompt else {
            panic!("opening choice");
        };
        decision.options.clear();
        assert!(has_options(&prompt).is_err());
        assert!(
            has_options(&Prompt::Turn {
                player: crate::Player::One,
                actions: Vec::new(),
            })
            .is_err()
        );
        assert!(
            has_options(&Prompt::Over(crate::MatchOutcome::Draw(
                crate::DrawReason::Loop
            )))
            .is_ok()
        );
    }

    #[test]
    fn the_subgames_flag_reaches_the_games_a_pool_starts() {
        for subgames in [false, true] {
            let game = Game::new(0, 1, 8, 0, subgames);
            assert_eq!(game.state.as_ref().unwrap().record().subgames, subgames);
        }
        let mut pool = ReflexPool::new(2).unwrap();
        let batch = pool
            .reset_with_subgames(&[1, 2], &[8, 24], 0, true)
            .unwrap();
        assert_eq!(batch.rows.len(), 2);
        assert!(pool.subgames);
        let batch = pool.step_with(&[0, 0], &[3], &[8]).unwrap();
        assert_eq!(batch.rows.len(), 3);
        pool.reset(&[1], &[8], 0).unwrap();
        assert!(!pool.subgames);
    }

    #[test]
    fn faults_keep_replay_context_and_leave_the_pool_without_a_policy_row() {
        let mut game = Game::new(42, 7, 0, 0, false);
        game.step(0);
        let session = game.state.as_ref().unwrap();
        let report = fault_report(42, session, &Error::InvalidStep, Some(usize::MAX));
        assert!(
            report
                .contains("WARNING engine_next_fault | game 42 | seed 7 | mode base | decisions 1")
        );
        assert!(report.contains(&format!("attempted_option Some({})", usize::MAX)));
        assert!(report.contains("prompt Decision("));
        let replay = session.record().to_text().unwrap();
        assert!(report.contains(&format!("replay {replay:?}")));
        assert_eq!(report.lines().count(), 1, "workers log one atomic line");
        assert_eq!(
            Match::from_record(&crate::Record::from_text(&replay).unwrap()).unwrap(),
            *session
        );

        game.step(usize::MAX);
        let batch = observe(&[game]).unwrap();
        assert!(batch.rows.is_empty());
        assert_eq!(
            batch.finished,
            vec![Finished {
                game: 42,
                decisions: 1,
                capped: true,
                winner: -1,
            }]
        );
    }

    fn expected_mode_name(index: usize) -> String {
        let names = ["figures", "cities", "echoes", "artifacts", "unseen"];
        let enabled: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|(bit, _)| index & (1 << bit) != 0)
            .map(|(_, name)| *name)
            .collect();
        if enabled.is_empty() {
            "base".into()
        } else {
            enabled.join("+")
        }
    }

    #[test]
    fn training_modes_name_every_combination_in_bit_order() {
        for (index, name) in TRAINING_MODES.iter().enumerate() {
            assert_eq!(*name, expected_mode_name(index), "mode {index}");
            let expansions = mode_expansions(index as u8).unwrap();
            assert_eq!(expansions.index(), index);
            assert_eq!(expansions.figures(), index & 1 != 0);
            assert_eq!(expansions.unseen(), index & 16 != 0);
        }
        assert_eq!(TRAINING_MODES[3], "figures+cities");
        assert_eq!(mode_expansions(32), None);
        assert_eq!(mode_expansions(255), None);
    }

    /// Game, player, option count and observation of every row.
    type Snapshot = Vec<(usize, usize, usize, Vec<f32>)>;

    fn snapshot(batch: &Batch) -> Snapshot {
        batch
            .rows
            .iter()
            .map(|row| (row.game, row.player, row.options, row.observation.clone()))
            .collect()
    }

    fn play(
        pool: &mut ReflexPool,
        seeds: &[u64],
        modes: &[u8],
        cap: usize,
        steps: usize,
    ) -> Vec<Snapshot> {
        let mut history = Vec::new();
        let mut batch = pool.reset(seeds, modes, cap).unwrap();
        history.push(snapshot(&batch));
        for _ in 0..steps {
            if batch.rows.is_empty() {
                break;
            }
            let choices: Vec<usize> = batch.rows.iter().map(|row| row.options - 1).collect();
            batch = pool.step(&choices).unwrap();
            history.push(snapshot(&batch));
        }
        history
    }

    #[test]
    fn rows_are_deterministic_and_well_formed() {
        let seeds = [3, 5, 7, 11, 13];
        let modes = [0, 1, 7, 15, 31];
        let mut first = ReflexPool::new(2).unwrap();
        let mut second = ReflexPool::new(3).unwrap();
        let a = play(&mut first, &seeds, &modes, 0, 12);
        let b = play(&mut second, &seeds, &modes, 0, 12);
        assert_eq!(a, b, "worker count must not change the rows");
        assert_eq!(a.len(), 13);
        for batch in &a {
            let games: Vec<usize> = batch.iter().map(|row| row.0).collect();
            assert!(games.windows(2).all(|w| w[0] < w[1]), "rows sorted by game");
            for (_, player, options, row) in batch {
                assert!(*player < 2);
                assert!(*options >= 1);
                assert!(row.len() > HEADER);
                assert_eq!(row[4] as usize, *options, "header option count");
                assert_eq!(row[5] as usize, *player, "header viewer seat");
            }
        }
    }

    #[test]
    fn refilled_slots_get_fresh_indices_and_the_pool_keeps_its_size() {
        let mut pool = ReflexPool::new(3).unwrap();
        let mut batch = pool
            .reset(&[1, 2, 3, 4, 5, 6, 7, 8], &[0, 1, 2, 3, 4, 8, 16, 31], 3)
            .unwrap();
        let mut started = 8u64;
        let mut finished = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..40 {
            let choices: Vec<usize> = batch.rows.iter().map(|_| 0).collect();
            let refill = batch.finished.len();
            for game in &batch.finished {
                assert!(seen.insert(game.game), "game {} reported twice", game.game);
                finished.push(game.game);
            }
            if finished.len() >= 24 {
                break;
            }
            let seeds: Vec<u64> = (0..refill as u64).map(|i| started + i).collect();
            let modes: Vec<u8> = (0..refill)
                .map(|i| ((started as usize + i) % 32) as u8)
                .collect();
            started += refill as u64;
            batch = pool.step_with(&choices, &seeds, &modes).unwrap();
            // Every finished slot was refilled: the live count stays at eight.
            assert_eq!(batch.rows.len() + batch.finished.len(), 8);
            let mut games: Vec<usize> = batch.rows.iter().map(|r| r.game).collect();
            games.sort_unstable();
            assert!(
                games.windows(2).all(|w| w[0] < w[1]),
                "rows sorted and distinct"
            );
        }
        assert!(finished.len() >= 24);
        assert!(
            finished.iter().max().unwrap() >= &20,
            "indices keep growing"
        );
        // With no live rows a step must start games; a plain step then works.
        let batch = if batch.rows.is_empty() {
            assert!(pool.step(&[]).is_err());
            pool.step_with(&[], &[99], &[0]).unwrap()
        } else {
            batch
        };
        let choices: Vec<usize> = batch.rows.iter().map(|_| 0).collect();
        assert!(!pool.step(&choices).unwrap().rows.is_empty());
    }

    #[test]
    fn every_mode_finishes_under_the_cap_exactly_once() {
        let seeds: Vec<u64> = (0..32).map(|i| 1000 + i).collect();
        let modes: Vec<u8> = (0..32).collect();
        let mut pool = ReflexPool::new(4).unwrap();
        let cap = 6;
        let mut batch = pool.reset(&seeds, &modes, cap).unwrap();
        let mut finished: Vec<Finished> = Vec::new();
        let mut steps = 0;
        loop {
            finished.append(&mut batch.finished);
            if batch.rows.is_empty() {
                break;
            }
            assert!(steps < cap, "every game caps within {cap} steps");
            let choices = vec![0; batch.rows.len()];
            batch = pool.step(&choices).unwrap();
            steps += 1;
        }
        let games: Vec<usize> = finished.iter().map(|f| f.game).collect();
        assert_eq!(games, (0..32).collect::<Vec<_>>());
        for game in &finished {
            assert!(game.decisions <= cap);
            assert!(!game.capped || game.winner == -1);
            assert!(game.capped || game.decisions < cap);
        }
        assert!(finished.iter().any(|game| game.capped));
        assert!(pool.step(&[]).is_err(), "nothing is active after the cap");
    }

    #[test]
    fn illegal_choice_is_rejected_without_advancing() {
        let mut pool = ReflexPool::new(2).unwrap();
        let batch = pool.reset(&[1, 2, 3], &[0, 1, 2], 0).unwrap();
        let before = snapshot(&batch);
        let mut choices: Vec<usize> = batch.rows.iter().map(|row| row.options - 1).collect();
        choices[1] = batch.rows[1].options;
        assert!(pool.step(&choices).is_err());
        assert!(pool.step(&choices[..2]).is_err(), "one choice per row");
        choices[1] = 0;
        let after = pool.step(&choices).unwrap();
        let mut replay = ReflexPool::new(1).unwrap();
        let replay_batch = replay.reset(&[1, 2, 3], &[0, 1, 2], 0).unwrap();
        assert_eq!(snapshot(&replay_batch), before);
        let replayed = replay.step(&choices).unwrap();
        assert_eq!(snapshot(&replayed), snapshot(&after));
    }

    #[test]
    fn invalid_arguments_are_rejected() {
        assert!(ReflexPool::new(0).is_err());
        assert!(ReflexPool::new(33).is_err());
        let mut pool = ReflexPool::new(1).unwrap();
        assert!(pool.reset(&[], &[], 0).is_err());
        assert!(pool.reset(&[1, 2], &[0], 0).is_err());
        assert!(pool.reset(&[1], &[32], 0).is_err());
        assert!(pool.step(&[0]).is_err(), "no active games before reset");
        pool.close();
        assert!(pool.reset(&[1], &[0], 0).is_err());
    }
}
