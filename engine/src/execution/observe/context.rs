//! Semantic continuation records. This is an immutable observer projection,
//! not a serialization of scheduler frames, tasks, pointers or physical slots.
use super::*;
use crate::observation::context::{Field as F, RecordKind as K};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextRecord {
    pub kind: K,
    pub parent: usize,
    pub sight: Option<Sight>,
    pub fields: Vec<(F, u64)>,
}
pub(in crate::execution) struct Projection<'a> {
    pub game: &'a Game,
    pub viewer: Player,
    pub rows: Vec<ContextRecord>,
}
impl Projection<'_> {
    pub fn add(
        &mut self,
        kind: K,
        parent: usize,
        sight: Option<Sight>,
        fields: &[(F, u64)],
    ) -> usize {
        self.rows.push(ContextRecord {
            kind,
            parent,
            sight,
            fields: fields.to_vec(),
        });
        self.rows.len()
    }
    pub fn field(&mut self, id: usize, field: F, value: u64) {
        self.rows[id - 1].fields.push((field, value));
    }
    pub fn player(&self, player: Player) -> u64 {
        if player == self.viewer { 1 } else { 2 }
    }
    pub fn sight(&self, reference: HeldCard) -> Sight {
        self.game
            .sight(reference.inner, reference.evidence, self.viewer, false)
    }
    pub fn reference(
        &mut self,
        kind: K,
        parent: usize,
        reference: HeldCard,
        fields: &[(F, u64)],
    ) -> usize {
        self.add(kind, parent, Some(self.sight(reference)), fields)
    }
    pub fn context(&mut self, id: usize, context: Context) {
        for (field, player) in [
            (F::Executor, context.executor),
            (F::Controller, context.controller),
            (F::Demander, context.demander),
            (F::SourceOwner, context.source_owner),
        ] {
            self.field(id, field, self.player(player));
        }
        self.field(id, F::Suppressed, u64::from(context.suppress_karma()));
    }
    pub fn outcome(&mut self, id: usize, value: HeldReturn) {
        self.field(id, F::Performed, u64::from(value.performed_count()));
        self.field(id, F::PublicOutcome, u64::from(value.inner.fulfilled()));
        if let Some(reference) = value.card() {
            self.reference(K::Record, id, reference, &[(F::Last, 1)]);
        }
        if let ReturnValue::Bulk(outcome) = value.inner {
            self.field(id, F::Replaced, u64::from(outcome.replaced));
            self.field(id, F::Failed, u64::from(outcome.inapplicable));
        }
    }
    pub fn records(&mut self, parent: usize, records: &slots::Records) {
        for (ordinal, (reference, origin)) in records.observed().enumerate() {
            self.reference(
                K::Record,
                parent,
                reference,
                &[
                    (F::Ordinal, ordinal as u64),
                    (F::Origin, origin.map_or(0, |owner| self.player(owner))),
                ],
            );
        }
    }
    pub fn held(&mut self, id: usize, held: Option<&slots::Held>, known: u8) {
        if known & (1 << self.viewer.index()) == 0 {
            self.field(id, F::ScalarKind, 5);
            self.field(id, F::Known, 0);
            return;
        }
        self.field(id, F::Known, 1);
        match held {
            None => self.field(id, F::ScalarKind, 0),
            Some(slots::Held::Cards(records)) => {
                self.field(id, F::ScalarKind, 1);
                self.records(id, records)
            }
            Some(slots::Held::Value(value)) => {
                self.field(id, F::ScalarKind, 2);
                self.field(id, F::Scalar, u64::from(*value));
            }
            Some(slots::Held::Colors(colors)) => {
                self.field(id, F::ScalarKind, 3);
                self.field(id, F::Scalar, u64::from(*colors));
            }
            Some(slots::Held::Icon(icon)) => {
                self.field(id, F::ScalarKind, 4);
                self.field(id, F::Scalar, *icon as u64 + 1);
            }
        }
    }
    pub fn slot(
        &mut self,
        parent: usize,
        slot: (Life, u8),
        owner: Player,
        source: Option<HeldCard>,
        held: Option<&slots::Held>,
        known: u8,
    ) -> usize {
        let (life, index) = slot;
        let id = self.add(
            K::Slot,
            parent,
            source.map(|source| self.sight(source)),
            &[
                (F::Life, life as u64),
                (F::Slot, u64::from(index)),
                (F::Owner, self.player(owner)),
            ],
        );
        self.held(id, held, known);
        id
    }
    pub fn parameter(&mut self, parent: usize, chosen: SelectionParameter) {
        let id = self.add(K::Parameter, parent, None, &[(F::Chosen, 1)]);
        match chosen {
            SelectionParameter::None => self.field(id, F::ScalarKind, 0),
            SelectionParameter::Value(value) => self.held(id, Some(&slots::Held::Value(value)), 3),
            SelectionParameter::Count(count) => {
                self.field(id, F::Known, 1);
                self.field(id, F::ScalarKind, 6);
                self.field(id, F::Scalar, u64::from(count));
            }
            SelectionParameter::Colors(colors) => {
                self.held(id, Some(&slots::Held::Colors(colors)), 3)
            }
            SelectionParameter::Icon(icon) => self.held(id, Some(&slots::Held::Icon(icon)), 3),
            SelectionParameter::Card(reference) => {
                self.field(id, F::ScalarKind, 1);
                self.reference(K::Record, id, reference, &[(F::Ordinal, 0)]);
            }
        }
    }
    pub fn schedule(&mut self, parent: usize, schedule: Option<&Schedule>, next: usize) {
        self.field(parent, F::Frozen, u64::from(schedule.is_some()));
        self.field(parent, F::Cursor, next as u64);
        if let Some(schedule) = schedule {
            for (ordinal, &(card, effect)) in schedule.iter().enumerate() {
                self.reference(
                    K::Schedule,
                    parent,
                    Captured::new(card, Evidence::PUBLIC),
                    &[(F::Ordinal, ordinal as u64), (F::Effect, u64::from(effect))],
                );
            }
        }
    }
    /// Exchange selections are anonymous from hidden sources, even if a
    /// remembered candidate happens to equal the real physical selection.
    pub fn exchange_target(&mut self, parent: usize, target: Target, side: usize, ordinal: usize) {
        let sight = if self.game.position.can_view(target.physical, self.viewer) {
            Sight::Card(target.card)
        } else {
            Sight::Back(self.game.position.card(target.card).origin_deck())
        };
        self.add(
            K::Record,
            parent,
            Some(sight),
            &[
                (F::Side, side as u64),
                (F::Ordinal, ordinal as u64),
                (
                    F::SourceZone,
                    u64::from(crate::observation::zone_code(target.physical, self.viewer)),
                ),
            ],
        );
    }
    pub fn exchange_plan(&mut self, parent: usize, plan: &ExchangePlan, stage: u64) {
        let id = self.add(
            K::Exchange,
            parent,
            None,
            &[(F::Stage, stage), (F::Committing, 1)],
        );
        self.context(id, plan.context);
        for (side, (cards, destination)) in [
            (&plan.left, plan.left_destination),
            (&plan.right, plan.right_destination),
        ]
        .into_iter()
        .enumerate()
        {
            let side_id = self.add(
                K::Group,
                id,
                None,
                &[
                    (F::Side, side as u64),
                    (
                        F::Destination,
                        u64::from(crate::observation::zone_code(destination, self.viewer)),
                    ),
                ],
            );
            for (ordinal, &target) in cards.iter().enumerate() {
                self.exchange_target(side_id, target, side, ordinal);
            }
        }
    }
}

impl Game {
    pub(crate) fn context_records(&self, viewer: Player) -> Vec<ContextRecord> {
        if !self.continuation_flow_known(viewer) {
            return vec![ContextRecord {
                kind: K::Opaque,
                parent: 0,
                sight: None,
                fields: vec![(F::Known, 0)],
            }];
        }
        let mut p = Projection {
            game: self,
            viewer,
            rows: Vec::new(),
        };
        // Existing specialized City/karma references retain the same evidence
        // policy. Structured records below add scopes, order and scalar state.
        for reference in self.context_refs(viewer).into_iter().filter(|reference| {
            matches!(
                reference.role,
                Role::CityIcon | Role::Packet | Role::DecisionSource
            )
        }) {
            p.add(
                K::Reference,
                0,
                Some(reference.sight),
                &[
                    (F::Role, reference.role as u64),
                    (F::Detail, u64::from(reference.detail)),
                ],
            );
        }
        let (mut action, mut execution, mut effect) = (0, 0, 0);
        let mut progress_known = true;
        fn flatten<'a>(frame: &'a Frame, frames: &mut Vec<&'a Frame>) {
            if let Frame::KarmaProof { saved, .. } = frame {
                flatten(saved, frames)
            } else {
                frames.push(frame)
            }
        }
        let mut frames = Vec::new();
        for frame in &self.frames {
            flatten(frame, &mut frames);
        }
        for (index, frame) in frames.iter().copied().enumerate() {
            if !progress_known
                && matches!(
                    frame,
                    Frame::Operation { .. }
                        | Frame::DrawCount(_)
                        | Frame::MoveAll(_)
                        | Frame::SelectExchange(_)
                        | Frame::Exchange { .. }
                        | Frame::SelectGroup(_)
                        | Frame::MoveSelected { .. }
                        | Frame::JunkAvailable(_)
                )
            {
                let kind = match frame {
                    Frame::Operation { .. } => K::Operation,
                    Frame::SelectExchange(_) | Frame::Exchange { .. } => K::Exchange,
                    Frame::SelectGroup(_) => K::Group,
                    _ => K::Batch,
                };
                p.add(kind, effect, None, &[(F::Known, 0), (F::ScalarKind, 5)]);
                continue;
            }
            match frame {
                Frame::ActionReturns {
                    memory,
                    democracy,
                    majority,
                    ..
                } => {
                    action = p.add(K::Action, action, None, &[]);
                    execution = 0;
                    effect = 0;
                    if let Some(memory) = memory {
                        memory.project(&mut p, action, Life::Action);
                    }
                    for player in Player::ALL {
                        p.add(
                            K::Record,
                            action,
                            None,
                            &[
                                (F::Owner, p.player(player)),
                                (F::Count, u64::from(democracy[player.index()])),
                            ],
                        );
                    }
                    if let Some(player) = majority {
                        p.field(action, F::Owner, p.player(*player));
                    }
                }
                Frame::ActionTail {
                    action: body,
                    context,
                    kind,
                    stage,
                    shared,
                    endorsed,
                    promoted,
                    melded,
                    ..
                } => {
                    let id = p.add(
                        K::Action,
                        action,
                        None,
                        &[
                            (F::ActionKind, *kind as u64),
                            (F::Stage, u64::from(*stage)),
                            (F::Shared, u64::from(*shared)),
                            (F::Endorsed, u64::from(*endorsed)),
                            (F::Promoted, u64::from(*promoted)),
                            (F::PublicOutcome, u64::from(*melded)),
                        ],
                    );
                    p.context(id, *context);
                    action = id;
                    match *body {
                        Action::Draw => {
                            p.field(id, F::Verb, 10);
                        }
                        Action::Dogma { card, .. } => {
                            p.rows[id - 1].sight = Some(Sight::Card(card));
                        }
                        Action::Meld { card } => {
                            p.rows[id - 1].sight =
                                Some(Sight::Back(self.position.card(card).origin_deck()));
                        }
                        Action::Achieve { card } => {
                            p.rows[id - 1].sight =
                                Some(Sight::Back(self.position.card(card).origin_deck()));
                        }
                        Action::Decree => {}
                    }
                }
                Frame::Dogma(dogma) => {
                    execution = p.reference(
                        K::Execution,
                        action,
                        Captured::new(dogma.card, Evidence::PUBLIC),
                        &[
                            (F::Executor, p.player(dogma.player)),
                            (F::SourceOwner, p.player(dogma.source_owner)),
                            (F::Stage, u64::from(dogma.participant)),
                            (F::Shared, u64::from(dogma.executing_shared)),
                            (F::Endorsed, u64::from(dogma.endorsed)),
                            (F::Performed, u64::from(dogma.demand_transfers)),
                            (F::Count, u64::from(dogma.demand_returns)),
                        ],
                    );
                    effect = 0;
                    p.schedule(
                        execution,
                        ExecutionExtra::scheduled(&dogma.extra),
                        dogma.effect,
                    );
                    if let Some(extra) = &dogma.extra {
                        extra.memory.project(&mut p, execution, Life::Execution);
                    }
                }
                Frame::Execute(copied) => {
                    execution = copied.project(
                        &mut p,
                        if effect == 0 {
                            execution.max(action)
                        } else {
                            effect
                        },
                    );
                    effect = 0;
                }
                Frame::Locals(locals) => {
                    let context = match frames.get(index + 1).copied() {
                        Some(Frame::Program { context, .. }) => *context,
                        _ => continue,
                    };
                    effect = p.add(K::Effect, execution.max(action), None, &[]);
                    p.context(effect, context);
                    progress_known = locals.progress_known & (1 << viewer.index()) != 0;
                    p.field(effect, F::Known, u64::from(progress_known));
                    locals.project(&mut p, effect, context);
                }
                Frame::Program {
                    context,
                    source,
                    pc,
                    chosen,
                    boxed,
                    retained,
                    last,
                } => {
                    if !matches!(
                        index.checked_sub(1).and_then(|i| frames.get(i)).copied(),
                        Some(Frame::Locals(_))
                    ) {
                        progress_known = true;
                        effect = p.add(K::Effect, execution.max(action), None, &[]);
                        p.context(effect, *context);
                    }
                    let (card, code) = match *source {
                        Source::Effect { card, effect } => (card, u64::from(effect)),
                        Source::Karma { card, clause } => (card, u64::from(clause)),
                        Source::CityIcon { card, icon } => (card, icon as u64),
                    };
                    p.rows[effect - 1].sight =
                        Some(self.sight(card, context.source_evidence, viewer, true));
                    p.field(effect, F::Cursor, u64::from(*pc));
                    p.field(effect, F::Effect, code);
                    p.field(effect, F::Foreseen, u64::from(self.foreseen(*context)));
                    p.parameter(effect, *chosen);
                    // The invocation's printed box adjustment affects later clauses.
                    p.field(
                        effect,
                        F::Requested,
                        u64::from(boxed.value(0).expect("zero box cannot overflow")),
                    );
                    if let Some(reference) = retained {
                        p.reference(K::Record, effect, *reference, &[(F::Bound, 1)]);
                    }
                    p.outcome(effect, *last);
                }
                Frame::Operation {
                    request,
                    context,
                    phase,
                    knowledge,
                    short,
                } => {
                    let id = p.add(
                        K::Operation,
                        effect.max(execution).max(action),
                        None,
                        &[(F::Stage, *phase as u64), (F::Failed, u64::from(*short))],
                    );
                    p.context(id, *context);
                    let target = match *request {
                        Operation::Move {
                            verb,
                            target,
                            destination,
                        } => {
                            p.field(id, F::Verb, verb as u64 + 1);
                            p.field(
                                id,
                                F::Destination,
                                u64::from(crate::observation::zone_code(destination, viewer)),
                            );
                            Some(target)
                        }
                        Operation::Reveal { target } => {
                            p.field(id, F::Verb, 11);
                            Some(target)
                        }
                        Operation::Draw { value, set } => {
                            p.field(id, F::Verb, 10);
                            p.field(id, F::Value, u64::from(value));
                            p.field(id, F::Back, set.unwrap_or(CardSet::Base) as u64);
                            None
                        }
                        Operation::Claim {
                            target,
                            eligibility,
                        } => {
                            p.field(id, F::Verb, Verb::Achieve as u64 + 1);
                            p.field(
                                id,
                                F::Value,
                                match eligibility {
                                    ClaimEligibility::Ignore => 0,
                                    ClaimEligibility::Available => 1,
                                    ClaimEligibility::Cost { score_factor } => {
                                        u64::from(score_factor) + 1
                                    }
                                },
                            );
                            Some(target)
                        }
                        _ => None,
                    };
                    if let Some(target) = target {
                        let evidence = knowledge.map_or_else(
                            || Evidence::selection(&self.position, target.physical),
                            |known| known.evidence,
                        );
                        p.rows[id - 1].sight =
                            Some(self.sight(target.card, evidence, viewer, false));
                        p.field(
                            id,
                            F::SourceZone,
                            u64::from(crate::observation::zone_code(target.physical, viewer)),
                        );
                    }
                }
                Frame::JunkAvailable(batch) => batch.project(&mut p, effect),
                Frame::DrawCount(draw) => {
                    let id = p.add(
                        K::Batch,
                        effect,
                        None,
                        &[
                            (F::Verb, 10),
                            (F::Remaining, u64::from(draw.remaining)),
                            (F::Value, u64::from(draw.value)),
                            (F::Back, draw.set.unwrap_or(CardSet::Base) as u64),
                            (F::Mode, draw.then as u64),
                            (F::Colors, u64::from(draw.colors)),
                            (F::Failed, u64::from(draw.repeated_color)),
                            (F::Stage, u64::from(draw.stage)),
                        ],
                    );
                    p.context(id, draw.context);
                    p.outcome(id, draw.outcome.returned());
                }
                Frame::Karma {
                    hook,
                    stage,
                    visited,
                    last,
                } => {
                    let id = p.reference(
                        K::Karma,
                        effect,
                        Captured::new(hook.source, Evidence::PUBLIC),
                        &[
                            (F::Effect, u64::from(hook.clause)),
                            (F::Stage, u64::from(*stage)),
                        ],
                    );
                    if let Some(reference) = hook.event.operand() {
                        p.reference(K::Record, id, reference, &[(F::Bound, 1)]);
                    }
                    for (ordinal, &reference) in visited.iter().enumerate() {
                        p.reference(K::Record, id, reference, &[(F::Ordinal, ordinal as u64)]);
                    }
                    if let Some(outcome) = last {
                        p.outcome(id, outcome.with(ReturnValue::Operation(outcome.inner)));
                    }
                }
                Frame::MoveAll(batch) => {
                    let id = p.add(
                        K::Batch,
                        effect,
                        None,
                        &[
                            (F::Verb, batch.verb as u64 + 1),
                            (F::Count, batch.visited.len() as u64),
                            (F::Limit, batch.limit.map_or(0, |n| u64::from(n) + 1)),
                            (F::Colors, u64::from(batch.colors_done)),
                            (F::Stopped, batch.stop as u64),
                            (F::Constraint, u64::from(batch.per_color)),
                            (
                                F::Frozen,
                                if batch.frozen_selection {
                                    2
                                } else {
                                    u64::from(batch.frozen.is_some())
                                },
                            ),
                        ],
                    );
                    p.context(id, batch.context);
                    p.outcome(id, batch.outcome.returned());
                    if let Some(quotas) = batch.quotas {
                        for (value, remaining) in quotas.into_iter().enumerate() {
                            p.add(
                                K::Item,
                                id,
                                None,
                                &[
                                    (F::Value, value as u64),
                                    (F::Remaining, u64::from(remaining)),
                                ],
                            );
                        }
                    }
                    for (ordinal, &card) in batch.proofs.iter().enumerate() {
                        p.reference(
                            K::Record,
                            id,
                            card,
                            &[(F::Ordinal, ordinal as u64), (F::Known, 1)],
                        );
                    }
                    for (ordinal, &card) in batch.visited.iter().enumerate() {
                        p.reference(
                            K::Record,
                            id,
                            card,
                            &[(F::Ordinal, ordinal as u64), (F::Stage, 1)],
                        );
                    }
                    if let Some(cards) = &batch.frozen {
                        for (ordinal, card) in cards.iter().enumerate() {
                            p.reference(
                                K::Record,
                                id,
                                card.with(card.card),
                                &[(F::Ordinal, ordinal as u64), (F::Frozen, 1)],
                            );
                        }
                    }
                }
                Frame::SelectGroup(group) => group.project(&mut p, effect),
                Frame::SelectExchange(selection) => selection.project(&mut p, effect),
                Frame::Exchange { plan, phase } => p.exchange_plan(effect, plan, *phase as u64),
                Frame::MoveSelected {
                    verb,
                    targets,
                    context,
                } => {
                    let id = p.add(
                        K::Batch,
                        effect,
                        None,
                        &[(F::Verb, *verb as u64 + 1), (F::Frozen, 1)],
                    );
                    p.context(id, *context);
                    for (ordinal, target) in targets.iter().enumerate() {
                        p.reference(
                            K::Record,
                            id,
                            target.with(target.card),
                            &[(F::Ordinal, ordinal as u64)],
                        );
                    }
                }
                Frame::Promote(promotion) => promotion.project(&mut p, action),
                Frame::RevealAll(packet) => packet.project(&mut p, effect),
                _ => {}
            }
        }
        p.rows
    }
}
