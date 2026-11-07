//! Exhaustive continuation traversal. Code sources are identities (a running
//! program survives its source moving and is looked up from it). A retained
//! reference the observer can name keeps its identity; any other follows its
//! [`Pick`](crate::tracking::Pick): the sample chose one of the candidate
//! places the observer's tracker kept for it. Neither the real card's place
//! nor which references share a real card reaches the sample.
use super::*;
use crate::execution::references::Evidence;
use crate::tracking::PICKS;

/// What a traversal does with the references it visits.
#[derive(Clone, Copy)]
pub(in crate::execution) enum Mode<'a> {
    /// Erase epistemic metadata, as mandatory-loop witnesses compare rules only.
    Rules,
    /// Leave the continuation unchanged; record live picks and refusals.
    Scan,
    /// Rewrite the continuation into a sampled world.
    Sample {
        /// The sampled card at each real card's physical position.
        cards: &'a [CardId],
        /// Each live pick's card: the one a reference names, else sampled.
        picks: &'a [Option<CardId>],
    },
}

pub(in crate::execution) struct Remap<'a> {
    pub(in crate::execution) mode: Mode<'a>,
    pub(in crate::execution) viewer: Player,
    pub(in crate::execution) position: &'a Position,
    pub(in crate::execution) stable: Vec<bool>,
    /// Why the observer's information cannot place a reference, if so.
    pub(in crate::execution) opaque: std::cell::Cell<Option<&'static str>>,
    /// Picks the continuation still holds.
    pub(in crate::execution) live: std::cell::Cell<u64>,
    /// Picks some reference names to this observer, and that card: every
    /// reference sharing the pick is that card.
    pub(in crate::execution) pinned: std::cell::RefCell<[Option<CardId>; PICKS]>,
    /// Picks some reference this observer cannot name follows.
    pub(in crate::execution) hidden: std::cell::Cell<u64>,
    /// The sampling observer's tracker: it decides which picks the observer
    /// can name, and (in debug builds) that each pick holds its real card.
    pub(in crate::execution) tracker: Option<&'a crate::tracking::Tracker>,
}

/// A running program's source is hidden from the observer and not pinned.
pub(crate) const HIDDEN_SOURCE: &str = "a running program's hidden source needs a pinned identity";
/// A karma condition or entered body already read a hidden operand's front.
pub(crate) const HIDDEN_FRONT: &str = "a karma already read a hidden operand's front";
/// A proof branch may depend on private cards other than its operand.
pub(crate) const PRIVATE_PROOF: &str =
    "conditional verification needs its original observer until the move completes";
/// A hidden reference has no pick (the pick pool was exhausted).
pub(crate) const UNTRACKED: &str = "a hidden reference has no tracked pick";

impl<'a> Remap<'a> {
    pub(in crate::execution) fn new(
        mode: Mode<'a>,
        viewer: Player,
        position: &'a Position,
        stable: Vec<bool>,
    ) -> Self {
        Self {
            mode,
            viewer,
            position,
            stable,
            opaque: std::cell::Cell::new(None),
            live: std::cell::Cell::new(0),
            pinned: std::cell::RefCell::new([None; PICKS]),
            hidden: std::cell::Cell::new(0),
            tracker: None,
        }
    }

    /// Whether the observer can name a reference's card: known evidence, or a
    /// pick its tracker pins to one known slot. Where the real card lies now
    /// never decides it.
    pub(in crate::execution) fn names(&self, evidence: Evidence) -> bool {
        evidence.known(self.viewer)
            || evidence.pick().is_some_and(|pick| {
                self.tracker
                    .is_some_and(|tracker| tracker.pick_identity(pick).is_some())
            })
    }

    /// Historical private scalar facts are not constraints on sampled physical
    /// cards. Refuse before sampling; only exact rules-only loop witnesses erase
    /// observer metadata. Mode::Sample never turns a private fact public.
    pub(in crate::execution) fn scalar(&self, known: &mut u8) {
        if self.rules_only() {
            *known = 3;
        } else if *known & (1 << self.viewer.index()) == 0 {
            self.opaque
                .set(Some("a retained scalar depends on private information"));
        }
    }

    /// Rules-only traversal: erases epistemic metadata.
    pub(in crate::execution) fn rules(position: &'a Position) -> Self {
        Self::new(Mode::Rules, Player::One, position, Vec::new())
    }

    pub(in crate::execution) fn rules_only(&self) -> bool {
        matches!(self.mode, Mode::Rules)
    }

    /// A running program's source selects its program text, so it cannot
    /// change identity. One the observer cannot pin rejects sampling.
    pub(in crate::execution) fn source(&self, card: CardId, evidence: Evidence) {
        if self.rules_only() {
            return;
        }
        if let Some(pick) = evidence.pick() {
            self.live.set(self.live.get() | 1 << pick.index());
        }
        if !evidence.known(self.viewer) && !self.stable[usize::from(card.0)] {
            self.opaque.set(Some(HIDDEN_SOURCE));
        }
    }
    /// A retired memory producer is an explicit named reference, not evidence
    /// obtained by finding that physical card in a public location later.
    pub(in crate::execution) fn memory_source(&self, card: CardId, evidence: &mut Evidence) {
        if self.rules_only() {
            self.erase(evidence);
            return;
        }
        // A tracker-named pick authorizes the source as explicit evidence
        // would, in Scan as well as Sample.
        let mut pin_evidence = *evidence;
        if self.names(*evidence) {
            pin_evidence |= Evidence::known_to(self.viewer);
        } else {
            self.opaque.set(Some(HIDDEN_SOURCE));
        }
        self.source(card, pin_evidence);
        // Pin the authorized identity through the ordinary retained-reference
        // path, including aliases whose older evidence did not yet name it.
        let mut source = card;
        self.reference(&mut source, &mut pin_evidence);
        if matches!(self.mode, Mode::Sample { .. }) {
            *evidence = pin_evidence;
        }
        self.erase(evidence);
    }
    /// Loop witnesses carry no epistemic metadata.
    pub(in crate::execution) fn erase(&self, evidence: &mut Evidence) {
        if self.rules_only() {
            *evidence = Evidence::NONE;
        } else if let Some(pick) = evidence.pick() {
            self.live.set(self.live.get() | 1 << pick.index());
        }
    }
    pub(in crate::execution) fn physical(&self, card: &mut CardId) {
        if let Mode::Sample { cards, .. } = self.mode {
            *card = cards[usize::from(card.0)];
        }
    }
    fn physical_target(&self, target: &mut Target) {
        self.physical(&mut target.card);
    }
    /// A retained reference, by what this observer knows about it.
    pub(in crate::execution) fn reference(&self, card: &mut CardId, evidence: &mut Evidence) {
        if self.rules_only() {
            *evidence = Evidence::NONE;
            return;
        }
        if let Some(pick) = evidence.pick() {
            self.live.set(self.live.get() | 1 << pick.index());
            if matches!(self.mode, Mode::Scan) {
                debug_assert!(
                    self.tracker.is_none_or(|tracker| tracker.pick_holds(
                        self.position,
                        *card,
                        pick
                    )),
                    "a pick lost its real card"
                );
                if evidence.known(self.viewer) {
                    let pinned = &mut self.pinned.borrow_mut()[pick.index()];
                    debug_assert!(pinned.is_none_or(|pinned| pinned == *card));
                    *pinned = Some(*card);
                } else {
                    self.hidden.set(self.hidden.get() | 1 << pick.index());
                }
            }
        }
        if evidence.known(self.viewer) {
            return;
        }
        match (evidence.pick(), self.mode) {
            (Some(pick), Mode::Sample { picks, .. }) => match picks[pick.index()] {
                Some(sampled) => *card = sampled,
                None => self.opaque.set(Some(UNTRACKED)),
            },
            (Some(_), _) => {}
            // A deck position the original still holds (Dragon's Lair's deck
            // tops) names the sampled card at that place, as a pending target
            // does; it needs no pick when the pool ran out.
            (None, _) if evidence.positional() => self.physical(card),
            (None, _) => self.opaque.set(Some(UNTRACKED)),
        }
        // Search worlds have no trackers. Preserve the sampling observer's
        // acquired knowledge on this same reference, including a pick that
        // became named after the reference was captured. First follow the
        // sampled pick above; the physical card's identity never grants it.
        if matches!(self.mode, Mode::Sample { .. }) && self.names(*evidence) {
            *evidence |= Evidence::known_to(self.viewer);
        }
    }
    pub(in crate::execution) fn held(&self, card: &mut HeldCard) {
        self.reference(&mut card.inner, &mut card.evidence);
    }
    pub(in crate::execution) fn held_optional(&self, card: &mut Option<HeldCard>) {
        if let Some(card) = card {
            self.held(card);
        }
    }
    pub(in crate::execution) fn parameter(&self, parameter: &mut SelectionParameter) {
        if let SelectionParameter::Card(card) = parameter {
            self.held(card);
        }
    }
    /// An untagged pending target: the card at its current position. Its
    /// zone is unchanged, and a visible or known card samples as itself.
    pub(in crate::execution) fn target(&self, target: &mut Target) {
        self.physical_target(target);
    }
    fn result_card(&self, card: &mut Option<CardId>, evidence: &mut Evidence) {
        match card {
            Some(card) => self.reference(card, evidence),
            None if self.rules_only() => *evidence = Evidence::NONE,
            None => {}
        }
    }
    pub(in crate::execution) fn outcome(&self, outcome: &mut HeldOutcome) {
        self.result_card(&mut outcome.inner.card, &mut outcome.evidence);
    }
    pub(in crate::execution) fn outcome_option(&self, outcome: &mut Option<HeldOutcome>) {
        if let Some(outcome) = outcome {
            self.outcome(outcome);
        }
    }
    pub(in crate::execution) fn bulk(&self, outcome: &mut HeldBulk) {
        self.result_card(&mut outcome.inner.card, &mut outcome.evidence);
    }
    pub(in crate::execution) fn returned(&self, value: &mut HeldReturn) {
        let card = match &mut value.inner {
            ReturnValue::Operation(outcome) => &mut outcome.card,
            ReturnValue::Bulk(outcome) => &mut outcome.card,
            _ => &mut None,
        };
        self.result_card(card, &mut value.evidence);
    }
    pub(in crate::execution) fn context(&self, context: &mut Context) {
        if let Some(card) = context.source_card {
            self.source(card, context.source_evidence);
        }
        if self.rules_only() {
            context.source_evidence = Evidence::NONE;
        } else if matches!(self.mode, Mode::Sample { .. }) && self.names(context.source_evidence) {
            // Rebuilt menus inherit this context rather than the old menu's
            // source reference. Keep only the same pick's acquired identity
            // when the sampled continuation has no trackers.
            context.source_evidence |= Evidence::known_to(self.viewer);
        }
    }
    pub(in crate::execution) fn regs(&self, regs: &mut Regs) {
        let Regs {
            last,
            bound,
            chosen,
            boxed: _,
            locals: _,
        } = regs;
        self.returned(last);
        self.held_optional(bound);
        self.parameter(chosen);
    }
    fn action(&self, action: &mut Action) {
        match action {
            Action::Meld { card } | Action::Achieve { card } => self.physical(card),
            Action::Draw | Action::Dogma { .. } | Action::Decree => {}
        }
    }
    fn board_destination(&self, card: CardId, destination: &mut Zone) {
        if matches!(self.mode, Mode::Sample { .. })
            && let Zone::Board(_, color) = destination
        {
            *color = self.position.card(card).color;
        }
    }
    fn operation(&self, request: &mut Operation) {
        match request {
            Operation::Move {
                target,
                destination,
                ..
            } => {
                self.target(target);
                self.board_destination(target.card, destination);
            }
            Operation::Claim { target, .. } | Operation::Reveal { target } => {
                self.physical_target(target)
            }
            Operation::ExecuteEffect { card, .. } => self.source(*card, Evidence::NONE),
            Operation::Dogma { .. } | Operation::ShareBonus { .. } => {}
            Operation::IssueDecree { .. }
            | Operation::ClaimDecree { .. }
            | Operation::ReturnDecree { .. }
            | Operation::Draw { .. }
            | Operation::DrawAction { .. }
            | Operation::IntroduceAgeZero
            | Operation::Shuffle { .. }
            | Operation::Splay { .. }
            | Operation::ExchangeAll { .. }
            | Operation::ClaimSpecial { .. }
            | Operation::JunkAvailableSpecial { .. }
            | Operation::RestoreSpecial { .. }
            | Operation::CheckAchievementVictory
            | Operation::ProposeDraw { .. }
            | Operation::RequireFade { .. }
            | Operation::ProposeWin { .. }
            | Operation::Lose { .. }
            | Operation::Flip
            | Operation::Ineligible { .. }
            | Operation::ClaimMuseum { .. }
            | Operation::DrawTop { .. } => {}
        }
    }
    fn descriptor(&self, descriptor: &mut TargetDescriptor) {
        if let TargetDescriptor::Known { card, .. } = descriptor {
            self.physical(card);
        }
    }
    fn alternative(&self, alternative: &mut Alternative) {
        match alternative {
            Alternative::MatchingPair { first, second } => {
                self.descriptor(first);
                self.descriptor(second);
            }
            Alternative::Select { target }
            | Alternative::Move { target, .. }
            | Alternative::Reveal { target } => self.descriptor(target),
            Alternative::SelfExecute(card) | Alternative::Showcase { card, .. } => {
                self.physical(card)
            }
            Alternative::Value(_)
            | Alternative::Count(_)
            | Alternative::Icon(_)
            | Alternative::BonusIcon
            | Alternative::Colors(_)
            | Alternative::Branch(_)
            | Alternative::BlindSelect { .. }
            | Alternative::Decree(_)
            | Alternative::Draw { .. }
            | Alternative::Splay { .. }
            | Alternative::Exchange { .. }
            | Alternative::BlindMove { .. }
            | Alternative::Deck(_)
            | Alternative::NameFigure(_)
            | Alternative::Pass
            | Alternative::Decline
            | Alternative::JunkSpecial(_)
            | Alternative::RestoreSpecial(_)
            | Alternative::ClaimSpecial(_)
            | Alternative::Karma { .. } => {}
        }
    }
    fn hook(&self, hook: &mut Hook, entered: bool) {
        // A triggered clause names its public Figure, even if it later moves.
        // The sample keeps the operand's back, so only a front-reading
        // condition, or an entered body that reads a front, would retain a
        // real-world hidden predicate: those reject sampling.
        let clause = hook.figure.def().karma[usize::from(hook.clause)];
        if !self.rules_only()
            && hook.event.unknown_operand(self)
            && (clause.when.is_some_and(Cond::reads_card_front)
                || entered && clause.body_reads_card_front())
        {
            self.opaque.set(Some(HIDDEN_FRONT));
        }
        hook.event.remap(self);
    }
    pub(in crate::execution) fn completed(&self, value: &mut HeldReturn) {
        self.returned(value);
    }
    fn task(&self, task: &mut Task) {
        match task {
            Task::Operation(_, context)
            | Task::CapturedOperation(_, context, _)
            | Task::SelfExecute { context, .. }
            | Task::NameFigure { context, .. }
            | Task::FindFigure { context, .. } => self.context(context),
            Task::DrawFrom(draw) => self.context(&mut draw.context),
            _ => {}
        }
        match task {
            Task::Verify(task) => self.task(task),
            Task::VerifyWhen(proof) => {
                self.task(&mut proof.task);
                self.regs(&mut proof.regs);
            }
            Task::MoveAllReference(reference) => {
                self.reference(&mut reference.inner.card, &mut reference.evidence)
            }
            Task::TerminalPick(target)
            | Task::DecreeReturn(target)
            | Task::ExchangeCard(target)
            | Task::GroupCard(target)
            | Task::JunkAchievement(target)
            | Task::FadeCard { target }
            | Task::MoveAllCard { target } => self.physical_target(target),
            Task::OpeningMeld { card, .. }
            | Task::BindCard(card)
            | Task::BlockCard(card)
            | Task::ChooseCard(card)
            | Task::SelfExecute { card, .. }
            | Task::Showcase { card, .. }
            | Task::SearchReturn(card) => self.physical(card),
            Task::ChooseBlind(blind) | Task::MoveAllBlind(blind) => {
                for target in &mut blind.targets {
                    self.physical_target(target);
                }
                for reference in &mut blind.excluded {
                    self.held(reference);
                }
            }
            Task::DecreeBlindReturn(blind)
            | Task::JunkAchievementBlind(blind)
            | Task::ExchangeBlind(blind) => {
                for target in &mut blind.targets {
                    self.physical_target(target);
                }
            }
            Task::GroupBlind(blind) => {
                for target in &mut blind.targets {
                    self.physical_target(target);
                }
            }
            Task::BlindOperations(group) => {
                self.context(&mut group.context);
                for reference in &mut group.excluded {
                    self.held(reference);
                }
                for (operation, reference) in &mut group.operations {
                    if let Some(reference) = reference {
                        self.held(reference);
                        match operation {
                            Operation::Move {
                                target,
                                destination,
                                ..
                            } => {
                                target.card = reference.inner;
                                self.board_destination(target.card, destination);
                            }
                            Operation::Claim { target, .. } | Operation::Reveal { target } => {
                                target.card = reference.inner;
                            }
                            _ => unreachable!("captured blind operation"),
                        }
                    } else {
                        self.operation(operation);
                    }
                }
            }
            Task::ChooseColors(parameter) => self.parameter(parameter),
            Task::ChooseReference(card) | Task::BindReference(card) => self.held(card),
            Task::MatchingPair { first, second } => {
                self.physical_target(first);
                self.physical_target(second);
            }
            Task::CapturedOperation(operation, _, reference) => {
                self.held(reference);
                match operation {
                    Operation::Move {
                        target,
                        destination,
                        ..
                    } => {
                        target.card = reference.inner;
                        self.board_destination(target.card, destination);
                    }
                    Operation::Reveal { target } | Operation::Claim { target, .. } => {
                        target.card = reference.inner
                    }
                    _ => unreachable!("captured menu operand is a move, reveal or claim"),
                }
            }
            Task::Operation(operation, _) => match operation {
                Operation::Move {
                    target,
                    destination,
                    ..
                } => {
                    self.physical_target(target);
                    self.board_destination(target.card, destination);
                }
                other => self.operation(other),
            },
            Task::Hook(hook) => self.hook(hook, false),
            Task::FusionValue(_)
            | Task::DecreeValue(_)
            | Task::ChooseValue(_)
            | Task::ChooseCount(_)
            | Task::BlockItem(_)
            | Task::BlockBlind(_)
            | Task::ChooseIcon(_)
            | Task::Branch(_)
            | Task::Stage(_)
            | Task::SplayEach { .. }
            | Task::BeginExchange
            | Task::DeclineExchange
            | Task::Decline
            | Task::DrawFrom(_)
            | Task::JunkSpecial(_)
            | Task::JunkAchievementSpecial(_)
            | Task::StopMoves
            | Task::StopGroup
            | Task::TakeSelected { .. }
            | Task::Endorse(_)
            | Task::DogmaIcon(_)
            | Task::NameFigure { .. }
            | Task::FindFigure { .. } => {}
        }
    }
    pub(in crate::execution) fn frames(&self, frames: &mut [Frame]) {
        for frame in frames {
            self.frame(frame);
        }
    }
    fn frame(&self, frame: &mut Frame) {
        // Source identities and rule contexts must not retain an omniscient
        // private front just because they are not physical movement targets.
        match frame {
            Frame::Program {
                source, context, ..
            } => {
                let (Source::Effect { card, .. }
                | Source::Karma { card, .. }
                | Source::CityIcon { card, .. }) = *source;
                let evidence = if context.source_card == Some(card) {
                    context.source_evidence
                } else {
                    Evidence::NONE
                };
                self.source(card, evidence);
                self.context(context);
            }
            // The action's public source remains that identity after moving.
            Frame::Dogma(dogma) => {
                if let Some(extra) = &mut dogma.extra {
                    extra.memory.remap(self);
                }
            }
            Frame::DecreePayment { context, .. }
            | Frame::JunkAvailableSpecials { context, .. }
            | Frame::ActionTail { context, .. }
            | Frame::EndAction { context, .. }
            | Frame::EndSpecials { context, .. }
            | Frame::MoveSelected { context, .. }
            | Frame::NameFigure { context, .. }
            | Frame::FindFigure { context, .. }
            | Frame::Operation { context, .. } => self.context(context),
            Frame::Subgame(step) => self.context(step.context_mut()),
            _ => {}
        }
        match frame {
            Frame::TerminalEffect(progress) => progress.remap(self),
            Frame::Opening(Opening { picks }) => {
                for (player, pick) in Player::ALL.into_iter().zip(picks) {
                    let Some(card) = pick else { continue };
                    if matches!(self.mode, Mode::Sample { .. }) && player != self.viewer {
                        // The secret commitment's position is unknown too.
                        // Setup deals one back, and sampling has uniformly
                        // permuted this hidden hand: its first card is a fresh
                        // commitment independent of the real selected slot.
                        *card = self.position.cards(Zone::Hand(player))[0];
                    } else {
                        self.physical(card);
                    }
                }
            }
            Frame::ActionReturns { last, memory, .. } => {
                self.returned(last);
                if let Some(memory) = memory {
                    memory.remap(self);
                }
            }
            Frame::Decree(progress) => progress.remap(self),
            Frame::ReturnOutcome(outcome) => self.outcome(outcome),
            Frame::KarmaProof { saved, revealed } => {
                self.frame(saved);
                self.held_optional(revealed);
            }
            Frame::ActionTail { action, last, .. } => {
                self.action(action);
                self.outcome_option(last);
            }
            Frame::Fading(fading) => {
                let Fading {
                    context,
                    visited,
                    awaiting_requirement: _,
                    last,
                } = fading.as_mut();
                self.context(context);
                for card in visited {
                    self.held(card);
                }
                self.outcome_option(last);
            }
            Frame::MoveAll(batch) => {
                let MoveAll {
                    verb: _,
                    from: _,
                    to: _,
                    context,
                    regs,
                    ordered_since: _,
                    seen,
                    visited,
                    outcome,
                    limit: _,
                    stop: _,
                    shortcut: _,
                    per_color: _,
                    colors_done: _,
                    quotas: _,
                    kept: _,
                    frozen,
                    frozen_selection,
                    verify: _,
                    when: _,
                    proofs,
                    proving: _,
                } = batch.as_mut();
                self.context(context);
                self.regs(regs);
                for card in visited.iter_mut().chain(proofs) {
                    self.held(card);
                }
                for card in seen {
                    self.physical(card);
                }
                if let Some(frozen) = frozen {
                    for target in frozen {
                        // Snapshot predicates may have inspected the selector owner's
                        // private fronts. Until cohort conditioning is represented,
                        // unknown snapshot membership cannot cross observers. A
                        // deck position (Dragon's Lair's deck tops) is membership
                        // by place alone; its pick follows the sampled card there.
                        if *frozen_selection
                            && !self.rules_only()
                            && !target.evidence.positional()
                            && !self.names(target.evidence)
                        {
                            self.opaque
                                .set(Some("a frozen selection contains an unnamed original"));
                        }
                        self.reference(&mut target.inner.card, &mut target.evidence);
                    }
                }
                self.bulk(outcome);
            }
            Frame::SplayAll(batch) => {
                let SplayAll {
                    step: _,
                    context,
                    regs,
                    done: _,
                    outcome,
                } = batch.as_mut();
                self.context(context);
                self.regs(regs);
                self.bulk(outcome);
            }
            Frame::MatchingPair(pair) => pair.remap(self),
            Frame::SelectGroup(group) => group.remap(self),
            Frame::JunkAvailable(batch) => batch.remap(self),
            Frame::RevealAll(packet) => packet.remap(self),
            Frame::DrawCount(DrawCount {
                remaining: _,
                value: _,
                set: _,
                then: _,
                colors: _,
                repeated_color: _,
                context,
                stage: _,
                last,
                outcome,
            }) => {
                self.context(context);
                self.returned(last);
                self.bulk(outcome);
            }
            Frame::SelectExchange(selection) => selection.remap(self),
            Frame::Exchange { plan, .. } => {
                let ExchangePlan {
                    left,
                    right,
                    left_destination: _,
                    right_destination: _,
                    context,
                } = plan.as_mut();
                self.context(context);
                for target in left.iter_mut().chain(right) {
                    self.target(target);
                }
            }
            Frame::MoveSelected { targets, .. } => {
                for target in targets {
                    self.reference(&mut target.inner.card, &mut target.evidence);
                }
            }
            Frame::Operation {
                request,
                context,
                knowledge,
                ..
            } => {
                if !self.rules_only()
                    && knowledge
                        .is_some_and(|k| k.private_query.is_some_and(|owner| owner != self.viewer))
                {
                    self.opaque.set(Some(PRIVATE_PROOF));
                }
                if !self.rules_only()
                    && knowledge.is_some_and(|k| k.conditioned && !self.names(k.evidence))
                {
                    self.opaque.set(Some(HIDDEN_FRONT));
                }
                match (request, knowledge.as_mut()) {
                    // A pending operand karma may have run past follows the
                    // pick its Would event saw.
                    (
                        Operation::Move {
                            verb: _,
                            target,
                            destination,
                        },
                        Some(knowledge),
                    ) => {
                        self.reference(&mut target.card, &mut knowledge.evidence);
                        self.board_destination(target.card, destination);
                    }
                    (
                        Operation::Claim { target, .. } | Operation::Reveal { target },
                        Some(knowledge),
                    ) => self.reference(&mut target.card, &mut knowledge.evidence),
                    (request, knowledge) => {
                        if let Some(knowledge) = knowledge {
                            // A draw's operand: the draw takes whatever is
                            // on top, but the pick stays live for its event.
                            self.erase(&mut knowledge.evidence);
                        }
                        if let Operation::ExecuteEffect { card, .. } = request
                            && context.source_card == Some(*card)
                        {
                            self.source(*card, context.source_evidence);
                        } else {
                            self.operation(request);
                        }
                    }
                }
                if self.rules_only() {
                    *knowledge = None;
                }
            }
            Frame::Execute(execution) => execution.remap(self),
            Frame::Locals(locals) => locals.remap(self),
            Frame::Program {
                source: _,
                pc: _,
                context: _,
                last,
                retained,
                chosen,
                boxed: _,
            } => {
                self.returned(last);
                self.held_optional(retained);
                self.parameter(chosen);
            }
            Frame::Choose { tasks, options, .. } => {
                for task in tasks.iter_mut() {
                    self.task(task);
                }
                self.held_optional(&mut options.source);
                fn named_operand(task: &Task) -> Option<CardId> {
                    match task {
                        Task::MoveAllReference(reference) => Some(reference.card),
                        Task::CapturedOperation(_, _, reference)
                        | Task::ChooseReference(reference)
                        | Task::BindReference(reference) => Some(reference.inner),
                        Task::Verify(task) => named_operand(task),
                        Task::VerifyWhen(proof) => named_operand(&proof.task),
                        _ => None,
                    }
                }
                for (option, task) in options.alternatives.iter_mut().zip(tasks) {
                    if let (
                        Alternative::Select {
                            target: TargetDescriptor::Known { card, .. },
                        }
                        | Alternative::Move {
                            target: TargetDescriptor::Known { card, .. },
                            ..
                        }
                        | Alternative::Reveal {
                            target: TargetDescriptor::Known { card, .. },
                        },
                        Some(reference),
                    ) = (&mut *option, named_operand(task))
                    {
                        *card = reference;
                    } else {
                        self.alternative(option);
                    }
                }
            }
            Frame::Karma {
                hook,
                stage: _,
                visited,
                last,
            } => {
                self.hook(hook, true);
                for card in visited {
                    self.held(card);
                }
                self.outcome_option(last);
            }
            Frame::Dogma(_)
            | Frame::Subgame(_)
            | Frame::DecreePayment { .. }
            | Frame::JunkAvailableSpecials { .. }
            | Frame::EndAction { .. }
            | Frame::EndSpecials { .. }
            | Frame::NameFigure { .. }
            | Frame::FindFigure { .. } => {}
            // Public cards: the melded card on top, a revealed packet.
            Frame::MeldAction(meld) => {
                self.physical(&mut meld.card);
                self.context(&mut meld.context);
            }
            Frame::Showcase(showcase) => self.physical(&mut showcase.card),
            Frame::MuseumCheck(check) => self.context(&mut check.context),
            Frame::DogmaPrep(prep) => {
                self.physical(&mut prep.card);
                self.context(&mut prep.context);
            }
            Frame::Promote(promotion) => promotion.remap(self),
            Frame::CitySearch(search) => {
                self.physical(&mut search.city);
                self.context(&mut search.context);
                for card in search.pending.iter_mut().chain(&mut search.chosen) {
                    self.physical(card);
                }
            }
        }
    }
}
