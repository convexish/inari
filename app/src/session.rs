//! Desktop session: authoritative choices, asynchronous CPU AI, and autosaves.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, mpsc};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use inno_engine::{Expansions, Match, Net, Record, SearchConfig, Step};
use serde::{Deserialize, Serialize};

use super::faces::Set;
use super::next;
use super::strength;
use super::table::TableView;
use super::theme::Theme;

/// Setup, relative to the working directory. Windows starts in the exe's
/// folder and keeps it beside it.
pub const DATA_DIR: &str = if cfg!(windows) { "" } else { ".inari" };
/// Autosaved games, relative to the working directory.
pub const SAVES_DIR: &str = if cfg!(windows) { "saves" } else { ".inari" };

pub fn fresh_seed(previous: Option<u64>) -> u64 {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    if previous == Some(seed) {
        seed.wrapping_add(1)
    } else {
        seed
    }
}

pub fn parse_seed(text: &str) -> Result<u64, String> {
    text.trim()
        .parse()
        .map_err(|_| "Enter a seed from 0 to 18446744073709551615.".into())
}

pub const MAX_SIMS: usize = 1_000_000;

pub fn parse_budget(text: &str) -> Result<usize, String> {
    let sims = text
        .trim()
        .parse::<usize>()
        .map_err(|_| "Enter a whole number of simulations (0 means greedy).")?;
    if sims > MAX_SIMS {
        return Err("Choose between 0 and 1,000,000 simulations.".into());
    }
    Ok(sims)
}

/// A policy sampling temperature: 0 (the network's best option) up to
/// infinity (random play), as `parse_temperature` reads it.
pub fn parse_temperature(text: &str) -> Result<f64, String> {
    let text = text.trim();
    let temperature = match text.to_ascii_lowercase().as_str() {
        "inf" | "infinity" | "random" => f64::INFINITY,
        _ => text
            .parse::<f64>()
            .map_err(|_| "Enter a temperature of 0 or more (0 is greedy).")?,
    };
    if temperature.is_nan() || temperature < 0.0 {
        return Err("Enter a temperature of 0 or more (0 is greedy).".into());
    }
    Ok(temperature)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub resume: String,
    pub human: usize,
    pub sims: usize,
    /// A network with a policy output, at zero simulations: the temperature
    /// its moves are sampled at (0 is greedy, higher is easier,
    /// infinity is random).
    pub temperature: f64,
    /// The strength `sims` and `temperature` were set from
    /// ([`strength::setting`]); None when they were set by hand.
    #[serde(default)]
    pub elo: Option<u32>,
    pub echoes: bool,
    pub figures: bool,
    pub cities: bool,
    pub unseen: bool,
    pub artifacts: bool,
    pub two_player: bool,
    pub auto_switch: bool,
    pub theme: Theme,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            resume: String::new(),
            human: 0,
            sims: 0,
            temperature: 0.0,
            elo: Some(strength::GREEDY_ELO),
            echoes: true,
            figures: false,
            cities: false,
            unseen: false,
            artifacts: false,
            two_player: false,
            auto_switch: true,
            theme: Theme::default(),
        }
    }
}
impl Settings {
    pub fn load() -> Self {
        std::fs::read(Path::new(DATA_DIR).join("setup.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|s| s.human < 2 && s.sims <= MAX_SIMS)
            .map(|mut s| {
                if let Some(elo) = s.elo {
                    (s.sims, s.temperature) = strength::setting(elo);
                }
                s
            })
            .unwrap_or_default()
    }
    pub fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(DATA_DIR).map_err(|e| e.to_string())?;
        std::fs::write(
            Path::new(DATA_DIR).join("setup.json"),
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// The deal these settings ask for.
    pub fn expansions(&self) -> Expansions {
        let mut expansions = Expansions::BASE;
        if self.echoes {
            expansions = expansions.union(Expansions::ECHOES);
        }
        if self.figures {
            expansions = expansions.union(Expansions::FIGURES);
        }
        if self.cities {
            expansions = expansions.union(Expansions::CITIES);
        }
        if self.artifacts {
            expansions = expansions.union(Expansions::ARTIFACTS);
        }
        if self.unseen {
            expansions = expansions.union(Expansions::UNSEEN);
        }
        expansions
    }
}

pub const HELP: &str = "Inari — Innovation 4E desktop play\n\
Usage: inari [--human 1|2] [--elo N] [--mcts N] [--temperature T] [--seed N]\n\
             [--two-player|--vs-ai] [--autoswitch|--no-autoswitch] [--resume SAVE.txt]\n\
             [--echoes|--no-echoes] [--figures|--no-figures] [--cities|--no-cities]\n\
             [--unseen|--no-unseen] [--artifacts|--no-artifacts]\n\
             [--theme cloisonne|porcelain|washi|lapis] [--start]\n\
Without arguments, opens the setup screen. --two-player, --start or --resume starts a game.\n\
--theme picks the table's look and is remembered; setup can change it at any time.\n\
Two-player mode has no AI. Switch seats changes the view; autoswitch follows the next decision after shortcuts.\n\
The AI plays the built-in network: its policy output by default, sampled at --temperature T (0 is greedy, higher is easier, inf is random), or searching with the network for --mcts N simulations (1 to 1,000,000). --elo N sets both from one strength, 0 (random) to 1,410, averaged over the expansions: temperature 0 is 860, search plays above it.\n\
Expansion settings are remembered; --no-echoes selects a deal without Echoes. Martian Internet plays its printed subgame; during a subgame, Parent / P shows the suspended boards.\n\
Hover a Safe to inspect its backs; click an eligible secret or supply achievement to claim it.\n\
Every choice autosaves under .inari/ (saves\\ on Windows). Undo / Ctrl+Z restores your last choice.\n\
Click a glowing card to choose it; right-click a card to enlarge it, and click again to close it.\n\
Click the card count under a stack to see the whole stack. Escape opens setup; F11 toggles fullscreen.\n";

pub struct Options {
    pub settings: Settings,
    pub seed: Option<u64>,
    pub start: bool,
    pub resume: bool,
    pub screenshot: Option<String>,
}
impl Options {
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut o = Self {
            settings: Settings::load(),
            seed: None,
            start: false,
            resume: false,
            screenshot: None,
        };
        let mut args = args.peekable();
        let (mut elo, mut by_hand) = (None, false);
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                println!("{HELP}");
                return Ok(None);
            }
            let settings = &mut o.settings;
            let toggled = match arg.as_str() {
                "--two-player" => {
                    settings.two_player = true;
                    o.start = true;
                    true
                }
                "--vs-ai" => {
                    settings.two_player = false;
                    true
                }
                "--autoswitch" | "--no-autoswitch" => {
                    settings.auto_switch = arg == "--autoswitch";
                    true
                }
                "--artifacts" | "--no-artifacts" => {
                    settings.artifacts = arg == "--artifacts";
                    true
                }
                "--unseen" | "--no-unseen" => {
                    settings.unseen = arg == "--unseen";
                    true
                }
                "--cities" | "--no-cities" => {
                    settings.cities = arg == "--cities";
                    true
                }
                "--echoes" | "--no-echoes" => {
                    settings.echoes = arg == "--echoes";
                    true
                }
                "--figures" | "--no-figures" => {
                    settings.figures = arg == "--figures";
                    true
                }
                _ => false,
            };
            if toggled {
                continue;
            }
            if arg == "--start" {
                o.start = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("Missing value for {arg}"))?;
            match arg.as_str() {
                "--human" => {
                    o.settings.human = match value.as_str() {
                        "1" => 0,
                        "2" => 1,
                        _ => return Err("--human must be 1 or 2".into()),
                    };
                }
                "--mcts" => {
                    o.settings.sims = parse_budget(&value)?;
                    if o.settings.sims == 0 {
                        return Err("--mcts must be positive".into());
                    }
                    by_hand = true;
                }
                "--temperature" => {
                    o.settings.temperature = parse_temperature(&value)?;
                    by_hand = true;
                }
                "--elo" => elo = Some(value),
                "--seed" => o.seed = Some(parse_seed(&value)?),
                "--resume" => {
                    o.settings.resume = value;
                    o.start = true;
                    o.resume = true;
                }
                "--theme" => {
                    o.settings.theme = Theme::parse(&value).ok_or_else(|| {
                        "--theme must be cloisonne, porcelain, washi or lapis".to_string()
                    })?;
                }
                "--smoke-test" => {
                    o.screenshot = Some(value);
                    o.start = true;
                }
                _ => return Err(format!("Unknown argument {arg}\n{HELP}")),
            }
        }
        if o.resume && o.seed.is_some() {
            return Err("Use either --seed or --resume".into());
        }
        if by_hand {
            if elo.is_some() {
                return Err("Use either --elo or --mcts and --temperature".into());
            }
            o.settings.elo = None;
        } else if let Some(value) = elo {
            let elo = strength::parse_elo(&value)?.ok_or("--elo needs a number")?;
            o.settings.elo = Some(elo);
            (o.settings.sims, o.settings.temperature) = strength::setting(elo);
        }
        Ok(Some(o))
    }
}

/// Leaves a network search lets wait for the network at once. With
/// `SearchConfig::learned` and one tree, this is the MCTS training search.
const LEAF_BATCH: usize = 8;

/// A background AI search; its step is ignored once cancelled.
struct Worker {
    receiver: mpsc::Receiver<Step>,
    cancel: Arc<AtomicBool>,
}

/// The first line of a save.
const RECORD_HEADER: &str = "inari-next-record";

/// The network weights shipped inside the app.
const BUILTIN_WEIGHTS: &[u8] = include_bytes!("../../weights/inari_v1.0.bin");
/// The status line's name for the built-in network.
const BUILTIN_NAME: &str = "Inari v1.0";
/// The built-in network, parsed once.
static BUILTIN_NET: LazyLock<Arc<Net>> = LazyLock::new(|| {
    Arc::new(Net::from_bytes(BUILTIN_WEIGHTS).expect("the built-in network loads"))
});

/// One match against the network AI or a second human.
struct Play {
    m: Match,
    /// The seat that chose each recorded step, for undo.
    movers: Vec<usize>,
    worker: Option<Worker>,
    /// The AI seat's network; None in 2-player mode (tests drop it for an
    /// instant random AI).
    net: Option<Arc<Net>>,
}

impl Play {
    /// `subgames`: Martian Internet's printed rule rather than the draw variant.
    fn new(expansions: Expansions, seed: u64, subgames: bool) -> Result<Self, String> {
        Ok(Self {
            m: Match::with_subgames(expansions, seed, subgames).map_err(|e| e.to_string())?,
            movers: Vec::new(),
            worker: None,
            net: None,
        })
    }

    /// Gives the AI seat the built-in network.
    fn with_net(mut self, settings: &Settings) -> Self {
        if !settings.two_player {
            self.net = Some(BUILTIN_NET.clone());
        }
        self
    }

    /// What the status line names the AI.
    fn model_name(&self, settings: &Settings) -> String {
        if settings.two_player {
            "2-player".into()
        } else {
            BUILTIN_NAME.into()
        }
    }

    /// Replays a saved record, noting who chose each step.
    fn from_text(text: &str) -> Result<Self, String> {
        if !text.starts_with(RECORD_HEADER) {
            return Err("Not an Inari save.".into());
        }
        let record = Record::from_text(text).map_err(|e| e.to_string())?;
        let mut play = Self::new(record.expansions, record.seed, record.subgames)?;
        for &step in &record.steps {
            play.apply(step)?;
        }
        Ok(play)
    }

    fn apply(&mut self, step: Step) -> Result<(), String> {
        let mover = self.m.to_move().map(next::index);
        self.m.apply(step).map_err(|e| e.to_string())?;
        self.movers.push(mover.unwrap_or(0));
        Ok(())
    }
}

pub struct Game {
    play: Play,
    pub seed: u64,
    pub path: PathBuf,
    pub human: usize,
    pub two_player: bool,
    pub auto_switch: bool,
    pub sims: usize,
    /// Policy sampling temperature of a network played without search
    /// (`Settings::temperature`).
    pub temperature: f64,
    pub model_name: String,
    pub error: Option<String>,
    pub saved: bool,
    /// During a Martian Internet subgame, show the suspended parent's boards.
    pub peek: bool,
    /// What the table draws, rebuilt after every change of state or seat.
    pub view: TableView,
    /// Counts rebuilds, so drawing caches know when the view changed.
    pub version: u64,
}

/// A save path under the data directory that does not overwrite an existing game.
fn fresh_path(seed: u64) -> PathBuf {
    let mut path = Path::new(SAVES_DIR).join(format!("game-{seed}.txt"));
    let mut copy = 2;
    while path.exists() {
        path = Path::new(SAVES_DIR).join(format!("game-{seed}-{copy}.txt"));
        copy += 1;
    }
    path
}

impl Game {
    pub fn open(settings: &Settings, resume: bool, seed: Option<u64>) -> Result<Self, String> {
        let (play, seed, path) = if resume {
            let path = PathBuf::from(settings.resume.trim());
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let play = Play::from_text(&text)?.with_net(settings);
            let seed = play.m.record().seed;
            (play, seed, path)
        } else {
            let seed = seed.unwrap_or_else(|| fresh_seed(None));
            let play =
                Play::new(settings.expansions(), seed, settings.artifacts)?.with_net(settings);
            (play, seed, fresh_path(seed))
        };
        let model_name = play.model_name(settings);
        let mut g = Self {
            play,
            seed,
            path,
            human: settings.human,
            two_player: settings.two_player,
            auto_switch: settings.auto_switch,
            sims: settings.sims,
            temperature: settings.temperature,
            model_name,
            error: None,
            saved: false,
            peek: false,
            view: TableView::default(),
            version: 0,
        };
        g.follow_decision();
        g.save();
        g.refresh();
        Ok(g)
    }

    /// Rebuilds the table view after any change of state or seat.
    pub fn refresh(&mut self) {
        self.view = next::view(&self.play.m, self.human, self.two_player, self.peek);
        self.version += 1;
    }

    /// Toggle between a subgame's boards and its suspended parent's.
    pub fn toggle_peek(&mut self) {
        self.peek = !self.peek;
        self.refresh();
    }

    /// Choices made so far, for spotting a new decision.
    pub fn steps(&self) -> usize {
        self.play.m.record().steps.len()
    }

    /// Whether the game has ended.
    #[cfg(test)]
    pub fn is_over(&self) -> bool {
        self.to_move().is_none()
    }

    /// The seat that must choose next, if the game is not over.
    fn to_move(&self) -> Option<usize> {
        self.play.m.to_move().map(next::index)
    }

    /// Picks the pending decision's `index`-th option.
    pub fn choose_option(&mut self, index: usize) {
        if let Some(&step) = next::option_steps(&self.play.m).get(index)
            && self.human_turn()
        {
            self.apply(step);
        }
    }

    /// Applies one step, then saves and refreshes.
    fn apply(&mut self, step: Step) {
        if let Err(error) = self.play.apply(step) {
            self.error = Some(format!("Engine error: {error}"));
            return;
        }
        self.follow_decision();
        self.save();
        self.refresh();
    }

    pub fn random_ai(&self) -> bool {
        !self.two_player && self.play.net.is_none()
    }
    /// How the AI chooses, for the status line.
    pub fn ai_label(&self) -> String {
        match &self.play.net {
            None => "random".into(),
            // The network plays its policy (or greedy Q) at zero simulations
            // and searches with learned leaves otherwise.
            Some(net) if self.sims == 0 && net.has_policy() => {
                if self.temperature > 0.0 {
                    format!("policy network, temperature {}", self.temperature)
                } else {
                    "policy network".into()
                }
            }
            Some(_) if self.sims == 0 => "greedy network".into(),
            Some(_) => format!("network search {}", self.sims),
        }
    }
    /// What the AI is doing while it thinks.
    pub fn ai_activity(&self) -> &'static str {
        if self.random_ai() {
            "Choosing a random legal move."
        } else if self.play.net.is_some() && self.sims == 0 {
            "Choosing with the learned network."
        } else {
            "Searching possible continuations."
        }
    }
    /// Only called at a decision boundary, never every frame: manually
    /// inspecting the other seat must not immediately snap the view back.
    fn follow_decision(&mut self) {
        if self.two_player
            && self.auto_switch
            && let Some(player) = self.to_move()
        {
            self.human = player;
        }
    }
    pub fn switch_seats(&mut self) {
        if self.two_player {
            self.human = 1 - self.human;
            self.refresh();
        }
    }
    pub fn set_auto_switch(&mut self, enabled: bool) {
        self.auto_switch = enabled;
        if enabled {
            self.follow_decision();
            self.refresh();
        }
    }
    pub fn thinking(&self) -> bool {
        self.play.worker.is_some()
    }
    pub fn can_undo(&self) -> bool {
        self.undo_point().is_some()
    }
    /// The step to return to: the viewer's last choice, or anyone's in
    /// 2-player mode.
    fn undo_point(&self) -> Option<usize> {
        self.play
            .movers
            .iter()
            .rposition(|&mover| self.two_player || mover == self.human)
    }
    pub fn cancel_search(&self) {
        if let Some(worker) = &self.play.worker {
            worker.cancel.store(true, Ordering::Release);
        }
    }
    pub fn set_sims(&mut self, sims: usize) {
        if self.sims != sims {
            self.cancel_search();
            self.sims = sims;
        }
    }
    pub fn set_temperature(&mut self, temperature: f64) {
        if self.temperature != temperature {
            self.cancel_search();
            self.temperature = temperature;
        }
    }
    pub fn undo(&mut self) {
        self.cancel_search();
        let Some(point) = self.undo_point() else {
            return;
        };
        if let Err(error) = self.play.m.undo_to(point) {
            self.error = Some(format!("Undo failed: {error}"));
            return;
        }
        self.play.movers.truncate(point);
        self.follow_decision();
        self.save();
        self.refresh();
    }
    pub fn human_turn(&self) -> bool {
        !self.thinking() && self.error.is_none() && self.to_move() == Some(self.human)
    }
    pub fn save(&mut self) {
        let result = self
            .play
            .m
            .record()
            .to_text()
            .map_err(|e| e.to_string())
            .and_then(|text| write_text(&self.path, &text));
        match result {
            Ok(()) => {
                self.saved = true;
                self.error = None;
            }
            Err(err) => {
                self.saved = false;
                self.error = Some(format!(
                    "Autosave failed: {err}. Fix the path and click Retry."
                ));
            }
        }
    }
    /// A supply click claims an available achievement with this public back.
    pub fn achieve_standard(&mut self, age: u8, set: Set) {
        if self.human_turn()
            && let Some(step) = next::achieve_step(&self.play.m, age, set)
        {
            self.apply(step);
        }
    }
    /// A Safe click claims `player`'s secret at `position`.
    pub fn choose_secret(&mut self, player: usize, position: usize) {
        if self.human_turn()
            && let Some(step) = next::secret_step(&self.play.m, player, position)
        {
            self.apply(step);
        }
    }
    /// Called even while the menu is open: completed AI work is always
    /// applied and saved. New AI work starts only on the game screen, and
    /// never after a failure.
    pub fn tick(&mut self, active: bool) {
        let ai_to_move = active
            && !self.two_player
            && self.error.is_none()
            && self.to_move().is_some_and(|p| p != self.human);
        let seed = self.seed.wrapping_add(self.steps() as u64);
        if let Some(step) = self.ai_step(ai_to_move, seed) {
            self.apply(step);
        }
    }

    /// The AI's step to apply now, if any. The random AI is instant (one
    /// step per frame keeps its moves visible); the network (its policy or
    /// greedy option, or its search) runs on a worker thread and falls back
    /// to a random step at a root the tracking contract refuses.
    fn ai_step(&mut self, ai_to_move: bool, seed: u64) -> Option<Step> {
        let play = &mut self.play;
        if let Some(worker) = &play.worker {
            let cancelled = worker.cancel.load(Ordering::Acquire);
            return match worker.receiver.try_recv() {
                Ok(step) => {
                    play.worker = None;
                    (!cancelled && ai_to_move).then_some(step)
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    play.worker = None;
                    if !cancelled {
                        self.error = Some("AI worker stopped unexpectedly. Click Retry.".into());
                    }
                    None
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
        }
        if !ai_to_move {
            return None;
        }
        let Some(net) = play.net.clone() else {
            return play.m.random_step(seed);
        };
        let m = play.m.clone();
        let sims = u32::try_from(self.sims).unwrap_or(u32::MAX);
        let temperature = self.temperature;
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        match thread::Builder::new()
            .name("inari-ai".into())
            .spawn(move || {
                let step = if sims == 0 {
                    next::policy_step(&m, &net, temperature, seed)
                        .or_else(|| next::greedy_step(&m, &net))
                } else {
                    let config = SearchConfig {
                        seed,
                        ..SearchConfig::learned(sims)
                    };
                    m.search_net_step(&net, config, LEAF_BATCH)
                        .ok()
                        .or_else(|| next::greedy_step(&m, &net))
                };
                if let Some(step) = step.or_else(|| m.random_step(seed)) {
                    let _ = tx.send(step);
                }
            }) {
            Ok(_) => {
                play.worker = Some(Worker {
                    receiver: rx,
                    cancel,
                })
            }
            Err(err) => self.error = Some(format!("Could not start AI: {err}")),
        }
        None
    }
}

/// Writes a text save atomically: a temporary file, then a rename. Windows
/// refuses to replace a file another program holds open (OneDrive syncing
/// it, a virus scan), so the rename retries briefly and then falls back to
/// overwriting the save in place.
fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    for _ in 0..5 {
        if std::fs::rename(&tmp, path).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

impl Drop for Game {
    fn drop(&mut self) {
        self.cancel_search();
    }
}

#[cfg(test)]
mod tests {
    use super::super::faces::{backs, by_code};
    use super::*;
    /// Plays a game to the end: the human takes the first
    /// option, the random AI moves in `tick`.
    fn play_to_the_end(game: &mut Game, limit: usize) {
        for _ in 0..limit {
            if game.is_over() {
                return;
            }
            assert!(game.error.is_none(), "{:?}", game.error);
            if game.human_turn() {
                game.choose_option(0);
            } else {
                game.tick(true);
            }
        }
        panic!("game did not finish in {limit} steps");
    }

    #[test]
    fn games_finish_save_resume_and_undo() {
        for mode in 0..8 {
            let (echoes, figures, cities) = (mode & 1 != 0, mode & 2 != 0, mode & 4 != 0);
            let settings = Settings {
                echoes,
                figures,
                cities,
                ..Default::default()
            };
            let mut game = Game::open(&settings, false, Some(11)).unwrap();
            assert!(!game.random_ai());
            assert_eq!(game.model_name, BUILTIN_NAME);
            // The instant random AI keeps this whole-game test fast.
            game.play.net = None;
            assert!(game.random_ai());
            assert_eq!(game.view.figures, figures);
            assert_eq!(game.view.cities, cities);
            assert_eq!(game.view.echoes, echoes);
            // Advance into the game, then undo the human's last choice.
            while game.steps() < 12 {
                if game.human_turn() {
                    game.choose_option(0);
                } else {
                    game.tick(true);
                }
            }
            let resume = Settings {
                resume: game.path.to_string_lossy().into(),
                echoes: !echoes,
                figures: !figures,
                cities: !cities,
                ..settings.clone()
            };
            let resumed = Game::open(&resume, true, None).unwrap();
            assert_eq!(resumed.steps(), game.steps());
            assert_eq!(resumed.view, game.view);
            assert!(game.can_undo());
            let before = game.steps();
            game.undo();
            assert!(game.steps() < before);
            assert!(game.human_turn());
            play_to_the_end(&mut game, 5000);
            assert!(game.is_over());
            std::fs::remove_file(&game.path).unwrap();
        }
    }

    #[test]
    fn echoes_hotseat_switches_views_without_running_ai() {
        for mode in 0..4 {
            let settings = Settings {
                echoes: true,
                figures: mode & 1 != 0,
                cities: mode & 2 != 0,
                two_player: true,
                auto_switch: false,
                ..Default::default()
            };
            let mut game = Game::open(&settings, false, Some(17)).unwrap();
            game.choose_option(0);
            assert_eq!(game.to_move(), Some(1));
            assert_eq!(game.human, 0);
            let steps = game.steps();
            game.tick(true);
            game.choose_option(0);
            assert_eq!(game.steps(), steps);
            assert!(!game.thinking());
            game.switch_seats();
            assert_eq!(game.view.private_owner, 1);
            assert!(game.human_turn());
            game.set_auto_switch(true);
            for _ in 0..20 {
                if game.is_over() {
                    break;
                }
                assert!(game.human_turn());
                game.choose_option(0);
                assert!(game.error.is_none(), "{:?}", game.error);
            }
            let before = game.steps();
            game.undo();
            assert_eq!(game.steps(), before - 1);
            assert!(game.human_turn());
            assert!(game.view.echoes);
            std::fs::remove_file(&game.path).unwrap();
        }
    }

    #[test]
    fn network_ai_moves_on_its_worker_and_undo_cancels_it() {
        for mode in 0..4 {
            let settings = Settings {
                echoes: true,
                figures: mode & 1 != 0,
                cities: mode & 2 != 0,
                ..Default::default()
            };
            let mut game = Game::open(&settings, false, Some(7)).unwrap();
            let wait = |game: &mut Game| {
                for _ in 0..2000 {
                    game.tick(true);
                    assert!(game.error.is_none(), "{:?}", game.error);
                    if game.human_turn() || game.is_over() {
                        return;
                    }
                    thread::sleep(std::time::Duration::from_millis(1));
                }
                panic!("search AI did not finish its choice");
            };
            let mut ai_steps = 0;
            while ai_steps < 6 && !game.is_over() {
                wait(&mut game);
                let before = game.steps();
                game.choose_option(0);
                // The AI's search starts on the next tick and runs off-thread.
                game.tick(true);
                if !game.human_turn() && !game.is_over() {
                    assert!(game.thinking());
                    wait(&mut game);
                    ai_steps += 1;
                }
                assert!(game.steps() > before);
            }
            // Undo while the AI is searching: its late result is discarded.
            wait(&mut game);
            game.choose_option(0);
            game.tick(true);
            if game.thinking() {
                let steps = game.steps();
                game.undo();
                assert!(game.steps() < steps);
                wait(&mut game);
                assert!(game.error.is_none());
            }
            std::fs::remove_file(&game.path).unwrap();
        }
    }

    #[test]
    fn network_ai_plays_its_greedy_option() {
        use inno_engine::net::{Dimensions, synthetic_bytes};
        let tiny = Arc::new(Net::from_bytes(&synthetic_bytes(Dimensions::TINY, 3, 0.2)).unwrap());
        for (echoes, figures, sims) in [(false, false, 0), (true, true, 0), (true, false, 12)] {
            let settings = Settings {
                echoes,
                figures,
                ..Default::default()
            };
            let mut game = Game::open(&settings, false, Some(13)).unwrap();
            game.play.net = Some(tiny.clone());
            game.set_sims(sims);
            assert_eq!(game.model_name, BUILTIN_NAME);
            assert!(!game.random_ai() && !game.thinking());
            if sims == 0 {
                assert_eq!(game.ai_label(), "greedy network");
                assert_eq!(game.ai_activity(), "Choosing with the learned network.");
            } else {
                assert_eq!(game.ai_label(), format!("network search {sims}"));
                assert_eq!(game.ai_activity(), "Searching possible continuations.");
            }
            let wait = |game: &mut Game| {
                for _ in 0..2000 {
                    game.tick(true);
                    assert!(game.error.is_none(), "{:?}", game.error);
                    if game.human_turn() || game.is_over() {
                        return;
                    }
                    thread::sleep(std::time::Duration::from_millis(1));
                }
                panic!("network AI did not finish its choice");
            };
            let mut ai_steps = 0;
            while ai_steps < 8 && !game.is_over() {
                wait(&mut game);
                let before = game.steps();
                game.choose_option(0);
                game.tick(true);
                if !game.human_turn() && !game.is_over() {
                    assert!(game.thinking());
                    wait(&mut game);
                    ai_steps += 1;
                }
                assert!(game.steps() > before);
            }
            assert!(ai_steps > 0);
            // Resuming the save keeps the network.
            let resumed = Game::open(
                &Settings {
                    resume: game.path.to_string_lossy().into(),
                    ..settings.clone()
                },
                true,
                None,
            )
            .unwrap();
            assert_eq!(resumed.model_name, game.model_name);
            assert!(!resumed.random_ai());
            std::fs::remove_file(&game.path).unwrap();
        }
    }

    #[test]
    fn the_builtin_network_loads_with_a_policy() {
        assert!(BUILTIN_NET.has_policy());
    }

    /// Writes a save for screenshots: `INARI_MIDGAME_STEPS`
    /// random steps (default 120) of a seed-`INARI_MIDGAME_SEED` game, to
    /// `INARI_MIDGAME_OUT`, stopping at the human's (seat 0) next choice.
    /// `INARI_MIDGAME_CITIES=1` adds Cities to the default Figures deal.
    /// `INARI_MIDGAME_ECHOES=1` adds Echoes.
    #[test]
    #[ignore]
    fn midgame_save() {
        let Ok(output) = std::env::var("INARI_MIDGAME_OUT") else {
            return;
        };
        let var = |name: &str, default: u64| {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let mut expansions = if var("INARI_MIDGAME_CITIES", 0) == 1 {
            Expansions::FIGURES.union(Expansions::CITIES)
        } else {
            Expansions::FIGURES
        };
        if var("INARI_MIDGAME_ECHOES", 0) == 1 {
            expansions = expansions.union(Expansions::ECHOES);
        }
        let mut game = Play::new(expansions, var("INARI_MIDGAME_SEED", 5), false).unwrap();
        let steps = var("INARI_MIDGAME_STEPS", 120) as usize;
        while game.m.record().steps.len() < steps || game.m.to_move().map(next::index) != Some(0) {
            let seed = game.m.record().steps.len() as u64;
            let Some(step) = game.m.random_step(seed) else {
                break;
            };
            game.apply(step).unwrap();
        }
        write_text(Path::new(&output), &game.m.record().to_text().unwrap()).unwrap();
    }

    #[test]
    fn elo_sets_the_network_strength_unless_set_by_hand() {
        let parse = |args: &[&str]| Options::parse(args.iter().map(|a| a.to_string()));
        let o = parse(&["--elo", "1,250"]).unwrap().unwrap();
        assert_eq!((o.settings.elo, o.settings.sims), (Some(1250), 256));
        let o = parse(&["--elo", "400"]).unwrap().unwrap();
        assert_eq!(o.settings.sims, 0);
        assert!(o.settings.temperature > 1.0 && o.settings.temperature.is_finite());
        let o = parse(&["--mcts", "64"]).unwrap().unwrap();
        assert_eq!((o.settings.elo, o.settings.sims), (None, 64));
        assert!(parse(&["--elo", "900", "--mcts", "64"]).is_err());
        assert!(parse(&["--elo", "2000"]).is_err());
        // Settings saved before the strength setting keep their hand-set strength.
        let legacy: Settings = serde_json::from_str(r#"{"sims":256}"#).unwrap();
        assert_eq!((legacy.elo, legacy.sims), (None, 256));
        assert_eq!(Settings::default().elo, Some(strength::GREEDY_ELO));
    }

    #[test]
    fn seed_validation_and_legacy_settings() {
        for seed in [0, 42, u64::MAX] {
            assert_eq!(parse_seed(&format!(" {seed} ")).unwrap(), seed);
            assert_ne!(fresh_seed(Some(seed)), seed);
        }
        for value in ["", "-1", "3.5", "18446744073709551616"] {
            assert!(parse_seed(value).is_err());
        }
        let legacy: Settings = serde_json::from_str(r#"{"weights":"old.bin","sims":256}"#).unwrap();
        assert_eq!(legacy.sims, 256);
        assert!(legacy.echoes);
        assert!(!legacy.cities);
        assert!(!legacy.unseen);
        assert!(!legacy.two_player);
        assert!(legacy.auto_switch);
        assert_eq!(legacy.theme, Theme::Cloisonne);
        // An unknown theme keeps the other settings.
        let unknown: Settings = serde_json::from_str(r#"{"sims":64,"theme":"inari"}"#).unwrap();
        assert_eq!((unknown.sims, unknown.theme), (64, Theme::Cloisonne));
        for theme in Theme::ALL {
            let settings = Settings {
                theme,
                ..Default::default()
            };
            let decoded: Settings =
                serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(decoded.theme, theme);
        }
        assert_eq!(
            Options::parse(["--theme", "washi"].into_iter().map(str::to_owned))
                .unwrap()
                .unwrap()
                .settings
                .theme,
            Theme::Washi
        );
        for mode in 0..8 {
            let (echoes, cities, unseen) = (mode & 1 != 0, mode & 2 != 0, mode & 4 != 0);
            let options = Options::parse(
                [
                    if echoes { "--echoes" } else { "--no-echoes" },
                    if cities { "--cities" } else { "--no-cities" },
                    if unseen { "--unseen" } else { "--no-unseen" },
                    "--seed",
                    "42",
                ]
                .into_iter()
                .map(str::to_owned),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                (
                    options.settings.echoes,
                    options.settings.cities,
                    options.settings.unseen
                ),
                (echoes, cities, unseen)
            );
            assert_eq!(options.seed, Some(42));
        }
    }

    #[test]
    fn cli_selects_every_expansion_combination() {
        for mode in 0..32 {
            let options = Options::parse(
                [
                    if mode & 8 != 0 {
                        "--artifacts"
                    } else {
                        "--no-artifacts"
                    },
                    if mode & 16 != 0 {
                        "--unseen"
                    } else {
                        "--no-unseen"
                    },
                    if mode & 1 != 0 {
                        "--echoes"
                    } else {
                        "--no-echoes"
                    },
                    if mode & 2 != 0 {
                        "--figures"
                    } else {
                        "--no-figures"
                    },
                    if mode & 4 != 0 {
                        "--cities"
                    } else {
                        "--no-cities"
                    },
                    "--seed",
                    "42",
                ]
                .into_iter()
                .map(str::to_owned),
            )
            .unwrap()
            .unwrap();
            assert!(!options.start);
            assert_eq!(options.seed, Some(42));
            let expansions = options.settings.expansions();
            assert_eq!(expansions.echoes(), mode & 1 != 0);
            assert_eq!(expansions.figures(), mode & 2 != 0);
            assert_eq!(expansions.cities(), mode & 4 != 0);
            assert_eq!(expansions.artifacts(), mode & 8 != 0);
            assert_eq!(expansions.unseen(), mode & 16 != 0);
        }
    }

    #[test]
    fn large_search_budgets_round_trip_without_starting_search() {
        for sims in [0, 9000, 90000, 1_000_000] {
            assert_eq!(parse_budget(&sims.to_string()).unwrap(), sims);
            let settings = Settings {
                sims,
                ..Default::default()
            };
            let decoded: Settings =
                serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(decoded.sims, sims);
        }
        for text in ["", "-1", "1.5", "1000001", "9999999999999999999999999"] {
            assert!(parse_budget(text).is_err());
        }
        assert_eq!(
            Options::parse(["--mcts", "9000"].into_iter().map(str::to_owned))
                .unwrap()
                .unwrap()
                .settings
                .sims,
            9000
        );
    }
    #[test]
    fn private_backs_erase_identity_and_order() {
        let a = by_code("E001").unwrap();
        let b = by_code("E002").unwrap();
        let c = by_code("B001").unwrap();
        assert_eq!(backs(&[a, c]), backs(&[c, b]));
        assert_eq!(backs(&[a]), "E1");
    }
    #[test]
    fn rejects_invalid_launch_options() {
        for args in [
            vec!["--human", "0"],
            vec!["--mcts", "0"],
            vec!["--seed", "1", "--resume", "game.json"],
            vec!["--ai"],
            vec!["--theme", "inari"],
        ] {
            assert!(Options::parse(args.into_iter().map(str::to_owned)).is_err());
        }
    }
}
