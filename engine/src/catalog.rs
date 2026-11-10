//! Immutable card metadata for every supported expansion.
//! Symbols retain top-left, bottom-left, bottom-center, bottom-right order,
//! then Cities' top-center and top-right.
//! Complete game catalogs require the corresponding authored programs;
//! missing declarations fail explicitly rather than substituting stubs.

use crate::model::{Card, CardSet, Color, Effect, EffectKind, Error, Figure, Icon, SLOTS, Symbol};
use crate::program::Instruction;

mod age_zero;
mod age_zero_metadata;
pub use age_zero_metadata::AGE_ZERO_METADATA;
mod artifacts;
mod artifacts_metadata;
pub use artifacts_metadata::ARTIFACTS_METADATA;
mod base;
mod base_metadata;
pub use base_metadata::BASE_METADATA;
mod cities_metadata;
pub use cities_metadata::CITIES_METADATA;
mod echoes;
mod echoes_metadata;
pub use echoes_metadata::ECHOES_METADATA;
mod figures;
mod figures_metadata;
pub use figures::{FIGURE_NAMES, FigureName};
pub use figures_metadata::FIGURES_METADATA;
#[cfg(test)]
mod proof_lint_tests;
mod unseen;
mod unseen_metadata;
pub use unseen_metadata::UNSEEN_METADATA;

/// One printed effect's instructions.
type Program = &'static [Instruction];

/// A card's dogma effects, none of them a demand.
const fn plain<const N: usize>(programs: [Program; N]) -> [(EffectKind, Program); N] {
    let mut out = [(EffectKind::NonDemand, programs[0]); N];
    let mut i = 0;
    while i < N {
        out[i].1 = programs[i];
        i += 1;
    }
    out
}

/// A card's dogma effects, the first of them a demand.
const fn demand<const N: usize>(programs: [Program; N]) -> [(EffectKind, Program); N] {
    let mut out = plain(programs);
    out[0].0 = EffectKind::Demand;
    out
}

/// One printed card. Only Cities print the last two of the six positions,
/// and only cards with dogma effects have a featured icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardMetadata {
    pub code: &'static str,
    pub title: &'static str,
    pub color: Color,
    pub value: u8,
    pub set: CardSet,
    pub symbols: [Symbol; SLOTS],
    pub featured: Option<Icon>,
}

/// A printed card's metadata by publisher code, from any set or Age 0.
pub fn card_metadata(code: &str) -> Option<&'static CardMetadata> {
    let table: &'static [CardMetadata] = match code.as_bytes().first()? {
        b'B' => &BASE_METADATA,
        b'E' => &ECHOES_METADATA,
        b'C' => &CITIES_METADATA,
        b'U' => &UNSEEN_METADATA,
        b'A' => &ARTIFACTS_METADATA,
        b'F' => &FIGURES_METADATA,
        b'Z' => &AGE_ZERO_METADATA,
        _ => return None,
    };
    table
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &table[index])
}

/// A printed card's title by publisher code.
pub fn title(code: &str) -> Option<&'static str> {
    card_metadata(code).map(|metadata| metadata.title)
}

/// Alphabetical order of two cards' titles (Basur Hoyuk Tokens): lowercase
/// alphanumeric characters only, with common accented Latin letters folded.
/// A card without a printed title sorts after every title.
pub fn title_order(left: &str, right: &str) -> std::cmp::Ordering {
    fn key(code: &str) -> Option<String> {
        Some(
            title(code)?
                .chars()
                .filter_map(|c| {
                    let folded = match c {
                        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => {
                            'a'
                        }
                        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
                        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
                        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
                        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
                        'ñ' | 'Ñ' => 'n',
                        'ç' | 'Ç' => 'c',
                        'ß' => 's',
                        c => c,
                    };
                    folded
                        .is_ascii_alphanumeric()
                        .then(|| folded.to_ascii_lowercase())
                })
                .collect(),
        )
    }
    match (key(left), key(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

impl Card {
    /// A printed card of any set, with its authored programs.
    ///
    /// A City's icons are rules, not effects: it can never be a Dogma source,
    /// and executing it runs nothing. A Figure's karma lives in its
    /// declaration, not in Dogma effects. Age 0 cards are Base cards of
    /// value 0. A card without an authored program is an error, never a stub.
    pub fn from_code(code: &str) -> Result<Self, Error> {
        let metadata = card_metadata(code).ok_or(Error::InvalidSetup("unknown card"))?;
        let unauthored = Error::Unsupported("card program not authored");
        let (mut echo, mut figure) = (None, None);
        let effects: &[(EffectKind, Program)] = match metadata.set {
            CardSet::Base if code.starts_with('Z') => age_zero::effects(code).ok_or(unauthored)?,
            CardSet::Base => base::effects(code).ok_or(unauthored)?,
            CardSet::Echoes => {
                let programs = echoes::programs(code).ok_or(unauthored)?;
                echo = programs.echo;
                programs.dogmas
            }
            CardSet::Cities => &[],
            CardSet::Unseen => unseen::effects(code).ok_or(unauthored)?,
            CardSet::Artifacts => artifacts::effects(code).ok_or(unauthored)?,
            CardSet::Figures => {
                figure = Some(
                    Figure::ALL
                        .into_iter()
                        .find(|figure| figure.code() == code)
                        .ok_or(unauthored)?,
                );
                &[]
            }
        };
        let effect = |kind, program: Program| Effect {
            kind,
            instructions: program.to_vec(),
        };
        Ok(Self {
            code: metadata.code,
            set: metadata.set,
            color: metadata.color,
            value: metadata.value,
            figure,
            echo: echo.map(|program| Box::new(effect(EffectKind::NonDemand, program))),
            symbols: metadata.symbols,
            featured: metadata.featured,
            effects: effects
                .iter()
                .map(|&(kind, program)| effect(kind, program))
                .collect(),
        })
    }
}

#[cfg(test)]
mod identity_tests {
    use crate::{CardSet, Expansions, Game, STABLE_IDS, StableId};

    #[test]
    fn stable_ids_reserve_each_set_and_age_zero_from_publisher_codes() {
        for (code, id) in [
            ("B001", 0),
            ("B115", 114),
            ("E001", 115),
            ("C001", 230),
            ("U001", 345),
            ("A001", 460),
            ("A115", 574),
            ("F001", 575),
            ("F115", 689),
            ("Z001", 690),
            ("Z015", 704),
        ] {
            assert_eq!(StableId::from_code(code), Some(StableId(id)), "{code}");
        }
        assert_eq!(STABLE_IDS, 705);
        for code in [
            "B000",
            "B116",
            "Z016",
            "X001",
            "B01",
            "synthetic-base-program",
        ] {
            assert_eq!(StableId::from_code(code), None, "{code}");
        }
    }

    #[test]
    fn every_dealt_card_names_its_set_and_a_distinct_stable_id_in_its_range() {
        let game = Game::new_game(0, Expansions::FIGURES).unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for card in game.position().catalog.iter() {
            let id = card.stable_id().expect("printed card").0;
            assert!(seen.insert(id), "{}", card.code);
            let range = if card.code.starts_with('Z') {
                assert_eq!((card.set, card.value), (CardSet::Base, 0));
                690..705
            } else {
                let first = card.set.index() as u16 * 115;
                first..first + 115
            };
            assert!(range.contains(&id), "{}", card.code);
            assert_eq!(card.figure.is_some(), card.set == CardSet::Figures);
        }
        assert_eq!(seen.len(), 245);
    }
}
