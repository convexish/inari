//! Resolve copied directions before announcing an ordinary splay operation.
use super::*;

impl Game {
    pub(super) fn splay_operations(
        &self,
        step: SplayStep,
        context: Context,
        regs: Regs,
    ) -> Result<(Vec<Operation>, bool), Error> {
        let fixed;
        let options = match step.choices {
            SplayChoices::Fixed {
                colors,
                direction,
                from,
            } => {
                fixed = [SplayOption {
                    colors,
                    direction: SplayDirection::Fixed(direction),
                    from,
                }];
                &fixed[..]
            }
            SplayChoices::Options(options) => options,
        };
        let mut result = Vec::new();
        for player in Player::ALL {
            if !self.player_matches(step.who, context, player) {
                continue;
            }
            for option in options {
                for color in self.colors(option.colors, context, regs)? {
                    if option
                        .from
                        .is_some_and(|from| self.position.splay(player, color) != from)
                    {
                        continue;
                    }
                    let directions = match option.direction {
                        SplayDirection::Fixed(direction) => vec![direction],
                        SplayDirection::Matching(who) => {
                            vec![self.position.splay(self.one_player(who, context)?, color)]
                        }
                        SplayDirection::Copy {
                            who,
                            colors,
                            splayed_only,
                        } => {
                            let mut directions = Vec::new();
                            for owner in Player::ALL {
                                if !self.player_matches(who, context, owner) {
                                    continue;
                                }
                                for source in self.colors(colors, context, regs)? {
                                    let direction = self.position.splay(owner, source);
                                    if !splayed_only || direction != Splay::None {
                                        directions.push(direction);
                                    }
                                }
                            }
                            directions
                        }
                    };
                    for direction in directions {
                        let admitted = match step.targets {
                            SplayTargets::Changes => self.can_splay(player, color, direction),
                            SplayTargets::Nonempty => {
                                !self.position.cards(Zone::Board(player, color)).is_empty()
                            }
                            SplayTargets::AnyColor => true,
                        };
                        if admitted {
                            let operation = Operation::Splay {
                                player,
                                color,
                                direction,
                            };
                            // Equal source directions describe the very same operation.
                            // No source reference is consumed or exposed by copying it.
                            let duplicates = cfg!(test) && self.splay_alias_oracle();
                            if duplicates || !result.contains(&operation) {
                                result.push(operation);
                            }
                        }
                    }
                }
            }
        }
        // Piles that would stay as they are (one card, or already splayed
        // that way) are one no-op whichever the player names: the first
        // stands for all of them, unless a Figure watching splays could tell
        // them apart. The oracle keeps the full menu.
        let before = result.len();
        if before > 1
            && !self.splay_alias_oracle()
            && self
                .position
                .dependencies_clear(&[ShortcutDependency::Splay], context, true)
        {
            let mut first_noop = true;
            result.retain(|operation| match *operation {
                Operation::Splay {
                    player,
                    color,
                    direction,
                } => {
                    self.can_splay(player, color, direction)
                        || std::mem::replace(&mut first_noop, false)
                }
                _ => true,
            });
        }
        let merged = result.len() < before;
        // A mandatory splay whose every offered pile would stay as it is:
        // whichever the player names, nothing changes, so the choice is not
        // asked. An optional one keeps its decline.
        if result.len() > 1
            && !step.optional
            && !self.splay_alias_oracle()
            && result.iter().all(|operation| match *operation {
                Operation::Splay {
                    player,
                    color,
                    direction,
                } => !self.can_splay(player, color, direction),
                _ => false,
            })
        {
            result.truncate(1);
        }
        Ok((result, merged))
    }

    fn splay_alias_oracle(&self) -> bool {
        #[cfg(test)]
        {
            self.shortcuts_disabled
        }
        #[cfg(not(test))]
        {
            false
        }
    }
}
