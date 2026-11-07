//! Figure clauses that can invalidate a choice reduction
//! ([`crate::program::Shortcut`]). Reductions only remove choices; every
//! semantic operation still runs.
use crate::*;

/// Dependencies that a live Figure can invalidate, derived from its
/// declaration. Each shortcut also needs a local proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutDependency {
    Move(Verb),
    Draw,
    Dogma,
    EffectValues,
    Exchange,
    Splay,
    Fade,
    Win,
    HandMembership,
    ScoreMembership,
    IconQuery,
    ScoreQuery,
    AchievementQuery,
    ClaimAchievement,
    /// A Would/When karma on claims: suppressed karma cannot discover it.
    ClaimReaction,
    SpecialRequirement,
    /// Supply-token junking, not a card move or achievement claim.
    JunkSpecial,
}

impl Figure {
    /// Everything a live Figure could observe, derived from its declaration:
    /// each clause's trigger, each Each modifier's queries, and the natives
    /// that change effect values.
    pub fn shortcut_dependencies(self) -> Vec<ShortcutDependency> {
        use ShortcutDependency::*;
        let def = self.def();
        let mut out = Vec::new();
        for karma in def.karma {
            match karma.on {
                On::Move(verbs) => out.extend(verbs.iter().map(|&verb| Move(verb))),
                On::Draw | On::DrawAction => out.push(Draw),
                On::Dogma(_) | On::Ineligible => out.push(Dogma),
                On::ShareDraw | On::NoShareDraw => out.push(Draw),
                On::MeldAction => out.push(Move(Verb::Meld)),
                On::Effect | On::DogmaEffect | On::DemandAgainst => out.push(EffectValues),
                On::IssueDecree => {}
                On::Claim => out.extend([Move(Verb::Achieve), ClaimReaction]),
                On::Splay(_) => out.push(Splay),
                On::Exchange => out.push(Exchange),
                On::Win => out.push(Win),
                On::Fade => out.push(Fade),
                On::ThisMelded => out.push(Move(Verb::Meld)),
            }
            if karma.body == Body::Native(KarmaNative::Kepler) {
                out.push(EffectValues);
            }
        }
        for each in def.each {
            out.push(match each {
                Each::Icons => IconQuery,
                Each::Score => ScoreQuery,
                Each::Achievements => AchievementQuery,
                Each::Claims | Each::AvailableAchievements => ClaimAchievement,
                Each::DogmaSources => Dogma,
                Each::HandMembership => HandMembership,
                Each::ScoreMembership => ScoreMembership,
                Each::SpecialRequirements => SpecialRequirement,
            });
        }
        out
    }
}

impl Position {
    /// A reduced phase's dependencies plus the rule consequences of its
    /// operations: with Cities, a meld or junk can claim a special
    /// achievement and a splay can draw a City.
    fn consequence_dependencies<'a>(
        &self,
        dependencies: &'a [ShortcutDependency],
    ) -> std::borrow::Cow<'a, [ShortcutDependency]> {
        use ShortcutDependency::*;
        if !self.expansions().cities() {
            return std::borrow::Cow::Borrowed(dependencies);
        }
        let mut out = dependencies.to_vec();
        for dependency in dependencies {
            let implied: &[ShortcutDependency] = match dependency {
                // Cities' tokens have no cost or eligibility, so only claim
                // karmas observe them, never Daedalus's or Leeuwenhoek's
                // claim modifiers.
                Move(Verb::Meld | Verb::Junk) => &[ClaimReaction],
                Splay => &[Draw],
                _ => continue,
            };
            for implied in implied {
                if !out.contains(implied) {
                    out.push(*implied);
                }
            }
        }
        std::borrow::Cow::Owned(out)
    }

    /// Whether a Figure could observe a reduced move with these dependencies.
    /// `live` is false for a covered Figure that the move might expose.
    pub(crate) fn figure_blocks(
        &self,
        figure: Figure,
        owner: Player,
        dependencies: &[ShortcutDependency],
        context: crate::Context,
        outside_piles: bool,
        live: bool,
    ) -> bool {
        let dependencies = &self.consequence_dependencies(dependencies)[..];
        figure.shortcut_dependencies().iter().any(|dependency| {
            dependencies.contains(dependency)
                && match dependency {
                    // Suppressed karma cannot discover Would/When.
                    ShortcutDependency::Move(_) | ShortcutDependency::ClaimReaction
                        if context.suppress_karma() =>
                    {
                        false
                    }
                    // Homer and Nightingale guard hand and score cards only;
                    // a sweep that cannot select either (decks, the junk
                    // without von Braun) never meets them.
                    ShortcutDependency::Move(_)
                        if outside_piles
                            && matches!(figure, Figure::Homer | Figure::Nightingale) =>
                    {
                        false
                    }
                    // A borrowed hand matters only to its executor.
                    ShortcutDependency::HandMembership if live => {
                        owner == context.executor && self.meiji_active(owner)
                    }
                    _ => true,
                }
        })
    }

    /// Whether a selector can never choose a hand or score-pile card,
    /// counting Figure aliases: Michelangelo and Meiji alias hands only, and
    /// von Braun's junk is a score pile while he is a top card.
    pub(crate) fn selector_avoids_piles(&self, selector: crate::program::Selector) -> bool {
        use crate::program::Selector;
        match selector {
            Selector::Junk => !Player::ALL
                .into_iter()
                .any(|player| self.has_figure(player, Figure::WernherVonBraun)),
            Selector::DeckTops
            | Selector::BaseDeck(_)
            | Selector::SetDeck(..)
            | Selector::BaseDeckTop(_)
            | Selector::Tops(_)
            | Selector::TopOf(..)
            | Selector::Board(..)
            | Selector::Achievements(_)
            | Selector::AvailableAchievements
            | Selector::StandardAchievements
            | Selector::Forecast(_)
            | Selector::Safe(_)
            | Selector::Museum(_) => true,
            Selector::Filtered(source, _) => self.selector_avoids_piles(*source),
            Selector::Union(sources) => sources
                .iter()
                .all(|&source| self.selector_avoids_piles(source)),
            _ => false,
        }
    }

    /// Public-board/timing guard only: do not peek into hands or decks to
    /// decide whether a reduction is available. Conservative trigger presence
    /// is intentional, even if one current candidate would not trigger it.
    /// Trebek, for example, may become applicable after a later transfer.
    pub(crate) fn dependencies_clear(
        &self,
        dependencies: &[ShortcutDependency],
        context: crate::Context,
        outside_piles: bool,
    ) -> bool {
        !self.figures()
            || Player::ALL.into_iter().all(|owner| {
                self.top_figures(owner).all(|card| {
                    let figure = self.card(card).figure.expect("top Figure");
                    !self.figure_blocks(figure, owner, dependencies, context, outside_piles, true)
                })
            })
    }
}
