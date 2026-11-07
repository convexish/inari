//! Conservative provenance for historical scalar queries. This proves public
//! dependencies; it never compares a computed value with the real hidden value.
use super::*;

impl Game {
    /// Observers who know a scalar. Only observation and sampling read these
    /// masks, and both need tracking from setup: an untracked game (including
    /// a search world) skips the proofs. The inline entry checks below also
    /// avoid entering the large proof bodies, while preserving local flow.
    fn scalar_mask(&self, mut known: impl FnMut(Player) -> bool) -> u8 {
        if !self.tracked() {
            return 3;
        }
        Player::ALL
            .into_iter()
            .filter(|&p| known(p))
            .fold(0, |mask, p| mask | (1 << p.index()))
    }
    #[inline]
    pub(super) fn binding_known(&self, binding: Binding, context: Context, regs: Regs) -> u8 {
        if !self.tracked() {
            return self.flow_known(regs);
        }
        self.binding_known_tracked(binding, context, regs)
    }
    fn binding_known_tracked(&self, binding: Binding, context: Context, regs: Regs) -> u8 {
        self.flow_known(regs)
            & self.scalar_mask(|viewer| match binding {
                Binding::Value(_, expr) => self.expr_known(expr, context, regs, viewer),
                Binding::Colors(_, colors) => self.colors_known(colors, context, regs, viewer),
                Binding::Card(_) => true,
            })
    }
    #[inline]
    pub(super) fn domain_known(&self, domain: Domain, context: Context, regs: Regs) -> u8 {
        if !self.tracked() {
            return self.flow_known(regs);
        }
        self.domain_known_tracked(domain, context, regs)
    }
    fn domain_known_tracked(&self, domain: Domain, context: Context, regs: Regs) -> u8 {
        self.flow_known(regs)
            & self.scalar_mask(|viewer| match domain {
                Domain::Once | Domain::Range(..) => true,
                Domain::Times(expr) => self.expr_known(expr, context, regs, viewer),
                Domain::Colors(colors) => self.colors_known(colors, context, regs, viewer),
                Domain::Values(from, _) | Domain::Cards(from) | Domain::Live { from, .. } => {
                    self.selector_known(*from, context, regs, viewer)
                }
                Domain::Bonuses(bonuses, _) => self.bonuses_known(*bonuses, context, regs, viewer),
            })
    }
    #[inline]
    pub(super) fn instruction_known(
        &self,
        instruction: Instruction,
        context: Context,
        regs: Regs,
    ) -> u8 {
        if !self.tracked() {
            return self.flow_known(regs);
        }
        self.instruction_known_tracked(instruction, context, regs)
    }
    fn instruction_known_tracked(
        &self,
        instruction: Instruction,
        context: Context,
        regs: Regs,
    ) -> u8 {
        self.flow_known(regs)
            & self.scalar_mask(|viewer| match instruction {
                Instruction::Draw(draw) => {
                    self.expr_known(draw.age, context, regs, viewer)
                        && draw
                            .count
                            .is_none_or(|count| self.expr_known(count, context, regs, viewer))
                }
                // One card moves or it does not: that outcome is public, and its
                // operand carries its own evidence. Which cards could have been
                // chosen never enters the progress or exactness.
                Instruction::Move(Move {
                    amount: Amount::One { .. },
                    ..
                }) => true,
                // A live, unbounded batch into a destination that admits
                // every card has no hidden requested quota or capacity
                // failures. Each attempted original has a public outcome
                // (Classification's transfers, Taqiyya's scores); unchosen
                // matching cards are still private references. One card per
                // color is public too when every admitted card is revealed
                // (Silk), since the revealed fronts are the colors used. A
                // Forecast or Safe can refuse a card unseen, and achieving
                // checks eligibility, so those keep the membership proof below.
                Instruction::Move(Move {
                    verb,
                    to,
                    amount:
                        Amount::Batch {
                            limit: None,
                            per_color,
                            frozen: false,
                            ..
                        },
                    verify,
                    when,
                    ..
                }) if verb != Verb::Achieve
                    && !matches!(to, Destination::Forecast(_) | Destination::Safe(_))
                    && (!per_color || (verify && when.is_none())) =>
                {
                    true
                }
                Instruction::Move(step) => {
                    self.selector_known(step.from, context, regs, viewer)
                        && match step.amount {
                            // Private membership can also change an invisible failure count
                            // (for example a refused foreshadow into a full Forecast).
                            // Card records still retain their own evidence; exactness
                            // and batch progress need this separate proof.
                            Amount::Batch {
                                limit, per_color, ..
                            } => {
                                limit
                                    .is_none_or(|expr| self.expr_known(expr, context, regs, viewer))
                                    && (!per_color
                                        || self.selector_fronts_known(
                                            step.from, context, regs, viewer,
                                        ))
                            }
                            Amount::PerValue(quota) => match quota.source {
                                ValueSource::Bonuses(bonuses) => {
                                    self.bonuses_known(bonuses, context, regs, viewer)
                                }
                                ValueSource::Cards(source) => {
                                    self.selector_known(*source, context, regs, viewer)
                                }
                            },
                            Amount::Group {
                                constraint: GroupConstraint::SameValueDifferentColors,
                                ..
                            } => self.selector_fronts_known(step.from, context, regs, viewer),
                            _ => true,
                        }
                }
                Instruction::Choose {
                    choice: Choice::Cards { from, .. },
                    ..
                } => self.selector_known(*from, context, regs, viewer),
                Instruction::Exchange(step) => {
                    self.selector_known(step.left, context, regs, viewer)
                        && self.selector_known(step.right, context, regs, viewer)
                        && step
                            .counts
                            .iter()
                            .flatten()
                            .all(|&expr| self.expr_known(expr, context, regs, viewer))
                }
                Instruction::Special(SpecialStep::JunkAvailable { count, .. }) => {
                    self.expr_known(*count, context, regs, viewer)
                }
                _ => true,
            })
    }
    #[inline]
    pub(super) fn condition_known(&self, cond: Cond, context: Context, regs: Regs) -> u8 {
        if !self.tracked() {
            return self.flow_known(regs);
        }
        self.condition_known_tracked(cond, context, regs)
    }
    fn condition_known_tracked(&self, cond: Cond, context: Context, regs: Regs) -> u8 {
        self.flow_known(regs)
            & self.scalar_mask(|viewer| self.cond_known(cond, context, regs, viewer))
    }
    #[inline]
    pub(super) fn selection_known(&self, from: Selector, context: Context, regs: Regs) -> u8 {
        if !self.tracked() {
            return self.flow_known(regs);
        }
        self.selection_known_tracked(from, context, regs)
    }
    fn selection_known_tracked(&self, from: Selector, context: Context, regs: Regs) -> u8 {
        self.flow_known(regs)
            & self.scalar_mask(|viewer| self.selector_known(from, context, regs, viewer))
    }
    pub(super) fn cond_known(
        &self,
        cond: Cond,
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        match cond {
            Cond::Compare(Expr::Count(from), Cmp::Greater | Cmp::Equal, Expr::Const(0)) => {
                self.selector_known(*from, context, regs, viewer)
                    || self.selection_has_known_witness(*from, context, regs, viewer)
            }
            // Deck sizes and set redirection are public.
            Cond::Compare(a, _, b) | Cond::SameDraw(a, b) => {
                self.expr_known(a, context, regs, viewer)
                    && self.expr_known(b, context, regs, viewer)
            }
            Cond::CommonIcon { from, .. } => {
                self.selector_known(*from, context, regs, viewer)
                    && self.selector_fronts_known(*from, context, regs, viewer)
            }
            Cond::EqualValues(a, b) => {
                self.selector_known(*a, context, regs, viewer)
                    && self.selector_known(*b, context, regs, viewer)
            }
            Cond::All(conditions) | Cond::Any(conditions) => conditions
                .iter()
                .all(|&condition| self.cond_known(condition, context, regs, viewer)),
            Cond::Not(condition) => self.cond_known(*condition, context, regs, viewer),
            Cond::Card(reg, filter) => match filter {
                Filter::Lowest | Filter::Highest => true,
                Filter::Has(test) | Filter::Lacks(test) => {
                    ((!test.reads_front()
                        && !Self::reads_location(test)
                        && !matches!(test, Test::Is(_) | Test::InSlot(_)))
                        || self.reg_named(reg, context, regs, viewer))
                        && self.test_dependencies_known(test, context, regs, viewer)
                }
            },
            Cond::Empty(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
            }
            // The commitment's outcome is disclosed at once by the scoring or
            // returning that follows it, so the branch is public even when
            // the opponent cannot tell the same-backed draws from held cards
            // (the branch survives determinization).
            Cond::FinalCount { .. } => true,
            Cond::Splayed { colors, .. } => self.colors_known(colors, context, regs, viewer),
            Cond::Did(Outcome::All) => self.last_known(regs) & (1 << viewer.index()) != 0,
            Cond::Did(Outcome::Any | Outcome::Exactly(_))
            | Cond::DrawnColor(_)
            | Cond::DrawnColorTwice => true,
            Cond::DemandMoved(_)
            | Cond::ReturnedMost
            | Cond::DancingGirlVisited
            | Cond::Action(_)
            | Cond::Foreseen
            | Cond::OwnTurn
            | Cond::Must => true,
        }
    }
    /// A public matching record proves existence without exposing every other
    /// record's front. This proves only zero/nonzero, never the exact count.
    fn selection_has_known_witness(
        &self,
        from: Selector,
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        let Selector::Filtered(base, filters) = from else {
            return false;
        };
        let membership_known = match *base {
            // Existence needs one retained member, not the exact number of
            // distinct identities after deduplication of the whole packet.
            Selector::Slot(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
            }
            Selector::Reg(reg) => self.reg_scalar_known(reg, context, regs, viewer),
            _ => self.selector_known(*base, context, regs, viewer),
        };
        if !membership_known {
            return false;
        }
        self.select(
            from,
            context,
            regs,
            Excluded {
                blind: true,
                ..Excluded::default()
            },
        )
        .is_ok_and(|targets| {
            targets.into_iter().any(|target| {
                self.selector_reference(*base, target.card, context, regs)
                    .is_some_and(|card| {
                        filters
                            .iter()
                            .all(|&filter| self.verdict_known(filter, card, context, regs, viewer))
                    })
            })
        })
    }
    fn reg_named(&self, reg: Reg, context: Context, regs: Regs, viewer: Player) -> bool {
        if let Reg::Slot(slot) = reg
            && self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) == 0
        {
            return false;
        }
        self.register_reference(reg, context, regs)
            .is_none_or(|reference| self.reference_visible(reference, viewer))
    }
    fn colors_known(&self, colors: ColorSet, context: Context, regs: Regs, viewer: Player) -> bool {
        match colors {
            ColorSet::Only(_)
            | ColorSet::Chosen
            | ColorSet::MostCards(_)
            | ColorSet::MostVisible(_) => true,
            ColorSet::Of(reg) => self.reg_named(reg, context, regs, viewer),
            ColorSet::Slot(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
            }
            ColorSet::OfCards(from) => {
                self.selector_known(*from, context, regs, viewer)
                    && self.selector_fronts_known(*from, context, regs, viewer)
            }
        }
    }
    fn bonuses_known(
        &self,
        bonuses: Bonuses,
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        match bonuses.only {
            BonusValues::Equal(expr) => self.expr_known(expr, context, regs, viewer),
            _ => true,
        }
    }
    fn expr_known(&self, expr: Expr, context: Context, regs: Regs, viewer: Player) -> bool {
        match expr {
            Expr::Const(_)
            | Expr::Boxed(_)
            | Expr::Chosen
            | Expr::Metric(..)
            | Expr::MostVisibleInAColor(_)
            | Expr::ThisTurn(_) => true,
            Expr::Done | Expr::DoneValues | Expr::DemandMoved(_) => true,
            Expr::IconsAmong(from, _)
            | Expr::IconTypesAmong(from)
            | Expr::Count(from)
            | Expr::Max(from)
            | Expr::Min(from)
            | Expr::DistinctColors(from)
            | Expr::Distinct(from)
            | Expr::Total(from) => self.selector_known(*from, context, regs, viewer),
            Expr::Value(Reg::Slot(slot)) | Expr::Slot(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
            }
            Expr::Value(_) => true, // values are printed on the back
            Expr::IconsOn(reg, _) | Expr::BonusOf(reg) => {
                self.reg_named(reg, context, regs, viewer)
            }
            Expr::Add(expr, _) | Expr::Mul(expr, _) | Expr::Div(expr, _) | Expr::NonZero(expr) => {
                self.expr_known(*expr, context, regs, viewer)
            }
            Expr::Sum(exprs) => exprs
                .iter()
                .all(|&expr| self.expr_known(expr, context, regs, viewer)),
            Expr::Visible(query) => self.colors_known(query.colors, context, regs, viewer),
            Expr::Bonuses(bonuses, _) => self.bonuses_known(*bonuses, context, regs, viewer),
        }
    }
    fn selector_fronts_known(
        &self,
        from: Selector,
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        self.select(
            from,
            context,
            regs,
            Excluded {
                blind: true,
                ..Excluded::default()
            },
        )
        .is_ok_and(|targets| {
            let named = self.named_references(from, context, regs);
            targets.into_iter().all(|target| {
                find_card(&named, target.card).map_or_else(
                    || {
                        // A hidden pile's card the viewer's tracker pins (a
                        // revealed hand) is as known as a visible one.
                        self.position.can_view(target.physical, viewer)
                            || (matches!(target.physical, Zone::Hand(_) | Zone::Score(_))
                                && self.tracker(viewer).is_some_and(|tracker| {
                                    tracker.knows(&self.position, target.card, target.physical)
                                }))
                    },
                    |reference| self.reference_visible(reference, viewer),
                )
            })
        })
    }
    fn test_dependencies_known(
        &self,
        test: Test,
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        match test {
            Test::Value(_, expr) => self.expr_known(expr, context, regs, viewer),
            Test::ValueIn(source) | Test::SharesIconWith(source) => {
                self.selector_known(*source, context, regs, viewer)
            }
            Test::ColorOf(reg)
            | Test::FeaturedAs(reg)
            | Test::SharesIcon(reg)
            | Test::Is(reg)
            | Test::EligibleBoth(reg) => self.reg_named(reg, context, regs, viewer),
            Test::ColorIn(colors) => self.colors_known(colors, context, regs, viewer),
            Test::InSlot(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
                    && self
                        .slot_cards(slot, context, regs)
                        .iter()
                        .all(|&reference| self.reference_visible(reference, viewer))
            }
            _ => true,
        }
    }
    /// The retained cards a `Has(Is)` or `Has(InSlot)` filter narrows to, when
    /// the viewer knows the register or slot itself.
    fn narrowing_refs(
        &self,
        filters: &[Filter],
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> Option<Vec<HeldCard>> {
        filters.iter().find_map(|filter| match *filter {
            Filter::Has(Test::Is(reg)) => {
                self.reg_scalar_known(reg, context, regs, viewer).then(|| {
                    self.register_reference(reg, context, regs)
                        .into_iter()
                        .collect()
                })
            }
            Filter::Has(Test::InSlot(slot)) => {
                (self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0)
                    .then(|| self.slot_cards(slot, context, regs).to_vec())
            }
            _ => None,
        })
    }
    fn reg_scalar_known(&self, reg: Reg, context: Context, regs: Regs, viewer: Player) -> bool {
        let Reg::Slot(slot) = reg else {
            return true;
        };
        self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
    }
    /// A selection an identity filter narrows to retained cards: it is known
    /// when the viewer knows how those cards relate to one another, whether
    /// each lies in the base, and every other filter's verdict on each. The
    /// base's other cards never matter, so their fronts need not be visible.
    fn narrowed_known(
        &self,
        from: Selector,
        filters: &[Filter],
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        let Some(refs) = self.narrowing_refs(filters, context, regs, viewer) else {
            return false;
        };
        refs.iter()
            .enumerate()
            .all(|(i, &a)| refs[..i].iter().all(|&b| self.relation_known(a, b, viewer)))
            && self.members_known(from, &refs, context, regs, viewer)
    }
    /// Union deduplicates by card, so its size depends on whether a retained
    /// card also lies in another part; known when each such membership is.
    fn union_overlap_known(
        &self,
        sources: &[Selector],
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        sources.iter().enumerate().all(|(i, &source)| {
            if !Self::selector_retains_reference(source) {
                return true;
            }
            let refs: Vec<_> = match self.select(
                source,
                context,
                regs,
                Excluded {
                    blind: true,
                    ..Excluded::default()
                },
            ) {
                Ok(targets) => {
                    let named = self.named_references(source, context, regs);
                    targets
                        .into_iter()
                        .filter_map(|target| find_card(&named, target.card))
                        .collect()
                }
                Err(_) => return false,
            };
            sources.iter().enumerate().all(|(j, &other)| {
                i == j || self.members_known(other, &refs, context, regs, viewer)
            })
        })
    }
    /// Whether `viewer` knows if the retained cards lie in `from`.
    fn members_known(
        &self,
        from: Selector,
        refs: &[HeldCard],
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        match from {
            Selector::Filtered(base, filters) => {
                self.members_known(*base, refs, context, regs, viewer)
                    && filters.iter().all(|&filter| {
                        refs.iter()
                            .all(|&card| self.verdict_known(filter, card, context, regs, viewer))
                    })
            }
            Selector::Union(parts) => parts
                .iter()
                .all(|&part| self.members_known(part, refs, context, regs, viewer)),
            Selector::Reg(reg) => {
                self.reg_scalar_known(reg, context, regs, viewer)
                    && self
                        .register_reference(reg, context, regs)
                        .is_none_or(|other| {
                            refs.iter()
                                .all(|&card| self.relation_known(card, other, viewer))
                        })
            }
            Selector::Slot(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
                    && self.slot_cards(slot, context, regs).iter().all(|&other| {
                        refs.iter()
                            .all(|&card| self.relation_known(card, other, viewer))
                    })
            }
            _ => self.plain_zones(from, context, regs).is_some_and(|zones| {
                refs.iter()
                    .all(|&card| self.placed_known(card, &zones, viewer))
            }),
        }
    }
    /// One filter's verdict on a retained card, as `viewer` knows it.
    fn verdict_known(
        &self,
        filter: Filter,
        card: HeldCard,
        context: Context,
        regs: Regs,
        viewer: Player,
    ) -> bool {
        let test = match filter {
            // These compare the card with the rest of the selection.
            Filter::Lowest | Filter::Highest => return false,
            Filter::Has(test) | Filter::Lacks(test) => test,
        };
        match test {
            Test::Is(reg) => {
                self.reg_scalar_known(reg, context, regs, viewer)
                    && self
                        .register_reference(reg, context, regs)
                        .is_none_or(|other| self.relation_known(card, other, viewer))
            }
            Test::InSlot(slot) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
                    && self
                        .slot_cards(slot, context, regs)
                        .iter()
                        .all(|&other| self.relation_known(card, other, viewer))
            }
            _ if Self::reads_location(test) => {
                self.reference_visible(card, viewer) && self.zone_known(card, viewer)
            }
            _ if test.reads_front() => {
                self.reference_visible(card, viewer)
                    && self.test_dependencies_known(test, context, regs, viewer)
            }
            // Values, sets and eligibility read the public back.
            _ => self.test_dependencies_known(test, context, regs, viewer),
        }
    }
    /// Whether `viewer` knows if two retained references name one card.
    fn relation_known(&self, a: HeldCard, b: HeldCard, viewer: Player) -> bool {
        // Copied references share their pick, so they name one card.
        a.evidence.pick().is_some() && a.evidence.pick() == b.evidence.pick()
            || self.reference_visible(a, viewer) && self.reference_visible(b, viewer)
            || match (a.evidence.pick(), b.evidence.pick(), &self.knowledge) {
                // Members of one selected group are distinct by construction,
                // or this observer's candidates leave no common witness.
                (Some(left), Some(right), Some(knowledge)) => {
                    knowledge.distinct[left.index()] & (1 << right.index()) != 0
                        || !knowledge.trackers[viewer.index()].picks_may_coincide(left, right)
                }
                // A card the viewer names is another card than a pick whose
                // candidates exclude it (Boerhaave Microscope's returned hand
                // card and returned top card), or is that pick's only card.
                (Some(pick), None, Some(knowledge)) if self.reference_visible(b, viewer) => {
                    let tracker = &knowledge.trackers[viewer.index()];
                    !tracker.pick_may_be(&self.position, pick, b.inner)
                        || tracker.pick_identity(pick) == Some(b.inner)
                }
                (None, Some(pick), Some(knowledge)) if self.reference_visible(a, viewer) => {
                    let tracker = &knowledge.trackers[viewer.index()];
                    !tracker.pick_may_be(&self.position, pick, a.inner)
                        || tracker.pick_identity(pick) == Some(a.inner)
                }
                _ => false,
            }
    }
    /// The physical zones a zone selector reads, including whole piles that
    /// Figures alias into it (Wernher's junk), when every card in them is a
    /// member now. Alias Figures and pile sizes are public; a partial alias
    /// (Cai Lun's board cards) fails the count and proves nothing.
    fn plain_zones(&self, from: Selector, context: Context, regs: Regs) -> Option<Vec<Zone>> {
        let players = |who| {
            Player::ALL
                .into_iter()
                .filter(move |&player| self.player_matches(who, context, player))
        };
        let mut zones: Vec<Zone> = match from {
            Selector::Hand(who) => players(who).map(Zone::Hand).collect(),
            Selector::Score(who) => players(who).map(Zone::Score).collect(),
            Selector::Achievements(who) => players(who).map(Zone::Achievements).collect(),
            Selector::AvailableAchievements | Selector::StandardAchievements => {
                vec![Zone::AvailableAchievements]
            }
            Selector::Junk => vec![Zone::Junk],
            Selector::DeckTops => self.position.enabled_decks().map(Zone::Deck).collect(),
            Selector::Forecast(who) => players(who).map(Zone::Forecast).collect(),
            Selector::Safe(who) => players(who).map(Zone::Safe).collect(),
            Selector::Museum(who) => players(who).map(Zone::Museum).collect(),
            _ => return None,
        };
        let targets = self
            .select(
                from,
                context,
                regs,
                Excluded {
                    blind: true,
                    ..Excluded::default()
                },
            )
            .ok()?;
        for target in &targets {
            if !zones.contains(&target.physical) {
                zones.push(target.physical);
            }
        }
        let physical: usize = zones
            .iter()
            .map(|&zone| self.position.cards(zone).len())
            .sum();
        (targets.len() == physical).then_some(zones)
    }
    /// The zones `viewer`'s tracker allows for a retained card: those of its
    /// pick, or of its identity when they can name it.
    fn places(&self, card: HeldCard, viewer: Player) -> Option<Vec<Zone>> {
        let tracker = self.tracker(viewer)?;
        let mut places = card
            .evidence
            .pick()
            .map(|pick| tracker.pick_zones(pick))
            .filter(|places| !places.is_empty());
        if self.reference_visible(card, viewer) {
            let named = tracker.card_zones(&self.position, card.inner);
            if places
                .as_ref()
                .is_none_or(|places| named.len() < places.len())
            {
                places = Some(named);
            }
        }
        places.filter(|places| !places.is_empty())
    }
    /// Whether `viewer` knows if a retained card lies in `zones`.
    fn placed_known(&self, card: HeldCard, zones: &[Zone], viewer: Player) -> bool {
        self.places(card, viewer).is_some_and(|places| {
            places.iter().all(|zone| zones.contains(zone))
                || places.iter().all(|zone| !zones.contains(zone))
        })
    }
    /// Whether `viewer` knows the one zone a retained card occupies.
    fn zone_known(&self, card: HeldCard, viewer: Player) -> bool {
        self.places(card, viewer)
            .is_some_and(|places| places.len() == 1)
    }
    /// Tests of where the card is now. Every candidate of a zone selector
    /// shares that public zone, but a retained reference's location is part
    /// of its identity, so it needs the same naming proof as a front.
    fn reads_location(test: Test) -> bool {
        matches!(
            test,
            Test::Inspectable(_) | Test::StackSplayed(_) | Test::StackShows(_)
        )
    }
    fn selector_retains_reference(from: Selector) -> bool {
        match from {
            Selector::Reg(_) | Selector::Slot(_) => true,
            Selector::Filtered(base, _) => Self::selector_retains_reference(*base),
            Selector::Union(parts) => parts
                .iter()
                .any(|&part| Self::selector_retains_reference(part)),
            _ => false,
        }
    }
    fn selector_known(&self, from: Selector, context: Context, regs: Regs, viewer: Player) -> bool {
        match from {
            Selector::Filtered(base, filters) => {
                if self.narrowed_known(from, filters, context, regs, viewer) {
                    return true;
                }
                if !self.selector_known(*base, context, regs, viewer) {
                    return false;
                }
                filters.iter().all(|filter| {
                    let test = match *filter {
                        Filter::Has(test) | Filter::Lacks(test) => test,
                        Filter::Lowest | Filter::Highest => return true,
                    };
                    if (test.reads_front()
                        || matches!(test, Test::Is(_) | Test::InSlot(_) | Test::EligibleBoth(_))
                        || (Self::reads_location(test) && Self::selector_retains_reference(*base)))
                        && !self.selector_fronts_known(*base, context, regs, viewer)
                    {
                        return false;
                    }
                    self.test_dependencies_known(test, context, regs, viewer)
                })
            }
            // BeneathBound locates the bound card itself.
            Selector::Board(_, colors, region) => {
                self.colors_known(colors, context, regs, viewer)
                    && (!matches!(
                        region,
                        BoardRegion::BeneathBound | BoardRegion::JustBeneathBound
                    ) || self.reg_named(Reg::Bound, context, regs, viewer))
            }
            Selector::BaseDeck(expr) | Selector::SetDeck(_, expr) | Selector::BaseDeckTop(expr) => {
                self.expr_known(expr, context, regs, viewer)
            }
            Selector::Union(sources) => {
                sources
                    .iter()
                    .all(|&source| self.selector_known(source, context, regs, viewer))
                    && (!sources
                        .iter()
                        .any(|&source| Self::selector_retains_reference(source))
                        || sources.iter().all(|&source| {
                            self.selector_fronts_known(source, context, regs, viewer)
                        })
                        || self.union_overlap_known(sources, context, regs, viewer))
            }
            // A slot deduplicates by card; known when the viewer knows whether
            // any two records name one card.
            Selector::Slot(slot) => {
                let cards = self.slot_cards(slot, context, regs);
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
                    && cards.iter().enumerate().all(|(i, &a)| {
                        cards[..i]
                            .iter()
                            .all(|&b| self.relation_known(a, b, viewer))
                    })
            }
            Selector::Reg(Reg::Slot(slot)) => {
                self.slot_scalar_known(slot, context, regs) & (1 << viewer.index()) != 0
            }
            Selector::Hand(_)
            | Selector::Score(_)
            | Selector::Achievements(_)
            | Selector::Tops(_)
            | Selector::TopOf(..)
            | Selector::AvailableAchievements
            | Selector::StandardAchievements
            | Selector::Junk
            | Selector::DeckTops
            | Selector::InPlay
            | Selector::Owned(_)
            | Selector::Reg(_)
            | Selector::Forecast(_)
            | Selector::Safe(_)
            | Selector::Museum(_) => true,
        }
    }
}
