# Engine

The dependency-free Rust crate that owns the rules, hidden-information
tracking, observations and search. `Game::new_game` deals any combination
of Echoes, Figures, Cities, Artifacts and Unseen with Base, requiring every
enabled set's authored catalog. Figures also adds the Decrees and the 15
dormant Age 0 cards. Initial hands and standard achievements come only from
Base; expansion decks shuffle afterward. `engine-py/` exposes the training
pools and native network to Python as `inno_engine`; `app/` is the desktop
table.

[ARCHITECTURE.md](../doc/design/ARCHITECTURE.md) owns architectural contracts and
regression probes; [SHORTCUTS.md](../doc/design/SHORTCUTS.md)
owns the choice reductions. [MAINTENANCE.md](../doc/design/MAINTENANCE.md)
is the workflow for rules fixes and links each expansion's reference.

```sh
pixi run check   # fmt, Clippy (warnings denied), all tests
```

## Layout

| Piece | Where |
|---|---|
| Card language: instructions, selectors, tests, expressions, conditions | `src/program.rs` |
| All 115 Base programs, with printed text | `src/catalog/base.rs` |
| The 15 Age 0 programs, with printed text | `src/catalog/age_zero.rs` |
| Figure declarations, with printed text | `src/catalog/figures.rs` |
| Interpreter: program steps, selection, expressions | `src/execution/interpreter.rs` |
| Operations, frames, turn structure | `src/execution.rs` |
| Karma discovery from declarations; native reactions | `src/execution/karma.rs` |
| Choice reductions and their Figure guards | `src/execution/shortcuts.rs`, `src/shortcuts.rs` |
| Setup, turns, legal actions | `src/execution/setup.rs`, `src/execution/turns.rs` |
| Fusion, Reclamation, Space Traffic | `src/execution/terminal_effects.rs` |
| Echoes programs and printed metadata | `src/catalog/echoes.rs`, `src/catalog/echoes_metadata.rs` |
| Echoes promotion, echo schedules and foreseen scope | `src/execution/echoes.rs` |
| Cities catalog (generated from `data/cities.json`) | `src/catalog/city_metadata.rs` |
| Cities rules: City draws, event claims, Meld-action icons, Search, endorsement | `src/execution/cities.rs` |

## The card language

A printed effect is a short list of instructions. Fifteen kinds cover Base
and Age 0:
`Move`, `Draw`, `Reveal`, `Splay`, `Exchange`, `Choose`, `Bind`, `Execute`,
`If`, `Jump`, `Repeat`, `Special`, `Win`, `EndAction` and `Native`. Shared
operands do the rest: a `Selector` names candidate cards and `Filter`s narrow
them with single-card `Test`s, an `Expr` computes a number, a `Cond` guards a
skip. Each effect has three registers: the previous instruction's result, a
bound card and a chosen value/icon/color set/card. Printed boxed values are
`Expr::Boxed`/`Test::BoxedValues` and follow the Archimedes/Kepler scope.

```rust
// You may return a card from your hand. If you do, draw and score a card of
// value one higher than the card you returned.
const AGRICULTURE: [Program; 1] = [&[
    return_(HAND).optional(),
    if_(DID, 2),
    draw_age(Add(&LAST_VALUE, 1)),
    score(LAST),
]];
```

A move takes one card, a batch (every card, exactly or up to a count, or one
per color, re-selected from live state after each card and its reactions) or
a committed pair. A move may carry one of six generic `Shortcut`s. Only
Fusion, Reclamation and Space Traffic are native code, as is Max
Verstappen's repeat of the effect his karma interrupts.

### Figures

Each Figure has one `FigureDef`: its Decree grants, the categories of its
Each modifiers (whose computations live with the queries they change) and
its karma clauses. A clause names the event it watches (`On`: a move verb, a
draw, a Dogma, a claim, a splay, ...), whose operation triggers it, an
optional `Cond`, and First, Instead or "when melded". Its body is an ordinary
program that starts with the event's card bound and a Dogma's featured icon
(or a splay's color) chosen; `Demander` names the triggering player. Hook
discovery, reaction kinds and shortcut dependencies all derive from the
declaration. All 115 Figures are declared; a few unique mechanics
(`KarmaNative`) are native. Sharing has its own events: ineligibility (after
the Dogma's First reactions) and drawing or not drawing for sharing (after
action end).

```rust
// If you would meld a card of value equal to your top red card, instead
// return it and draw and meld a card of value one higher.
instead(On::Move(&[Meld]), Executor, &[
    return_(THE),
    draw_age(Expr::Add(&Value(Reg::Bound), 1)),
    meld(LAST),
])
.when(&Compare(Value(Reg::Bound), Cmp::Equal, Max(&TopOf(Executor, Red))))
```

## Decisions and outcomes

`Decision::source_card(viewer)` identifies the running effect or karma's
source, including copied effects and sources that have since moved. Rule
choices (opening melds, fading) have no card source. `option_label(index,
viewer)` supplies branch wording; other options retain their structured
`Alternative` descriptors. Each `Choice::Branch` pairs its forward offsets
with explicit wording from the choosing player's perspective. Neither source
names nor identifying branch text are exposed to viewers who did not know
that source at capture time. Consumers must use these viewer-specific methods,
not debug output or an inferred "last Dogma".

`Choice::SetBranch` pairs the same branches with their origin sets and offers
only enabled sets. The parallel lists must have equal lengths; filtering
preserves branch identities and labels. Harrison uses it for his any-set draw,
keeping Base and Figures as the first two options when Cities is disabled.

`Game::victory()` reports the **committed** `WinReason`: achievements,
exhaustion (score, then achievements), card effect, karma, or elimination.
`Victory::source_card(viewer)` preserves the same source privacy. Win proposals
carry their reason through First/Instead resolution; a replacing karma's win
reports its own cause, not the interrupted proposal. `Status::Winner` retains
its gameplay-only player payload; the session's `MatchOutcome::Winner` includes
the victory metadata for presentation.

Presentation does not change option ordering or the text record format. The
frozen replay digest compares the pre-existing gameplay fields; presentation
has explicit scenario and app snapshot tests.

### Training fault diagnostics

The reflex and search pools log `WARNING engine_next_fault` before discarding
a faulted match, including an empty decision or turn prompt. The single log
line contains the seed, mode, completed-step count, error, current prompt,
last 16 events and the complete semantic replay as a quoted, escaped string.
`attempted_option` is present when an input failed atomically and therefore
is not yet in that replay; after an empty prompt, the triggering input is
already recorded. These internal diagnostics include hidden game information.

Faults still finish as capped draws, so the training collector also saves its
option-index history under `caps/`. Match the files and log entries by seed.
An `automatic_choice_cap` file alone does not distinguish a loop from a fault;
the fault warning does. Replays require the run's frozen engine source because
rules corrections can change option ordering.

## Play and benchmarks

`examples/random_games.rs` plays seeded uniformly random games through the
public API (`Game::new_base`, `legal_actions`, `take_action`,
`advance`, `choose`) and reports faults, outcomes and throughput:

```sh
pixi run cargo run --release --locked -p inno-engine \
  --example random_games -- 20000 0 check coverage
```

Flags: `check` verifies card conservation after every step, `clone` times
state clones, `figures` plays the Base deal under Figures rules, `full`
plays complete Base + Figures deals and reports Figure, karma, Decree and
Age 0 coverage, `cities` adds Cities and reports City melds, reveals,
endorsements and Cities tokens, `echoes` adds Echoes and reports Echoes melds,
Forecast arrivals, promotions, Echoes tokens and how many Echoes cards took a
Dogma action (these three combine freely), and `no-achieve` never
takes Achieve actions (so `figures` plays the same games as Base, isolating
the Figures code's cost). `Game::new_game(seed, expansions)` deals any
supported combination.
The retired engine's semantic traces (`tests/fixtures/`) replay through the
trace tests (`random_game_traces.rs`, `echoes_traces.rs`, `artifacts_traces.rs`,
`unseen_traces.rs`).

`tests/random_game_traces.rs` replays 300 seeded random games and compares a
digest of every offered option, choice, action and event with
`tests/fixtures/random_base_traces.txt`. An interpreter or language refactor
must reproduce it exactly; a deliberate rules change regenerates it with
`UPDATE_TRACES=1`.

## Tracking

`Game::with_tracking` keeps each observer's card beliefs and
`Game::determinize` samples a hidden world at a decision or action boundary;
`Game::public_tracker` retains common history so reconstructed opponents keep
public revelations alongside their sampled private cards.
[TRACKING.md](../doc/design/TRACKING.md) owns the
contract. `examples/tracking_probe.rs` determinizes every root of seeded
random games and plays each sample forward (`play`), checking beliefs; it
takes the same `full`, `cities` and `echoes` flags.

## Search

Single-observer information-set MCTS (`src/search.rs`): each simulation
determinizes the searching player's world and descends one shared tree.
Edges are keyed by the mover and what the mover sees, so a hidden standard
achievement is named by its back. Sampled worlds continue their own RNG
stream, never the real game's. `threads` searches independent trees in
parallel and merges their root statistics. A root that tracking refuses
returns the error, including the deliberate private-history refusal described
in the observation contract; callers choose a fallback.

- **Heuristic** (`Game::search`): untracked worlds, availability-corrected
  UCB1, a few random actions after expansion, then the fitted `logistic4`
  potential (score, achievements, highest top value, visible
  icons). `examples/search_smoke.rs` plays it against uniform random moves.
- **Learned** (`Game::search_net` with an exported `Net`): tracked worlds
  (`Game::determinize`), so each new leaf is observed by its own mover. One
  network call scores the leaf: its value is the best option Q, each option's
  prior is `softmax(Q / prior_temperature)`, and an unvisited edge starts from
  its own Q. Selection is PUCT over availability. The first simulation scores
  the root. `SearchConfig::learned` uses the training search's PUCT 0.25 and
  prior temperature 0.3, and `leaf_batch` lets leaves wait as a batched tree
  does. Optional Dirichlet root noise is for self-play.
  `examples/search_net_smoke.rs` plays it against the same network's greedy
  play on duplicate deals (`subgames=on` plays Martian Internet's printed
  subgame instead of the draw variant).
- **Batched** (`search::Tree`, `SearchPool`): a tree runs until up to
  `leaf_batch` leaves wait (with a virtual loss on their paths), then backs up
  the caller's Q vectors. `SearchPool` (Python `inno_engine.SearchPool`) runs
  many training games in worker threads; each step returns every waiting
  leaf observation and every searched root with per-option visits, values
  and priors in observation option order and whether tracking refused it,
  and takes Q vectors and root choices back. Results do not depend on the
  worker count. The trainer's search collector drives it.

Simulated tracked steps cost about 25 times untracked ones (about 50 µs per
step on 2026-10-11), and one CPU network call about 0.4 ms, so the learned
search runs roughly 1,000 simulations per second per thread.

The desktop app's opponent plays its built-in network's policy (or greedy Q)
at zero simulations and searches with it otherwise (`Match::search_net_step`):
one tree of the simulations setting with up to eight leaves waiting, as the
MCTS training searches, playing the most-visited move. It falls back to a
random step at a refused root.

The desktop setup offers every expansion in any combination, with two-player
play or the AI. For example:

```sh
pixi run play-app --echoes --figures --cities --vs-ai --start
```

Use `--no-echoes` for Base games without Echoes; expansion settings are
remembered. `pixi run play-app --help` lists every option.

## Card identity

Each `Card` names its `CardSet` (all six printed sets exist as names; Base,
Figures, Cities and Echoes have catalogs) and its back is its origin `Deck`. Hidden
descriptors and blind alternatives carry that back. A card has six printed
positions (`SLOTS`): Cities use all six, other sets the first four, and
`Splay::reveals` is the one covered-slot predicate. `StableId` maps
publisher codes into 705 reserved identities shared by every mode, while a
`CardId` indexes one game's catalog. Special achievements are a roster of
enabled tokens over 20 reserved slots.

## Observation

`Game::observe(viewer)` encodes the frozen contract
[`next-1`](../doc/design/OBSERVATION.md) (header 101, revision 7): scalars, belief-weighted
card tokens and one option row per legal step, laid out for all six sets.
Revision 5 fills Echoes Forecast ownership, foreign beliefs and structured retained context. It reads only the viewer's information; tests require a determinized world to encode identically for its sampling viewer across all eight Echoes/Figures/Cities combinations. Private historical scalar/control state is hidden and refuses sampling when it cannot be proved public; that conservative boundary is tested explicitly.
