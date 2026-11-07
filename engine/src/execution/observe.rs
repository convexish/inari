//! Viewer-safe projections of the running continuation for observation
//! encoders: the current prompt, and the cards the pending work refers to.
//! A reference names a card only if this viewer knows it or its tracker pins
//! the reference's pick to one known card; otherwise only its public back.
use super::*;
mod context;
#[cfg(test)]
mod tests;
pub(in crate::execution) use context::Projection;

/// The prompt a settled game is waiting at.
pub(crate) enum PromptView {
    Decision(Decision),
    Turn(Player),
    Over,
    /// Mid-scheduler: advance to a prompt first.
    Busy,
}

/// What a viewer may know about a referenced card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sight {
    Card(CardId),
    Back(Deck),
}

/// Why the continuation refers to a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Role {
    /// The effect or karma asking the current question.
    DecisionSource = 1,
    /// A running card effect; detail is its effect index.
    Effect,
    /// A running karma clause's Figure; detail is the clause.
    Karma,
    /// The card of an ongoing Dogma action; detail is its next effect.
    Dogma,
    /// A card being self- or super-executed.
    Executed,
    /// The operand of a suspended operation (a pending meld, say).
    Operand,
    /// The card a running karma reacted to.
    KarmaOperand,
    /// A card a running program retains in its registers.
    Register,
    /// A City whose Meld-action icons are running; detail is the icon kind
    /// (Search after the nine [`CityIcon`]s).
    CityIcon,
    /// A revealed Search card waiting to be returned.
    Packet,
    /// A named effect slot; detail identifies the slot.
    Slot,
    /// The current card of a running block; detail identifies the item slot.
    BlockItem,
    /// A committed or still being selected group, in selection order.
    Group,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContextRef {
    pub role: Role,
    pub sight: Sight,
    pub detail: u8,
}

impl Game {
    /// Resolve the retained source's acquired knowledge before the public
    /// snapshot drops its pick. Sampling and real play use the same evidence;
    /// a card's current physical identity or location cannot name the source.
    pub(super) fn decision_snapshot(&self, player: Player, options: &DecisionOptions) -> Decision {
        let source = options.source.map(|mut source| {
            for viewer in Player::ALL {
                if self.reference_visible(source, viewer) {
                    source.evidence |= Evidence::known_to(viewer);
                }
            }
            source
        });
        // The innermost running program of the source card itself names
        // the effect; copied executions and karma keep their own sources.
        let effect = source.and_then(|source| {
            self.frames.iter().rev().find_map(|frame| match frame {
                Frame::Program {
                    source: Source::Effect { card, effect },
                    ..
                } if *card == source.inner => Some(*effect),
                _ => None,
            })
        });
        Decision::new(
            player,
            options.alternatives.clone(),
            source,
            effect,
            options.branches,
        )
    }

    pub(crate) fn prompt_view(&self) -> PromptView {
        if self.terminal.is_some() {
            return PromptView::Over;
        }
        match self.frames.last() {
            None => PromptView::Turn(self.position.turn_owner),
            Some(Frame::Choose {
                player,
                tasks,
                options,
            }) if tasks.len() > 1 => PromptView::Decision(self.decision_snapshot(*player, options)),
            Some(_) => PromptView::Busy,
        }
    }

    /// A reference as this viewer may see it: never named just because the
    /// real card now lies in view, which would tell the viewer where an
    /// unseen pick went.
    pub(in crate::execution) fn sight(
        &self,
        card: CardId,
        evidence: Evidence,
        viewer: Player,
        source: bool,
    ) -> Sight {
        let visible = || {
            self.position
                .locate(card)
                .is_some_and(|zone| self.position.can_view(zone, viewer))
        };
        let named = evidence.known(viewer)
            || match (self.tracker(viewer), evidence.pick()) {
                (Some(tracker), Some(pick)) => tracker.pick_identity(pick).is_some(),
                // A running program's source is pinned by contract, and
                // untracked play has no beliefs: only these use what is in
                // view now. Other pickless references stay anonymous.
                (Some(_), None) => source && visible(),
                (None, _) => visible(),
            };
        if named {
            Sight::Card(card)
        } else {
            Sight::Back(self.position.card(card).origin_deck())
        }
    }

    /// Cards the pending work refers to, outermost first, as this viewer
    /// may see them. Bounded by the frame stack.
    pub(crate) fn context_refs(&self, viewer: Player) -> Vec<ContextRef> {
        let mut refs = Vec::new();
        let mut push = |role, card: Option<(CardId, Evidence)>, detail: usize| {
            if let Some((card, evidence)) = card {
                let source = matches!(role, Role::Effect | Role::Karma);
                refs.push(ContextRef {
                    role,
                    sight: self.sight(card, evidence, viewer, source),
                    detail: detail.min(255) as u8,
                });
            }
        };
        let held = |card: HeldCard| (card.inner, card.evidence);
        for frame in &self.frames {
            match frame {
                Frame::Program {
                    source,
                    context,
                    retained,
                    last,
                    ..
                } => {
                    let (role, card, detail) = match *source {
                        // An echo follows the at most three dogma effects.
                        Source::Effect { card, effect } if effect == Card::ECHO => {
                            (Role::Effect, card, 3)
                        }
                        Source::Effect { card, effect } => {
                            (Role::Effect, card, usize::from(effect))
                        }
                        Source::Karma { card, clause } => (Role::Karma, card, usize::from(clause)),
                        Source::CityIcon { card, icon } => (Role::CityIcon, card, icon as usize),
                    };
                    let evidence = if context.source_card == Some(card) {
                        context.source_evidence
                    } else {
                        Evidence::NONE
                    };
                    push(role, Some((card, evidence)), detail);
                    push(Role::Register, retained.map(held), 0);
                    push(Role::Register, last.card().map(held), 1);
                }
                Frame::Locals(locals) => {
                    for (index, slot) in locals.slots.iter().enumerate() {
                        if let Some(slots::Held::Cards(cards)) = slot {
                            for &card in cards.as_slice() {
                                push(Role::Slot, Some(held(card)), index);
                            }
                        }
                    }
                    for block in &locals.blocks {
                        let Some(slot) = block.item_slot() else {
                            continue;
                        };
                        if let Some(slots::Held::Cards(cards)) =
                            &locals.slots[usize::from(slot.index())]
                        {
                            for &card in cards.as_slice() {
                                push(Role::BlockItem, Some(held(card)), usize::from(slot.index()));
                            }
                        }
                    }
                }
                Frame::SelectGroup(group) => {
                    for card in group.selected_cards() {
                        push(Role::Group, Some(held(card)), 0);
                    }
                }
                Frame::MoveAll(batch) => {
                    if let Some(cards) = &batch.frozen {
                        for card in cards {
                            push(Role::Group, Some(held(card.with(card.card))), 0);
                        }
                    }
                }
                Frame::Dogma(dogma) => push(
                    Role::Dogma,
                    Some((dogma.card, Evidence::PUBLIC)),
                    dogma.effect,
                ),
                Frame::Execute(execution) => push(Role::Executed, Some(held(execution.card())), 0),
                Frame::Operation {
                    request: Operation::Move { verb, target, .. },
                    knowledge,
                    ..
                } => {
                    let evidence = knowledge.map_or_else(
                        || Evidence::selection(&self.position, target.physical),
                        |knowledge| knowledge.evidence,
                    );
                    push(Role::Operand, Some((target.card, evidence)), *verb as usize);
                }
                Frame::DogmaPrep(prep) => {
                    push(Role::Dogma, Some((prep.card, Evidence::PUBLIC)), 0);
                }
                Frame::MeldAction(meld) => {
                    push(Role::CityIcon, Some((meld.card, Evidence::PUBLIC)), 0);
                }
                Frame::CitySearch(search) => {
                    push(Role::CityIcon, Some((search.city, Evidence::PUBLIC)), 9);
                    for &card in &search.pending {
                        push(Role::Packet, Some((card, Evidence::PUBLIC)), 0);
                    }
                }
                Frame::Karma { hook, .. } => {
                    push(
                        Role::Karma,
                        Some((hook.source, Evidence::PUBLIC)),
                        usize::from(hook.clause),
                    );
                    push(Role::KarmaOperand, hook.event.operand().map(held), 0);
                }
                _ => {}
            }
        }
        if let PromptView::Decision(decision) = self.prompt_view()
            && decision.player == viewer
            && let Some(card) = decision.source_card(viewer)
        {
            push(Role::DecisionSource, Some((card, Evidence::PUBLIC)), 0);
        }
        refs
    }
}
