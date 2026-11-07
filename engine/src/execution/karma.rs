//! Figure karma: Would/When clauses discovered from each Figure's
//! declaration ([`crate::FigureDef`]), and the few native reactions.
use super::*;

/// What a karma clause sees of an operation: who performs it, its card (the
/// card a draw would take), and a featured icon or splay color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KarmaEvent {
    pub(super) actor: Player,
    card: Option<HeldCard>,
    chosen: SelectionParameter,
}

impl KarmaEvent {
    pub(super) fn operand(&self) -> Option<HeldCard> {
        self.card
    }

    /// An operand this observer cannot name. Seeing the real card arrive
    /// somewhere visible does not name it: the observer may not know that
    /// card is the operand.
    pub(super) fn unknown_operand(&self, map: &tracking::remap::Remap<'_>) -> bool {
        self.card.is_some_and(|card| !map.names(card.evidence))
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            actor: _,
            card,
            chosen,
        } = self;
        if let Some(card) = card {
            map.held(card);
        }
        map.parameter(chosen);
    }
}

impl Game {
    /// The card a karma event may see as this operation's operand. It must
    /// cover every card-bearing arm of [`Self::karma_event`], which takes its
    /// pick from the operation's knowledge.
    pub(super) fn operand(
        &self,
        request: Operation,
        executor: Player,
    ) -> Result<Option<CardId>, Error> {
        Ok(match request {
            Operation::Move { target, .. } | Operation::Claim { target, .. } => Some(target.card),
            Operation::Draw { value, set } => self
                .draw_deck(value, set, executor)?
                .and_then(|deck| self.position.cards(Zone::Deck(deck)).last().copied()),
            Operation::ExecuteEffect { card, .. }
            | Operation::Dogma { card, .. }
            | Operation::ShareBonus { card, .. } => Some(card),
            _ => None,
        })
    }

    /// The event a clause watching `on` sees in this operation, if any.
    /// `action` is the action whose body this operation is, if it is one.
    fn karma_event(
        &self,
        on: On,
        owner: Player,
        request: Operation,
        context: Context,
        action: Option<Action>,
    ) -> Result<Option<KarmaEvent>, Error> {
        // The operation's knowledge supplies the operand's pick.
        let event = |actor, card: Option<CardId>, chosen| {
            Some(KarmaEvent {
                actor,
                card: card.map(|card| Captured::new(card, self.selection_evidence(card))),
                chosen,
            })
        };
        // The executing card is the context's source: what each observer
        // knew of it when the execution began travels with the context, even
        // after it leaves view (Escapism self-executes the card it revealed
        // and junked).
        let effect_event = |actor, card: CardId| {
            let mut evidence = self.selection_evidence(card);
            if context.source_card == Some(card) {
                evidence |= context.source_evidence;
            }
            Some(KarmaEvent {
                actor,
                card: Some(Captured::new(card, evidence)),
                chosen: SelectionParameter::None,
            })
        };
        let none = SelectionParameter::None;
        Ok(match (on, request) {
            (On::Move(verbs), Operation::Move { verb, target, .. }) if verbs.contains(&verb) => {
                event(context.executor, Some(target.card), none)
            }
            (On::DrawAction, Operation::DrawAction { .. }) => event(context.executor, None, none),
            (On::Draw, Operation::Draw { value, set }) => {
                let card = self
                    .draw_deck(value, set, context.executor)?
                    .and_then(|deck| self.position.cards(Zone::Deck(deck)).last().copied());
                event(context.executor, card, none)
            }
            (
                On::MeldAction,
                Operation::Move {
                    verb: Verb::Meld,
                    target,
                    ..
                },
            ) if matches!(action, Some(Action::Meld { .. })) => {
                event(context.executor, Some(target.card), none)
            }
            (On::DemandAgainst, Operation::ExecuteEffect { card, effect, .. })
                if self
                    .position
                    .card(card)
                    .effect(effect)
                    .is_some_and(|e| e.demand())
                    && context.executor == owner
                    && context.demander != owner =>
            {
                effect_event(context.demander, card)
            }
            (On::IssueDecree, Operation::IssueDecree { .. }) => event(context.executor, None, none),
            (On::Dogma(icon), Operation::Dogma { card, icon: used })
                if icon.is_none_or(|icon| icon == used) =>
            {
                event(context.executor, Some(card), SelectionParameter::Icon(used))
            }
            (On::Effect, Operation::ExecuteEffect { card, effect, .. })
            | (On::DogmaEffect, Operation::ExecuteEffect { card, effect, .. })
                if on == On::Effect || effect != Card::ECHO =>
            {
                // The demander executes a demand against the victim. The
                // context's executor still performs its instructions ("you"),
                // so movement/draw karma and choices keep the victim's role.
                let actor = if self
                    .position
                    .card(card)
                    .effect(effect)
                    .is_some_and(|e| e.demand())
                {
                    context.demander
                } else {
                    context.executor
                };
                effect_event(actor, card)
            }
            (On::Claim, _) if self.claim_is_applicable(request, context)? => {
                let card = match request {
                    Operation::Claim { target, .. } | Operation::Move { target, .. } => {
                        Some(target.card)
                    }
                    _ => None,
                };
                event(context.executor, card, none)
            }
            (
                On::Splay(wanted),
                Operation::Splay {
                    player,
                    color,
                    direction,
                },
            ) if player == owner
                && wanted.map_or(direction != Splay::None, |wanted| wanted == direction) =>
            {
                event(
                    context.executor,
                    None,
                    SelectionParameter::Colors(1 << color as u8),
                )
            }
            (On::Exchange, Operation::ExchangeAll { .. }) => event(context.executor, None, none),
            (On::Win, Operation::ProposeWin { winner, .. }) => event(winner, None, none),
            (On::Fade, Operation::RequireFade { player }) => event(player, None, none),
            (On::Ineligible, Operation::Ineligible { player }) => event(player, None, none),
            (On::ShareDraw | On::NoShareDraw, Operation::ShareBonus { card, icon, earned })
                if earned == (on == On::ShareDraw) =>
            {
                event(context.executor, Some(card), SelectionParameter::Icon(icon))
            }
            _ => None,
        })
    }

    /// A clause's context: performed by its owner (or the triggering player),
    /// with `Demander` naming the triggering player, under suppression.
    fn karma_context(&self, owner: Player, source: CardId, actor: Player, by: Who) -> Context {
        let executor = if by == Who::Demander { actor } else { owner };
        let mut context = Context {
            demander: actor,
            source_owner: owner,
            ..Context::reaction(executor, source)
        };
        self.inherit_override(&mut context);
        context
    }

    /// A reaction inside a controlled execution (Hitchhiking) has its decisions made by
    /// that controller too.
    pub(super) fn inherit_override(&self, context: &mut Context) {
        let controller = self.frames.iter().rev().find_map(|frame| match frame {
            Frame::Execute(execution) => Some(execution.override_controller()),
            Frame::Dogma(_) | Frame::ActionTail { .. } => Some(None),
            _ => None,
        });
        if let Some(Some(controller)) = controller {
            context.controller = controller;
            context.set_controlled();
        }
    }

    fn karma_regs(event: KarmaEvent) -> Regs {
        Regs {
            bound: event.card,
            chosen: event.chosen,
            ..Regs::EMPTY
        }
    }

    /// The trigger kinds whose clauses could match this operation; it must
    /// cover every arm of [`Self::karma_event`].
    pub(super) fn trigger_kinds(request: Operation) -> u32 {
        match request {
            Operation::Move { verb, .. } => {
                On::Move(&[]).bit()
                    | if verb == Verb::Meld {
                        On::MeldAction.bit()
                    } else {
                        0
                    }
                    | if verb == Verb::Achieve {
                        On::Claim.bit()
                    } else {
                        0
                    }
            }
            Operation::Draw { .. } => On::Draw.bit(),
            Operation::DrawAction { .. } => On::DrawAction.bit(),
            Operation::ExecuteEffect { .. } => {
                On::Effect.bit() | On::DogmaEffect.bit() | On::DemandAgainst.bit()
            }
            Operation::IssueDecree { .. } => On::IssueDecree.bit(),
            Operation::Dogma { .. } => On::Dogma(None).bit(),
            Operation::Claim { .. }
            | Operation::ClaimSpecial { .. }
            | Operation::ClaimDecree { .. }
            | Operation::ClaimMuseum { .. } => On::Claim.bit(),
            Operation::Splay { .. } => On::Splay(None).bit(),
            Operation::ExchangeAll { .. } => On::Exchange.bit(),
            Operation::ProposeWin { .. } => On::Win.bit(),
            Operation::RequireFade { .. } => On::Fade.bit(),
            Operation::Ineligible { .. } => On::Ineligible.bit(),
            Operation::ShareBonus { .. } => On::ShareDraw.bit() | On::NoShareDraw.bit(),
            _ => 0,
        }
    }

    /// Would clauses of top Figures on either board that this operation
    /// triggers, for the current player to choose among. The flag reports a
    /// triggered clause whose condition reads the operand's front: a hidden
    /// operand is revealed to prove it, whichever way it decides (Yuna Kim).
    pub(super) fn hooks(
        &self,
        request: Operation,
        context: Context,
        mut knowledge: Option<OperandKnowledge>,
    ) -> Result<(Vec<Hook>, Option<OperandKnowledge>, bool), Error> {
        let mut out = Vec::new();
        let mut proof = false;
        // The operation's frame is off the stack: an action body sits on
        // its action's tail.
        let action = match self.frames.last() {
            Some(Frame::ActionTail { action, .. }) => Some(*action),
            _ => None,
        };
        for owner in Player::ALL {
            for color in Color::ALL {
                let Some(source) = self.position.top(owner, color) else {
                    continue;
                };
                let Some(figure) = self.position.card(source).figure else {
                    continue;
                };
                for (clause, karma) in figure.def().karma.iter().enumerate() {
                    if karma.reaction == Reaction::Melded {
                        continue;
                    }
                    let Some(mut event) =
                        self.karma_event(karma.on, owner, request, context, action)?
                    else {
                        continue;
                    };
                    let guard = self.karma_context(owner, source, event.actor, Who::Executor);
                    if !self.player_matches(karma.actor, guard, event.actor) {
                        continue;
                    }
                    if let Some(card) = &mut event.card {
                        let evidence = knowledge.get_or_insert(OperandKnowledge {
                            evidence: card.evidence,
                            conditioned: false,
                            private_query: None,
                            verify: false,
                        });
                        // The operation's own reference supplies the pick; a
                        // fresh look adds identities.
                        evidence.evidence |= card.evidence;
                        card.evidence = evidence.evidence;
                        // A false condition is evidence too: filtering only
                        // the offered hooks would retain an omniscient menu.
                        let reads = karma.when.is_some_and(Cond::reads_card_front);
                        evidence.conditioned |= reads;
                        proof |= reads;
                    }
                    if !karma.when.map_or(Ok(true), |cond| {
                        self.holds(*cond, guard, Self::karma_regs(event))
                    })? {
                        continue;
                    }
                    out.push(Hook {
                        source,
                        owner,
                        figure,
                        clause: clause as u8,
                        reaction: karma.reaction,
                        event,
                    });
                }
            }
        }
        Ok((out, knowledge, proof))
    }

    /// "When you meld this card": after the Figure's own committed meld.
    #[inline]
    pub(super) fn when_melded(
        &self,
        request: Operation,
        context: Context,
        outcome: OperationOutcome,
    ) -> Option<Hook> {
        if !self.position.figures()
            || context.suppress_karma()
            || outcome.disposition != Disposition::Performed
        {
            return None;
        }
        let Operation::Move {
            verb: Verb::Meld,
            target,
            destination: Zone::Board(owner, _),
        } = request
        else {
            return None;
        };
        self.when_hook(target.card, owner, context)
    }

    /// The When clause of a melded Figure, unless karma is suppressed.
    pub(super) fn when_hook(&self, card: CardId, owner: Player, context: Context) -> Option<Hook> {
        if !self.position.figures() || context.suppress_karma() {
            return None;
        }
        let figure = self.position.card(card).figure?;
        let clause = figure
            .def()
            .karma
            .iter()
            .position(|karma| karma.reaction == Reaction::Melded)?;
        Some(Hook {
            source: card,
            owner,
            figure,
            clause: clause as u8,
            reaction: Reaction::Melded,
            event: KarmaEvent {
                actor: owner,
                card: Some(Captured::new(card, Evidence::PUBLIC)),
                chosen: SelectionParameter::None,
            },
        })
    }

    /// The proof names this entered reaction's operand, and the interrupted
    /// operation which supplied it. This is a causal stack link, not equality
    /// of unrelated anonymous cards which happen to share a physical identity.
    pub(super) fn name_entered_karma_operand(&mut self, source: Source, reference: HeldCard) {
        let Source::Karma { card, clause } = source else {
            return;
        };
        let Some(index) = self.frames.iter().rposition(|frame| {
            matches!(frame,
            Frame::Karma { hook, .. } if hook.source == card && hook.clause == clause)
        }) else {
            return;
        };
        let Frame::Karma { hook, .. } = &mut self.frames[index] else {
            unreachable!()
        };
        // A rebound register is a different reference even if a hidden
        // sample happens to put the same card there. Without a tracked pick
        // keep the outer reference's conservative evidence unchanged.
        if !hook.event.card.is_some_and(|prior| {
            prior.evidence.pick().is_some() && prior.evidence.pick() == reference.evidence.pick()
        }) {
            return;
        }
        hook.event.card = Some(reference);
        if let Some(Frame::Operation {
            knowledge: Some(knowledge),
            ..
        }) = index.checked_sub(1).map(|i| &mut self.frames[i])
        {
            knowledge.evidence = reference.evidence;
        }
    }

    pub(super) fn resume_karma(
        &mut self,
        hook: Hook,
        stage: u8,
        visited: Vec<HeldCard>,
        last: Option<HeldOutcome>,
    ) -> Result<(), Error> {
        let karma = hook.figure.def().karma[usize::from(hook.clause)];
        match karma.body {
            Body::Native(native) => self.resume_native_karma(native, hook, stage, visited, last),
            Body::Program(_) if stage == 0 => {
                let context =
                    self.karma_context(hook.owner, hook.source, hook.event.actor, karma.by);
                let regs = Self::karma_regs(hook.event);
                self.frames.push(Frame::Karma {
                    hook,
                    stage: 1,
                    visited,
                    last: None,
                });
                self.frames.push(Frame::Program {
                    source: Source::Karma {
                        card: hook.source,
                        clause: hook.clause,
                    },
                    pc: 0,
                    context,
                    last: regs.last,
                    retained: regs.bound,
                    chosen: regs.chosen,
                    boxed: regs.boxed,
                });
                Ok(())
            }
            Body::Program(_) => {
                self.complete(ReturnValue::Unit);
                Ok(())
            }
        }
    }

    fn resume_native_karma(
        &mut self,
        native: KarmaNative,
        hook: Hook,
        stage: u8,
        mut visited: Vec<HeldCard>,
        last: Option<HeldOutcome>,
    ) -> Result<(), Error> {
        let mut context = Context::reaction(hook.owner, hook.source);
        self.inherit_override(&mut context);
        match native {
            KarmaNative::Kepler => {
                let Some(Frame::Operation {
                    request: request @ Operation::Dogma { card, icon },
                    ..
                }) = self.frames.last()
                else {
                    unreachable!("Kepler Dogma caller");
                };
                let (request, card, icon) = (*request, *card, *icon);
                let (action, kind) = self
                    .frames
                    .iter_mut()
                    .rev()
                    .find_map(|frame| match frame {
                        Frame::ActionTail {
                            action,
                            kind,
                            stage,
                            ..
                        } if matches!(*action, Action::Dogma { card: selected, .. } if selected == card) => {
                            // This replacement owns the complete action end;
                            // leave only the original tail's turn advancement.
                            *stage = 3;
                            Some((*action, *kind))
                        }
                        _ => None,
                    })
                    .unwrap_or((
                        Action::Dogma {
                            card,
                            auspice: icon == Icon::Avatar,
                        },
                        ActionKind::Free,
                    ));
                let values = Color::ALL
                    .into_iter()
                    .filter_map(|color| self.position.top(hook.owner, color))
                    .map(|card| self.position.value(card))
                    .filter(|&value| value > 3)
                    .fold(0_u16, |values, value| values | (1 << value));
                let mut context = context;
                context.boxed_action.increase(values.count_ones() as u8)?;
                // Preserve the replaced action's kind, but let its outer tail
                // own the single regular-turn advance. The nested replacement
                // owns all completion/reward work under this karma scope.
                self.frames.push(Frame::ActionTail {
                    action,
                    context,
                    kind,
                    advance_turn: false,
                    stage: 0,
                    last: None,
                    shared: false,
                    // A replaced endorsed action is not doubled.
                    endorsed: false,
                    melded: false,
                    // Nor is a replaced promoted Dogma foreseen.
                    promoted: false,
                });
                self.frames.push(Frame::operation(request, context));
            }
            KarmaNative::Archimedes => {
                let Some(Frame::Operation {
                    request: Operation::ExecuteEffect { boxed, .. },
                    ..
                }) = self.frames.last_mut()
                else {
                    unreachable!("Archimedes effect caller");
                };
                boxed.increase(1)?;
                self.complete(ReturnValue::Unit);
            }
            KarmaNative::BearGrylls => {
                if stage == 3 {
                    self.complete(ReturnValue::Unit);
                    return Ok(());
                }
                self.frames.push(Frame::Karma {
                    hook,
                    stage: stage + 1,
                    visited,
                    last: None,
                });
                match stage {
                    0 => {
                        // Every non-achievement card outside the decks,
                        // Fission's set: Forecasts, Safes and displays too.
                        let mut batch = MoveAll::new(
                            Verb::Junk,
                            Selector::Owned(Who::Either),
                            Destination::Junk,
                            context,
                        );
                        batch.shortcut = Some(Shortcut::AnyOrder);
                        self.frames.push(Frame::MoveAll(Box::new(batch)));
                    }
                    1 => self
                        .frames
                        .push(Frame::operation(Operation::IntroduceAgeZero, context)),
                    2 => self.frames.push(Frame::operation(
                        Operation::Draw {
                            value: 0,
                            set: None,
                        },
                        context,
                    )),
                    _ => unreachable!("Bear Grylls stage"),
                }
            }
            KarmaNative::EmmaWatson => {
                let Some(Frame::Operation {
                    request:
                        Operation::Move {
                            verb: Verb::Meld,
                            target,
                            ..
                        },
                    context: meld_context,
                    ..
                }) = self.frames.last()
                else {
                    unreachable!("Emma meld caller");
                };
                let color = self.position.card(target.card).color;
                let melder = meld_context.executor;
                let controller = meld_context.controller;
                let returned = last
                    .filter(|outcome| outcome.disposition == Disposition::Performed)
                    .and_then(HeldOutcome::card_ref);
                if let Some(card) = returned {
                    visited.push(card);
                }
                if visited.len() == 6 {
                    self.complete(ReturnValue::Unit);
                } else if (stage == 0 || returned.is_some())
                    && let Some(card) = self.position.top(melder, color)
                    && !contains_card(&visited, card)
                {
                    // Only completed original returns contribute to "six".
                    // Retain these identities in rare reaction progress rather
                    // than widening every common single-operation outcome.
                    self.frames.push(Frame::Karma {
                        hook,
                        stage: 1,
                        visited,
                        last: None,
                    });
                    let source = Zone::Board(melder, color);
                    self.frames.push(Frame::operation(
                        Operation::Move {
                            verb: Verb::Return,
                            target: Target {
                                card,
                                physical: source,
                                logical: LogicalZone::Physical(source),
                            },
                            destination: Zone::Deck(self.position.card(card).origin_deck()),
                        },
                        context,
                    ));
                } else {
                    // The text explicitly makes the melder score these cards,
                    // not Emma's owner. Preserve source ownership and the
                    // interrupted execution's controller, with karma suppressed.
                    let mut scoring = Context::reaction(melder, hook.source);
                    scoring.source_owner = hook.owner;
                    scoring.controller = controller;
                    self.frames.push(Frame::MoveAll(Box::new(MoveAll::new(
                        Verb::Score,
                        Selector::TopOf(Who::Either, color),
                        Destination::Score(Who::Executor),
                        scoring,
                    ))));
                }
            }
            KarmaNative::YiSunSin => {
                if stage == 5 {
                    self.complete(ReturnValue::Unit);
                    return Ok(());
                }
                self.frames.push(Frame::Karma {
                    hook,
                    stage: stage + 1,
                    visited,
                    last: None,
                });
                // The caller is below the continuation just pushed.
                let exchange = |game: &Self| match &game.frames[game.frames.len() - 2] {
                    Frame::Exchange { plan, .. } => Some(
                        plan.movements()
                            .map(|(target, _)| Target {
                                // Exchange membership and overlap were frozen
                                // before Yi intervened. Moving an Each source
                                // cannot revoke another selected card's
                                // authorization.
                                logical: LogicalZone::Physical(target.physical),
                                ..target
                            })
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                };
                match stage {
                    // Exchanged cards are all revealed, then tucked: the tuck orders
                    // known cards, and needs no pick per hidden card.
                    0 => match exchange(self) {
                        Some(targets) => {
                            self.frames.push(Frame::RevealAll(Box::new(
                                verification::RevealPacket::new(targets, context),
                            )));
                        }
                        None => {
                            let Frame::Operation {
                                request:
                                    Operation::Move {
                                        verb: Verb::Transfer,
                                        target,
                                        ..
                                    },
                                knowledge,
                                ..
                            } = &self.frames[self.frames.len() - 2]
                            else {
                                unreachable!("Yi Sun-sin caller")
                            };
                            // The transfer's operand keeps the pick its
                            // Would event saw.
                            let (target, knowledge) = (*target, *knowledge);
                            let reference = match knowledge {
                                Some(knowledge) => Captured::new(target.card, knowledge.evidence),
                                None => self.capture(target.card),
                            };
                            self.frames.push(Frame::MoveSelected {
                                verb: Verb::Tuck,
                                targets: vec![reference.with(target)],
                                context,
                            });
                        }
                    },
                    1 => {
                        if let Some(targets) = exchange(self) {
                            let targets = targets
                                .into_iter()
                                .filter(|target| {
                                    self.position.locate(target.card) == Some(target.physical)
                                })
                                .map(|target| {
                                    Captured::new(target.card, Evidence::PUBLIC).with(target)
                                })
                                .collect();
                            self.frames.push(Frame::MoveSelected {
                                verb: Verb::Tuck,
                                targets,
                                context,
                            });
                        }
                    }
                    2 => self.frames.push(Frame::operation(
                        Operation::Draw {
                            value: 4,
                            set: None,
                        },
                        context,
                    )),
                    3 => {
                        if let Some(card) = last
                            .filter(|o| o.disposition == Disposition::Performed)
                            .and_then(|o| o.card)
                        {
                            self.frames.push(Frame::operation(
                                Operation::Move {
                                    verb: Verb::Tuck,
                                    target: Target {
                                        card,
                                        physical: Zone::Hand(hook.owner),
                                        logical: LogicalZone::Physical(Zone::Hand(hook.owner)),
                                    },
                                    destination: Zone::Board(
                                        hook.owner,
                                        self.position.card(card).color,
                                    ),
                                },
                                context,
                            ));
                        }
                    }
                    4 => {
                        let ops = self
                            .select(
                                Selector::Tops(Who::Either),
                                context,
                                Regs::EMPTY,
                                Excluded::default(),
                            )?
                            .into_iter()
                            .filter(|t| {
                                self.position
                                    .card(t.card)
                                    .printed_icon_count(Icon::Authority)
                                    > 0
                            })
                            .map(|target| Operation::Move {
                                verb: Verb::Score,
                                target,
                                destination: Zone::Score(hook.owner),
                            })
                            .collect();
                        self.choose_operations(context, ops, false);
                    }
                    _ => unreachable!("Yi Sun-sin stage"),
                }
            }
            KarmaNative::AlexTrebek if stage == 0 => {
                self.frames.push(Frame::Karma {
                    hook,
                    stage: 1,
                    visited,
                    last: None,
                });
                // Any Figures age can be searched, including an empty deck.
                // The name menu uses its original roster, not current contents.
                let decks: Vec<_> = (1..=11).map(Deck::figures).collect();
                self.frames.push(Frame::Choose {
                    player: hook.owner,
                    options: DecisionOptions::new(
                        decks.iter().copied().map(Alternative::Deck).collect(),
                        context,
                    ),
                    tasks: decks
                        .into_iter()
                        .map(|deck| Task::NameFigure { deck, context })
                        .collect(),
                });
            }
            KarmaNative::Kalashnikov if stage == 0 => {
                self.frames.push(Frame::Karma {
                    hook,
                    stage: 1,
                    visited,
                    last: None,
                });
                if let Some(card) = self.position.top(hook.owner.opponent(), Color::Red) {
                    let physical = Zone::Board(hook.owner.opponent(), Color::Red);
                    let target = Target {
                        card,
                        physical,
                        logical: LogicalZone::Physical(physical),
                    };
                    self.frames.push(Frame::Choose {
                        player: hook.owner,
                        options: DecisionOptions::new(
                            vec![
                                Alternative::Move {
                                    verb: Verb::Score,
                                    target: self.position.descriptor(target, hook.owner),
                                },
                                Alternative::SelfExecute(card),
                            ],
                            context,
                        ),
                        tasks: vec![
                            Task::Operation(
                                Operation::Move {
                                    verb: Verb::Score,
                                    target,
                                    destination: Zone::Score(hook.owner),
                                },
                                context,
                            ),
                            Task::SelfExecute {
                                card,
                                context: Context {
                                    source_owner: hook.owner.opponent(),
                                    ..context
                                },
                            },
                        ],
                    });
                }
            }
            KarmaNative::Hammurabi if stage == 0 => {
                let Some(&Frame::Operation {
                    request: request @ Operation::ExecuteEffect { card, .. },
                    ..
                }) = self.frames.last()
                else {
                    unreachable!("Hammurabi demand caller");
                };
                self.frames.push(Frame::Karma {
                    hook,
                    stage: 1,
                    visited,
                    last: None,
                });
                // The owner demands it of the demanding player, in karma.
                let actor = hook.event.actor;
                let reflected = Context {
                    executor: actor,
                    controller: actor,
                    demander: hook.owner,
                    source_owner: hook.owner,
                    source_card: Some(card),
                    source_evidence: hook
                        .event
                        .card
                        .map_or(Evidence::NONE, |reference| reference.evidence),
                    ..context
                };
                self.frames.push(Frame::operation(request, reflected));
            }
            KarmaNative::Huygens => self.resume_huygens(hook, stage, visited, last, context)?,
            KarmaNative::LilyHevesh => self.resume_lily_hevesh(hook, stage, visited, last, context),
            KarmaNative::Kalashnikov | KarmaNative::AlexTrebek | KarmaNative::Hammurabi => {
                self.complete(ReturnValue::Unit)
            }
        }
        Ok(())
    }

    /// Huygens: stage 1 chooses the batch; stages 2-4 draw and reveal one
    /// [7], two [8] or three [9] (stage - 1 cards of value stage + 5), each
    /// revealed before the next draw; stage 5 compares [Efficiency].
    fn resume_huygens(
        &mut self,
        hook: Hook,
        stage: u8,
        mut visited: Vec<HeldCard>,
        last: Option<HeldOutcome>,
        context: Context,
    ) -> Result<(), Error> {
        let owner = hook.owner;
        let karma = |stage, visited| Frame::Karma {
            hook,
            stage,
            visited,
            last: None,
        };
        match stage {
            0 => {
                self.frames.push(karma(1, visited));
                let mut junk = MoveAll::new(
                    Verb::Junk,
                    Selector::BaseDeck(Expr::Const(6)),
                    Destination::Junk,
                    context,
                );
                junk.shortcut = Some(Shortcut::DeckTop);
                self.frames.push(Frame::MoveAll(Box::new(junk)));
            }
            1 => {
                self.frames.push(karma(1, visited));
                self.frames.push(Frame::Choose {
                    player: owner,
                    options: DecisionOptions::new(
                        (7..=9)
                            .map(|value| Alternative::Draw {
                                value,
                                set: CardSet::Base,
                            })
                            .collect(),
                        context,
                    ),
                    tasks: (2..=4).map(Task::Stage).collect(),
                });
            }
            2..=4 => {
                let drawn = last
                    .filter(|outcome| outcome.disposition == Disposition::Performed)
                    .and_then(HeldOutcome::card_ref)
                    .filter(|card| !contains_card(&visited, card.inner));
                if let Some(card) = drawn {
                    // Reveal each drawn card before the next draw.
                    visited.push(Captured::new(
                        card.inner,
                        Evidence::PUBLIC.with_pick(card.evidence.pick()),
                    ));
                    self.frames.push(karma(stage, visited));
                    let card = card.inner;
                    let hand = Zone::Hand(owner);
                    self.frames.push(Frame::operation(
                        Operation::Reveal {
                            target: Target {
                                card,
                                physical: hand,
                                logical: LogicalZone::Physical(hand),
                            },
                        },
                        context,
                    ));
                } else if visited.len() < usize::from(stage - 1) {
                    self.frames.push(karma(stage, visited));
                    self.frames.push(Frame::operation(
                        Operation::Draw {
                            value: stage + 5,
                            set: None,
                        },
                        context,
                    ));
                } else {
                    self.frames.push(karma(5, visited));
                }
            }
            5 => {
                let drawn: usize = visited
                    .iter()
                    .map(|&card| {
                        self.position
                            .card(card.inner)
                            .printed_icon_count(Icon::Efficiency)
                    })
                    .sum();
                let hand = Zone::Hand(owner);
                let targets: Vec<_> = visited
                    .into_iter()
                    .filter(|card| self.position.cards(hand).contains(&card.inner))
                    .map(|card| {
                        card.with(Target {
                            card: card.inner,
                            physical: hand,
                            logical: LogicalZone::Physical(hand),
                        })
                    })
                    .collect();
                if drawn > self.position.icon_count(owner, Icon::Efficiency) {
                    self.frames.push(karma(6, Vec::new()));
                    self.frames.push(Frame::MoveSelected {
                        verb: Verb::Return,
                        targets,
                        context,
                    });
                } else {
                    self.complete(ReturnValue::Unit);
                }
            }
            _ => self.complete(ReturnValue::Unit),
        }
        Ok(())
    }

    /// Lily Hevesh: stage 1 reveals deck tops one at a time; stage 2 wins
    /// or draws one of them; stage 3 splays aslant.
    fn resume_lily_hevesh(
        &mut self,
        hook: Hook,
        stage: u8,
        mut visited: Vec<HeldCard>,
        last: Option<HeldOutcome>,
        context: Context,
    ) {
        let owner = hook.owner;
        let karma = |stage, visited| Frame::Karma {
            hook,
            stage,
            visited,
            last: None,
        };
        // Revealed cards stay on their decks; a deck is chosen at most once.
        let tops = |game: &Self, visited: &[HeldCard]| -> Vec<(Deck, CardId)> {
            game.position
                .enabled_decks()
                .filter_map(|deck| {
                    let top = *game.position.cards(Zone::Deck(deck)).last()?;
                    Some((deck, top))
                })
                .filter(|(_, top)| !contains_card(visited, *top))
                .collect()
        };
        match stage {
            0 | 1 => {
                if let Some(card) = last
                    .filter(|outcome| outcome.disposition == Disposition::Performed)
                    .and_then(HeldOutcome::card_ref)
                    .filter(|card| !contains_card(&visited, card.inner))
                {
                    visited.push(card);
                }
                let decks = tops(self, &visited);
                if visited.len() < 5 && !decks.is_empty() {
                    self.frames.push(karma(1, visited));
                    self.frames.push(Frame::Choose {
                        player: owner,
                        options: DecisionOptions::new(
                            decks
                                .iter()
                                .map(|&(deck, _)| Alternative::Deck(deck))
                                .collect(),
                            context,
                        ),
                        tasks: decks
                            .into_iter()
                            .map(|(deck, card)| {
                                let physical = Zone::Deck(deck);
                                Task::Operation(
                                    Operation::Reveal {
                                        target: Target {
                                            card,
                                            physical,
                                            logical: LogicalZone::Physical(physical),
                                        },
                                    },
                                    context,
                                )
                            })
                            .collect(),
                    });
                } else {
                    self.frames.push(karma(2, visited));
                }
            }
            2 => {
                let colors = visited.iter().fold(0_u8, |colors, &card| {
                    colors | 1 << self.position.card(card.inner).color as u8
                });
                self.frames.push(karma(3, Vec::new()));
                if colors.count_ones() == 5 {
                    self.frames.push(Frame::operation(
                        Operation::ProposeWin {
                            winner: owner,
                            reason: WinReason::Karma,
                        },
                        context,
                    ));
                    return;
                }
                let decks: Vec<_> = self
                    .position
                    .enabled_decks()
                    .filter(|&deck| {
                        self.position
                            .cards(Zone::Deck(deck))
                            .last()
                            .is_some_and(|top| contains_card(&visited, *top))
                    })
                    .collect();
                if !decks.is_empty() {
                    self.frames.push(Frame::Choose {
                        player: owner,
                        options: DecisionOptions::new(
                            decks.iter().copied().map(Alternative::Deck).collect(),
                            context,
                        ),
                        tasks: decks
                            .into_iter()
                            .map(|deck| Task::Operation(Operation::DrawTop { deck }, context))
                            .collect(),
                    });
                }
            }
            3 => {
                self.frames.push(karma(4, visited));
                let splays = Color::ALL
                    .into_iter()
                    .filter(|&color| self.can_splay(owner, color, Splay::Aslant))
                    .map(|color| Operation::Splay {
                        player: owner,
                        color,
                        direction: Splay::Aslant,
                    })
                    .collect();
                self.choose_operations(context, splays, false);
            }
            _ => self.complete(ReturnValue::Unit),
        }
    }
}
