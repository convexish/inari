use super::*;

impl Game {
    pub(super) fn push_action_returns(&mut self) {
        let dancing_girl = self.dancing_girl_boards();
        self.frames.push(Frame::ActionReturns {
            democracy: [0; 2],
            majority: None,
            dancing_girl,
            last: HeldReturn::UNIT,
            memory: None,
        });
    }

    /// The player given sole majority in every icon for this action (Yunus).
    pub(super) fn action_majority(&self) -> Option<Player> {
        self.frames.iter().rev().find_map(|frame| match frame {
            Frame::ActionReturns { majority, .. } => Some(*majority),
            _ => None,
        })?
    }

    pub(super) fn grant_action_majority(&mut self, player: Player) {
        if let Some(majority) = self.frames.iter_mut().rev().find_map(|frame| match frame {
            Frame::ActionReturns { majority, .. } => Some(majority),
            _ => None,
        }) {
            *majority = Some(player);
        }
    }

    /// Attribute committed originals to the nearest printed invocation. A
    /// reaction's return is not Democracy's; a copied Democracy invoked from
    /// that reaction is. All such invocations share the enclosing action scope.
    pub(super) fn note_democracy_return(&mut self, context: Context) -> Result<(), Error> {
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Program {
                    source,
                    context: scope,
                    ..
                } => {
                    let Source::Effect { card, .. } = *source else {
                        return Ok(());
                    };
                    if *scope != context || self.position.card(card).code != "B064" {
                        return Ok(());
                    }
                    break;
                }
                Frame::Karma { .. } | Frame::ActionReturns { .. } => return Ok(()),
                _ => {}
            }
        }
        if let Some(democracy) = self.frames.iter_mut().rev().find_map(|frame| match frame {
            Frame::ActionReturns { democracy, .. } => Some(democracy),
            _ => None,
        }) {
            let count = &mut democracy[context.executor.index()];
            *count = count
                .checked_add(1)
                .ok_or(Error::Unsupported("action return count overflow"))?;
        }
        Ok(())
    }

    /// The boards Dancing Girl lies on as an action begins: it "has been on"
    /// them during that action.
    fn dancing_girl_boards(&self) -> u8 {
        match self
            .position
            .dancing_girl
            .map(|card| self.position.locate(card))
        {
            Some(Some(Zone::Board(player, _))) => 1 << player.index(),
            _ => 0,
        }
    }

    /// A card arrived on this player's board: Dancing Girl's action scope
    /// remembers it, whatever moved it.
    pub(super) fn note_board_arrival(&mut self, card: CardId, player: Player) {
        if self.position.dancing_girl != Some(card) {
            return;
        }
        if let Some(boards) = self.frames.iter_mut().rev().find_map(|frame| match frame {
            Frame::ActionReturns { dancing_girl, .. } => Some(dancing_girl),
            _ => None,
        }) {
            *boards |= 1 << player.index();
        }
    }

    pub(super) fn dancing_girl_visited(&self, player: Player) -> bool {
        self.frames.iter().rev().any(|frame| {
            matches!(frame, Frame::ActionReturns { dancing_girl, .. }
                if dancing_girl & 1 << player.index() != 0)
        })
    }

    pub(super) fn democracy_leader(&self, player: Player) -> bool {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame {
                Frame::ActionReturns { democracy, .. } => {
                    Some(democracy[player.index()] > democracy[player.opponent().index()])
                }
                _ => None,
            })
            .unwrap_or(false)
    }
}
