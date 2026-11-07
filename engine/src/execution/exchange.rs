use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExchangeSelection {
    selectors: [Selector; 2],
    counts: [Option<u16>; 2],
    zones: [LogicalZone; 2],
    selected: [Vec<Target>; 2],
    side: usize,
    context: Context,
    regs: Regs,
    /// Re-admit a frozen cohort after a capacity change, without reading its faces.
    frozen: Option<ExchangePlan>,
    committing: bool,
}

impl ExchangeSelection {
    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            selectors: _,
            counts: _,
            zones: _,
            selected,
            side: _,
            context,
            regs,
            frozen,
            committing: _,
        } = self;
        map.regs(regs);
        if let Some(plan) = frozen {
            map.context(&mut plan.context);
            for target in plan.left.iter_mut().chain(&mut plan.right) {
                map.target(target);
            }
        }
        map.context(context);
        for target in selected.iter_mut().flatten() {
            map.target(target);
        }
    }

    pub(super) fn take(&mut self, target: Target) {
        self.selected[self.side].push(target);
    }
}

fn physical(zone: LogicalZone) -> Zone {
    match zone {
        LogicalZone::Hand(player) => Zone::Hand(player),
        LogicalZone::Score(player) => Zone::Score(player),
        LogicalZone::AvailableAchievements(_) => Zone::AvailableAchievements,
        LogicalZone::Physical(
            zone @ (Zone::Hand(_)
            | Zone::Score(_)
            | Zone::Forecast(_)
            | Zone::Safe(_)
            | Zone::Board(..)
            | Zone::AvailableAchievements
            | Zone::Achievements(_)
            | Zone::Junk),
        ) => zone,
        _ => unreachable!("validated exchange zone"),
    }
}

fn blind_order(option: &Alternative) -> (u8, usize, u8, CardSet) {
    match option {
        Alternative::BlindSelect { source, back } => {
            let origin = match source {
                Zone::Hand(p) => p.index(),
                Zone::Score(p) => 2 + p.index(),
                _ => 4,
            };
            (0, origin, back.age, back.set)
        }
        _ => (1, 0, 0, CardSet::Base),
    }
}

impl Game {
    /// The pile an exchange cohort comes from and the other cohort goes to.
    /// A slot cohort (Tasmanian Tiger's chosen cards) is wherever its
    /// recorded card lies: a hand, score pile or board pile.
    fn exchange_zone(
        &self,
        selector: Selector,
        context: Context,
        regs: Regs,
    ) -> Result<LogicalZone, Error> {
        let player = |who| match who {
            Who::Executor => Ok(context.executor),
            Who::Opponent => Ok(context.executor.opponent()),
            Who::Demander => Ok(context.demander),
            Who::Either => Err(Error::Unsupported("exchange cohorts name one player")),
        };
        match selector {
            Selector::Filtered(source, _) => self.exchange_zone(*source, context, regs),
            Selector::Hand(who) => Ok(LogicalZone::Hand(player(who)?)),
            Selector::Score(who) => Ok(LogicalZone::Score(player(who)?)),
            Selector::Forecast(who) => Ok(LogicalZone::Physical(Zone::Forecast(player(who)?))),
            Selector::Safe(who) => Ok(LogicalZone::Physical(Zone::Safe(player(who)?))),
            Selector::Museum(who) => Ok(LogicalZone::Physical(Zone::Museum(player(who)?))),
            Selector::AvailableAchievements => {
                Ok(LogicalZone::AvailableAchievements(context.executor))
            }
            // The same pile; the selector alone excludes Figure board aliases.
            Selector::StandardAchievements => {
                Ok(LogicalZone::AvailableAchievements(context.executor))
            }
            Selector::Junk => Ok(LogicalZone::Physical(Zone::Junk)),
            Selector::Achievements(who) => {
                Ok(LogicalZone::Physical(Zone::Achievements(player(who)?)))
            }
            Selector::Slot(slot) => {
                let card = self
                    .slot_cards(slot, context, regs)
                    .last()
                    .ok_or(Error::Unsupported("an empty exchange slot"))?;
                match self.position.locate(card.inner) {
                    Some(Zone::Hand(player)) => Ok(LogicalZone::Hand(player)),
                    Some(Zone::Score(player)) => Ok(LogicalZone::Score(player)),
                    Some(zone @ (Zone::Forecast(_) | Zone::Safe(_) | Zone::Board(..))) => {
                        Ok(LogicalZone::Physical(zone))
                    }
                    _ => Err(Error::Unsupported("unsupported exchange cohort")),
                }
            }
            _ => Err(Error::Unsupported("unsupported exchange cohort")),
        }
    }

    pub(super) fn begin_exchange_selection(
        &mut self,
        step: ExchangeStep,
        context: Context,
        regs: Regs,
    ) -> Result<(), Error> {
        let ExchangeStep {
            left,
            right,
            counts: &[left_count, right_count],
            optional,
        } = step;
        let zones = [
            self.exchange_zone(left, context, regs)?,
            self.exchange_zone(right, context, regs)?,
        ];
        let mut counts = [None; 2];
        for (count, expr) in counts.iter_mut().zip([left_count, right_count]) {
            *count = expr
                .map(|expr| self.eval_count(expr, context, regs))
                .transpose()?;
        }
        // Validate both selections before presenting choices or discovering Would.
        let left_targets = self.select(left, context, regs, Excluded::default())?;
        let right_targets = self.select(right, context, regs, Excluded::default())?;
        let targets = [&left_targets, &right_targets];
        for side in 0..2 {
            // A full outgoing cohort is already frozen, so its departures can
            // fund admission before any incoming hidden selections are made.
            if counts[1 - side].is_none()
                && physical(zones[side]) != physical(zones[1 - side])
                && let Some(room) = self.exchange_room(physical(zones[1 - side]), targets[1 - side])
            {
                counts[side] = Some(counts[side].map_or(room, |count| count.min(room)));
            }
        }
        let [left_count, right_count] = counts;
        if optional
            && left_count.is_none()
            && right_count.is_none()
            && left_targets
                .iter()
                .all(|t| right_targets.iter().any(|r| r.card == t.card))
            && right_targets
                .iter()
                .all(|t| left_targets.iter().any(|l| l.card == t.card))
        {
            self.complete(ReturnValue::Operation(OperationOutcome::empty(
                Disposition::Inapplicable,
            )));
            return Ok(());
        }
        let selection = ExchangeSelection {
            selectors: [left, right],
            counts: [left_count, right_count],
            zones,
            selected: [
                if left_count.is_none() {
                    left_targets
                } else {
                    Vec::new()
                },
                if right_count.is_none() {
                    right_targets
                } else {
                    Vec::new()
                },
            ],
            side: 0,
            context,
            regs,
            frozen: None,
            committing: false,
        };
        self.frames.push(Frame::SelectExchange(Box::new(selection)));
        if optional && !context.must() {
            self.frames.push(Frame::Choose {
                player: context.controller,
                tasks: vec![Task::BeginExchange, Task::DeclineExchange],
                options: DecisionOptions::new(
                    vec![
                        Alternative::Exchange {
                            left: zones[0],
                            right: zones[1],
                        },
                        Alternative::Decline,
                    ],
                    context,
                ),
            });
        }
        Ok(())
    }

    pub(super) fn resume_exchange_selection(
        &mut self,
        mut selection: Box<ExchangeSelection>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        while selection.side < 2 {
            let side = selection.side;
            let selector = selection.selectors[side];
            let Some(count) = selection.counts[side] else {
                selection.side += 1;
                continue;
            };
            if selection.selected[side].len() >= usize::from(count) {
                selection.side += 1;
                continue;
            }
            let visited: Vec<_> = selection.selected[side]
                .iter()
                .map(|t| self.capture(t.card))
                .collect();
            let mut targets = if let Some(plan) = &selection.frozen {
                let cohort = if side == 0 { &plan.left } else { &plan.right };
                cohort
                    .iter()
                    .copied()
                    .filter(|target| {
                        !selection.selected[side]
                            .iter()
                            .any(|chosen| chosen.card == target.card)
                    })
                    .collect()
            } else {
                self.select(
                    selector,
                    selection.context,
                    selection.regs,
                    Excluded {
                        cards: &visited,
                        ..Excluded::default()
                    },
                )?
            };
            self.reduce_exchange_order(&selection, &mut targets);
            if targets.is_empty() {
                selection.side += 1;
                continue;
            }
            let mut options = Vec::new();
            let mut tasks = Vec::new();
            for target in targets {
                match self
                    .position
                    .descriptor(target, selection.context.controller)
                {
                    TargetDescriptor::Hidden { source, back, .. }
                        if !matches!(source, Zone::Safe(_)) =>
                    {
                        let option = Alternative::BlindSelect { source, back };
                        if let Some(index) = options.iter().position(|prior| *prior == option) {
                            let Task::ExchangeBlind(group) = &mut tasks[index] else {
                                unreachable!("blind cohort")
                            };
                            group.targets.push(target);
                        } else {
                            // Stable insertion uses public backs only; physical
                            // group order is retained for native blind sampling.
                            let key = blind_order(&option);
                            let index = options.partition_point(|prior| blind_order(prior) <= key);
                            options.insert(index, option);
                            tasks.insert(
                                index,
                                Task::ExchangeBlind(Box::new(BlindReturn {
                                    targets: vec![target],
                                })),
                            );
                        }
                    }
                    target_descriptor => {
                        options.push(Alternative::Select {
                            target: target_descriptor,
                        });
                        tasks.push(Task::ExchangeCard(target));
                    }
                }
            }
            let context = selection.context;
            self.frames.push(Frame::SelectExchange(selection));
            self.frames.push(Frame::Choose {
                player: context.controller,
                tasks,
                options: DecisionOptions::new(options, context),
            });
            return Ok(());
        }
        // A cohort is a set, not a sequence of transfers. Preserve its original
        // logical-pile order even when its tied subset was selected in reverse.
        for side in 0..2 {
            if selection.counts[side].is_none() {
                continue;
            }
            let frozen_cohort = selection
                .frozen
                .as_ref()
                .map(|plan| if side == 0 { &plan.left } else { &plan.right });
            selection.selected[side].sort_unstable_by_key(|target| {
                if let Some(cohort) = frozen_cohort {
                    return cohort
                        .iter()
                        .position(|original| original.card == target.card)
                        .expect("frozen exchange membership");
                }
                self.position
                    .targets(selection.zones[side])
                    .position(|original| original.card == target.card)
                    .expect("frozen exchange membership")
            });
        }
        let [left, right] = selection.selected;
        let [left_zone, right_zone] = selection.zones;
        let plan = ExchangePlan {
            left,
            right,
            left_destination: selection
                .frozen
                .as_ref()
                .map_or(physical(right_zone), |p| p.left_destination),
            right_destination: selection
                .frozen
                .as_ref()
                .map_or(physical(left_zone), |p| p.right_destination),
            context: selection.context,
        };
        if selection.committing {
            self.finish_exchange_plan(plan, events)
        } else {
            self.start_exchange_plan(plan, left_zone, right_zone, selection.context, events)
        }
    }

    /// Capacity includes the simultaneous outgoing cards. The caller excludes
    /// overlap before passing departures, since an overlapping card stays put.
    fn exchange_room(&self, destination: Zone, departing: &[Target]) -> Option<u16> {
        let limit = match destination {
            Zone::Forecast(owner) => self.position.forecast_limit(owner),
            Zone::Safe(owner) => self.position.safe_limit(owner),
            _ => return None,
        };
        let leaving = departing
            .iter()
            .filter(|t| t.physical == destination)
            .count();
        let staying = self
            .position
            .cards(destination)
            .len()
            .saturating_sub(leaving);
        Some(limit.saturating_sub(staying) as u16)
    }

    /// Never truncate a frozen hidden cohort by its internal identity order.
    /// Re-offer only its public backs if live capacity admits a proper subset.
    fn exchange_admission(&mut self, plan: &ExchangePlan, committing: bool) -> bool {
        let cohorts = [&plan.left, &plan.right];
        let destinations = [plan.left_destination, plan.right_destination];
        let mut counts = [None; 2];
        let mut selected = [Vec::new(), Vec::new()];
        for side in 0..2 {
            let overlap = |target: &&Target| {
                cohorts[1 - side]
                    .iter()
                    .any(|other| other.card == target.card)
            };
            let departures: Vec<_> = cohorts[1 - side]
                .iter()
                .filter(|target| {
                    !cohorts[side].iter().any(|other| other.card == target.card)
                        && target.physical != destinations[1 - side]
                })
                .copied()
                .collect();
            let moving = cohorts[side]
                .iter()
                .filter(|target| !overlap(target) && target.physical != destinations[side])
                .count();
            if let Some(room) = self.exchange_room(destinations[side], &departures)
                && moving > usize::from(room)
            {
                selected[side] = cohorts[side]
                    .iter()
                    .filter(|target| overlap(target) || target.physical == destinations[side])
                    .copied()
                    .collect();
                counts[side] = Some(selected[side].len() as u16 + room);
            } else {
                selected[side] = cohorts[side].clone();
            }
        }
        if counts == [None, None] {
            return false;
        }
        // Only Forecast destinations are limited today. Their opposite source
        // is enough to restore the original logical endpoint for Would hooks.
        let zones = [
            LogicalZone::Physical(plan.right_destination),
            LogicalZone::Physical(plan.left_destination),
        ];
        self.frames
            .push(Frame::SelectExchange(Box::new(ExchangeSelection {
                selectors: [Selector::AvailableAchievements; 2],
                counts,
                zones,
                selected,
                side: 0,
                context: plan.context,
                regs: Regs::EMPTY,
                frozen: Some(plan.clone()),
                committing,
            })));
        true
    }

    pub(super) fn finish_exchange_plan(
        &mut self,
        mut plan: ExchangePlan,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        // A First reaction may have moved selected cards. Rejected or stale
        // incoming cards stay where they are and never become visible here.
        plan.left
            .retain(|target| self.position.cards(target.physical).contains(&target.card));
        plan.right
            .retain(|target| self.position.cards(target.physical).contains(&target.card));
        if self.exchange_admission(&plan, true) {
            return Ok(());
        }
        let outcome = self.commit_exchange(&plan, plan.context, events)?;
        if outcome.disposition == Disposition::Performed {
            self.note_shared_change(plan.context);
        }
        self.complete(ReturnValue::Operation(outcome));
        Ok(())
    }

    /// A cohort is a set, not a sequence: taking tied candidates in any order
    /// freezes the same plan. Keep one canonical order of each counted subset
    /// while no reaction can run during selection and the chooser sees every
    /// candidate in its own pile. Hidden picks keep their native sampling.
    fn reduce_exchange_order(&self, selection: &ExchangeSelection, targets: &mut Vec<Target>) {
        #[cfg(test)]
        if self.shortcuts_disabled {
            return;
        }
        let side = selection.side;
        let Some(count) = selection.counts[side] else {
            return;
        };
        let remaining = usize::from(count) - selection.selected[side].len();
        if (matches!(
            selection.selectors[side],
            Selector::AvailableAchievements | Selector::StandardAchievements
        ) || selection.frozen.is_some())
            && targets
                .iter()
                .chain(&selection.selected[side])
                .all(|target| target.physical == Zone::AvailableAchievements)
        {
            let back = |target: &Target| self.position.card(target.card).origin_deck();
            // No card moves and no reaction occurs between these picks. When
            // every card fits, force a public-back group, retaining native
            // sampling within it. The same cohort, RNG count and reaction order
            // result even with Yi Sun-sin on the board.
            if remaining >= targets.len() {
                if let Some(first) = targets
                    .iter()
                    .map(back)
                    .min_by_key(|deck| (deck.age, deck.set))
                {
                    targets.retain(|target| back(target) == first);
                }
                return;
            }
            // Proper subsets can be canonicalized when every public back is
            // unique: then no sampling stream depends on the selection order.
            // Mixed duplicate-back subsets retain their native order choices.
            let cohort: Vec<_> = targets.iter().chain(&selection.selected[side]).collect();
            if cohort.iter().enumerate().all(|(index, target)| {
                cohort[..index]
                    .iter()
                    .all(|other| back(other) != back(target))
            }) {
                targets.sort_by_key(|target| (back(target).age, back(target).set));
                if let Some(last) = selection.selected[side].last() {
                    let floor = (back(last).age, back(last).set);
                    targets.retain(|target| (back(target).age, back(target).set) > floor);
                }
                targets.truncate(targets.len().saturating_sub(remaining) + 1);
            }
            return;
        }
        let pile = physical(selection.zones[side]);
        if selection.context.controller != selection.context.executor
            || targets.iter().any(|target| {
                target.physical != pile
                    || !self.position.can_view(pile, selection.context.controller)
            })
            || (self.position.figures()
                && Player::ALL.into_iter().any(|player| {
                    Color::ALL.into_iter().any(|color| {
                        self.position
                            .top(player, color)
                            .is_some_and(|card| self.position.card(card).figure.is_some())
                    })
                }))
        {
            return;
        }
        let remaining = usize::from(count) - selection.selected[side].len();
        let needed_in_tier = remaining.min(targets.len());
        if let Some(last) = selection.selected[side].last() {
            let pile = self.position.cards(last.physical);
            let last_index = pile.iter().position(|&card| card == last.card).unwrap();
            let last_value = self.position.value(last.card);
            targets.retain(|target| {
                self.position.value(target.card) != last_value
                    || pile.iter().position(|&card| card == target.card).unwrap() > last_index
            });
        }
        let choices = targets.len().saturating_sub(needed_in_tier) + 1;
        targets.truncate(choices);
    }

    pub(super) fn start_exchange_plan(
        &mut self,
        plan: ExchangePlan,
        left: LogicalZone,
        right: LogicalZone,
        context: Context,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        if self.exchange_admission(&plan, false) {
            return Ok(());
        }
        // Membership and overlap are frozen before Would. An empty half still
        // exchanges; zero physical movers is inapplicable before any reaction.
        let hooks = if self.position.watched() & On::Exchange.bit() != 0
            && !context.suppress_karma()
            && plan.movements().next().is_some()
        {
            self.hooks(Operation::ExchangeAll { left, right }, context, None)?
                .0
        } else {
            Vec::new()
        };
        if hooks.is_empty() {
            self.finish_exchange_plan(plan, events)?;
        } else {
            self.frames.push(Frame::Exchange {
                plan: Box::new(plan),
                phase: Phase::Discover,
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
        }
        Ok(())
    }
}

#[cfg(test)]
use super::shortcuts::common;
#[cfg(test)]
#[path = "exchange_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../echoes_tests/exchanges.rs"]
mod echoes_tests;

impl ExchangeSelection {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.add(
            K::Exchange,
            parent,
            None,
            &[
                (F::Side, self.side as u64),
                (F::Committing, u64::from(self.committing)),
                (F::Frozen, u64::from(self.frozen.is_some())),
            ],
        );
        p.context(id, self.context);
        for side in 0..2 {
            let side_id = p.add(
                K::Group,
                id,
                None,
                &[
                    (F::Side, side as u64),
                    (
                        F::Limit,
                        self.counts[side].map_or(0, |count| u64::from(count) + 1),
                    ),
                    (
                        F::SourceZone,
                        u64::from(crate::observation::zone_code(
                            physical(self.zones[side]),
                            p.viewer,
                        )),
                    ),
                ],
            );
            for (ordinal, &target) in self.selected[side].iter().enumerate() {
                p.exchange_target(side_id, target, side, ordinal);
            }
        }
        if let Some(plan) = &self.frozen {
            p.exchange_plan(id, plan, 0);
        }
    }
}
