//! Menus for one step of a batch move.
use super::*;

fn blind_order(option: &Alternative) -> (u8, u8, u8, CardSet) {
    match option {
        Alternative::BlindMove { source, back, .. } => {
            let origin = match source {
                Zone::Score(player) => player.index() as u8,
                Zone::Hand(player) => 2 + player.index() as u8,
                _ => 4,
            };
            (0, origin, back.age, back.set)
        }
        _ => (1, 0, 0, CardSet::Base),
    }
}

fn blind_back(batch: &MoveAll, descriptor: &TargetDescriptor) -> Option<(Zone, Deck)> {
    let TargetDescriptor::Hidden { source, back, .. } = *descriptor else {
        return None;
    };
    // Ordered deck sweeps still preserve explicit deck positions, and a
    // Safe's secrets are ordered public slots. Live hidden unordered piles
    // choose backs, just like frozen groups.
    if matches!(source, Zone::Safe(_)) {
        return None;
    }
    (matches!(
        batch.verb,
        Verb::Return | Verb::Foreshadow | Verb::Safeguard
    ) || batch.frozen.is_some()
        || (matches!(batch.verb, Verb::Score | Verb::Transfer)
            && matches!(
                source,
                Zone::Hand(_)
                    | Zone::Score(_)
                    | Zone::Forecast(_)
                    | Zone::Junk
                    | Zone::Achievements(_)
                    | Zone::AvailableAchievements
            )))
    .then_some((source, back))
}

impl Game {
    /// The reference a batch retains for one candidate.
    pub(super) fn batch_reference(&self, batch: &MoveAll, card: CardId) -> Option<HeldCard> {
        Self::batch_reference_with(batch, card, || {
            self.selector_reference(batch.from, card, batch.context, batch.regs)
        })
    }

    /// The references the batch's selector names, for a whole menu.
    fn batch_named(&self, batch: &MoveAll) -> Vec<HeldCard> {
        self.named_references(batch.from, batch.context, batch.regs)
    }

    fn batch_reference_with(
        batch: &MoveAll,
        card: CardId,
        selector: impl FnOnce() -> Option<HeldCard>,
    ) -> Option<HeldCard> {
        // A disclosure strengthens the same retained pick. An older anonymous
        // selector reference must not hide its new, explicit public evidence.
        find_card(&batch.proofs, card)
            .or_else(|| {
                batch
                    .frozen
                    .as_ref()
                    .and_then(|cards| cards.iter().find(|held| held.card == card))
                    .map(|held| held.with(card))
            })
            .or_else(selector)
            .or_else(|| find_card(&batch.visited, card))
    }

    pub(super) fn batch_descriptor(&self, batch: &MoveAll, target: Target) -> TargetDescriptor {
        let viewer = batch.context.controller;
        let named = self.position.can_view(target.physical, viewer)
            || self
                .batch_reference(batch, target.card)
                .is_some_and(|held| self.reference_visible(held, viewer));
        self.batch_candidate_descriptor(target, viewer, named)
    }

    fn batch_candidate_descriptor(
        &self,
        target: Target,
        viewer: Player,
        named: bool,
    ) -> TargetDescriptor {
        if named || self.position.can_view(target.physical, viewer) {
            TargetDescriptor::Known {
                card: target.card,
                source: target.physical,
            }
        } else {
            self.position.descriptor(target, viewer)
        }
    }

    pub(super) fn batch_blind_back(&self, batch: &MoveAll, target: Target) -> Option<(Zone, Deck)> {
        blind_back(batch, &self.batch_descriptor(batch, target))
    }

    /// Hidden moves choose a public back, then sample with the same native
    /// modulo primitive as Decrees. No hidden identity or slot is a policy input.
    pub(super) fn batch_move_choices(
        &self,
        batch: &MoveAll,
        targets: Vec<Target>,
    ) -> (Vec<Alternative>, Vec<Task>) {
        // Reuse each lookup within this menu construction. Nothing mutates
        // the game before the descriptor and task consume the reference.
        let viewer = batch.context.controller;
        let references = self.batch_named(batch);
        let mut options = Vec::with_capacity(targets.len());
        let mut tasks = Vec::with_capacity(targets.len());
        for target in targets {
            let reference = Self::batch_reference_with(batch, target.card, || {
                find_card(&references, target.card)
            });
            let named = reference.is_some_and(|held| self.reference_visible(held, viewer));
            let descriptor = self.batch_candidate_descriptor(target, viewer, named);
            if let Some((source, back)) = blind_back(batch, &descriptor) {
                let option = Alternative::BlindMove {
                    verb: batch.verb,
                    source,
                    back,
                };
                if let Some(index) = options.iter().position(|prior| *prior == option) {
                    let Task::MoveAllBlind(group) = &mut tasks[index] else {
                        unreachable!("bulk blind return group");
                    };
                    group.targets.push(target);
                } else {
                    // Insert by public back only. Known menu order and
                    // physical order within each RNG group remain intact.
                    let key = blind_order(&option);
                    let index = options.partition_point(|prior| blind_order(prior) <= key);
                    options.insert(index, option);
                    tasks.insert(
                        index,
                        Task::MoveAllBlind(Box::new(BlindChoice {
                            targets: vec![target],
                            // Include named selector members even when the
                            // ordering proof chose their anonymous cohort first.
                            excluded: self
                                .position
                                .cards(source)
                                .iter()
                                .filter_map(|&card| {
                                    Self::batch_reference_with(batch, card, || {
                                        find_card(&references, card)
                                    })
                                    .filter(|held| {
                                        self.position.card(card).origin_deck() == back
                                            && self
                                                .reference_visible(*held, batch.context.controller)
                                    })
                                })
                                .collect(),
                            chooser: batch.context.controller,
                        })),
                    );
                }
                continue;
            }
            options.push(Alternative::Move {
                verb: batch.verb,
                target: descriptor,
            });
            tasks.push(match reference {
                Some(reference) => Task::MoveAllReference(Box::new(reference.with(target))),
                None => Task::MoveAllCard { target },
            });
        }
        let all_known = options.iter().all(|option| {
            matches!(
                option,
                Alternative::Move {
                    target: TargetDescriptor::Known { .. },
                    ..
                }
            )
        });
        if all_known {
            // No blind groups: list the cards the way the app lays piles out
            // (the order an origin floor measures them by, so a choice
            // removes the options before it), or by colour for a colour
            // floor. Stable sorts by one card key keep options and tasks
            // paired without a temporary candidate Vec.
            let descending = matches!(
                batch.shortcut,
                Some(Shortcut::ByOrigin {
                    descending: true,
                    ..
                })
            );
            let key = |card: CardId| {
                let display = self.display_key(card, descending, true);
                if batch.shortcut == Some(Shortcut::ByColor) {
                    ((self.position.card(card).color as u64) << 32) | u64::from(display)
                } else {
                    u64::from(display)
                }
            };
            options.sort_by_key(|option| match option {
                Alternative::Move {
                    target: TargetDescriptor::Known { card, .. },
                    ..
                } => key(*card),
                _ => unreachable!("known batch option"),
            });
            tasks.sort_by_key(|task| match task {
                Task::MoveAllReference(reference) => key(reference.card),
                Task::MoveAllCard { target } => key(target.card),
                _ => unreachable!("known batch task"),
            });
        }
        (options, tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::shortcuts::common::{A, B, Fixture};

    fn fixture() -> (Game, [Target; 4], MoveAll) {
        let mut f = Fixture::default();
        let targets = ["B001", "B002", "B003", "B004"].map(|code| Target {
            card: f.add(base_card(code).unwrap(), Zone::Hand(A)),
            physical: Zone::Hand(A),
            logical: LogicalZone::Physical(Zone::Hand(A)),
        });
        let game = Game::new(f.position(), Expansions::BASE).unwrap();
        let batch = MoveAll::new(
            Verb::Return,
            Selector::Hand(Who::Executor),
            Destination::Deck,
            Context::new(A),
        );
        (game, targets, batch)
    }

    #[test]
    fn known_menus_list_cards_in_display_order_whether_named_or_merely_visible() {
        let (game, targets, mut batch) = fixture();
        for named in [false, true] {
            if named {
                batch.proofs = targets
                    .iter()
                    .map(|target| Captured::new(target.card, Evidence::PUBLIC))
                    .collect();
            }
            let (options, tasks) = game.batch_move_choices(&batch, vec![targets[2], targets[0]]);
            // Display order, not selection order; the task kind still records
            // whether a retained reference names the card.
            let expected = [0, 2];
            for ((option, task), index) in options.iter().zip(&tasks).zip(expected) {
                assert_eq!(
                    *option,
                    Alternative::Move {
                        verb: Verb::Return,
                        target: TargetDescriptor::Known {
                            card: targets[index].card,
                            source: Zone::Hand(A),
                        },
                    }
                );
                let target = match task {
                    Task::MoveAllReference(reference) if named => reference.inner,
                    Task::MoveAllCard { target } if !named => *target,
                    other => panic!("wrong task for named={named}: {other:?}"),
                };
                assert_eq!(target, targets[index]);
            }
            assert_eq!(options.len(), 2);
            assert_eq!(tasks.len(), 2);
        }
    }

    #[test]
    fn mixed_menu_keeps_blind_order_and_excludes_names_outside_its_candidates() {
        let (game, targets, mut batch) = fixture();
        batch.context.controller = B;
        batch.proofs = targets[..2]
            .iter()
            .map(|target| Captured::new(target.card, Evidence::PUBLIC))
            .collect();
        let (options, tasks) =
            game.batch_move_choices(&batch, vec![targets[3], targets[2], targets[0]]);
        assert_eq!(options.len(), 2);
        assert_eq!(tasks.len(), 2);
        assert_eq!(
            options[0],
            Alternative::BlindMove {
                verb: Verb::Return,
                source: Zone::Hand(A),
                back: Deck::base(1),
            }
        );
        let Task::MoveAllBlind(group) = &tasks[0] else {
            panic!("expected blind cohort")
        };
        assert_eq!(group.targets, [targets[3], targets[2]]);
        assert_eq!(group.excluded, batch.proofs);
        assert!(matches!(
            &tasks[1],
            Task::MoveAllReference(reference) if reference.inner == targets[0]
        ));
    }
}
