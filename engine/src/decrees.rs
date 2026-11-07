//! Figures Decree identities and singular ownership, separate from specials.

use crate::{ActionKind, Color, Error, Player, Position, Zone};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decree {
    War,
    Expansion,
    Trade,
    Advancement,
    Rivalry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecreeLocation {
    Supply,
    Owned(Player),
    /// Owned Decrees leave play with an eliminated civilization.
    Junk,
}

impl Decree {
    pub const ALL: [Self; 5] = [
        Self::War,
        Self::Expansion,
        Self::Trade,
        Self::Advancement,
        Self::Rivalry,
    ];

    pub const fn color(self) -> Color {
        match self {
            Self::War => Color::Red,
            Self::Expansion => Color::Yellow,
            Self::Trade => Color::Green,
            Self::Advancement => Color::Blue,
            Self::Rivalry => Color::Purple,
        }
    }
}

impl Position {
    /// Initial-hand permissions, before any payment movement or replacement.
    pub(crate) fn decree_action_options(&self, kind: ActionKind) -> Result<Vec<Decree>, Error> {
        let owner = self.turn_owner;
        if !self.figures()
            || (!self.meiji_active(owner)
                && self
                    .cards(Zone::Hand(owner))
                    .iter()
                    .all(|&card| self.card(card).figure.is_none()))
        {
            return Ok(Vec::new());
        }
        let mut count = 0;
        let mut values = 0_u16;
        let mut colors = 0_u8;
        for target in self.action_hand(kind) {
            let card = self.card(target.card);
            if card.figure.is_some() {
                count += 1;
                values |= 1 << card.value;
                colors |= 1 << card.color as u8;
            }
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        // Three Figures of different values: a Decree of one of their colors.
        // Decree karma lowers the count and lifts the color restriction.
        let grants: Vec<_> = self
            .top_figures(self.turn_owner)
            .filter_map(|card| self.card(card).figure)
            .flat_map(|figure| figure.def().decrees.iter().copied())
            .collect();
        Ok(Decree::ALL
            .into_iter()
            .filter(|&decree| {
                (values.count_ones() >= 3 && colors & (1 << decree.color() as u8) != 0)
                    || grants.iter().any(|&(granted, figures)| {
                        granted.is_none_or(|granted| granted == decree) && count >= figures
                    })
            })
            .collect())
    }

    pub fn decree_location(&self, decree: Decree) -> DecreeLocation {
        self.decrees
            .as_ref()
            .map_or(DecreeLocation::Supply, |locations| {
                locations[decree as usize]
            })
    }
}
