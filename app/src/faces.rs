//! Card faces the table draws: every printed card once, keyed by publisher
//! code, so a `FaceId` also names that card on the table. The five
//! publisher sets come from `data/*.json` in the engine's stable-identity
//! order; Figures and Age 0 follow from the engine's reference text.
use super::table::title_case;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use inno_engine::{CardSet, Color as CardColor, Icon};
use serde::Deserialize;

pub type FaceId = u16;

/// Board column order, as the table lays the colors out.
pub const COLORS: [CardColor; 5] = [
    CardColor::Red,
    CardColor::Yellow,
    CardColor::Green,
    CardColor::Blue,
    CardColor::Purple,
];

/// A color's board column, the index of its stack in a player view.
pub fn stack_index(color: CardColor) -> usize {
    COLORS
        .iter()
        .position(|&c| c == color)
        .expect("every color has a board column")
}

/// The seven icons with their own tiles; Food (Age 0) is drawn from
/// `Face::food_slots`.
pub const ICONS: [Icon; 7] = [
    Icon::Authority,
    Icon::Concept,
    Icon::Health,
    Icon::Prosperity,
    Icon::Industry,
    Icon::Efficiency,
    Icon::Avatar,
];

/// A City's own icons; Search shares its slot with a standard icon.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CityIcon {
    Plus,
    Uplift,
    Junk,
    Left,
    Right,
    Up,
    Unsplay,
    Flag,
    Fountain,
    Search,
}

impl CityIcon {
    pub const ALL: [Self; 10] = [
        Self::Plus,
        Self::Uplift,
        Self::Junk,
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Unsplay,
        Self::Flag,
        Self::Fountain,
        Self::Search,
    ];
}

/// A card's set, as its back shows it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Set {
    Base,
    Echoes,
    Cities,
    Unseen,
    Artifacts,
    Figures,
}

impl Set {
    pub const fn name(self) -> &'static str {
        match self {
            Set::Base => "Base",
            Set::Echoes => "Echoes",
            Set::Cities => "Cities",
            Set::Unseen => "Unseen",
            Set::Artifacts => "Artifacts",
            Set::Figures => "Figures",
        }
    }
}

impl From<CardSet> for Set {
    fn from(set: CardSet) -> Self {
        match set {
            CardSet::Base => Set::Base,
            CardSet::Echoes => Set::Echoes,
            CardSet::Cities => Set::Cities,
            CardSet::Unseen => Set::Unseen,
            CardSet::Artifacts => Set::Artifacts,
            CardSet::Figures => Set::Figures,
        }
    }
}

/// What leads a paragraph of card text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lead {
    /// An echo effect, marked with the ripple.
    Echo,
    /// A dogma effect, led by its featured icon.
    Icon(Icon),
    /// An Age 0 dogma led by Food.
    Food,
    /// A Figure's karma: a Would, When or Each clause.
    Karma,
    /// A Figure's Decree permission.
    Decree,
}

#[derive(Clone, Debug)]
pub struct Paragraph {
    pub lead: Lead,
    pub text: String,
}

/// Everything printed on a card's face. Slot vectors have four entries, or
/// six for Cities (top centre and top right appended).
#[derive(Clone, Debug)]
pub struct Face {
    pub code: String,
    pub title: String,
    pub color: CardColor,
    /// The printed age, which the back shows.
    pub age: u8,
    /// The value while face up (Battleship Yamato is 11).
    pub value: u8,
    pub set: Set,
    pub icons: Vec<Option<Icon>>,
    /// Slots printing Food, the Age 0 icon.
    pub food_slots: Vec<usize>,
    pub bonuses: Vec<Option<u8>>,
    pub city_icons: Vec<Option<CityIcon>>,
    pub echo_slots: Vec<usize>,
    pub image_slot: usize,
    /// Echo effects first, then dogma effects or karmas, in printed order.
    pub paragraphs: Vec<Paragraph>,
}

/// The five publisher sets' faces, in the order their `FaceId`s run.
const PRINTED_SETS: [(&str, Set); 5] = [
    (include_str!("../../data/base.json"), Set::Base),
    (include_str!("../../data/echoes.json"), Set::Echoes),
    (include_str!("../../data/cities.json"), Set::Cities),
    (include_str!("../../data/unseen.json"), Set::Unseen),
    (include_str!("../../data/artifacts.json"), Set::Artifacts),
];

/// Cards in the five publisher sets: 115 each.
pub const PRINTED_CARDS: usize = 5 * 115;

const FIGURES_TEXT: &str = include_str!("../../engine/reference/figures-text.json");

/// The icon a card's text or data names in capitals (`INDUSTRY`).
pub fn icon_named(name: &str) -> Option<Icon> {
    Some(match name {
        "AUTHORITY" => Icon::Authority,
        "CONCEPT" => Icon::Concept,
        "HEALTH" => Icon::Health,
        "PROSPERITY" => Icon::Prosperity,
        "INDUSTRY" => Icon::Industry,
        "EFFICIENCY" => Icon::Efficiency,
        "AVATAR" => Icon::Avatar,
        _ => return None,
    })
}

fn card_icon(name: &str, code: &str) -> Icon {
    icon_named(name).unwrap_or_else(|| panic!("{code}: unknown icon {name:?}"))
}

fn color_named(name: &str, code: &str) -> CardColor {
    match name {
        "RED" => CardColor::Red,
        "YELLOW" => CardColor::Yellow,
        "GREEN" => CardColor::Green,
        "BLUE" => CardColor::Blue,
        "PURPLE" => CardColor::Purple,
        _ => panic!("{code}: unknown color {name:?}"),
    }
}

#[derive(Deserialize)]
struct RawIcons {
    top_left: Option<String>,
    bottom_left: Option<String>,
    bottom_center: Option<String>,
    bottom_right: Option<String>,
    #[serde(default)]
    top_center: Option<String>,
    #[serde(default)]
    top_right: Option<String>,
}

#[derive(Deserialize)]
struct RawEcho {
    slot: usize,
    text: String,
}

#[derive(Deserialize)]
struct RawEffect {
    featured_icon: String,
    text: String,
}

#[derive(Deserialize)]
struct RawCard {
    code: String,
    title: String,
    color: String,
    age: u8,
    effects: Vec<RawEffect>,
    icons: RawIcons,
    #[serde(default)]
    bonuses: Vec<Option<u8>>,
    #[serde(default)]
    city_icons: Vec<Option<CityIcon>>,
    #[serde(default)]
    echoes: Vec<RawEcho>,
    #[serde(default)]
    image_slot: Option<usize>,
}

impl RawCard {
    fn into_face(self, set: Set) -> Face {
        let code = &self.code;
        let slots = if set == Set::Cities { 6 } else { 4 };
        let icons: Vec<Option<Icon>> = [
            &self.icons.top_left,
            &self.icons.bottom_left,
            &self.icons.bottom_center,
            &self.icons.bottom_right,
            &self.icons.top_center,
            &self.icons.top_right,
        ][..slots]
            .iter()
            .map(|slot| slot.as_deref().map(|name| card_icon(name, code)))
            .collect();
        let bonuses = if self.bonuses.is_empty() {
            vec![None; slots]
        } else {
            self.bonuses
        };
        let city_icons = if self.city_icons.is_empty() {
            vec![None; slots]
        } else {
            self.city_icons
        };
        assert!(
            icons.len() == slots && bonuses.len() == slots && city_icons.len() == slots,
            "{code}: expected {slots} slots"
        );
        let echo_slots: Vec<usize> = self.echoes.iter().map(|e| e.slot).collect();
        // The art fills the slot nothing else prints in, unless the data
        // names one (Battleship Yamato prints no icon in two slots).
        let image_slot = self.image_slot.unwrap_or_else(|| {
            (0..slots)
                .find(|&slot| {
                    icons[slot].is_none()
                        && bonuses[slot].is_none()
                        && city_icons[slot].is_none()
                        && !echo_slots.contains(&slot)
                })
                .unwrap_or_else(|| panic!("{code}: no slot for the image"))
        });
        let mut paragraphs: Vec<Paragraph> = self
            .echoes
            .into_iter()
            .map(|e| Paragraph {
                lead: Lead::Echo,
                text: e.text,
            })
            .collect();
        paragraphs.extend(self.effects.into_iter().map(|e| Paragraph {
            lead: Lead::Icon(card_icon(&e.featured_icon, code)),
            text: e.text,
        }));
        Face {
            value: if self.code == "A077" { 11 } else { self.age },
            color: color_named(&self.color, code),
            code: self.code,
            title: self.title,
            age: self.age,
            set,
            icons,
            food_slots: Vec::new(),
            bonuses,
            city_icons,
            echo_slots,
            image_slot,
            paragraphs,
        }
    }
}

#[derive(Deserialize)]
struct Reference {
    cards: Vec<ReferenceFace>,
}

#[derive(Deserialize)]
struct ReferenceFace {
    code: String,
    title: String,
    color: String,
    age: u8,
    origin_set: String,
    symbols: Vec<serde_json::Value>,
    #[serde(default)]
    karmas: Vec<ReferenceKarma>,
    #[serde(default)]
    effects: Vec<ReferenceEffect>,
}

#[derive(Deserialize)]
struct ReferenceKarma {
    kind: String,
    text: String,
}

#[derive(Deserialize)]
struct ReferenceEffect {
    featured_icon: String,
    text: String,
}

impl ReferenceFace {
    fn into_face(self) -> Face {
        let code = &self.code;
        let color = color_named(&self.color, code);
        let set = match self.origin_set.as_str() {
            "Figures" => Set::Figures,
            "Base" => Set::Base,
            set => panic!("{code}: unknown origin set {set:?}"),
        };
        assert_eq!(self.symbols.len(), 4, "{code}: expected four slots");
        let mut icons = vec![None; 4];
        let mut bonuses = vec![None; 4];
        let mut food_slots = Vec::new();
        let mut image_slot = None;
        for (slot, symbol) in self.symbols.iter().enumerate() {
            match symbol.as_str() {
                Some("IMAGE") => {
                    assert!(
                        image_slot.replace(slot).is_none(),
                        "{code}: duplicate image"
                    );
                }
                Some("FOOD") => food_slots.push(slot),
                Some(name) => icons[slot] = Some(card_icon(name, code)),
                None => {
                    let bonus = symbol
                        .as_object()
                        .filter(|object| object.len() == 1)
                        .and_then(|object| object.get("bonus"))
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|n| u8::try_from(n).ok())
                        .unwrap_or_else(|| panic!("{code}: unknown symbol {symbol}"));
                    bonuses[slot] = Some(bonus);
                }
            }
        }
        let mut paragraphs: Vec<_> = self
            .karmas
            .into_iter()
            .map(|karma| Paragraph {
                lead: match karma.kind.as_str() {
                    "would" | "when" | "each" => Lead::Karma,
                    "decree" => Lead::Decree,
                    kind => panic!("{code}: unknown karma kind {kind:?}"),
                },
                text: karma.text,
            })
            .collect();
        paragraphs.extend(self.effects.into_iter().map(|effect| Paragraph {
            lead: if effect.featured_icon == "FOOD" {
                Lead::Food
            } else {
                Lead::Icon(card_icon(&effect.featured_icon, code))
            },
            text: effect.text,
        }));
        let image_slot = image_slot.unwrap_or_else(|| panic!("{code}: missing image"));
        Face {
            code: self.code,
            title: self.title,
            color,
            age: self.age,
            value: self.age,
            set,
            icons,
            food_slots,
            bonuses,
            city_icons: vec![None; 4],
            echo_slots: Vec::new(),
            image_slot,
            paragraphs,
        }
    }
}

struct Table {
    faces: Vec<Face>,
    by_code: HashMap<String, FaceId>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut faces: Vec<Face> = PRINTED_SETS
            .into_iter()
            .flat_map(|(json, set)| {
                let raw: Vec<RawCard> = serde_json::from_str(json).expect("card data parses");
                assert_eq!(raw.len(), PRINTED_CARDS / 5, "{} cards", set.name());
                raw.into_iter().map(move |card| card.into_face(set))
            })
            .collect();
        let reference: Reference =
            serde_json::from_str(FIGURES_TEXT).expect("invalid Figures face reference JSON");
        faces.extend(reference.cards.into_iter().map(ReferenceFace::into_face));
        let by_code = faces
            .iter()
            .enumerate()
            .map(|(id, f)| (f.code.clone(), id as FaceId))
            .collect();
        Table { faces, by_code }
    })
}

pub fn faces() -> &'static [Face] {
    &table().faces
}

pub fn face(id: FaceId) -> &'static Face {
    &table().faces[usize::from(id)]
}

pub fn by_code(code: &str) -> Option<FaceId> {
    table().by_code.get(code).copied()
}

/// Martian Internet's printed effect. The card data records the draw variant
/// both engines play by default; a match playing the printed subgame shows
/// this text instead.
const MARTIAN_INTERNET_PRINTED: &str = "Start a new game, ignoring all players' current cards and \
    without shuffling, each player drawing and melding a [9] to begin, the winner then \
    transfering their cards to this game and junking each player's other cards from that game.";

/// A card's effect paragraphs as the current match plays them.
pub fn paragraphs(id: FaceId, martian_internet_printed: bool) -> Cow<'static, [Paragraph]> {
    let face = face(id);
    if martian_internet_printed && face.code == "A113" {
        let mut paragraphs = face.paragraphs.clone();
        for paragraph in &mut paragraphs {
            if matches!(paragraph.lead, Lead::Icon(_)) {
                paragraph.text = MARTIAN_INTERNET_PRINTED.into();
            }
        }
        Cow::Owned(paragraphs)
    } else {
        Cow::Borrowed(&face.paragraphs)
    }
}

/// Private piles are represented solely by sorted backs for the opponent.
pub fn backs(cards: &[FaceId]) -> String {
    let mut backs: Vec<_> = cards
        .iter()
        .map(|&id| {
            let f = face(id);
            format!("{}{}", &f.code[..1], f.age)
        })
        .collect();
    backs.sort();
    if backs.is_empty() {
        "none".into()
    } else {
        backs.join("  ")
    }
}

/// Card titles for a pile this viewer may see, age order, "none" if empty.
pub fn names(cards: &[FaceId]) -> String {
    let mut names: Vec<_> = cards
        .iter()
        .map(|&id| {
            let f = face(id);
            (f.age, title_case(&f.title))
        })
        .collect();
    names.sort();
    if names.is_empty() {
        "none".into()
    } else {
        names
            .into_iter()
            .map(|(age, title)| format!("{title} ({age})"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stack_index_follows_board_order() {
        for (i, color) in COLORS.into_iter().enumerate() {
            assert_eq!(stack_index(color), i);
        }
    }

    use super::*;

    #[test]
    fn martian_internet_reads_as_its_printed_text_only_under_that_rule() {
        let id = by_code("A113").unwrap();
        assert_eq!(paragraphs(id, false)[0].text, "The game ends in a draw.");
        assert_eq!(paragraphs(id, true)[0].text, MARTIAN_INTERNET_PRINTED);
    }

    #[test]
    fn reference_faces_preserve_order_slots_and_text() {
        let reference: Reference = serde_json::from_str(FIGURES_TEXT).unwrap();
        let added = &faces()[PRINTED_CARDS..];
        assert_eq!(added.len(), 130);
        assert_eq!(added.iter().filter(|f| f.set == Set::Figures).count(), 115);
        assert_eq!(
            added
                .iter()
                .filter(|f| f.set == Set::Base && f.age == 0)
                .count(),
            15
        );
        let mut food_count = 0;
        for (index, (f, record)) in added.iter().zip(reference.cards).enumerate() {
            assert_eq!(f.code, record.code);
            assert_eq!(
                by_code(&record.code),
                Some((PRINTED_CARDS + index) as FaceId)
            );
            assert_eq!(f.title, record.title);
            assert_eq!(f.age, record.age);
            assert_eq!(f.value, record.age);
            assert_eq!(f.icons.len(), 4, "{}", f.code);
            assert_eq!(f.bonuses.len(), 4, "{}", f.code);
            assert_eq!(f.city_icons, vec![None; 4], "{}", f.code);
            assert!(f.echo_slots.is_empty());
            assert_eq!(record.symbols.iter().filter(|s| *s == "IMAGE").count(), 1);
            assert_eq!(record.symbols[f.image_slot], "IMAGE");
            let expected_food: Vec<_> = record
                .symbols
                .iter()
                .enumerate()
                .filter_map(|(slot, s)| (s == "FOOD").then_some(slot))
                .collect();
            assert_eq!(f.food_slots, expected_food, "{}", f.code);
            food_count += f.food_slots.len();
            for slot in 0..4 {
                let occupied = usize::from(f.icons[slot].is_some())
                    + usize::from(f.bonuses[slot].is_some())
                    + usize::from(f.food_slots.contains(&slot))
                    + usize::from(f.image_slot == slot);
                assert_eq!(occupied, 1, "{} slot {slot}", f.code);
            }
            let expected_text: Vec<_> = record
                .karmas
                .iter()
                .map(|p| p.text.as_str())
                .chain(record.effects.iter().map(|p| p.text.as_str()))
                .collect();
            assert_eq!(
                f.paragraphs
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>(),
                expected_text
            );
        }
        assert_eq!(food_count, 14);
        assert!(by_code("F052").is_some());
    }

    #[test]
    fn figures_keep_bonuses_and_printed_paragraph_order() {
        let hammurabi = face(by_code("F001").unwrap());
        assert_eq!(hammurabi.title, "HAMMURABI");
        assert_eq!(hammurabi.color, CardColor::Red);
        assert_eq!(
            hammurabi.icons,
            vec![Some(Icon::Concept), Some(Icon::Avatar), None, None]
        );
        assert_eq!(hammurabi.bonuses, vec![None, None, None, Some(2)]);
        assert_eq!(hammurabi.image_slot, 2);
        assert_eq!(hammurabi.paragraphs[0].lead, Lead::Karma);
        let polhem = face(by_code("F049").unwrap());
        assert_eq!(polhem.title, "CHRISTOPHER POLHEM");
        assert_eq!(polhem.age, 5);
        assert_eq!(polhem.set, Set::Figures);
        assert_eq!(
            polhem.paragraphs.iter().map(|p| p.lead).collect::<Vec<_>>(),
            vec![Lead::Decree, Lead::Karma]
        );
        assert_eq!(
            polhem.paragraphs[0].text,
            "You may issue an Expansion Decree with any two figures."
        );
        assert_eq!(
            polhem.paragraphs[1].text,
            "Each {[INDUSTRY]} on your board provides one additional point towards your score for every achievement you have."
        );
    }

    #[test]
    fn age_zero_keeps_base_back_food_and_dogma() {
        let water = face(by_code("Z001").unwrap());
        assert_eq!(water.title, "FRESH WATER");
        assert_eq!(water.color, CardColor::Yellow);
        assert_eq!((water.set, water.age, water.value), (Set::Base, 0, 0));
        assert_eq!(water.food_slots, vec![0]);
        assert_eq!(
            water.icons,
            vec![None, Some(Icon::Health), None, Some(Icon::Health)]
        );
        assert_eq!(water.image_slot, 2);
        assert_eq!(water.paragraphs.len(), 1);
        assert_eq!(water.paragraphs[0].lead, Lead::Icon(Icon::Health));
        assert_eq!(
            water.paragraphs[0].text,
            "Splay an unsplayed color on your board left. If you don't, you lose."
        );
        let skinning = face(by_code("Z003").unwrap());
        assert_eq!(skinning.paragraphs[0].lead, Lead::Food);
        assert_eq!(skinning.food_slots, vec![2, 3]);
    }

    #[test]
    #[should_panic(expected = "F001: unknown color")]
    fn unknown_reference_color_names_the_card() {
        let mut reference: Reference = serde_json::from_str(FIGURES_TEXT).unwrap();
        let mut card = reference.cards.remove(0);
        card.color = "OCHRE".into();
        card.into_face();
    }

    #[test]
    #[should_panic(expected = "F001: unknown icon")]
    fn unknown_reference_icon_names_the_card() {
        let mut reference: Reference = serde_json::from_str(FIGURES_TEXT).unwrap();
        let mut card = reference.cards.remove(0);
        card.symbols[0] = serde_json::json!("MYSTERY");
        card.into_face();
    }

    #[test]
    #[should_panic(expected = "F001: unknown symbol")]
    fn unknown_reference_symbol_names_the_card() {
        let mut reference: Reference = serde_json::from_str(FIGURES_TEXT).unwrap();
        let mut card = reference.cards.remove(0);
        card.symbols[0] = serde_json::json!({"mystery": 2});
        card.into_face();
    }

    #[test]
    fn printed_faces_keep_their_stable_order_and_fill_every_slot_once() {
        assert_eq!(faces().len(), PRINTED_CARDS + 130);
        for (id, f) in faces()[..PRINTED_CARDS].iter().enumerate() {
            let set = [
                Set::Base,
                Set::Echoes,
                Set::Cities,
                Set::Unseen,
                Set::Artifacts,
            ][id / 115];
            assert_eq!(f.set, set, "{}", f.code);
            assert_eq!(by_code(&f.code), Some(id as FaceId));
            let slots = if set == Set::Cities { 6 } else { 4 };
            assert_eq!(f.icons.len(), slots, "{}", f.code);
            let mut free = 0;
            for slot in 0..slots {
                let search = f.city_icons[slot] == Some(CityIcon::Search);
                let printed = usize::from(f.icons[slot].is_some())
                    + usize::from(f.bonuses[slot].is_some())
                    + usize::from(f.city_icons[slot].is_some() && !search)
                    + usize::from(f.echo_slots.contains(&slot));
                assert!(printed <= 1, "{} slot {slot}", f.code);
                assert!(!search || f.icons[slot].is_some(), "{} slot {slot}", f.code);
                if printed == 0 {
                    free += 1;
                    assert!(
                        f.image_slot == slot || f.code == "A077",
                        "{} slot {slot}",
                        f.code
                    );
                }
            }
            // Battleship Yamato prints its image across four blank slots.
            assert_eq!(free, if f.code == "A077" { 4 } else { 1 }, "{}", f.code);
        }
        let yamato = face(by_code("A077").unwrap());
        assert_eq!((yamato.age, yamato.value), (8, 11));
        let archery = face(by_code("B001").unwrap());
        assert_eq!(archery.title, "ARCHERY");
        assert_eq!(archery.icons[0], Some(Icon::Authority));
        assert_eq!(archery.image_slot, 2);
    }
}
