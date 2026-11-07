//! Parallel learned search over many training games, with leaf inference
//! left to the caller so one network call can serve every game.
//!
//! Each live game is queued for a worker, searching, or reported: its tree
//! waits for Q vectors of up to `leaf_batch` leaves, or its search is
//! finished and the root waits for the caller's choice. Workers take games
//! from one queue, oldest first, and report each as it stops. The caller
//! collects what has reported, scores it in one network call and answers;
//! answered games rejoin the queue while the caller works, so no game waits
//! for a slow one. Batches are answered in collection order, and the caller
//! may collect the next before answering the last, to score one batch while
//! preparing another. The caller owns inference, move choice and learning, as
//! with [`crate::ReflexPool`]. Batches are ordered by game index, and search
//! seeds derive from each game's seed, so a game's play depends only on its
//! seed and the Q vectors it receives. `reset` and `step` wait for every
//! game, so their batches also do not depend on worker count or timing.

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crate::reflex::{Finished, Game as Played};
use crate::search::Tree;
use crate::session::Prompt;
use crate::{Expansions, Rng, SearchConfig};

/// Search settings for every tree of a pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolSearch {
    pub config: SearchConfig,
    /// Leaves a tree may leave waiting at once (each with a virtual loss).
    pub leaf_batch: usize,
}

/// A leaf observation waiting for its Q vector.
pub struct LeafRow {
    pub game: usize,
    pub observation: Vec<f32>,
    pub options: usize,
}

/// A root waiting for the caller's choice, with its search statistics per
/// option (the observation's option order). Options the searcher cannot tell
/// apart share the first one's statistics; the others report zero visits.
pub struct RootRow {
    pub game: usize,
    pub player: usize,
    pub observation: Vec<f32>,
    pub options: usize,
    pub visits: Vec<u32>,
    /// Mean search value for the player to move, zero when unvisited.
    pub values: Vec<f32>,
    /// The network's root prior per option.
    pub priors: Vec<f32>,
    /// False when there was nothing to search (one move as the player sees
    /// it) or the root refused determinization; the statistics are then
    /// zero and the caller chooses from the network or the rules.
    pub searched: bool,
    /// Tracking refused a world (the root's or a later sample's), so the
    /// root is unsearched.
    pub refused: bool,
}

#[derive(Default)]
pub struct SearchBatch {
    /// In game order, each game's leaves in its tree's order.
    pub leaves: Vec<LeafRow>,
    pub roots: Vec<RootRow>,
    pub finished: Vec<Finished>,
}

struct Game {
    played: Played,
    tree: Option<Tree>,
    rng: Rng,
}

impl Game {
    fn new(index: usize, seed: u64, mode: u8, cap: usize) -> Self {
        Self {
            // Training keeps the Martian Internet draw variant its search values.
            played: Played::new(index, seed, mode, cap, false),
            tree: None,
            rng: Rng::new(seed ^ 0x5EA2_C4ED_9001_7EE5),
        }
    }

    fn index(&self) -> usize {
        self.played.index
    }

    /// Run the search until it waits for leaves or its root waits for a
    /// choice; report either (or the finished game).
    fn advance(&mut self, search: PoolSearch, batch: &mut SearchBatch) -> Result<(), String> {
        if let Some(finished) = self.played.finished() {
            batch.finished.push(finished);
            return Ok(());
        }
        let index = self.index();
        let session = self
            .played
            .state
            .as_ref()
            .map_err(|error| format!("live search game {index} faulted: {error}"))?;
        let (player, options) = match session.prompt() {
            Prompt::Decision(decision) => (decision.player, decision.options.len()),
            Prompt::Turn { player, actions } => (*player, actions.len()),
            Prompt::Over(_) => unreachable!("finished games were reported"),
        };
        let mut refused = false;
        if self.tree.is_none() {
            let config = SearchConfig {
                seed: self.rng.next_u64(),
                threads: 1,
                ..search.config
            };
            match Tree::new(session.game(), config, true) {
                Ok(tree) => self.tree = Some(tree),
                Err(_) => refused = true,
            }
        }
        let mut root = RootRow {
            game: index,
            player: player.index(),
            observation: Vec::new(),
            options,
            visits: vec![0; options],
            values: vec![0.0; options],
            priors: vec![0.0; options],
            searched: false,
            refused,
        };
        if let Some(tree) = &mut self.tree {
            match tree.select(search.leaf_batch) {
                Ok(0) => {}
                Ok(_) => {
                    for observation in tree.pending() {
                        batch.leaves.push(LeafRow {
                            game: index,
                            observation: observation.row(),
                            options: observation.options.len(),
                        });
                    }
                    return Ok(());
                }
                // A later world refused: report the root unsearched.
                Err(_) => {
                    self.tree = None;
                    root.refused = true;
                }
            }
        }
        if let Some(tree) = self.tree.take() {
            let result = tree.result().map_err(|e| format!("game {index}: {e:?}"))?;
            root.searched = result.moves.len() > 1;
            if root.searched {
                for m in &result.moves {
                    root.visits[m.option] = m.visits;
                    root.values[m.option] = m.value as f32;
                    root.priors[m.option] = m.prior as f32;
                }
            }
        }
        let observation = session
            .game()
            .observe(player)
            .map_err(|error| format!("observing search game {index}: {error:?}"))?;
        if observation.options.len() != options {
            return Err(format!(
                "search game {index}: observation encodes {} options for a prompt with {options}",
                observation.options.len()
            ));
        }
        root.observation = observation.row();
        batch.roots.push(root);
        Ok(())
    }
}

/// A live game with its leaves' Q vectors or its root's choice, or a game
/// to start, under the settings it was queued with.
struct Task {
    work: Work,
    cap: usize,
    search: PoolSearch,
}

enum Work {
    Live {
        game: Box<Game>,
        q: Vec<Vec<f32>>,
        choice: Option<usize>,
    },
    New {
        index: usize,
        seed: u64,
        mode: u8,
    },
}

impl Task {
    fn index(&self) -> usize {
        match &self.work {
            Work::Live { game, .. } => game.index(),
            Work::New { index, .. } => *index,
        }
    }

    /// Resolve the leaves or apply the choice, then advance the game;
    /// returns the game while it is live.
    fn run(self, batch: &mut SearchBatch) -> Result<Option<Game>, String> {
        let mut game = match self.work {
            Work::New { index, seed, mode } => Game::new(index, seed, mode, self.cap),
            Work::Live {
                mut game,
                q,
                choice,
            } => {
                let index = game.index();
                if !q.is_empty() {
                    game.tree
                        .as_mut()
                        .ok_or("Q values for a game that is not searching")?
                        .resolve(&q)
                        .map_err(|e| format!("game {index}: {e:?}"))?;
                }
                if let Some(choice) = choice {
                    if game.tree.is_some() {
                        return Err(format!("choice for game {index} while it searches"));
                    }
                    game.played.step(choice);
                }
                *game
            }
        };
        game.advance(self.search, batch)?;
        Ok(game.played.finished().is_none().then_some(game))
    }
}

/// Tasks waiting for a worker, oldest first.
struct Queue {
    tasks: VecDeque<Task>,
    open: bool,
}

/// The queue the workers wait on.
struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
}

impl Shared {
    /// The oldest waiting task; `None` once the pool closes.
    fn next(&self) -> Option<Task> {
        let mut queue = self.queue.lock().ok()?;
        loop {
            if !queue.open {
                return None;
            }
            if let Some(task) = queue.tasks.pop_front() {
                return Some(task);
            }
            queue = self.ready.wait(queue).ok()?;
        }
    }
}

/// One task's batch rows and its game while live.
type Done = Result<(SearchBatch, Option<Game>), String>;

/// A collected batch's games, waiting for the caller's answer, and the game
/// and option count of each of its leaves and roots, in batch order.
struct Unanswered {
    games: Vec<Game>,
    leaves: Vec<(usize, usize)>,
    roots: Vec<(usize, usize)>,
}

pub struct SearchPool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
    results: Receiver<Done>,
    /// Queued or searching games, which have not reported.
    running: usize,
    /// Games that reported since the last collected batch, and their rows.
    reported: SearchBatch,
    reported_games: Vec<Game>,
    /// Collected batches not yet answered, oldest first.
    unanswered: VecDeque<Unanswered>,
    failed: bool,
    next_index: usize,
    decision_cap: usize,
    search: PoolSearch,
}

impl SearchPool {
    pub fn new(workers: usize) -> Result<Self, String> {
        if !(1..=32).contains(&workers) {
            return Err("search worker count must be between 1 and 32".into());
        }
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                tasks: VecDeque::new(),
                open: true,
            }),
            ready: Condvar::new(),
        });
        let (results, receive) = mpsc::channel();
        let mut pool = Self {
            shared,
            workers: Vec::new(),
            results: receive,
            running: 0,
            reported: SearchBatch::default(),
            reported_games: Vec::new(),
            unanswered: VecDeque::new(),
            failed: false,
            next_index: 0,
            decision_cap: 0,
            search: PoolSearch {
                config: SearchConfig::learned(64),
                leaf_batch: 8,
            },
        };
        for index in 0..workers {
            let shared = Arc::clone(&pool.shared);
            let results = results.clone();
            let thread = thread::Builder::new()
                .name(format!("search-next-{index}"))
                .spawn(move || worker_loop(&shared, &results))
                .map_err(|e| e.to_string())?;
            pool.workers.push(thread);
        }
        Ok(pool)
    }

    /// Start one match per seed in the given training modes, searching each
    /// root with `search`, and wait for every game; `decision_cap` of zero
    /// means uncapped.
    pub fn reset(
        &mut self,
        seeds: &[u64],
        modes: &[u8],
        decision_cap: usize,
        search: PoolSearch,
    ) -> Result<SearchBatch, String> {
        self.check_open()?;
        check_search(search)?;
        if seeds.is_empty() || seeds.len() != modes.len() || !valid_modes(modes) {
            return Err(
                "reset needs equally sized nonempty seeds/modes with modes in 0..32".into(),
            );
        }
        // Games from before are dropped, faults included.
        self.failed = true;
        while self.running > 0 {
            let _ = self.results.recv().map_err(|_| "search worker stopped")?;
            self.running -= 1;
        }
        self.failed = false;
        self.reported = SearchBatch::default();
        self.reported_games.clear();
        self.unanswered.clear();
        self.next_index = seeds.len();
        self.decision_cap = decision_cap;
        self.search = search;
        let tasks = seeds
            .iter()
            .zip(modes)
            .enumerate()
            .map(|(index, (&seed, &mode))| self.task(Work::New { index, seed, mode }))
            .collect();
        self.queue(tasks)?;
        self.collect(None)
    }

    /// Searches started from now on use `search`; trees already running
    /// finish with their own settings.
    pub fn set_search(&mut self, search: PoolSearch) -> Result<(), String> {
        check_search(search)?;
        self.search = search;
        Ok(())
    }

    /// `answer`, then wait for every game.
    pub fn step(
        &mut self,
        q: &[f32],
        choices: &[usize],
        seeds: &[u64],
        modes: &[u8],
    ) -> Result<SearchBatch, String> {
        self.answer(q, choices, seeds, modes)?;
        self.collect(None)
    }

    /// Answer the oldest collected batch not yet answered: back up `q` (every
    /// leaf's Q vector, concatenated in batch order), apply one choice per
    /// root, and start one new match per seed. The games search on while the
    /// caller works.
    pub fn answer(
        &mut self,
        q: &[f32],
        choices: &[usize],
        seeds: &[u64],
        modes: &[u8],
    ) -> Result<(), String> {
        self.check_open()?;
        let batch = self
            .unanswered
            .front()
            .ok_or("no collected batch waits for an answer")?;
        let expected: usize = batch.leaves.iter().map(|&(_, options)| options).sum();
        if q.len() != expected {
            return Err(format!(
                "{} Q values for leaves with {expected} options",
                q.len()
            ));
        }
        if choices.len() != batch.roots.len() {
            return Err("one choice is required for every waiting root".into());
        }
        if q.iter().any(|v| !v.is_finite()) {
            return Err("Q values must be finite".into());
        }
        if seeds.len() != modes.len() || !valid_modes(modes) {
            return Err("new games need equally sized seeds/modes with modes in 0..32".into());
        }
        if batch.games.is_empty()
            && seeds.is_empty()
            && self.running == 0
            && self.unanswered.len() == 1
        {
            return Err("the pool has no live games".into());
        }
        for (&choice, &(game, options)) in choices.iter().zip(&batch.roots) {
            if choice >= options {
                return Err(format!(
                    "illegal choice {choice} for game {game} with {options} options"
                ));
            }
        }
        // Every game of the batch waits for either its leaves' Q vectors or
        // its root's choice.
        let Unanswered {
            mut games,
            leaves,
            roots,
        } = self.unanswered.pop_front().expect("checked above");
        games.sort_unstable_by_key(Game::index);
        let slot = |game: usize| {
            games
                .binary_search_by_key(&game, Game::index)
                .map_err(|_| format!("search answer for unknown game {game}"))
        };
        let mut work = vec![(Vec::new(), None); games.len()];
        let mut at = 0;
        for &(game, options) in &leaves {
            work[slot(game)?].0.push(q[at..at + options].to_vec());
            at += options;
        }
        for (&choice, &(game, _)) in choices.iter().zip(&roots) {
            work[slot(game)?].1 = Some(choice);
        }
        let mut tasks: Vec<Task> = games
            .into_iter()
            .zip(work)
            .map(|(game, (q, choice))| {
                self.task(Work::Live {
                    game: Box::new(game),
                    q,
                    choice,
                })
            })
            .collect();
        for (&seed, &mode) in seeds.iter().zip(modes) {
            tasks.push(self.task(Work::New {
                index: self.next_index,
                seed,
                mode,
            }));
            self.next_index += 1;
        }
        self.queue(tasks)
    }

    /// The games that have reported, once their leaf and root rows number at
    /// least `min_rows` or no game is searching; `None` waits for every game,
    /// which makes batches independent of thread timing. Several batches may
    /// wait for answers at once; `answer` takes them in collection order.
    pub fn collect(&mut self, min_rows: Option<usize>) -> Result<SearchBatch, String> {
        self.check_open()?;
        self.failed = true;
        loop {
            while let Ok(done) = self.results.try_recv() {
                self.receive(done)?;
            }
            let rows = self.reported.leaves.len() + self.reported.roots.len();
            let enough = match min_rows {
                Some(min_rows) => rows >= min_rows.max(1),
                None => false,
            };
            if enough || self.running == 0 {
                break;
            }
            let done = self.results.recv().map_err(|_| "search worker stopped")?;
            self.receive(done)?;
        }
        let mut batch = std::mem::take(&mut self.reported);
        // Stable: a game's leaves keep their tree order.
        batch.leaves.sort_by_key(|leaf| leaf.game);
        batch.roots.sort_unstable_by_key(|root| root.game);
        batch.finished.sort_unstable_by_key(|game| game.game);
        self.unanswered.push_back(Unanswered {
            games: std::mem::take(&mut self.reported_games),
            leaves: batch.leaves.iter().map(|l| (l.game, l.options)).collect(),
            roots: batch.roots.iter().map(|r| (r.game, r.options)).collect(),
        });
        self.failed = false;
        Ok(batch)
    }

    fn receive(&mut self, done: Done) -> Result<(), String> {
        self.running -= 1;
        let (mut rows, game) = done?;
        self.reported.leaves.append(&mut rows.leaves);
        self.reported.roots.append(&mut rows.roots);
        self.reported.finished.append(&mut rows.finished);
        self.reported_games.extend(game);
        Ok(())
    }

    fn task(&self, work: Work) -> Task {
        Task {
            work,
            cap: self.decision_cap,
            search: self.search,
        }
    }

    fn queue(&mut self, tasks: Vec<Task>) -> Result<(), String> {
        self.running += tasks.len();
        self.shared
            .queue
            .lock()
            .map_err(|e| e.to_string())?
            .tasks
            .extend(tasks);
        self.shared.ready.notify_all();
        Ok(())
    }

    fn check_open(&self) -> Result<(), String> {
        if self.workers.is_empty() || self.failed {
            Err("search pool is closed or a worker failed".into())
        } else {
            Ok(())
        }
    }

    pub fn close(&mut self) {
        if let Ok(mut queue) = self.shared.queue.lock() {
            queue.open = false;
            queue.tasks.clear();
        }
        self.shared.ready.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        self.running = 0;
        self.reported = SearchBatch::default();
        self.reported_games.clear();
        self.unanswered.clear();
    }
}

impl Drop for SearchPool {
    fn drop(&mut self) {
        self.close();
    }
}

fn valid_modes(modes: &[u8]) -> bool {
    modes
        .iter()
        .all(|&mode| usize::from(mode) < Expansions::COMBINATIONS)
}

fn check_search(search: PoolSearch) -> Result<(), String> {
    if search.leaf_batch == 0 || search.config.iterations == 0 {
        return Err("search needs positive iterations and leaf batch".into());
    }
    Tree::check(&search.config).map_err(|e| format!("{e:?}"))
}

fn worker_loop(shared: &Shared, results: &Sender<Done>) {
    while let Some(task) = shared.next() {
        let index = task.index();
        let mut batch = SearchBatch::default();
        // A game that panics must still report, or `collect` would wait for
        // it forever; its error fails the pool.
        let done = panic::catch_unwind(AssertUnwindSafe(|| task.run(&mut batch)))
            .unwrap_or_else(|_| Err(format!("search game {index} panicked")))
            .map(|game| (batch, game));
        if results.send(done).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Net;
    use crate::net::{Dimensions, synthetic_bytes};

    fn search(iterations: u32) -> PoolSearch {
        PoolSearch {
            config: SearchConfig::learned(iterations),
            leaf_batch: 4,
        }
    }

    /// Finished games and each game's root visits, in game order, and the
    /// leaf count.
    type Outcome = (Vec<Finished>, Vec<(usize, Vec<u32>)>, usize);

    /// Drive a pool to the end with a network, choosing the most-visited
    /// option (the first option of an unsearched root); with `min_rows`,
    /// answer and collect batches as games report instead of stepping,
    /// keeping up to `depth` batches collected ahead of their answers.
    fn run(
        workers: usize,
        min_rows: Option<usize>,
        depth: usize,
        seeds: &[u64],
        modes: &[u8],
    ) -> Outcome {
        let net = Net::from_bytes(&synthetic_bytes(Dimensions::TINY, 3, 0.2)).unwrap();
        let mut pool = SearchPool::new(workers).unwrap();
        let mut waiting = VecDeque::from([pool.reset(seeds, modes, 40, search(12)).unwrap()]);
        let mut finished = Vec::new();
        let mut visits = Vec::new();
        let mut leaves = 0;
        while let Some(mut batch) = waiting.pop_front() {
            finished.append(&mut batch.finished);
            let idle = pool.running == 0 && waiting.is_empty();
            if batch.leaves.is_empty() && batch.roots.is_empty() && idle {
                break;
            }
            leaves += batch.leaves.len();
            let mut q = Vec::new();
            for leaf in &batch.leaves {
                let row = net.q_row(&leaf.observation).unwrap();
                assert_eq!(row.len(), leaf.options);
                q.extend(row);
            }
            let choices: Vec<usize> = batch
                .roots
                .iter()
                .map(|root| {
                    assert_eq!(root.visits.len(), root.options);
                    assert!(!root.refused);
                    if root.searched {
                        assert_eq!(root.visits.iter().sum::<u32>(), 11);
                        visits.push((root.game, root.visits.clone()));
                    }
                    (0..root.options)
                        .max_by_key(|&i| (root.visits[i], std::cmp::Reverse(i)))
                        .unwrap()
                })
                .collect();
            match min_rows {
                None => waiting.push_back(pool.step(&q, &choices, &[], &[]).unwrap()),
                Some(min_rows) => {
                    pool.answer(&q, &choices, &[], &[]).unwrap();
                    while waiting.len() < depth && pool.running > 0 {
                        waiting.push_back(pool.collect(Some(min_rows)).unwrap());
                    }
                }
            }
        }
        finished.sort_by_key(|game| game.game);
        visits.sort_by_key(|&(game, _)| game);
        (finished, visits, leaves)
    }

    #[test]
    fn pools_search_every_game_to_its_cap_independent_of_workers() {
        // Games move between workers from step to step.
        let seeds = [3, 4, 5, 6, 7];
        let modes = [0, 31, 5, 16, 9];
        let (finished, visits, leaves) = run(1, None, 1, &seeds, &modes);
        assert_eq!(finished.len(), 5);
        assert!(finished.iter().all(|f| f.decisions <= 40));
        assert!(!visits.is_empty() && leaves > visits.len());
        assert_eq!(run(3, None, 1, &seeds, &modes), (finished, visits, leaves));
    }

    #[test]
    fn collected_batches_change_with_timing_but_games_do_not() {
        let seeds = [3, 4, 5, 6, 7];
        let modes = [0, 31, 5, 16, 9];
        let exact = run(1, None, 1, &seeds, &modes);
        assert_eq!(run(3, Some(1), 1, &seeds, &modes), exact);
        assert_eq!(run(2, Some(6), 1, &seeds, &modes), exact);
        // Two batches out at once: the next is collected before the last is answered.
        assert_eq!(run(3, Some(4), 2, &seeds, &modes), exact);
        assert_eq!(run(1, Some(1), 3, &seeds, &modes), exact);
    }

    #[test]
    fn steps_reject_mismatched_q_and_choices() {
        let mut pool = SearchPool::new(1).unwrap();
        assert!(
            pool.answer(&[], &[], &[1], &[0]).is_err(),
            "nothing was collected"
        );
        let batch = pool.reset(&[1], &[0], 0, search(8)).unwrap();
        // The opening is searched: the first batch waits for the root leaf.
        assert_eq!(batch.leaves.len(), 1);
        assert!(batch.roots.is_empty());
        let options = batch.leaves[0].options;
        assert!(pool.step(&vec![0.0; options + 1], &[], &[], &[]).is_err());
        assert!(pool.step(&vec![0.0; options], &[0], &[], &[]).is_err());
        assert!(pool.step(&vec![f32::NAN; options], &[], &[], &[]).is_err());
        assert!(pool.step(&vec![0.0; options], &[], &[], &[]).is_ok());
    }
}
