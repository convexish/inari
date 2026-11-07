//! Replayable two-player Base / Base + Figures matches. Matches track both
//! players' card beliefs from setup, so either seat can be searched.
//!
//! Event tags are one-based step numbers (`record.steps[tag - 1]`); zero
//! denotes setup. Events are internal engine facts, not viewer-safe messages.
//! Records store inputs, not snapshots; resuming requires compatible rules and
//! catalog ordering for decision options. Card references in text use codes.

use crate::{
    Action, ActionKind, CardId, Decision, DrawReason, Event, Expansions, Game, Net, Player, Rng,
    SearchConfig, Status, TRAINING_MODES,
};
use std::fmt;
use std::fmt::Write;

const WORK: usize = 4096;
const MAX_ADVANCES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Action(Action),
    Choice(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The deal's expansions: Base, Figures, Cities or Figures + Cities.
    pub expansions: Expansions,
    pub seed: u64,
    /// Martian Internet plays its printed subgame ([`Game::with_subgames`])
    /// instead of the draw variant.
    pub subgames: bool,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prompt {
    Decision(Decision),
    Turn {
        player: Player,
        actions: Vec<Action>,
    },
    Over(Outcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Winner {
        player: Player,
        victory: crate::Victory,
    },
    Draw(DrawReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Engine(crate::Error),
    InvalidStep,
    InvalidUndo,
    /// Automatic execution exceeded 4096 batches of 4096 work units between
    /// prompts; not a game outcome.
    WorkLimit,
    InvalidRecord {
        line: usize,
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => write!(f, "engine error: {error:?}"),
            Self::InvalidStep => f.write_str("step does not match the current prompt"),
            Self::InvalidUndo => f.write_str("undo exceeds the recorded step count"),
            Self::WorkLimit => f.write_str("automatic execution exceeded the match work cap"),
            Self::InvalidRecord { line, reason } => write!(f, "record line {line}: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<crate::Error> for Error {
    fn from(error: crate::Error) -> Self {
        Self::Engine(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    game: Game,
    record: Record,
    prompt: Prompt,
    events: Vec<(usize, Event)>,
}

fn deal(expansions: Expansions, seed: u64) -> Result<Game, Error> {
    Ok(Game::new_game(seed, expansions)?)
}

fn advance(game: &mut Game, events: &mut Vec<Event>) -> Result<Prompt, Error> {
    advance_with_budget(game, events, WORK, MAX_ADVANCES)
}

fn advance_with_budget(
    game: &mut Game,
    events: &mut Vec<Event>,
    work: usize,
    max_advances: usize,
) -> Result<Prompt, Error> {
    for _ in 0..max_advances {
        match game.advance(work, events)? {
            Status::Decision(decision) => return Ok(Prompt::Decision(decision)),
            Status::Complete(_) => {
                return Ok(Prompt::Turn {
                    player: game.position().turn_owner(),
                    actions: game.legal_actions(),
                });
            }
            Status::Winner(player) => {
                return Ok(Prompt::Over(Outcome::Winner {
                    player,
                    victory: game.victory().expect("terminal victory metadata"),
                }));
            }
            Status::Draw(reason) => return Ok(Prompt::Over(Outcome::Draw(reason))),
            Status::WorkLimit => {}
        }
    }
    Err(Error::WorkLimit)
}

impl Match {
    /// A match under the Martian Internet draw variant.
    pub fn new(expansions: Expansions, seed: u64) -> Result<Self, Error> {
        Self::with_subgames(expansions, seed, false)
    }

    /// With `subgames`, Martian Internet plays its printed subgame.
    pub fn with_subgames(expansions: Expansions, seed: u64, subgames: bool) -> Result<Self, Error> {
        // A match keeps a log, so it narrates turn and Dogma starts.
        let mut game = deal(expansions, seed)?
            .with_tracking()?
            .with_narration()
            .with_subgames(subgames);
        let mut events = Vec::new();
        let prompt = advance(&mut game, &mut events)?;
        Ok(Self {
            game,
            record: Record {
                expansions,
                seed,
                subgames,
                steps: Vec::new(),
            },
            prompt,
            events: events.into_iter().map(|event| (0, event)).collect(),
        })
    }

    pub fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn record(&self) -> &Record {
        &self.record
    }

    /// Setup is tagged 0; subsequent tags index the one-based input history.
    pub fn events(&self) -> &[(usize, Event)] {
        &self.events
    }

    pub fn to_move(&self) -> Option<Player> {
        match &self.prompt {
            Prompt::Decision(decision) => Some(decision.player),
            Prompt::Turn { player, .. } => Some(*player),
            Prompt::Over(_) => None,
        }
    }

    /// Apply one input atomically. All errors, including engine faults and the
    /// automatic-work cap, leave the game, prompt, record and events unchanged.
    pub fn apply(&mut self, step: Step) -> Result<(), Error> {
        let valid = match (&self.prompt, step) {
            (Prompt::Decision(decision), Step::Choice(index)) => index < decision.options.len(),
            (Prompt::Turn { actions, .. }, Step::Action(action)) => actions.contains(&action),
            _ => false,
        };
        if !valid {
            return Err(Error::InvalidStep);
        }
        let mut events = Vec::new();
        if let (Prompt::Decision(decision), Step::Choice(index)) = (&self.prompt, step)
            && let Some(event) = decision.announcement(index)
        {
            events.push(event);
        }
        // Clone only the engine, not the growing record/event history.
        let mut game = self.game.clone();
        match step {
            Step::Action(action) => game.take_action(action, ActionKind::Regular)?,
            Step::Choice(index) => game.choose(index)?,
        }
        let prompt = advance(&mut game, &mut events)?;
        self.game = game;
        self.prompt = prompt;
        self.record.steps.push(step);
        let tag = self.record.steps.len();
        self.events
            .extend(events.into_iter().map(|event| (tag, event)));
        Ok(())
    }

    pub fn from_record(record: &Record) -> Result<Self, Error> {
        let mut session = Self::with_subgames(record.expansions, record.seed, record.subgames)?;
        for &step in &record.steps {
            session.apply(step)?;
        }
        Ok(session)
    }

    /// Discard later inputs and their events by replaying from the original deal.
    pub fn undo_to(&mut self, steps: usize) -> Result<(), Error> {
        let prefix = self.record.steps.get(..steps).ok_or(Error::InvalidUndo)?;
        let record = Record {
            expansions: self.record.expansions,
            seed: self.record.seed,
            subgames: self.record.subgames,
            steps: prefix.to_vec(),
        };
        *self = Self::from_record(&record)?;
        Ok(())
    }

    /// Information-set search for the player to move ([`Game::search`]); it
    /// never consumes the game's RNG. A root the tracking contract refuses
    /// returns that error, so callers choose a fallback such as
    /// [`Match::random_step`].
    pub fn search_step(&self, config: SearchConfig) -> Result<Step, Error> {
        if matches!(self.prompt, Prompt::Over(_)) {
            return Err(Error::InvalidStep);
        }
        Ok(self.game.search(config)?.step)
    }

    /// The learned search's step with a next-1 network ([`Game::search_net`]);
    /// same contract as [`Match::search_step`].
    pub fn search_net_step(
        &self,
        net: &Net,
        config: SearchConfig,
        leaf_batch: usize,
    ) -> Result<Step, Error> {
        if matches!(self.prompt, Prompt::Over(_)) {
            return Err(Error::InvalidStep);
        }
        Ok(self.game.search_net(net, config, leaf_batch)?.step)
    }

    /// Uses a separate splitmix64 stream; never consumes the game's RNG.
    pub fn random_step(&self, seed: u64) -> Option<Step> {
        let count = match &self.prompt {
            Prompt::Decision(decision) => decision.options.len(),
            Prompt::Turn { actions, .. } => actions.len(),
            Prompt::Over(_) => return None,
        };
        let mut rng = Rng::new(seed);
        let bound = count as u64;
        // Reject the short residue classes rather than biasing with modulo.
        let threshold = bound.wrapping_neg() % bound;
        let index = loop {
            let value = rng.next_u64();
            if value >= threshold {
                break (value % bound) as usize;
            }
        };
        Some(match &self.prompt {
            Prompt::Decision(_) => Step::Choice(index),
            Prompt::Turn { actions, .. } => Step::Action(actions[index]),
            Prompt::Over(_) => unreachable!(),
        })
    }
}

fn record_error(line: usize, reason: impl Into<String>) -> Error {
    Error::InvalidRecord {
        line,
        reason: reason.into(),
    }
}

impl Record {
    /// Encode without replaying. Fails if a public, manually built record has
    /// a card identity outside its mode's catalog. Legality is checked by
    /// `Match::from_record`, not by the text codec.
    pub fn to_text(&self) -> Result<String, Error> {
        let game = deal(self.expansions, self.seed)?;
        // Records name deals as training modes do.
        let mode = TRAINING_MODES[self.expansions.index()];
        let mut text = format!("inari-next-record 1\nmode {mode}\nseed {}\n", self.seed);
        if self.subgames {
            text.push_str("martian-internet printed\n");
        }
        for (index, step) in self.steps.iter().enumerate() {
            let (name, card) = match step {
                Step::Choice(choice) => {
                    writeln!(text, "choice {choice}").expect("writing to String");
                    continue;
                }
                Step::Action(Action::Draw) => ("draw", None),
                Step::Action(Action::Decree) => ("decree", None),
                Step::Action(Action::Meld { card }) => ("meld", Some(card)),
                Step::Action(Action::Achieve { card }) => ("achieve", Some(card)),
                Step::Action(Action::Dogma { card, auspice }) => {
                    (if *auspice { "dogma-auspice" } else { "dogma" }, Some(card))
                }
            };
            text.push_str(name);
            if let Some(card) = card {
                let card = game
                    .position()
                    .catalog
                    .get(usize::from(card.0))
                    .ok_or_else(|| {
                        record_error(index + 4, "card identity is outside the mode's catalog")
                    })?;
                text.push(' ');
                text.push_str(card.code);
            }
            text.push('\n');
        }
        Ok(text)
    }

    /// Decode codes through the selected game's catalog, including dormant
    /// Age 0 cards in Figures mode. This checks syntax, not gameplay legality.
    pub fn from_text(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        if lines.next() != Some("inari-next-record 1") {
            return Err(record_error(1, "expected inari-next-record version 1"));
        }
        let expansions = lines
            .next()
            .and_then(|line| line.strip_prefix("mode "))
            .and_then(|name| TRAINING_MODES.iter().position(|mode| *mode == name))
            .and_then(|index| crate::mode_expansions(index as u8))
            .ok_or_else(|| {
                record_error(
                    2,
                    "expected a training mode name such as base or figures+echoes",
                )
            })?;
        let seed = lines
            .next()
            .and_then(|line| line.strip_prefix("seed "))
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| record_error(3, "expected seed N (u64)"))?;
        let game = deal(expansions, seed)?;
        let mut steps = Vec::new();
        let mut subgames = false;
        for (index, line) in lines.enumerate() {
            let number = index + 4;
            let fields: Vec<_> = line.split_whitespace().collect();
            let step = match fields.as_slice() {
                // Optional, before any step: Martian Internet's printed rule.
                ["martian-internet", "printed"] if steps.is_empty() && !subgames => {
                    subgames = true;
                    continue;
                }
                ["choice", value] => Step::Choice(
                    value
                        .parse()
                        .map_err(|_| record_error(number, "invalid choice index"))?,
                ),
                ["draw"] => Step::Action(Action::Draw),
                ["decree"] => Step::Action(Action::Decree),
                [
                    verb @ ("meld" | "achieve" | "dogma" | "dogma-auspice"),
                    code,
                ] => {
                    let id = game
                        .position()
                        .catalog
                        .iter()
                        .position(|card| card.code == *code)
                        .ok_or_else(|| record_error(number, format!("unknown card code {code}")))?;
                    let card = CardId(id as u16);
                    Step::Action(match *verb {
                        "meld" => Action::Meld { card },
                        "achieve" => Action::Achieve { card },
                        "dogma" => Action::Dogma {
                            card,
                            auspice: false,
                        },
                        "dogma-auspice" => Action::Dogma {
                            card,
                            auspice: true,
                        },
                        _ => unreachable!(),
                    })
                }
                _ => return Err(record_error(number, "unknown or malformed step")),
            };
            steps.push(step);
        }
        Ok(Self {
            expansions,
            seed,
            subgames,
            steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_limit_resumes_without_losing_events_or_changing_the_prompt() {
        let mut session = Match::new(Expansions::FIGURES, 7).unwrap();
        session.apply(Step::Choice(0)).unwrap();
        let mut game = session.game.clone();
        game.choose(0).unwrap();
        let mut expected_game = game.clone();
        let mut expected_events = Vec::new();
        let expected_prompt = advance(&mut expected_game, &mut expected_events).unwrap();
        let mut events = Vec::new();
        let prompt = advance_with_budget(&mut game, &mut events, 1, 1000).unwrap();
        assert_eq!(game, expected_game);
        assert_eq!(prompt, expected_prompt);
        assert_eq!(events, expected_events);
        assert!(!events.is_empty());
    }

    #[test]
    fn exhausted_work_cap_is_an_error_not_an_outcome() {
        let mut game = Game::new_base(0).unwrap();
        assert_eq!(
            advance_with_budget(&mut game, &mut Vec::new(), 1, 1),
            Err(Error::WorkLimit)
        );
        assert!(matches!(
            advance(&mut game, &mut Vec::new()).unwrap(),
            Prompt::Decision(_)
        ));
    }
}
