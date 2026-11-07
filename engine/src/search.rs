//! Single-observer information-set MCTS over determinized worlds.
//!
//! Each simulation samples a hidden world for the searching player, descends
//! one shared tree and scores one new leaf. Edges are keyed by the mover and
//! what the mover sees (decision alternatives, or actions with hidden
//! standard achievements named by their back), so worlds that differ only in
//! hidden cards share statistics. Values are two-player zero-sum; each edge
//! stores them from its mover's perspective.
//!
//! Two leaf evaluators share the tree:
//!
//! - **Heuristic** (no network): untracked worlds, availability-corrected
//!   UCB1, a few random actions after expansion, then the fitted
//!   `logistic4` potential.
//! - **Learned** (a next-1 Q network): tracked worlds
//!   ([`Game::determinize`]), so every leaf can be observed by its own mover.
//!   One network call per leaf gives a Q per legal option. The leaf's value is
//!   its best Q; each option's prior is a softmax of the network's policy
//!   logits if it has them, else of Q, and an unvisited edge starts from its
//!   own Q. Selection is availability-corrected PUCT.
//!
//! [`Tree`] is resumable: [`Tree::select`] runs simulations until enough
//! leaves wait for the network (with a virtual loss on their paths), and
//! [`Tree::resolve`] backs up the Q vectors the caller computed, so training
//! can batch leaves from many games into one inference call. A node is sent
//! to the network once: a simulation whose line reaches a node still waiting
//! is discarded and ends the batch, so the next one descends with the
//! answer instead of queuing the node again or being pushed to a worse line
//! by the virtual loss. A batch is therefore as wide as the tree has distinct
//! lines worth scoring, up to the caller's limit. [`Game::search`] and
//! [`Game::search_net`] drive trees to completion on the CPU.

use crate::execution::SamplingPlan;
use crate::session::Step;
use crate::{
    Action, Alternative, CardSet, Error, Game, Net, Observation, Player, Rng, Status, Zone,
};

/// Search effort and shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchConfig {
    /// Simulations per tree. A learned tree spends its first one on the root.
    pub iterations: u32,
    /// The UCB1 constant (heuristic) or the PUCT constant (learned), on
    /// values in [-1, 1].
    pub exploration: f64,
    /// Heuristic only: random actions played after expansion before the leaf
    /// is scored. Work grows with `iterations × rollout_actions`.
    pub rollout_actions: u32,
    pub seed: u64,
    /// Independent trees searched in parallel; their root statistics are
    /// merged (root parallelization). The heuristic search runs
    /// `iterations` per tree; the learned search splits `iterations` among
    /// them.
    pub threads: u32,
    /// Learned only: priors are `softmax(policy / prior_temperature)` given
    /// policy logits, else `softmax(Q / prior_temperature)`.
    pub prior_temperature: f64,
    /// Learned only: Dirichlet noise mixed into the root priors, for
    /// self-play exploration.
    pub root_noise: Option<RootNoise>,
    /// Learned only: the value counted against a path while its leaf waits
    /// for the network, so a batch of leaves spreads over different lines.
    /// 1 is a full loss; the backed-up value replaces it.
    pub virtual_loss: f64,
}

/// Root prior noise: `(1 - fraction) · prior + fraction · Dirichlet(alpha)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootNoise {
    pub alpha: f64,
    pub fraction: f64,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            iterations: 2000,
            exploration: 0.7,
            rollout_actions: 4,
            seed: 0,
            threads: 1,
            prior_temperature: 0.1,
            root_noise: None,
            virtual_loss: 1.0,
        }
    }
}

impl SearchConfig {
    /// Defaults for a learned search of `iterations` simulations: the PUCT
    /// constant and prior temperature next-1's MCTS training uses.
    pub fn learned(iterations: u32) -> Self {
        Self {
            iterations,
            exploration: 0.25,
            rollout_actions: 0,
            prior_temperature: 0.3,
            ..Self::default()
        }
    }

    fn validate(&self) -> Result<(), Error> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !self.exploration.is_finite() || self.exploration < 0.0 {
            return Err(Error::Unsupported(
                "search exploration must be finite and nonnegative",
            ));
        }
        if !self.virtual_loss.is_finite() || self.virtual_loss < 0.0 {
            return Err(Error::Unsupported(
                "search virtual loss must be finite and nonnegative",
            ));
        }
        if !positive(self.prior_temperature) {
            return Err(Error::Unsupported(
                "search prior temperature must be positive",
            ));
        }
        if let Some(noise) = self.root_noise
            && !(positive(noise.alpha) && (0.0..=1.0).contains(&noise.fraction))
        {
            return Err(Error::Unsupported("search root noise is out of range"));
        }
        Ok(())
    }
}

/// The searched root: the chosen step and per-move statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub step: Step,
    pub moves: Vec<RootMove>,
    /// Simulations run, over all trees.
    pub iterations: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RootMove {
    pub step: Step,
    /// The step's index among the root's legal steps: the decision's options
    /// or the turn's legal actions, which is also the observation's option
    /// order. Moves that look alike to the searcher (hidden standard
    /// achievements of one back) are one move at its first option.
    pub option: usize,
    pub visits: u32,
    /// Mean value for the searching player, in [-1, 1].
    pub value: f64,
    /// Learned only: the network's root prior, without noise (zero for
    /// heuristic search).
    pub prior: f64,
    /// Learned only: the network's root Q for this move; None when the
    /// network never scored it (heuristic search, or an unscored move).
    pub q: Option<f64>,
}

/// What the mover sees of a move. Hidden standard achievements share a key
/// across worlds by their public back and secrets by their public Safe slot;
/// everything else is the native move.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Key {
    Choice(Alternative),
    Action(Action),
    Achieve { value: u8, set: CardSet },
    Secret { slot: usize },
}

struct Edge {
    actor: Player,
    key: Key,
    child: Option<usize>,
    visits: u32,
    /// Sum of values for `actor`, the player moving from the parent node.
    value: f64,
    /// Visits of the parent in which this move was legal.
    available: u32,
    /// Learned: the softmax prior and the network's Q for this move, from
    /// the first evaluation that offered it.
    prior: f64,
    q: Option<f64>,
}

#[derive(Default)]
struct Node {
    edges: Vec<Edge>,
    /// Heuristic: rolled out once. Learned: the network scored it.
    evaluated: bool,
    /// Learned: a leaf at this node waits for the network.
    pending: bool,
    /// Learned: the evaluated mover and its best Q, the starting value of a
    /// move the network has not scored.
    value: Option<(Player, f64)>,
}

impl Node {
    fn edge(&mut self, actor: Player, key: Key) -> usize {
        if let Some(index) = self
            .edges
            .iter()
            .position(|edge| edge.actor == actor && edge.key == key)
        {
            return index;
        }
        self.edges.push(Edge {
            actor,
            key,
            child: None,
            visits: 0,
            value: 0.0,
            available: 0,
            prior: 0.0,
            q: None,
        });
        self.edges.len() - 1
    }
}

/// A world's next prompt, from its own perspective.
enum Prompt {
    Decision(Player, Vec<Alternative>),
    Turn(Player, Vec<Action>),
    /// Final value for Player One.
    Over(f64),
}

const WORK: usize = 1 << 16;
/// Automatic batches before a simulated world is scored as a draw. Real play
/// reports a stuck loop; search must still finish.
const MAX_ADVANCES: usize = 256;

fn settle(world: &mut Game) -> Prompt {
    for _ in 0..MAX_ADVANCES {
        match world.advance(WORK, &mut Vec::new()) {
            Ok(Status::Decision(decision)) => {
                return Prompt::Decision(decision.player, decision.options);
            }
            Ok(Status::Complete(_)) => {
                let actions = world.legal_actions();
                if actions.is_empty() {
                    return Prompt::Over(0.0);
                }
                return Prompt::Turn(world.position().turn_owner(), actions);
            }
            Ok(Status::Winner(player)) => {
                return Prompt::Over(if player == Player::One { 1.0 } else { -1.0 });
            }
            Ok(Status::Draw(_)) | Err(_) => return Prompt::Over(0.0),
            Ok(Status::WorkLimit) => {}
        }
    }
    Prompt::Over(0.0)
}

fn key(world: &Game, action: Action) -> Key {
    if let Action::Achieve { card } = action {
        match world.position().locate(card) {
            Some(Zone::AvailableAchievements) => {
                let card = world.position().card(card);
                return Key::Achieve {
                    value: card.value,
                    set: card.origin_deck().set,
                };
            }
            Some(safe @ Zone::Safe(_)) => {
                let slot = world
                    .position()
                    .cards(safe)
                    .iter()
                    .position(|&c| c == card)
                    .expect("located secret");
                return Key::Secret { slot };
            }
            _ => {}
        }
    }
    Key::Action(action)
}

/// Fitted logistic potential, as a value in [-1, 1] for
/// Player One. Signed powers keep negative Figures scores well defined.
fn evaluate(world: &Game) -> f64 {
    let position = world.position();
    let signed = |x: f64, exponent: f64| x.signum() * x.abs().powf(exponent);
    let diff = |f: &dyn Fn(Player) -> f64, exponent: f64| {
        signed(f(Player::One), exponent) - signed(f(Player::Two), exponent)
    };
    let icons = |player| {
        position
            .standard_icons()
            .map(|icon| position.icon_count(player, icon))
            .sum::<usize>() as f64
    };
    let logit = 0.053_110_708_1 * diff(&|p| position.score(p) as f64, 0.75)
        + 0.005_205_523_37 * diff(&|p| position.effective_achievement_count(p) as f64, 4.0)
        + 0.122_809_859_8 * diff(&|p| f64::from(position.highest_top_value(p)), 1.5)
        + 0.618_744_371_6 * diff(&icons, 0.5);
    (logit / 2.0).tanh()
}

fn below(rng: &mut Rng, n: usize) -> usize {
    (rng.next_u64() % n as u64) as usize
}

/// A uniform float in [0, 1).
fn unit(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A Gamma(alpha, 1) sample (Marsaglia and Tsang).
fn gamma(rng: &mut Rng, alpha: f64) -> f64 {
    if alpha < 1.0 {
        let u = unit(rng).max(f64::MIN_POSITIVE);
        return gamma(rng, alpha + 1.0) * u.powf(1.0 / alpha);
    }
    let d = alpha - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        let (u1, u2) = (unit(rng).max(f64::MIN_POSITIVE), unit(rng));
        let x = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
        let v = (1.0 + c * x).powi(3);
        if v <= 0.0 {
            continue;
        }
        let u = unit(rng).max(f64::MIN_POSITIVE);
        if u.ln() < 0.5 * x * x + d - d * v + d * v.ln() {
            return d * v;
        }
    }
}

/// Apply the move a key names in this world, if it is legal here.
fn apply(world: &mut Game, prompt: &Prompt, wanted: &Key) -> bool {
    match prompt {
        Prompt::Decision(_, options) => options
            .iter()
            .position(|option| matches!(wanted, Key::Choice(o) if o == option))
            .is_some_and(|index| world.choose(index).is_ok()),
        Prompt::Turn(_, actions) => actions
            .iter()
            .find(|&&action| key(world, action) == *wanted)
            .is_some_and(|&action| {
                world
                    .take_action(action, crate::ActionKind::Regular)
                    .is_ok()
            }),
        Prompt::Over(_) => false,
    }
}

/// One key per legal option, in option order (alike moves repeat a key).
fn option_keys(world: &Game, prompt: &Prompt) -> Vec<Key> {
    match prompt {
        Prompt::Decision(_, options) => options.iter().cloned().map(Key::Choice).collect(),
        Prompt::Turn(_, actions) => actions.iter().map(|&action| key(world, action)).collect(),
        Prompt::Over(_) => Vec::new(),
    }
}

/// The distinct keys, in first-option order.
fn keys(world: &Game, prompt: &Prompt) -> Vec<Key> {
    let mut keys: Vec<Key> = Vec::new();
    for key in option_keys(world, prompt) {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

fn mover(prompt: &Prompt) -> Option<Player> {
    match prompt {
        Prompt::Decision(player, _) | Prompt::Turn(player, _) => Some(*player),
        Prompt::Over(_) => None,
    }
}

/// `value` for Player One as seen by `player`.
fn signed(value: f64, player: Player) -> f64 {
    if player == Player::One { value } else { -value }
}

/// Play uniformly random moves for `actions` action boundaries, then score.
fn rollout(world: &mut Game, mut prompt: Prompt, actions: u32, rng: &mut Rng) -> f64 {
    let mut taken = 0;
    loop {
        match &prompt {
            Prompt::Over(value) => return *value,
            Prompt::Decision(_, options) => {
                if world.choose(below(rng, options.len())).is_err() {
                    return 0.0;
                }
            }
            Prompt::Turn(_, legal) => {
                if taken == actions {
                    return evaluate(world);
                }
                taken += 1;
                let action = legal[below(rng, legal.len())];
                if world
                    .take_action(action, crate::ActionKind::Regular)
                    .is_err()
                {
                    return 0.0;
                }
            }
        }
        prompt = settle(world);
    }
}

/// A leaf waiting for the network.
struct Leaf {
    path: Vec<(usize, usize)>,
    node: usize,
    mover: Player,
    /// The leaf node's edge for each observation option.
    edges: Vec<usize>,
    observation: Observation,
}

/// One resumable search tree over a fixed root.
pub struct Tree {
    root: Game,
    sampling: Option<SamplingPlan>,
    player: Player,
    learned: bool,
    config: SearchConfig,
    nodes: Vec<Node>,
    rng: Rng,
    /// Simulations begun and backed up.
    started: u32,
    finished: u32,
    leaves: Vec<Leaf>,
    /// Each simulation's depth and whether it answered an opponent prompt.
    depths: Vec<(u32, bool)>,
    /// Normalized Dirichlet samples for the root edges scored first.
    noise: Vec<f64>,
    /// The root has one move as the searcher sees it: no search needed.
    forced: bool,
    /// The root's legal steps and each one's key.
    root_steps: Vec<Step>,
    root_keys: Vec<Key>,
}

impl Tree {
    /// A tree for the current prompt's acting player. The game must be
    /// tracked from setup and idle at a decision or action boundary.
    /// Determinization refusals are seed-independent, so a refused root
    /// returns the error immediately; callers choose their own fallback.
    /// A `learned` tree asks for leaf evaluations ([`Tree::pending`]);
    /// otherwise leaves are rolled out and scored natively.
    pub fn new(game: &Game, config: SearchConfig, learned: bool) -> Result<Self, Error> {
        config.validate()?;
        let mut settled = game.clone();
        let prompt = settle(&mut settled);
        let Some(player) = mover(&prompt) else {
            return Err(Error::InvalidAction);
        };
        if settled != *game {
            return Err(Error::Busy);
        }
        let root_steps = match &prompt {
            Prompt::Decision(_, options) => (0..options.len()).map(Step::Choice).collect(),
            Prompt::Turn(_, actions) => actions.iter().copied().map(Step::Action).collect(),
            Prompt::Over(_) => unreachable!("a mover"),
        };
        let root_keys = option_keys(game, &prompt);
        let forced = root_keys.iter().all(|key| *key == root_keys[0]);
        let sampling = if forced {
            None
        } else {
            Some(game.prepare_sampling(player)?)
        };
        let tree = Self {
            root: game.clone(),
            sampling,
            player,
            learned,
            config,
            nodes: vec![Node {
                // A heuristic root needs no rollout of its own.
                evaluated: !learned,
                ..Node::default()
            }],
            rng: Rng::new(config.seed ^ 0x5EA2_C4ED_0000_0001),
            started: 0,
            finished: 0,
            leaves: Vec::new(),
            depths: Vec::new(),
            noise: Vec::new(),
            forced,
            root_steps,
            root_keys,
        };
        // A forced step needs no search (or a sample); fail fast on a
        // refused root otherwise.
        if !tree.forced {
            tree.world(config.seed)?;
        }
        Ok(tree)
    }

    /// Check a configuration without building a tree.
    pub fn check(config: &SearchConfig) -> Result<(), Error> {
        config.validate()
    }

    fn world(&self, seed: u64) -> Result<Game, Error> {
        let plan = self.sampling.as_ref().expect("non-forced search root");
        if self.learned {
            self.root.determinize_prepared(plan, seed)
        } else {
            Ok(self.root.sample_world_prepared(plan, seed)?.world)
        }
    }

    /// Per simulation so far: how many prompts its path answered below the
    /// root before it reached its leaf or a terminal, and whether one of them
    /// was the opponent's. A diagnostic of search depth.
    pub fn leaf_depths(&self) -> &[(u32, bool)] {
        &self.depths
    }

    fn record(&mut self, path: &[(usize, usize)]) {
        let crossed = path
            .iter()
            .any(|&(node, index)| self.nodes[node].edges[index].actor != self.player);
        self.depths.push((path.len() as u32, crossed));
    }

    /// Every simulation has been backed up.
    pub fn done(&self) -> bool {
        self.forced || self.finished == self.config.iterations
    }

    /// Run simulations until `limit` leaves wait for evaluation, a
    /// simulation selects a line through a node already waiting (the batch
    /// is then as wide as the tree can inform), or every simulation has
    /// begun. Heuristic trees never wait. Returns how many leaves wait.
    pub fn select(&mut self, limit: usize) -> Result<usize, Error> {
        while !self.forced
            && self.leaves.len() < limit.max(1)
            && self.started < self.config.iterations
        {
            if !self.simulate()? {
                debug_assert!(!self.leaves.is_empty(), "a pending node has a leaf");
                break;
            }
            self.started += 1;
        }
        Ok(self.leaves.len())
    }

    /// The waiting leaves' observations, in the order [`Tree::resolve`]
    /// expects their Q vectors.
    pub fn pending(&self) -> impl ExactSizeIterator<Item = &Observation> {
        self.leaves.iter().map(|leaf| &leaf.observation)
    }

    /// Back up one Q vector per waiting leaf (one Q per observation option).
    pub fn resolve<Q: AsRef<[f32]>>(&mut self, q: &[Q]) -> Result<(), Error> {
        self.resolve_leaves(q, None::<&[Q]>)
    }

    /// [`Tree::resolve`] with one vector of policy logits per waiting leaf,
    /// which set the leaves' priors in place of their Q.
    pub fn resolve_with_policy<Q: AsRef<[f32]>, P: AsRef<[f32]>>(
        &mut self,
        q: &[Q],
        policy: &[P],
    ) -> Result<(), Error> {
        self.resolve_leaves(q, Some(policy))
    }

    fn resolve_leaves<Q: AsRef<[f32]>, P: AsRef<[f32]>>(
        &mut self,
        q: &[Q],
        policy: Option<&[P]>,
    ) -> Result<(), Error> {
        if q.len() != self.leaves.len() || policy.is_some_and(|p| p.len() != q.len()) {
            return Err(Error::Unsupported("one Q vector per pending leaf"));
        }
        for (i, (leaf, q)) in std::mem::take(&mut self.leaves)
            .into_iter()
            .zip(q)
            .enumerate()
        {
            let q = q.as_ref();
            let logits = policy.map(|p| p[i].as_ref());
            if q.len() != leaf.edges.len()
                || q.iter().any(|v| !v.is_finite())
                || logits.is_some_and(|l| l.len() != q.len() || l.iter().any(|v| !v.is_finite()))
            {
                return Err(Error::Unsupported("one finite Q per leaf option"));
            }
            let value = self.score(&leaf, q, logits);
            for &(node, index) in &leaf.path {
                let edge = &mut self.nodes[node].edges[index];
                // Replace the virtual loss with the real value.
                edge.value += self.config.virtual_loss + signed(value, edge.actor);
            }
            self.finished += 1;
        }
        Ok(())
    }

    /// Record a leaf's priors (from the policy logits if given, else from Q)
    /// and Qs; its value for Player One.
    fn score(&mut self, leaf: &Leaf, q: &[f32], logits: Option<&[f32]>) -> f64 {
        let best = q
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(f64::from(b)));
        let temperature = self.config.prior_temperature;
        let scores = logits.unwrap_or(q);
        let top = scores
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(f64::from(b)));
        let weights: Vec<f64> = scores
            .iter()
            .map(|&v| ((f64::from(v) - top) / temperature).exp())
            .collect();
        let total: f64 = weights.iter().sum();
        let node = &mut self.nodes[leaf.node];
        // Alike options share a key: the key keeps the best Q and the summed
        // prior.
        let mut fresh = vec![false; node.edges.len()];
        for (option, &index) in leaf.edges.iter().enumerate() {
            let edge = &mut node.edges[index];
            let v = f64::from(q[option]).clamp(-1.0, 1.0);
            if edge.q.is_none() {
                fresh[index] = true;
                edge.q = Some(v);
                edge.prior = 0.0;
            }
            if fresh[index] {
                edge.q = Some(edge.q.expect("set").max(v));
                edge.prior += weights[option] / total;
            }
        }
        let value = best.clamp(-1.0, 1.0);
        node.pending = false;
        if !node.evaluated {
            node.evaluated = true;
            node.value = Some((leaf.mover, value));
            if leaf.node == 0
                && let Some(noise) = self.config.root_noise
            {
                let samples: Vec<f64> = (0..node.edges.len())
                    .map(|_| gamma(&mut self.rng, noise.alpha))
                    .collect();
                let sum: f64 = samples.iter().sum::<f64>().max(f64::MIN_POSITIVE);
                self.noise = samples.into_iter().map(|x| x / sum).collect();
            }
        }
        signed(value, leaf.mover)
    }

    fn backup(&mut self, path: &[(usize, usize)], value: f64) {
        self.record(path);
        for &(node, index) in path {
            let edge = &mut self.nodes[node].edges[index];
            edge.visits += 1;
            edge.value += signed(value, edge.actor);
        }
        self.finished += 1;
    }

    /// One simulation: back it up, or leave its leaf waiting. False when the
    /// line reached a node whose leaf still waits for the network; the
    /// simulation then leaves no trace and is not counted.
    fn simulate(&mut self) -> Result<bool, Error> {
        let seed = self.rng.next_u64();
        let mut world = self.world(seed)?;
        let mut prompt = settle(&mut world);
        let mut node = 0;
        let mut path: Vec<(usize, usize)> = Vec::new();
        // Moves counted available along the path, to take back on a discard.
        let mut offered: Vec<(usize, usize)> = Vec::new();
        loop {
            let Some(actor) = mover(&prompt) else {
                let Prompt::Over(value) = prompt else {
                    unreachable!()
                };
                self.backup(&path, value);
                return Ok(true);
            };
            if !self.nodes[node].evaluated {
                if !self.learned {
                    self.nodes[node].evaluated = true;
                    let value = rollout(
                        &mut world,
                        prompt,
                        self.config.rollout_actions,
                        &mut self.rng,
                    );
                    self.backup(&path, value);
                    return Ok(true);
                }
                if self.nodes[node].pending {
                    for (node, index) in offered {
                        self.nodes[node].edges[index].available -= 1;
                    }
                    return Ok(false);
                }
                self.wait(world, prompt, actor, node, path)?;
                return Ok(true);
            }
            let legal = keys(&world, &prompt);
            let count = legal.len();
            let candidates: Vec<usize> = legal
                .into_iter()
                .map(|key| {
                    let index = self.nodes[node].edge(actor, key);
                    self.nodes[node].edges[index].available += 1;
                    offered.push((node, index));
                    index
                })
                .collect();
            let index = if self.learned {
                self.puct(node, actor, &candidates, count)
            } else {
                self.ucb1(node, &candidates)
            };
            path.push((node, index));
            let key = self.nodes[node].edges[index].key.clone();
            if !apply(&mut world, &prompt, &key) {
                self.backup(&path, 0.0);
                return Ok(true);
            }
            prompt = settle(&mut world);
            node = match self.nodes[node].edges[index].child {
                Some(child) => child,
                None => {
                    let child = self.nodes.len();
                    self.nodes.push(Node::default());
                    self.nodes[node].edges[index].child = Some(child);
                    child
                }
            };
        }
    }

    /// Unvisited moves first (at random), then UCB1 over availability.
    fn ucb1(&mut self, node: usize, candidates: &[usize]) -> usize {
        let edges = &self.nodes[node].edges;
        let fresh: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&index| edges[index].visits == 0)
            .collect();
        if !fresh.is_empty() {
            return fresh[below(&mut self.rng, fresh.len())];
        }
        let c = self.config.exploration;
        let score = |e: &Edge| {
            e.value / f64::from(e.visits)
                + c * (f64::from(e.available).ln() / f64::from(e.visits)).sqrt()
        };
        *candidates
            .iter()
            .max_by(|&&a, &&b| score(&edges[a]).total_cmp(&score(&edges[b])))
            .expect("a legal move")
    }

    /// PUCT over availability: `Q + c · P · √available / (1 + visits)`. An
    /// unvisited move starts from its own Q, or the node's value if this
    /// world offers a move no evaluation scored (prior `1 / legal moves`).
    fn puct(&self, node: usize, actor: Player, candidates: &[usize], count: usize) -> usize {
        let noisy = |index: usize, prior: f64| match (node, self.config.root_noise) {
            (0, Some(noise)) if index < self.noise.len() => {
                (1.0 - noise.fraction) * prior + noise.fraction * self.noise[index]
            }
            _ => prior,
        };
        let node = &self.nodes[node];
        let fallback = match node.value {
            Some((player, value)) if player == actor => value,
            _ => 0.0,
        };
        let c = self.config.exploration;
        let score = |index: usize| {
            let e = &node.edges[index];
            let (q, prior) = match e.q {
                Some(q) => (q, noisy(index, e.prior)),
                None => (fallback, 1.0 / count as f64),
            };
            let mean = if e.visits == 0 {
                q
            } else {
                e.value / f64::from(e.visits)
            };
            mean + c * prior * f64::from(e.available).sqrt() / (1.0 + f64::from(e.visits))
        };
        *candidates
            .iter()
            .max_by(|&&a, &&b| score(a).total_cmp(&score(b)))
            .expect("a legal move")
    }

    /// Leave a learned leaf for the network, with a virtual loss on its path.
    fn wait(
        &mut self,
        world: Game,
        prompt: Prompt,
        mover: Player,
        node: usize,
        path: Vec<(usize, usize)>,
    ) -> Result<(), Error> {
        let observation = match world.observe(mover) {
            Ok(observation) => observation,
            Err(_) => {
                // Unobservable worlds should not occur; score them natively.
                let value = evaluate(&world);
                self.backup(&path, value);
                return Ok(());
            }
        };
        let edges = option_keys(&world, &prompt)
            .into_iter()
            .map(|key| self.nodes[node].edge(mover, key))
            .collect();
        for &(node, index) in &path {
            let edge = &mut self.nodes[node].edges[index];
            edge.visits += 1;
            edge.value -= self.config.virtual_loss;
        }
        self.record(&path);
        self.nodes[node].pending = true;
        self.leaves.push(Leaf {
            path,
            node,
            mover,
            edges,
            observation,
        });
        Ok(())
    }

    /// The root's statistics and its most-visited step.
    pub fn result(&self) -> Result<SearchResult, Error> {
        let mut moves = Vec::new();
        let root = &self.nodes[0];
        for (option, key) in self.root_keys.iter().enumerate() {
            if self.root_keys[..option].contains(key) {
                continue;
            }
            let edge = root
                .edges
                .iter()
                .find(|edge| edge.actor == self.player && edge.key == *key);
            let (visits, value) = edge.map_or((0, 0.0), |edge| {
                let value = if edge.visits == 0 {
                    0.0
                } else {
                    edge.value / f64::from(edge.visits)
                };
                (edge.visits, value)
            });
            moves.push(RootMove {
                step: self.root_steps[option],
                option,
                visits,
                value,
                prior: edge.map_or(0.0, |edge| edge.prior),
                q: edge.and_then(|edge| edge.q),
            });
        }
        if self.forced {
            moves.truncate(1);
        }
        Ok(SearchResult {
            step: best_step(&moves)?,
            moves,
            iterations: self.finished,
        })
    }
}

impl Game {
    /// Heuristic search of the current prompt for its acting player; see
    /// [`Tree::new`] for the root's requirements.
    pub fn search(&self, config: SearchConfig) -> Result<SearchResult, Error> {
        self.search_trees(config, false, config.iterations, |tree| {
            tree.select(1)?;
            Ok(())
        })
    }

    /// Learned search with a next-1 network evaluated on this thread (one
    /// call per leaf), its priors from the network's Q as training's search
    /// does. A policy output is not read: as a PUCT prior it was at best even
    /// with `softmax(Q)`; it is for move choice
    /// without search ([`Net::sample_policy`]). Each tree runs until
    /// `leaf_batch` leaves wait, as a training tree does, then scores them.
    /// `config.iterations` is the total over all trees.
    pub fn search_net(
        &self,
        net: &Net,
        config: SearchConfig,
        leaf_batch: usize,
    ) -> Result<SearchResult, Error> {
        let per_tree = config.iterations.div_ceil(config.threads.max(1));
        self.search_trees(config, true, per_tree, |tree| {
            if tree.select(leaf_batch)? == 0 {
                return Ok(());
            }
            let q: Vec<Vec<f32>> = tree
                .pending()
                .map(|observation| net.q(observation))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Error::Unsupported("network evaluation failed"))?;
            tree.resolve(&q)
        })
    }

    /// Run `config.threads` trees of `iterations` each to completion, `step`
    /// at a time, and merge their root statistics.
    fn search_trees(
        &self,
        config: SearchConfig,
        learned: bool,
        iterations: u32,
        step: impl Fn(&mut Tree) -> Result<(), Error> + Sync,
    ) -> Result<SearchResult, Error> {
        let threads = config.threads.max(1);
        let run = |thread: u64| {
            let config = SearchConfig {
                seed: config.seed ^ thread.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                iterations,
                threads: 1,
                ..config
            };
            let mut tree = Tree::new(self, config, learned)?;
            while !tree.done() {
                step(&mut tree)?;
            }
            tree.result()
        };
        if threads == 1 {
            return run(0);
        }
        let results: Vec<Result<SearchResult, Error>> = std::thread::scope(|scope| {
            let trees: Vec<_> = (0..u64::from(threads))
                .map(|thread| scope.spawn(move || run(thread)))
                .collect();
            trees
                .into_iter()
                .map(|tree| tree.join().expect("search thread"))
                .collect()
        });
        let mut merged: Vec<RootMove> = Vec::new();
        let mut total = 0;
        for result in results {
            let result = result?;
            total += result.iterations;
            for root in result.moves {
                match merged.iter_mut().find(|m| m.option == root.option) {
                    Some(m) => {
                        let visits = m.visits + root.visits;
                        if visits > 0 {
                            m.value = (m.value * f64::from(m.visits)
                                + root.value * f64::from(root.visits))
                                / f64::from(visits);
                        }
                        m.visits = visits;
                        m.prior += root.prior;
                    }
                    None => merged.push(root),
                }
            }
        }
        for m in &mut merged {
            m.prior /= f64::from(threads);
        }
        Ok(SearchResult {
            step: best_step(&merged)?,
            moves: merged,
            iterations: total,
        })
    }
}

/// Most visits, then best value, then highest prior (a learned search too
/// short to visit every move follows the network).
fn best_step(moves: &[RootMove]) -> Result<Step, Error> {
    moves
        .iter()
        .max_by(|a, b| {
            a.visits
                .cmp(&b.visits)
                .then(a.value.total_cmp(&b.value))
                .then(a.prior.total_cmp(&b.prior))
        })
        .map(|best| best.step)
        .ok_or(Error::InvalidAction)
}

#[cfg(test)]
mod tests;
