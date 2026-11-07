//! Blocks: a program runs its next few
//! instructions once per item of a domain, then reads the whole block's
//! result. State lives in the program's [`slots::Locals`] sidecar.
use super::slots::Held;
use super::*;

/// One pass's item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Item {
    Pass,
    Value(u8),
    Color(Color),
    Card(HeldCard),
}

/// A running block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Active {
    block: Block,
    /// Knowledge of frozen scalar domain membership, not card identities.
    known: u8,
    /// The first body instruction; the body ends `block.len` later.
    start: u16,
    /// Frozen items not visited yet, in visiting order.
    items: Vec<Item>,
    /// A live domain's cards and colors already visited.
    visited: Vec<HeldCard>,
    colors: u8,
    /// The chooser stopped a live block.
    stopped: bool,
    /// An any-order block's most splays in one pass.
    splays: u8,
    /// An operation instruction of this pass awaits its result.
    tally: bool,
    performed: u16,
    /// Operations that did not fully happen.
    failed: u16,
    /// The latest performed operation's card.
    card: Option<HeldCard>,
}

impl Active {
    pub(super) fn known(&self) -> u8 {
        self.known
    }
    pub(super) fn item_slot(&self) -> Option<Slot> {
        self.block.item
    }

    fn end(&self) -> u16 {
        self.start + u16::from(self.block.len)
    }

    /// Fold one operation's result into the block's: each performed
    /// operation counts, with or without a card (a splay has none).
    fn count(&mut self, result: HeldReturn) {
        let performed = match result.inner {
            ReturnValue::Operation(outcome) => {
                u16::from(outcome.disposition == Disposition::Performed)
            }
            other => other.performed_count(),
        };
        self.performed = self.performed.saturating_add(performed);
        if !result.inner.fulfilled() {
            self.failed = self.failed.saturating_add(1);
        }
        if let Some(card) = result.card() {
            self.card = Some(card);
        }
    }

    /// The whole block's result: every operation fully happened, and at
    /// least one did.
    fn result(&self) -> HeldReturn {
        let outcome = BulkOutcome {
            performed: self.performed,
            replaced: 0,
            inapplicable: self.failed,
            distinct_values: 0,
            card: self.card.map(|card| card.inner),
            value: None,
        };
        let evidence = self.card.map_or(Evidence::NONE, |card| card.evidence);
        Captured::new(ReturnValue::Bulk(outcome), evidence)
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            block: _,
            known,
            start: _,
            items,
            visited,
            colors: _,
            stopped: _,
            splays: _,
            tally: _,
            performed: _,
            failed: _,
            card,
        } = self;
        map.scalar(known);
        for item in items {
            if let Item::Card(card) = item {
                map.held(card);
            }
        }
        for card in visited {
            map.held(card);
        }
        map.held_optional(card);
    }
}

/// What a pass boundary decided.
pub(super) enum Boundary {
    /// Run (or continue after) the program at this pc.
    Continue,
    /// The program waits for the chooser of the next item.
    Suspended,
}

/// Instructions whose results are a pass's obligations; choices, binds and
/// control flow are not.
fn obliges(instruction: Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Move(_)
            | Instruction::Draw(_)
            | Instruction::Reveal { .. }
            | Instruction::Splay(_)
            | Instruction::Exchange(_)
            | Instruction::Special(_)
            | Instruction::Execute(..)
    )
}

impl Game {
    fn innermost(&mut self) -> Option<&mut Active> {
        match self.frames.last_mut() {
            Some(Frame::Locals(locals)) => locals.blocks.last_mut(),
            _ => None,
        }
    }

    /// The program resumed after an operation of a pass: count its result.
    pub(super) fn tally(&mut self, regs: Regs) {
        let known = self.last_known(regs);
        if let Some(active) = self.innermost()
            && active.tally
        {
            active.tally = false;
            active.known &= known;
            active.count(regs.last);
        }
    }

    /// The program is about to run `instruction`: inside a block, an
    /// operation's result will count toward the block's.
    pub(super) fn oblige(&mut self, instruction: Instruction) {
        if obliges(instruction)
            && let Some(active) = self.innermost()
        {
            active.tally = true;
        }
    }

    /// Enter a block at `pc`, freezing its items; the caller continues at
    /// the end of its (empty) zeroth pass.
    pub(super) fn enter_block(
        &mut self,
        block: Block,
        source: Source,
        pc: u16,
        context: Context,
        regs: Regs,
    ) -> Result<u16, Error> {
        if block.len == 0 {
            return Err(Error::Unsupported("an empty block"));
        }
        let splays = match block.shortcut {
            None => 0,
            Some(_) => self
                .any_order_splays(block, source, pc)
                .ok_or(Error::Unsupported(
                    "a block shortcut needs a frozen chosen order over splays",
                ))?,
        };
        if block.item.is_some_and(|slot| slot.life() != Life::Effect) {
            return Err(Error::Unsupported("a block item needs an effect slot"));
        }
        if let Domain::Live { stop, .. } = block.domain
            && stop != Stop::Never
            && !matches!(block.order, Order::Chosen(_))
        {
            return Err(Error::Unsupported("a stoppable live block needs a chooser"));
        }
        let known = self.domain_known(block.domain, context, regs);
        let mut regs = regs;
        self.retain_private_flow(known, source, &mut regs);
        let mut items = self.block_items(block, context, regs)?;
        match block.order {
            Order::Printed | Order::Chosen(_) => {}
            Order::Ascending => items.sort_by_key(|&item| self.item_value(item)),
            Order::Descending => {
                items.sort_by_key(|&item| std::cmp::Reverse(self.item_value(item)));
            }
        }
        let locals = self.own_locals();
        if locals.blocks.len() >= Block::DEPTH {
            return Err(Error::Unsupported("blocks nested too deep"));
        }
        let start = pc + 1;
        locals.blocks.push(Active {
            block,
            known,
            start,
            items,
            visited: Vec::new(),
            colors: 0,
            stopped: false,
            splays,
            tally: false,
            performed: 0,
            failed: 0,
            card: None,
        });
        Ok(start + u16::from(block.len))
    }

    /// The only block reduction: [`Shortcut::AnyOrder`] on a chosen
    /// order of frozen items whose body only splays. Splays of different
    /// piles commute (Touchscreen); the guard is checked at each pass.
    /// Returns the most splays one pass can perform, along any path of its
    /// forward-only conditions and jumps.
    fn any_order_splays(&self, block: Block, source: Source, pc: u16) -> Option<u8> {
        if block.shortcut != Some(Shortcut::AnyOrder)
            || !matches!(block.order, Order::Chosen(_))
            || matches!(block.domain, Domain::Live { .. })
        {
            return None;
        }
        let len = usize::from(block.len);
        // `most[i]`: the most splays from body instruction `i` to the end.
        let mut most = vec![0_u8; len + 1];
        for i in (0..len).rev() {
            let after = |skip: u8| most.get(i + 1 + usize::from(skip)).copied();
            most[i] = match self.instruction(source, pc + 1 + i as u16)? {
                Instruction::If(_, skip) => after(0)?.max(after(skip)?),
                Instruction::Jump(skip) => after(skip)?,
                // A batch may splay every pile of both boards.
                Instruction::Splay(step) => {
                    if !matches!(step.choices, SplayChoices::Fixed { .. }) {
                        return None; // Copying directions can depend on earlier splays.
                    }
                    let piles = if step.all { 2 * Color::ALL.len() } else { 1 };
                    after(0)?.saturating_add(piles as u8)
                }
                _ => return None,
            };
        }
        Some(most[0])
    }

    /// Whether an any-order block may take its next item in printed order:
    /// as for a splay batch, no live Figure observes splays (or, with
    /// Cities, the City draws after them), and those draws cannot exhaust.
    fn block_order_commutes(&self, block: Block, context: Context, splays: usize) -> bool {
        let commutes = block.shortcut == Some(Shortcut::AnyOrder)
            && (context.suppress_karma()
                || self
                    .position
                    .dependencies_clear(&[ShortcutDependency::Splay], context, false))
            && self.city_draws_cannot_exhaust(context.executor, splays);
        #[cfg(test)]
        let commutes = commutes && !self.shortcuts_disabled;
        commutes
    }

    fn item_value(&self, item: Item) -> u8 {
        match item {
            Item::Value(value) => value,
            Item::Card(card) => self.position.value(card.inner),
            Item::Pass | Item::Color(_) => 0,
        }
    }

    /// A frozen domain's items, in printed order (values ascending).
    fn block_items(
        &mut self,
        block: Block,
        context: Context,
        regs: Regs,
    ) -> Result<Vec<Item>, Error> {
        Ok(match block.domain {
            Domain::Once => vec![Item::Pass],
            Domain::Times(count) => {
                vec![Item::Pass; usize::from(self.eval_count(count, context, regs)?)]
            }
            Domain::Range(low, high) => (low..=high).map(Item::Value).collect(),
            Domain::Colors(set) => self.colors(set, context, regs)?.map(Item::Color).collect(),
            Domain::Values(from, multiplicity) => {
                let mut counts = [0_u16; 16];
                for target in self.select(*from, context, regs, Excluded::default())? {
                    counts[usize::from(self.selected_value(*from, target.card, context, regs))] +=
                        1;
                }
                let mut values = Vec::new();
                for (value, &count) in counts.iter().enumerate() {
                    values.extend(std::iter::repeat_n(
                        Item::Value(value as u8),
                        multiplicity.times(count),
                    ));
                }
                values
            }
            Domain::Bonuses(bonuses, multiplicity) => {
                let counts = self.bonus_counts(*bonuses, context, regs)?;
                let mut values = Vec::new();
                for (value, &count) in counts.iter().enumerate() {
                    values.extend(std::iter::repeat_n(
                        Item::Value(value as u8),
                        multiplicity.times(count),
                    ));
                }
                values
            }
            Domain::Cards(Selector::Slot(slot)) => self
                .slot_cards(*slot, context, regs)
                .iter()
                .map(|&card| Item::Card(card))
                .collect(),
            Domain::Cards(from) => {
                let targets = self.select(*from, context, regs, Excluded::default())?;
                targets
                    .into_iter()
                    .map(|target| Item::Card(self.capture(target.card)))
                    .collect()
            }
            Domain::Live { .. } => Vec::new(),
        })
    }

    /// Pass and block ends at `pc`. Finishing a block leaves its result in
    /// `regs.last` and continues after it (perhaps ending an outer pass).
    pub(super) fn block_boundary(
        &mut self,
        source: Source,
        pc: &mut u16,
        context: Context,
        regs: &mut Regs,
    ) -> Result<Boundary, Error> {
        loop {
            let Some(active) = self.innermost() else {
                return Ok(Boundary::Continue);
            };
            let end = active.end();
            // Declining an optional branch ends the whole effect.
            if *pc < end || *pc == u16::MAX {
                return Ok(Boundary::Continue);
            }
            if *pc > end {
                return Err(Error::Unsupported("a jump out of a block"));
            }
            let (block, start) = (active.block, active.start);
            match self.next_items(block, context, *regs)? {
                Next::Item(item) => {
                    self.begin_pass(item);
                    *pc = start;
                    return Ok(Boundary::Continue);
                }
                Next::Choose(player, options, tasks) => {
                    // The body starts once the chooser picks its item.
                    self.frames.push(Frame::Program {
                        source,
                        pc: start,
                        context,
                        last: regs.last,
                        retained: regs.bound,
                        chosen: regs.chosen,
                        boxed: regs.boxed,
                    });
                    self.frames.push(Frame::Choose {
                        player,
                        options: DecisionOptions::new(options, context),
                        tasks,
                    });
                    return Ok(Boundary::Suspended);
                }
                Next::Done => {
                    let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
                        unreachable!("a running block's sidecar");
                    };
                    let finished = locals.blocks.pop().expect("innermost block");
                    regs.last = finished.result();
                    locals.last_exact_known = finished.known;
                    // A nested block is one obligation of its outer pass.
                    if let Some(outer) = locals.blocks.last_mut() {
                        outer.known &= finished.known;
                        outer.count(regs.last);
                    }
                }
            }
        }
    }

    /// The innermost block's next item, a choice of it, or the end.
    fn next_items(&mut self, block: Block, context: Context, regs: Regs) -> Result<Next, Error> {
        let chooser = match block.order {
            Order::Chosen(Who::Executor) => Some(context.controller),
            Order::Chosen(who) => Some(self.one_player(who, context)?),
            _ => None,
        };
        let Some(active) = self.innermost() else {
            unreachable!("a running block");
        };
        if active.stopped {
            return Ok(Next::Done);
        }
        if let Domain::Live {
            from,
            per_color,
            stop,
        } = block.domain
        {
            let visited = std::mem::take(&mut active.visited);
            let colors = active.colors;
            let first = visited.is_empty();
            let excluded = Excluded {
                cards: &visited,
                colors: if per_color { colors } else { 0 },
                blind: false,
                skip_junk: false,
            };
            let targets = self.select(*from, context, regs, excluded);
            let Some(active) = self.innermost() else {
                unreachable!("a running block");
            };
            active.visited = visited;
            let targets = targets?;
            if targets.is_empty() {
                return Ok(Next::Done);
            }
            let Some(player) = chooser else {
                let card = targets[0].card;
                return Ok(Next::Item(Item::Card(self.capture(card))));
            };
            let (mut options, mut tasks): (Vec<_>, Vec<_>) = targets
                .into_iter()
                .map(|target| {
                    (
                        Alternative::Select {
                            target: self.position.descriptor(target, player),
                        },
                        Task::BlockCard(target.card),
                    )
                })
                .unzip();
            if stop.may_stop(first, context.must()) {
                options.push(Alternative::Decline);
                tasks.push(Task::BlockItem(None));
            }
            return Ok(Next::Choose(player, options, tasks));
        }
        if active.items.is_empty() {
            return Ok(Next::Done);
        }
        let items = active.items.clone();
        let splays = items.len() * usize::from(active.splays);
        // One option per different item; equal values are one choice.
        let mut distinct: Vec<(usize, Item)> = Vec::new();
        for (index, &item) in items.iter().enumerate() {
            if !distinct.iter().any(|&(_, seen)| seen == item) {
                distinct.push((index, item));
            }
        }
        match chooser {
            Some(player)
                if distinct.len() > 1 && !self.block_order_commutes(block, context, splays) =>
            {
                let mut options = Vec::with_capacity(distinct.len());
                let mut tasks = Vec::with_capacity(distinct.len());
                for (index, item) in distinct {
                    let option = match item {
                        Item::Value(value) => Alternative::Value(value),
                        Item::Color(color) => Alternative::Colors(1 << color as u8),
                        Item::Card(card) => match self.position.locate(card.inner) {
                            Some(physical) => Alternative::Select {
                                target: self.position.descriptor(
                                    Target {
                                        card: card.inner,
                                        physical,
                                        logical: LogicalZone::Physical(physical),
                                    },
                                    player,
                                ),
                            },
                            None => continue,
                        },
                        Item::Pass => Alternative::Branch(index),
                    };
                    if let Alternative::Select {
                        target: TargetDescriptor::Hidden { source, back, .. },
                    } = option
                        && !matches!(source, Zone::Safe(_))
                    {
                        let option = Alternative::BlindSelect { source, back };
                        if let Some(prior) = options.iter().position(|existing| *existing == option)
                        {
                            let Task::BlockBlind(indices) = &mut tasks[prior] else {
                                unreachable!("blind block group")
                            };
                            indices.push(index);
                        } else {
                            options.push(option);
                            tasks.push(Task::BlockBlind(vec![index]));
                        }
                        continue;
                    }
                    options.push(option);
                    tasks.push(Task::BlockItem(Some(index)));
                }
                // Hidden slot order must never set the public order of backs.
                let mut choices: Vec<_> = options.into_iter().zip(tasks).collect();
                choices.sort_by_key(|(option, _)| groups::blind_order(option));
                let (options, tasks) = choices.into_iter().unzip();
                Ok(Next::Choose(player, options, tasks))
            }
            _ => {
                let Some(active) = self.innermost() else {
                    unreachable!("a running block");
                };
                Ok(Next::Item(active.items.remove(0)))
            }
        }
    }

    /// Start a pass on `item`: record it in the block's item slot.
    fn begin_pass(&mut self, item: Item) {
        let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
            unreachable!("a running block's sidecar");
        };
        let active = locals.blocks.last_mut().expect("innermost block");
        if let Item::Card(card) = item {
            active.visited.push(card);
            if let Domain::Live {
                per_color: true, ..
            } = active.block.domain
            {
                active.colors |= 1 << self.position.card(card.inner).color as u8;
            }
        }
        let Some(slot) = active.block.item else {
            return;
        };
        let held = match item {
            Item::Pass => return,
            Item::Value(value) => Held::Value(value),
            Item::Color(color) => Held::Colors(1 << color as u8),
            Item::Card(card) => Held::Cards(vec![card].into()),
        };
        locals.slots[usize::from(slot.index())] = Some(held);
        locals.scalar_known[usize::from(slot.index())] =
            if matches!(item, Item::Value(_) | Item::Color(_)) {
                active.known
            } else {
                3
            };
    }

    /// The chooser picked the next item (`None`: stop a live block). The
    /// program frame is on top, its sidecar beneath.
    pub(super) fn choose_block_item(&mut self, index: Option<usize>) {
        let [.., Frame::Locals(locals), Frame::Program { pc, .. }] = self.frames.as_mut_slice()
        else {
            unreachable!("block item chooser");
        };
        let active = locals.blocks.last_mut().expect("innermost block");
        match index {
            Some(index) => {
                let item = active.items.remove(index);
                let program = self.frames.pop().expect("program frame");
                self.begin_pass(item);
                self.frames.push(program);
            }
            // Stopping ends the live block: resume at its end.
            None => {
                active.stopped = true;
                *pc = active.end();
            }
        }
    }

    /// The chooser picked a live block's next card.
    pub(super) fn choose_block_card(&mut self, card: CardId) {
        let card = self.capture(card);
        let program = self.frames.pop().expect("program frame");
        self.begin_pass(Item::Card(card));
        self.frames.push(program);
    }
}

/// The next step of a block.
enum Next {
    Item(Item),
    Choose(Player, Vec<Alternative>, Vec<Task>),
    Done,
}

impl Active {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize) {
        use crate::observation::context::{Field as F, RecordKind as K};
        let id = p.add(
            K::Block,
            parent,
            None,
            &[
                (F::Cursor, u64::from(self.start)),
                (F::Performed, u64::from(self.performed)),
                (F::Failed, u64::from(self.failed)),
                (F::Colors, u64::from(self.colors)),
                (F::Stopped, u64::from(self.stopped)),
                (
                    F::Frozen,
                    u64::from(!matches!(self.block.domain, Domain::Live { .. })),
                ),
            ],
        );
        if let Some(slot) = self.block.item {
            p.field(id, F::Slot, u64::from(slot.index()));
        }
        let known = self.known & (1 << p.viewer.index()) != 0;
        p.field(id, F::Known, u64::from(known));
        if known {
            p.field(id, F::Remaining, self.items.len() as u64);
            for (ordinal, item) in self.items.iter().enumerate() {
                let item_id = p.add(K::Item, id, None, &[(F::Ordinal, ordinal as u64)]);
                match *item {
                    Item::Pass => p.field(item_id, F::ScalarKind, 0),
                    Item::Value(value) => p.held(item_id, Some(&Held::Value(value)), 3),
                    Item::Color(color) => p.held(item_id, Some(&Held::Colors(1 << color as u8)), 3),
                    Item::Card(reference) => {
                        p.reference(K::Record, item_id, reference, &[]);
                    }
                }
            }
        }
        for (ordinal, &reference) in self.visited.iter().enumerate() {
            p.reference(
                K::Record,
                id,
                reference,
                &[(F::Ordinal, ordinal as u64), (F::Stage, 1)],
            );
        }
        if let Some(reference) = self.card {
            p.reference(K::Record, id, reference, &[(F::Last, 1)]);
        }
    }
}
