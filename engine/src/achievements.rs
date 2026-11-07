//! Special-achievement requirements and singular token ownership.
//! Structured numerical modifiers leave fixed icon lists and predicates intact.
//! Cities' five tokens have no standing requirement: specific completed melds
//! and junks earn them.

use crate::model::{CityIcon, Symbol};
use crate::{Color, Figure, Icon, Player, Position, Splay, Zone};

/// Discriminants are reserved slots: five each for Base, Echoes, Cities and
/// Unseen, in stable index order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Special {
    Monument = 0,
    Empire = 1,
    World = 2,
    Wonder = 3,
    Universe = 4,
    Supremacy = 5,
    Destiny = 6,
    Wealth = 7,
    Heritage = 8,
    History = 9,
    Tradition = 10,
    Repute = 11,
    Fame = 12,
    Glory = 13,
    Victory = 14,
    Confidence = 15,
    Zen = 16,
    Anonymity = 17,
    Folklore = 18,
    Mystery = 19,
}

impl Special {
    /// The achievement's printed name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Monument => "Monument",
            Self::Empire => "Empire",
            Self::World => "World",
            Self::Wonder => "Wonder",
            Self::Universe => "Universe",
            Self::Supremacy => "Supremacy",
            Self::Destiny => "Destiny",
            Self::Wealth => "Wealth",
            Self::Heritage => "Heritage",
            Self::History => "History",
            Self::Tradition => "Tradition",
            Self::Repute => "Repute",
            Self::Fame => "Fame",
            Self::Glory => "Glory",
            Self::Victory => "Victory",
            Self::Confidence => "Confidence",
            Self::Zen => "Zen",
            Self::Anonymity => "Anonymity",
            Self::Folklore => "Folklore",
            Self::Mystery => "Mystery",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialLocation {
    Available,
    Owned(Player),
    Junk,
}

/// Numerical requirements are distinct from predicates and fixed icon lists.
/// Each modifiers transform these requirements, not the final eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialRequirement {
    TopDemands {
        count: usize,
    },
    /// At least `count` of each of at least `types` of these icon types. Hedy lowers
    /// both: two of any five of Empire's six.
    EachIcon {
        icons: &'static [Icon],
        count: usize,
        types: usize,
    },
    IconCount {
        icon: Icon,
        count: usize,
    },
    SplayedColors {
        directions: &'static [Splay],
        count: usize,
    },
    TopValues {
        count: usize,
        minimum: u8,
    },
    /// At least `count` of one icon type in each of at least `colors`
    /// colors. A type is a standard icon (a Search icon counts as its
    /// standard icon), Bonus of any value, or one City icon kind.
    CommonIcon {
        count: usize,
        colors: usize,
    },
    ForecastCards {
        count: usize,
    },
    /// Visible bonuses, each occurrence counting.
    Bonuses {
        count: usize,
    },
    /// Visible image hexes in one color.
    ImagesInColor {
        count: usize,
    },
    /// Visible echo effects in one color.
    EchoesInColor {
        count: usize,
    },
    /// Unseen: a top card of at least `minimum`, plus one more condition. The value is
    /// the claim value, so Daedalus's unsplayed-color allowance counts.
    TopValueWith {
        minimum: u8,
        with: UnseenCondition,
    },
}

/// The second half of each Unseen special's requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnseenCondition {
    /// At least this many secrets.
    Secrets(usize),
    /// No top card of odd value.
    NoOddTops,
    /// No standard achievements.
    NoStandardAchievements,
    /// None of this icon visible on the board.
    NoIcon(Icon),
    /// Fewer than this many colors on the board.
    FewerColors(usize),
}

impl SpecialRequirement {
    fn one_lower(mut self) -> Self {
        match &mut self {
            Self::TopDemands { count }
            | Self::IconCount { count, .. }
            | Self::SplayedColors { count, .. }
            | Self::ForecastCards { count }
            | Self::Bonuses { count }
            | Self::ImagesInColor { count }
            | Self::EchoesInColor { count } => *count = count.saturating_sub(1),
            Self::TopValues { count, minimum } => {
                *count = count.saturating_sub(1);
                *minimum = minimum.saturating_sub(1);
            }
            Self::CommonIcon { count, colors }
            | Self::EachIcon {
                count,
                types: colors,
                ..
            } => {
                *count = count.saturating_sub(1);
                *colors = colors.saturating_sub(1);
            }
            // Hedy lowers each number, which makes Mystery harder; the "no..."
            // conditions have none.
            Self::TopValueWith { minimum, with } => {
                *minimum = minimum.saturating_sub(1);
                match with {
                    UnseenCondition::Secrets(count) => *count = count.saturating_sub(1),
                    UnseenCondition::FewerColors(bound) => *bound = bound.saturating_sub(1),
                    UnseenCondition::NoOddTops
                    | UnseenCondition::NoStandardAchievements
                    | UnseenCondition::NoIcon(_) => {}
                }
            }
        }
        self
    }
}

impl Special {
    /// Storage reserved for every expansion's tokens: five each from Base,
    /// Echoes, Cities and Unseen.
    pub const RESERVED: usize = 20;

    /// Base's five tokens. Games iterate [`Position::specials`] instead.
    pub const ALL: [Self; 5] = [
        Self::Monument,
        Self::Empire,
        Self::World,
        Self::Wonder,
        Self::Universe,
    ];

    /// Echoes' five tokens, claimed by the end-of-action check like Base's.
    pub const ECHOES: [Self; 5] = [
        Self::Supremacy,
        Self::Destiny,
        Self::Wealth,
        Self::Heritage,
        Self::History,
    ];

    /// Cities' five tokens, each earned by an event.
    pub const CITIES: [Self; 5] = [
        Self::Tradition,
        Self::Repute,
        Self::Fame,
        Self::Glory,
        Self::Victory,
    ];

    /// Unseen's five tokens (`data/unseen_specials.json`), claimed by the
    /// end-of-action check like Base's. Their cards' direct claims follow.
    pub const UNSEEN: [Self; 5] = [
        Self::Confidence,
        Self::Zen,
        Self::Anonymity,
        Self::Folklore,
        Self::Mystery,
    ];

    /// Fourth-edition Base requirements, not the earlier edition's score/tuck version
    /// of Monument. Event-earned tokens have none and are never claimed by the
    /// end-of-action check.
    pub const fn requirement(self) -> Option<SpecialRequirement> {
        Some(match self {
            Self::Monument => SpecialRequirement::TopDemands { count: 4 },
            Self::Empire => SpecialRequirement::EachIcon {
                icons: &[
                    Icon::Authority,
                    Icon::Concept,
                    Icon::Health,
                    Icon::Prosperity,
                    Icon::Industry,
                    Icon::Efficiency,
                ],
                count: 3,
                types: 6,
            },
            Self::World => SpecialRequirement::IconCount {
                icon: Icon::Efficiency,
                count: 12,
            },
            Self::Wonder => SpecialRequirement::SplayedColors {
                directions: &[Splay::Right, Splay::Up, Splay::Aslant],
                count: 5,
            },
            Self::Universe => SpecialRequirement::TopValues {
                count: 5,
                minimum: 8,
            },
            Self::Supremacy => SpecialRequirement::CommonIcon {
                count: 3,
                colors: 4,
            },
            Self::Destiny => SpecialRequirement::ForecastCards { count: 5 },
            Self::Wealth => SpecialRequirement::Bonuses { count: 8 },
            Self::Heritage => SpecialRequirement::ImagesInColor { count: 8 },
            Self::History => SpecialRequirement::EchoesInColor { count: 4 },
            Self::Confidence => SpecialRequirement::TopValueWith {
                minimum: 5,
                with: UnseenCondition::Secrets(4),
            },
            Self::Zen => SpecialRequirement::TopValueWith {
                minimum: 6,
                with: UnseenCondition::NoOddTops,
            },
            Self::Anonymity => SpecialRequirement::TopValueWith {
                minimum: 7,
                with: UnseenCondition::NoStandardAchievements,
            },
            Self::Folklore => SpecialRequirement::TopValueWith {
                minimum: 8,
                with: UnseenCondition::NoIcon(Icon::Industry),
            },
            Self::Mystery => SpecialRequirement::TopValueWith {
                minimum: 9,
                with: UnseenCondition::FewerColors(5),
            },
            Self::Tradition | Self::Repute | Self::Fame | Self::Glory | Self::Victory => {
                return None;
            }
        })
    }
}

impl Special {
    /// The token's slot in reserved storage and masks.
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl Position {
    pub fn special_location(&self, special: Special) -> SpecialLocation {
        self.specials[special.index()]
    }

    /// This game's enabled special-achievement tokens, in roster order.
    /// Base's five are always present; each enabled expansion adds its own.
    pub fn specials(&self) -> impl Iterator<Item = Special> + use<> {
        let expansions = self.expansions();
        let echoes: &'static [Special] = if expansions.echoes() {
            &Special::ECHOES
        } else {
            &[]
        };
        let cities: &'static [Special] = if expansions.cities() {
            &Special::CITIES
        } else {
            &[]
        };
        let unseen: &'static [Special] = if expansions.unseen() {
            &Special::UNSEEN
        } else {
            &[]
        };
        Special::ALL
            .into_iter()
            .chain(echoes.iter().copied())
            .chain(cities.iter().copied())
            .chain(unseen.iter().copied())
    }

    #[inline]
    pub fn meets_special_requirement(
        &self,
        player: Player,
        mut requirement: SpecialRequirement,
    ) -> bool {
        let figures = self.figure_rules(player);
        if figures.contains(Figure::HedyLamarr) {
            requirement = requirement.one_lower();
        }
        match requirement {
            SpecialRequirement::TopDemands { count } => {
                Color::ALL
                    .into_iter()
                    .filter_map(|color| self.top(player, color))
                    .filter(|&card| self.card(card).effects.iter().any(|effect| effect.demand()))
                    .count()
                    >= count
            }
            SpecialRequirement::EachIcon {
                icons,
                count,
                types,
            } => {
                // Share one symbol scan across all required icon types.
                let symbols = self.board_symbols(player);
                icons
                    .iter()
                    .filter(|&&icon| self.effective_icon(&symbols, figures, player, icon) >= count)
                    .count()
                    >= types
            }
            SpecialRequirement::IconCount { icon, count } => self.icon_count(player, icon) >= count,
            SpecialRequirement::SplayedColors { directions, count } => {
                Color::ALL
                    .into_iter()
                    .filter(|&color| directions.contains(&self.splay(player, color)))
                    .count()
                    >= count
            }
            SpecialRequirement::TopValues { count, minimum } => {
                Color::ALL
                    .into_iter()
                    .filter_map(|color| self.top(player, color))
                    .filter(|&card| self.value(card) >= minimum)
                    .count()
                    >= count
            }
            other => self.meets_echoes_requirement(player, other),
        }
    }

    // Keep the larger Echoes scans out of the common Base requirement checks.
    #[inline(never)]
    fn meets_echoes_requirement(&self, player: Player, requirement: SpecialRequirement) -> bool {
        match requirement {
            SpecialRequirement::CommonIcon { count, colors } => {
                self.common_icon_colors(player, count) >= colors
            }
            SpecialRequirement::ForecastCards { count } => {
                self.cards(Zone::Forecast(player)).len() >= count
            }
            SpecialRequirement::Bonuses { count } => {
                Color::ALL
                    .into_iter()
                    .map(|color| {
                        self.visible_count(player, color, |s| matches!(s, Symbol::Bonus(_)))
                    })
                    .sum::<usize>()
                    >= count
            }
            SpecialRequirement::ImagesInColor { count } => Color::ALL
                .into_iter()
                .any(|color| self.visible_count(player, color, |s| s == Symbol::Image) >= count),
            SpecialRequirement::EchoesInColor { count } => Color::ALL
                .into_iter()
                .any(|color| self.visible_count(player, color, |s| s == Symbol::Echo) >= count),
            SpecialRequirement::TopValueWith { minimum, with } => {
                self.highest_claim_value(player) >= minimum
                    && match with {
                        UnseenCondition::Secrets(count) => {
                            self.cards(Zone::Safe(player)).len() >= count
                        }
                        UnseenCondition::NoOddTops => Color::ALL
                            .into_iter()
                            .filter_map(|color| self.top(player, color))
                            .all(|card| self.value(card).is_multiple_of(2)),
                        UnseenCondition::NoStandardAchievements => {
                            self.cards(Zone::Achievements(player)).is_empty()
                        }
                        UnseenCondition::NoIcon(icon) => self.icon_count(player, icon) == 0,
                        UnseenCondition::FewerColors(bound) => {
                            Color::ALL
                                .into_iter()
                                .filter(|&color| self.top(player, color).is_some())
                                .count()
                                < bound
                        }
                    }
            }
            _ => unreachable!("Base requirement on Echoes path"),
        }
    }

    /// The most colors in which one icon type shows at least `count` times. Only
    /// symbols visible on the board count: Figures' Each additions change totals, not
    /// any pile.
    fn common_icon_colors(&self, player: Player, count: usize) -> usize {
        let colors_with = |per_color: &dyn Fn(Color) -> usize| {
            Color::ALL
                .into_iter()
                .filter(|&color| per_color(color) >= count)
                .count()
        };
        let standard = [
            Icon::Authority,
            Icon::Concept,
            Icon::Health,
            Icon::Prosperity,
            Icon::Industry,
            Icon::Efficiency,
            Icon::Avatar,
            Icon::Food,
        ]
        .into_iter()
        .map(|icon| {
            colors_with(&|color| self.visible_count(player, color, |s| s.standard() == Some(icon)))
        });
        let bonus = colors_with(&|color| {
            self.visible_count(player, color, |s| matches!(s, Symbol::Bonus(_)))
        });
        let city = CityIcon::ALL.into_iter().map(|kind| {
            colors_with(&|color| self.visible_count(player, color, |s| s == Symbol::City(kind)))
        });
        standard.chain([bonus]).chain(city).max().unwrap_or(0)
    }

    /// Whether the end-of-action check awards this token now. Event-earned
    /// tokens are never eligible here.
    pub fn eligible_for_special(&self, player: Player, special: Special) -> bool {
        self.special_location(special) == SpecialLocation::Available
            && special
                .requirement()
                .is_some_and(|requirement| self.meets_special_requirement(player, requirement))
    }
}
