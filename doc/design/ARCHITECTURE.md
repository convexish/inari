# Architecture

The engine's architectural contracts. The [crate README](../../engine/README.md)
maps the source; the [maintenance guide](MAINTENANCE.md) owns the change
workflow. Rules authority is the Ultimate v4.1 rulebook, the printed cards and
appendices.

## Execution contract

1. Card effects are programs in a small instruction language
   (`engine/src/program.rs`; overview in the
   [crate README](../../engine/README.md#the-card-language)). Every
   observable game operation has a suspension boundary. Simple programs keep
   a program counter and three registers; operation/batch continuations
   retain their own progress. Opt-in retained slots and structured bodies
   (`execution/slots.rs`, `execution/blocks.rs`) support expansion programs.
   Truly unique mechanics use a named routine behind `Instruction::Native`
   (Fusion, Reclamation, Space Traffic and Max Verstappen).
2. Effects request semantic operations. Only the commit layer mutates card
   locations. Action versus effect verbs, exchanges as units, proposed wins,
   draw routing, reveal boundaries and immediate consequences remain distinct.
3. A Would resolver selects one matching clause using the current player's
   choice. Its reaction owns a scope that suppresses When/Would discovery in
   its entire subtree: reaction effects never trigger other reactions, even
   when the source Figure leaves play. First resumes the original operation
   once; Instead returns an unfulfilled original result. A selected meld
   retains its physical identity and follows it from any current zone after
   First. There is no holding zone or implied reveal; a private continuation
   reference is not observer knowledge.
4. Results return to their actual parent continuation. Performed, replaced,
   prevented and inapplicable outcomes stay distinct, including admission
   failures in capacity-limited zones. Reaction events cannot satisfy the
   caller's conditional.
5. Physical storage is singular. Logical-zone views retain physical origins.
   Each modifiers inspect underlying state independently; no modifier reads
   another's additions. Viewing permission and remembered knowledge are
   separate from membership. Exchanges compute both groups before mutation
   and leave their intersection in place.
6. All immediate consequences finish before the next instruction: City splay
   draws and immediate specials, Meld-action ordering, fading and proposed
   terminal results. The Meld order runs
   fading/special/victory checks even after the melded top leaves; promotion
   rechecks that top after them.
7. Immutable catalogs and programs live outside cloned mutable state.
   Presentation history is external. A simulation retains exact
   continuations, RNG, scoped rule data and knowledge. No operation
   reconstructs its progress by replaying an earlier effect on a snapshot.
8. Every reducible choice phase names a generic reduction
   ([shortcuts](SHORTCUTS.md)), checked with its Figure guard at every
   choice.

One build serves every expansion combination: a game's `Expansions` (a
runtime rules profile on its position) switches rules on. Each board caches
its top Figures, the trigger kinds their Would clauses watch and its visible
symbol tally; the position keeps a card-to-zone index. `refresh_board`
refreshes them after a pile or splay changes and `refresh_boards` rebuilds
everything after bulk changes. The shared frame stays within 56 bytes on
64-bit targets (`common_frame_layout_budget`).

`advance(work_budget)` yields only a real decision, completion, proposed-win
resolution or a resumable work limit. Internal scheduler transitions are
never MCTS decision nodes. An unsupported path enters an explicit fault.

## Identity contract

Every card has an explicit `CardSet` and a public back (`Deck`). `StableId`
reserves 705 card identities (690 across six sets plus 15 Age 0 cards);
game-local `CardId`s index one game's catalog. Specials, Decrees and Museums
are physical cards with no card value. The tracker indexes 72 backs, and the
two-player victory target is `6 + enabled expansion count`. Observation
changes must reject incompatible checkpoints explicitly.

## Hard cases

Native scenario tests under `engine/tests/` (search for the Figure's name)
pin these architectural cases:

| Figure | What it exercises |
|---|---|
| F087 Mikhail Kalashnikov | Opponent meld suspends for another player's choice and foreign self-execution. |
| F015 Homer + B004 Agriculture | A replacement cannot satisfy "if you do"; a compound draw-and-score uses the original result. |
| F030 Shen Kuo + B002 Metalworking | Reveal is not movement; repetition can depend on the revealed icon. |
| F085 Duke Ellington + fading | Replace the requirement before choosing which Figure fades. |
| F017 Alexander the Great; F114 Emma Watson | When consequences finish before a committed meld returns. |
| F110 Bear Grylls + Age 0 | Global junking removes the running Figure and caller without erasing their continuations. |
| F002 Gilgamesh; F073 Maxwell; F071 Nobel | Each contributions do not recursively amplify each other. |
| F022 Archimedes | Boxed literals differ from computed ages, counts and whole-action modifiers. |
| F043 Johannes Kepler | Instead replaces an entire action. |
| F039 Michelangelo; F074 Emperor Meiji | Logical score-pile and hand membership with timed inspection. |
| F096 Jackie Chan | Victory is interruptible; scoring top Figures exposes further Figures. |
| F037 Yi Sun-sin | Replaces a transfer or an entire exchange as one operation. |
| F090 Hedy Lamarr | Modifies structured requirements and suspends claims for First splays. |
| F040 Galileo; F077 von Neumann | First can change a deck before the original compound instruction resumes. |

Cross-expansion probes: a City splay draw triggering a Figure reaction or
terminal result; Unseen controlled nested execution keeping controller,
executor and demander distinct; Echoes/Artifacts promotion and showcase scopes,
Museum operations, and card value versus printed back.
