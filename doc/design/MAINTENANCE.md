# Rules maintenance

Workflow for fixing rules in `engine/`. The engine and app support all 32
subsets of Figures, Echoes, Cities, Unseen and Artifacts with Base in
two-player games. [ARCHITECTURE.md](ARCHITECTURE.md) owns the
architectural contracts.

## Where things are

| Set | Reference | Catalog |
|---|---|---|
| Base / Figures / Age 0 | [Architecture](ARCHITECTURE.md) | `engine/src/catalog/{base,figures,age_zero}.rs` |
| Cities | [Cities](EXPANSIONS.md#cities) | `engine/src/catalog/city_metadata.rs` |
| Echoes | [Echoes](EXPANSIONS.md#echoes) | `engine/src/catalog/echoes.rs` |
| Artifacts | [Artifacts](EXPANSIONS.md#artifacts), [Martian Internet](EXPANSIONS.md#martian-internet) | `engine/src/catalog/artifacts.rs` |
| Unseen | [Unseen](EXPANSIONS.md#unseen) | `engine/src/catalog/unseen.rs` |

`data/` holds card text and symbols; the `*_metadata.rs` tables are generated
from it.

## Architecture rules

- **One runtime engine.** Expansion checks live at setup, catalog admission
  and shared query/scheduling boundaries. Disabled expansions must not alter
  Base deals, RNG use, menus or traces.
- **Declare cards once.** Clauses and programs live in the catalog. Reuse the
  language and operands; discuss new instruction kinds or native routines
  before adding them. Trigger discovery and shortcut dependencies derive from
  declarations.
- **Preserve operation boundaries.** Only the commit layer moves cards. Child
  reactions cannot supply success to their caller's "if you do". Controller,
  executor, demander and source identity survive nested execution.
- **Keep knowledge separate from storage.** A hidden reference must not reveal
  or pin an unknown card in menus, observations or determinization.
- **Cache only derived facts** and refresh them at mutation boundaries.
- **Fail explicitly.** Unsupported paths are faults. A loop draw needs an
  exact recurrence witness.

## Change workflow

1. Add a focused expected-outcome scenario that reproduces the defect.
2. Fix the shared boundary, checking other callers and expansions. Preserve
   stable IDs, replay/RNG and card conservation.
3. Review choices against the [shortcut checklist](SHORTCUTS.md#card-port-checklist).
   Reveal what proves a hidden pick; a sampling refusal usually means a missing reveal, not a sampler bug.
4. Check tracking, observation, search, replay and app presentation. An
   incompatible observation change needs a new revision.
5. Test affected expansion combinations with and without Figures, clone/resume
   at choices and tiny work budgets, and both observers' tracker invariants.
   Deliberate rules changes update fixtures after review, never just to pass.
   Finish with `pixi run check`.
