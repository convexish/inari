//! Resumable interpretation of card programs ([`crate::program`]).
use std::ops::ControlFlow;

use super::*;

/// An effect invocation's registers. Batches keep a copy so their selectors
/// see the bound card and chosen value of the instruction that started them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Regs {
    pub(super) last: HeldReturn,
    pub(super) bound: Option<HeldCard>,
    pub(super) chosen: SelectionParameter,
    pub(super) boxed: BoxedValues,
    /// The program has slots ([`Frame::Locals`]) beside its frame.
    pub(super) locals: bool,
}

impl Regs {
    pub(super) const EMPTY: Self = Self {
        last: HeldReturn::UNIT,
        bound: None,
        chosen: SelectionParameter::None,
        boxed: BoxedValues::ZERO,
        locals: false,
    };

    pub(super) fn last_card(self) -> Option<HeldCard> {
        match self.last.inner {
            ReturnValue::Operation(result) if result.disposition == Disposition::Performed => {
                self.last.card()
            }
            // A batch that moved exactly one card names it.
            ReturnValue::Bulk(outcome) if outcome.performed == 1 => self.last.card(),
            _ => None,
        }
    }
}

/// Cards a batch has already considered, excluded from every selector leaf so
/// an enclosing extremum filter sees only the remaining candidates.
#[derive(Clone, Copy, Default)]
pub(super) struct Excluded<'a> {
    pub(super) cards: &'a [HeldCard],
    pub(super) colors: u8,
    /// No menu will be offered: front-reading filters may judge hidden cards.
    pub(super) blind: bool,
    /// The move goes to the junk: a card already there (a junk card counted as a score
    /// pile card by Wernher von Braun) is neither moved nor offered.
    pub(super) skip_junk: bool,
}

impl Game {
    #[inline]
    pub(super) fn register_reference(
        &self,
        reg: Reg,
        context: Context,
        regs: Regs,
    ) -> Option<HeldCard> {
        match reg {
            Reg::Last => regs.last_card(),
            Reg::Bound => regs.bound,
            Reg::Source => context
                .source_card
                .map(|card| Captured::new(card, context.source_evidence)),
            Reg::Chosen => match regs.chosen {
                SelectionParameter::Card(card) => Some(card),
                _ => None,
            },
            Reg::Slot(slot) => self.slot_register(slot, context, regs),
        }
    }

    /// Visit the references a selector names explicitly, in precedence
    /// order: a positive identity filter before its base, union parts in
    /// order. A positive identity filter narrows a remembered reference,
    /// rather than making a new selection of the same physical card, so its
    /// evidence is kept while the selector still checks current zone
    /// membership. Most selectors name nothing and return at once.
    fn visit_named(
        &self,
        from: Selector,
        context: Context,
        regs: Regs,
        visit: &mut impl FnMut(HeldCard) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        match from {
            Selector::Reg(reg) => self
                .register_reference(reg, context, regs)
                .map_or(ControlFlow::Continue(()), visit),
            Selector::Slot(slot) => self
                .slot_cards(slot, context, regs)
                .iter()
                .try_for_each(|&held| visit(held)),
            Selector::Filtered(base, filters) => {
                for filter in filters {
                    match *filter {
                        Filter::Has(Test::Is(reg)) => {
                            if let Some(held) = self.register_reference(reg, context, regs) {
                                visit(held)?;
                            }
                        }
                        Filter::Has(Test::InSlot(slot)) => {
                            self.slot_cards(slot, context, regs)
                                .iter()
                                .try_for_each(|&held| visit(held))?;
                        }
                        _ => {}
                    }
                }
                self.visit_named(*base, context, regs, visit)
            }
            Selector::Union(parts) => parts
                .iter()
                .try_for_each(|&part| self.visit_named(part, context, regs, visit)),
            _ => ControlFlow::Continue(()),
        }
    }

    /// Every reference a selector names, for matching a whole selection
    /// against with [`find_card`]: one walk of the selector, not one per
    /// candidate. Empty, and unallocated, for ordinary zone selectors.
    pub(super) fn named_references(
        &self,
        from: Selector,
        context: Context,
        regs: Regs,
    ) -> Vec<HeldCard> {
        let mut named = Vec::new();
        let _ = self.visit_named(from, context, regs, &mut |held| {
            named.push(held);
            ControlFlow::Continue(())
        });
        named
    }

    /// The reference a selector names for one card, if any.
    pub(super) fn selector_reference(
        &self,
        from: Selector,
        card: CardId,
        context: Context,
        regs: Regs,
    ) -> Option<HeldCard> {
        let mut found = None;
        let _ = self.visit_named(from, context, regs, &mut |held| {
            if held.inner == card {
                found = Some(held);
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        found
    }

    /// Only explicit retained evidence may name a hidden reference.
    pub(super) fn reference_visible(&self, held: HeldCard, viewer: Player) -> bool {
        held.evidence.known(viewer)
            || self.tracker(viewer).is_some_and(|tracker| {
                held.evidence
                    .pick()
                    .is_some_and(|pick| tracker.pick_identity(pick).is_some())
            })
    }

    fn register(&self, reg: Reg, context: Context, regs: Regs) -> Option<CardId> {
        self.register_reference(reg, context, regs)
            .map(|card| card.inner)
    }

    pub(super) fn player_matches(&self, who: Who, context: Context, player: Player) -> bool {
        match who {
            Who::Executor => player == context.executor,
            Who::Demander => player == context.demander,
            Who::Opponent => player == context.executor.opponent(),
            Who::Either => true,
        }
    }

    /// The single player a choice or destination names.
    /// A selected card's value as the instruction sees it: a slot's record
    /// keeps the value its operation showed (a returned Yamato stays 11);
    /// any other selection reads the card where it lies.
    pub(super) fn selected_value(
        &self,
        selector: Selector,
        card: CardId,
        context: Context,
        regs: Regs,
    ) -> u8 {
        let mut base = selector;
        while let Selector::Filtered(inner, _) = base {
            base = *inner;
        }
        match base {
            Selector::Slot(slot) => self.slot_shown_value(slot, card, context, regs),
            _ => None,
        }
        .unwrap_or_else(|| self.position.value(card))
    }

    /// A register card's value: the previous result's or a slot record's
    /// shown value, otherwise the card's value where it lies.
    fn register_value(&self, reg: Reg, context: Context, regs: Regs) -> Option<u8> {
        let card = self.register(reg, context, regs)?;
        let shown = match reg {
            Reg::Slot(slot) => self.slot_shown_value(slot, card, context, regs),
            Reg::Last => self.shown_value(card, Self::last_value(regs)),
            _ => None,
        };
        Some(shown.unwrap_or_else(|| self.position.value(card)))
    }

    /// A single achieve into another player's achievements is that player's
    /// own claim ("that player achieves it"): the operation, its events and
    /// its claim karma belong to the claimant, while the selection was the
    /// executing program's.
    fn claimant_context(
        &self,
        verb: Verb,
        to: Destination,
        context: Context,
    ) -> Result<Context, Error> {
        if verb == Verb::Achieve
            && let Destination::Achievements(who) = to
            && who != Who::Executor
        {
            let claimant = self.one_player(who, context)?;
            return Ok(Context {
                executor: claimant,
                controller: claimant,
                ..context
            });
        }
        Ok(context)
    }

    pub(super) fn one_player(&self, who: Who, context: Context) -> Result<Player, Error> {
        match who {
            Who::Executor => Ok(context.executor),
            Who::Demander => Ok(context.demander),
            Who::Opponent => Ok(context.executor.opponent()),
            Who::Either => Err(Error::Unsupported("a single player is required")),
        }
    }

    pub(super) fn color_mask(
        &self,
        set: ColorSet,
        context: Context,
        regs: Regs,
    ) -> Result<u8, Error> {
        Ok(match set {
            ColorSet::Only(colors) => colors.iter().fold(0, |mask, &c| mask | (1 << c as u8)),
            ColorSet::Of(reg) => self
                .register(reg, context, regs)
                .map_or(0, |id| 1 << self.position.card(id).color as u8),
            ColorSet::Chosen => regs.chosen.colors().unwrap_or(0),
            ColorSet::OfCards(selector) => self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .fold(0, |mask, target| {
                    mask | 1 << self.position.card(target.card).color as u8
                }),
            ColorSet::Slot(slot) => self.slot_colors(slot, context, regs),
            ColorSet::MostCards(who) => {
                let counts = Color::ALL.map(|color| {
                    Player::ALL
                        .into_iter()
                        .filter(|&player| self.player_matches(who, context, player))
                        .map(|player| self.position.cards(Zone::Board(player, color)).len())
                        .sum::<usize>()
                });
                let most = counts.iter().copied().max().unwrap_or(0);
                Color::ALL
                    .into_iter()
                    .zip(counts)
                    .fold(0, |mask, (color, count)| {
                        mask | (u8::from(count == most) << color as u8)
                    })
            }
            ColorSet::MostVisible(who) => {
                let player = self.one_player(who, context)?;
                let counts = Color::ALL.map(|color| self.visible(player, color, Seen::Cards));
                let most = counts.iter().copied().max().unwrap_or(0);
                Color::ALL
                    .into_iter()
                    .zip(counts)
                    .fold(0, |mask, (color, count)| {
                        mask | (u8::from(count == most) << color as u8)
                    })
            }
        })
    }

    /// Colors in iteration order: printed order for literal sets.
    pub(super) fn colors(
        &self,
        set: ColorSet,
        context: Context,
        regs: Regs,
    ) -> Result<impl Iterator<Item = Color> + use<>, Error> {
        let mask = self.color_mask(set, context, regs)?;
        let literal = match set {
            ColorSet::Only(colors) => Some(colors),
            _ => None,
        };
        let ordered: &'static [Color] = literal.unwrap_or(&Color::ALL);
        Ok(ordered
            .iter()
            .copied()
            .filter(move |&color| mask & (1 << color as u8) != 0))
    }

    pub(super) fn eval(
        &self,
        expr: Expr,
        context: Context,
        regs: Regs,
    ) -> Result<Option<i64>, Error> {
        Ok(Some(match expr {
            Expr::Const(value) => value,
            Expr::Boxed(literal) => i64::from(regs.boxed.value(literal)?),
            Expr::Count(selector) => self
                .select(*selector, context, regs, Excluded::default())?
                .len() as i64,
            Expr::IconsAmong(selector, icon) => self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .map(|target| self.position.card(target.card).printed_icon_count(icon) as i64)
                .sum(),
            Expr::IconTypesAmong(selector) => i64::from(
                self.select(*selector, context, regs, Excluded::default())?
                    .iter()
                    .fold(0, |mask, target| mask | self.icon_types(target.card))
                    .count_ones(),
            ),
            Expr::Max(selector) => self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .map(|target| i64::from(self.selected_value(*selector, target.card, context, regs)))
                .max()
                .unwrap_or(0),
            Expr::Total(selector) => self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .map(|target| i64::from(self.selected_value(*selector, target.card, context, regs)))
                .sum(),
            Expr::Min(selector) => match self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .map(|target| i64::from(self.selected_value(*selector, target.card, context, regs)))
                .min()
            {
                Some(value) => value,
                None => return Ok(None),
            },
            Expr::DistinctColors(selector) => self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .fold(0_u8, |colors, target| {
                    colors | (1 << self.position.card(target.card).color as u8)
                })
                .count_ones()
                .into(),
            Expr::Distinct(selector) => self
                .select(*selector, context, regs, Excluded::default())?
                .iter()
                .fold(0_u16, |values, target| {
                    values | (1 << self.selected_value(*selector, target.card, context, regs))
                })
                .count_ones()
                .into(),
            Expr::Value(reg) => match self.register_value(reg, context, regs) {
                Some(value) => i64::from(value),
                None => return Ok(None),
            },
            Expr::IconsOn(reg, icon) => match self.register(reg, context, regs) {
                Some(card) => self.position.card(card).printed_icon_count(icon) as i64,
                None => return Ok(None),
            },
            Expr::Chosen => match regs.chosen.value() {
                Some(value) => i64::from(value),
                None => return Ok(None),
            },
            Expr::Slot(slot) => match self.slot_value(slot, context, regs) {
                Some(value) => i64::from(value),
                None => return Ok(None),
            },
            Expr::Done => i64::from(regs.last.performed_count()),
            Expr::DemandMoved(verb) => i64::from(match verb {
                Verb::Transfer => self.prior_demand_transfers(context),
                Verb::Return => self.prior_demand_returns(context),
                _ => 0,
            }),
            Expr::DoneValues => i64::from(regs.last.distinct_value_count()),
            Expr::Metric(who, metric) => {
                let metric = match metric {
                    Metric::ChosenIcon => match regs.chosen.icon() {
                        Some(icon) => Metric::Icons(icon),
                        None => return Ok(None),
                    },
                    metric => metric,
                };
                match who {
                    Who::Either => {
                        self.metric(metric, Player::One) + self.metric(metric, Player::Two)
                    }
                    who => self.metric(metric, self.one_player(who, context)?),
                }
            }
            Expr::Add(inner, amount) => match self.eval(*inner, context, regs)? {
                Some(value) => (value + amount).max(0),
                None => return Ok(None),
            },
            Expr::Mul(inner, factor) => match self.eval(*inner, context, regs)? {
                Some(value) => value * factor,
                None => return Ok(None),
            },
            Expr::Div(inner, divisor) => match self.eval(*inner, context, regs)? {
                Some(value) => value.div_euclid(divisor),
                None => return Ok(None),
            },
            Expr::NonZero(inner) => match self.eval(*inner, context, regs)? {
                Some(value) if value != 0 => value,
                _ => return Ok(None),
            },
            Expr::Sum(terms) => {
                let mut sum = 0;
                for &term in terms {
                    let Some(value) = self.eval(term, context, regs)? else {
                        return Ok(None);
                    };
                    sum += value;
                }
                sum
            }
            Expr::ThisTurn(verb) => {
                let moves = self.position.turn_moves[context.executor.index()];
                i64::from(match verb {
                    Verb::Tuck => moves[0],
                    Verb::Score => moves[1],
                    _ => 0,
                })
            }
            Expr::Visible(query) => {
                self.visible_count(query.who, query.colors, query.seen, context, regs)?
            }
            Expr::MostVisibleInAColor(who) => {
                let player = self.one_player(who, context)?;
                Color::ALL
                    .into_iter()
                    .map(|color| self.visible(player, color, Seen::Cards) as i64)
                    .max()
                    .unwrap_or(0)
            }
            Expr::Bonuses(bonuses, tally) => {
                let counts = self.bonus_counts(*bonuses, context, regs)?;
                let shown = counts.iter().enumerate().filter(|&(_, &count)| count > 0);
                match tally {
                    Tally::Count => counts.iter().map(|&count| i64::from(count)).sum(),
                    Tally::Max => shown.map(|(value, _)| value as i64).max().unwrap_or(0),
                    Tally::Distinct => shown.count() as i64,
                }
            }
            Expr::BonusOf(reg) => match self
                .register(reg, context, regs)
                .and_then(|card| self.position.card(card).bonus())
            {
                Some(bonus) => i64::from(bonus),
                None => return Ok(None),
            },
        }))
    }

    fn visible_count(
        &self,
        who: Who,
        colors: ColorSet,
        seen: Seen,
        context: Context,
        regs: Regs,
    ) -> Result<i64, Error> {
        if seen == Seen::MostCommonIcon {
            // Icon types as Supremacy counts them: each standard icon, bonuses as one
            // type, and each City icon kind.
            let mut best = 0;
            for icon in self.position.standard_icons() {
                best =
                    best.max(self.visible_count(who, colors, Seen::Icon(icon), context, regs)?);
            }
            let colors: Vec<Color> = self.colors(colors, context, regs)?.collect();
            let count = |matches: &dyn Fn(Symbol) -> bool| -> i64 {
                Player::ALL
                    .into_iter()
                    .filter(|&player| self.player_matches(who, context, player))
                    .flat_map(|player| colors.iter().map(move |&color| (player, color)))
                    .map(|(player, color)| {
                        self.position.visible_count(player, color, matches) as i64
                    })
                    .sum()
            };
            best = best.max(count(&|s| matches!(s, Symbol::Bonus(_))));
            for kind in CityIcon::ALL {
                best = best.max(count(&|s| s == Symbol::City(kind)));
            }
            return Ok(best);
        }
        let mut total = 0;
        for player in Player::ALL {
            if self.player_matches(who, context, player) {
                for color in self.colors(colors, context, regs)? {
                    total += self.visible(player, color, seen) as i64;
                }
            }
        }
        Ok(total)
    }

    /// A [`Bonuses`] query's visible bonuses, counted per value.
    pub(super) fn bonus_counts(
        &self,
        bonuses: Bonuses,
        context: Context,
        regs: Regs,
    ) -> Result<[u16; 16], Error> {
        let mut counts = [0_u16; 16];
        for player in Player::ALL {
            if self.player_matches(bonuses.on, context, player) {
                let board = self.position.visible_bonuses(player);
                for (total, count) in counts.iter_mut().zip(board) {
                    *total += count;
                }
            }
        }
        let only = match bonuses.only {
            BonusValues::Equal(expr) => Some(self.eval(expr, context, regs)?),
            _ => None,
        };
        for (value, count) in counts.iter_mut().enumerate() {
            let keep = match bonuses.only {
                BonusValues::All => true,
                BonusValues::Even => value % 2 == 0,
                BonusValues::Odd => value % 2 == 1,
                BonusValues::Repeated => *count > 1,
                BonusValues::Equal(_) => only == Some(Some(value as i64)),
            };
            if !keep {
                *count = 0;
            }
        }
        Ok(counts)
    }

    /// Visible cards, icons or images in one pile.
    fn visible(&self, player: Player, color: Color, seen: Seen) -> usize {
        let stack = self.position.cards(Zone::Board(player, color));
        let splay = self.position.splay(player, color);
        stack
            .iter()
            .enumerate()
            .map(|(depth, &card)| {
                let top = depth + 1 == stack.len();
                if !top && splay == Splay::None {
                    return 0;
                }
                let mut count = 0;
                let card = self.position.card(card);
                match seen {
                    Seen::Cards => return 1,
                    Seen::Icon(icon) => card.visit_shown(top, splay, |s| {
                        count += usize::from(s.standard() == Some(icon));
                    }),
                    Seen::Images => card.visit_shown(top, splay, |s| {
                        count += usize::from(s == Symbol::Image);
                    }),
                    Seen::Echoes => card.visit_shown(top, splay, |s| {
                        count += usize::from(s == Symbol::Echo);
                    }),
                    Seen::MostCommonIcon => unreachable!("resolved per icon type"),
                }
                count
            })
            .sum()
    }

    pub(super) fn metric(&self, metric: Metric, player: Player) -> i64 {
        match metric {
            Metric::Score => self.position.score(player),
            Metric::Achievements => self.position.effective_achievement_count(player) as i64,
            Metric::Specials => self
                .position
                .specials()
                .filter(|&special| {
                    self.position.special_location(special) == SpecialLocation::Owned(player)
                })
                .count() as i64,
            Metric::Icons(icon) => self.position.icon_count(player, icon) as i64,
            Metric::IconTypesAtLeast(count) => {
                let count = usize::from(count);
                let symbols = self.position.board_symbols(player);
                let standard = self
                    .position
                    .standard_icons()
                    .filter(|&icon| self.position.icon_count(player, icon) >= count)
                    .count();
                let city = CityIcon::ALL
                    .into_iter()
                    .filter(|&icon| symbols.city(icon) >= count)
                    .count();
                let bonuses = usize::from(symbols.bonus_count() >= count);
                (standard + city + bonuses) as i64
            }
            Metric::BonusPoints => self.position.bonus_score(player),
            Metric::Bonuses => self.position.board_symbols(player).bonus_count() as i64,
            Metric::CityIcons(icon) => self.position.board_symbols(player).city(icon) as i64,
            Metric::ChosenIcon => unreachable!("resolved against the chosen register"),
        }
    }

    fn eval_u8(&self, expr: Expr, context: Context, regs: Regs) -> Result<Option<u8>, Error> {
        self.eval(expr, context, regs)?
            .map(|value| {
                u8::try_from(value).map_err(|_| Error::Unsupported("computed draw value overflow"))
            })
            .transpose()
    }

    pub(super) fn eval_count(
        &self,
        expr: Expr,
        context: Context,
        regs: Regs,
    ) -> Result<u16, Error> {
        // A negative quantity ("all but two" of one card) is nothing to do.
        self.eval(expr, context, regs)?.map_or(Ok(0), |value| {
            u16::try_from(value.max(0)).map_err(|_| Error::Unsupported("quantity domain"))
        })
    }

    fn passes(
        &self,
        test: Test,
        card: CardId,
        zone: Option<Zone>,
        context: Context,
        regs: Regs,
    ) -> Result<bool, Error> {
        let printed = self.position.card(card);
        let stack = |zone: Option<Zone>| match zone {
            Some(Zone::Board(player, color)) => Some((player, color)),
            _ => None,
        };
        Ok(match test {
            Test::Icon(icon) => printed.printed_icon_count(icon) > 0,
            Test::Set(set) => printed.set == set,
            Test::Color(color) => printed.color == color,
            Test::Code(code) => printed.code == code,
            Test::Figure => printed.figure.is_some(),
            Test::TitleBefore(reg) => match self.register(reg, context, regs) {
                Some(other) => {
                    crate::catalog::title_order(printed.code, self.position.card(other).code)
                        == std::cmp::Ordering::Less
                }
                None => false,
            },
            Test::MoreVisibleCardsThan(who) => match self.position.locate(card) {
                Some(Zone::Board(owner, color)) => {
                    let other = self.one_player(who, context)?;
                    self.position.visible_cards_in(owner, color)
                        > self.position.visible_cards_in(other, color)
                }
                _ => false,
            },
            Test::FewerVisibleCardsThanColorOf(reg) => {
                match (
                    self.position.locate(card),
                    self.register(reg, context, regs),
                ) {
                    (Some(Zone::Board(owner, color)), Some(other)) => {
                        self.position.visible_cards_in(owner, color)
                            < self
                                .position
                                .visible_cards_in(owner, self.position.card(other).color)
                    }
                    _ => false,
                }
            }
            Test::CardsOfColorOnBoard(who, cmp, expr) => match self.eval(expr, context, regs)? {
                Some(wanted) => {
                    let owner = self.one_player(who, context)?;
                    let count = self.position.cards(Zone::Board(owner, printed.color)).len();
                    cmp.holds(count as i64, wanted)
                }
                None => false,
            },
            Test::ValueRepeated(selector) => {
                let value = self.position.value(card);
                self.select(*selector, context, regs, Excluded::default())?
                    .iter()
                    .filter(|target| self.position.value(target.card) == value)
                    .count()
                    >= 2
            }
            Test::ValueCount(selector, cmp, count) => {
                let value = self.position.value(card);
                let occurrences = self
                    .select(*selector, context, regs, Excluded::default())?
                    .iter()
                    .filter(|target| self.position.value(target.card) == value)
                    .count();
                cmp.holds(occurrences as i64, i64::from(count))
            }
            Test::NoPrintedEffects => {
                printed.effects.is_empty() && printed.echo.is_none() && printed.figure.is_none()
            }
            Test::BoardHasTopSet(set) => match self.position.locate(card) {
                Some(Zone::Board(owner, _)) => Color::ALL
                    .into_iter()
                    .filter_map(|color| self.position.top(owner, color))
                    .any(|top| self.position.card(top).set == set),
                _ => false,
            },
            Test::Demand => printed.effects.iter().any(|effect| effect.demand()),
            Test::Value(cmp, expr) => self
                .eval(expr, context, regs)?
                .is_some_and(|value| cmp.holds(i64::from(self.position.value(card)), value)),
            Test::Values(values) => values.contains(&self.position.value(card)),
            Test::ValueIn(source) => self
                .select(*source, context, regs, Excluded::default())?
                .iter()
                .any(|target| {
                    self.selected_value(*source, target.card, context, regs)
                        == self.position.value(card)
                }),
            Test::BoxedValues(values) => {
                let mut any = false;
                for &literal in values {
                    any |= regs.boxed.value(literal)? == self.position.value(card);
                }
                any
            }
            Test::SharesIcon(reg) => self
                .register(reg, context, regs)
                .is_some_and(|other| self.icon_types(card) & self.icon_types(other) != 0),
            Test::SharesIconWith(selector) => {
                let others = self
                    .select(*selector, context, regs, Excluded::default())?
                    .iter()
                    .fold(0, |mask, target| mask | self.icon_types(target.card));
                self.icon_types(card) & others != 0
            }
            Test::ColorOf(reg) => self
                .register(reg, context, regs)
                .is_some_and(|other| self.position.card(other).color == printed.color),
            Test::ColorIn(set) => {
                self.color_mask(set, context, regs)? & (1 << printed.color as u8) != 0
            }
            Test::FeaturedChosenIcon => regs
                .chosen
                .icon()
                .is_some_and(|icon| printed.featured == Some(icon)),
            Test::ChosenIcon => regs
                .chosen
                .icon()
                .is_some_and(|icon| printed.printed_icon_count(icon) > 0),
            Test::Shows(shown) => {
                stack(self.position.locate(card)).is_some_and(|(player, color)| {
                    let top = self.position.top(player, color) == Some(card);
                    let mut shows = false;
                    printed.visit_shown(top, self.position.splay(player, color), |symbol| {
                        shows |= match shown {
                            Shown::Icon(icon) => symbol.standard() == Some(icon),
                            Shown::Image => symbol == Symbol::Image,
                            Shown::Echo => symbol == Symbol::Echo,
                        };
                    });
                    shows
                })
            }
            Test::ShowsChosenIcon => stack(zone).is_some_and(|(player, color)| {
                let top = self.position.top(player, color) == Some(card);
                let mut shows = false;
                if let Some(icon) = regs.chosen.icon() {
                    printed.visit_shown(top, self.position.splay(player, color), |symbol| {
                        shows |= symbol.standard() == Some(icon);
                    });
                }
                shows
            }),
            Test::ColorOnBoard(who) => self
                .position
                .top(self.one_player(who, context)?, printed.color)
                .is_some(),
            Test::AboveTop(who) => {
                self.position.value(card)
                    > self
                        .position
                        .top(self.one_player(who, context)?, printed.color)
                        .map_or(0, |top| self.position.value(top))
            }
            Test::Is(reg) => self.register(reg, context, regs) == Some(card),
            Test::InSlot(slot) => contains_card(self.slot_cards(slot, context, regs), card),
            Test::Inspectable(who) => zone.is_some_and(|zone| {
                Player::ALL.into_iter().any(|player| {
                    self.player_matches(who, context, player)
                        && self.position.can_view(zone, player)
                })
            }),
            Test::StackSplayed(direction) => stack(zone).is_some_and(|(player, color)| {
                let splay = self.position.splay(player, color);
                direction.map_or(splay != Splay::None, |wanted| splay == wanted)
            }),
            Test::StackShows(icon) => stack(zone)
                .is_some_and(|(player, color)| self.position.color_has_icon(player, color, icon)),
            Test::Eligible { score_factor } => {
                let player = context.executor;
                self.position.meets_achievement_cost(
                    player,
                    card,
                    self.position.score(player) * i64::from(score_factor),
                    self.position.highest_claim_value(player),
                )
            }
            Test::FeaturedAs(reg) => self.register(reg, context, regs).is_some_and(|other| {
                printed.featured.is_some() && self.position.card(other).featured == printed.featured
            }),
            Test::EligibleBoth(reg) => {
                let player = context.executor;
                let score = self.position.score(player) - i64::from(self.position.value(card));
                let claim = self.position.highest_claim_value(player);
                let affords = |card| {
                    self.position
                        .meets_achievement_cost(player, card, score, claim)
                };
                affords(card) && self.register(reg, context, regs).is_some_and(affords)
            }
            Test::Bonus => printed.bonus().is_some(),
        })
    }

    /// Standard symbols (including the icon within Search), one Bonus type,
    /// and each City type. Images and echo text are not icon types.
    fn icon_types(&self, card: CardId) -> u32 {
        self.position
            .card(card)
            .symbols
            .iter()
            .fold(0, |mask, symbol| {
                let bit = match *symbol {
                    Symbol::Icon(icon) | Symbol::Search(icon) => Some(icon as u8),
                    Symbol::Bonus(_) => Some(8),
                    Symbol::City(kind) => Some(9 + kind as u8),
                    _ => None,
                };
                mask | bit.map_or(0, |bit| 1 << bit)
            })
    }

    pub(super) fn holds(&self, cond: Cond, context: Context, regs: Regs) -> Result<bool, Error> {
        Ok(match cond {
            Cond::Compare(left, cmp, right) => {
                match (
                    self.eval(left, context, regs)?,
                    self.eval(right, context, regs)?,
                ) {
                    (Some(left), Some(right)) => cmp.holds(left, right),
                    _ => false,
                }
            }
            Cond::CommonIcon { from, except } => {
                // Standard types use bits 0..8, Bonus bit 8, Cities 9..18.
                // Search contributes its standard type, never a new type.
                let mut common = !(1_u32 << except as u8);
                for target in self.select(*from, context, regs, Excluded::default())? {
                    common &= self.icon_types(target.card);
                }
                common != 0
            }
            Cond::EqualValues(left, right) => {
                let values = |selector| -> Result<Vec<u8>, Error> {
                    let mut values: Vec<_> = self
                        .select(selector, context, regs, Excluded::default())?
                        .iter()
                        .map(|target| self.selected_value(selector, target.card, context, regs))
                        .collect();
                    values.sort_unstable();
                    Ok(values)
                };
                values(*left)? == values(*right)?
            }
            Cond::All(conds) => {
                for &cond in conds {
                    if !self.holds(cond, context, regs)? {
                        return Ok(false);
                    }
                }
                true
            }
            Cond::Any(conds) => {
                for &cond in conds {
                    if self.holds(cond, context, regs)? {
                        return Ok(true);
                    }
                }
                false
            }
            Cond::Not(cond) => !self.holds(*cond, context, regs)?,
            Cond::Card(reg, filter) => match self.register(reg, context, regs) {
                Some(card) => {
                    // Only stack tests need to know where the card is now.
                    let zone = match filter {
                        Filter::Has(
                            Test::StackSplayed(_) | Test::StackShows(_) | Test::ShowsChosenIcon,
                        )
                        | Filter::Lacks(
                            Test::StackSplayed(_) | Test::StackShows(_) | Test::ShowsChosenIcon,
                        ) => self.position.locate(card),
                        _ => None,
                    };
                    match filter {
                        Filter::Has(test) => self.passes(test, card, zone, context, regs)?,
                        Filter::Lacks(test) => !self.passes(test, card, zone, context, regs)?,
                        Filter::Lowest | Filter::Highest => true,
                    }
                }
                None => false,
            },
            Cond::Empty(slot) => self.slot_empty(slot, context, regs),
            Cond::SameDraw(a, b) => {
                match (
                    self.eval_u8(a, context, regs)?,
                    self.eval_u8(b, context, regs)?,
                ) {
                    (Some(a), Some(b)) => {
                        self.draw_deck(a, None, context.executor)?
                            == self.draw_deck(b, None, context.executor)?
                    }
                    _ => false,
                }
            }
            Cond::FinalCount { hand, drawn } => {
                let count = |selector: Selector| -> Result<usize, Error> {
                    Ok(self
                        .select(selector, context, regs, Excluded::default())?
                        .len())
                };
                match regs.chosen {
                    SelectionParameter::Count(target) => count(*hand)? == usize::from(target),
                    SelectionParameter::Value(target) => count(*drawn)? == usize::from(target),
                    _ => false,
                }
            }
            Cond::Splayed {
                who,
                colors,
                direction,
                present,
            } => {
                let player = self.one_player(who, context)?;
                self.colors(colors, context, regs)?
                    .any(|color| (self.position.splay(player, color) == direction) == present)
            }
            Cond::Did(outcome) => match outcome {
                Outcome::All => regs.last.fulfilled(),
                Outcome::Any => regs.last.performed_count() > 0,
                Outcome::Exactly(count) => count > 0 && regs.last.performed_count() == count,
            },
            Cond::DemandMoved(verb) => match verb {
                Verb::Transfer => self.prior_demand_transfers(context) > 0,
                Verb::Return => self.prior_demand_returns(context) > 0,
                _ => false,
            },
            Cond::DrawnColor(color) => {
                matches!(regs.last.inner, ReturnValue::DrawColors { colors, .. }
                if colors & (1 << color as u8) != 0)
            }
            Cond::DrawnColorTwice => {
                matches!(
                    regs.last.inner,
                    ReturnValue::DrawColors { repeated: true, .. }
                )
            }
            Cond::ReturnedMost => self.democracy_leader(context.executor),
            Cond::DancingGirlVisited => self.dancing_girl_visited(context.executor.opponent()),
            Cond::Action(ordinal) => self.own_regular_action(context.executor, ordinal).is_some(),
            Cond::Foreseen => self.foreseen(context),
            Cond::OwnTurn => self.position.turn_owner == context.executor,
            Cond::Must => context.must(),
        })
    }

    fn logical(&self, zone: LogicalZone) -> impl Iterator<Item = Target> + '_ {
        self.position.targets(zone)
    }

    fn tops(&self, who: Who, color: Option<Color>, context: Context, out: &mut Vec<Target>) {
        for player in Player::ALL {
            if !self.player_matches(who, context, player) {
                continue;
            }
            for c in Color::ALL {
                if color.is_some_and(|wanted| wanted != c) {
                    continue;
                }
                if let Some(card) = self.position.top(player, c) {
                    let physical = Zone::Board(player, c);
                    out.push(Target {
                        card,
                        physical,
                        logical: LogicalZone::Physical(physical),
                    });
                }
            }
        }
    }

    pub(super) fn select_board(
        &self,
        who: Who,
        colors: ColorSet,
        region: BoardRegion,
        context: Context,
        regs: Regs,
    ) -> Result<Vec<Target>, Error> {
        let mask = self.color_mask(colors, context, regs)?;
        let mut targets = Vec::new();
        for player in Player::ALL {
            if !self.player_matches(who, context, player) {
                continue;
            }
            for color in Color::ALL {
                if mask & (1 << color as u8) == 0 {
                    continue;
                }
                let physical = Zone::Board(player, color);
                let stack = self.position.cards(physical);
                let (start, end) = match region {
                    BoardRegion::Top => (stack.len().saturating_sub(1), stack.len()),
                    BoardRegion::Bottom => (0, usize::from(!stack.is_empty())),
                    BoardRegion::BottomN(n) => (0, stack.len().min(usize::from(n))),
                    BoardRegion::BelowTop(keep) => {
                        (0, stack.len().saturating_sub(usize::from(keep)))
                    }
                    BoardRegion::AboveBottom => (usize::from(!stack.is_empty()), stack.len()),
                    BoardRegion::BeneathBound => {
                        let Some(index) = regs
                            .bound
                            .and_then(|id| stack.iter().position(|&card| card == id.inner))
                        else {
                            continue;
                        };
                        (0, index)
                    }
                    BoardRegion::JustBeneathBound => {
                        let Some(index) = regs
                            .bound
                            .and_then(|id| stack.iter().position(|&card| card == id.inner))
                        else {
                            continue;
                        };
                        (index.saturating_sub(1), index)
                    }
                };
                targets.extend(stack[start..end].iter().map(|&card| Target {
                    card,
                    physical,
                    logical: LogicalZone::BoardDepth(player, color),
                }));
            }
        }
        Ok(targets)
    }

    /// Candidates in stable order, with considered cards removed at each leaf.
    pub(super) fn select(
        &self,
        selector: Selector,
        context: Context,
        regs: Regs,
        excluded: Excluded,
    ) -> Result<Vec<Target>, Error> {
        let mut targets = Vec::new();
        match selector {
            Selector::Hand(who) | Selector::Score(who) => {
                let hand = matches!(selector, Selector::Hand(_));
                for player in Player::ALL {
                    if !self.player_matches(who, context, player) {
                        continue;
                    }
                    let (zone, physical) = if hand {
                        (LogicalZone::Hand(player), Zone::Hand(player))
                    } else {
                        (LogicalZone::Score(player), Zone::Score(player))
                    };
                    targets.reserve(self.position.cards(physical).len());
                    for target in self.logical(zone) {
                        // Aliased memberships can list a card in both piles.
                        if !targets
                            .iter()
                            .any(|prior: &Target| prior.card == target.card)
                        {
                            targets.push(target);
                        }
                    }
                }
            }
            Selector::Achievements(who) => {
                for player in Player::ALL {
                    if self.player_matches(who, context, player) {
                        let physical = Zone::Achievements(player);
                        targets.extend(self.position.cards(physical).iter().map(|&card| Target {
                            card,
                            physical,
                            logical: LogicalZone::Physical(physical),
                        }));
                    }
                }
            }
            Selector::Tops(who) => {
                targets.reserve(Color::ALL.len());
                self.tops(who, None, context, &mut targets);
            }
            Selector::TopOf(who, color) => self.tops(who, Some(color), context, &mut targets),
            Selector::Board(who, colors, region) => {
                targets = self.select_board(who, colors, region, context, regs)?;
            }
            Selector::AvailableAchievements => {
                targets.extend(self.logical(LogicalZone::AvailableAchievements(context.executor)));
            }
            Selector::StandardAchievements => {
                targets.extend(self.logical(LogicalZone::Physical(Zone::AvailableAchievements)));
            }
            Selector::Junk => {
                targets.extend(self.logical(LogicalZone::Physical(Zone::Junk)));
            }
            Selector::DeckTops => {
                for deck in self.position.enabled_decks() {
                    if let Some(&card) = self.position.cards(Zone::Deck(deck)).last() {
                        let physical = Zone::Deck(deck);
                        targets.push(Target {
                            card,
                            physical,
                            logical: LogicalZone::Physical(physical),
                        });
                    }
                }
            }
            Selector::BaseDeck(age) | Selector::SetDeck(_, age) | Selector::BaseDeckTop(age) => {
                let set = match selector {
                    Selector::SetDeck(set, _) => set,
                    _ => CardSet::Base,
                };
                if let Some(age) = self.eval(age, context, regs)?
                    && (0..=11).contains(&age)
                    && self.position.expansions().includes(set)
                {
                    targets.extend(self.logical(LogicalZone::Physical(Zone::Deck(Deck {
                        set,
                        age: age as u8,
                    }))));
                    if matches!(selector, Selector::BaseDeckTop(_)) {
                        let top = targets.pop();
                        targets.clear();
                        targets.extend(top);
                    }
                }
            }
            Selector::InPlay => {
                // Physical membership, not logical aliases: each physical card
                // appears once, and buried cards become selectable as exposed.
                for player in Player::ALL {
                    for physical in [Zone::Hand(player), Zone::Score(player)] {
                        targets.extend(self.position.cards(physical).iter().map(|&card| Target {
                            card,
                            physical,
                            logical: LogicalZone::Physical(physical),
                        }));
                    }
                }
                self.tops(Who::Either, None, context, &mut targets);
            }
            Selector::Owned(who) => {
                for player in Player::ALL {
                    if self.player_matches(who, context, player) {
                        targets.extend(self.owned_targets(player, false));
                    }
                }
            }
            Selector::Reg(reg) => {
                if let Some(card) = self.register(reg, context, regs) {
                    targets.extend(self.named_target(card));
                }
            }
            Selector::Slot(slot) => {
                for card in self.slot_cards(slot, context, regs) {
                    if !targets.iter().any(|prior| prior.card == card.inner) {
                        targets.extend(self.named_target(card.inner));
                    }
                }
            }
            Selector::Forecast(who) | Selector::Safe(who) | Selector::Museum(who) => {
                for player in Player::ALL {
                    if self.player_matches(who, context, player) {
                        let physical = match selector {
                            Selector::Forecast(_) => Zone::Forecast(player),
                            Selector::Safe(_) => Zone::Safe(player),
                            _ => Zone::Museum(player),
                        };
                        targets.extend(self.position.cards(physical).iter().map(|&card| Target {
                            card,
                            physical,
                            logical: LogicalZone::Physical(physical),
                        }));
                    }
                }
            }
            Selector::Union(sources) => {
                for &source in sources {
                    for target in self.select(source, context, regs, excluded)? {
                        if !targets
                            .iter()
                            .any(|prior: &Target| prior.card == target.card)
                        {
                            targets.push(target);
                        }
                    }
                }
            }
            Selector::Filtered(source, filters) => {
                targets = self.select(*source, context, regs, excluded)?;
                let references = self.named_references(selector, context, regs);
                for &filter in filters {
                    let named = targets.iter().all(|target| {
                        self.position.can_view(target.physical, context.controller)
                            || find_card(&references, target.card).is_some_and(|held| {
                                self.reference_visible(held, context.controller)
                            })
                    });
                    self.apply(filter, &mut targets, context, regs, excluded.blind || named)?;
                }
            }
        }
        // Remove considered cards at the leaves, before an enclosing extremum.
        // A replaced or inapplicable highest card must not hide the next one.
        if excluded.skip_junk {
            targets.retain(|target| target.physical != Zone::Junk);
        }
        if !excluded.cards.is_empty() || excluded.colors != 0 {
            targets.retain(|target| {
                !contains_card(excluded.cards, target.card)
                    && excluded.colors & (1 << self.position.card(target.card).color as u8) == 0
            });
        }
        Ok(targets)
    }

    /// A card a register or slot names, wherever it is: naming it is explicit
    /// permission to take it from anywhere in its pile.
    fn named_target(&self, card: CardId) -> Option<Target> {
        let physical = self.position.locate(card)?;
        let logical = match physical {
            Zone::Board(player, color) => LogicalZone::BoardDepth(player, color),
            zone => LogicalZone::Physical(zone),
        };
        Some(Target {
            card,
            physical,
            logical,
        })
    }

    fn apply(
        &self,
        filter: Filter,
        targets: &mut Vec<Target>,
        context: Context,
        regs: Regs,
        blind: bool,
    ) -> Result<(), Error> {
        let value = |target: &Target| self.position.value(target.card);
        match filter {
            Filter::Lowest | Filter::Highest => {
                let extreme = if filter == Filter::Lowest {
                    targets.iter().map(value).min()
                } else {
                    targets.iter().map(value).max()
                };
                targets.retain(|target| Some(value(target)) == extreme);
            }
            Filter::Has(test) | Filter::Lacks(test) => {
                if test.reads_front()
                    && !blind
                    && targets
                        .iter()
                        .any(|target| !self.position.can_view(target.physical, context.controller))
                {
                    return Err(Error::Unsupported("private filtered card selection"));
                }
                let want = matches!(filter, Filter::Has(_));
                let mut failure = None;
                targets.retain(|target| {
                    match self.passes(test, target.card, Some(target.physical), context, regs) {
                        Ok(passed) => passed == want,
                        Err(error) => {
                            failure.get_or_insert(error);
                            false
                        }
                    }
                });
                if let Some(error) = failure {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    /// The instruction at `pc` of a program frame's source, if any.
    #[inline(always)]
    pub(super) fn instruction(&self, source: Source, pc: u16) -> Option<Instruction> {
        self.program(source)?.get(usize::from(pc)).copied()
    }

    /// A source's program text; none for a native karma body.
    #[inline(always)]
    pub(super) fn program(&self, source: Source) -> Option<&[Instruction]> {
        match source {
            Source::Effect { card, effect } => {
                Some(&self.position.card(card).effect(effect)?.instructions)
            }
            Source::Karma { card, clause } => {
                let figure = self.position.card(card).figure?;
                match figure.def().karma[usize::from(clause)].body {
                    Body::Program(program) => Some(program),
                    Body::Native(_) => None,
                }
            }
            Source::CityIcon { icon, .. } => Some(cities::icon_program(icon)),
        }
    }

    /// Resume one step of an effect program. A program with slots or blocks
    /// takes [`Self::resume_locals`]; the others stay on this short path.
    pub(super) fn resume_program(
        &mut self,
        source: Source,
        pc: u16,
        context: Context,
        regs: Regs,
    ) -> Result<(), Error> {
        if let Some(Frame::Locals(_)) = self.frames.last() {
            return self.resume_locals(source, pc, context, regs);
        }
        let Some(instruction) = self.instruction(source, pc) else {
            self.complete(ReturnValue::Unit);
            return Ok(());
        };
        self.run(instruction, source, pc, context, regs)
    }

    /// Run one instruction of a program whose frame is popped.
    // This large body is shared by ordinary programs and programs with
    // locals. Inlining it duplicates the interpreter in both callers.
    #[inline(never)]
    fn run(
        &mut self,
        instruction: Instruction,
        source: Source,
        pc: u16,
        context: Context,
        mut regs: Regs,
    ) -> Result<(), Error> {
        let locals = regs.locals;
        // Verification is a rules event: only explicit retained evidence or
        // current public visibility may suppress it. Tracker inference must
        // not change the event stream in tracked versus untracked play.
        if matches!(source, Source::Karma { .. })
            && instruction.reads_bound_front()
            && let Some(reference) = regs.bound
            && let Some(zone) = self.position.locate(reference.inner)
            && !Player::ALL.into_iter().all(|player| {
                reference.evidence.known(player) || self.position.can_view(zone, player)
            })
        {
            self.frames.push(Frame::KarmaProof {
                saved: Box::new(Frame::Program {
                    source,
                    pc,
                    context,
                    last: regs.last,
                    retained: regs.bound,
                    chosen: regs.chosen,
                    boxed: regs.boxed,
                }),
                revealed: None,
            });
            self.frames.push(Frame::referenced_operation(
                Operation::Reveal {
                    target: Target {
                        card: reference.inner,
                        physical: zone,
                        logical: LogicalZone::Physical(zone),
                    },
                },
                context,
                reference,
            ));
            return Ok(());
        }
        let push = |game: &mut Self, pc: u16, regs: Regs| {
            game.frames.push(Frame::Program {
                source,
                pc,
                context,
                last: regs.last,
                retained: regs.bound,
                chosen: regs.chosen,
                boxed: regs.boxed,
            });
        };
        match instruction {
            Instruction::If(cond, skip) => {
                let known = self.condition_known(*cond, context, regs);
                self.retain_private_flow(known, source, &mut regs);
                let skip = if self.holds(*cond, context, regs)? {
                    0
                } else {
                    u16::from(skip)
                };
                push(self, pc + 1 + skip, regs);
                return Ok(());
            }
            Instruction::Jump(skip) => {
                push(self, pc + 1 + u16::from(skip), regs);
                return Ok(());
            }
            Instruction::Block(_) => return self.resume_locals(source, pc, context, regs),
            Instruction::Repeat { keep } => {
                if locals {
                    self.restart_locals(keep);
                }
                push(
                    self,
                    0,
                    Regs {
                        boxed: regs.boxed,
                        ..Regs::EMPTY
                    },
                );
                self.end_mandatory_loop();
                return Ok(());
            }
            Instruction::Bind(Binding::Card(Selector::Reg(reg))) => {
                regs.bound = self.register_reference(reg, context, regs);
                push(self, pc + 1, regs);
                return Ok(());
            }
            Instruction::Bind(binding @ (Binding::Value(..) | Binding::Colors(..))) => {
                self.bind_slot(binding, context, regs)?;
                regs.locals = true;
                push(self, pc + 1, regs);
                return Ok(());
            }
            Instruction::Bind(Binding::Card(from)) => {
                let known = self.selection_known(from, context, regs);
                self.retain_private_flow(known, source, &mut regs);
                // Select with the current registers, then replace the binding.
                let targets = self.select(from, context, regs, Excluded::default())?;
                let named = self.named_references(from, context, regs);
                let references: Vec<_> = targets
                    .iter()
                    .map(|target| find_card(&named, target.card))
                    .collect();
                regs.bound = None;
                push(self, pc + 1, regs);
                if !targets.is_empty() {
                    let (options, tasks) = targets
                        .into_iter()
                        .zip(references)
                        .map(|(target, reference)| {
                            let descriptor = if reference.is_some_and(|held| {
                                self.reference_visible(held, context.controller)
                            }) {
                                TargetDescriptor::Known {
                                    card: target.card,
                                    source: target.physical,
                                }
                            } else {
                                self.position.descriptor(target, context.controller)
                            };
                            (
                                Alternative::Select { target: descriptor },
                                reference.map_or(Task::BindCard(target.card), Task::BindReference),
                            )
                        })
                        .unzip();
                    self.frames.push(Frame::Choose {
                        player: context.controller,
                        options: DecisionOptions::new(options, context),
                        tasks,
                    });
                }
                return Ok(());
            }
            _ => {}
        }
        let progress_known = self.instruction_known(instruction, context, regs);
        if progress_known != 3 {
            self.own_locals().progress_known = progress_known;
            regs.locals = true;
        }
        push(self, pc + 1, regs);
        match instruction {
            Instruction::If(..)
            | Instruction::Jump(_)
            | Instruction::Repeat { .. }
            | Instruction::Bind(_)
            | Instruction::Block(_) => {
                unreachable!("handled before the program frame is pushed")
            }
            Instruction::Move(step) => {
                if let Some(keep) = step.keep {
                    self.begin_keep(keep, false, &mut regs)?;
                }
                self.start_move(step, context, regs)?
            }
            Instruction::Draw(draw) => {
                if let Some(keep) = draw.keep {
                    self.begin_keep(keep, false, &mut regs)?;
                }
                self.start_draw(draw, context, regs)?
            }
            Instruction::Reveal {
                from,
                optional,
                all,
                unless_proven,
                keep,
                by,
            } => {
                // A performer shows cards they may look at (a demander's
                // witness from their own hand) and chooses which.
                let context = match by {
                    Some(who) => {
                        let performer = self.one_player(who, context)?;
                        Context {
                            executor: performer,
                            controller: context.chooser(performer),
                            ..context
                        }
                    }
                    None => context,
                };
                if let Some(condition) = unless_proven {
                    if !all || optional || keep.is_some() {
                        return Err(Error::Unsupported("invalid conditional proof packet"));
                    }
                    // The decision to omit a proof uses only an already public
                    // fact. No private branch is retained in the continuation.
                    if Player::ALL
                        .into_iter()
                        .all(|viewer| self.cond_known(*condition, context, regs, viewer))
                        && self.holds(*condition, context, regs)?
                    {
                        self.complete(ReturnValue::Unit);
                        return Ok(());
                    }
                }
                if let Some(keep) = keep {
                    self.begin_keep(keep, false, &mut regs)?;
                }
                let targets = self.select(from, context, regs, Excluded::default())?;
                let reference = if targets.len() == 1 {
                    self.selector_reference(from, targets[0].card, context, regs)
                } else {
                    None
                };
                let operations: Vec<_> = targets
                    .into_iter()
                    .map(|target| Operation::Reveal { target })
                    .collect();
                if all {
                    let targets = operations
                        .into_iter()
                        .map(|operation| match operation {
                            Operation::Reveal { target } => target,
                            _ => unreachable!(),
                        })
                        .collect();
                    self.frames
                        .push(Frame::RevealAll(Box::new(verification::RevealPacket::new(
                            targets, context,
                        ))));
                } else {
                    self.choose_referenced_operations(context, operations, optional, reference);
                }
            }
            Instruction::Splay(step) if step.all => {
                self.frames.push(Frame::SplayAll(Box::new(SplayAll {
                    step,
                    context,
                    regs,
                    done: 0,
                    outcome: HeldBulk::default(),
                })));
            }
            Instruction::Splay(step) => {
                let (operations, merged) = self.splay_operations(step, context, regs)?;
                self.choose_operations(context, operations, step.optional);
                if merged && let Some(Frame::Choose { options, .. }) = self.frames.last_mut() {
                    options.aliased = true;
                }
            }
            Instruction::Exchange(step) => self.begin_exchange_selection(step, context, regs)?,
            Instruction::Choose {
                who,
                choice,
                optional,
                keep,
                alias,
            } => {
                if let Some(keep) = keep {
                    self.begin_keep(keep, true, &mut regs)?;
                }
                self.start_choice(who, choice, optional, alias, context, regs)?
            }
            Instruction::Execute(execute) => {
                if let Some(selected) = self.register_reference(execute.target, context, regs) {
                    let executor = self.one_player(execute.who, context)?;
                    let source_owner = match self.position.locate(selected.inner) {
                        Some(
                            Zone::Board(owner, _)
                            | Zone::Hand(owner)
                            | Zone::Score(owner)
                            | Zone::Achievements(owner),
                        ) => owner,
                        _ => executor,
                    };
                    let mut child = Context {
                        source_owner,
                        ..context
                    };
                    if executor != context.executor {
                        child.executor = executor;
                        // An inherited controller override outranks the
                        // new executor's own decisions.
                        if !child.controlled() {
                            child.controller = executor;
                        }
                    }
                    if let Some(who) = execute.controller {
                        child.controller = self.one_player(who, context)?;
                        child.set_controlled();
                    }
                    if execute.must {
                        child.set_must();
                    }
                    // An outer grant stays active through its own frame.
                    child.set_inspect(execute.inspect);
                    self.push_copied_reference(selected, execute.mode, child);
                    if let Some(Frame::Execute(execution)) = self.frames.last_mut() {
                        execution.report(execute.count_reveals);
                    }
                } else {
                    self.complete(ReturnValue::Operation(OperationOutcome::empty(
                        Disposition::Inapplicable,
                    )));
                }
            }
            Instruction::Special(step) => match step {
                SpecialStep::JunkAvailable { count, shortcut } => {
                    let remaining = self.eval_count(*count, context, regs)?;
                    self.frames
                        .push(Frame::JunkAvailable(Box::new(JunkAvailable::new(
                            remaining, shortcut, context,
                        ))));
                }
                SpecialStep::Claim(special) => self.frames.push(Frame::operation(
                    Operation::ClaimSpecial {
                        special,
                        check_eligibility: false,
                    },
                    context,
                )),
                SpecialStep::Toggle { optional } => {
                    let operations = self
                        .position
                        .specials()
                        .filter_map(|special| match self.position.special_location(special) {
                            SpecialLocation::Available => {
                                Some(Operation::JunkAvailableSpecial { special })
                            }
                            SpecialLocation::Junk => Some(Operation::RestoreSpecial { special }),
                            SpecialLocation::Owned(_) => None,
                        })
                        .collect();
                    self.choose_operations(context, operations, optional);
                }
                SpecialStep::JunkAll => self.frames.push(Frame::JunkAvailableSpecials {
                    context,
                    considered: 0,
                }),
                SpecialStep::ClaimAny => {
                    let mut operations: Vec<_> = self
                        .select(
                            Selector::AvailableAchievements,
                            context,
                            regs,
                            Excluded::default(),
                        )?
                        .into_iter()
                        .map(|target| {
                            self.movement(
                                Verb::Achieve,
                                target,
                                Destination::Achievements(Who::Executor),
                                context,
                            )
                        })
                        .collect();
                    operations.extend(
                        self.position
                            .specials()
                            .filter(|&special| {
                                self.position.special_location(special)
                                    == SpecialLocation::Available
                            })
                            .map(|special| Operation::ClaimSpecial {
                                special,
                                check_eligibility: false,
                            }),
                    );
                    self.choose_operations(context, operations, false);
                }
                SpecialStep::Majority => {
                    self.grant_action_majority(context.executor);
                    self.complete(ReturnValue::Operation(OperationOutcome::empty(
                        Disposition::Performed,
                    )));
                }
                SpecialStep::Decree(colors) => {
                    let mask = self.color_mask(colors, context, regs)?;
                    match Decree::ALL
                        .into_iter()
                        .find(|decree| mask & (1 << decree.color() as u8) != 0)
                    {
                        Some(decree) if self.position.figures() => self
                            .frames
                            .push(Frame::operation(Operation::IssueDecree { decree }, context)),
                        _ => self.complete(ReturnValue::Operation(OperationOutcome::empty(
                            Disposition::Inapplicable,
                        ))),
                    }
                }
            },
            Instruction::Win(rule) => {
                if rule == WinRule::Draw {
                    self.frames.push(Frame::operation(
                        Operation::ProposeDraw {
                            reason: DrawReason::Effect,
                        },
                        context,
                    ));
                    return Ok(());
                }
                if let WinRule::Loses(who) = rule {
                    let player = self.one_player(who, context)?;
                    self.frames
                        .push(Frame::operation(Operation::Lose { player }, context));
                    return Ok(());
                }
                let winner = match rule {
                    WinRule::Executor => Some(context.executor),
                    WinRule::Player(who) => Some(self.one_player(who, context)?),
                    WinRule::Loses(_) | WinRule::Draw => unreachable!("handled above"),
                    WinRule::Most(metric) | WinRule::Fewest(metric) => {
                        let one = self.metric(metric, Player::One);
                        let two = self.metric(metric, Player::Two);
                        let most = matches!(rule, WinRule::Most(_));
                        match one.cmp(&two) {
                            std::cmp::Ordering::Equal => None,
                            std::cmp::Ordering::Greater if most => Some(Player::One),
                            std::cmp::Ordering::Less if !most => Some(Player::One),
                            _ => Some(Player::Two),
                        }
                    }
                };
                if let Some(winner) = winner {
                    self.frames.push(Frame::operation(
                        Operation::ProposeWin {
                            winner,
                            reason: match source {
                                Source::Effect { .. } => WinReason::Effect,
                                Source::Karma { .. } => WinReason::Karma,
                                Source::CityIcon { .. } => {
                                    unreachable!("City icon programs never win")
                                }
                            },
                        },
                        context,
                    ));
                } else {
                    self.complete(ReturnValue::Unit);
                }
            }
            Instruction::EndAction => self.finish_dogma_action(),
            Instruction::Subgame => {
                if self.subgames {
                    self.begin_subgame(context)?;
                } else {
                    // The draw variant: the game ends in a draw.
                    self.frames.push(Frame::operation(
                        Operation::ProposeDraw {
                            reason: DrawReason::Effect,
                        },
                        context,
                    ));
                }
                return Ok(());
            }
            Instruction::Flip => {
                self.frames.push(Frame::operation(Operation::Flip, context));
                return Ok(());
            }
            Instruction::Native(Native::MaxVerstappen) => {
                // The triggering effect's operation waits beneath this karma.
                let karma = self
                    .frames
                    .iter()
                    .rposition(|frame| matches!(frame, Frame::Karma { .. }));
                let Some(&Frame::Operation {
                    request: request @ Operation::ExecuteEffect { .. },
                    context: original,
                    ..
                }) = karma.and_then(|index| self.frames.get(index.checked_sub(1)?))
                else {
                    unreachable!("Max Verstappen effect caller");
                };
                for _ in 0..regs.last.performed_count() {
                    self.frames.push(Frame::operation(
                        request,
                        Context {
                            scope: Context::SUPPRESS,
                            ..original
                        },
                    ));
                }
            }
            Instruction::Native(native) => {
                self.frames
                    .push(Frame::TerminalEffect(Box::new(TerminalEffect::new(
                        native, context, regs.boxed,
                    )?)))
            }
        }
        Ok(())
    }

    /// A program step with slots or blocks: store the record of the
    /// instruction that just finished, count it toward a running block, end
    /// passes and blocks and enter new ones, then run the next instruction.
    /// Called with the program frame popped (and its sidecar on top, if it
    /// has one).
    #[cold]
    #[inline(never)]
    fn resume_locals(
        &mut self,
        source: Source,
        mut pc: u16,
        context: Context,
        mut regs: Regs,
    ) -> Result<(), Error> {
        let mut locals = matches!(self.frames.last(), Some(Frame::Locals(_)));
        if locals {
            self.commit_kept(context, regs)?;
            self.tally(regs);
            self.own_locals().progress_known = 3;
        }
        let instruction = loop {
            regs.locals = locals;
            if locals
                && let blocks::Boundary::Suspended =
                    self.block_boundary(source, &mut pc, context, &mut regs)?
            {
                return Ok(());
            }
            let Some(instruction) = self.instruction(source, pc) else {
                if locals {
                    self.frames.pop();
                }
                self.complete(ReturnValue::Unit);
                return Ok(());
            };
            let Instruction::Block(block) = instruction else {
                break instruction;
            };
            // Entering is the end of an empty zeroth pass.
            locals = true;
            regs.locals = true;
            pc = self.enter_block(block, source, pc, context, regs)?;
        };
        if locals {
            self.oblige(instruction);
        }
        self.run(instruction, source, pc, context, regs)
    }

    /// Set a slot to a number or a color set (no operation). The program
    /// frame is popped; its sidecar is pushed if it has none.
    #[cold]
    #[inline(never)]
    fn bind_slot(&mut self, binding: Binding, context: Context, regs: Regs) -> Result<(), Error> {
        let known = self.binding_known(binding, context, regs);
        let (slot, value) = match binding {
            Binding::Value(slot, expr) => {
                let value = self.eval_u8(expr, context, regs)?.map(slots::Held::Value);
                (slot, value)
            }
            Binding::Colors(slot, set) => {
                let colors = self.color_mask(set, context, regs)?;
                (slot, Some(slots::Held::Colors(colors)))
            }
            Binding::Card(_) => unreachable!("binds the register"),
        };
        if slot.life() == Life::Effect {
            self.own_locals();
        }
        self.store_scalar(
            Keep {
                slot,
                append: false,
            },
            value,
            context,
            known,
        )
    }

    fn start_move(&mut self, step: Move, context: Context, regs: Regs) -> Result<(), Error> {
        let step = if let Destination::Origin(slot) = step.to {
            let Some(origin) = self.slot_origin(slot, context, regs) else {
                self.complete(ReturnValue::Operation(OperationOutcome::empty(
                    Disposition::Inapplicable,
                )));
                return Ok(());
            };
            Move {
                to: Destination::Board(if origin == context.executor {
                    Who::Executor
                } else {
                    Who::Opponent
                }),
                ..step
            }
        } else {
            step
        };
        let Move {
            verb,
            from,
            to,
            amount,
            shortcut,
            verify,
            when,
            reveal_if_empty,
            keep: _,
            by,
        } = step;
        if reveal_if_empty.is_some() && !matches!(amount, Amount::One { optional: false }) {
            return Err(Error::Unsupported(
                "empty-selection proof requires a mandatory single move",
            ));
        }
        if by.is_some() && !matches!(amount, Amount::One { .. }) {
            return Err(Error::Unsupported("a performer applies to a single move"));
        }
        let excluded = Excluded {
            skip_junk: to == Destination::Junk,
            ..Excluded::default()
        };
        match amount {
            Amount::One { optional } => {
                let mut targets = self.select(from, context, regs, excluded)?;
                if targets.is_empty()
                    && let Some(proof) = reveal_if_empty
                {
                    let known = self.selection_known(from, context, regs)
                        & self.selection_known(*proof, context, regs);
                    let targets = self.select(*proof, context, regs, Excluded::default())?;
                    let mut packet = verification::RevealPacket::new(targets, context);
                    packet.empty_selection = Some(known);
                    self.frames.push(Frame::RevealAll(Box::new(packet)));
                    return Ok(());
                }
                match shortcut {
                    Some(Shortcut::LowestFirst) => {
                        self.reduce_lowest_first(step, context, &mut targets);
                    }
                    Some(Shortcut::SharedPool) if !optional => {
                        self.reduce_shared_pool(step, context, &mut targets);
                    }
                    Some(Shortcut::AnyOrder) if !optional => {
                        self.reduce_mandatory_claim_order(step, context, &mut targets);
                    }
                    _ => {}
                }
                let positional = from.selects_deck_top();
                let named = self.named_references(from, context, regs);
                let operations = targets
                    .into_iter()
                    .map(|target| {
                        let mut reference = find_card(&named, target.card);
                        if positional {
                            let reference =
                                reference.get_or_insert_with(|| self.capture(target.card));
                            reference.evidence = reference.evidence.at_position();
                        }
                        (
                            self.program_movement(verb, from, target, to, context),
                            reference,
                        )
                    })
                    .collect();
                let mut context = self.claimant_context(verb, to, context)?;
                // The operations above resolved "you" as the program's
                // executor; the performer runs and chooses them.
                if let Some(who) = by {
                    let performer = self.one_player(who, context)?;
                    context.executor = performer;
                    context.controller = context.chooser(performer);
                }
                self.choose_operation_references(context, operations, optional);
                self.verify_pending_move_for(verify, when, regs)?;
            }
            Amount::Batch {
                limit,
                stop,
                per_color,
                frozen,
            } => {
                let limit = limit
                    .map(|limit| self.eval_count(limit, context, regs))
                    .transpose()?;
                let mut batch = MoveAll::new(verb, from, to, context);
                batch.regs = regs;
                batch.limit = limit;
                batch.stop = stop;
                batch.per_color = per_color;
                batch.shortcut = shortcut;
                batch.verify = verify;
                batch.when = when;
                if frozen {
                    let targets = self.select(from, context, regs, excluded)?;
                    let named = self.named_references(from, context, regs);
                    let mut originals = Vec::with_capacity(targets.len());
                    for target in targets {
                        let mut reference = find_card(&named, target.card)
                            .unwrap_or_else(|| self.capture(target.card));
                        if from.selects_deck_top() {
                            reference.evidence = reference.evidence.at_position();
                        }
                        originals.push(reference.with(target));
                    }
                    batch.frozen = Some(originals);
                    batch.frozen_selection = true;
                }
                self.frames.push(Frame::MoveAll(Box::new(batch)));
            }
            Amount::PerValue(quota) => {
                let counts = match quota.source {
                    ValueSource::Bonuses(bonuses) => self.bonus_counts(bonuses, context, regs)?,
                    ValueSource::Cards(source) => {
                        let mut counts = [0_u16; 16];
                        for target in self.select(*source, context, regs, Excluded::default())? {
                            counts[usize::from(self.position.value(target.card))] += 1;
                        }
                        counts
                    }
                };
                let mut batch = MoveAll::new(verb, from, to, context);
                batch.regs = regs;
                batch.quotas = Some(counts.map(|count| {
                    u16::try_from(quota.multiplicity.times(count)).unwrap_or(u16::MAX)
                }));
                batch.shortcut = shortcut;
                batch.verify = verify;
                batch.when = when;
                self.frames.push(Frame::MoveAll(Box::new(batch)));
            }
            Amount::Group {
                size: 2,
                constraint: GroupConstraint::SameValueDifferentColors,
                optional: false,
            } if step.keep.is_none()
                && !verify
                && shortcut.is_none()
                && matches!(from, Selector::Tops(_) | Selector::Board(..)) =>
            {
                self.begin_matching_pair(verb, from, to, context, regs)?;
            }
            Amount::Group {
                size,
                constraint,
                optional,
            } => {
                self.begin_group(
                    from,
                    size,
                    Some(constraint),
                    optional,
                    Some(step),
                    false,
                    context,
                    regs,
                )?;
            }
        }
        Ok(())
    }

    fn start_draw(&mut self, draw: Draw, context: Context, regs: Regs) -> Result<(), Error> {
        let Some(value) = self.eval_u8(draw.age, context, regs)? else {
            self.complete(ReturnValue::Operation(OperationOutcome::empty(
                Disposition::Inapplicable,
            )));
            return Ok(());
        };
        let count = match draw.count {
            Some(count) => Some(self.eval_count(count, context, regs)?),
            None => None,
        };
        if draw.any_set {
            // "From any set": one choice among the enabled sets, made before
            // the first draw and kept for a counted series.
            let expansions = self.position.expansions();
            let (options, tasks): (Vec<_>, Vec<_>) = CardSet::ALL
                .into_iter()
                .filter(|&set| expansions.includes(set))
                .map(|set| {
                    (
                        Alternative::Draw { value, set },
                        Task::DrawFrom(Box::new(DrawFrom {
                            value,
                            set: Some(set),
                            count,
                            then: draw.then,
                            context,
                        })),
                    )
                })
                .unzip();
            let mut options = options;
            let mut tasks = tasks;
            if draw.optional && !context.must() {
                options.push(Alternative::Decline);
                tasks.push(Task::Decline);
            }
            self.frames.push(Frame::Choose {
                player: context.controller,
                tasks,
                options: DecisionOptions::new(options, context),
            });
            return Ok(());
        }
        // A card never names the Base deck: Base is the ordinary draw.
        let set = match draw.set {
            CardSet::Base => None,
            set => Some(set),
        };
        self.draw_from(value, set, count, draw.then, draw.optional, context);
        Ok(())
    }

    /// Start a single or counted draw of a resolved set.
    pub(super) fn draw_from(
        &mut self,
        value: u8,
        set: Option<CardSet>,
        count: Option<u16>,
        then: Then,
        optional: bool,
        context: Context,
    ) {
        let Some(count) = count else {
            let request = Operation::Draw { value, set };
            if optional {
                self.choose_operations(context, vec![request], true);
            } else {
                self.frames.push(Frame::operation(request, context));
            }
            return;
        };
        self.frames.push(Frame::DrawCount(DrawCount {
            remaining: count,
            value,
            set,
            then,
            colors: 0,
            repeated_color: false,
            context,
            stage: 0,
            last: HeldReturn::UNIT,
            outcome: HeldBulk::default(),
        }));
    }

    /// Sampling can permute a hidden pile around retained named cards. A
    /// blind choice still ranges over the selector's unnamed remainder,
    /// rather than the old physical positions that happened to contain it.
    pub(super) fn refresh_blind_choice_groups(&mut self) -> Result<(), Error> {
        if !matches!(self.frames.last(), Some(Frame::Choose { tasks, .. })
            if tasks.iter().any(|task| matches!(task, Task::ChooseBlind(_))))
        {
            return Ok(());
        }
        let Some(Frame::Choose {
            player,
            mut tasks,
            options,
        }) = self.frames.pop()
        else {
            unreachable!("checked choice frame")
        };
        let result = (|| {
            let Some(&Frame::Program {
                source,
                pc,
                context,
                last,
                retained,
                chosen,
                boxed,
            }) = self.frames.last()
            else {
                return Ok(());
            };
            let Some(Instruction::Choose {
                choice: Choice::Card(selector),
                ..
            }) = pc
                .checked_sub(1)
                .and_then(|pc| self.instruction(source, pc))
            else {
                return Ok(());
            };
            let regs = Regs {
                last,
                bound: retained,
                chosen,
                boxed,
                locals: matches!(
                    self.frames.as_slice(),
                    [.., Frame::Locals(_), Frame::Program { .. }]
                ),
            };
            let targets = self.select(
                *selector,
                Context {
                    controller: player,
                    ..context
                },
                regs,
                Excluded::default(),
            )?;
            let named: Vec<_> = options
                .alternatives
                .iter()
                .filter_map(|option| match option {
                    Alternative::Select {
                        target: TargetDescriptor::Known { card, .. },
                    } => Some(*card),
                    _ => None,
                })
                .collect();
            for (task, option) in tasks.iter_mut().zip(&options.alternatives) {
                let (Task::ChooseBlind(group), &Alternative::BlindSelect { source, back }) =
                    (task, option)
                else {
                    continue;
                };
                group.targets = targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        target.physical == source
                            && self.position.card(target.card).origin_deck() == back
                            && !named.contains(&target.card)
                    })
                    .collect();
                if group.targets.is_empty() {
                    return Err(Error::Unsupported("sampled blind choice has no targets"));
                }
            }
            Ok(())
        })();
        self.frames.push(Frame::Choose {
            player,
            tasks,
            options,
        });
        result
    }

    /// Named claim alternatives retain identity while anonymous alternatives
    /// retain only their public back. Rebuild the latter from the sampled
    /// selector, excluding the named alternatives, just as for card choices.
    pub(super) fn refresh_blind_operation_groups(&mut self) -> Result<(), Error> {
        fn unverified(task: &mut Task) -> &mut Task {
            match task {
                Task::Verify(inner) => unverified(inner),
                Task::VerifyWhen(proof) => unverified(&mut proof.task),
                task => task,
            }
        }
        fn blind(task: &Task) -> bool {
            match task {
                Task::Verify(inner) => blind(inner),
                Task::VerifyWhen(proof) => blind(&proof.task),
                Task::BlindOperations(_) => true,
                _ => false,
            }
        }
        if !matches!(self.frames.last(), Some(Frame::Choose { tasks, .. })
            if tasks.iter().any(blind))
        {
            return Ok(());
        }
        let Some(Frame::Choose {
            player,
            mut tasks,
            options,
        }) = self.frames.pop()
        else {
            unreachable!("checked operation choice frame")
        };
        let result = (|| {
            let Some(&Frame::Program {
                source,
                pc,
                context,
                last,
                retained,
                chosen,
                boxed,
            }) = self.frames.last()
            else {
                return Ok(());
            };
            let Some(Instruction::Move(step)) = pc
                .checked_sub(1)
                .and_then(|pc| self.instruction(source, pc))
            else {
                return Ok(());
            };
            if !matches!(step.amount, Amount::One { .. }) {
                return Ok(());
            }
            let regs = Regs {
                last,
                bound: retained,
                chosen,
                boxed,
                locals: matches!(
                    self.frames.as_slice(),
                    [.., Frame::Locals(_), Frame::Program { .. }]
                ),
            };
            let excluded = Excluded {
                skip_junk: step.to == Destination::Junk,
                ..Excluded::default()
            };
            let mut targets = self.select(step.from, context, regs, excluded)?;
            match step.shortcut {
                Some(Shortcut::LowestFirst) => {
                    self.reduce_lowest_first(step, context, &mut targets)
                }
                Some(Shortcut::SharedPool)
                    if matches!(step.amount, Amount::One { optional: false }) =>
                {
                    self.reduce_shared_pool(step, context, &mut targets)
                }
                Some(Shortcut::AnyOrder)
                    if matches!(step.amount, Amount::One { optional: false }) =>
                {
                    self.reduce_mandatory_claim_order(step, context, &mut targets)
                }
                _ => {}
            }
            let named: Vec<_> = options
                .alternatives
                .iter()
                .filter_map(|option| match option {
                    Alternative::Move {
                        target: TargetDescriptor::Known { card, .. },
                        ..
                    } => Some(*card),
                    _ => None,
                })
                .collect();
            for (task, option) in tasks.iter_mut().zip(&options.alternatives) {
                let (Task::BlindOperations(group), &Alternative::BlindMove { source, back, .. }) =
                    (unverified(task), option)
                else {
                    continue;
                };
                group.operations = targets
                    .iter()
                    .copied()
                    .filter(|target| {
                        target.physical == source
                            && self.position.card(target.card).origin_deck() == back
                            && !named.contains(&target.card)
                    })
                    .map(|target| {
                        (
                            self.program_movement(step.verb, step.from, target, step.to, context),
                            self.selector_reference(step.from, target.card, context, regs),
                        )
                    })
                    .collect();
                if group.operations.is_empty() {
                    return Err(Error::Unsupported("sampled blind operation has no targets"));
                }
            }
            Ok(())
        })();
        self.frames.push(Frame::Choose {
            player,
            tasks,
            options,
        });
        result
    }

    /// Merge the options an [`Alias`] proves interchangeable, keeping the
    /// first of each class in menu order. The annotated condition may read
    /// only what both players see (backs, boards, decks, achievements), or
    /// the menu's shape would disclose something; the catalog lint enforces
    /// that, so the merge itself never consults a tracker.
    fn alias_choice(
        &self,
        alias: Alias,
        options: &mut Vec<Alternative>,
        tasks: &mut Vec<Task>,
        context: Context,
        regs: Regs,
    ) -> Result<bool, Error> {
        #[cfg(test)]
        if self.shortcuts_disabled {
            return Ok(false);
        }
        let mut keys = Vec::with_capacity(tasks.len());
        for task in tasks.iter() {
            let chosen = match *task {
                Task::ChooseValue(value) => SelectionParameter::Value(value),
                Task::ChooseColors(SelectionParameter::Colors(mask)) => {
                    SelectionParameter::Colors(mask)
                }
                _ => return Ok(false),
            };
            let probe = Regs { chosen, ..regs };
            // The condition reads public facts only (the catalog lint
            // `alias_conditions_are_public`), so tracked games and untracked
            // search worlds build the same menu.
            let key = match alias {
                Alias::NoOpWhen(cond) => self.holds(*cond, context, probe)?.then_some(0),
                Alias::SameKey(expr) => self.eval(*expr, context, probe)?,
            };
            keys.push(key);
        }
        let mut seen: Vec<i64> = Vec::new();
        let keep: Vec<bool> = keys
            .iter()
            .map(|key| match key {
                Some(key) if seen.contains(key) => false,
                Some(key) => {
                    seen.push(*key);
                    true
                }
                None => true,
            })
            .collect();
        let before = tasks.len();
        let mut index = 0;
        options.retain(|_| {
            index += 1;
            keep[index - 1]
        });
        index = 0;
        tasks.retain(|_| {
            index += 1;
            keep[index - 1]
        });
        Ok(tasks.len() < before)
    }

    fn start_choice(
        &mut self,
        who: Who,
        choice: Choice,
        optional: bool,
        alias: Option<Alias>,
        context: Context,
        regs: Regs,
    ) -> Result<(), Error> {
        // A choice the text assigns to another player still belongs to an
        // overriding controller (Hitchhiking).
        let player = match who {
            Who::Executor => context.controller,
            who => context.chooser(self.one_player(who, context)?),
        };
        if let Choice::Cards {
            from,
            up_to,
            constraint,
        } = choice
        {
            return self.begin_group(
                *from,
                up_to,
                constraint,
                optional,
                None,
                true,
                Context {
                    controller: player,
                    ..context
                },
                regs,
            );
        }
        let (mut options, mut tasks): (Vec<_>, Vec<_>) = match choice {
            Choice::Cards { .. } => unreachable!("group choice handled above"),
            Choice::Value(ValueChoices::Any) => (u8::from(!self.position.age_zero_introduced())
                ..=11)
                .map(|value| (Alternative::Value(value), Task::ChooseValue(value)))
                .unzip(),
            Choice::Value(ValueChoices::AnyExceptBoxed(excluded)) => {
                let mut mask = 0_u16;
                for &literal in excluded {
                    mask |= 1 << regs.boxed.value(literal)?.min(15);
                }
                (u8::from(!self.position.age_zero_introduced())..=11)
                    .filter(|&value| mask & (1 << value) == 0)
                    .map(|value| (Alternative::Value(value), Task::ChooseValue(value)))
                    .unzip()
            }
            Choice::Value(ValueChoices::Range(low, high)) => (low..=high)
                .map(|value| (Alternative::Value(value), Task::ChooseValue(value)))
                .unzip(),
            Choice::Value(ValueChoices::Odd) => (1..=11)
                .step_by(2)
                .map(|value| (Alternative::Value(value), Task::ChooseValue(value)))
                .unzip(),
            Choice::FinalCount { colors, draws } => {
                // The drawn-count menu holds only while nothing can
                // change the hand during the draws: a Figure watching draws
                // or a hand-membership modifier switches to the literal
                // final count, bounded publicly by the enabled catalog.
                let guarded = self.position.dependencies_clear(
                    &[
                        crate::shortcuts::ShortcutDependency::Draw,
                        crate::shortcuts::ShortcutDependency::HandMembership,
                    ],
                    context,
                    false,
                );
                if guarded {
                    (0..=draws + 1)
                        .map(|drawn| (Alternative::Value(drawn), Task::ChooseValue(drawn)))
                        .unzip()
                } else {
                    let mask = self.color_mask(colors, context, regs)?;
                    let expansions = self.position.expansions();
                    let bound = self
                        .position
                        .catalog
                        .iter()
                        .filter(|card| {
                            expansions.includes(card.set) && mask & (1 << card.color as u8) != 0
                        })
                        .count() as u16;
                    (0..=bound + 1)
                        .map(|count| (Alternative::Count(count), Task::ChooseCount(count)))
                        .unzip()
                }
            }
            Choice::Value(ValueChoices::Among(selector) | ValueChoices::NotAmong(selector)) => {
                let values = self
                    .select(*selector, context, regs, Excluded::default())?
                    .iter()
                    .fold(0_u16, |mask, target| {
                        mask | (1 << self.selected_value(*selector, target.card, context, regs))
                    });
                let among = matches!(choice, Choice::Value(ValueChoices::Among(_)));
                let low = if among {
                    0
                } else {
                    u8::from(!self.position.age_zero_introduced())
                };
                (low..=11)
                    .filter(|value| (values & (1 << value) != 0) == among)
                    .map(|value| (Alternative::Value(value), Task::ChooseValue(value)))
                    .unzip()
            }
            Choice::Value(ValueChoices::Bonuses(bonuses)) => self
                .bonus_counts(*bonuses, context, regs)?
                .iter()
                .enumerate()
                .filter(|&(_, &count)| count > 0)
                .map(|(value, _)| {
                    let value = value as u8;
                    (Alternative::Value(value), Task::ChooseValue(value))
                })
                .unzip(),
            Choice::Card(selector) => {
                let targets = self.select(
                    *selector,
                    Context {
                        controller: player,
                        ..context
                    },
                    regs,
                    Excluded::default(),
                )?;
                let mut options = Vec::new();
                let mut tasks = Vec::new();
                let named = self.named_references(*selector, context, regs);
                for target in targets {
                    let reference = find_card(&named, target.card);
                    let descriptor =
                        if reference.is_some_and(|held| self.reference_visible(held, player)) {
                            TargetDescriptor::Known {
                                card: target.card,
                                source: target.physical,
                            }
                        } else {
                            self.position.descriptor(target, player)
                        };
                    match descriptor {
                        TargetDescriptor::Hidden { source, back, .. }
                            if !matches!(source, Zone::Safe(_)) =>
                        {
                            let option = Alternative::BlindSelect { source, back };
                            if let Some(index) = options.iter().position(|prior| *prior == option) {
                                let Task::ChooseBlind(group) = &mut tasks[index] else {
                                    unreachable!("blind card choice")
                                };
                                group.targets.push(target);
                            } else {
                                // Only public backs decide menu order. Physical
                                // order within a back is kept for native sampling.
                                let index = options.partition_point(|prior| match prior {
                                    Alternative::BlindSelect { back: prior, .. } => {
                                        (prior.age, prior.set) <= (back.age, back.set)
                                    }
                                    _ => false,
                                });
                                options.insert(index, option);
                                tasks.insert(
                                    index,
                                    Task::ChooseBlind(Box::new(BlindChoice {
                                        targets: vec![target],
                                        excluded: Vec::new(),
                                        chooser: player,
                                    })),
                                );
                            }
                        }
                        // A secret is an ordered public slot, never a random
                        // member of its back group.
                        descriptor => {
                            options.push(Alternative::Select { target: descriptor });
                            tasks.push(
                                reference
                                    .map_or(Task::ChooseCard(target.card), Task::ChooseReference),
                            );
                        }
                    }
                }
                let named: Vec<_> = tasks
                    .iter()
                    .zip(&options)
                    .filter_map(|(task, option)| match (task, option) {
                        (
                            Task::ChooseReference(reference),
                            Alternative::Select {
                                target: TargetDescriptor::Known { source, .. },
                            },
                        ) => Some((*reference, *source)),
                        _ => None,
                    })
                    .collect();
                for (task, option) in tasks.iter_mut().zip(&options) {
                    if let (Task::ChooseBlind(group), Alternative::BlindSelect { source, back }) =
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
                (options, tasks)
            }
            Choice::Value(ValueChoices::Boxed(values)) => {
                let mut pairs = Vec::with_capacity(values.len());
                for &literal in values {
                    let value = regs.boxed.value(literal)?;
                    pairs.push((Alternative::Value(value), Task::ChooseValue(value)));
                }
                pairs.into_iter().unzip()
            }
            Choice::IconTypes { merge_tied_leaders } => {
                #[cfg(test)]
                let merge_tied_leaders = merge_tied_leaders && !self.shortcuts_disabled;
                let mut choices: Vec<_> = self
                    .position
                    .standard_icons()
                    .filter(|&icon| {
                        !merge_tied_leaders
                            || self.position.icon_count(Player::One, icon)
                                != self.position.icon_count(Player::Two, icon)
                    })
                    .map(|icon| (Alternative::Icon(icon), Task::ChooseIcon(icon)))
                    .collect();
                // Kept outside Icon and its fixed standard-count arrays.
                choices.push((
                    Alternative::BonusIcon,
                    Task::ChooseColors(SelectionParameter::Value(u8::MAX)),
                ));
                choices.into_iter().unzip()
            }
            Choice::Icon { except } => self
                .position
                .standard_icons()
                .filter(|&icon| Some(icon) != except)
                .map(|icon| (Alternative::Icon(icon), Task::ChooseIcon(icon)))
                .unzip(),
            Choice::MostIcons(who) => {
                let owner = self.one_player(who, context)?;
                let counts: Vec<_> = self
                    .position
                    .standard_icons()
                    .map(|icon| (icon, self.position.icon_count(owner, icon)))
                    .collect();
                let most = counts.iter().map(|&(_, count)| count).max().unwrap_or(0);
                counts
                    .into_iter()
                    .filter(|&(_, count)| count == most)
                    .map(|(icon, _)| (Alternative::Icon(icon), Task::ChooseIcon(icon)))
                    .unzip()
            }
            Choice::Color(set) => self
                .colors(set, context, regs)?
                .map(|color| {
                    let mask = 1 << color as u8;
                    (
                        Alternative::Colors(mask),
                        Task::ChooseColors(SelectionParameter::Colors(mask)),
                    )
                })
                .unzip(),
            Choice::Colors(count) => {
                if count == 0 || count > Color::ALL.len() as u8 {
                    return Err(Error::Unsupported("color choice count"));
                }
                (1_u8..(1 << Color::ALL.len()))
                    .filter(|mask| mask.count_ones() == u32::from(count))
                    .map(|mask| {
                        (
                            Alternative::Colors(mask),
                            Task::ChooseColors(SelectionParameter::Colors(mask)),
                        )
                    })
                    .unzip()
            }
            Choice::ColorsAmong(count, colors) => {
                if count == 0 || count > Color::ALL.len() as u8 {
                    return Err(Error::Unsupported("color choice count"));
                }
                let among = self.color_mask(colors, context, regs)?;
                let wanted = u32::from(count).min(among.count_ones());
                (1_u8..(1 << Color::ALL.len()))
                    .filter(|mask| mask & !among == 0 && mask.count_ones() == wanted)
                    .map(|mask| {
                        (
                            Alternative::Colors(mask),
                            Task::ChooseColors(SelectionParameter::Colors(mask)),
                        )
                    })
                    .unzip()
            }
            Choice::Branch(branches) | Choice::SetBranch { branches, .. } => {
                let sets = match choice {
                    Choice::SetBranch { sets, .. } => {
                        if sets.len() != branches.len() {
                            return Err(Error::Unsupported("set branch count"));
                        }
                        Some(sets)
                    }
                    _ => None,
                };
                branches
                    .iter()
                    .enumerate()
                    .filter(|&(index, _)| {
                        sets.is_none_or(|sets| self.position.expansions().includes(sets[index]))
                    })
                    .map(|(index, &(skip, _))| {
                        (
                            Alternative::Branch(index),
                            Task::Branch(Some(usize::from(skip))),
                        )
                    })
                    .unzip()
            }
        };
        if tasks.is_empty() {
            // Nothing to choose from (an empty pile): the register stays empty.
            return Ok(());
        }
        let aliased = match alias {
            Some(alias) if tasks.len() > 1 => {
                self.alias_choice(alias, &mut options, &mut tasks, context, regs)?
            }
            _ => false,
        };
        if optional && !context.must() {
            options.push(Alternative::Decline);
            tasks.push(match choice {
                Choice::Colors(_) | Choice::ColorsAmong(..) | Choice::Color(_) => {
                    Task::ChooseColors(SelectionParameter::None)
                }
                Choice::Branch(_) | Choice::SetBranch { .. } => Task::Branch(None),
                _ => Task::Decline,
            });
        }
        let mut options = DecisionOptions::new(options, context);
        options.aliased = aliased;
        if let Choice::Branch(branches) | Choice::SetBranch { branches, .. } = choice {
            options.branches = branches;
        }
        self.frames.push(Frame::Choose {
            player,
            options,
            tasks,
        });
        Ok(())
    }
}

#[cfg(test)]
impl Game {
    /// Test adapter: select with explicit bound/chosen registers.
    pub(super) fn select_bound(
        &self,
        selector: Selector,
        context: Context,
        bound: Option<CardId>,
        chosen: SelectionParameter,
    ) -> Result<Vec<Target>, Error> {
        self.select(
            selector,
            context,
            Regs {
                bound: bound.map(|card| self.held(card)),
                chosen,
                ..Regs::EMPTY
            },
            Excluded::default(),
        )
    }

    /// Test adapter: evaluate an expression with only a context.
    pub(super) fn query_count(&self, expr: Expr, context: Context) -> i64 {
        self.eval(expr, context, Regs::EMPTY)
            .unwrap()
            .expect("defined count")
    }

    /// Test adapter: evaluate a value with explicit registers.
    pub(super) fn query_value(
        &self,
        expr: Expr,
        context: Context,
        bound: Option<CardId>,
        chosen: SelectionParameter,
    ) -> Option<u8> {
        self.eval(
            expr,
            context,
            Regs {
                bound: bound.map(|card| self.held(card)),
                chosen,
                ..Regs::EMPTY
            },
        )
        .unwrap()
        .map(|value| value as u8)
    }

    /// Test adapter: apply a single move's shortcut at a program position.
    pub(super) fn reduce_moves(
        &self,
        shortcut: Shortcut,
        (card, effect, pc): (CardId, usize, usize),
        context: Context,
        targets: &mut Vec<Target>,
    ) {
        let Instruction::Move(step) = self.position.card(card).effects[effect].instructions[pc]
        else {
            return;
        };
        if step.shortcut == Some(shortcut) && shortcut == Shortcut::LowestFirst {
            self.reduce_lowest_first(step, context, targets);
        }
    }
}

impl Game {
    /// A Repeat that finds the whole game exactly as at an earlier Repeat
    /// with no decision since can never stop: the game is drawn. Any set
    /// whose cards return cards to decks can loop (Figures karma, Echoes and
    /// Artifacts Repeat effects), so every mode checks; a loop that reshuffles
    /// never repeats exactly and ends at the automatic-work cap instead.
    fn end_mandatory_loop(&mut self) {
        let state = Recurrence {
            position: self.position.clone(),
            frames: self.rules_frames(),
            rng: self.rng.clone(),
        };
        if self.repeats.contains(&state) {
            self.repeats.clear();
            self.frames.clear();
            self.terminal = Some(Terminal::Draw(DrawReason::Loop));
        } else if self.repeats.len() < 256 {
            // Bound witness storage; the automatic-work cap also catches
            // cycles that do not fit in this exact cache.
            self.repeats.push(state);
        }
    }
}
