use crate::*;

mod action_returns;
mod artifacts;
mod batch;
mod blocks;
mod cities;
mod copied;
mod decrees;
#[cfg(test)]
mod deepfake_tests;
mod demand_results;
mod echoes;
mod exchange;
mod groups;
mod interpreter;
mod karma;
#[cfg(test)]
mod named_choice_tests;
mod observe;
mod pair;
mod references;
mod scalar_knowledge;
mod setup;
mod shortcuts;
mod slots;
mod splays;
mod subgame;
mod terminal_effects;
mod tracking;
pub(crate) use tracking::SamplingPlan;
mod turns;
mod verification;

use copied::CopiedExecution;
use decrees::{BlindReturn, DecreeProgress};
use exchange::ExchangeSelection;
use groups::GroupSelection;
use interpreter::{Excluded, Regs};
pub(crate) use observe::{PromptView, Sight};
use pair::MatchingPair;
use references::{
    Captured, Evidence, HeldBulk, HeldCard, HeldOutcome, HeldReturn, contains_card, find_card,
};
use setup::Opening;
use std::sync::Arc;
use subgame::{Parent, SubgameStep};
use terminal_effects::TerminalEffect;
use verification::JunkAvailable;

/// Which requirements a standard-card claim must recheck after First.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimEligibility {
    /// A printed claim that explicitly ignores eligibility.
    Ignore,
    /// An ordinary achievement action also requires live supply membership.
    Available,
    /// A printed eligible claim checks score and top value for its authorized
    /// source. Claim-only points are added after this score multiplier.
    Cost { score_factor: u8 },
}

/// Independent from the retained physical identity, so naming a color/icon
/// never destroys a preceding card reference or its observer provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SelectionParameter {
    None,
    Value(u8),
    /// A literal final count ([`Choice::FinalCount`]), not a card value.
    Count(u16),
    Colors(u8),
    Icon(Icon),
    Card(HeldCard),
}

impl SelectionParameter {
    pub(super) fn value(self) -> Option<u8> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }
    pub(super) fn colors(self) -> Option<u8> {
        match self {
            Self::Colors(colors) => Some(colors),
            _ => None,
        }
    }
    pub(super) fn icon(self) -> Option<Icon> {
        match self {
            Self::Icon(icon) => Some(icon),
            _ => None,
        }
    }
}

/// A running program's identity: what it runs and who performs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RevealScope {
    source: Source,
    executor: Player,
}

/// What a program frame runs: a card's effect, or a Figure's karma clause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Effect {
        card: CardId,
        effect: u16,
    },
    Karma {
        card: CardId,
        clause: u8,
    },
    /// A City icon's shared rule program: never a card effect.
    CityIcon {
        card: CardId,
        icon: CityIcon,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    /// Performs the program's instructions ("you"); in a demand, its victim.
    pub executor: Player,
    pub controller: Player,
    /// Says "my" in a demand and executes it for effect-level karma. Distinct
    /// from the victim performing its instructions and making its choices.
    pub demander: Player,
    /// Physical source owner is retained even for foreign self-execution.
    pub source_owner: Player,
    pub source_card: Option<CardId>,
    /// Capture-time identity evidence, independent of subsequent source moves.
    source_evidence: Evidence,
    /// Whole-action values inherited by each invocation, separately from its
    /// Archimedes adjustment. This occupies existing Context padding.
    boxed_action: BoxedValues,
    /// Scoped execution modifiers (Unseen), packed to keep Frame small:
    /// [`Self::MUST`] (Blackmail: "may" is "must" here and in nested
    /// executions), [`Self::CONTROLLED`] (Hitchhiking: `controller` is an
    /// explicit override that nested executions and assigned choices keep)
    /// and [`Self::INSPECT`] (the controller may look at what the executor
    /// may while this execution runs).
    scope: u8,
}

impl Context {
    pub fn new(executor: Player) -> Self {
        Self {
            executor,
            controller: executor,
            demander: executor,
            source_owner: executor,
            source_card: None,
            source_evidence: Evidence::NONE,
            boxed_action: BoxedValues::default(),
            scope: 0,
        }
    }

    const MUST: u8 = 1;
    const CONTROLLED: u8 = 2;
    const INSPECT: u8 = 4;
    /// Inside a karma body: its operations trigger no further karma.
    pub(crate) const SUPPRESS: u8 = 8;

    pub(crate) fn suppress_karma(self) -> bool {
        self.scope & Self::SUPPRESS != 0
    }

    pub(crate) fn set_suppress_karma(&mut self, suppress: bool) {
        self.scope = (self.scope & !Self::SUPPRESS) | if suppress { Self::SUPPRESS } else { 0 };
    }

    pub(crate) fn must(self) -> bool {
        self.scope & Self::MUST != 0
    }

    pub(crate) fn controlled(self) -> bool {
        self.scope & Self::CONTROLLED != 0
    }

    pub(crate) fn inspect(self) -> bool {
        self.scope & Self::INSPECT != 0
    }

    pub(crate) fn set_must(&mut self) {
        self.scope |= Self::MUST;
    }

    pub(crate) fn set_controlled(&mut self) {
        self.scope |= Self::CONTROLLED;
    }

    pub(crate) fn set_inspect(&mut self, inspect: bool) {
        self.scope = (self.scope & !Self::INSPECT) | if inspect { Self::INSPECT } else { 0 };
    }

    /// The player who decides a choice the text assigns to `named`.
    pub(crate) fn chooser(self, named: Player) -> Player {
        if self.controlled() {
            self.controller
        } else {
            named
        }
    }

    /// Native action-end rules change executor/controller, but inherit the
    /// action's suppression and boxed-value lifetime (not its source card).
    fn rules_for(self, player: Player) -> Self {
        Self {
            scope: self.scope & Self::SUPPRESS,
            boxed_action: self.boxed_action,
            ..Self::new(player)
        }
    }

    fn reaction(owner: Player, source: CardId) -> Self {
        Self {
            source_card: Some(source),
            source_evidence: Evidence::PUBLIC,
            scope: Context::SUPPRESS,
            ..Self::new(owner)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    SelfExecute,
    SuperExecute,
    /// One non-demand dogma effect by index among the card's non-demand
    /// dogma effects, excluding echoes (Teleprompter). An absent effect runs
    /// nothing. Not a self-execution: no Chain.
    Effect(u8),
}

impl ExecutionMode {
    /// A dense code for observations.
    pub(crate) fn code(self) -> u64 {
        match self {
            Self::SelfExecute => 0,
            Self::SuperExecute => 1,
            Self::Effect(index) => 2 + u64::from(index),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// One card-effect invocation, not a Dogma action or a self-execution loop.
    ExecuteEffect {
        card: CardId,
        effect: u16,
        boxed: BoxedValues,
    },
    /// Issue an already-authorized Decree; this is not a Decree action payment.
    IssueDecree {
        decree: Decree,
    },
    /// A Museum Check awards this player one of their occupied Museums.
    ClaimMuseum {
        player: Player,
    },
    ClaimDecree {
        decree: Decree,
    },
    ReturnDecree {
        decree: Decree,
    },
    /// The already-selected action body, after the optional Auspice decision.
    /// Locks sharing/vulnerability after its First reaction, before effects.
    Dogma {
        card: CardId,
        icon: Icon,
    },
    Move {
        verb: Verb,
        target: Target,
        destination: Zone,
    },
    /// `set`: an explicit or chosen expansion set, never redirected.
    /// `None` is an ordinary draw, which expansion rules may redirect (no
    /// card names the Base deck).
    Draw {
        value: u8,
        set: Option<CardSet>,
    },
    /// A Draw action: a container whose "would take a Draw action" karma resolve before
    /// the draw inside it is determined and discovered as a separate event.
    DrawAction {
        set: CardSet,
    },
    /// Atomically introduce and shuffle the dormant Age 0 roster once.
    IntroduceAgeZero,
    Shuffle {
        deck: Deck,
    },
    Reveal {
        target: Target,
    },
    Splay {
        player: Player,
        color: Color,
        direction: Splay,
    },
    /// Select both logical piles together, then move their non-overlap.
    /// This prototype supports hands and score piles, not subset exchanges.
    ExchangeAll {
        left: LogicalZone,
        right: LogicalZone,
    },
    /// Claim a selected card. Ordinary actions require available membership;
    /// printed effects either check their stated cost or explicitly bypass it.
    /// Neither implies an action boundary.
    Claim {
        target: Target,
        eligibility: ClaimEligibility,
    },
    ClaimSpecial {
        special: Special,
        check_eligibility: bool,
    },
    JunkAvailableSpecial {
        special: Special,
    },
    RestoreSpecial {
        special: Special,
    },
    /// Explicit action-end probe; callers must finish fading and specials first.
    CheckAchievementVictory,
    ProposeDraw {
        reason: DrawReason,
    },
    /// A fading requirement is resolved before choosing which Figure to score.
    RequireFade {
        player: Player,
    },
    ProposeWin {
        winner: Player,
        reason: WinReason,
    },
    /// The player loses: elimination, not an opponent's proposed win.
    Lose {
        player: Player,
    },
    /// A fair coin (Quantum Computers): performed when won.
    Flip,
    /// The player would not be eligible to share this Dogma; Replaced makes
    /// them eligible.
    Ineligible {
        player: Player,
    },
    /// After a Dogma action, the executor draws for sharing if `earned`.
    /// Both outcomes are events a karma can precede.
    ShareBonus {
        card: CardId,
        icon: Icon,
        earned: bool,
    },
    /// Draw the top card of exactly this deck (a revealed card).
    DrawTop {
        deck: Deck,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Performed,
    Replaced,
    Inapplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationOutcome {
    pub disposition: Disposition,
    pub card: Option<CardId>,
    pub value: Option<u8>,
    /// A draw that took a card above its requested value (an empty age was skipped):
    /// performed, but it fails a value-specific "if you do".
    pub short: bool,
}

impl OperationOutcome {
    fn empty(disposition: Disposition) -> Self {
        Self {
            disposition,
            card: None,
            value: None,
            short: false,
        }
    }
}

/// Counts only the original attempted movements of a sequential instruction.
/// An identity is considered once, so each count is bounded by the catalog's
/// u16 domain. Reaction descendants cannot contribute to these counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BulkOutcome {
    pub performed: u16,
    pub replaced: u16,
    /// Inapplicable originals, and performed ones that did not fully
    /// happen (a draw above its requested value).
    pub inapplicable: u16,
    /// Printed values of successful originals only, including Age 0.
    pub distinct_values: u16,
    /// The latest successful original's card: "it" after exactly one.
    pub card: Option<CardId>,
    /// That card's value before it moved.
    pub value: Option<u8>,
}

impl BulkOutcome {
    fn record(&mut self, disposition: Disposition) {
        let count = match disposition {
            Disposition::Performed => &mut self.performed,
            Disposition::Replaced => &mut self.replaced,
            Disposition::Inapplicable => &mut self.inapplicable,
        };
        *count += 1;
    }

    fn record_original(&mut self, outcome: OperationOutcome) {
        self.record(outcome.disposition);
        if outcome.short {
            self.inapplicable += 1;
        }
        if outcome.disposition == Disposition::Performed {
            self.card = outcome.card;
            self.value = outcome.value;
            if let Some(value) = outcome.value {
                self.distinct_values |= 1_u16 << value;
            }
        }
    }

    fn fulfilled(self) -> bool {
        self.performed > 0 && self.replaced == 0 && self.inapplicable == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnValue {
    Unit,
    Operation(OperationOutcome),
    Bulk(BulkOutcome),
    Dogma { shared: bool },
    DrawColors { colors: u8, repeated: bool },
}

impl ReturnValue {
    fn operation(self) -> Option<OperationOutcome> {
        match self {
            Self::Operation(outcome) => Some(outcome),
            _ => None,
        }
    }

    fn fulfilled(self) -> bool {
        match self {
            Self::Operation(outcome) => {
                outcome.disposition == Disposition::Performed && !outcome.short
            }
            Self::Bulk(outcome) => outcome.fulfilled(),
            _ => false,
        }
    }

    fn performed_count(self) -> u16 {
        match self {
            Self::Operation(outcome) => {
                u16::from(outcome.disposition == Disposition::Performed && outcome.card.is_some())
            }
            Self::Bulk(outcome) => outcome.performed,
            _ => 0,
        }
    }

    fn distinct_value_count(self) -> u16 {
        match self {
            Self::Operation(outcome) => {
                u16::from(outcome.disposition == Disposition::Performed && outcome.value.is_some())
            }
            Self::Bulk(outcome) => outcome.distinct_values.count_ones() as u16,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alternative {
    Value(u8),
    /// A literal final count of cards (Mona Lisa), not a card value.
    Count(u16),
    Icon(Icon),
    /// Bonus, and equivalent types which cannot be a featured icon.
    BonusIcon,
    /// An unordered mask of printed colors, including absent board colors.
    Colors(u8),
    /// Ordered, public-board cohort selected before either operation starts.
    MatchingPair {
        first: TargetDescriptor,
        second: TargetDescriptor,
    },
    Branch(usize),
    BlindSelect {
        source: Zone,
        back: Deck,
    },
    Decree(Decree),
    Draw {
        value: u8,
        set: CardSet,
    },
    Select {
        target: TargetDescriptor,
    },
    Splay {
        player: Player,
        color: Color,
        direction: Splay,
    },
    Exchange {
        left: LogicalZone,
        right: LogicalZone,
    },
    /// One public-back group; native RNG selects a hidden physical member.
    BlindMove {
        verb: Verb,
        source: Zone,
        back: Deck,
    },
    Deck(Deck),
    NameFigure(&'static FigureName),
    /// Name an intentionally unsuccessful Figure; still shuffle the chosen deck.
    Pass,
    /// Decline an optional instruction, not a failed named search.
    Decline,
    JunkSpecial(Special),
    RestoreSpecial(Special),
    ClaimSpecial(Special),
    Move {
        verb: Verb,
        target: TargetDescriptor,
    },
    Reveal {
        target: TargetDescriptor,
    },
    SelfExecute(CardId),
    /// Take the showcase's free Dogma action with the displayed Artifact.
    Showcase {
        card: CardId,
        auspice: bool,
    },
    Karma {
        source: CardId,
        figure: Figure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub player: Player,
    pub options: Vec<Alternative>,
    source: Option<HeldCard>,
    /// The source card's effect whose program is asking, when it is one.
    effect: Option<u16>,
    branches: &'static [(u8, &'static str)],
}

impl Decision {
    /// A public snapshot: which viewers know the source, not its pick.
    fn new(
        player: Player,
        options: Vec<Alternative>,
        source: Option<HeldCard>,
        effect: Option<u16>,
        branches: &'static [(u8, &'static str)],
    ) -> Self {
        Self {
            player,
            options,
            source: source
                .map(|source| Captured::new(source.inner, source.evidence.with_pick(None))),
            effect,
            branches,
        }
    }

    /// The printed effect index ([`Card::ECHO`] for an echo) of the source
    /// card's program that is asking, for viewers who know the source. A
    /// karma, rules procedure or City icon program has none.
    pub fn effect(&self, viewer: Player) -> Option<u16> {
        self.source_card(viewer).and(self.effect)
    }

    /// The actual effect/karma source, only when its identity is known to this
    /// viewer. Capture-time knowledge survives source movement and copying.
    pub fn source_card(&self, viewer: Player) -> Option<CardId> {
        self.source
            .filter(|source| source.evidence.known(viewer))
            .map(|source| source.inner)
    }

    /// Wording for a branch option; other alternatives describe themselves.
    /// Exact effect wording can identify a private source, so it has the same
    /// viewer restriction as the source name.
    /// The table announcement of choosing this option, if it is a public
    /// choice ([`Event::Chose`]). A branch is announced only with wording
    /// both players can read.
    pub fn announcement(&self, option: usize) -> Option<Event> {
        let choice = self.options.get(option)?.clone();
        let label = match choice {
            Alternative::Value(_)
            | Alternative::Count(_)
            | Alternative::Icon(_)
            | Alternative::BonusIcon
            | Alternative::Colors(_)
            | Alternative::Deck(_)
            | Alternative::NameFigure(_) => None,
            Alternative::Branch(_) => {
                let label = self.option_label(option, self.player)?;
                if self.option_label(option, self.player.opponent()) != Some(label) {
                    return None;
                }
                Some(label)
            }
            _ => return None,
        };
        Some(Event::Chose {
            player: self.player,
            choice,
            label,
        })
    }

    pub fn option_label(&self, option: usize, viewer: Player) -> Option<&'static str> {
        if self.source.is_some() && self.source_card(viewer).is_none() {
            return None;
        }
        let Alternative::Branch(branch) = self.options.get(option)? else {
            return None;
        };
        self.branches.get(*branch).map(|&(_, label)| label)
    }
}

/// Keep presentation outside the common frame's inline storage budget.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DecisionOptions {
    alternatives: Vec<Alternative>,
    source: Option<HeldCard>,
    branches: &'static [(u8, &'static str)],
    /// Equivalent options were merged (an [`Alias`] or a no-op splay).
    aliased: bool,
}

impl DecisionOptions {
    fn new(alternatives: Vec<Alternative>, context: Context) -> Box<Self> {
        Box::new(Self {
            alternatives,
            source: context
                .source_card
                .map(|card| Captured::new(card, context.source_evidence)),
            branches: &[],
            aliased: false,
        })
    }
}

/// Viewer-specific information about an already selected meld. Hidden fronts do not
/// disclose their destination color, current physical zone, or deck position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardView {
    Known(CardId),
    /// Only the public back: printed set and age.
    Back(Deck),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingMeld {
    pub player: Player,
    pub card: CardView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OperandKnowledge {
    evidence: Evidence,
    /// Discovery already evaluated a predicate, including an unoffered clause.
    conditioned: bool,
    /// A resolved conditional proof may depend on other private cards too.
    private_query: Option<Player>,
    /// This selected move must reveal its face before discovering reactions.
    verify: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawReason {
    Exhaustion,
    Effect,
    /// A mandatory loop repeated a state or exhausted automatic work.
    Loop,
}

/// The cause of the committed victory, not an earlier proposal replaced by karma.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WinReason {
    Achievements,
    Exhaustion,
    Effect,
    Karma,
    Elimination { loser: Player },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Victory {
    pub reason: WinReason,
    source: Option<HeldCard>,
}

impl Victory {
    fn new(reason: WinReason, context: Context) -> Self {
        Self {
            reason,
            source: context
                .source_card
                .map(|card| Captured::new(card, context.source_evidence)),
        }
    }

    /// Retained source identity, subject to the same privacy as decisions.
    pub fn source_card(self, viewer: Player) -> Option<CardId> {
        self.source
            .filter(|source| source.evidence.known(viewer))
            .map(|source| source.inner)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Terminal {
    Winner(Player, Victory),
    Draw(DrawReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Decision(Decision),
    Complete(ReturnValue),
    Winner(Player),
    Draw(DrawReason),
    /// Machine work was exhausted; no game outcome or extra player choice.
    WorkLimit,
}

/// Internal factual event. This is for native tracker consumers, never a
/// player observation. Optional tracking consumes facts at the commit layer;
/// a full observation encoding remains a separate milestone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Movement {
    pub verb: Option<Verb>,
    pub card: CardId,
    pub source: Zone,
    pub destination: Zone,
    pub executor: Player,
    pub inside_karma: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExchangeMovement {
    pub card: CardId,
    pub source: Zone,
    pub destination: Zone,
}

/// Caller-owned internal facts. Search records concern only the named card;
/// a shuffle records no resulting order. Viewer projection is still separate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A turn began: the opening turn after both starting melds, then each
    /// turn change, before that turn's start procedure (an Artifacts
    /// showcase). Narrated games only ([`Game::with_narration`]).
    TurnBegan {
        player: Player,
    },
    /// A Dogma action started executing, after any karma that could replace
    /// it. Narrated games only ([`Game::with_narration`]).
    DogmaStarted {
        player: Player,
        card: CardId,
        auspice: bool,
        kind: DogmaKind,
    },
    /// A Figure's karma clause triggered and is about to run. Narrated games
    /// only ([`Game::with_narration`]). `would` is the operation a First or
    /// Instead clause interrupts, performed by `actor`; its card may be
    /// hidden from a viewer.
    KarmaTriggered {
        owner: Player,
        card: CardId,
        clause: u8,
        reaction: Reaction,
        actor: Player,
        would: Option<Operation>,
    },
    /// A self-execution, super-execution or single-effect execution of
    /// `card` by `executor` began; its Chain award and effects follow until
    /// [`Event::ExecutionEnded`]. `source` is the card whose effect or karma
    /// asked for it (a Figure: its karma). Narrated games only
    /// ([`Game::with_narration`]).
    ExecutionStarted {
        executor: Player,
        card: CardId,
        mode: ExecutionMode,
        source: Option<CardId>,
        inside_karma: bool,
    },
    /// The execution begun by the matching [`Event::ExecutionStarted`]
    /// finished. Narrated games only ([`Game::with_narration`]).
    ExecutionEnded {
        executor: Player,
        card: CardId,
    },
    /// The Chain rule: `executor` self- or super-executed `card` while
    /// executing a card effect, so first draws and achieves an 11. The draw
    /// and claim follow. Narrated games only ([`Game::with_narration`]).
    ChainAwarded {
        executor: Player,
        card: CardId,
    },
    /// A choice announced at the table: a color, value, count, icon, deck,
    /// named Figure, or a branch with public wording (`label`). Card picks
    /// are not announced; their moves are. Recorded by [`crate::Match`].
    Chose {
        player: Player,
        choice: Alternative,
        label: Option<&'static str>,
    },
    DecreeMoved {
        decree: Decree,
        source: DecreeLocation,
        destination: DecreeLocation,
        executor: Player,
        inside_karma: bool,
    },
    Moved(Movement),
    /// Martian Internet's printed rule: a subgame began at this nesting depth
    /// (1 for a direct subgame).
    SubgameStarted {
        executor: Player,
        depth: usize,
    },
    /// The subgame at this depth ended; its winner's cards join the parent.
    SubgameEnded {
        winner: Option<Player>,
        depth: usize,
    },
    /// A card leaving a finished subgame: into the winner's zone of the
    /// same kind, or junked. `owner` held it in the subgame.
    Transferred {
        card: CardId,
        source: Zone,
        destination: Zone,
        owner: Player,
    },
    /// The new deck's existence/count are public; its shuffled order is not.
    AgeZeroIntroduced {
        executor: Player,
        count: usize,
    },
    SpecialMoved {
        special: Special,
        source: SpecialLocation,
        destination: SpecialLocation,
        executor: Player,
        inside_karma: bool,
    },
    /// The player endorsed their Dogma action of this card (Cities).
    MuseumClaimed {
        player: Player,
        executor: Player,
        inside_karma: bool,
    },
    Endorsed {
        player: Player,
        card: CardId,
    },
    /// Public identity evidence; the card stays in its physical zone. This is
    /// not a movement, an ordinary draw, or ongoing zone inspection permission.
    Revealed {
        card: CardId,
        source: Zone,
        executor: Player,
        inside_karma: bool,
    },
    Splayed {
        player: Player,
        color: Color,
        from: Splay,
        to: Splay,
        executor: Player,
        inside_karma: bool,
    },
    /// One simultaneous movement of both groups, not sequential transfers.
    Exchanged {
        executor: Player,
        inside_karma: bool,
        movements: Vec<ExchangeMovement>,
    },
    FigureSearch {
        executor: Player,
        deck: Deck,
        name: Option<&'static FigureName>,
        found: Option<CardId>,
    },
    Shuffled {
        executor: Player,
        deck: Deck,
    },
    /// A public coin flip by the executor (Quantum Computers).
    Flipped {
        executor: Player,
        won: bool,
    },
}

/// Why a Dogma action runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DogmaKind {
    /// One of the turn's regular actions.
    Action,
    /// The free Dogma of a card promoted from the Forecast (Echoes).
    Promoted,
    /// The free Dogma of the displayed Artifact at turn start.
    Showcased,
    /// Any other free Dogma action.
    Free,
}

/// A triggered karma clause: its Figure card, owner and clause index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hook {
    source: CardId,
    owner: Player,
    figure: Figure,
    clause: u8,
    reaction: Reaction,
    /// What the clause saw when it triggered.
    event: karma::KarmaEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Discover,
    Commit,
    /// A suppressed child acting on an already-retained physical identity.
    Retained,
    Replaced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Task {
    /// Resolve the ordinary selection, then verify its selected movement.
    Verify(Box<Task>),
    /// Rare conditional proof carries its original program registers.
    VerifyWhen(Box<verification::ConditionalVerification>),
    FusionValue(u8),
    TerminalPick(Target),
    OpeningMeld {
        player: Player,
        card: CardId,
    },
    DecreeValue(u8),
    DecreeReturn(Target),
    DecreeBlindReturn(Box<BlindReturn>),
    BindCard(CardId),
    BindReference(HeldCard),
    /// A block's next frozen item, by index; `None` stops a live block.
    BlockItem(Option<usize>),
    /// Frozen hidden block items sharing one public back.
    BlockBlind(Vec<usize>),
    /// A live block's next card.
    BlockCard(CardId),
    ChooseValue(u8),
    ChooseCount(u16),
    ChooseCard(CardId),
    ChooseReference(HeldCard),
    ChooseBlind(Box<BlindChoice>),
    ChooseIcon(Icon),
    ChooseColors(SelectionParameter),
    GroupCard(Target),
    GroupBlind(Box<BlindReturn>),
    StopGroup,
    JunkAchievement(Target),
    JunkAchievementBlind(Box<BlindReturn>),
    JunkAchievementSpecial(Special),
    MatchingPair {
        first: Target,
        second: Target,
    },
    MoveAllBlind(Box<BlindChoice>),
    BlindOperations(Box<BlindOperations>),
    Branch(Option<usize>),
    /// Continue a native karma at this stage.
    Stage(u8),
    /// One pile of a splay batch.
    SplayEach {
        player: Player,
        color: Color,
        direction: Splay,
    },
    ExchangeCard(Target),
    ExchangeBlind(Box<BlindReturn>),
    BeginExchange,
    DeclineExchange,
    FadeCard {
        target: Target,
    },
    Decline,
    JunkSpecial(Special),
    MoveAllCard {
        target: Target,
    },
    /// A named packet member retains one identity through menu sampling.
    MoveAllReference(Box<Captured<Target>>),
    StopMoves,
    TakeSelected {
        index: usize,
    },
    /// The next Search card to return to its deck's bottom.
    SearchReturn(CardId),
    /// Whether to endorse the Dogma action being prepared.
    Endorse(bool),
    Showcase {
        card: CardId,
        auspice: bool,
    },
    /// A draw "from any set" after its set was chosen.
    DrawFrom(Box<DrawFrom>),
    /// The icon the prepared Dogma action uses (Auspice or featured).
    DogmaIcon(Icon),
    Operation(Operation, Context),
    /// A register operand, not a fresh selection from its current hidden pile.
    CapturedOperation(Operation, Context, HeldCard),
    SelfExecute {
        card: CardId,
        context: Context,
    },
    Hook(Hook),
    NameFigure {
        deck: Deck,
        context: Context,
    },
    FindFigure {
        deck: Deck,
        name: Option<&'static FigureName>,
        context: Context,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Frame {
    TerminalEffect(Box<TerminalEffect>),
    Opening(Opening),
    /// Martian Internet's printed rule: a subgame's setup, or its one-card
    /// junk when it cannot start.
    Subgame(SubgameStep),
    /// One enclosing action, shared by copied and shared executions. A true
    /// nested free action gets a fresh scope; terminal unwinding drops both.
    /// One action's scope: Democracy's return tallies, Yunus's majority and
    /// the boards Dancing Girl has been on (player bits).
    ActionReturns {
        democracy: [u32; 2],
        majority: Option<Player>,
        dancing_girl: u8,
        last: HeldReturn,
        /// Action slots, allocated when a card first records one.
        memory: Option<Box<slots::Memory>>,
    },
    Decree(Box<DecreeProgress>),
    DecreePayment {
        context: Context,
        allowed: Vec<Decree>,
    },
    /// Immediate consequences cannot overwrite the committed operation result.
    ReturnOutcome(HeldOutcome),
    /// A reached karma instruction's proof cannot replace Last/Did/slot state.
    KarmaProof {
        saved: Box<Frame>,
        revealed: Option<HeldCard>,
    },
    JunkAvailableSpecials {
        context: Context,
        /// Bit [`Special::index`] per token already considered.
        considered: u32,
    },
    ActionTail {
        action: Action,
        context: Context,
        kind: ActionKind,
        advance_turn: bool,
        stage: u8,
        last: Option<HeldOutcome>,
        shared: bool,
        /// This action's own Dogma is endorsed (Cities): a replacement
        /// action has its own tail and is not.
        endorsed: bool,
        /// A Meld action's own meld performed: promotion (Echoes) follows
        /// its action-end checks.
        melded: bool,
        /// This is a promotion's free Dogma, the scope of "foreseen".
        promoted: bool,
    },
    Dogma(Dogma),
    /// Echoes promotion after a Meld action's checks.
    Promote(Box<echoes::Promotion>),
    EndAction {
        context: Context,
        stage: u8,
    },
    EndSpecials {
        context: Context,
        /// Bit `player * Special::RESERVED + Special::index` per attempted claim.
        considered: u64,
    },
    Fading(Box<Fading>),
    /// Keep bulk progress out of the common frame's inline storage.
    MoveAll(Box<MoveAll>),
    SplayAll(Box<SplayAll>),
    MatchingPair(Box<MatchingPair>),
    SelectGroup(Box<GroupSelection>),
    JunkAvailable(Box<JunkAvailable>),
    RevealAll(Box<verification::RevealPacket>),
    DrawCount(DrawCount),
    SelectExchange(Box<ExchangeSelection>),
    /// Rare interrupted-exchange data stays out of the common operation frame.
    Exchange {
        plan: Box<ExchangePlan>,
        phase: Phase,
    },
    /// Tuck (to the owner's board) or return these cards in a chosen order.
    /// Karma may run between them, so each target is a retained reference.
    MoveSelected {
        verb: Verb,
        targets: Vec<Captured<Target>>,
        context: Context,
    },
    NameFigure {
        deck: Deck,
        context: Context,
    },
    FindFigure {
        deck: Deck,
        name: Option<&'static FigureName>,
        context: Context,
    },
    Operation {
        request: Operation,
        context: Context,
        phase: Phase,
        knowledge: Option<OperandKnowledge>,
        /// A draw already resolved above its requested age.
        short: bool,
    },
    Execute(CopiedExecution),
    /// The slots of the program frame directly above it.
    Locals(Box<slots::Locals>),
    Program {
        source: Source,
        pc: u16,
        context: Context,
        last: HeldReturn,
        retained: Option<HeldCard>,
        chosen: SelectionParameter,
        boxed: BoxedValues,
    },
    Choose {
        player: Player,
        tasks: Vec<Task>,
        options: Box<DecisionOptions>,
    },
    Karma {
        hook: Hook,
        stage: u8,
        visited: Vec<HeldCard>,
        last: Option<HeldOutcome>,
    },
    MeldAction(Box<cities::MeldAction>),
    Showcase(Box<artifacts::Showcase>),
    MuseumCheck(artifacts::MuseumCheck),
    CitySearch(Box<cities::CitySearch>),
    DogmaPrep(Box<cities::DogmaPrep>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Dogma {
    card: CardId,
    player: Player,
    source_owner: Player,
    effect: usize,
    /// 0: the opponent (a sharer, or a demand's victim); 1: the owner; 2: an
    /// endorsed repeat (the victim again for a demand, else the owner).
    participant: u8,
    endorsed: bool,
    sharing: bool,
    /// The opponent has fewer of the featured icon. Fixed with the first
    /// comparison: Sargon's eligibility leaves its owner vulnerable.
    vulnerable: bool,
    /// Waiting for the opponent's ineligibility event (Sargon replaces it).
    eligibility: bool,
    shared: bool,
    executing_shared: bool,
    /// Successful original transfers in the preceding demand, across repeats.
    demand_transfers: u32,
    demand_returns: u32,
    suppress_karma: bool,
    boxed_action: BoxedValues,
    /// The frozen schedule and execution slots, when there are any.
    extra: Option<Box<ExecutionExtra>>,
}

/// A card execution's rare data, boxed to keep the common frame small.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ExecutionExtra {
    /// The effects frozen at lock-in when echoes run with the card; `None`
    /// runs the card's own dogma effects.
    schedule: Option<Schedule>,
    /// Execution slots.
    memory: slots::Memory,
}

impl ExecutionExtra {
    /// The effect at a schedule position, if the schedule names one, or
    /// `None` without a schedule.
    fn scheduled(extra: &Option<Box<Self>>) -> Option<&Schedule> {
        extra.as_ref().and_then(|extra| extra.schedule.as_ref())
    }
}

/// Effects frozen when a Dogma locks in or a card is self/super-executed: (source card,
/// effect index), echoes first. Moving or covering a source afterwards does not change
/// it; a newly exposed echo waits for a later invocation.
pub(super) type Schedule = Vec<(CardId, u16)>;

impl Dogma {
    /// The effect at the current position, if the schedule has one.
    fn current(&self) -> Option<(CardId, u16)> {
        match ExecutionExtra::scheduled(&self.extra) {
            Some(schedule) => schedule.get(self.effect).copied(),
            None => u16::try_from(self.effect)
                .ok()
                .map(|effect| (self.card, effect)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Fading {
    context: Context,
    visited: Vec<HeldCard>,
    awaiting_requirement: bool,
    last: Option<HeldOutcome>,
}

/// A card choice's anonymous remainder after explicit names were offered.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BlindChoice {
    targets: Vec<Target>,
    excluded: Vec<HeldCard>,
    chooser: Player,
}

/// Operations sharing one public back. The chooser never names their hidden
/// identities; the engine samples natively after that back is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BlindOperations {
    operations: Vec<(Operation, Option<HeldCard>)>,
    /// Explicit named alternatives omitted from this anonymous back group.
    excluded: Vec<HeldCard>,
    context: Context,
}

/// A count snapshot, not a pre-drawn packet. The common frame stays compact.
/// A chosen-set draw's resolved request.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DrawFrom {
    value: u8,
    set: Option<CardSet>,
    count: Option<u16>,
    then: Then,
    context: Context,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DrawCount {
    remaining: u16,
    value: u8,
    set: Option<CardSet>,
    then: Then,
    colors: u8,
    repeated_color: bool,
    context: Context,
    stage: u8,
    last: HeldReturn,
    outcome: HeldBulk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MoveAll {
    verb: Verb,
    from: Selector,
    to: Destination,
    context: Context,
    regs: Regs,
    /// Start of the current uninterrupted proof-valid ordering prefix.
    ordered_since: Option<u16>,
    /// The candidates of the last menu: an ordering floor binds only cards
    /// the chooser passed over, never one that arrived since (Currency's
    /// draws).
    seen: Vec<CardId>,
    visited: Vec<HeldCard>,
    outcome: HeldBulk,
    limit: Option<u16>,
    stop: Stop,
    shortcut: Option<Shortcut>,
    verify: bool,
    when: Option<&'static Cond>,
    /// Named matching arrivals disclosed by this verified live batch.
    proofs: Vec<HeldCard>,
    proving: bool,
    per_color: bool,
    /// Colors already attempted by a per-color batch.
    colors_done: u8,
    /// A per-value batch's remaining cap on each value (index = value),
    /// lowered as each card is attempted.
    quotas: Option<[u16; 16]>,
    /// Performed originals already offered to the program's slot record.
    kept: u16,
    /// A committed group: every original is attempted once, never replaced
    /// with a newly eligible card exposed or introduced by a reaction.
    frozen: Option<Vec<Captured<Target>>>,
    /// Full selector snapshot, rather than a selected group whose predicate
    /// must remain true. Only each original physical/logical origin is rechecked.
    frozen_selection: bool,
}

impl MoveAll {
    fn new(verb: Verb, from: Selector, to: Destination, context: Context) -> Self {
        Self {
            verb,
            from,
            to,
            context,
            regs: Regs::EMPTY,
            ordered_since: None,
            seen: Vec::new(),
            visited: Vec::new(),
            outcome: HeldBulk::default(),
            limit: None,
            stop: Stop::Never,
            shortcut: None,
            verify: false,
            when: None,
            proofs: Vec::new(),
            proving: false,
            per_color: false,
            colors_done: 0,
            quotas: None,
            kept: 0,
            frozen: None,
            frozen_selection: false,
        }
    }
}

/// "Splay every eligible color": each player and color at most once.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SplayAll {
    step: SplayStep,
    context: Context,
    regs: Regs,
    /// Player-color pairs already attempted.
    done: u16,
    outcome: HeldBulk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExchangePlan {
    left: Vec<Target>,
    right: Vec<Target>,
    left_destination: Zone,
    right_destination: Zone,
    context: Context,
}

impl ExchangePlan {
    fn movements(&self) -> impl Iterator<Item = (Target, Zone)> + '_ {
        self.left
            .iter()
            .filter(|t| {
                t.physical != self.left_destination
                    && !self.right.iter().any(|other| other.card == t.card)
            })
            .map(|&t| (t, self.left_destination))
            .chain(
                self.right
                    .iter()
                    .filter(|t| {
                        t.physical != self.right_destination
                            && !self.left.iter().any(|other| other.card == t.card)
                    })
                    .map(|&t| (t, self.right_destination)),
            )
    }
}

impl Frame {
    /// The program this frame runs, including one saved for a karma proof.
    fn program(&self) -> Option<RevealScope> {
        match self {
            Frame::Program {
                source, context, ..
            } => Some(RevealScope {
                source: *source,
                executor: context.executor,
            }),
            Frame::KarmaProof { saved, .. } => saved.program(),
            _ => None,
        }
    }

    fn fading(context: Context) -> Self {
        Self::Fading(Box::new(Fading {
            context,
            visited: Vec::new(),
            awaiting_requirement: false,
            last: None,
        }))
    }

    fn operation(request: Operation, context: Context) -> Self {
        Self::Operation {
            request,
            context,
            phase: Phase::Discover,
            knowledge: None,
            short: false,
        }
    }

    /// An operation on a card a retained reference already names: a move of
    /// it keeps that reference's pick, so observers follow one card.
    fn referenced_operation(request: Operation, context: Context, reference: HeldCard) -> Self {
        match request {
            Operation::Move { target, .. }
            | Operation::Reveal { target }
            | Operation::Claim { target, .. }
                if target.card == reference.inner =>
            {
                Self::captured_operation(request, context, reference)
            }
            _ => Self::operation(request, context),
        }
    }

    fn captured_operation(request: Operation, context: Context, reference: HeldCard) -> Self {
        debug_assert!(
            matches!(request, Operation::Move { target, .. } | Operation::Reveal { target } | Operation::Claim { target, .. } if target.card == reference.inner)
        );
        Self::Operation {
            request,
            context,
            phase: Phase::Discover,
            knowledge: Some(OperandKnowledge {
                evidence: reference.evidence,
                conditioned: false,
                private_query: None,
                verify: false,
            }),
            short: false,
        }
    }

    fn effect(card: CardId, effect: u16, context: Context) -> Self {
        Self::operation(
            Operation::ExecuteEffect {
                card,
                effect,
                boxed: context.boxed_action,
            },
            context,
        )
    }

    fn receive(&mut self, value: HeldReturn) {
        match self {
            Self::KarmaProof { revealed, .. } => {
                *revealed = value
                    .operation()
                    .filter(|outcome| outcome.disposition == Disposition::Performed)
                    .and_then(HeldOutcome::card_ref);
            }
            Self::TerminalEffect(progress) => progress.last = value,
            Self::ActionReturns { last, .. } | Self::Program { last, .. } => *last = value,
            Self::ActionTail { shared, last, .. } => match value.inner {
                ReturnValue::Dogma { shared: result } => *shared = result,
                _ => *last = value.operation(),
            },
            Self::Fading(fading) => fading.last = value.operation(),
            Self::Dogma(dogma) => {
                if dogma.eligibility {
                    dogma.eligibility = false;
                    dogma.sharing = value
                        .operation()
                        .is_some_and(|outcome| outcome.disposition == Disposition::Replaced);
                }
            }
            Self::Promote(promotion) => promotion.receive(value),
            Self::Execute(execution) => execution.receive(value),
            Self::DrawCount(batch) => batch.last = value,
            Self::MoveAll(batch) => {
                if let Some(outcome) = value.operation() {
                    if batch.proving {
                        if let Some(card) = outcome.card {
                            batch.proofs.push(outcome.with(card));
                        }
                    } else {
                        batch.outcome.record_original(outcome);
                    }
                }
                batch.proving = false;
            }
            Self::SplayAll(batch) => {
                if let Some(outcome) = value.operation() {
                    batch.outcome.record_original(outcome);
                }
            }
            Self::MatchingPair(pair) => {
                if let Some(outcome) = value.operation() {
                    pair.outcome.record_original(outcome);
                }
            }
            Self::RevealAll(packet) => packet.last = value,
            Self::JunkAvailable(batch) => {
                if let Some(outcome) = value.operation() {
                    batch.outcome.record_original(outcome);
                }
            }
            Self::Karma { last, .. } => *last = value.operation(),
            Self::Opening(_)
            | Self::Subgame(_)
            | Self::Decree(_)
            | Self::DecreePayment { .. }
            | Self::ReturnOutcome(_)
            | Self::JunkAvailableSpecials { .. }
            | Self::EndAction { .. }
            | Self::EndSpecials { .. }
            | Self::SelectGroup(_)
            | Self::SelectExchange(_)
            | Self::Exchange { .. }
            | Self::MoveSelected { .. }
            | Self::NameFigure { .. }
            | Self::FindFigure { .. }
            | Self::Operation { .. }
            | Self::Locals(_)
            | Self::Choose { .. }
            | Self::MeldAction(_)
            | Self::Showcase(_)
            | Self::MuseumCheck(_)
            | Self::CitySearch(_)
            | Self::DogmaPrep(_) => {}
        }
    }
}

/// Two compiled execution paths using the same operation and effect programs. This
/// first slice specializes dispatch, not frame storage. Splitting rare Figure payloads
/// out of the common frame is an explicit performance gate. Generous enough for long
/// forced chains, finite even when nested copies grow their continuation instead of
/// reaching the same state again. Automatic steps (frame pops: instructions,
/// operations, batch items, forced choices) between decisions before a mandatory loop
/// is drawn. Random play over ~950,000 prompts in all 32 modes averaged 17 steps
/// between prompts; the longest legitimate chain was 1,446 (a return-and-redraw cycle
/// that drew 170 cards until a draw ended the game by exhaustion). A longer cycle ends
/// as an expedient draw instead, and a reshuffling loop (never an exact repeat) then
/// ends within a second even at milliseconds per tracked step.
const AUTOMATIC_WORK_LIMIT: usize = 4096;

/// Everything that decides how a repeating effect continues.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Recurrence {
    position: Position,
    frames: Vec<Frame>,
    rng: Rng,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    position: Position,
    frames: Vec<Frame>,
    completed: HeldReturn,
    terminal: Option<Terminal>,
    fault: Option<Error>,
    rng: Rng,
    knowledge: Option<Box<crate::tracking::Knowledge>>,
    /// The state at each Repeat since the last decision (Figures only).
    repeats: Vec<Recurrence>,
    /// Interpreter steps since the last real decision or completed invocation.
    automatic_work: usize,
    /// The program whose explicit reveal currently shows a face
    /// ([`Position::revealed_card`]); the reveal ends with it.
    reveal_scope: Option<RevealScope>,
    /// An execution with an inspection grant may be on the stack, so
    /// [`Self::refresh_inspection`] must scan it. Cleared once no grant is
    /// live, so games without Hitchhiking never scan.
    inspecting: bool,
    /// Also report [`Event::TurnBegan`] and [`Event::DogmaStarted`]
    /// ([`Game::with_narration`]).
    narrated: bool,
    /// Martian Internet plays its printed subgame ([`Game::with_subgames`])
    /// rather than ending the game in a draw. Search worlds never do.
    subgames: bool,
    /// The game this subgame suspended, frozen until the subgame ends
    /// (`None` in an ordinary game). Shared, so cloning a simulation inside a
    /// subgame never copies its parent.
    parent: Option<Arc<Parent>>,
    /// Manual path oracle only; never a production rules-profile switch.
    #[cfg(test)]
    shortcuts_disabled: bool,
}

impl Game {
    pub fn new(position: Position, expansions: Expansions) -> Result<Self, Error> {
        Self::seeded(position, expansions, 0)
    }

    /// A game on this position under these expansions' rules.
    pub fn seeded(position: Position, expansions: Expansions, seed: u64) -> Result<Self, Error> {
        let position = position.with_expansions(expansions);
        if position.catalog.iter().any(|c| !expansions.includes(c.set))
            || !expansions.figures()
                && (position.catalog.iter().any(|c| c.value == 0)
                    || Decree::ALL
                        .into_iter()
                        .any(|d| position.decree_location(d) != DecreeLocation::Supply))
        {
            return Err(Error::InvalidSetup(
                "a disabled expansion's cards, Age 0 or Decrees in the position",
            ));
        }
        if !expansions.echoes()
            && Player::ALL
                .into_iter()
                .any(|p| !position.cards(Zone::Forecast(p)).is_empty())
        {
            return Err(Error::InvalidSetup("a Forecast without Echoes"));
        }
        if !expansions.unseen()
            && Player::ALL
                .into_iter()
                .any(|p| !position.cards(Zone::Safe(p)).is_empty())
        {
            return Err(Error::InvalidSetup("a Safe without Unseen"));
        }
        if !expansions.artifacts() && position.artifacts.as_ref().is_some_and(|a| !a.is_empty()) {
            return Err(Error::InvalidSetup("a display or Museum without Artifacts"));
        }
        Ok(Self {
            position,
            frames: Vec::new(),
            completed: HeldReturn::UNIT,
            terminal: None,
            fault: None,
            rng: Rng::new(seed),
            knowledge: None,
            repeats: Vec::new(),
            automatic_work: 0,
            reveal_scope: None,
            inspecting: false,
            narrated: false,
            subgames: false,
            parent: None,
            #[cfg(test)]
            shortcuts_disabled: false,
        })
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    /// Also report turn starts and Dogma starts as events, for a log. Off by
    /// default, so event traces stay the rules' physical facts.
    pub fn with_narration(mut self) -> Self {
        self.narrated = true;
        self
    }

    /// Advance fixture timing only between completed invocations. A free
    /// action inherits the number of regular actions already started.
    pub fn begin_turn(&mut self, player: Player) -> Result<(), Error> {
        self.ensure_idle()?;
        self.position.begin_turn(player);
        self.start_turn_procedure(player);
        self.refresh_tracking()
    }

    pub fn begin_opening_turn(&mut self, player: Player) -> Result<(), Error> {
        self.ensure_idle()?;
        self.position.begin_opening_turn(player);
        self.refresh_tracking()
    }

    /// Structured presentation for a completed victory. Draws and pending or
    /// replaced win proposals have no victory reason.
    pub fn victory(&self) -> Option<Victory> {
        match self.terminal {
            Some(Terminal::Winner(_, victory)) => Some(victory),
            _ => None,
        }
    }

    pub fn begin_action(&mut self, kind: ActionKind) -> Result<(), Error> {
        self.ensure_idle()?;
        self.position.begin_action(kind)?;
        self.refresh_tracking()
    }

    /// Execute an admitted action, its immediate consequences and action end.
    /// Requests are native identities, not observer-safe policy encodings.
    /// Invalid requests leave the entire machine (including RNG) unchanged.
    pub fn take_action(&mut self, action: Action, kind: ActionKind) -> Result<(), Error> {
        self.ensure_idle()?;
        if kind == ActionKind::Regular
            && self.position.regular_actions_started >= self.position.regular_action_limit
        {
            return Err(Error::InvalidAction);
        }
        self.start_action(action, kind, true)
    }

    /// Admit and schedule an action: its body, then its tail. A showcase's
    /// free Dogma runs inside the turn-start procedure and never flips the
    /// turn.
    fn start_action(
        &mut self,
        action: Action,
        kind: ActionKind,
        advance_turn: bool,
    ) -> Result<(), Error> {
        let player = self.position.turn_owner;
        // A showcased source is admitted here, as an endorsed preparation
        // admits its source before payment.
        let context = match action {
            Action::Dogma { card, .. } if self.showcase_source(player, card) => Context {
                source_card: Some(card),
                source_evidence: Evidence::PUBLIC,
                ..Context::new(player)
            },
            _ => Context::new(player),
        };
        let body = if action == Action::Decree {
            let allowed = self.position.decree_action_options(kind)?;
            if allowed.is_empty() {
                return Err(Error::InvalidAction);
            }
            Frame::DecreePayment { context, allowed }
        } else if let Some(prep) = self.dogma_preparation(action, kind)? {
            Frame::DogmaPrep(Box::new(prep))
        } else {
            let operation = self.action_operation(action, kind)?;
            Frame::operation(self.blind_claim(operation), context)
        };
        self.position.begin_action(kind)?;
        self.refresh_tracking()?;
        self.push_action_returns();
        self.frames.push(Frame::ActionTail {
            action,
            context,
            kind,
            advance_turn,
            stage: 0,
            last: None,
            shared: false,
            endorsed: false,
            melded: false,
            promoted: false,
        });
        self.frames.push(body);
        Ok(())
    }

    /// An available achievement is claimed by its back: among identical backs the card
    /// is random. A unique back draws nothing.
    fn blind_claim(&mut self, operation: Operation) -> Operation {
        let Operation::Claim {
            target,
            eligibility,
        } = operation
        else {
            return operation;
        };
        if self.position.locate(target.card) != Some(Zone::AvailableAchievements) {
            return operation;
        }
        let back = self.position.card(target.card).origin_deck();
        let twins: Vec<_> = self
            .position
            .targets(LogicalZone::AvailableAchievements(self.position.turn_owner))
            .filter(|twin| self.position.card(twin.card).origin_deck() == back)
            .collect();
        if twins.len() < 2 {
            return operation;
        }
        let slot = (self.rng.next_u64() % twins.len() as u64) as usize;
        Operation::Claim {
            target: twins[slot],
            eligibility,
        }
    }

    fn action_operation(&self, action: Action, kind: ActionKind) -> Result<Operation, Error> {
        let player = self.position.turn_owner;
        Ok(match action {
            Action::Draw => Operation::DrawAction { set: CardSet::Base },
            Action::Meld { card } => {
                let target = self
                    .position
                    .action_hand(kind)
                    .chain(self.position.museum_melds(player))
                    .find(|target| target.card == card)
                    .ok_or(Error::InvalidAction)?;
                Operation::Move {
                    verb: Verb::Meld,
                    target,
                    destination: Zone::Board(player, self.position.card(card).color),
                }
            }
            Action::Dogma { card, auspice } => {
                let selected = self
                    .position
                    .catalog
                    .get(usize::from(card.0))
                    .ok_or(Error::InvalidAction)?;
                let icon = selected.featured.ok_or(Error::InvalidAction)?;
                if (self.position.dogma_source_owner(player, card).is_none()
                    && !self.showcase_source(player, card))
                    || (auspice
                        && !self.auspice_offered(
                            player,
                            card,
                            (kind == ActionKind::Regular)
                                .then(|| self.position.regular_actions_started() + 1),
                        ))
                {
                    return Err(Error::InvalidAction);
                }
                Operation::Dogma {
                    card,
                    icon: if auspice { Icon::Avatar } else { icon },
                }
            }
            Action::Achieve { card } => {
                if !self.position.eligible_for_achievement(player, card) {
                    return Err(Error::InvalidAction);
                }
                let safe = Zone::Safe(player);
                let target = self
                    .position
                    .targets(LogicalZone::AvailableAchievements(player))
                    .find(|target| target.card == card)
                    .or_else(|| {
                        self.position.cards(safe).contains(&card).then_some(Target {
                            card,
                            physical: safe,
                            logical: LogicalZone::Physical(safe),
                        })
                    })
                    .ok_or(Error::InvalidAction)?;
                Operation::Claim {
                    target,
                    eligibility: ClaimEligibility::Available,
                }
            }
            Action::Decree => unreachable!("Decree action has a payment continuation"),
        })
    }

    /// A narrow continuation projection, not a full observation or belief
    /// tracker. The retained reference remains private if only its back is
    /// known. Previously visible identity is not forgotten if the card moves.
    pub fn pending_melds(&self, viewer: Player) -> Vec<PendingMeld> {
        self.frames
            .iter()
            .filter_map(|frame| {
                let Frame::Operation {
                    request:
                        Operation::Move {
                            verb: Verb::Meld,
                            target,
                            ..
                        },
                    context,
                    phase,
                    knowledge,
                    ..
                } = frame
                else {
                    return None;
                };
                if *phase == Phase::Replaced {
                    return None;
                }
                let knowledge = knowledge.unwrap_or_else(|| OperandKnowledge {
                    evidence: Evidence::selection(&self.position, target.physical),
                    conditioned: false,
                    private_query: None,
                    verify: false,
                });
                let card = self.position.card(target.card);
                Some(PendingMeld {
                    player: context.executor,
                    card: if knowledge.evidence.known(viewer) {
                        CardView::Known(target.card)
                    } else {
                        CardView::Back(card.origin_deck())
                    },
                })
            })
            .collect()
    }

    fn refresh_meld_knowledge(&mut self) {
        if !self.position.figures() {
            return;
        }
        let Some(trackers) = self.knowledge.as_ref().map(|k| &k.trackers) else {
            return;
        };
        for frame in &mut self.frames {
            if let Frame::Operation {
                request:
                    Operation::Move {
                        verb: Verb::Meld, ..
                    },
                knowledge: Some(knowledge),
                ..
            } = frame
                && let Some(pick) = knowledge.evidence.pick()
            {
                // A pending meld's operand becomes known once the viewer can
                // name its pick: a reveal of that pick, or a move of it into
                // view. Where the real card went does not decide it.
                for viewer in Player::ALL {
                    if trackers[viewer.index()].pick_identity(pick).is_some() {
                        knowledge.evidence |= Evidence::known_to(viewer);
                    }
                }
            }
        }
    }

    pub fn start(&mut self, request: Operation, context: Context) -> Result<(), Error> {
        self.ensure_idle()?;
        self.push_action_returns();
        self.frames.push(Frame::operation(request, context));
        Ok(())
    }

    /// Finish an action after its body has resolved: fade the non-active player,
    /// then the active player, then resolve Base specials and achievement victory.
    /// Does not advance the turn or reset regular/free-action timing.
    pub fn finish_action(&mut self) -> Result<(), Error> {
        self.ensure_idle()?;
        self.frames.push(Frame::EndAction {
            context: Context::new(self.position.turn_owner),
            stage: 0,
        });
        Ok(())
    }

    /// Resolve one player's fading as an isolated operation probe.
    pub fn fade(&mut self, context: Context) -> Result<(), Error> {
        self.ensure_idle()?;
        self.frames.push(Frame::fading(context));
        Ok(())
    }

    pub fn self_execute(&mut self, card: CardId, context: Context) -> Result<(), Error> {
        self.ensure_idle()?;
        self.push_action_returns();
        self.push_copied(card, ExecutionMode::SelfExecute, context);
        Ok(())
    }

    pub fn super_execute(&mut self, card: CardId, context: Context) -> Result<(), Error> {
        self.ensure_idle()?;
        self.push_action_returns();
        self.push_copied(card, ExecutionMode::SuperExecute, context);
        Ok(())
    }

    fn ensure_idle(&self) -> Result<(), Error> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        if !self.frames.is_empty() || self.terminal.is_some() {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }

    pub fn choose(&mut self, option: usize) -> Result<(), Error> {
        let Some(Frame::Choose { tasks, .. }) = self.frames.last() else {
            return Err(Error::NoDecision);
        };
        let task = tasks.get(option).ok_or(Error::InvalidChoice)?.clone();
        self.frames.pop();
        self.push_task(task);
        Ok(())
    }

    /// Event storage belongs to the caller, so cloning a simulation never
    /// copies an accumulated presentation log or replay history.
    pub fn advance(&mut self, work: usize, events: &mut Vec<Event>) -> Result<Status, Error> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        for _ in 0..work {
            self.refresh_tracking()?;
            // Between steps every retained reference lives in the frames.
            self.collect_picks();
            // A reveal lasts through its program and that program's children.
            if let Some(scope) = self.reveal_scope
                && !self
                    .frames
                    .iter()
                    .any(|frame| frame.program() == Some(scope))
            {
                self.reveal_scope = None;
                self.position.end_reveal();
            }
            if let Some(terminal) = self.terminal {
                // A finished subgame rejoins its parent instead of ending
                // the game; a search world (no subgames) ends here.
                if self.subgames && self.parent.is_some() {
                    self.end_subgame(terminal, events)?;
                    continue;
                }
                self.repeats.clear();
                self.automatic_work = 0;
                return Ok(match terminal {
                    Terminal::Winner(winner, _) => Status::Winner(winner),
                    Terminal::Draw(reason) => Status::Draw(reason),
                });
            }
            let Some(frame) = self.frames.pop() else {
                self.refresh_inspection(None);
                self.repeats.clear();
                self.automatic_work = 0;
                return Ok(Status::Complete(self.completed.inner));
            };
            self.refresh_inspection(Some(&frame));
            // Separate from the caller's resumable `work` budget. A real
            // decision at the boundary is still allowed to break a loop.
            if !matches!(&frame, Frame::Choose { tasks, .. } if tasks.len() > 1) {
                if self.automatic_work >= AUTOMATIC_WORK_LIMIT {
                    self.frames.clear();
                    self.repeats.clear();
                    self.terminal = Some(Terminal::Draw(DrawReason::Loop));
                    self.automatic_work = 0;
                    // A capped subgame is a drawn child, merged above.
                    if self.subgames && self.parent.is_some() {
                        continue;
                    }
                    return Ok(Status::Draw(DrawReason::Loop));
                }
                self.automatic_work += 1;
            }
            if let Frame::Choose {
                player,
                tasks,
                options,
            } = frame
            {
                if tasks.len() == 1 {
                    self.push_task(tasks.into_iter().next().expect("one task"));
                    continue;
                }
                // A decision ends any mandatory loop; resting states keep
                // no loop history.
                self.repeats.clear();
                self.automatic_work = 0;
                let decision = self.decision_snapshot(player, &options);
                self.frames.push(Frame::Choose {
                    player,
                    tasks,
                    options,
                });
                return Ok(Status::Decision(decision));
            }
            if let Err(error) = self.resume(frame, events) {
                // Unsupported prototype paths fail closed. They never silently
                // skip an instruction and are not resumable rules outcomes.
                self.fault = Some(error.clone());
                return Err(error);
            }
        }
        self.refresh_tracking()?;
        Ok(Status::WorkLimit)
    }

    #[inline]
    fn complete(&mut self, value: impl Into<HeldReturn>) {
        let value = value.into();
        if let [.., Frame::Locals(locals), Frame::Program { .. }] = self.frames.as_mut_slice() {
            locals.last_exact_known = locals.progress_known;
        }
        if let Some(parent) = self.frames.last_mut() {
            parent.receive(value);
        } else {
            self.completed = value;
        }
    }

    fn take_batch_card(&mut self, target: Target, reference: Option<HeldCard>) {
        let mut reference = reference.unwrap_or_else(|| self.capture(target.card));
        let Some(Frame::MoveAll(batch)) = self.frames.last_mut() else {
            unreachable!("bulk movement caller");
        };
        if batch.from.selects_deck_top() {
            reference.evidence = reference.evidence.at_position();
        }
        // Consider this identity before its operation starts, so a
        // replacement leaving it eligible cannot make us retry it.
        batch.visited.push(reference);
        if batch.per_color {
            batch.colors_done |= 1 << self.position.card(target.card).color as u8;
        }
        if let Some(quota) = batch
            .quotas
            .as_mut()
            .and_then(|quotas| quotas.get_mut(usize::from(self.position.value(target.card))))
        {
            *quota = quota.saturating_sub(1);
        }
        let (verb, from, to, context, verify, when, regs) = (
            batch.verb,
            batch.from,
            batch.to,
            batch.context,
            batch.verify,
            batch.when,
            batch.regs,
        );
        let operation = self.program_movement(verb, from, target, to, context);
        self.frames
            .push(Frame::referenced_operation(operation, context, reference));
        if let Err(error) = self.verify_pending_move_for(verify, when, regs) {
            self.fault = Some(error);
        }
    }

    fn push_task(&mut self, task: Task) {
        match task {
            Task::GroupCard(target) => self.choose_group_card(target),
            Task::GroupBlind(group) => {
                let index = if group.targets.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % group.targets.len() as u64) as usize
                };
                self.choose_group_card(group.targets[index]);
            }
            Task::StopGroup => self.stop_group(),
            Task::Verify(task) => {
                self.push_task(*task);
                self.verify_pending_move();
            }
            Task::VerifyWhen(proof) => {
                self.push_task(proof.task);
                if let Err(error) = self.verify_pending_move_when(proof.condition, proof.regs) {
                    self.fault = Some(error);
                }
            }
            Task::JunkAchievement(target) => self.junk_achievement(Some(target), None),
            Task::JunkAchievementSpecial(special) => self.junk_achievement(None, Some(special)),
            Task::JunkAchievementBlind(group) => {
                let index = if group.targets.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % group.targets.len() as u64) as usize
                };
                self.junk_achievement(Some(group.targets[index]), None);
            }
            Task::FusionValue(value) => self.choose_fusion_value(value),
            Task::TerminalPick(target) => self.choose_terminal_pick(target),
            Task::ChooseColors(parameter) => {
                let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
                    unreachable!("color selection caller");
                };
                *chosen = parameter;
            }
            Task::ChooseIcon(icon) => {
                let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
                    unreachable!("icon selection caller");
                };
                *chosen = SelectionParameter::Icon(icon);
            }
            Task::MatchingPair { first, second } => self.select_matching_pair(first, second),
            Task::Branch(skip) => {
                let Some(Frame::Program { pc, .. }) = self.frames.last_mut() else {
                    unreachable!("branch caller");
                };
                *pc = skip.map_or(u16::MAX, |skip| *pc + skip as u16);
            }
            Task::Stage(next) => {
                let Some(Frame::Karma { stage, .. }) = self.frames.last_mut() else {
                    unreachable!("native karma stage caller");
                };
                *stage = next;
            }
            Task::BlindOperations(group) => {
                let index = if group.operations.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % group.operations.len() as u64) as usize
                };
                let (operation, reference) = group.operations[index];
                let target = match operation {
                    Operation::Claim { target, .. } | Operation::Move { target, .. } => target,
                    _ => unreachable!("blind card operation"),
                };
                let reference = reference.unwrap_or_else(|| self.capture(target.card));
                self.constrain_anonymous_choice(
                    reference,
                    &group.excluded,
                    group.context.controller,
                );
                self.push_task(Task::CapturedOperation(operation, group.context, reference));
            }
            Task::ExchangeCard(target) => {
                let Some(Frame::SelectExchange(selection)) = self.frames.last_mut() else {
                    unreachable!("exchange selection caller");
                };
                selection.take(target);
            }
            Task::ExchangeBlind(group) => {
                let slot = if group.targets.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % group.targets.len() as u64) as usize
                };
                self.push_task(Task::ExchangeCard(group.targets[slot]));
            }
            Task::BeginExchange => {}
            Task::SplayEach {
                player,
                color,
                direction,
            } => {
                self.mark_splay(player, color);
                let Some(Frame::SplayAll(batch)) = self.frames.last() else {
                    unreachable!("splay batch caller");
                };
                let context = batch.context;
                self.frames.push(Frame::operation(
                    Operation::Splay {
                        player,
                        color,
                        direction,
                    },
                    context,
                ));
            }
            Task::DeclineExchange => {
                self.frames.pop();
                self.complete(ReturnValue::Operation(OperationOutcome::empty(
                    Disposition::Inapplicable,
                )));
            }
            Task::ChooseValue(value) => {
                let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
                    unreachable!("value selection caller");
                };
                *chosen = SelectionParameter::Value(value);
            }
            Task::ChooseCount(count) => {
                let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
                    unreachable!("count selection caller");
                };
                *chosen = SelectionParameter::Count(count);
            }
            Task::ChooseBlind(group) => {
                // Hand selection uses the native random-hidden-card primitive:
                // even one candidate consumes a draw (supply picks differ).
                let slot = (self.rng.next_u64() % group.targets.len() as u64) as usize;
                let reference = self.capture(group.targets[slot].card);
                self.constrain_anonymous_choice(reference, &group.excluded, group.chooser);
                self.push_task(Task::ChooseReference(reference));
            }
            Task::ChooseReference(card) => {
                let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
                    unreachable!("retained card selection caller");
                };
                *chosen = SelectionParameter::Card(card);
            }
            Task::ChooseCard(card) => {
                let card = self.capture(card);
                let Some(Frame::Program { chosen, .. }) = self.frames.last_mut() else {
                    unreachable!("card selection caller");
                };
                *chosen = SelectionParameter::Card(card);
            }
            Task::MoveAllBlind(group) => {
                let slot = if group.targets.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % group.targets.len() as u64) as usize
                };
                let target = group.targets[slot];
                let reference = match self.frames.last() {
                    Some(Frame::MoveAll(batch)) => self.batch_reference(batch, target.card),
                    _ => unreachable!("anonymous batch caller"),
                }
                .unwrap_or_else(|| self.capture(target.card));
                self.constrain_anonymous_choice(reference, &group.excluded, group.chooser);
                self.push_task(Task::MoveAllReference(Box::new(reference.with(target))));
            }
            Task::OpeningMeld { player, card } => {
                let Some(Frame::Opening(opening)) = self.frames.last_mut() else {
                    unreachable!("opening selection caller");
                };
                opening.picks[player.index()] = Some(card);
            }
            Task::BlockItem(index) => self.choose_block_item(index),
            Task::BlockBlind(indices) => {
                let index = if indices.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % indices.len() as u64) as usize
                };
                self.choose_block_item(Some(indices[index]));
            }
            Task::BlockCard(card) => self.choose_block_card(card),
            Task::BindReference(card) => {
                let Some(Frame::Program { retained, .. }) = self.frames.last_mut() else {
                    unreachable!("retained binding caller");
                };
                *retained = Some(card);
            }
            Task::BindCard(card) => {
                let card = self.capture(card);
                let Some(Frame::Program { retained, .. }) = self.frames.last_mut() else {
                    unreachable!("retained selection caller");
                };
                *retained = Some(card);
            }
            Task::JunkSpecial(special) => {
                let Some(Frame::JunkAvailableSpecials {
                    context,
                    considered,
                    ..
                }) = self.frames.last_mut()
                else {
                    unreachable!("special junk caller");
                };
                *considered |= 1 << special.index();
                let context = *context;
                self.frames.push(Frame::operation(
                    Operation::JunkAvailableSpecial { special },
                    context,
                ));
            }
            Task::DecreeValue(value) => self.select_decree_value(value),
            Task::DecreeReturn(target) => self.return_decree_card(target),
            Task::DecreeBlindReturn(group) => {
                let slot = if group.targets.len() == 1 {
                    0
                } else {
                    (self.rng.next_u64() % group.targets.len() as u64) as usize
                };
                self.return_decree_card(group.targets[slot]);
            }
            Task::FadeCard { target } => {
                let reference = self.capture(target.card);
                let Some(Frame::Fading(fading)) = self.frames.last_mut() else {
                    unreachable!("fading caller");
                };
                fading.visited.push(reference);
                let context = fading.context;
                let request = self.movement(
                    Verb::Score,
                    target,
                    Destination::Score(Who::Executor),
                    context,
                );
                self.frames
                    .push(Frame::referenced_operation(request, context, reference));
            }
            Task::Decline => self.complete(ReturnValue::Operation(OperationOutcome::empty(
                Disposition::Inapplicable,
            ))),
            Task::Showcase { card, auspice } => {
                if let Err(error) = self.start_showcase_dogma(card, auspice) {
                    self.fault = Some(error);
                }
            }
            Task::DrawFrom(draw) => {
                self.draw_from(
                    draw.value,
                    draw.set,
                    draw.count,
                    draw.then,
                    false,
                    draw.context,
                );
            }
            Task::MoveAllCard { target } => {
                let Some(Frame::MoveAll(batch)) = self.frames.last() else {
                    unreachable!("batch")
                };
                let reference = self.batch_reference(batch, target.card);
                self.take_batch_card(target, reference);
            }
            Task::MoveAllReference(reference) => {
                self.take_batch_card(reference.inner, Some(reference.with(reference.card)));
            }
            Task::StopMoves => {
                let Some(Frame::MoveAll(batch)) = self.frames.pop() else {
                    unreachable!("optional bulk movement caller");
                };
                self.complete(batch.outcome.returned());
            }
            Task::SearchReturn(card) => self.choose_search_return(card),
            Task::Endorse(endorse) => self.choose_endorsement(endorse),
            Task::DogmaIcon(icon) => self.choose_dogma_icon(icon),
            Task::TakeSelected { index } => {
                let Some(Frame::MoveSelected {
                    verb,
                    targets,
                    context,
                }) = self.frames.last_mut()
                else {
                    unreachable!("selected-card batch caller");
                };
                // Consume the selected reference before dispatch, including
                // if a future operation is prevented or becomes inapplicable.
                let reference = targets.remove(index);
                let (verb, context) = (*verb, *context);
                let target = reference.inner;
                let card = self.position.card(target.card);
                let destination = match verb {
                    Verb::Tuck => Zone::Board(context.executor, card.color),
                    Verb::Return => Zone::Deck(card.origin_deck()),
                    _ => unreachable!("selected-card verb"),
                };
                self.frames.push(Frame::captured_operation(
                    Operation::Move {
                        verb,
                        target,
                        destination,
                    },
                    context,
                    reference.with(target.card),
                ));
            }
            Task::NameFigure { deck, context } => {
                self.frames.push(Frame::NameFigure { deck, context })
            }
            Task::FindFigure {
                deck,
                name,
                context,
            } => self.frames.push(Frame::FindFigure {
                deck,
                name,
                context,
            }),
            Task::Operation(op, ctx) => self.frames.push(Frame::operation(op, ctx)),
            Task::CapturedOperation(op, ctx, reference) => {
                self.frames
                    .push(Frame::captured_operation(op, ctx, reference));
            }
            Task::SelfExecute { card, context } => {
                self.push_copied(card, ExecutionMode::SelfExecute, context);
            }
            Task::Hook(hook) => {
                let phase = match self.frames.last_mut() {
                    Some(Frame::Operation { phase, .. } | Frame::Exchange { phase, .. }) => phase,
                    _ => unreachable!("hook caller"),
                };
                *phase = match hook.reaction {
                    Reaction::First => Phase::Commit,
                    Reaction::Instead => Phase::Replaced,
                    Reaction::Melded => unreachable!("When is dispatched after commit"),
                };
                self.frames.push(Frame::Karma {
                    hook,
                    stage: 0,
                    visited: Vec::new(),
                    last: None,
                });
            }
        }
    }

    fn resume(&mut self, frame: Frame, events: &mut Vec<Event>) -> Result<(), Error> {
        match frame {
            Frame::TerminalEffect(progress) => self.resume_terminal_effect(progress, events)?,
            Frame::Opening(opening) => self.resume_opening(opening, events)?,
            Frame::Subgame(step) => self.resume_subgame(step, events)?,
            Frame::ActionReturns { last, .. } => self.complete(last),
            Frame::JunkAvailableSpecials {
                context,
                considered,
            } => {
                self.resume_junk_available_specials(context, considered);
            }
            Frame::Decree(progress) => self.resume_decree(progress)?,
            Frame::DecreePayment { context, allowed } => {
                self.resume_decree_payment(context, allowed)
            }
            Frame::ReturnOutcome(outcome) => self.complete(outcome.returned()),
            Frame::KarmaProof { saved, revealed } => self.resume_karma_proof(saved, revealed),
            Frame::MeldAction(meld) => self.resume_meld_action(meld)?,
            Frame::Showcase(showcase) => self.resume_showcase(showcase)?,
            Frame::MuseumCheck(check) => self.resume_museum_check(check)?,
            Frame::CitySearch(search) => self.resume_city_search(search, events)?,
            Frame::DogmaPrep(prep) => self.resume_dogma_prep(prep, events)?,
            Frame::Promote(promotion) => self.resume_promotion(promotion)?,
            Frame::ActionTail {
                action,
                context,
                kind,
                advance_turn,
                stage,
                last,
                shared,
                endorsed,
                melded,
                promoted,
            } => {
                let player = context.executor;
                let melded = melded
                    || (stage == 0
                        && matches!(action, Action::Meld { .. })
                        && last.is_some_and(|o| o.disposition == Disposition::Performed));
                if stage == 3 {
                    if advance_turn
                        && kind == ActionKind::Regular
                        && self.position.regular_actions_started
                            >= self.position.regular_action_limit
                    {
                        // The finished action's memory retires with it: the
                        // next player's turn-start procedure (a showcase)
                        // runs above this action's frames and must not
                        // inherit its private flow.
                        if let Some(Frame::ActionReturns { memory, .. }) = self.frames.last_mut() {
                            *memory = None;
                        }
                        self.position.begin_turn(player.opponent());
                        if self.narrated {
                            events.push(Event::TurnBegan {
                                player: player.opponent(),
                            });
                        }
                        self.start_turn_procedure(player.opponent());
                    }
                    self.complete(ReturnValue::Unit);
                } else {
                    self.frames.push(Frame::ActionTail {
                        action,
                        context,
                        kind,
                        advance_turn,
                        stage: stage + 1,
                        last: None,
                        shared,
                        endorsed,
                        melded,
                        promoted,
                    });
                    match stage {
                        0 if self.position.figures()
                            && matches!(action, Action::Achieve { .. })
                            && last.is_some_and(|o| o.disposition == Disposition::Performed) =>
                        {
                            // This is an effect draw, not another action. The
                            // recipient uses their own top value; active-player
                            // identity and action ordinals remain unchanged.
                            let recipient = player.opponent();
                            self.frames.push(Frame::operation(
                                Operation::Draw {
                                    value: self.position.highest_top_value(recipient),
                                    set: Some(CardSet::Figures),
                                },
                                context.rules_for(recipient),
                            ));
                        }
                        0 => {}
                        1 => self.frames.push(Frame::EndAction { context, stage: 0 }),
                        2 if let Action::Dogma { card, auspice } = action
                            && (shared || self.position.watched() & On::NoShareDraw.bit() != 0) =>
                        {
                            // Drawing or not drawing for sharing follows Dogma
                            // action end, before the regular turn flips.
                            let icon = if auspice {
                                Icon::Avatar
                            } else {
                                self.position.card(card).featured.unwrap_or(Icon::Avatar)
                            };
                            self.frames.push(Frame::operation(
                                Operation::ShareBonus {
                                    card,
                                    icon,
                                    earned: shared,
                                },
                                context,
                            ));
                        }
                        // Echoes promotion is step 9, after the action-end
                        // checks of step 6-8.
                        2 if let Action::Meld { card } = action
                            && melded
                            && self.position.expansions().echoes() =>
                        {
                            self.frames
                                .push(Frame::Promote(Box::new(echoes::Promotion::new(
                                    card, context,
                                ))));
                        }
                        2 => {}
                        _ => unreachable!("action-tail stage"),
                    }
                }
            }
            Frame::EndAction { context, stage } => {
                let active = context.executor;
                if stage == 4 {
                    self.complete(ReturnValue::Unit);
                } else {
                    self.frames.push(Frame::EndAction {
                        context,
                        stage: stage + 1,
                    });
                    match stage {
                        0 | 1 if self.position.figures() => {
                            let player = if stage == 0 {
                                active.opponent()
                            } else {
                                active
                            };
                            self.frames.push(Frame::fading(context.rules_for(player)));
                        }
                        0 | 1 => {}
                        2 => self.frames.push(Frame::EndSpecials {
                            context,
                            considered: 0,
                        }),
                        3 => self.frames.push(Frame::operation(
                            Operation::CheckAchievementVictory,
                            context,
                        )),
                        _ => unreachable!("action-end stage"),
                    }
                }
            }
            Frame::EndSpecials {
                context,
                considered,
            } => {
                let active = context.executor;
                // Re-query after each claim and its reactions. Each player's
                // claim of a token is attempted once even if replaced.
                let next = [active.opponent(), active].into_iter().find_map(|player| {
                    self.position.specials().find_map(|special| {
                        let bit = 1 << (player.index() * Special::RESERVED + special.index());
                        (considered & bit == 0
                            && self.position.eligible_for_special(player, special))
                        .then_some((player, special, bit))
                    })
                });
                if let Some((player, special, bit)) = next {
                    self.frames.push(Frame::EndSpecials {
                        context,
                        considered: considered | bit,
                    });
                    self.frames.push(Frame::operation(
                        Operation::ClaimSpecial {
                            special,
                            check_eligibility: true,
                        },
                        context.rules_for(player),
                    ));
                } else {
                    self.complete(ReturnValue::Unit);
                }
            }
            Frame::Fading(fading) => self.resume_fading(fading)?,
            Frame::DrawCount(mut batch) => {
                if batch.stage == 1
                    && let Some(drawn) = batch
                        .last
                        .operation()
                        .filter(|result| result.disposition == Disposition::Performed)
                {
                    if drawn.short {
                        // Drawn above its age: it did not fully happen.
                        batch.outcome.inner.inapplicable += 1;
                    }
                    if let Some(card) = drawn.card_ref() {
                        self.record_kept(card, drawn.value);
                    }
                }
                if batch.stage == 1 && batch.then == Then::Reveal {
                    if let Some(card) = batch
                        .last
                        .operation()
                        .filter(|result| result.disposition == Disposition::Performed)
                        .and_then(|result| result.card)
                    {
                        let bit = 1 << self.position.card(card).color as u8;
                        batch.repeated_color |= batch.colors & bit != 0;
                        batch.colors |= bit;
                        if let Some(physical) = self.position.locate(card) {
                            let context = batch.context;
                            batch.stage = 2;
                            self.frames.push(Frame::DrawCount(batch));
                            self.frames.push(Frame::operation(
                                Operation::Reveal {
                                    target: Target {
                                        card,
                                        physical,
                                        logical: LogicalZone::Physical(physical),
                                    },
                                },
                                context,
                            ));
                            return Ok(());
                        }
                    }
                } else if batch.stage == 1 && matches!(batch.then, Then::Score | Then::Achieve) {
                    let target = batch
                        .last
                        .operation()
                        .filter(|result| result.disposition == Disposition::Performed)
                        .and_then(|result| result.card)
                        .and_then(|card| {
                            self.position
                                .targets(LogicalZone::Hand(batch.context.executor))
                                .find(|target| target.card == card)
                        });
                    if let Some(target) = target {
                        let context = batch.context;
                        let request = if batch.then == Then::Score {
                            self.movement(
                                Verb::Score,
                                target,
                                Destination::Score(Who::Executor),
                                context,
                            )
                        } else {
                            self.movement(
                                Verb::Achieve,
                                target,
                                Destination::Achievements(Who::Executor),
                                context,
                            )
                        };
                        batch.stage = 2;
                        self.frames.push(Frame::DrawCount(batch));
                        self.frames.push(Frame::operation(request, context));
                        return Ok(());
                    }
                    batch.outcome.record(batch.last.operation().map_or(
                        Disposition::Inapplicable,
                        |result| {
                            if result.disposition == Disposition::Performed {
                                Disposition::Inapplicable
                            } else {
                                result.disposition
                            }
                        },
                    ));
                } else if batch.stage != 0 {
                    batch.outcome.record(
                        batch
                            .last
                            .operation()
                            .map_or(Disposition::Inapplicable, |result| result.disposition),
                    );
                }
                if batch.remaining == 0 {
                    self.complete(if batch.then == Then::Reveal {
                        ReturnValue::DrawColors {
                            colors: batch.colors,
                            repeated: batch.repeated_color,
                        }
                        .into()
                    } else {
                        batch.outcome.returned()
                    });
                } else {
                    batch.remaining -= 1;
                    batch.stage = 1;
                    batch.last = HeldReturn::UNIT;
                    let context = batch.context;
                    let request = Operation::Draw {
                        value: batch.value,
                        set: batch.set,
                    };
                    self.frames.push(Frame::DrawCount(batch));
                    self.frames.push(Frame::operation(request, context));
                }
            }
            Frame::MatchingPair(pair) => self.resume_matching_pair(pair)?,
            Frame::SelectGroup(group) => self.resume_group(group)?,
            Frame::JunkAvailable(batch) => self.resume_junk_available(batch)?,
            Frame::RevealAll(packet) => self.resume_reveal_packet(packet),
            Frame::SplayAll(batch) => self.resume_splay_all(batch)?,
            Frame::MoveAll(mut batch) => {
                // One child completed since the last resume.
                if batch.outcome.performed > batch.kept {
                    batch.kept = batch.outcome.performed;
                    if let Some(card) = batch.outcome.card {
                        let before = batch.outcome.value;
                        self.record_kept(batch.outcome.with(card), before);
                    }
                }
                // Once a full Forecast or Safe would refuse every remaining
                // card, before any karma could react, an "any number" batch
                // ends as if its player stopped. Mandatory batches still
                // attempt (and count) each refused card.
                let full = |game: &Self| {
                    let zone = match batch.to {
                        Destination::Forecast(who) => {
                            game.one_player(who, batch.context).map(Zone::Forecast)
                        }
                        Destination::Safe(who) => {
                            game.one_player(who, batch.context).map(Zone::Safe)
                        }
                        _ => return false,
                    };
                    zone.is_ok_and(|zone| !game.position.has_room(zone))
                };
                if batch
                    .limit
                    .is_some_and(|n| batch.visited.len() >= usize::from(n))
                    || (batch.stop.any_time() && full(self))
                {
                    self.complete(batch.outcome.returned());
                    return Ok(());
                }
                // A per-value batch is one return attempt per cap, not an
                // "all cards" sweep: a card a karma put back can be attempted
                // again while its value's cap lasts.
                let verified_sweep = batch.verify && batch.when.is_none()
                    // Capacity may admit only a proper subset. Keep its blind
                    // order choices and verify each admitted operation instead.
                    && !matches!(batch.to, Destination::Forecast(_) | Destination::Safe(_))
                    && batch.limit.is_none()
                    && batch.quotas.is_none()
                    && !batch.per_color
                    && batch.stop == Stop::Never;
                let excluded = Excluded {
                    cards: if batch.quotas.is_some() {
                        &[]
                    } else {
                        &batch.visited
                    },
                    colors: batch.colors_done,
                    // Verification authorizes matching, but never a menu of
                    // unseen fronts: disclosures below happen before choices.
                    blind: verified_sweep,
                    skip_junk: batch.to == Destination::Junk,
                };
                let mut targets = if batch.frozen_selection {
                    let mut targets = Vec::new();
                    for original in batch.frozen.as_ref().expect("frozen selection") {
                        if !contains_card(&batch.visited, original.card)
                            && excluded.colors
                                & (1 << self.position.card(original.card).color as usize)
                                == 0
                            && self.valid_source(original.inner)?
                        {
                            targets.push(original.inner);
                        }
                    }
                    targets
                } else {
                    match self.select(batch.from, batch.context, batch.regs, excluded) {
                        Err(Error::Unsupported("private filtered card selection"))
                            if batch.limit.is_none() && batch.stop == Stop::Never =>
                        {
                            let mut targets = self.select(
                                batch.from,
                                batch.context,
                                batch.regs,
                                Excluded {
                                    blind: true,
                                    ..excluded
                                },
                            )?;
                            targets.truncate(1);
                            targets
                        }
                        targets => targets?,
                    }
                };
                // Every candidate may become one visited reference; reserve
                // once instead of growing the record one card at a time.
                batch.visited.reserve(targets.len());
                if let Some(frozen) = &batch.frozen {
                    targets
                        .retain(|target| frozen.iter().any(|original| original.inner == *target));
                    let usable_colors = targets.iter().fold(0_u8, |colors, target| {
                        colors | (1 << self.position.card(target.card).color as usize)
                    });
                    for original in frozen {
                        if contains_card(&batch.visited, original.card)
                            || targets.contains(&original.inner)
                        {
                            continue;
                        }
                        if batch.frozen_selection {
                            let color = 1 << self.position.card(original.card).color as usize;
                            // Unselected peers are alternatives, not obligations. A
                            // chosen original still counts through the normal outcome
                            // path if First or Instead prevents its operation.
                            if batch.stop.any_time()
                                || batch.colors_done & color != 0
                                || batch.per_color && usable_colors & color != 0
                            {
                                continue;
                            }
                            let usable = if batch.per_color {
                                usable_colors.count_ones() as usize
                            } else {
                                targets.len()
                            };
                            if batch.limit.is_some_and(|limit| {
                                batch.visited.len() + usable >= usize::from(limit)
                            }) {
                                continue;
                            }
                            // No original can satisfy this still-required color.
                            // Count its deficit once, regardless of its lost peers.
                            if batch.per_color {
                                batch.colors_done |= color;
                            }
                        }
                        batch.visited.push(original.with(original.card));
                        batch.outcome.record(Disposition::Inapplicable);
                    }
                }
                if verified_sweep
                    && let Some(target) = targets.iter().copied().find(|target| {
                        !self
                            .position
                            .can_view(target.physical, batch.context.controller)
                            && !self
                                .batch_reference(&batch, target.card)
                                .is_some_and(|held| {
                                    self.reference_visible(held, batch.context.controller)
                                })
                    })
                {
                    let context = batch.context;
                    let operation =
                        self.program_movement(batch.verb, batch.from, target, batch.to, context);
                    let destination = match operation {
                        Operation::Move { destination, .. } => destination,
                        Operation::Claim { .. } => Zone::Achievements(context.executor),
                        _ => unreachable!("verified batch movement"),
                    };
                    if (batch.verb == Verb::Transfer && target.physical == destination)
                        || !self.valid_source(target)?
                        || !self.position.has_room(destination)
                        || (matches!(operation, Operation::Claim { .. })
                            && !self.claim_is_applicable(operation, context)?)
                    {
                        // A refused card needs no proof and cannot create an
                        // ordering choice or reaction before admission.
                        self.frames.push(Frame::MoveAll(batch));
                        self.take_batch_card(target, None);
                    } else {
                        let reference = self
                            .batch_reference(&batch, target.card)
                            .unwrap_or_else(|| self.capture(target.card));
                        batch.proving = true;
                        self.frames.push(Frame::MoveAll(batch));
                        self.frames.push(Frame::Operation {
                            request: Operation::Reveal { target },
                            context,
                            phase: Phase::Discover,
                            knowledge: Some(OperandKnowledge {
                                evidence: reference.evidence,
                                conditioned: false,
                                private_query: None,
                                verify: false,
                            }),
                            short: false,
                        });
                    }
                    return Ok(());
                }
                if let Some(quotas) = &batch.quotas {
                    targets.retain(|target| {
                        quotas
                            .get(usize::from(self.position.value(target.card)))
                            .is_some_and(|&quota| quota > 0)
                    });
                }
                self.reduce_batch_moves(&mut batch, &mut targets);
                let (mut options, mut tasks) = self.batch_move_choices(&batch, targets);
                if tasks.is_empty() {
                    self.complete(batch.outcome.returned());
                } else {
                    // Declining before the first card is a "may"; stopping a
                    // quantity after it is not (Blackmail keeps it).
                    if batch
                        .stop
                        .may_stop(batch.visited.is_empty(), batch.context.must())
                    {
                        options.push(Alternative::Decline);
                        tasks.push(Task::StopMoves);
                    }
                    let context = batch.context;
                    self.frames.push(Frame::MoveAll(batch));
                    self.frames.push(Frame::Choose {
                        player: context.controller,
                        tasks,
                        options: DecisionOptions::new(options, context),
                    });
                }
            }
            Frame::SelectExchange(selection) => {
                self.resume_exchange_selection(selection, events)?
            }
            Frame::Exchange { plan, phase } => {
                let outcome = match phase {
                    Phase::Replaced => OperationOutcome::empty(Disposition::Replaced),
                    Phase::Commit => {
                        self.finish_exchange_plan(*plan, events)?;
                        return Ok(());
                    }
                    Phase::Discover => unreachable!("hook selection resolves the exchange phase"),
                    Phase::Retained => unreachable!("only physical child moves retain identity"),
                };
                self.complete(ReturnValue::Operation(outcome));
            }
            Frame::MoveSelected {
                verb,
                targets,
                context,
            } => self.resume_move_selected(verb, targets, context)?,
            Frame::NameFigure { deck, context } => self.resume_name_figure(deck, context),
            Frame::FindFigure {
                deck,
                name,
                context,
            } => self.resume_find_figure(deck, name, context, events),
            Frame::Operation {
                mut request,
                mut context,
                phase,
                mut knowledge,
                mut short,
            } => {
                if let Operation::Move {
                    verb: Verb::Transfer,
                    target,
                    destination,
                } = request
                    && phase == Phase::Discover
                    && target.physical == destination
                {
                    // Transfer requires another physical area. A logical alias
                    // cannot transfer a card to the place it already occupies.
                    self.complete(ReturnValue::Operation(OperationOutcome::empty(
                        Disposition::Inapplicable,
                    )));
                    return Ok(());
                }
                if let Operation::Move {
                    target,
                    destination,
                    ..
                } = request
                    && phase == Phase::Discover
                    && target.physical != destination
                    && !self.position.has_room(destination)
                {
                    // Capacity prevents the move before any karma sees it; the card
                    // stays at its source.
                    self.complete(ReturnValue::Operation(OperationOutcome::empty(
                        Disposition::Inapplicable,
                    )));
                    return Ok(());
                }
                if phase == Phase::Discover && knowledge.is_some_and(|known| known.verify) {
                    let target = match request {
                        Operation::Move { target, .. } | Operation::Claim { target, .. } => target,
                        _ => unreachable!("verified movement"),
                    };
                    if !self.valid_source(target)?
                        || (matches!(request, Operation::Claim { .. })
                            && !self.claim_is_applicable(request, context)?)
                    {
                        self.complete(ReturnValue::Operation(OperationOutcome::empty(
                            Disposition::Inapplicable,
                        )));
                        return Ok(());
                    }
                    let mut verified = knowledge.expect("verification marker");
                    verified.verify = false;
                    // Board movements already supply their printed witness.
                    let hidden = !Player::ALL
                        .into_iter()
                        .all(|viewer| self.position.can_view(target.physical, viewer));
                    if hidden {
                        let revealing = verified;
                        verified.evidence |= Evidence::PUBLIC;
                        self.frames.push(Frame::Operation {
                            request,
                            context,
                            phase,
                            knowledge: Some(verified),
                            short,
                        });
                        self.frames.push(Frame::Operation {
                            request: Operation::Reveal { target },
                            context,
                            phase: Phase::Discover,
                            knowledge: Some(revealing),
                            short: false,
                        });
                        return Ok(());
                    }
                    knowledge = Some(verified);
                }
                if let Operation::ExchangeAll { left, right } = request {
                    let plan = self.plan_exchange(left, right, context)?;
                    self.start_exchange_plan(plan, left, right, context, events)?;
                    return Ok(());
                }
                if let Operation::Splay {
                    player,
                    color,
                    direction,
                } = request
                    && phase == Phase::Discover
                    && !self.can_splay(player, color, direction)
                {
                    self.complete(ReturnValue::Operation(OperationOutcome::empty(
                        Disposition::Inapplicable,
                    )));
                    return Ok(());
                }
                if let Operation::RequireFade { player } = request
                    && phase == Phase::Discover
                    && (!self.position.figures() || self.position.top_figures(player).count() <= 1)
                {
                    self.complete(ReturnValue::Operation(OperationOutcome::empty(
                        Disposition::Inapplicable,
                    )));
                    return Ok(());
                }
                if let Operation::ExecuteEffect { card, effect, .. } = request
                    && self
                        .position
                        .catalog
                        .get(usize::from(card.0))
                        .is_none_or(|definition| {
                            definition.figure.is_some() || definition.effect(effect).is_none()
                        })
                {
                    return Err(Error::InvalidAction);
                }
                if let Operation::Dogma { card, .. } = request
                    && phase == Phase::Discover
                    // An endorsed preparation admitted it before payment.
                    && context.source_card != Some(card)
                {
                    // A karma that re-issues the Dogma (Kepler) carries its
                    // own source; the showcased Artifact is still admitted.
                    context.source_owner = self
                        .position
                        .dogma_source_owner(context.executor, card)
                        .or_else(|| {
                            self.showcased_source(context.executor, card)
                                .then_some(context.executor)
                        })
                        .ok_or(Error::InvalidAction)?;
                    context.source_card = Some(card);
                    context.source_evidence = Evidence::PUBLIC; // Dogma sources are public tops.
                }
                if self.position.figures() && phase == Phase::Discover {
                    if let Operation::Move { target, .. } = request
                        && !self.valid_source(target)?
                    {
                        self.complete(ReturnValue::Operation(OperationOutcome::empty(
                            Disposition::Inapplicable,
                        )));
                        return Ok(());
                    }
                    if let Operation::Move { target, .. } = request
                        && knowledge.is_none()
                    {
                        knowledge = Some(OperandKnowledge {
                            evidence: self.capture(target.card).evidence,
                            conditioned: false,
                            private_query: None,
                            verify: false,
                        });
                    }
                }
                if let Operation::Draw { value, set } = request
                    && phase != Phase::Replaced
                {
                    match self.draw_deck(value, set, context.executor)? {
                        // Exhaustion is decided before draw karma: they see
                        // only a draw with a card to take. Proposed terminal
                        // outcomes still run through Would, retaining
                        // suppression if exhaustion occurred in karma.
                        None => {
                            let request = self.exhaustion_request();
                            self.frames.push(Frame::operation(request, context));
                            return Ok(());
                        }
                        // A First reaction does not reset the age: the draw
                        // continues from the age resolved here, escalating if
                        // that Base deck empties. The set is re-derived from
                        // the original request at commit.
                        Some(deck) => {
                            short |= deck.age != value;
                            request = Operation::Draw {
                                value: deck.age,
                                set,
                            }
                        }
                    }
                }
                if phase == Phase::Discover
                    && !context.suppress_karma()
                    && self.position.watched() & Self::trigger_kinds(request) != 0
                {
                    if knowledge.is_none()
                        && let Some(card) = self.operand(request, context.executor)?
                    {
                        // Every clause's event shares this operand's pick.
                        knowledge = Some(OperandKnowledge {
                            evidence: self.capture(card).evidence,
                            conditioned: false,
                            private_query: None,
                            verify: false,
                        });
                    }
                    let (hooks, discovered, proof) = self.hooks(request, context, knowledge)?;
                    knowledge = discovered;
                    // A condition reading a hidden operand's front: reveal it
                    // first, then rediscover with the card public.
                    if proof
                        && let Operation::Move { target, .. } | Operation::Claim { target, .. } =
                            request
                        && let Some(mut known) = knowledge
                        && !Player::ALL.into_iter().all(|viewer| {
                            known.evidence.known(viewer)
                                || self.position.can_view(target.physical, viewer)
                        })
                    {
                        let revealing = known;
                        known.evidence |= Evidence::PUBLIC;
                        self.frames.push(Frame::Operation {
                            request,
                            context,
                            phase,
                            knowledge: Some(known),
                            short,
                        });
                        self.frames.push(Frame::Operation {
                            request: Operation::Reveal { target },
                            context,
                            phase: Phase::Discover,
                            knowledge: Some(revealing),
                            short: false,
                        });
                        return Ok(());
                    }
                    if !hooks.is_empty() {
                        self.frames.push(Frame::Operation {
                            request,
                            context,
                            phase,
                            knowledge,
                            short,
                        });
                        self.frames.push(Frame::Choose {
                            player: self.position.turn_owner,
                            options: DecisionOptions::new(
                                hooks
                                    .iter()
                                    .map(|h| Alternative::Karma {
                                        source: h.source,
                                        figure: h.figure,
                                    })
                                    .collect(),
                                context,
                            ),
                            tasks: hooks.into_iter().map(Task::Hook).collect(),
                        });
                        return Ok(());
                    }
                }
                if phase != Phase::Replaced {
                    if let Operation::ExecuteEffect {
                        card,
                        effect,
                        boxed,
                    } = request
                    {
                        let mut context = context;
                        let evidence = self.capture(card).evidence
                            | if context.source_card == Some(card) {
                                context.source_evidence
                            } else {
                                Evidence::NONE
                            };
                        context.source_card = Some(card);
                        context.source_evidence = evidence;
                        self.frames.push(Frame::Program {
                            source: Source::Effect { card, effect },
                            pc: 0,
                            context,
                            last: HeldReturn::UNIT,
                            retained: None,
                            chosen: SelectionParameter::None,
                            boxed,
                        });
                        return Ok(());
                    }
                    if let Operation::ShareBonus { earned: true, .. } = request {
                        // Sharing earns a full free Draw action.
                        self.push_action_returns();
                        self.frames.push(Frame::ActionTail {
                            action: Action::Draw,
                            context,
                            kind: ActionKind::Free,
                            advance_turn: false,
                            stage: 0,
                            last: None,
                            shared: false,
                            endorsed: false,
                            melded: false,
                            promoted: false,
                        });
                        self.frames.push(Frame::operation(
                            Operation::DrawAction {
                                set: if self.position.figures() {
                                    CardSet::Figures
                                } else {
                                    CardSet::Base
                                },
                            },
                            context,
                        ));
                        return Ok(());
                    }
                    if let Operation::DrawAction { set } = request {
                        // The draw inside the action starts only now, after
                        // the action's own reactions, so it reads the live
                        // highest top and discovers its own draw karma.
                        let value = self.position.highest_top_value(context.executor);
                        let set = match set {
                            CardSet::Base => None,
                            set => Some(set),
                        };
                        self.frames
                            .push(Frame::operation(Operation::Draw { value, set }, context));
                        return Ok(());
                    }
                    if let Operation::IssueDecree { decree } = request {
                        if !self.position.figures() {
                            return Err(Error::Unsupported("Decrees require Figures"));
                        }
                        if self.position.decree_location(decree)
                            == DecreeLocation::Owned(context.executor.opponent())
                        {
                            self.frames
                                .push(Frame::Decree(Box::new(DecreeProgress::return_only(
                                    decree, context,
                                ))));
                            self.frames.push(Frame::operation(
                                Operation::ReturnDecree { decree },
                                context,
                            ));
                        } else {
                            let claim =
                                self.position.decree_location(decree) == DecreeLocation::Supply;
                            self.frames.push(Frame::Decree(Box::new(DecreeProgress::new(
                                decree, context, claim,
                            ))));
                        }
                        return Ok(());
                    }
                    if let Operation::Dogma { card, icon } = request {
                        let player = context.executor;
                        let sharing = match self.action_majority() {
                            Some(holder) => holder != player,
                            None => self.opponent_shares(player, card, icon),
                        };
                        // Eligibility locks here, after the Dogma's First
                        // reactions; ineligibility is itself a Would event.
                        let eligibility = !sharing
                            && !context.suppress_karma()
                            && self.position.watched() & On::Ineligible.bit() != 0;
                        let endorsed = matches!(
                            self.frames.last(),
                            Some(Frame::ActionTail { endorsed: true, .. })
                        );
                        let kind = match self.frames.last() {
                            Some(Frame::ActionTail {
                                kind: ActionKind::Regular,
                                ..
                            }) => DogmaKind::Action,
                            Some(Frame::ActionTail { promoted: true, .. }) => DogmaKind::Promoted,
                            _ if self.showcased_source(player, card) => DogmaKind::Showcased,
                            _ => DogmaKind::Free,
                        };
                        if self.narrated {
                            events.push(Event::DogmaStarted {
                                player,
                                card,
                                auspice: icon == Icon::Avatar,
                                kind,
                            });
                        }
                        let extra = self.echo_schedule(card);
                        self.frames.push(Frame::Dogma(Dogma {
                            card,
                            player,
                            source_owner: context.source_owner,
                            effect: 0,
                            participant: 0,
                            endorsed,
                            sharing,
                            vulnerable: !sharing,
                            eligibility,
                            shared: false,
                            executing_shared: false,
                            demand_transfers: 0,
                            demand_returns: 0,
                            suppress_karma: context.suppress_karma(),
                            boxed_action: context.boxed_action,
                            extra,
                        }));
                        if eligibility {
                            self.frames.push(Frame::operation(
                                Operation::Ineligible {
                                    player: player.opponent(),
                                },
                                context,
                            ));
                        }
                        return Ok(());
                    }
                    if let Operation::CheckAchievementVictory = request
                        && let Some(winner) = self.achievement_winner()
                    {
                        self.frames.push(Frame::operation(
                            Operation::ProposeWin {
                                winner,
                                reason: WinReason::Achievements,
                            },
                            context,
                        ));
                        return Ok(());
                    }
                }
                // The operand is the pick its Would event saw, if any: the
                // same card by construction, so it keeps that pick.
                let selected = match request {
                    Operation::Move { target, .. }
                    | Operation::Claim { target, .. }
                    | Operation::Reveal { target } => Some(
                        match knowledge.and_then(|knowledge| knowledge.evidence.pick()) {
                            Some(pick) => Captured::new(
                                target.card,
                                knowledge
                                    .expect("selected operand evidence")
                                    .evidence
                                    .with_pick(Some(pick)),
                            ),
                            None => self.capture(target.card),
                        },
                    ),
                    _ => None,
                };
                if let Some(knowledge) = &mut self.knowledge {
                    if let Some(reference) = selected
                        && let Some(pick) = reference.evidence.pick()
                    {
                        for tracker in &mut knowledge.trackers {
                            let named = tracker.viewer().map_or_else(
                                || {
                                    Player::ALL
                                        .into_iter()
                                        .all(|viewer| reference.evidence.known(viewer))
                                },
                                |viewer| reference.evidence.known(viewer),
                            );
                            if named {
                                tracker.identify_pick(&self.position, pick, reference.inner);
                            }
                        }
                    }
                    knowledge.moving = selected.and_then(|selected| selected.evidence.pick());
                    knowledge.moved = None;
                }
                let mut outcome = if phase == Phase::Replaced {
                    OperationOutcome::empty(Disposition::Replaced)
                } else {
                    // First preserves the selected meld; retained suppressed
                    // children can also address an exact buried physical card.
                    let retain = phase == Phase::Retained
                        || (phase == Phase::Commit
                            && matches!(
                                request,
                                Operation::Move {
                                    verb: Verb::Meld,
                                    ..
                                }
                            ));
                    let departure = knowledge
                        .filter(|knowledge| knowledge.evidence.positional())
                        .map_or(tracking::Departure::Unordered, |knowledge| {
                            tracking::Departure::Retained(knowledge.evidence.pick())
                        });
                    self.commit_departure(request, context, retain, departure, events)?
                };
                // Resolved above the requested age, at discovery or after a
                // First reaction emptied the resolved deck.
                outcome.short = short && matches!(request, Operation::Draw { .. });
                let moved = self.knowledge.as_mut().and_then(|knowledge| {
                    knowledge.moving = None;
                    knowledge.moved.take()
                });
                // The result is the card that arrived, under the pick its move
                // followed. Identity knowledge from its selection survives.
                let evidence = outcome.card.map_or(Evidence::NONE, |card| {
                    self.arrived(card, moved)
                        | selected.map_or(Evidence::NONE, |selected| selected.evidence)
                        // A Draw's Would event saw the old deck top. First
                        // may change it; that identity evidence must NOT flow
                        // to whichever card the original draw later takes.
                        | if selected.is_some() {
                            knowledge.map_or(Evidence::NONE, |knowledge| knowledge.evidence)
                        } else {
                            Evidence::NONE
                        }
                        | if matches!(request, Operation::Reveal { .. }) {
                            Evidence::PUBLIC
                        } else {
                            Evidence::NONE
                        }
                });
                // A result names the card at its new place. Only a reference
                // still awaiting its original departure retains that position.
                let evidence = if matches!(request, Operation::Reveal { .. }) {
                    evidence
                } else {
                    evidence.after_move()
                };
                let held_outcome = Captured::new(outcome, evidence);
                if outcome.disposition == Disposition::Performed
                    && matches!(
                        request,
                        Operation::Move { .. }
                            | Operation::Draw { .. }
                            | Operation::DrawTop { .. }
                            | Operation::Splay { .. }
                            | Operation::Claim { .. }
                            | Operation::ClaimSpecial { .. }
                            | Operation::JunkAvailableSpecial { .. }
                            | Operation::RestoreSpecial { .. }
                            | Operation::IntroduceAgeZero
                            | Operation::Shuffle { .. }
                    )
                {
                    self.note_shared_change(context);
                }
                if outcome.disposition == Disposition::Performed
                    && matches!(request, Operation::Reveal { .. })
                {
                    self.note_execution_change(true);
                }
                if outcome.disposition == Disposition::Performed
                    && let Operation::Move {
                        verb: verb @ (Verb::Transfer | Verb::Return),
                        ..
                    } = request
                {
                    self.note_demand_move(context, verb)?;
                    if verb == Verb::Return {
                        self.note_democracy_return(context)?;
                    }
                }
                if self.terminal.is_none() {
                    let expansions = self.position.expansions();
                    if expansions.cities() || expansions.artifacts() {
                        self.finish_city_operation(request, context, held_outcome);
                    } else if let Some(hook) = self.when_melded(request, context, outcome) {
                        self.frames.push(Frame::ReturnOutcome(held_outcome));
                        self.frames.push(Frame::Karma {
                            hook,
                            stage: 0,
                            visited: Vec::new(),
                            last: None,
                        });
                    } else {
                        self.complete(held_outcome.returned());
                    }
                }
            }
            Frame::Dogma(mut dogma) => {
                dogma.executing_shared = false;
                let Some((source, index, effect)) = dogma.current().and_then(|(source, index)| {
                    let effect = self.position.card(source).effect(index)?;
                    Some((source, index, effect))
                }) else {
                    self.complete(ReturnValue::Dogma {
                        shared: dogma.shared,
                    });
                    return Ok(());
                };
                if dogma.participant == if dogma.endorsed { 3 } else { 2 } {
                    dogma.effect += 1;
                    dogma.participant = 0;
                    self.frames.push(Frame::Dogma(dogma));
                    return Ok(());
                }
                let repeat = dogma.participant == 2;
                let demand = effect.demand();
                let opponent = dogma.participant == 0 || (repeat && demand);
                if dogma.participant == 0 && demand {
                    // One summary per original demand clause, across its
                    // endorsed repeat.
                    dogma.demand_transfers = 0;
                    dogma.demand_returns = 0;
                }
                // A compel is executed by an eligible sharer, never by the
                // owner, and earns no reward. Demands follow icon counts
                // alone, so Sargon's owner shares and stays vulnerable.
                let execute = match effect.kind {
                    EffectKind::Compel => opponent && dogma.sharing,
                    EffectKind::Demand => opponent && dogma.vulnerable,
                    EffectKind::NonDemand => !opponent || dogma.sharing,
                };
                let sharer = dogma.participant == 0 && !demand;
                dogma.participant += 1;
                if execute {
                    dogma.executing_shared = sharer;
                    let executor = if opponent {
                        dogma.player.opponent()
                    } else {
                        dogma.player
                    };
                    let mut context = Context::new(executor);
                    context.demander = dogma.player;
                    context.source_owner = dogma.source_owner;
                    // An echo's own card is its source, not the activated card.
                    context.source_card = Some(source);
                    context.source_evidence = Evidence::PUBLIC;
                    context.set_suppress_karma(dogma.suppress_karma);
                    context.boxed_action = dogma.boxed_action;
                    let program = Frame::effect(source, index, context);
                    self.frames.push(Frame::Dogma(dogma));
                    self.frames.push(program);
                } else {
                    self.frames.push(Frame::Dogma(dogma));
                }
            }
            Frame::Execute(execution) => self.resume_copied(execution, events)?,
            Frame::Locals(_) => unreachable!("a program's slots end with it"),
            Frame::Program {
                source,
                pc,
                context,
                last,
                retained,
                chosen,
                boxed,
            } => self.resume_program(
                source,
                pc,
                context,
                Regs {
                    last,
                    bound: retained,
                    chosen,
                    boxed,
                    locals: false,
                },
            )?,
            Frame::Karma {
                hook,
                stage,
                visited,
                last,
            } => {
                // Stage 0 is a clause's first entry; continuations are later.
                if stage == 0 && self.narrated {
                    let would = match (hook.reaction, self.frames.last()) {
                        (
                            Reaction::First | Reaction::Instead,
                            Some(Frame::Operation { request, .. }),
                        ) => Some(*request),
                        _ => None,
                    };
                    events.push(Event::KarmaTriggered {
                        owner: hook.owner,
                        card: hook.source,
                        clause: hook.clause,
                        reaction: hook.reaction,
                        actor: hook.event.actor,
                        would,
                    });
                }
                self.resume_karma(hook, stage, visited, last)?;
            }
            Frame::Choose { .. } => unreachable!("choices handled by advance"),
        }
        Ok(())
    }

    #[cold]
    #[inline(never)]
    fn resume_decree_payment(&mut self, context: Context, allowed: Vec<Decree>) {
        // Freeze permissions from the initial hand, but do not offer
        // the choice until all junk attempts and reactions finish.
        self.frames.push(Frame::Choose {
            player: context.controller,
            options: DecisionOptions::new(
                allowed.iter().copied().map(Alternative::Decree).collect(),
                context,
            ),
            tasks: allowed
                .into_iter()
                .map(|decree| Task::Operation(Operation::IssueDecree { decree }, context))
                .collect(),
        });
        // The whole hand goes to the unordered junk, as in Fission's sweep.
        let mut payment = MoveAll::new(
            Verb::Junk,
            Selector::Hand(Who::Executor),
            Destination::Junk,
            context,
        );
        payment.shortcut = Some(Shortcut::AnyOrder);
        self.frames.push(Frame::MoveAll(Box::new(payment)));
    }

    #[cold]
    #[inline(never)]
    fn resume_karma_proof(&mut self, mut saved: Box<Frame>, revealed: Option<HeldCard>) {
        if let Some(reference) = revealed {
            let Frame::Program {
                source, retained, ..
            } = saved.as_mut()
            else {
                unreachable!("karma proof continuation")
            };
            *retained = Some(reference);
            self.name_entered_karma_operand(*source, reference);
        }
        self.frames.push(*saved);
    }

    #[cold]
    #[inline(never)]
    fn resume_fading(&mut self, mut fading: Box<Fading>) -> Result<(), Error> {
        let context = fading.context;
        let player = context.executor;
        if fading.awaiting_requirement
            && !fading
                .last
                .is_some_and(|o| o.disposition == Disposition::Performed)
        {
            // Duke replaces the requirement itself. Do not ask for a
            // meaningless card choice, or repeatedly require fading.
            self.complete(ReturnValue::Unit);
            return Ok(());
        }
        if self.position.figure_rules(player).len() <= 1 {
            // Fading needs at least two top Figures.
            self.complete(ReturnValue::Unit);
            return Ok(());
        }
        let tops: Vec<_> = self.position.top_figures(player).collect();
        let targets: Vec<_> = tops
            .iter()
            .copied()
            .filter(|card| !contains_card(&fading.visited, *card))
            .map(|card| {
                let physical = Zone::Board(player, self.position.card(card).color);
                Target {
                    card,
                    physical,
                    logical: LogicalZone::Physical(physical),
                }
            })
            .collect();
        if tops.len() <= 1 || targets.is_empty() {
            self.complete(ReturnValue::Unit);
        } else if !fading.awaiting_requirement {
            fading.awaiting_requirement = true;
            fading.last = None;
            self.frames.push(Frame::Fading(fading));
            self.frames
                .push(Frame::operation(Operation::RequireFade { player }, context));
        } else {
            fading.awaiting_requirement = false;
            self.frames.push(Frame::Fading(fading));
            self.frames.push(Frame::Choose {
                player: context.controller,
                options: DecisionOptions::new(
                    targets
                        .iter()
                        .map(|&target| Alternative::Move {
                            verb: Verb::Score,
                            target: self.position.descriptor(target, context.controller),
                        })
                        .collect(),
                    context,
                ),
                tasks: targets
                    .into_iter()
                    .map(|target| Task::FadeCard { target })
                    .collect(),
            });
        }
        Ok(())
    }

    #[cold]
    #[inline(never)]
    /// A sampled world rebuilds a pending menu over selected cards from its
    /// remapped references: in an unordered hidden pile the sampler may
    /// permute known same-back cards, so the original menu's named options
    /// would point at other slots.
    pub(super) fn refresh_move_selected_menu(&mut self) -> Result<(), Error> {
        let [.., Frame::MoveSelected { .. }, Frame::Choose { tasks, .. }] = self.frames.as_slice()
        else {
            return Ok(());
        };
        if !tasks
            .iter()
            .all(|task| matches!(task, Task::TakeSelected { .. }))
        {
            return Ok(());
        }
        self.frames.pop();
        let Some(Frame::MoveSelected {
            verb,
            targets,
            context,
        }) = self.frames.pop()
        else {
            unreachable!("checked selected movement");
        };
        self.resume_move_selected(verb, targets, context)
    }

    fn resume_move_selected(
        &mut self,
        verb: Verb,
        targets: Vec<Captured<Target>>,
        context: Context,
    ) -> Result<(), Error> {
        let mut offered = Vec::new();
        for (
            index,
            &Captured {
                inner: target,
                evidence,
            },
        ) in targets.iter().enumerate()
        {
            if self.valid_source(target)? {
                // A card its reference shows the chooser (Yi Sun-sin's
                // revealed exchange) is named, wherever it lies.
                let named = self
                    .reference_visible(HeldCard::new(target.card, evidence), context.controller);
                offered.push((index, named));
            }
        }
        // Every card must go: when the chooser sees them all, cards bound
        // for different decks or colours commute (one group at a time).
        if offered.len() > 1 && offered.iter().all(|&(_, named)| named) {
            let candidates: Vec<Target> = offered
                .iter()
                .map(|&(index, _)| targets[index].inner)
                .collect();
            let keep = self.selected_move_group(verb, context, &candidates);
            offered.retain(|&(index, _)| keep.contains(&targets[index].inner.card));
        }
        let mut options = Vec::new();
        let mut tasks = Vec::new();
        for (index, named) in offered {
            let target = targets[index].inner;
            let descriptor = if named {
                TargetDescriptor::Known {
                    card: target.card,
                    source: target.physical,
                }
            } else {
                self.position.descriptor(target, context.controller)
            };
            options.push(Alternative::Move {
                verb,
                target: descriptor,
            });
            tasks.push(Task::TakeSelected { index });
        }
        if tasks.is_empty() {
            self.complete(ReturnValue::Unit);
        } else {
            self.frames.push(Frame::MoveSelected {
                verb,
                targets,
                context,
            });
            self.frames.push(Frame::Choose {
                player: context.controller,
                tasks,
                options: DecisionOptions::new(options, context),
            });
        }
        Ok(())
    }

    #[cold]
    #[inline(never)]
    fn resume_name_figure(&mut self, deck: Deck, context: Context) {
        let names: Vec<_> = FIGURE_NAMES.iter().filter(|n| n.age == deck.age).collect();
        let mut options: Vec<_> = names.iter().copied().map(Alternative::NameFigure).collect();
        options.push(Alternative::Pass);
        let mut tasks: Vec<_> = names
            .into_iter()
            .map(|name| Task::FindFigure {
                deck,
                name: Some(name),
                context,
            })
            .collect();
        tasks.push(Task::FindFigure {
            deck,
            name: None,
            context,
        });
        self.frames.push(Frame::Choose {
            player: context.controller,
            tasks,
            options: DecisionOptions::new(options, context),
        });
    }

    #[cold]
    #[inline(never)]
    fn resume_find_figure(
        &mut self,
        deck: Deck,
        name: Option<&'static FigureName>,
        context: Context,
        events: &mut Vec<Event>,
    ) {
        // The menu came from the immutable roster. Only now consult
        // the physical deck; no menu filtering can reveal a hit/miss.
        let found = name.and_then(|name| {
            self.position
                .cards(Zone::Deck(deck))
                .iter()
                .copied()
                .find(|&card| self.position.card(card).code == name.code)
        });
        if let Some(name) = name
            && let Some(knowledge) = &mut self.knowledge
        {
            for tracker in &mut knowledge.trackers {
                tracker.search(&self.position, deck, name.code, found.is_some());
            }
            knowledge.named = found.map(|card| (deck, card));
        }
        events.push(Event::FigureSearch {
            executor: context.executor,
            deck,
            name,
            found,
        });
        self.frames
            .push(Frame::operation(Operation::Shuffle { deck }, context));
        if let Some(card) = found {
            let source = Zone::Deck(deck);
            self.frames.push(Frame::operation(
                Operation::Move {
                    verb: Verb::Transfer,
                    target: Target {
                        card,
                        physical: source,
                        logical: LogicalZone::Physical(source),
                    },
                    destination: Zone::Hand(context.executor),
                },
                context,
            ));
        }
    }

    /// The current target of a retained identity inside a logical zone.
    pub(super) fn selected_target(&self, card: CardId, zone: LogicalZone) -> Option<Target> {
        self.position
            .targets(zone)
            .find(|target| target.card == card)
    }

    fn choose_operations(&mut self, context: Context, operations: Vec<Operation>, optional: bool) {
        self.choose_referenced_operations(context, operations, optional, None);
    }

    fn choose_referenced_operations(
        &mut self,
        context: Context,
        operations: Vec<Operation>,
        optional: bool,
        reference: Option<HeldCard>,
    ) {
        self.choose_operation_references(
            context,
            operations.into_iter().map(|op| (op, reference)).collect(),
            optional,
        );
    }

    fn choose_operation_references(
        &mut self,
        context: Context,
        operations: Vec<(Operation, Option<HeldCard>)>,
        optional: bool,
    ) {
        if operations.is_empty() {
            self.complete(ReturnValue::Operation(OperationOutcome::empty(
                Disposition::Inapplicable,
            )));
            return;
        }
        if operations.len() == 1 && !optional {
            // A forced choice resolves immediately; skip the menu allocation.
            self.frames.push(match operations[0] {
                (operation, Some(reference)) => {
                    Frame::captured_operation(operation, context, reference)
                }
                (operation, None) => Frame::operation(operation, context),
            });
            return;
        }
        let mut options = Vec::with_capacity(operations.len());
        let mut tasks: Vec<Task> = Vec::with_capacity(operations.len());
        for (operation, reference) in operations {
            let descriptor = |target: Target| {
                if reference.is_some_and(|held| self.reference_visible(held, context.controller)) {
                    TargetDescriptor::Known {
                        card: target.card,
                        source: target.physical,
                    }
                } else {
                    self.position.descriptor(target, context.controller)
                }
            };
            let option = match operation {
                Operation::Move { verb, target, .. } => Alternative::Move {
                    verb,
                    target: descriptor(target),
                },
                Operation::Claim { target, .. } => Alternative::Move {
                    verb: Verb::Achieve,
                    target: descriptor(target),
                },
                Operation::Draw { value, set } => Alternative::Draw {
                    value,
                    set: set.unwrap_or(CardSet::Base),
                },
                Operation::Splay {
                    player,
                    color,
                    direction,
                } => Alternative::Splay {
                    player,
                    color,
                    direction,
                },
                Operation::Reveal { target } => Alternative::Reveal {
                    target: descriptor(target),
                },
                Operation::ExchangeAll { left, right } => Alternative::Exchange { left, right },
                Operation::JunkAvailableSpecial { special } => Alternative::JunkSpecial(special),
                Operation::RestoreSpecial { special } => Alternative::RestoreSpecial(special),
                Operation::ClaimSpecial { special, .. } => Alternative::ClaimSpecial(special),
                _ => unreachable!("card choices"),
            };
            // A secret stays its own ordered public slot, never one random
            // member of a back group.
            if let Alternative::Move {
                verb,
                target: TargetDescriptor::Hidden { source, back, .. },
            } = option
                && !matches!(source, Zone::Safe(_))
            {
                let option = Alternative::BlindMove { verb, source, back };
                if let Some(index) = options.iter().position(|prior| *prior == option) {
                    let Task::BlindOperations(group) = &mut tasks[index] else {
                        unreachable!("blind operation group");
                    };
                    group.operations.push((operation, reference));
                } else {
                    // Order by public back only, retaining each operation's own
                    // evidence and native physical order inside a blind group.
                    let index = options.partition_point(|prior| matches!(prior,
                        Alternative::BlindMove { back: prior, .. } if (prior.age, prior.set) <= (back.age, back.set)));
                    options.insert(index, option);
                    tasks.insert(
                        index,
                        Task::BlindOperations(Box::new(BlindOperations {
                            operations: vec![(operation, reference)],
                            excluded: Vec::new(),
                            context,
                        })),
                    );
                }
            } else {
                options.push(option);
                tasks.push(match reference {
                    Some(reference) => Task::CapturedOperation(operation, context, reference),
                    None => Task::Operation(operation, context),
                });
            }
        }
        let named: Vec<_> = tasks
            .iter()
            .zip(&options)
            .filter_map(|(task, option)| match (task, option) {
                (
                    Task::CapturedOperation(_, _, reference),
                    Alternative::Move {
                        target: TargetDescriptor::Known { source, .. },
                        ..
                    },
                ) => Some((*reference, *source)),
                _ => None,
            })
            .collect();
        for (task, option) in tasks.iter_mut().zip(&options) {
            if let (Task::BlindOperations(group), Alternative::BlindMove { source, back, .. }) =
                (task, option)
            {
                group.excluded = named
                    .iter()
                    .filter(|(reference, zone)| {
                        *zone == *source
                            && self.position.card(reference.inner).origin_deck() == *back
                    })
                    .map(|(reference, _)| *reference)
                    .collect();
            }
        }
        // A "may" operation; Blackmail removes the decline.
        if optional && !context.must() {
            options.push(Alternative::Decline);
            tasks.push(Task::Decline);
        }
        self.frames.push(Frame::Choose {
            player: context.controller,
            options: DecisionOptions::new(options, context),
            tasks,
        });
    }

    /// Claim eligibility must hold before a Would reaction can earn side
    /// effects, and again when the original claim commits after First.
    fn claim_is_applicable(&self, request: Operation, context: Context) -> Result<bool, Error> {
        Ok(match request {
            Operation::Claim {
                target,
                eligibility,
            } => {
                let player = context.executor;
                self.valid_source(target)?
                    && match eligibility {
                        ClaimEligibility::Ignore => true,
                        ClaimEligibility::Available => {
                            self.position.eligible_for_achievement(player, target.card)
                        }
                        ClaimEligibility::Cost { score_factor } => {
                            self.position.meets_achievement_cost(
                                player,
                                target.card,
                                self.position.score(player) * i64::from(score_factor),
                                self.position.highest_claim_value(player),
                            )
                        }
                    }
            }
            Operation::ClaimSpecial {
                special,
                check_eligibility,
            } => {
                self.position.special_location(special) == SpecialLocation::Available
                    && (!check_eligibility
                        || self
                            .position
                            .eligible_for_special(context.executor, special))
            }
            Operation::ClaimDecree { decree } => {
                self.position.figures()
                    && self.position.decree_location(decree) == DecreeLocation::Supply
            }
            Operation::ClaimMuseum { player } => self.position.can_claim_museum(player),
            Operation::Move {
                verb: Verb::Achieve,
                target,
                destination,
            } => {
                destination == Zone::Achievements(context.executor) && self.valid_source(target)?
            }
            _ => false,
        })
    }

    /// Timing belongs to the nearest real action, not an idle fixture counter
    /// or an enclosing regular action around a nested free action.
    fn own_regular_action(&self, owner: Player, ordinal: u8) -> Option<Action> {
        if self.position.turn_owner != owner || self.position.regular_actions_started() != ordinal {
            return None;
        }
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame {
                Frame::ActionTail {
                    action,
                    kind,
                    context,
                    ..
                } => Some(
                    (*kind == ActionKind::Regular && context.executor == owner).then_some(*action),
                ),
                _ => None,
            })
            .flatten()
    }

    /// Inspection grants follow the executions on the stack (Hitchhiking).
    /// A grant that starts shows the owner's hand, score and Forecast to the
    /// viewer's tracker, later arrivals included through `can_view`; a grant
    /// that ends removes the permission, not what was learned.
    fn refresh_inspection(&mut self, resuming: Option<&Frame>) {
        if !self.inspecting {
            return;
        }
        let mut grants = [[false; 2]; 2];
        for frame in self.frames.iter().chain(resuming) {
            if let Frame::Execute(execution) = frame
                && let Some((viewer, owner)) = execution.inspection()
            {
                grants[viewer.index()][owner.index()] = true;
            }
        }
        self.inspecting = grants != [[false; 2]; 2];
        if grants == self.position.inspections {
            return;
        }
        if let Some(knowledge) = &mut self.knowledge {
            for viewer in Player::ALL {
                for owner in Player::ALL {
                    if !grants[viewer.index()][owner.index()]
                        || self.position.inspections[viewer.index()][owner.index()]
                    {
                        continue;
                    }
                    for zone in [Zone::Hand(owner), Zone::Score(owner), Zone::Forecast(owner)] {
                        for &card in self.position.cards(zone) {
                            for tracker in knowledge
                                .trackers
                                .iter_mut()
                                .filter(|tracker| tracker.viewer() == Some(viewer))
                            {
                                tracker.reveal(&self.position, card, zone, None);
                            }
                        }
                    }
                }
            }
        }
        self.position.inspections = grants;
    }

    /// A reveal, movement, splay or claim happened: every copied execution
    /// on the stack caused a change (The Big Bang). A reveal alone is not
    /// activity for an execution that ignores reveals (Teleprompter).
    fn note_execution_change(&mut self, reveal: bool) {
        for frame in &mut self.frames {
            if let Frame::Execute(execution) = frame {
                execution.note_change(reveal);
            }
        }
    }

    fn note_shared_change(&mut self, context: Context) {
        self.note_execution_change(false);
        // Compare with the invocation's own scope: karma children of an
        // ordinary shared effect do not count, but the original effects of
        // a replacement Dogma inside karma still satisfy its own sharing test.
        if let Some(Frame::Dogma(dogma)) = self
            .frames
            .iter_mut()
            .rev()
            .find(|frame| matches!(frame, Frame::Dogma(_)))
            && dogma.executing_shared
            && context.suppress_karma() == dogma.suppress_karma
        {
            dogma.shared = true;
        }
    }

    /// An attempted move whose card has left its source fails in public, so
    /// every observer learns its pick is no longer there (Queen Victoria
    /// moved the card a batch had blindly attempted). Only while the real
    /// card's own slot still holds the pick.
    fn attempt_found_absent(&mut self, target: Target) {
        if self.position.locate(target.card) == Some(target.physical) {
            return;
        }
        let Some(knowledge) = self.knowledge.as_mut() else {
            return;
        };
        let Some(pick) = knowledge.moving else {
            return;
        };
        for tracker in &mut knowledge.trackers {
            if tracker.pick_holds(&self.position, target.card, pick) {
                tracker.absent(target.physical, pick);
            }
        }
    }

    fn valid_source(&self, target: Target) -> Result<bool, Error> {
        Ok(target.physical != Zone::AgeZeroReserve
            && self.position.cards(target.physical).contains(&target.card)
            && (target.logical.home() == target.physical
                || self
                    .position
                    .targets(target.logical)
                    .any(|t| t.card == target.card && t.physical == target.physical))
            && (!matches!(target.physical, Zone::Board(..))
                || matches!(target.logical, LogicalZone::BoardDepth(..))
                || self.position.cards(target.physical).last() == Some(&target.card)))
    }

    /// Eligibility is an operand of a printed claim, retained through First
    /// so the live cost check uses the same score factor as its selection.
    fn program_movement(
        &self,
        verb: Verb,
        from: Selector,
        target: Target,
        to: Destination,
        context: Context,
    ) -> Operation {
        if verb == Verb::Achieve
            && to == Destination::Achievements(Who::Executor)
            && let Some(score_factor) = from.claim_score_factor()
        {
            Operation::Claim {
                target,
                eligibility: ClaimEligibility::Cost { score_factor },
            }
        } else if verb == Verb::Achieve
            && let Destination::Achievements(who) = to
            && who != Who::Executor
        {
            // That player's own claim, ignoring eligibility; it runs in
            // the claimant's context (see `claimant_context`).
            Operation::Claim {
                target,
                eligibility: ClaimEligibility::Ignore,
            }
        } else {
            self.movement(verb, target, to, context)
        }
    }

    /// Destinations name one player; the catalog validation test rejects
    /// `Who::Either` there, which would otherwise mean the executor.
    fn movement(&self, verb: Verb, target: Target, to: Destination, context: Context) -> Operation {
        let player = |who| self.one_player(who, context).unwrap_or(context.executor);
        let destination = match to {
            Destination::Hand(who) => Zone::Hand(player(who)),
            Destination::Score(who) => Zone::Score(player(who)),
            Destination::Board(who) => {
                Zone::Board(player(who), self.position.card(target.card).color)
            }
            Destination::Origin(_) => unreachable!("resolved before starting the move"),
            Destination::Deck => Zone::Deck(self.position.card(target.card).origin_deck()),
            Destination::Junk => Zone::Junk,
            Destination::Achievements(who) => Zone::Achievements(player(who)),
            Destination::Supply => Zone::AvailableAchievements,
            Destination::Forecast(who) => Zone::Forecast(player(who)),
            Destination::Safe(who) => Zone::Safe(player(who)),
        };
        Operation::Move {
            verb,
            target,
            destination,
        }
    }

    fn plan_exchange(
        &self,
        left: LogicalZone,
        right: LogicalZone,
        context: Context,
    ) -> Result<ExchangePlan, Error> {
        let physical = |zone| match zone {
            LogicalZone::Hand(p) => Ok(Zone::Hand(p)),
            LogicalZone::Score(p) => Ok(Zone::Score(p)),
            LogicalZone::AvailableAchievements(_) => Ok(Zone::AvailableAchievements),
            LogicalZone::Physical(
                zone @ (Zone::Forecast(_) | Zone::Board(..) | Zone::AvailableAchievements),
            ) => Ok(zone),
            _ => Err(Error::Unsupported("unsupported exchange endpoint")),
        };
        let left_destination = physical(right)?;
        let right_destination = physical(left)?;
        // Take both selections before any mutation. A physical card selected
        // for both directions stays where it is. Single-card verbs do not use
        // this rule: scoring a Michelangelo hand card still moves it.
        Ok(ExchangePlan {
            left: self.position.targets(left).collect(),
            right: self.position.targets(right).collect(),
            left_destination,
            right_destination,
            context,
        })
    }

    fn commit_exchange(
        &mut self,
        plan: &ExchangePlan,
        context: Context,
        events: &mut Vec<Event>,
    ) -> Result<OperationOutcome, Error> {
        let movements: Vec<_> = plan.movements().collect();
        if movements.is_empty() {
            return Ok(OperationOutcome::empty(Disposition::Inapplicable));
        }
        // One commit and one grouped fact. No intermediate tracker or
        // scheduler boundary exposes half of an exchange.
        let mut tracked = Vec::new();
        for &(target, destination) in &movements {
            if let Some(slots) = self.track_extract(
                target.card,
                target.physical,
                destination,
                tracking::Departure::Unordered,
            ) {
                tracked.push(slots);
            }
            let cards = self.position.zone_mut(target.physical);
            let index = cards
                .iter()
                .position(|&c| c == target.card)
                .expect("exchange selection");
            cards.remove(index);
        }
        let mut exchanged = Vec::with_capacity(movements.len());
        let mut tracked = tracked.into_iter();
        for &(target, destination) in &movements {
            self.track_insert(destination, tracked.next(), false);
            self.position.zone_mut(destination).push(target.card);
            self.position.set_location(target.card, destination);
            if let Zone::Board(player, _) = destination {
                self.note_board_arrival(target.card, player);
            }
            exchanged.push(ExchangeMovement {
                card: target.card,
                source: target.physical,
                destination,
            });
        }
        if movements.iter().any(|&(target, destination)| {
            matches!(target.physical, Zone::Board(..)) || matches!(destination, Zone::Board(..))
        }) {
            // Available-achievement aliases can bring a board top into the
            // exchange. Its stack loses its splay as part of the atomic move.
            for &(target, _) in &movements {
                if let Zone::Board(player, color) = target.physical
                    && self.position.cards(target.physical).len() < 2
                {
                    self.position.players[player.index()].splays[color as usize] = Splay::None;
                }
            }
            self.position.refresh_boards();
        }
        events.push(Event::Exchanged {
            executor: context.executor,
            inside_karma: context.suppress_karma(),
            movements: exchanged,
        });
        self.refresh_tracking()?;
        self.refresh_meld_knowledge();
        debug_assert!(self.position.check_conservation());
        Ok(OperationOutcome::empty(Disposition::Performed))
    }

    fn move_special(
        &mut self,
        special: Special,
        destination: SpecialLocation,
        context: Context,
        events: &mut Vec<Event>,
    ) {
        let source = self.position.special_location(special);
        self.position.specials[special.index()] = destination;
        events.push(Event::SpecialMoved {
            special,
            source,
            destination,
            executor: context.executor,
            inside_karma: context.suppress_karma(),
        });
    }

    /// The frozen effect list when echoes run with this card: its visible
    /// echoes, then its dogma effects. `None` (the card's own dogma effects)
    /// without Echoes or when no echo would run, keeping the common path.
    fn echo_schedule(&self, card: CardId) -> Option<Box<ExecutionExtra>> {
        if !self.position.expansions().echoes() {
            return None;
        }
        let echoes = self.position.visible_echoes(card);
        if echoes.is_empty() {
            return None;
        }
        let dogmas = (0..self.position.card(card).effects.len()).map(|index| (card, index as u16));
        Some(Box::new(ExecutionExtra {
            schedule: Some(
                echoes
                    .into_iter()
                    .map(|source| (source, Card::ECHO))
                    .chain(dogmas)
                    .collect(),
            ),
            memory: slots::Memory::default(),
        }))
    }

    /// The deck a draw takes from, or `None` when the game would end by
    /// exhaustion. An ordinary draw (`set: None`) may be redirected by
    /// expansion rules; an explicit or chosen set, Base included, never is.
    fn draw_deck(
        &self,
        value: u8,
        set: Option<CardSet>,
        drawer: Player,
    ) -> Result<Option<Deck>, Error> {
        if let Some(set) = set
            && !self.position.expansions().includes(set)
        {
            return Err(Error::Unsupported("draw from a disabled expansion"));
        }
        // Echoes: an ordinary draw by a player with a single highest top card
        // comes from the Echoes deck. Unseen: otherwise, a player's first draw
        // of a turn comes from the Unseen deck. A Base fallback is not
        // redirected again.
        let expansions = self.position.expansions();
        let set = match set {
            Some(set) => set,
            None if expansions.echoes() && self.position.unique_highest_top(drawer) => {
                CardSet::Echoes
            }
            None if expansions.unseen() && !self.position.first_drawn(drawer) => CardSet::Unseen,
            None => CardSet::Base,
        };
        let Some(age) =
            (value..=11).find(|&age| !self.position.cards(Zone::Deck(Deck::base(age))).is_empty())
        else {
            return Ok(None);
        };
        let requested = Deck { set, age };
        Ok(Some(
            if self.position.cards(Zone::Deck(requested)).is_empty() {
                Deck::base(age)
            } else {
                requested
            },
        ))
    }

    /// The innermost running program, the scope of a reveal it performs.
    fn revealing_program(&self) -> Option<RevealScope> {
        self.frames.iter().rev().find_map(Frame::program)
    }

    fn exhaustion_request(&self) -> Operation {
        let rank = |player| {
            (
                self.position.score(player),
                self.position.effective_achievement_count(player),
            )
        };
        match rank(Player::One).cmp(&rank(Player::Two)) {
            std::cmp::Ordering::Greater => Operation::ProposeWin {
                winner: Player::One,
                reason: WinReason::Exhaustion,
            },
            std::cmp::Ordering::Less => Operation::ProposeWin {
                winner: Player::Two,
                reason: WinReason::Exhaustion,
            },
            std::cmp::Ordering::Equal => Operation::ProposeDraw {
                reason: DrawReason::Exhaustion,
            },
        }
    }

    fn achievement_winner(&self) -> Option<Player> {
        // Two players need six achievements, plus one per enabled expansion.
        let threshold = 6 + self.position.expansions.count();
        let mut eligible = Player::ALL
            .into_iter()
            .filter(|&p| self.position.effective_achievement_count(p) >= threshold);
        let first = eligible.next()?;
        let Some(second) = eligible.next() else {
            return Some(first);
        };
        match self
            .position
            .effective_achievement_count(first)
            .cmp(&self.position.effective_achievement_count(second))
        {
            std::cmp::Ordering::Greater => Some(first),
            std::cmp::Ordering::Less => Some(second),
            std::cmp::Ordering::Equal => Some(self.position.turn_owner.opponent()),
        }
    }

    fn can_splay(&self, player: Player, color: Color, direction: Splay) -> bool {
        self.position.cards(Zone::Board(player, color)).len() >= 2
            && self.position.splay(player, color) != direction
    }

    fn commit(
        &mut self,
        request: Operation,
        context: Context,
        retain: bool,
        events: &mut Vec<Event>,
    ) -> Result<OperationOutcome, Error> {
        self.commit_departure(
            request,
            context,
            retain,
            tracking::Departure::Unordered,
            events,
        )
    }

    fn commit_departure(
        &mut self,
        request: Operation,
        context: Context,
        retain: bool,
        departure: tracking::Departure,
        events: &mut Vec<Event>,
    ) -> Result<OperationOutcome, Error> {
        let (verb, card, source, destination) = match request {
            Operation::Dogma { .. }
            | Operation::IssueDecree { .. }
            | Operation::ExecuteEffect { .. }
            | Operation::DrawAction { .. } => {
                unreachable!("operation expands into a continuation")
            }
            Operation::ClaimDecree { decree } | Operation::ReturnDecree { decree } => {
                if !self.position.figures() {
                    return Err(Error::Unsupported("Decrees require Figures"));
                }
                let source = self.position.decree_location(decree);
                let destination = match request {
                    Operation::ClaimDecree { .. } if source == DecreeLocation::Supply => {
                        DecreeLocation::Owned(context.executor)
                    }
                    Operation::ReturnDecree { .. }
                        if matches!(source, DecreeLocation::Owned(_)) =>
                    {
                        DecreeLocation::Supply
                    }
                    _ => return Ok(OperationOutcome::empty(Disposition::Inapplicable)),
                };
                self.position
                    .decrees
                    .get_or_insert_with(|| Box::new([DecreeLocation::Supply; 5]))
                    [decree as usize] = destination;
                events.push(Event::DecreeMoved {
                    decree,
                    source,
                    destination,
                    executor: context.executor,
                    inside_karma: context.suppress_karma(),
                });
                self.note_shared_change(context);
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::IntroduceAgeZero => {
                if !self.position.figures() {
                    return Err(Error::Unsupported("Age 0 introduction requires Figures"));
                }
                let age_zero = self
                    .position
                    .age_zero
                    .as_mut()
                    .ok_or(Error::Unsupported("Age 0 roster was not supplied"))?;
                if age_zero.introduced {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                if let Some(knowledge) = &mut self.knowledge {
                    for tracker in &mut knowledge.trackers {
                        tracker.introduce_age_zero();
                    }
                }
                let mut cards = std::mem::take(&mut age_zero.reserve);
                age_zero.introduced = true;
                self.rng.shuffle(&mut cards);
                let count = cards.len();
                for &card in &cards {
                    self.position.set_location(card, Zone::Deck(Deck::base(0)));
                }
                self.position
                    .zone_mut(Zone::Deck(Deck::base(0)))
                    .extend(cards);
                events.push(Event::AgeZeroIntroduced {
                    executor: context.executor,
                    count,
                });
                debug_assert!(self.position.check_conservation());
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::ClaimSpecial { special, .. } => {
                if !self.claim_is_applicable(request, context)? {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                self.move_special(
                    special,
                    SpecialLocation::Owned(context.executor),
                    context,
                    events,
                );
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::JunkAvailableSpecial { special } => {
                if self.position.special_location(special) != SpecialLocation::Available {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                self.move_special(special, SpecialLocation::Junk, context, events);
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::RestoreSpecial { special } => {
                if self.position.special_location(special) != SpecialLocation::Junk {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                self.move_special(special, SpecialLocation::Available, context, events);
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::ClaimMuseum { player } => {
                if !self.claim_is_applicable(request, context)? {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                self.position.claim_museum(player)?;
                events.push(Event::MuseumClaimed {
                    player,
                    executor: context.executor,
                    inside_karma: context.suppress_karma(),
                });
                self.note_shared_change(context);
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::Claim { target, .. } => {
                if !self.claim_is_applicable(request, context)? {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                (
                    Some(Verb::Achieve),
                    target.card,
                    target.physical,
                    Zone::Achievements(context.executor),
                )
            }
            Operation::CheckAchievementVictory => {
                return Ok(OperationOutcome::empty(Disposition::Inapplicable));
            }
            Operation::ProposeDraw { reason } => {
                self.terminal = Some(Terminal::Draw(reason));
                self.frames.clear();
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::Reveal { target } => {
                if !self.valid_source(target)? {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                if let Some(knowledge) = &mut self.knowledge {
                    let pick = knowledge.moving.take();
                    for tracker in &mut knowledge.trackers {
                        tracker.reveal(&self.position, target.card, target.physical, pick);
                    }
                }
                events.push(Event::Revealed {
                    card: target.card,
                    source: target.physical,
                    executor: context.executor,
                    inside_karma: context.suppress_karma(),
                });
                // An explicit reveal shows the face through the revealing
                // program and its children (a hand Yamato counts 11).
                if let Some(scope) = self.revealing_program()
                    && self.position.reveal_face(target.card)
                {
                    self.reveal_scope = Some(scope);
                }
                // A pending meld learns the revealed card only through its
                // own pick, never because the real cards coincide.
                self.refresh_meld_knowledge();
                return Ok(OperationOutcome {
                    disposition: Disposition::Performed,
                    card: Some(target.card),
                    value: Some(self.position.value(target.card)),
                    short: false,
                });
            }
            Operation::Splay {
                player,
                color,
                direction,
            } => {
                if !self.can_splay(player, color, direction) {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                let from = self.position.splay(player, color);
                self.position.players[player.index()].splays[color as usize] = direction;
                self.position.refresh_board(player);
                events.push(Event::Splayed {
                    player,
                    color,
                    from,
                    to: direction,
                    executor: context.executor,
                    inside_karma: context.suppress_karma(),
                });
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::Move {
                verb,
                target,
                destination,
            } => {
                let source = if self.position.figures() && retain {
                    // The identity was validated by the interrupted meld or
                    // produced by a completed child; never reselect its top.
                    self.position
                        .locate(target.card)
                        .ok_or(Error::InvalidSetup(
                            "retained card lost from physical state",
                        ))?
                } else {
                    if !self.valid_source(target)? {
                        self.attempt_found_absent(target);
                        return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                    }
                    target.physical
                };
                if destination == Zone::AgeZeroReserve
                    || matches!(destination, Zone::Board(_, color) if color != self.position.card(target.card).color)
                    || matches!(destination, Zone::Deck(deck) if deck.age > 11 || deck != self.position.card(target.card).origin_deck())
                {
                    return Err(Error::Unsupported("invalid movement destination"));
                }
                // Recheck live capacity at commit: a First reaction may have
                // filled the destination or lowered its limit.
                if source != destination && !self.position.has_room(destination) {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                }
                (Some(verb), target.card, source, destination)
            }
            Operation::ExchangeAll { .. } => unreachable!("exchange has its own continuation"),
            Operation::Shuffle { deck } => {
                if deck.age > 11 || (deck.set == CardSet::Figures && !self.position.figures()) {
                    return Err(Error::Unsupported("invalid shuffle deck"));
                }
                // An absent Figures allocation has only empty decks, which
                // still produce the same shuffle fact and consume no RNG.
                if !self.position.cards(Zone::Deck(deck)).is_empty() {
                    self.rng.shuffle(self.position.zone_mut(Zone::Deck(deck)));
                }
                if let Some(knowledge) = &mut self.knowledge {
                    for tracker in &mut knowledge.trackers {
                        tracker.shuffle(Zone::Deck(deck));
                    }
                }
                events.push(Event::Shuffled {
                    executor: context.executor,
                    deck,
                });
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::Draw { value, set } => {
                // Resolve after First reactions; the scheduler has already
                // routed an exhausted draw through proposed terminal handling.
                let deck = self
                    .draw_deck(value, set, context.executor)?
                    .expect("non-exhausted draw");
                let card = *self
                    .position
                    .cards(Zone::Deck(deck))
                    .last()
                    .expect("resolved draw");
                // Any actual draw, explicit sets and Base fallbacks included,
                // is this player's draw for the turn. Direct deck tops are not.
                self.position.first_drawn[context.executor.index()] = true;
                (None, card, Zone::Deck(deck), Zone::Hand(context.executor))
            }
            Operation::Ineligible { .. } | Operation::ShareBonus { earned: false, .. } => {
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::ShareBonus { earned: true, .. } => {
                unreachable!("an earned share bonus expands into a free Draw action")
            }
            Operation::DrawTop { deck } => {
                let Some(&card) = self.position.cards(Zone::Deck(deck)).last() else {
                    return Ok(OperationOutcome::empty(Disposition::Inapplicable));
                };
                (None, card, Zone::Deck(deck), Zone::Hand(context.executor))
            }
            Operation::RequireFade { player } => {
                return Ok(OperationOutcome::empty(
                    if self.position.figures() && self.position.top_figures(player).count() > 1 {
                        Disposition::Performed
                    } else {
                        Disposition::Inapplicable
                    },
                ));
            }
            Operation::ProposeWin { winner, reason } => {
                self.terminal = Some(Terminal::Winner(winner, Victory::new(reason, context)));
                self.frames.clear();
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::Lose { player } => {
                self.eliminate(player, context, events)?;
                return Ok(OperationOutcome::empty(Disposition::Performed));
            }
            Operation::Flip => {
                // Low bit set wins. The coin is public
                // and counts as activity either way (Teleprompter).
                let won = self.rng.next_u64() & 1 == 1;
                events.push(Event::Flipped {
                    executor: context.executor,
                    won,
                });
                self.note_execution_change(false);
                return Ok(OperationOutcome::empty(if won {
                    Disposition::Performed
                } else {
                    Disposition::Inapplicable
                }));
            }
        };
        self.place(
            verb,
            card,
            source,
            destination,
            if verb.is_none() || verb == Some(Verb::Dig) {
                tracking::Departure::Position
            } else {
                departure
            },
            context,
            events,
        )
    }

    /// Physically move one validated card, with its tracking and event.
    /// `departure` can take it from its deck position without forgetting the
    /// deck's order (a draw, or a revealed card). No karma is discovered.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn place(
        &mut self,
        verb: Option<Verb>,
        card: CardId,
        source: Zone,
        destination: Zone,
        departure: tracking::Departure,
        context: Context,
        events: &mut Vec<Event>,
    ) -> Result<OperationOutcome, Error> {
        let value = self.position.value(card);
        let slots = self.track_extract(card, source, destination, departure);
        let cards = self.position.zone_mut(source);
        let index = cards
            .iter()
            .position(|&c| c == card)
            .expect("validated source");
        cards.remove(index);
        if let Zone::Board(player, color) = source
            && cards.len() < 2
            && destination != source
        {
            // Losing a stack's splay is part of leaving the board, not a
            // separate splay instruction or an opportunity for karma. A top
            // card tucked into its own pile never shrinks it.
            self.position.players[player.index()].splays[color as usize] = Splay::None;
        }
        // A Museum is a card too: junking the Artifact stored in it (Fission's
        // "non-achievement cards", elimination) junks the Museum with it.
        if matches!(source, Zone::Museum(_)) && destination == Zone::Junk {
            self.position.junk_emptied_museum();
        }
        let bottom = verb == Some(Verb::Tuck)
            || (matches!(destination, Zone::Deck(_)) && verb != Some(Verb::Place));
        self.track_insert(destination, slots, bottom);
        let cards = self.position.zone_mut(destination);
        if bottom {
            cards.insert(0, card);
        } else {
            cards.push(card);
        }
        self.position.set_location(card, destination);
        for zone in [source, destination] {
            if let Zone::Board(player, _) = zone {
                self.position.refresh_board(player);
            }
        }
        if let Zone::Board(player, _) = destination {
            self.note_board_arrival(card, player);
        }
        if let Some(slot) = match verb {
            Some(Verb::Tuck) => Some(0),
            Some(Verb::Score) => Some(1),
            _ => None,
        } {
            let count = &mut self.position.turn_moves[context.executor.index()][slot];
            *count = count.saturating_add(1);
        }
        events.push(Event::Moved(Movement {
            verb,
            card,
            source,
            destination,
            executor: context.executor,
            inside_karma: context.suppress_karma(),
        }));
        self.refresh_tracking()?;
        self.refresh_meld_knowledge();
        debug_assert!(self.position.check_conservation());
        if verb.is_some()
            && let Zone::Board(owner, _) = source
        {
            self.record_kept_origin(owner);
        }
        Ok(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(card),
            value: Some(value),
            short: false,
        })
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn common_frame_layout_budget() {
        println!(
            "Frame={} Operation={} Context={} Outcome={} Task={} Alternative={} Position={} Game={} Game={}",
            size_of::<Frame>(),
            size_of::<Operation>(),
            size_of::<Context>(),
            size_of::<OperationOutcome>(),
            size_of::<Task>(),
            size_of::<Alternative>(),
            size_of::<Position>(),
            size_of::<Game>(),
            size_of::<Game>()
        );
        // An explicit review gate for new continuation payloads, not proof of
        // Base replay parity or heap allocation/clone cost.
        #[cfg(target_pointer_width = "64")]
        assert!(
            size_of::<Frame>() <= 56,
            "box rare progress rather than enlarging every frame"
        );
        assert_eq!(size_of::<Game>(), size_of::<Game>());
    }

    #[test]
    fn cloning_shares_immutable_programs_without_allocating_figures_decks() {
        let position = Position::new(
            vec![Card {
                code: "layout-probe",
                set: CardSet::Base,
                color: Color::Red,
                value: 1,
                figure: None,
                echo: None,
                symbols: [Symbol::Blank; SLOTS],
                featured: Some(Icon::Authority),
                effects: vec![Effect {
                    kind: EffectKind::NonDemand,
                    instructions: vec![build::draw(1)],
                }],
            }],
            &[(CardId(0), Zone::Junk)],
            Player::One,
        )
        .unwrap();
        let mut game = Game::new(position, Expansions::BASE).unwrap();
        game.self_execute(CardId(0), Context::new(Player::One))
            .unwrap();
        let fork = game.clone();
        assert!(std::sync::Arc::ptr_eq(
            &game.position.catalog,
            &fork.position.catalog
        ));
        assert!(fork.position.set_decks.iter().all(Option::is_none));
        assert!(fork.position.decrees.is_none());
        assert!(fork.position.age_zero.is_none());
        assert_eq!(game, fork);
    }
}

#[cfg(test)]
mod board_permission_tests {
    use super::*;

    #[test]
    fn foreign_dogma_context_retains_physical_owner_for_demand_and_shared_invocations() {
        for demand in [false, true] {
            let mut source = base_card("B012").unwrap();
            source.featured = Some(if demand {
                Icon::Prosperity
            } else {
                Icon::Concept
            });
            source.effects[0].kind = EffectKind::from_demand(demand);
            let position = Position::new(
                vec![figure_card("F020").unwrap(), source],
                &[
                    (CardId(0), Zone::Board(Player::One, Color::Green)),
                    (CardId(1), Zone::Board(Player::Two, Color::Blue)),
                ],
                Player::One,
            )
            .unwrap();
            let mut game = Game::new(position, Expansions::FIGURES).unwrap();
            game.take_action(
                Action::Dogma {
                    card: CardId(1),
                    auspice: false,
                },
                ActionKind::Regular,
            )
            .unwrap();
            // Stop at the effect boundary, before the deliberately absent draw
            // supply. This asserts the private retained source, not a projection.
            let mut reached = false;
            for _ in 0..30 {
                if let Some(Frame::Operation {
                    request:
                        Operation::ExecuteEffect {
                            card: CardId(1), ..
                        },
                    context,
                    ..
                }) = game.frames.last()
                {
                    assert_eq!(context.executor, Player::Two);
                    assert_eq!(context.controller, Player::Two);
                    assert_eq!(context.demander, Player::One);
                    assert_eq!(context.source_owner, Player::Two);
                    assert_eq!(context.source_card, Some(CardId(1)));
                    reached = true;
                    break;
                }
                assert_eq!(game.advance(1, &mut Vec::new()).unwrap(), Status::WorkLimit);
            }
            assert!(reached, "foreign effect was never scheduled");
        }
    }
}

#[cfg(test)]
mod robocar_tests;

#[cfg(test)]
mod proof_search_tests;

#[cfg(test)]
#[path = "echoes_tests/artificial_heart.rs"]
mod artificial_heart_tests;

#[cfg(test)]
#[path = "echoes_tests/age01.rs"]
mod echoes_age01_tests;

#[cfg(test)]
#[path = "echoes_tests/age02.rs"]
mod echoes_age_two_tests;

#[cfg(test)]
#[path = "echoes_tests/age03_mechanics.rs"]
mod age03_mechanics_tests;

#[cfg(test)]
#[path = "echoes_tests/age03_cards.rs"]
mod age03_cards;

#[cfg(test)]
#[path = "echoes_tests/age03_shortcuts.rs"]
mod age03_shortcuts;

#[cfg(test)]
#[path = "echoes_tests/age05.rs"]
mod echoes_age05_tests;

#[cfg(test)]
#[path = "echoes_tests/age06.rs"]
mod age06_tests;

#[cfg(test)]
#[path = "echoes_tests/age07_cards.rs"]
mod age07_cards;

#[cfg(test)]
#[path = "echoes_tests/age07_shortcuts.rs"]
mod age07_shortcuts;

#[cfg(test)]
#[path = "echoes_tests/age07_privacy.rs"]
mod age07_privacy;

#[cfg(test)]
#[path = "echoes_tests/age09.rs"]
mod echoes_age09_tests;

#[cfg(test)]
#[path = "echoes_tests/age10.rs"]
mod age10_tests;

#[cfg(test)]
#[path = "echoes_tests/age11.rs"]
mod age11_tests;

#[cfg(test)]
#[path = "echoes_tests/single_blind.rs"]
mod single_blind;
