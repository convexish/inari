# Tracking and determinization

`engine/src/tracking.rs` and `engine/src/tracking/` keep each observer's card
beliefs plus a common (public) tracker; `Game::determinize` samples a hidden
world from them. Search and the [observation](OBSERVATION.md) both read them.

Tracking records **possibilities, not probabilities**. Samples are uniform
over the retained constraints, not a posterior over histories. Losing a
correlation is acceptable; gaining a private fact is not.

```rust
let mut game = Game::new_figures(seed)?.with_tracking()?;
let beliefs = game.tracker(viewer).unwrap().belief(Zone::Hand(viewer.opponent()));
let world = game.determinize(viewer, sampling_seed)?;
```

`with_tracking` must be called before opening play. Tracking lives on `Game`,
not `Position`, and is outside the mandatory-loop witness.

## Knowledge and sampling

- Each physical card has one slot per observer. Masks are local to a
  `(set, printed age)` back roster.
- Own hands and scores, public boards and inspection grants supply identity
  evidence. Losing permission does not erase memory.
- Reveals, ordered draws/returns, hidden departures, exchanges, named searches
  and shuffles update slots. Executing a card discloses it to both observers.
- All-different closure removes edges unsupported by any perfect matching
  (`tracking/matching.rs`).
- Sampling draws uniform perfect matchings of the slot constraints. Hidden
  unordered members are ordered by support, never by physical index, so two
  real games an observer cannot tell apart sample identically.
- The other observer is rebuilt from common history plus its sampled
  inspection rights; no real opponent-private history is copied.
- Sampling never mutates the real game or consumes its RNG; the sample's RNG
  continues a separate stream.

The public tracker learns a face only when both players can see it. Each
private tracker must refine it at every refresh.

## Retained references

A suspended effect names cards in registers, pending operands and results.
Each reference carries `Evidence`: which observers know its identity, and its
**pick**. A known identity is kept; otherwise the sample takes one of the
pick's candidate slots uniformly. Where the real card went never decides it,
and the real card's slot is always a candidate (checked in debug builds).

References copied from one another share a pick; separate picks may coincide
or not in a sample. An observer can name a reference when its evidence is
known or its tracker pins the pick to one known identity
(`Tracker::pick_identity`).

Sampling refuses, rather than guess, when a hidden reference has no pick, a
running program's hidden source is unpinned, a karma already read a hidden
operand's front, or a retained scalar or branch depends on private
information. In that last case the observation shows one canonical `Opaque`
continuation record. A refusal over hidden information usually means a card
failed to reveal something it should have.

## Tests

`tests/tracking.rs`, `tests/public_tracking.rs`, `tests/search_verification.rs`
and `src/execution/tracking/{tests,reference_tests}.rs`.
`examples/tracking_probe.rs` determinizes every root of seeded random games
and, with `play` and `nested`, plays and resamples each world, checking truth
and conservation:

```sh
pixi run cargo run --release --locked -p inno-engine \
  --example tracking_probe -- 300 0 full play
```
