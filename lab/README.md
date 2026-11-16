# Lab

Experiments with the engine and the shipped network: which openings win,
how strength settings compare, what the AI does in practice. Each experiment
is a binary in `src/bin/` and each finished study is a report in `reports/`
with the commands that reproduce it.

## Reports

| Date | Report | Question |
|---|---|---|
| 2026-11-15 | [Opening hands](reports/2026-11-15-base-opening-hands.md) | Which Base opening hands and starting melds win? |
| 2026-11-15 | [Hand vs. skill](reports/2026-11-15-hand-vs-skill-example.md) | Does the best hand against the worst beat a stronger player? |

## Running

```sh
pixi run lab <experiment> run [key=value ...] > lab/runs/<name>.csv
pixi run lab <experiment> summary lab/runs/<name>.csv
```

`pixi run lab` builds and runs `src/bin/<experiment>.rs` with the `lab`
profile (optimized, no LTO, so an engine edit rebuilds quickly). Every
binary's `//!` header lists its options and its output columns.

| Experiment | What it plays |
|---|---|
| `opening_hands` | Self-play from random Base deals with coin-flip starting melds |
| `hand_matchup` | Deals where one seat has a chosen good hand and the other a bad one, each played with both seatings, between two settings |
| `beta_duel` | Policy play at two inverse temperatures on duplicate deals, every expansion mode |
| `search_net_smoke` | Learned search against greedy play or another search setting |

## Conventions

- **Players are named by strength.** `elo:N` plays exactly what the app's
  strength setting plays at N ([`inno_engine::strength`](../engine/src/strength.rs):
  policy sampling up to 860, search above). `t:X` (policy at temperature X)
  and `sims:N` (search) name a setting directly. The Elo scale is the
  average over all 32 expansion modes; Base alone may differ.
- **Runs are deterministic.** Every game is seeded from its deal seed, and
  `run_ordered` prints results in job order, so the same command at the same
  revision produces the same data lines whatever `workers=` is (checked on
  one machine; floating-point differences between CPUs could still change a
  search's choice).
- **Raw output starts with provenance.** `#` lines give the experiment and
  its options, the `git describe` revision (`-dirty` for uncommitted code)
  and the weights file with its FNV-1a digest. A `columns` line follows.
  `summary` skips both.
- **Raw output is not committed.** `lab/runs/` is ignored; a report commits
  its summary tables and the commands, and the data can be regenerated.
- **Shared code lives in `src/lib.rs`** (`Ai`, `play`, `play_opening`,
  `run_ordered`, `header`, `Args`). A new experiment should not re-implement a
  game loop. `lab` is a workspace member, so `pixi run check` keeps every
  experiment compiling as the engine changes.

## Writing a report

Name it `reports/<date>-<slug>.md` and add a row to the table above. Keep it
to the question, the answer, the method, the results and the commands.

Report only numbers that tell the reader something:

- Leave out differences that are within their error, rather than labelling
  or caveating them.
- If a small model explains a large table (15 card values instead of 105
  hands), report the model.
- A weaker or noisier setting of the same player is not a second opinion;
  do not run it alongside the stronger one.
- The same goes for a tool's `summary` and the columns it records.
