//! Native Innovation execution with a complete two-player Base setup API.
//!
//! Card effects are programs in a small instruction language ([`program`]),
//! interpreted resumably by [`Game`]. Optional [`Tracker`] beliefs support
//! hidden-world sampling ([`Game::determinize`]) and information-set search
//! ([`Game::search`], [`Game::search_net`], and [`SearchPool`] for training).
#![forbid(unsafe_code)]

mod achievements;
#[cfg(test)]
mod artifacts_tests;
mod catalog;
#[cfg(test)]
mod cities_tests;
mod decrees;
#[cfg(test)]
mod echoes_tests;
mod execution;
mod figure_names;
mod model;
pub mod net;
pub mod observation;
pub mod program;
pub mod reflex;
mod rng;
mod search;
mod search_pool;
pub mod session;
mod shortcuts;
mod tracking;

// Reuse the native acceptance fixtures in crate-private shortcut oracle tests.
#[cfg(test)]
extern crate self as inno_engine;

pub use achievements::*;
pub use catalog::*;
pub use decrees::*;
pub use execution::*;
pub use figure_names::*;
pub use model::*;
pub use net::Net;
pub use observation::Observation;
pub use program::*;
pub use reflex::{Batch, Finished, ReflexPool, Row, TRAINING_MODES, mode_expansions};
pub use rng::Rng;
pub use search::{RootMove, RootNoise, SearchConfig, SearchResult, Tree as SearchTree};
pub use search_pool::{LeafRow, PoolSearch, RootRow, SearchBatch, SearchPool};
pub use session::{Error as SessionError, Match, Outcome as MatchOutcome, Prompt, Record, Step};
pub use shortcuts::*;
pub use tracking::{BeliefSlot, Tracker};
