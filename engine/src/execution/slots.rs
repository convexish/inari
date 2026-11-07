//! Card slots: a card's named memories. An
//! instruction with `.keep(slot)` or `.add(slot)` records what it actually
//! did; operands read the slot back. Effect slots live beside their program;
//! execution and action slots in the execution's or action's frame, one
//! record per card, slot and player. A program that never records anything
//! allocates nothing.
use super::*;

/// Card references, their original board owners and the values their operations showed.
/// The uncommon origin column is allocated only when a recorded operation moves a board
/// card, the value column only when a recorded card's value depends on where it lies.
/// Both stay aligned with the references across `.keep` and `.add`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Records {
    cards: Vec<HeldCard>,
    origins: Vec<Option<Player>>,
    values: Vec<Option<u8>>,
}

impl From<Vec<HeldCard>> for Records {
    fn from(cards: Vec<HeldCard>) -> Self {
        Self {
            cards,
            origins: Vec::new(),
            values: Vec::new(),
        }
    }
}

impl Records {
    pub(super) fn as_slice(&self) -> &[HeldCard] {
        &self.cards
    }

    fn push(&mut self, card: HeldCard, origin: Option<Player>, shown: Option<u8>) {
        if origin.is_some() && self.origins.is_empty() {
            self.origins.resize(self.cards.len(), None);
        }
        if origin.is_some() || !self.origins.is_empty() {
            self.origins.push(origin);
        }
        if shown.is_some() && self.values.is_empty() {
            self.values.resize(self.cards.len(), None);
        }
        if shown.is_some() || !self.values.is_empty() {
            self.values.push(shown);
        }
        self.cards.push(card);
    }

    fn extend(&mut self, more: Self) {
        for (index, card) in more.cards.into_iter().enumerate() {
            let origin = more.origins.get(index).copied().flatten();
            let shown = more.values.get(index).copied().flatten();
            self.push(card, origin, shown);
        }
    }

    /// The value this card's latest recorded operation showed, if it is one
    /// whose value depends on where it lies.
    fn shown_value(&self, card: CardId) -> Option<u8> {
        if self.values.is_empty() {
            return None;
        }
        self.cards
            .iter()
            .zip(&self.values)
            .rev()
            .find(|(held, _)| held.inner == card)
            .and_then(|(_, &shown)| shown)
    }
}

/// What a slot holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Held {
    /// Retained references, in the order they were recorded.
    Cards(Records),
    Value(u8),
    Colors(u8),
    Icon(Icon),
}

/// An instruction's record, collected until its program resumes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Pending {
    keep: Keep,
    /// A choice records its chosen parameter; anything else, its cards.
    choice: bool,
    /// Cards a batch or counted draw performed, one original at a time.
    records: Records,
    /// Physical board of the latest successful original move, staged until
    /// its ordinary result is recorded. Reactions never write this field.
    origin: Option<Player>,
}

/// One effect invocation's slots and running blocks: a sidecar frame
/// directly beneath its [`Frame::Program`], pushed the first time the
/// program records anything or enters a block. A sidecar is always its
/// program's neighbor, so the program that owns one is the frame directly
/// above it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Locals {
    pub(super) slots: [Option<Held>; Slot::COUNT],
    /// Observers who know a scalar (including an undefined result). Card
    /// records retain their own evidence instead. PUBLIC is the common case.
    pub(super) scalar_known: [u8; Slot::COUNT],
    pub(super) progress_known: u8,
    pub(super) flow_known: u8,
    pub(super) last_exact_known: u8,
    pending: Option<Pending>,
    /// Running blocks, innermost last.
    pub(super) blocks: Vec<blocks::Active>,
}

/// Execution or action slots: the record of each card's slot per player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Memory {
    flow_known: u8,
    entries: Vec<(CardId, u8, Player, Held, Evidence)>,
    /// Only nonpublic scalar knowledge needs storage; keys may have no value.
    scalar_known: Vec<(CardId, u8, Player, u8, Evidence)>,
}

impl Default for Locals {
    fn default() -> Self {
        Self {
            slots: Default::default(),
            scalar_known: [3; Slot::COUNT],
            progress_known: 3,
            flow_known: 3,
            last_exact_known: 3,
            pending: None,
            blocks: Vec::new(),
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            flow_known: 3,
            entries: Vec::new(),
            scalar_known: Vec::new(),
        }
    }
}
impl Memory {
    fn get(&self, card: CardId, index: u8, player: Player) -> Option<&Held> {
        self.entries
            .iter()
            .find(|entry| (entry.0, entry.1, entry.2) == (card, index, player))
            .map(|entry| &entry.3)
    }

    fn record(
        &mut self,
        card: CardId,
        index: u8,
        player: Player,
        append: bool,
        value: Option<Held>,
        evidence: Evidence,
    ) {
        let position = self
            .entries
            .iter()
            .position(|entry| (entry.0, entry.1, entry.2) == (card, index, player));
        let mut held = position.map(|position| self.entries.remove(position).3);
        record(&mut held, append, value);
        if let Some(held) = held {
            self.entries.push((card, index, player, held, evidence));
        }
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        map.scalar(&mut self.flow_known);
        if map.rules_only() {
            self.scalar_known.clear();
        }
        for (card, _, _, known, evidence) in &mut self.scalar_known {
            map.memory_source(*card, evidence);
            map.scalar(known);
        }
        for (source, _, _, held, evidence) in &mut self.entries {
            map.memory_source(*source, evidence);
            if let Held::Cards(cards) = held {
                for card in &mut cards.cards {
                    map.held(card);
                }
            }
        }
    }
}

/// Store a record: `keep` replaces the slot, `add` appends cards.
fn record(slot: &mut Option<Held>, append: bool, value: Option<Held>) {
    match (append, slot, value) {
        (true, Some(Held::Cards(cards)), Some(Held::Cards(more))) => cards.extend(more),
        (true, _, None) => {}
        (_, slot, value) => *slot = value,
    }
}

impl Locals {
    fn held(&self, slot: Slot) -> Option<&Held> {
        self.slots[usize::from(slot.index())].as_ref()
    }

    pub(super) fn remap(&mut self, map: &tracking::remap::Remap<'_>) {
        let Self {
            slots,
            scalar_known,
            progress_known,
            flow_known,
            last_exact_known,
            pending,
            blocks,
        } = self;
        for known in scalar_known {
            map.scalar(known);
        }
        map.scalar(progress_known);
        map.scalar(flow_known);
        map.scalar(last_exact_known);
        let pending = pending
            .iter_mut()
            .flat_map(|pending| &mut pending.records.cards);
        let held = slots.iter_mut().flat_map(|slot| match slot {
            Some(Held::Cards(cards)) => cards.cards.as_mut_slice(),
            _ => &mut [],
        });
        for card in held.chain(pending) {
            map.held(card);
        }
        for block in blocks {
            block.remap(map);
        }
    }
}

impl Game {
    /// A conditional verification sees what its recording instruction has
    /// actually performed so far. This projection never yields: restore the
    /// complete owning frame before propagating either success or an error.
    pub(super) fn verification_condition(
        &mut self,
        condition: Cond,
        context: Context,
        regs: Regs,
    ) -> Result<bool, Error> {
        let Some((keep, records)) = self.locals().and_then(|locals| {
            locals
                .pending
                .as_ref()
                .map(|pending| (pending.keep, pending.records.clone()))
        }) else {
            return self.holds(condition, context, regs);
        };
        let owner = self
            .frames
            .iter()
            .rposition(|frame| {
                matches!(
                    (keep.slot.life(), frame),
                    (Life::Effect, Frame::Locals(_))
                        | (Life::Execution, Frame::Dogma(_) | Frame::Execute(_))
                        | (Life::Action, Frame::ActionReturns { .. })
                )
            })
            .ok_or(Error::Unsupported("verification outside slot scope"))?;
        let saved = self.frames[owner].clone();
        let result = (|| {
            if keep.slot.life() == Life::Effect {
                let Frame::Locals(locals) = &mut self.frames[owner] else {
                    unreachable!()
                };
                record(
                    &mut locals.slots[usize::from(keep.slot.index())],
                    keep.append,
                    Some(Held::Cards(records)),
                );
            } else {
                self.store(keep, Some(Held::Cards(records)), context)?;
            }
            self.holds(condition, context, regs)
        })();
        self.frames[owner] = saved;
        result
    }

    /// The executing program's slots: the sidecar on top while its program
    /// frame is popped, beneath the program frame on top, or beneath that
    /// program's batch while its selected task resolves.
    fn locals(&self) -> Option<&Locals> {
        match self.frames.as_slice() {
            [.., Frame::Locals(locals)]
            | [.., Frame::Locals(locals), Frame::Program { .. }]
            | [
                ..,
                Frame::Locals(locals),
                Frame::Program { .. },
                Frame::MoveAll(_),
            ] => Some(locals),
            _ => None,
        }
    }

    /// The memory of the execution or action the program runs in: the
    /// nearest Dogma or copied execution, or the nearest action.
    fn memory(&self, life: Life) -> Option<&Memory> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match (life, frame) {
                (Life::Execution, Frame::Dogma(dogma)) => {
                    Some(dogma.extra.as_ref().map(|extra| &extra.memory))
                }
                (Life::Execution, Frame::Execute(execution)) => Some(execution.memory()),
                (Life::Action, Frame::ActionReturns { memory, .. }) => Some(memory.as_deref()),
                _ => None,
            })?
    }

    fn memory_mut(&mut self, life: Life) -> Result<&mut Memory, Error> {
        self.frames
            .iter_mut()
            .rev()
            .find_map(|frame| match (life, frame) {
                (Life::Execution, Frame::Dogma(dogma)) => {
                    Some(&mut dogma.extra.get_or_insert_with(Box::default).memory)
                }
                (Life::Execution, Frame::Execute(execution)) => Some(execution.memory_mut()),
                (Life::Action, Frame::ActionReturns { memory, .. }) => {
                    Some(memory.get_or_insert_with(Box::default).as_mut())
                }
                _ => None,
            })
            .ok_or(Error::Unsupported("a slot outside its execution or action"))
    }

    fn held_slot(&self, slot: Slot, context: Context, regs: Regs) -> Option<&Held> {
        match slot.life() {
            Life::Effect => self
                .locals()
                .filter(|_| regs.locals)
                .and_then(|locals| locals.held(slot)),
            Life::Execution | Life::Action => {
                let card = context.source_card?;
                let player = self.one_player(slot.owner(), context).ok()?;
                self.memory(slot.life())?.get(card, slot.index(), player)
            }
        }
    }

    pub(super) fn flow_known(&self, regs: Regs) -> u8 {
        self.locals()
            .filter(|_| regs.locals)
            .map_or(3, |locals| locals.flow_known)
    }
    pub(super) fn last_known(&self, regs: Regs) -> u8 {
        self.locals()
            .filter(|_| regs.locals)
            .map_or(3, |locals| locals.last_exact_known)
    }
    /// A private branch restricts this program's sidecar, and an enclosing
    /// execution or action memory only if the program could change that
    /// memory's records. Records are keyed by their source card, so a
    /// program that neither records into such a slot nor starts another
    /// program cannot reveal its branch through them.
    pub(super) fn retain_private_flow(&mut self, known: u8, source: Source, regs: &mut Regs) {
        if known == 3 {
            return;
        }
        self.own_locals().flow_known &= known;
        regs.locals = true;
        for life in [Life::Execution, Life::Action] {
            let present = self.frames.iter().any(|frame| {
                matches!(
                    (life, frame),
                    (Life::Execution, Frame::Dogma(_) | Frame::Execute(_))
                        | (Life::Action, Frame::ActionReturns { .. })
                )
            });
            let records = self.program(source).is_none_or(|program| {
                program
                    .iter()
                    .any(|instruction| instruction.may_record(life))
            });
            if present && records {
                self.memory_mut(life).expect("matching scope").flow_known &= known;
            }
        }
    }
    pub(super) fn continuation_flow_known(&self, viewer: Player) -> bool {
        fn memory_known(game: &Game, memory: &Memory, viewer: Player) -> bool {
            memory.flow_known & (1 << viewer.index()) != 0
                && memory
                    .scalar_known
                    .iter()
                    .all(|&(card, _, _, known, evidence)| {
                        known & (1 << viewer.index()) != 0
                            && game.reference_visible(Captured::new(card, evidence), viewer)
                    })
                && memory.entries.iter().all(|&(card, _, _, _, evidence)| {
                    game.reference_visible(Captured::new(card, evidence), viewer)
                })
        }
        fn frame_known(game: &Game, frame: &Frame, viewer: Player) -> bool {
            let bit = 1 << viewer.index();
            match frame {
                Frame::Locals(locals) => {
                    (locals.flow_known
                        & locals.last_exact_known
                        & locals.progress_known
                        & locals
                            .scalar_known
                            .iter()
                            .copied()
                            .fold(3, |known, slot| known & slot)
                        & locals
                            .blocks
                            .iter()
                            .fold(3, |known, block| known & block.known()))
                        & bit
                        != 0
                }
                Frame::ActionReturns {
                    memory: Some(memory),
                    ..
                } => memory_known(game, memory, viewer),
                Frame::Dogma(dogma) => dogma
                    .extra
                    .as_ref()
                    .is_none_or(|extra| memory_known(game, &extra.memory, viewer)),
                Frame::Execute(execution) => {
                    game.reference_visible(execution.card(), viewer)
                        && execution
                            .memory()
                            .is_none_or(|memory| memory_known(game, memory, viewer))
                }
                Frame::Program { context, .. } => context.source_card.is_none_or(|card| {
                    game.reference_visible(Captured::new(card, context.source_evidence), viewer)
                }),
                Frame::KarmaProof { saved, .. } => frame_known(game, saved, viewer),
                Frame::RevealAll(packet) => {
                    packet.empty_selection.is_none_or(|known| known & bit != 0)
                }
                _ => true,
            }
        }
        self.frames
            .iter()
            .all(|frame| frame_known(self, frame, viewer))
    }

    pub(super) fn slot_scalar_known(&self, slot: Slot, context: Context, regs: Regs) -> u8 {
        match slot.life() {
            Life::Effect => self
                .locals()
                .filter(|_| regs.locals)
                .map_or(3, |locals| locals.scalar_known[usize::from(slot.index())]),
            Life::Execution | Life::Action => {
                let Some(card) = context.source_card else {
                    return 3;
                };
                let Ok(player) = self.one_player(slot.owner(), context) else {
                    return 3;
                };
                self.memory(slot.life())
                    .and_then(|memory| {
                        memory.scalar_known.iter().find(|entry| {
                            (entry.0, entry.1, entry.2) == (card, slot.index(), player)
                        })
                    })
                    .map_or(3, |entry| entry.3)
            }
        }
    }

    pub(super) fn store_scalar(
        &mut self,
        keep: Keep,
        value: Option<Held>,
        context: Context,
        known: u8,
    ) -> Result<(), Error> {
        // An unsuccessful `.add` leaves both the value and its provenance intact.
        if keep.append && value.is_none() {
            return Ok(());
        }
        self.store(keep, value, context)?;
        match keep.slot.life() {
            Life::Effect => {
                let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
                    unreachable!("scalar owner")
                };
                locals.scalar_known[usize::from(keep.slot.index())] = known;
            }
            life => {
                let card = context
                    .source_card
                    .ok_or(Error::Unsupported("a slot without a card"))?;
                let writers = record_owners(keep.slot, context);
                let memory = self.memory_mut(life)?;
                for &player in writers {
                    let key = (card, keep.slot.index(), player);
                    memory
                        .scalar_known
                        .retain(|entry| (entry.0, entry.1, entry.2) != key);
                    if known != 3 {
                        memory.scalar_known.push((
                            key.0,
                            key.1,
                            key.2,
                            known,
                            context.source_evidence,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// The latest card a slot records.
    #[cold]
    #[inline(never)]
    pub(super) fn slot_register(
        &self,
        slot: Slot,
        context: Context,
        regs: Regs,
    ) -> Option<HeldCard> {
        self.slot_cards(slot, context, regs).last().copied()
    }

    /// The cards a slot records, oldest first.
    #[inline(never)]
    pub(super) fn slot_cards(&self, slot: Slot, context: Context, regs: Regs) -> &[HeldCard] {
        match self.held_slot(slot, context, regs) {
            Some(Held::Cards(cards)) => &cards.cards,
            _ => &[],
        }
    }

    /// Original board owner of the latest successful card in the slot.
    pub(super) fn slot_origin(&self, slot: Slot, context: Context, regs: Regs) -> Option<Player> {
        match self.held_slot(slot, context, regs) {
            Some(Held::Cards(records)) => records.origins.last().copied().flatten(),
            _ => None,
        }
    }

    #[inline(never)]
    pub(super) fn slot_value(&self, slot: Slot, context: Context, regs: Regs) -> Option<u8> {
        match self.held_slot(slot, context, regs) {
            Some(&Held::Value(value)) => Some(value),
            _ => None,
        }
    }

    /// The value a slot's recorded operation on this card showed, when that
    /// differs from reading the card where it lies now.
    pub(super) fn slot_shown_value(
        &self,
        slot: Slot,
        card: CardId,
        context: Context,
        regs: Regs,
    ) -> Option<u8> {
        match self.held_slot(slot, context, regs) {
            Some(Held::Cards(records)) => records.shown_value(card),
            _ => None,
        }
    }

    /// The value an operation on this card showed: its face value if the
    /// card was face up before (`before`, the pre-move value) or after the
    /// operation, otherwise its back. `None` for every card whose value
    /// does not depend on where it lies, so ordinary records allocate
    /// nothing.
    pub(super) fn shown_value(&self, card: CardId, before: Option<u8>) -> Option<u8> {
        if !self.position.expansions().artifacts() {
            return None;
        }
        let printed = self.position.card(card);
        (printed.face_value() != printed.value)
            .then(|| before.unwrap_or(0).max(self.position.value(card)))
    }

    /// The value the previous instruction's single successful operation
    /// showed, before any movement.
    pub(super) fn last_value(regs: Regs) -> Option<u8> {
        match regs.last.inner {
            ReturnValue::Operation(outcome) => outcome.value,
            ReturnValue::Bulk(outcome) => outcome.value,
            _ => None,
        }
    }

    #[inline(never)]
    pub(super) fn slot_colors(&self, slot: Slot, context: Context, regs: Regs) -> u8 {
        match self.held_slot(slot, context, regs) {
            Some(&Held::Colors(colors)) => colors,
            _ => 0,
        }
    }

    #[inline(never)]
    pub(super) fn slot_empty(&self, slot: Slot, context: Context, regs: Regs) -> bool {
        match self.held_slot(slot, context, regs) {
            None => true,
            Some(Held::Cards(cards)) => cards.cards.is_empty(),
            Some(Held::Colors(colors)) => *colors == 0,
            Some(Held::Value(_) | Held::Icon(_)) => false,
        }
    }

    /// `Repeat` empties the effect slots outside `keep` and any running
    /// block; the program frame is popped, its sidecar on top.
    pub(super) fn restart_locals(&mut self, keep: u8) {
        let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
            unreachable!("a program's sidecar");
        };
        if keep == 0 && locals.flow_known == 3 {
            self.frames.pop();
            return;
        }
        locals.pending = None;
        locals.progress_known = 3;
        locals.last_exact_known = 3;
        locals.blocks.clear();
        for (index, slot) in locals.slots.iter_mut().enumerate() {
            if keep & (1 << index) == 0 {
                *slot = None;
                locals.scalar_known[index] = 3;
            }
        }
    }

    /// The popped program's sidecar, pushed now if it has none yet.
    pub(super) fn own_locals(&mut self) -> &mut Locals {
        if !matches!(self.frames.last(), Some(Frame::Locals(_))) {
            self.frames.push(Frame::Locals(Box::default()));
        }
        let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
            unreachable!("pushed above");
        };
        locals
    }

    /// Start recording an instruction into a slot. Its program frame was
    /// just pushed back; a sidecar goes beneath it if it has none yet.
    #[cold]
    #[inline(never)]
    pub(super) fn begin_keep(
        &mut self,
        keep: Keep,
        choice: bool,
        regs: &mut Regs,
    ) -> Result<(), Error> {
        // A keep writes the executor's record, or every player's (Either).
        if !matches!(keep.slot.owner(), Who::Executor | Who::Either) {
            return Err(Error::Unsupported("a slot records its executor's record"));
        }
        let program = self.frames.len() - 1;
        if !matches!(
            self.frames.get(program.wrapping_sub(1)),
            Some(Frame::Locals(_))
        ) {
            self.frames.insert(program, Frame::Locals(Box::default()));
        }
        let [.., Frame::Locals(locals), Frame::Program { chosen, .. }] = self.frames.as_mut_slice()
        else {
            unreachable!("a recording program's frame");
        };
        if choice {
            // Nothing chosen records nothing, not an earlier choice.
            *chosen = SelectionParameter::None;
            regs.chosen = SelectionParameter::None;
        }
        locals.pending = Some(Pending {
            keep,
            choice,
            records: Records::default(),
            origin: None,
        });
        regs.locals = true;
        Ok(())
    }

    /// A batch or counted draw performed an original: record it in the
    /// in-flight instruction of the program directly beneath it, if that
    /// instruction records. Other batches (karma, rules) sit on other frames.
    pub(super) fn record_kept(&mut self, card: HeldCard, before: Option<u8>) {
        let shown = self.shown_value(card.inner, before);
        if let [.., Frame::Locals(locals), Frame::Program { .. }] = self.frames.as_mut_slice()
            && let Some(pending) = &mut locals.pending
            && !pending.choice
        {
            pending.records.push(card, pending.origin.take(), shown);
        }
    }

    /// Called only after a successful original move commits. Only the
    /// instruction's direct operation or batch may supply its origin;
    /// a native karma or a nested program cannot borrow the outer pending
    /// record. Origins are public player identities, independent of hidden
    /// reference remapping and the copied program's source owner.
    pub(super) fn record_kept_origin(&mut self, origin: Player) {
        let frames = match self.frames.last() {
            Some(Frame::MoveAll(_) | Frame::MatchingPair(_)) => {
                let len = self.frames.len() - 1;
                &mut self.frames[..len]
            }
            _ => self.frames.as_mut_slice(),
        };
        if let [.., Frame::Locals(locals), Frame::Program { .. }] = frames
            && let Some(pending) = &mut locals.pending
            && !pending.choice
        {
            pending.origin = Some(origin);
        }
    }

    /// The program resumes after its recording instruction: store what it
    /// did. A single operation's card is its result; a batch recorded each
    /// performed original as it happened. Execution and action slots record
    /// for the executing player, under the executing card.
    pub(super) fn commit_kept(&mut self, context: Context, regs: Regs) -> Result<(), Error> {
        let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
            return Ok(());
        };
        let Some(Pending {
            keep,
            choice,
            mut records,
            origin,
        }) = locals.pending.take()
        else {
            return Ok(());
        };
        let value = if choice && !records.cards.is_empty() {
            Some(Held::Cards(records))
        } else if choice {
            match regs.chosen {
                SelectionParameter::None => None,
                SelectionParameter::Value(value) => Some(Held::Value(value)),
                SelectionParameter::Count(count) => {
                    Some(Held::Value(u8::try_from(count).unwrap_or(u8::MAX)))
                }
                SelectionParameter::Colors(colors) => Some(Held::Colors(colors)),
                SelectionParameter::Icon(icon) => Some(Held::Icon(icon)),
                SelectionParameter::Card(card) => Some(Held::Cards(vec![card].into())),
            }
        } else {
            if records.cards.is_empty()
                && matches!(regs.last.inner, ReturnValue::Operation(_))
                && let Some(card) = regs.last_card()
            {
                let shown = self.shown_value(card.inner, Self::last_value(regs));
                records.push(card, origin, shown);
            }
            Some(Held::Cards(records))
        };
        // Performed original moves/draws have public cardinality; their
        // identities keep each record's own evidence. The requested private
        // quota retires with the instruction, not with these result cards.
        self.store(keep, value, context)
    }

    /// A multi-card choice records the whole committed set, including empty
    /// selection, without counting the selection as a card operation.
    pub(super) fn record_chosen_group(&mut self, cards: Vec<HeldCard>) {
        if let [.., Frame::Locals(locals), Frame::Program { .. }] = self.frames.as_mut_slice()
            && let Some(pending) = &mut locals.pending
            && pending.choice
        {
            pending.records = cards.into();
        }
    }

    /// Write a record into its slot.
    pub(super) fn store(
        &mut self,
        keep: Keep,
        value: Option<Held>,
        context: Context,
    ) -> Result<(), Error> {
        let index = keep.slot.index();
        let writes = !keep.append || value.is_some();
        match keep.slot.life() {
            Life::Effect => {
                let Some(Frame::Locals(locals)) = self.frames.last_mut() else {
                    unreachable!("a recording program's sidecar");
                };
                record(&mut locals.slots[usize::from(index)], keep.append, value);
                if writes {
                    locals.scalar_known[usize::from(index)] = 3;
                }
            }
            life @ (Life::Execution | Life::Action) => {
                let card = context
                    .source_card
                    .ok_or(Error::Unsupported("a slot without a card"))?;
                let writers = record_owners(keep.slot, context);
                let memory = self.memory_mut(life)?;
                for &player in writers {
                    memory.record(
                        card,
                        index,
                        player,
                        keep.append,
                        value.clone(),
                        context.source_evidence,
                    );
                    if writes {
                        memory
                            .scalar_known
                            .retain(|entry| (entry.0, entry.1, entry.2) != (card, index, player));
                    }
                }
            }
        }
        Ok(())
    }
}

/// The records a keep writes: the executing player's, or every player's
/// for a slot kept `of(Who::Either)`, a history the whole action shares
/// (Brethren of Purity's last meld). Reads name one player as usual.
fn record_owners(slot: Slot, context: Context) -> &'static [Player] {
    if slot.owner() == Who::Either {
        &Player::ALL
    } else {
        match context.executor {
            Player::One => &Player::ALL[..1],
            Player::Two => &Player::ALL[1..],
        }
    }
}

impl Records {
    pub(super) fn observed(&self) -> impl Iterator<Item = (HeldCard, Option<Player>)> + '_ {
        self.cards
            .iter()
            .copied()
            .enumerate()
            .map(|(index, card)| (card, self.origins.get(index).copied().flatten()))
    }
}
impl Memory {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize, life: Life) {
        let mut keys: Vec<_> = self
            .entries
            .iter()
            .map(|&(card, index, owner, _, _)| (card, index, owner))
            .chain(
                self.scalar_known
                    .iter()
                    .map(|&(card, index, owner, _, _)| (card, index, owner)),
            )
            .collect();
        keys.sort_by_key(|&(card, index, owner)| {
            (p.game.position.card(card).stable_id(), owner.index(), index)
        });
        keys.dedup();
        for (card, index, owner) in keys {
            let held = self.get(card, index, owner);
            let known = self
                .scalar_known
                .iter()
                .find(|entry| (entry.0, entry.1, entry.2) == (card, index, owner))
                .map_or(3, |entry| entry.3);
            p.slot(
                parent,
                (life, index),
                owner,
                Some(Captured::new(
                    card,
                    self.entries
                        .iter()
                        .find(|entry| (entry.0, entry.1, entry.2) == (card, index, owner))
                        .map(|entry| entry.4)
                        .or_else(|| {
                            self.scalar_known
                                .iter()
                                .find(|entry| (entry.0, entry.1, entry.2) == (card, index, owner))
                                .map(|entry| entry.4)
                        })
                        .expect("retained source evidence"),
                )),
                held,
                known,
            );
        }
    }
}
impl Locals {
    pub(super) fn project(&self, p: &mut observe::Projection<'_>, parent: usize, context: Context) {
        use crate::observation::context::{Field as F, RecordKind as K};
        for (index, slot) in self.slots.iter().enumerate() {
            p.slot(
                parent,
                (Life::Effect, index as u8),
                context.executor,
                None,
                slot.as_ref(),
                self.scalar_known[index],
            );
        }
        if let Some(pending) = &self.pending {
            let id = p.add(
                K::Pending,
                parent,
                None,
                &[
                    (F::Life, pending.keep.slot.life() as u64),
                    (F::Slot, u64::from(pending.keep.slot.index())),
                    (F::Owner, p.player(context.executor)),
                    (F::Append, u64::from(pending.keep.append)),
                    (F::Choice, u64::from(pending.choice)),
                ],
            );
            p.records(id, &pending.records);
        }
        for block in &self.blocks {
            block.project(p, parent);
        }
    }
}
