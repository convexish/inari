use super::*;

impl Game {
    /// An authored mandatory claim-exhaustion loop may pick ages in ascending
    /// order: without changing score/board or reactions, claiming one age only
    /// changes that age's duplicate cost. Keep all back choices within the age.
    /// Duplicate backs are deferred because reordering them can allocate native
    /// random words to different hidden cohorts.
    pub(super) fn reduce_mandatory_claim_order(
        &self,
        step: Move,
        context: Context,
        targets: &mut Vec<Target>,
    ) {
        #[cfg(test)]
        if self.shortcuts_disabled {
            return;
        }
        if step.verb != Verb::Achieve
            || step.to != Destination::Achievements(Who::Executor)
            || step.from.claim_score_factor().is_none()
            || targets.len() < 2
            || targets
                .iter()
                .any(|target| target.physical != Zone::AvailableAchievements)
            || !self.position.dependencies_clear(
                &[
                    ShortcutDependency::Move(Verb::Achieve),
                    ShortcutDependency::ClaimAchievement,
                    ShortcutDependency::ClaimReaction,
                    ShortcutDependency::ScoreQuery,
                    ShortcutDependency::ScoreMembership,
                    ShortcutDependency::IconQuery,
                    ShortcutDependency::AchievementQuery,
                    ShortcutDependency::EffectValues,
                    ShortcutDependency::Win,
                ],
                context,
                false,
            )
        {
            return;
        }
        let back = |target: &Target| self.position.card(target.card).origin_deck();
        for (index, target) in targets.iter().enumerate() {
            if targets[..index]
                .iter()
                .any(|prior| back(prior) == back(target))
            {
                return;
            }
        }
        let lowest = targets.iter().map(|target| back(target).age).min().unwrap();
        targets.retain(|target| back(target).age == lowest);
    }

    /// "Splay every eligible color": each player and color once, re-queried
    /// after every splay. Splays of different piles commute, so the order is
    /// canonical (players, then printed colors) unless a live Would clause
    /// outside suppressed karma could observe it.
    pub(super) fn resume_splay_all(&mut self, batch: Box<SplayAll>) -> Result<(), Error> {
        let (step, context) = (batch.step, batch.context);
        let (mut candidates, _) = self.splay_operations(step, context, batch.regs)?;
        candidates.retain(|operation| match *operation {
            Operation::Splay { player, color, .. } => {
                let bit = 1 << (player.index() * Color::ALL.len() + color as usize);
                batch.done & bit == 0
            }
            _ => unreachable!("splay candidates"),
        });
        if candidates.is_empty() {
            self.complete(batch.outcome.returned());
            return Ok(());
        }
        // With Cities each splay may draw; suppression stops karma, not the
        // draw, so the order also needs the draws to be unable to exhaust.
        let canonical = matches!(step.choices, SplayChoices::Fixed { .. })
            && (context.suppress_karma()
                || self
                    .position
                    .dependencies_clear(&[ShortcutDependency::Splay], context, false))
            && (matches!(
                step.choices,
                SplayChoices::Fixed {
                    direction: Splay::None,
                    ..
                }
            ) || self.city_draws_cannot_exhaust(context.executor, candidates.len()));
        #[cfg(test)]
        let canonical = canonical && !self.shortcuts_disabled;
        if canonical {
            candidates.truncate(1);
        }
        self.frames.push(Frame::SplayAll(batch));
        if let [operation @ Operation::Splay { player, color, .. }] = candidates[..] {
            self.mark_splay(player, color);
            self.frames.push(Frame::operation(operation, context));
        } else {
            self.frames.push(Frame::Choose {
                player: context.controller,
                options: DecisionOptions::new(
                    candidates
                        .iter()
                        .map(|operation| match *operation {
                            Operation::Splay {
                                player,
                                color,
                                direction,
                            } => Alternative::Splay {
                                player,
                                color,
                                direction,
                            },
                            _ => unreachable!("splay candidates"),
                        })
                        .collect(),
                    context,
                ),
                tasks: candidates
                    .into_iter()
                    .map(|operation| match operation {
                        Operation::Splay {
                            player,
                            color,
                            direction,
                        } => Task::SplayEach {
                            player,
                            color,
                            direction,
                        },
                        _ => unreachable!("splay candidates"),
                    })
                    .collect(),
            });
        }
        Ok(())
    }

    /// Record one attempted pile of the splay batch beneath the operation.
    pub(super) fn mark_splay(&mut self, player: Player, color: Color) {
        let Some(Frame::SplayAll(batch)) = self.frames.last_mut() else {
            unreachable!("splay batch caller");
        };
        batch.done |= 1 << (player.index() * Color::ALL.len() + color as usize);
    }

    pub(super) fn resume_junk_available_specials(&mut self, context: Context, considered: u32) {
        let reduce =
            self.position
                .dependencies_clear(&[ShortcutDependency::JunkSpecial], context, false);
        #[cfg(test)]
        let reduce = reduce && !self.shortcuts_disabled;
        // Specials are singular public tokens, with no pile order, RNG, hidden
        // front or owned-achievement delta. No supported Figure intercepts
        // supply-token junking. Thus every unblocked order has the same outcome;
        // the canonical order still commits each semantic operation separately.
        // Re-query and recheck dependencies after every operation, never latch.
        let mut available = self.position.specials().filter(|&special| {
            considered & (1 << special.index()) == 0
                && self.position.special_location(special) == SpecialLocation::Available
        });
        let Some(first) = available.next() else {
            self.complete(ReturnValue::Unit);
            return;
        };
        if reduce {
            self.frames.push(Frame::JunkAvailableSpecials {
                context,
                considered: considered | (1 << first as u8),
            });
            self.frames.push(Frame::operation(
                Operation::JunkAvailableSpecial { special: first },
                context,
            ));
        } else {
            let mut tasks = vec![Task::JunkSpecial(first)];
            let mut options = vec![Alternative::JunkSpecial(first)];
            for special in available {
                tasks.push(Task::JunkSpecial(special));
                options.push(Alternative::JunkSpecial(special));
            }
            self.frames.push(Frame::JunkAvailableSpecials {
                context,
                considered,
            });
            self.frames.push(Frame::Choose {
                player: context.controller,
                tasks,
                options: DecisionOptions::new(options, context),
            });
        }
    }

    /// Figure clauses that could observe a reduced move: its verb, and the
    /// hand/score/available-achievement memberships that define its candidates.
    /// A destination's membership never changes which cards remain to choose.
    pub(super) fn move_dependencies(
        verb: Verb,
        from: Selector,
        to: Destination,
    ) -> Vec<ShortcutDependency> {
        fn touches(selector: Selector, hand: bool) -> bool {
            match selector {
                Selector::Hand(_) => hand,
                Selector::Score(_) => !hand,
                Selector::Filtered(source, _) => touches(*source, hand),
                Selector::Union(sources) => sources.iter().any(|&s| touches(s, hand)),
                _ => false,
            }
        }
        fn available_achievements(selector: Selector) -> bool {
            match selector {
                Selector::AvailableAchievements => true,
                Selector::Filtered(source, _) => available_achievements(*source),
                Selector::Union(sources) => sources.iter().any(|&s| available_achievements(s)),
                _ => false,
            }
        }
        fn piles_only(selector: Selector) -> bool {
            match selector {
                Selector::Hand(_) | Selector::Score(_) => true,
                Selector::Filtered(source, _) => piles_only(*source),
                Selector::Union(sources) => sources.iter().all(|&s| piles_only(s)),
                _ => false,
            }
        }
        let mut dependencies = vec![ShortcutDependency::Move(verb)];
        // An Each modifier can add board aliases to the live supply. Removing
        // that Figure changes later candidates; physical standards do not.
        if available_achievements(from) {
            dependencies.extend([
                ShortcutDependency::ClaimAchievement,
                ShortcutDependency::ClaimReaction,
            ]);
        }
        if touches(from, true) {
            dependencies.push(ShortcutDependency::HandMembership);
        }
        // A score alias (Michelangelo's hand, von Braun's junk) holds while
        // its Figure stays a top card. A sweep over hands and score piles
        // that puts nothing on a board cannot cover or move that Figure, and
        // each card is visited once, so its order cannot change the members
        // still to come. Melding onto the Figure's pile can end the alias.
        if touches(from, false) && (matches!(to, Destination::Board(_)) || !piles_only(from)) {
            dependencies.push(ShortcutDependency::ScoreMembership);
        }
        dependencies
    }

    /// Removing a card from a board can expose whatever lies beneath it.
    /// A covered Figure in a candidate's stack blocks the reduction if it
    /// could observe these moves once exposed.
    fn exposure_blocks(
        &self,
        targets: &[Target],
        dependencies: &[ShortcutDependency],
        context: Context,
        deck_targets: bool,
    ) -> bool {
        self.position.figures()
            && targets.iter().any(|target| {
                let Zone::Board(owner, _) = target.physical else {
                    return false;
                };
                let stack = self.position.cards(target.physical);
                stack[..stack.len().saturating_sub(1)].iter().any(|&card| {
                    self.position.card(card).figure.is_some_and(|figure| {
                        self.position.figure_blocks(
                            figure,
                            owner,
                            dependencies,
                            context,
                            deck_targets,
                            false,
                        )
                    })
                })
            })
    }

    /// The card's own pile, not a Figure-granted alias of another pile.
    fn unaliased(target: Target) -> bool {
        match target.logical {
            LogicalZone::Hand(player) => target.physical == Zone::Hand(player),
            LogicalZone::Score(player) => target.physical == Zone::Score(player),
            _ => true,
        }
    }

    /// Oars: the lowest-first policy, for an ordinary demand whose executor moves cards
    /// from their own hand.
    pub(super) fn reduce_lowest_first(
        &self,
        step: Move,
        context: Context,
        targets: &mut Vec<Target>,
    ) {
        #[cfg(test)]
        if self.shortcuts_disabled {
            return;
        }
        let mut dependencies = Self::move_dependencies(step.verb, step.from, step.to);
        dependencies.extend([ShortcutDependency::Draw, ShortcutDependency::Win]);
        if context.executor == context.demander
            || context.controller != context.executor
            || context.demander != self.position.turn_owner()
            || targets
                .iter()
                .any(|target| target.physical != Zone::Hand(context.executor))
            || !self
                .position
                .dependencies_clear(&dependencies, context, false)
        {
            return;
        }
        if let Some(&selected) = targets
            .iter()
            .min_by_key(|t| (self.position.value(t.card), t.card))
        {
            targets.clear();
            targets.push(selected);
        }
    }

    /// Archery/Medicine: a sharer junks one of at most two available
    /// achievements, and the owner junks from the same pool with the same
    /// effect right after. Both cards go whichever the sharer picks.
    pub(super) fn reduce_shared_pool(
        &self,
        step: Move,
        context: Context,
        targets: &mut Vec<Target>,
    ) {
        #[cfg(test)]
        if self.shortcuts_disabled {
            return;
        }
        if targets.len() != 2
            || step.verb != Verb::Junk
            || targets
                .iter()
                .any(|target| target.physical != Zone::AvailableAchievements)
            || !self.owner_runs_this_effect_next(context)
            || !self.position.dependencies_clear(
                &[
                    ShortcutDependency::Move(Verb::Junk),
                    ShortcutDependency::ClaimAchievement,
                    ShortcutDependency::ClaimReaction,
                ],
                context,
                false,
            )
        {
            return;
        }
        targets.truncate(1);
    }

    /// The executing program is a sharer's run of the Dogma's current
    /// non-demand effect, with no copied execution between them.
    fn owner_runs_this_effect_next(&self, context: Context) -> bool {
        let mut program = None;
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Program {
                    source: Source::Effect { card, effect },
                    ..
                } if program.is_none() => {
                    program = Some((*card, *effect));
                }
                Frame::Dogma(dogma) => {
                    return dogma.executing_shared
                        && dogma.participant == 1
                        && context.executor != dogma.player
                        && program.is_some()
                        && program == dogma.current();
                }
                Frame::Program { .. }
                | Frame::Execute(_)
                | Frame::Karma { .. }
                | Frame::ActionTail { .. } => return false,
                _ => {}
            }
        }
        false
    }

    /// Apply a batch move's reduction, if its preconditions hold at
    /// this choice. Called before every choice, never latched.
    pub(super) fn reduce_batch_moves(&self, batch: &mut MoveAll, targets: &mut Vec<Target>) {
        let candidates: Vec<CardId> = targets.iter().map(|target| target.card).collect();
        self.reduce_batch_candidates(batch, targets);
        batch.seen = candidates;
    }

    fn reduce_batch_candidates(&self, batch: &mut MoveAll, targets: &mut Vec<Target>) {
        let previous_ordered_since = batch.ordered_since.take();
        let Some(shortcut) = batch.shortcut else {
            return;
        };
        // A progress-dependent witness can make order a real information
        // choice even when the physical moves commute. Do not canonicalize
        // which private identity is disclosed first.
        if batch.when.is_some() {
            return;
        }
        // A Safe is an ordered hidden pile: arrival order into it is a real choice even
        // when every card fits, and its same-back secrets are distinct public slots,
        // never one random back group. Junking is the exception: secrets leave for the
        // unordered junk with the same outcome in any order; karmas that react to
        // junking (Rod Serling) are checked below.
        if matches!(batch.to, Destination::Safe(_))
            || (batch.to != Destination::Junk
                && targets
                    .iter()
                    .any(|target| matches!(target.physical, Zone::Safe(_))))
        {
            return;
        }
        #[cfg(test)]
        if self.shortcuts_disabled {
            return;
        }
        let context = batch.context;
        let outside_piles =
            shortcut == Shortcut::DeckTop || self.position.selector_avoids_piles(batch.from);
        if targets.is_empty() {
            return;
        }
        // A full Forecast rejects every remaining admission before Would
        // reactions. If every candidate must still be attempted, their order
        // has no effect; retain complete blind back groups for native sampling.
        if let Shortcut::ByOrigin { descending, .. } = shortcut
            && batch.verb == Verb::Foreshadow
            && let Destination::Forecast(who) = batch.to
            && let Ok(owner) = self.one_player(who, context)
            && self.position.cards(Zone::Forecast(owner)).len()
                >= self.position.forecast_limit(owner)
            && self.batch_required(batch, targets) == targets.len()
            && targets.iter().all(|target| {
                Self::unaliased(*target)
                    && matches!(target.physical, Zone::Hand(_) | Zone::Forecast(_))
                    && target.physical != Zone::Forecast(owner)
            })
        {
            let by_card = targets
                .iter()
                .all(|target| self.position.can_view(target.physical, context.controller));
            self.retain_origin_subset(batch, targets, descending, by_card, previous_ordered_since);
            return;
        }
        if self.position.figures() {
            let dependencies = Self::move_dependencies(batch.verb, batch.from, batch.to);
            if !self
                .position
                .dependencies_clear(&dependencies, context, outside_piles)
                || self.exposure_blocks(targets, &dependencies, context, outside_piles)
            {
                return;
            }
        }
        let required = self.batch_required(batch, targets);
        let all_must_go = required == targets.len();
        match shortcut {
            Shortcut::LowestFirst | Shortcut::SharedPool => {}
            Shortcut::DeckTop => {
                // The whole deck goes to an unordered junk or score pile:
                // physical top order, never hidden fronts or sorted identities.
                let deck = targets[0].physical;
                if ((batch.verb == Verb::Junk && batch.to == Destination::Junk)
                    || (batch.verb == Verb::Score && matches!(batch.to, Destination::Score(_))))
                    && batch.limit.is_none()
                    && !batch.stop.any_time()
                    && matches!(deck, Zone::Deck(_))
                    && targets.iter().all(|target| target.physical == deck)
                {
                    let top = *targets.last().expect("nonempty");
                    targets.clear();
                    targets.push(top);
                }
            }
            // Only an order is equivalent: which cards move stays a choice
            // unless every candidate must go (Buried Treasure's three).
            Shortcut::AnyOrder if !all_must_go => {}
            Shortcut::AnyOrder => {
                // Each move lands in its own place or an unordered pile.
                // Keep an anonymous back's complete cohort: the native sampler
                // must consume the same RNG stream as unrestricted play.
                if let Some(back) = self.batch_blind_back(batch, targets[0]) {
                    targets.retain(|target| self.batch_blind_back(batch, *target) == Some(back));
                } else {
                    targets.truncate(1);
                }
            }
            Shortcut::ByOrigin { descending, quota } => {
                if batch.verb == Verb::Foreshadow {
                    let Destination::Forecast(who) = batch.to else {
                        return;
                    };
                    let Ok(owner) = self.one_player(who, context) else {
                        return;
                    };
                    let free = self
                        .position
                        .forecast_limit(owner)
                        .saturating_sub(self.position.cards(Zone::Forecast(owner)).len());
                    // Every candidate fits, so the Forecast never rejects
                    // one: only the order is at stake (X-Ray's "any number").
                    if (!all_must_go && !quota)
                        || targets.len() > free
                        || targets.iter().any(|target| {
                            !Self::unaliased(*target)
                                || !matches!(target.physical, Zone::Hand(_) | Zone::Forecast(_))
                                || target.physical == Zone::Forecast(owner)
                        })
                    {
                        return;
                    }
                    // All members of this unordered source must fit. Back groups
                    // commute, including duplicate backs: keep each entire blind
                    // group so the normal sampler still runs on every departure.
                    let by_card = targets
                        .iter()
                        .all(|target| self.position.can_view(target.physical, context.controller));
                    self.retain_origin_subset(
                        batch,
                        targets,
                        descending,
                        by_card,
                        previous_ordered_since,
                    );
                    return;
                }
                // A finite top-card quota need not exhaust the live closure.
                // Keep its initial choice if removing a top could expose another.
                // Covered nonmatching cards are conservatively retained too.
                if batch.limit.is_some()
                    && batch.from.selects_tops()
                    && targets.iter().any(|target| {
                        matches!(target.physical, Zone::Board(_, _))
                            && self.position.cards(target.physical).len() > 1
                    })
                {
                    return;
                }
                let compatible = matches!(
                    (batch.verb, batch.to),
                    (Verb::Return, Destination::Deck)
                        | (Verb::Score, Destination::Score(_))
                        | (Verb::Transfer, Destination::Hand(_) | Destination::Score(_))
                );
                let demander_pile = matches!(
                    batch.to,
                    Destination::Hand(Who::Demander) | Destination::Score(Who::Demander)
                );
                if !compatible
                    || (batch.verb == Verb::Transfer
                        && demander_pile
                        && context.demander == context.executor)
                    || !targets.iter().all(|&target| Self::unaliased(target))
                    || targets.iter().any(|target| {
                        // Reordering two blind picks of one back could assign
                        // different RNG words to different deck positions.
                        // Hands and score piles are unordered, so there the
                        // picks land in the same place in any order.
                        batch.to == Destination::Deck
                            && !self.position.can_view(target.physical, context.controller)
                            && targets
                                .iter()
                                .filter(|other| {
                                    other.physical == target.physical
                                        && self.origin_group(**other) == self.origin_group(*target)
                                })
                                .count()
                                > 1
                    })
                    || (!all_must_go && !quota)
                {
                    return;
                }
                // Distinct origin groups commute; every same-group order and
                // every quota subset stays a real choice. Hands and score
                // piles are unordered, so when the chooser sees every
                // candidate, one canonical order remains (each subset once).
                let unordered = batch.to != Destination::Deck
                    && targets
                        .iter()
                        .all(|target| self.position.can_view(target.physical, context.controller));
                self.retain_origin_subset(
                    batch,
                    targets,
                    descending,
                    unordered,
                    previous_ordered_since,
                );
            }
            Shortcut::ByColor => {
                let ordinary = matches!(batch.verb, Verb::Meld | Verb::Tuck)
                    && batch.to == Destination::Board(Who::Executor);
                let demanded = batch.verb == Verb::Transfer
                    && batch.to == Destination::Board(Who::Demander)
                    && context.demander != context.executor
                    && targets.iter().all(|target| {
                        matches!(target.physical,
                        Zone::Board(owner, _) if owner == context.executor)
                    });
                let player = if demanded {
                    context.demander
                } else {
                    context.executor
                };
                if !(ordinary || demanded)
                    || targets.iter().any(|&target| {
                        let card = self.position.card(target.card);
                        !Self::unaliased(target)
                            || !self
                                .position
                                .can_view(target.physical, context.controller)
                            // A placed Figure would become a live top mid-phase,
                            // unless tucked beneath an existing pile.
                            || (self.position.figures()
                                && card.figure.is_some()
                                && (batch.verb != Verb::Tuck
                                    || self.position.top(player, card.color).is_none()))
                    })
                {
                    return;
                }
                let color = |id: CardId| self.position.card(id).color as u32;
                if all_must_go {
                    // Whole-phase commitment: keep one color at a time.
                    if let Some(min) = targets.iter().map(|target| color(target.card)).min() {
                        targets.retain(|target| color(target.card) == min);
                    }
                } else {
                    // A monotone colour floor removes only cross-colour
                    // orders; every subset, stop and within-colour order
                    // remains, and a quota withholds the highest colours
                    // until enough picks are made (Order of the Occult Hand).
                    self.retain_window(batch, targets, color, previous_ordered_since);
                }
            }
        }
    }

    /// A hand-built list of cards that must all return or tuck (Yi Sun-sin's
    /// tucks, Huygens's returns), every one named to the chooser: cards bound
    /// for different decks or colours commute, so keep the lowest origin
    /// group or colour. Same-group orders stay real (deck bottoms, piles).
    pub(super) fn selected_move_group(
        &self,
        verb: Verb,
        context: Context,
        targets: &[Target],
    ) -> Vec<CardId> {
        let all: Vec<CardId> = targets.iter().map(|target| target.card).collect();
        #[cfg(test)]
        if self.shortcuts_disabled {
            return all;
        }
        let key = |target: &Target| match verb {
            Verb::Return => Some(self.display_key(target.card, false, false)),
            Verb::Tuck => Some(self.position.card(target.card).color as u32),
            _ => None,
        };
        if targets
            .iter()
            .any(|&target| !Self::unaliased(target) || key(&target).is_none())
        {
            return all;
        }
        if self.position.figures() {
            let in_pile =
                |target: &Target| matches!(target.physical, Zone::Hand(_) | Zone::Score(_));
            let outside_piles = !targets.iter().any(in_pile);
            let mut dependencies = vec![ShortcutDependency::Move(verb)];
            if targets
                .iter()
                .any(|target| matches!(target.physical, Zone::Hand(_)))
            {
                dependencies.push(ShortcutDependency::HandMembership);
            }
            if verb == Verb::Tuck
                && targets
                    .iter()
                    .any(|target| matches!(target.physical, Zone::Score(_)))
            {
                dependencies.push(ShortcutDependency::ScoreMembership);
            }
            if !self
                .position
                .dependencies_clear(&dependencies, context, outside_piles)
                || self.exposure_blocks(targets, &dependencies, context, outside_piles)
            {
                return all;
            }
        }
        let Some(min) = targets.iter().filter_map(key).min() else {
            return all;
        };
        targets
            .iter()
            .filter(|target| key(target) == Some(min))
            .map(|target| target.card)
            .collect()
    }

    pub(super) fn origin_group(&self, target: Target) -> (u8, u8) {
        let deck = self.position.card(target.card).origin_deck();
        (deck.age, deck.set as u8)
    }

    /// The order a batch menu lists a card and an origin floor measures it
    /// by: age, then set, then (`by_card`, for unordered destinations) the
    /// card's colour and identity, which is how the app lays piles out.
    /// Descending batches ("highest" loops) count age down from 11.
    pub(super) fn display_key(&self, id: CardId, descending_value: bool, by_card: bool) -> u32 {
        let card = self.position.card(id);
        let deck = card.origin_deck();
        let age = if descending_value {
            11 - deck.age
        } else {
            deck.age
        };
        // Age-major, then set: the same order for any enabled sets.
        let group = (u32::from(age) * CardSet::ALL.len() as u32) + deck.set.index() as u32;
        if by_card {
            debug_assert!(id.0 < 1 << 12, "catalog identities fit twelve bits");
            (group << 16) | ((card.color as u32) << 12) | u32::from(id.0)
        } else {
            group << 16
        }
    }

    /// The caller proves distinct origin groups commute for this exact phase. A
    /// monotone floor removes permutations, and suffix capacity withholds the highest
    /// cards while fewer than the remaining requirement lie at or above them, so every
    /// required-size subset is reached by exactly one path. `by_card` refines the key
    /// to the card, for unordered destinations.
    pub(super) fn retain_origin_subset(
        &self,
        batch: &mut MoveAll,
        targets: &mut Vec<Target>,
        descending_value: bool,
        by_card: bool,
        previous_ordered_since: Option<u16>,
    ) {
        let key = |id: CardId| self.display_key(id, descending_value, by_card);
        self.retain_window(batch, targets, key, previous_ordered_since);
    }

    /// The window over any monotone key: a floor at the last card passed
    /// over, and suffix capacity measured by the same key. Ties count toward
    /// the suffix, so a colour key keeps a colour's other cards available
    /// after one of them (their order is real).
    fn retain_window(
        &self,
        batch: &mut MoveAll,
        targets: &mut Vec<Target>,
        key: impl Fn(CardId) -> u32,
        previous_ordered_since: Option<u16>,
    ) {
        let visited = batch.visited.len() as u16;
        let since = previous_ordered_since.unwrap_or(visited);
        let floor = if visited > since {
            batch.visited.last().map_or(0, |id| key(id.inner))
        } else {
            0
        };
        let mut keys: Vec<u32> = targets.iter().map(|target| key(target.card)).collect();
        keys.sort_unstable();
        let required = self.batch_required(batch, targets);
        let quotas = batch.quotas.filter(|_| !batch.stop.any_time());
        if let Some(quotas) = quotas {
            // A per-value batch must leave, at or above the card it moves,
            // enough cards of every value to fill that value's cap, so it
            // finishes each value before a higher one.
            let needs = self.value_requirements(&quotas, targets);
            let keyed: Vec<(u32, u8)> = targets
                .iter()
                .map(|target| (key(target.card), self.position.value(target.card)))
                .collect();
            let fills = |own: u32| {
                needs.iter().enumerate().all(|(value, &need)| {
                    let above = keyed
                        .iter()
                        .filter(|&&(other, v)| usize::from(v) == value && other >= own)
                        .count();
                    above >= usize::from(need)
                })
            };
            // Never empty by the monotone floor's proof; keep the menu if so.
            if !targets.iter().any(|target| fills(key(target.card))) {
                return;
            }
            targets.retain(|target| fills(key(target.card)));
        }
        // The floor binds the cards the chooser passed over. One that joined
        // since the last menu (a card drawn mid-batch, an uncovered top) was
        // never declined, so it stays on offer wherever it sorts.
        let keep = |target: &Target| {
            let own = key(target.card);
            let suffix = keys.len() - keys.partition_point(|&other| other < own);
            (own >= floor || !batch.seen.contains(&target.card))
                && (quotas.is_some() || suffix >= required)
        };
        // If every candidate fell to the window while the batch must still
        // move a card, the floor no longer proves anything: keep the full
        // menu rather than end it early.
        if required > 0 && !targets.iter().any(keep) {
            return;
        }
        targets.retain(keep);
        // A failed guard clears this marker before ordinary choices. A newly
        // proved suffix therefore never inherits an unrestricted prefix's floor.
        batch.ordered_since = Some(since);
    }

    /// How many of these candidates the batch must still move: none if it
    /// may stop, else its remaining limit, each value's remaining cap, one
    /// per colour still represented, or every candidate.
    fn batch_required(&self, batch: &MoveAll, targets: &[Target]) -> usize {
        if batch.stop.any_time() {
            return 0;
        }
        if let Some(quotas) = &batch.quotas {
            return self
                .value_requirements(quotas, targets)
                .iter()
                .map(|&need| usize::from(need))
                .sum();
        }
        let pool = if batch.per_color {
            targets
                .iter()
                .fold(0_u8, |colors, target| {
                    colors | (1 << self.position.card(target.card).color as u8)
                })
                .count_ones() as usize
        } else {
            targets.len()
        };
        batch.limit.map_or(pool, |limit| {
            usize::from(limit)
                .saturating_sub(batch.visited.len())
                .min(pool)
        })
    }

    /// Each value's cards still to move: its remaining cap, or every
    /// candidate of that value if there are fewer.
    fn value_requirements(&self, quotas: &[u16; 16], targets: &[Target]) -> [u16; 16] {
        let mut counts = [0_u16; 16];
        for target in targets {
            if let Some(count) = counts.get_mut(usize::from(self.position.value(target.card))) {
                *count += 1;
            }
        }
        std::array::from_fn(|value| quotas[value].min(counts[value]))
    }
}

#[cfg(test)]
#[path = "../../tests/common/mod.rs"]
pub(super) mod common;

#[cfg(test)]
#[path = "shortcut_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_shortcut_tests.rs"]
mod block_tests;

#[cfg(test)]
#[path = "return_quota_shortcut_tests.rs"]
mod return_quota_tests;

#[cfg(test)]
#[path = "batch_sweep_tests.rs"]
mod batch_sweep_tests;

#[cfg(test)]
#[path = "tools_shortcut_tests.rs"]
mod tools_tests;

#[cfg(test)]
#[path = "ecology_shortcut_tests.rs"]
mod ecology_tests;

#[cfg(test)]
#[path = "galileo_shortcut_tests.rs"]
mod galileo_tests;

#[cfg(test)]
#[path = "claim_figures_shortcut_tests.rs"]
mod claim_figures_tests;

#[cfg(test)]
#[path = "von_neumann_shortcut_tests.rs"]
mod von_neumann_tests;

#[cfg(test)]
#[path = "railroad_shortcut_tests.rs"]
mod railroad_tests;

#[cfg(test)]
#[path = "reformation_shortcut_tests.rs"]
mod reformation_tests;

#[cfg(test)]
#[path = "stem_cells_shortcut_tests.rs"]
mod stem_cells_tests;

#[cfg(test)]
#[path = "feudalism_shortcut_tests.rs"]
mod feudalism_tests;

#[cfg(test)]
#[path = "satellites_shortcut_tests.rs"]
mod satellites_tests;

#[cfg(test)]
#[path = "per_color_shortcut_tests.rs"]
mod per_color_tests;

#[cfg(test)]
#[path = "live_reference_shortcut_tests.rs"]
mod live_reference_tests;

#[cfg(test)]
#[path = "optional_board_batches_shortcut_tests.rs"]
mod optional_board_batches_tests;

#[cfg(test)]
#[path = "colored_icon_shortcut_tests.rs"]
mod colored_icon_tests;

#[cfg(test)]
#[path = "aggregate_shortcut_tests.rs"]
mod aggregate_tests;

#[cfg(test)]
#[path = "chosen_value_shortcut_tests.rs"]
mod chosen_value_tests;

#[cfg(test)]
#[path = "reference_subset_shortcut_tests.rs"]
mod reference_subset_tests;

#[cfg(test)]
#[path = "constrained_tests.rs"]
mod constrained_tests;

#[cfg(test)]
#[path = "board_depth_tests.rs"]
mod board_depth_tests;

#[cfg(test)]
#[path = "cities_shortcut_tests.rs"]
mod cities_tests;

#[cfg(test)]
#[path = "../echoes_tests/queries.rs"]
mod echoes_query_tests;

#[cfg(test)]
#[path = "../echoes_tests/age4.rs"]
mod echoes_age4_tests;

#[cfg(test)]
#[path = "../echoes_tests/age8.rs"]
mod echoes_age8_tests;
