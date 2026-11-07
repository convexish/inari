# Choice reductions (shortcuts)

Every reducible card phase carries a reduction; enabling Figures must not
disable them all.

## Contract

A shortcut removes choices, never semantic operations. The canonical sequence
still performs every move, draw, reaction, result update, information event,
RNG call and terminal check. Menus use only facts the chooser may see; blind
picks keep the native RNG primitive.

A batch move or block names one `Shortcut` kind (`engine/src/program.rs`);
its preconditions are checked from the move at every choice
(`engine/src/execution/shortcuts.rs`). Nothing is latched for the phase. The
test-only `shortcuts_disabled` oracle plays the unrestricted menus.

## Kinds

| Kind | Reduction |
|---|---|
| `LowestFirst` | Take the lowest card. A policy, not an equivalence (Oars). |
| `ByOrigin { descending, quota }` | Cards bound for different decks/piles commute: one origin group at a time, or with a quota a floor that reaches every subset by exactly one path. Same-deck return orders stay real. |
| `ByColor` | Board placements of different colours commute; same-colour orders stay real. |
| `DeckTop` | A whole deck to junk or a score pile: take the physical top. |
| `AnyOrder` | Every order is equivalent: take the first candidate, only while every remaining candidate must move. |
| `SharedPool` | A sharer's single junk from at most two achievements the owner junks next (Archery, Medicine). |

Batch menus list known cards in display order (`Game::display_key`), the same
order a floor measures, so a choice removes exactly the options above it.

Other rules: hidden sweeps that must move every matching card take physical
order (hands are revealed first, so they never reach this); moves into a Safe
keep their order; exchange cohorts and committed groups are sets; splays that
would all change nothing are not asked.

## Choice aliases

`.alias(..)` on a value or colour choice merges options that lead to the same
place: `Alias::NoOpWhen(cond)` keeps the first no-op option,
`Alias::SameKey(expr)` the first per key. The condition may read only what both
players see (card backs show values; colours of hidden cards are private);
the lint `alias_conditions_are_public` enforces this. Aliases never consult a
tracker, so tracked games and search worlds build the same menu.

## Figure guard

Each Figure's dependencies derive from its declaration
(`Figure::shortcut_dependencies`). A reduction is blocked when a top Figure
on either board, or one covered in a candidate's stack, has an intersecting
dependency. Inside suppressed karma, Would/When clauses cannot fire.

## Random-play oracle

`execution/batch_sweep_tests.rs` plays seeded random games in every mode and
compares each reduced batch with the unrestricted oracle over the rest of the
action (`SWEEP_GAMES` raises the count). The ignored diagnostic
`batch_menus_without_shortcuts_in_random_play` lists unannotated batches that
still asked for an order.

## Card-port checklist

1. Pick the kind whose equivalence (or policy) holds for the whole phase, or
   an alias over a public condition.
2. Cover activation and a blocking Figure (unrelated, covered, inactive).
3. Compare against the unrestricted oracle.
4. Check privacy and clone/resume at partial progress.
5. Witness the decision-count reduction.
