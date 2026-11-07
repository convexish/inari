# Observation and action contract `next-1`

Header 101, revision 7, implemented in `engine/src/observation.rs` for all 32
subsets. Any change bumps the
revision; networks reject rows with another header or revision.

## Shape

`Game::observe(viewer)` encodes a tracked game at a settled prompt as one
variable-length `f32` row: scalars, pooled card tokens and option rows:

| Part | Size | Contents |
|---|---:|---|
| Header | 8 | `100 + generation` (101), revision, scalar, token and option counts, viewer seat, enabled-set mask, achievement target |
| Scalars | 345 | global state, then "me" and "them" |
| Tokens | 5 each | card, group, weight, code, value |
| Options | 13 each | one per legal step, in the engine's order |

The row length includes every retained semantic record; context records are
not pooled. Every categorical field and token code is an integer below 2^24,
so the `f32` row carries it exactly.

**Cards** are `StableId + 1` (zero: no identity): the 705 reserved
identities of the [identity contract](ARCHITECTURE.md#identity-contract), so a
card has one embedding in every mode. **Players** are viewer-relative.

## Scalars

| Offset | Size | Contents |
|---:|---:|---|
| 0 | 6 | enabled sets (Base, Echoes, Cities, Unseen, Artifacts, Figures) |
| 6 | 1 | achievement target / 12 (`6 + enabled expansions`) |
| 7 | 5 | my turn, regular actions started / 2, action limit / 2, I am choosing a decision option, I am choosing an action |
| 12 | 1 | Age 0 introduced |
| 13 | 72 | cards per printed back (set × 12 ages) / 15 |
| 85 | 80 | twenty special-token slots × available, mine, theirs, junked |
| 165 | 20 | five Decrees × supply, mine, theirs, junked (Figures) |
| 185 | 2 | Cities: the turn's endorsement is used; the running Dogma action is endorsed |
| 187 | 2 × 79 | per player, below |

Per player: score / 50 (signed), bonus / 20, effective achievements / 12,
claimed achievements / 12, hand / 10, score pile / 20, highest top value /
11, visible cards / 20, cards tucked and scored this turn / 6, inspects the
other hand (Meiji); 12 icon counts (seven standard and Food, four reserved);
per color a splay one-hot (none, left, right, up, aslant), stack size / 10,
top value / 11 and top-is-a-Figure; Forecast capacity / 10 and remaining room /
10 (zero when Echoes is disabled); Safe capacity / 10, remaining room / 10 and
first-draw-taken (zero when Unseen is disabled), then 11 reserved
expansion-zone values. Counts
that tokens already carry (zone histograms, supply and junk) are omitted.

## Tokens

Twenty-two groups. Zone and option groups pool by identity; the final Context
group is an ordered record stream and must preserve its boundaries and order:

| Group | Tokens | Weight | Code / value |
|---|---|---|---|
| Decks | candidates of every deck slot, merged per card | expected count | depths from the top the card may occupy: bits 0–7, then 8+ |
| Junk, supply (standard achievements), Age 0 reserve | candidates | expected count | — |
| Board, mine and theirs | every board card | 1 | visibility bits for the card's printed positions (six on a City, four elsewhere), top/second/third, bottom/second/third; value = cards above / 5 |
| Hand, score, achievements, mine and theirs | candidates | expected count | — |
| Forecast, mine and theirs | viewer-tracker candidates | expected count | — |
| Safe, mine and theirs (Unseen) | viewer-tracker candidates of every Safe slot, merged per card | expected count | slots the card may occupy, oldest first: bits 0–7, then 8+; bit *i* is option slot *i* + 1 |
| Display, Museums, mine and theirs (Artifacts) | candidates | expected count | — |
| Options | cards the legal options name | 1 | class mask: bit 0 Meld, 1 Achieve, 2 Dogma, 3 Select/Reveal/BlindSelect, 4 Move/BlindMove, 5 MatchingPair, 6 SelfExecute/Showcase/Karma, 7 NameFigure |
| Context | ordered semantic records and their fields | 1 | header `256 + RecordKind`; fields `512 + 4 × Field + limb`, value = unsigned 16-bit limb |

Hidden zones come from the viewer's [tracker](TRACKING.md): a slot
spreads one card over its candidates, so per-back weight sums are exact zone
counts. The tracker keeps decks and Safes positional (a Safe's belief slots
are its physical arrival order, the order `Safe` options use), so their codes
are per-slot bits; other hidden piles are unordered and have code 0. Context never reads the actual identity of an anonymous retained pick.
A card record names its card only when retained evidence or that viewer's
filtered pick domain names it; otherwise it carries its printed back. Physical
card IDs, frame addresses, pick IDs and the other observer's private exclusions
are never encoded.

### Structured continuation records

[`observation/context.rs`](../../engine/src/observation/context.rs) fixes the
record-kind and field numbers. Each header begins a record; its one-based
ordinal is its observation-local ID. Header `card` is a stable identity or zero.
Following field tokens belong to that header until the next header. `Parent`
links scopes by record ID (zero means root). A hidden reference adds `Back` as
`Deck::index + 1`. Other metadata has `card=0`. Field values use one to four
little-endian 16-bit limbs, including one zero limb for zero; every integer is
represented exactly in `f32`, with no clamping of program counters or counts.

Records describe action, execution, effect and block scopes; executor,
controller, demander and original source owner; source/effect and cursor;
chosen values/colors/icons/cards; frozen echo schedules and their cursor;
promotion and foreseen state; pending operations, checked claims, batches and
exchanges; and named slots. Players and original-board owners are relative to
the viewer (none=0, mine=1, theirs=2). A slot includes lifetime, index, recording
player and source; its cards have separate oldest-first record ordinals and
retained original-board owners. Duplicate records remain duplicates. Execution
and action memories are sorted by stable source, player and slot, independently
of storage insertion order. In-flight `.keep`/`.add` records have a separate
`Pending` record and append/choice flags; the previous committed slot remains
present until the instruction commits.

Scalar-kind values are empty=0, card records=1, value=2, colors=3, icon=4,
Hidden=5 and literal count=6 (Mona Lisa's final-count commitment, a public
number of cards rather than a card value). Nonpublic scalar provenance covers undefined/empty results as well as
present values, so private producer presence does not reveal a value. Public
replacement clears the old provenance; a failed `.add` preserves it. Frozen
block domains and in-flight derived counts retain the same provenance.

If any retained scalar, pending progress or source identity lacks observer evidence, the whole retained continuation has one canonical `Opaque` record for that viewer. Private control flow also retains this restriction until its owning scopes retire.
This hides skipped slot producers, child counts and program positions together.
The restriction persists in the enclosing execution/action scope, including
through copied programs. Public board state and the viewer's native legal
options are still encoded normally. This is a conservative inference boundary:
sampling refuses an unknown historical scalar or private branch before RNG or
reference erasure, rather than importing a real private value into a sampled
world. Exact rules-only clones may erase this inference-only provenance after
copying the exact rules state. Scope retirement restores ordinary precise
continuation records.

## Options: the action contract

There is no fixed action vocabulary. Each legal step becomes one option row,
in the order the engine offers them, so a policy scores rows (a pointer
head) and the chosen index is the step: `Step::Choice(i)` at a decision,
the `i`th `legal_actions()` entry at an action boundary. Options appear only
when the viewer is the one choosing.

| Field | Contents |
|---:|---|
| 0 | kind: 1–5 actions (Draw, Meld, Achieve, Dogma, Decree), 6–28 decision alternatives, 29 showcase, 30 literal final count (Mona Lisa) |
| 1 | named card, if the viewer may know it; a named Figure (kind 19) is its card identity |
| 2 | viewer-relative zone (1–12; 13–14 Forecast, 15–16 display, 17–18 Museums, 19–20 Safe; mine then theirs); an exchange's left zone |
| 3 | printed back (`Deck::index + 1`) |
| 4 | number: value, age, branch index, splay direction, draw value or literal count |
| 5 | color mask |
| 6 | standard icon + 1; 9 = Bonus and equivalent nonfeatured icon-type no-op |
| 7 | verb + 1 |
| 8 | token: special 1–20 (index + 1) or Decree 21–25 (`21 + Decree`) |
| 9 | hidden slot + 1 (deck or Safe position), or the auspice flag of a Dogma action or Showcase |
| 10 | owner: 1 mine, 2 theirs |
| 11 | second card (matching pairs) |
| 12 | an exchange's right zone |

A face-down standard achievement is named by its back only, and a secret by
its Safe's zone, slot and back (the Safe is hidden from its owner too).
Every printed Safe selector is the executor's own, so zone 20 never names an
option.

### Network consumption

The first network reads the scalars, the pooled token groups, and a
projection of each context record: its kind, named card or back, parent
depth, and the semantic fields (Role, Executor, Controller, Demander,
SourceOwner, Owner, ScalarKind, Scalar, Count, Limit, Performed, Verb,
SourceZone, Destination, Effect, Known, Optional, Shared, Endorsed, Promoted,
Failed, Append, Choice, ActionKind, Value, Back). The engine-layout fields
(Cursor, Stage, Ordinal, Origin and the rest) are transported for the privacy
tests but ignored by the network, so program edits that move counters do not
shift its inputs.

## Privacy and tests

The encoder reads only public zones, the viewer's beliefs, its own legal
options and evidence-aware continuation references. `tests/observation.rs`
requires `observe(real, viewer) == observe(determinize(real, viewer), viewer)`
at roots of all 32 modes; the only accepted refusal is the explicit
private-context error. Mutations that encode the opponent's real hand or a
reference's real card must fail it. Other tests pin the row layout and each
expansion's groups.

## Notes

- `next-1` encodes current beliefs, not an event log.
- Inherited normalizations are part of the checkpoint contract.
- `determinize()` rebuilds the opponent's beliefs from common history plus its
  sampled inspection rights; earlier private observations are conservatively
  lost.
- Requested age/quota exactness stays private after a public partial result:
  public `Done` does not disclose the requested quota.
