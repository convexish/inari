use super::*;

impl Game {
    /// Regular actions the turn owner may start now. Empty while a
    /// continuation, decision or terminal result is pending. Achievement
    /// requests name a supply card only so the native request is exact; a
    /// policy should present them by public age, never by hidden front.
    pub fn legal_actions(&self) -> Vec<Action> {
        if self.ensure_idle().is_err()
            || self.position.regular_actions_started >= self.position.regular_action_limit
        {
            return Vec::new();
        }
        let player = self.position.turn_owner;
        let mut actions = Vec::with_capacity(16);
        actions.push(Action::Draw);
        actions.extend(
            self.position
                .action_hand(ActionKind::Regular)
                .chain(self.position.museum_melds(player))
                .map(|target| Action::Meld { card: target.card }),
        );
        // One score and claim value for the whole supply.
        let score = self.position.score(player);
        let claim_value = self.position.highest_claim_value(player);
        actions.extend(
            self.position
                .targets(LogicalZone::AvailableAchievements(player))
                .filter(|target| {
                    self.position
                        .meets_achievement_cost(player, target.card, score, claim_value)
                })
                .map(|target| Action::Achieve { card: target.card }),
        );
        // Own secrets (Unseen) by their public slots, under the same costs.
        actions.extend(
            self.position
                .cards(Zone::Safe(player))
                .iter()
                .filter(|&&card| {
                    self.position
                        .meets_achievement_cost(player, card, score, claim_value)
                })
                .map(|&card| Action::Achieve { card }),
        );
        for owner in [player, player.opponent()] {
            for color in Color::ALL {
                let Some(card) = self.position.top(owner, color) else {
                    continue;
                };
                if self.position.dogma_source_owner(player, card).is_none() {
                    continue;
                }
                actions.push(Action::Dogma {
                    card,
                    auspice: false,
                });
                // With an endorsement offer, Auspice is chosen after it.
                if !self.endorsement_offered(player, card)
                    && self.auspice_offered(
                        player,
                        card,
                        Some(self.position.regular_actions_started() + 1),
                    )
                {
                    actions.push(Action::Dogma {
                        card,
                        auspice: true,
                    });
                }
            }
        }
        if self
            .position
            .decree_action_options(ActionKind::Regular)
            .is_ok_and(|options| !options.is_empty())
        {
            actions.push(Action::Decree);
        }
        actions
    }
}
