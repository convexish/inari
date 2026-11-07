//! Immutable card metadata for every supported expansion.
//! Symbols retain top-left, bottom-left, bottom-center, bottom-right order,
//! then Cities' top-center and top-right.
//! Complete game catalogs require the corresponding authored programs;
//! missing declarations fail explicitly rather than substituting stubs.

use crate::model::{
    Card, CardSet, Color, Effect, EffectKind, Error, Icon, SLOTS, Symbol, four_slots,
};

mod age_zero;
mod artifacts;
mod artifacts_metadata;
pub use artifacts_metadata::ARTIFACTS_METADATA;
mod base;
mod base_metadata;
pub use base_metadata::BASE_METADATA;
mod city_metadata;
pub use city_metadata::CITY_METADATA;
mod echoes;
mod echoes_metadata;
pub use echoes_metadata::ECHOES_METADATA;
mod figure_metadata;
mod figures;
pub use figure_metadata::{AGE_ZERO_METADATA, FIGURE_METADATA};
pub use figures::{figure_card, figure_metadata};
#[cfg(test)]
mod proof_lint_tests;
mod unseen;
mod unseen_metadata;
pub use unseen_metadata::UNSEEN_METADATA;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardMetadata {
    pub code: &'static str,
    pub title: &'static str,
    pub color: Color,
    pub value: u8,
    pub set: CardSet,
    pub symbols: [Symbol; 4],
    pub featured: Option<Icon>,
}

/// A City: six printed positions, no featured icon and no dogma effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CityMetadata {
    pub code: &'static str,
    pub title: &'static str,
    pub color: Color,
    pub value: u8,
    pub symbols: [Symbol; SLOTS],
}

pub fn city_metadata(code: &str) -> Option<&'static CityMetadata> {
    CITY_METADATA
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &CITY_METADATA[index])
}

/// A City card. Its icons are rules, not effects: it can never be a Dogma
/// source, and executing it runs nothing.
pub fn city_card(code: &str) -> Result<Card, Error> {
    let metadata = city_metadata(code).ok_or(Error::InvalidSetup("unknown City"))?;
    Ok(Card {
        code: metadata.code,
        set: CardSet::Cities,
        color: metadata.color,
        value: metadata.value,
        figure: None,
        echo: None,
        symbols: metadata.symbols,
        featured: None,
        effects: Vec::new(),
    })
}

/// Printed Echoes metadata, separate from the required authored programs.
pub fn echoes_metadata(code: &str) -> Option<&'static CardMetadata> {
    ECHOES_METADATA
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &ECHOES_METADATA[index])
}

/// A printed card's title by publisher code, from any set's metadata.
pub fn title(code: &str) -> Option<&'static str> {
    let prefix = code.chars().next()?;
    match prefix {
        'B' => base_metadata(code).map(|m| m.title),
        'E' => echoes_metadata(code).map(|m| m.title),
        'C' => city_metadata(code).map(|m| m.title),
        'A' => artifacts_metadata(code).map(|m| m.title),
        'U' => unseen_metadata(code).map(|m| m.title),
        'F' => figure_metadata(code).map(|m| m.title),
        'Z' => AGE_ZERO_METADATA
            .iter()
            .find(|m| m.code == code)
            .map(|m| m.title),
        _ => None,
    }
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

pub fn artifacts_metadata(code: &str) -> Option<&'static CardMetadata> {
    ARTIFACTS_METADATA
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &ARTIFACTS_METADATA[index])
}

pub fn unseen_metadata(code: &str) -> Option<&'static CardMetadata> {
    UNSEEN_METADATA
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &UNSEEN_METADATA[index])
}

pub fn base_metadata(code: &str) -> Option<&'static CardMetadata> {
    BASE_METADATA
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &BASE_METADATA[index])
}

/// Construct a complete authored program, or explicitly reject its absence.
pub fn base_card(code: &str) -> Result<Card, Error> {
    let metadata = base_metadata(code).ok_or(Error::InvalidSetup("unknown Base card"))?;
    let effects = base::effects(code).ok_or(Error::Unsupported("Base program not authored"))?;
    Ok(programmed(metadata, effects))
}

/// An authored Echoes card: its echo effect and dogma effects.
pub fn echoes_card(code: &str) -> Result<Card, Error> {
    let metadata = echoes_metadata(code).ok_or(Error::InvalidSetup("unknown Echoes card"))?;
    let programs =
        echoes::programs(code).ok_or(Error::Unsupported("Echoes program not authored"))?;
    Ok(Card {
        set: CardSet::Echoes,
        echo: programs.echo.map(|program| {
            Box::new(Effect {
                kind: EffectKind::NonDemand,
                instructions: program.to_vec(),
            })
        }),
        ..programmed(metadata, programs.dogmas)
    })
}

/// An authored Artifact: its compels and non-demand effects.
pub fn artifacts_card(code: &str) -> Result<Card, Error> {
    let metadata = artifacts_metadata(code).ok_or(Error::InvalidSetup("unknown Artifacts card"))?;
    let effects =
        artifacts::effects(code).ok_or(Error::Unsupported("Artifacts program not authored"))?;
    Ok(Card {
        set: CardSet::Artifacts,
        effects: effects
            .iter()
            .map(|&(kind, program)| Effect {
                kind,
                instructions: program.to_vec(),
            })
            .collect(),
        ..programmed(metadata, &[])
    })
}

/// An authored Unseen card: its demands and non-demand effects.
pub fn unseen_card(code: &str) -> Result<Card, Error> {
    let metadata = unseen_metadata(code).ok_or(Error::InvalidSetup("unknown Unseen card"))?;
    let effects = unseen::effects(code).ok_or(Error::Unsupported("Unseen program not authored"))?;
    Ok(Card {
        set: CardSet::Unseen,
        effects: effects
            .iter()
            .map(|&(kind, program)| Effect {
                kind,
                instructions: program.to_vec(),
            })
            .collect(),
        ..programmed(metadata, &[])
    })
}

/// An Age 0 card (Base set, value 0), dormant until Bear Grylls.
pub fn age_zero_card(code: &str) -> Result<Card, Error> {
    let metadata = AGE_ZERO_METADATA
        .iter()
        .find(|metadata| metadata.code == code)
        .ok_or(Error::InvalidSetup("unknown Age 0 card"))?;
    let effects =
        age_zero::effects(code).ok_or(Error::Unsupported("Age 0 program not authored"))?;
    Ok(programmed(metadata, effects))
}

fn programmed(
    metadata: &'static CardMetadata,
    effects: &'static [(bool, &'static [crate::program::Instruction])],
) -> Card {
    Card {
        code: metadata.code,
        set: CardSet::Base,
        color: metadata.color,
        value: metadata.value,
        figure: None,
        echo: None,
        symbols: four_slots(metadata.symbols),
        featured: metadata.featured,
        effects: effects
            .iter()
            .map(|&(demand, program)| Effect {
                kind: EffectKind::from_demand(demand),
                instructions: program.to_vec(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod identity_tests {
    use crate::{CardSet, Game, STABLE_IDS, StableId};

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
        let game = Game::new_figures(0).unwrap();
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
