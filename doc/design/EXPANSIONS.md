# Expansions

How the engine implements each expansion. Base, Figures and Age 0 are
covered by the [architecture](ARCHITECTURE.md).

## Cities

- Card data: `data/cities.json`; metadata `engine/src/catalog/city_metadata.rs`
  is generated from it.
- Mechanics: `engine/src/execution/cities.rs` (City draws, event claims,
  Meld-action icons, Search, endorsement).
- Tests: `engine/tests/cities.rs`, `cities_meld.rs`, `cities_dogma.rs`,
  `cities_figures.rs`.

Cities have no dogma clauses: their icons are rule executions run by shared
programs and scheduler procedures, not effects, so effect hooks do not fire on
them. Slot geometry (six positions per City) is centralized in the metadata.
A splay's City draw is an immediate consequence of the operation, not a draw
replacement. Search reveals and returns without being an ordinary draw and is
exempt from karma. Flags and fountains are counted live, never claimed.
Endorsement is offered and charged once per original Dogma action.

## Echoes

- Card data: `data/echoes.json`.
- Programs and metadata: `engine/src/catalog/echoes.rs`, `echoes_metadata.rs`.
- Mechanics: `engine/src/execution/echoes.rs` (promotion, echo schedules,
  foreseen scope); retained slots and structured bodies use
  `execution/slots.rs` and `execution/blocks.rs`.
- Tests: `engine/tests/echoes_setup.rs`, `echoes_session.rs`,
  `echoes_combinations.rs`, `echoes_traces.rs`, `ported_echoes15.rs`,
  `ported_echoes611.rs`.

The Forecast is a capacity-limited hidden zone. Ordinary draws and explicitly
requested draws are distinct. A Dogma freezes the echo schedule of the visible
stack before it runs; echo effects are not dogma effects. Promotion keeps the
foreseen source identity through its free Dogma. Bonuses span 1–12.

## Artifacts

- Card data: `data/artifacts.json` (executable text and symbols).
- Programs and metadata: `engine/src/catalog/artifacts.rs`, `artifacts_metadata.rs`.
- Mechanics: `engine/src/execution/artifacts.rs`. Dig, seize, showcase,
  rotation and the Museum Check are scheduler procedures, not card
  instructions.
- Tests: `engine/tests/artifacts_foundation.rs`, `artifacts_procedures.rs`,
  `artifacts_cleanup.rs`, `artifacts_traces.rs`, `ported_artifacts.rs`.

Display and Museum storage are dedicated public zones, not logical hands or
boards; their icons do not count toward board queries. Museums are physical
cards with no card value; a Museum holding a junked Artifact is junked with
it. Compel is effect metadata, not an instruction. Battleship Yamato's
effective value differs from its immutable printed back.

### Martian Internet

#### Two behaviors, one card

Both run the same `Instruction::Subgame`; a `Game` flag picks the behavior:

- **Draw variant** (`martian-internet-draw-v1`, default): the effect ends the
  game in a draw. Training, search and every search simulation use it.
- **Printed rule** (`Game::with_subgames`): the parent game is suspended and a
  child game plays on the same decks; its winner's cards join the parent and
  the effect continues. The app uses it; duels opt in (`subgames=on` in
  `search_net_smoke`).

Search worlds are always cloned with the flag off, so the AI values the effect
as a draw and, inside a child, plays to win the child.

#### How a child runs

A child is a `Game` whose parent is frozen behind `Option<Arc<Parent>>`, so
nesting is a push and clones never copy the parent
(`engine/src/execution/subgame.rs`). On entry the supply moves into the
child, the child's trackers fork from the parent's, and each player draws and
melds a 9. When the child reaches a terminal, it merges instead: the winner's
cards transfer, everything else is junked, the parent resumes and its program
continues.

`Record` carries the flag (`rules printed-martian-internet`); child steps are
ordinary steps, so undo and replay work across the boundary. The app shows a
subgame banner, its own turn count, and the parked parent boards.

## Unseen

- Card data: `data/unseen.json` (executable text; Teleprompter uses the
  project's effect-based variant).
- Programs and metadata: `engine/src/catalog/unseen.rs`, `unseen_metadata.rs`.
- Mechanics: shared movement, copied execution, tracking and observation under
  `engine/src/execution/` and `engine/src/tracking/`; no Unseen-specific opcodes.
- Tests: `engine/tests/unseen_*.rs` (foundation, safe, draws, execution,
  histories, teleprompter, specials, traces, hard cases, one file per age) and
  `ported_cities_unseen.rs`.

The Safe is an ordered pile hidden from everyone, its owner included, with
capacity checked at commit. Secrets are proved by reveals. Hitchhiking's
control, Blackmail's may-to-must and inspection grants are scoped to one
execution and keep controller, executor and demander distinct. Teleprompter
continues on invocation activity, a channel separate from original-operation
success. Coin flips use native RNG. The first-draw counter spans the real
turn, free actions included.
