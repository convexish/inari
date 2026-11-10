//! Card faces the table draws: every printed card once, keyed by publisher
//! code, so a `FaceId` also names that card on the table. Every set comes
//! from `data/*.json`, in the engine's stable-identity order.
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

/// Every set's card data, in the order the `FaceId`s run: the engine's
/// stable-identity order. Age 0 cards are Base cards with their own codes.
const SETS: [(&str, Set); 7] = [
    (include_str!("../../data/base.json"), Set::Base),
    (include_str!("../../data/echoes.json"), Set::Echoes),
    (include_str!("../../data/cities.json"), Set::Cities),
    (include_str!("../../data/unseen.json"), Set::Unseen),
    (include_str!("../../data/artifacts.json"), Set::Artifacts),
    (include_str!("../../data/figures.json"), Set::Figures),
    (include_str!("../../data/age_zero.json"), Set::Base),
];

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

/// Food, the Age 0 icon, as the data names it. It has no tile of its own.
const FOOD: &str = "FOOD";

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
struct RawKarma {
    kind: String,
    text: String,
}

/// One card of `data/*.json`. Every set shares the schema; a set leaves out
/// the fields it never prints.
#[derive(Deserialize)]
struct RawCard {
    code: String,
    title: String,
    color: String,
    age: u8,
    #[serde(default)]
    face_value: Option<u8>,
    icons: RawIcons,
    #[serde(default)]
    image_slot: Option<usize>,
    #[serde(default)]
    bonuses: Vec<Option<u8>>,
    #[serde(default)]
    city_icons: Vec<Option<CityIcon>>,
    #[serde(default)]
    echoes: Vec<RawEcho>,
    effects: Vec<RawEffect>,
    #[serde(default)]
    karmas: Vec<RawKarma>,
}

impl RawCard {
    fn into_face(self, set: Set) -> Face {
        let code = &self.code;
        let slots = if set == Set::Cities { 6 } else { 4 };
        let names = [
            &self.icons.top_left,
            &self.icons.bottom_left,
            &self.icons.bottom_center,
            &self.icons.bottom_right,
            &self.icons.top_center,
            &self.icons.top_right,
        ];
        let food_slots: Vec<usize> = (0..slots)
            .filter(|&slot| names[slot].as_deref() == Some(FOOD))
            .collect();
        let icons: Vec<Option<Icon>> = names[..slots]
            .iter()
            .map(|name| {
                name.as_deref()
                    .filter(|&name| name != FOOD)
                    .map(|name| card_icon(name, code))
            })
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
            bonuses.len() == slots && city_icons.len() == slots,
            "{code}: expected {slots} slots"
        );
        let echo_slots: Vec<usize> = self.echoes.iter().map(|e| e.slot).collect();
        // The art fills the slot nothing else prints in, unless the data
        // names one (Battleship Yamato prints no icon in any slot).
        let image_slot = self.image_slot.unwrap_or_else(|| {
            (0..slots)
                .find(|&slot| {
                    icons[slot].is_none()
                        && bonuses[slot].is_none()
                        && city_icons[slot].is_none()
                        && !echo_slots.contains(&slot)
                        && !food_slots.contains(&slot)
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
        paragraphs.extend(self.karmas.into_iter().map(|karma| Paragraph {
            lead: match karma.kind.as_str() {
                "would" | "when" | "each" => Lead::Karma,
                "decree" => Lead::Decree,
                kind => panic!("{code}: unknown karma kind {kind:?}"),
            },
            text: karma.text,
        }));
        paragraphs.extend(self.effects.into_iter().map(|e| Paragraph {
            lead: if e.featured_icon == FOOD {
                Lead::Food
            } else {
                Lead::Icon(card_icon(&e.featured_icon, code))
            },
            text: e.text,
        }));
        Face {
            value: self.face_value.unwrap_or(self.age),
            color: color_named(&self.color, code),
            code: self.code,
            title: self.title,
            age: self.age,
            set,
            icons,
            food_slots,
            bonuses,
            city_icons,
            echo_slots,
            image_slot,
            paragraphs,
        }
    }
}

fn raw_cards(json: &str) -> Vec<RawCard> {
    serde_json::from_str(json).expect("card data parses")
}

struct Table {
    faces: Vec<Face>,
    by_code: HashMap<String, FaceId>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let faces: Vec<Face> = SETS
            .into_iter()
            .flat_map(|(json, set)| raw_cards(json).into_iter().map(move |c| c.into_face(set)))
            .collect();
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
/// the engine plays by default; a match playing the printed subgame shows
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
    use inno_engine::{STABLE_IDS, StableId};

    #[test]
    fn martian_internet_reads_as_its_printed_text_only_under_that_rule() {
        let id = by_code("A113").unwrap();
        assert_eq!(paragraphs(id, false)[0].text, "The game ends in a draw.");
        assert_eq!(paragraphs(id, true)[0].text, MARTIAN_INTERNET_PRINTED);
    }

    #[test]
    fn faces_follow_stable_identity_order_and_fill_every_slot_once() {
        assert_eq!(faces().len(), usize::from(STABLE_IDS));
        let mut food_count = 0;
        for (id, f) in faces().iter().enumerate() {
            assert_eq!(
                StableId::from_code(&f.code),
                Some(StableId(id as u16)),
                "{}",
                f.code
            );
            assert_eq!(by_code(&f.code), Some(id as FaceId));
            let age_zero = f.code.starts_with('Z');
            let set = [
                Set::Base,
                Set::Echoes,
                Set::Cities,
                Set::Unseen,
                Set::Artifacts,
                Set::Figures,
                Set::Base,
            ][id / 115];
            assert_eq!(f.set, set, "{}", f.code);
            assert_eq!(f.age == 0, age_zero, "{}", f.code);
            let slots = if set == Set::Cities { 6 } else { 4 };
            assert_eq!(f.icons.len(), slots, "{}", f.code);
            assert_eq!(f.bonuses.len(), slots, "{}", f.code);
            assert_eq!(f.city_icons.len(), slots, "{}", f.code);
            assert!(age_zero || f.food_slots.is_empty(), "{}", f.code);
            assert!(set == Set::Echoes || f.echo_slots.is_empty(), "{}", f.code);
            assert!(
                set == Set::Cities || f.city_icons.iter().all(Option::is_none),
                "{}",
                f.code
            );
            food_count += f.food_slots.len();
            let mut free = 0;
            for slot in 0..slots {
                let search = f.city_icons[slot] == Some(CityIcon::Search);
                let printed = usize::from(f.icons[slot].is_some())
                    + usize::from(f.bonuses[slot].is_some())
                    + usize::from(f.city_icons[slot].is_some() && !search)
                    + usize::from(f.echo_slots.contains(&slot))
                    + usize::from(f.food_slots.contains(&slot));
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
            assert_eq!(
                f.value,
                if f.code == "A077" { 11 } else { f.age },
                "{}",
                f.code
            );
        }
        assert_eq!(food_count, 14);
        let yamato = face(by_code("A077").unwrap());
        assert_eq!((yamato.age, yamato.value, yamato.image_slot), (8, 11, 1));
        let archery = face(by_code("B001").unwrap());
        assert_eq!(archery.title, "ARCHERY");
        assert_eq!(archery.icons[0], Some(Icon::Authority));
        assert_eq!(archery.image_slot, 2);
    }

    #[test]
    fn paragraphs_keep_the_data_order_and_text() {
        for (json, set) in SETS {
            for card in raw_cards(json) {
                let f = face(by_code(&card.code).unwrap());
                assert_eq!(f.set, set, "{}", f.code);
                assert_eq!(f.title, card.title);
                let expected: Vec<_> = card
                    .echoes
                    .iter()
                    .map(|e| e.text.as_str())
                    .chain(card.karmas.iter().map(|k| k.text.as_str()))
                    .chain(card.effects.iter().map(|e| e.text.as_str()))
                    .collect();
                let shown: Vec<_> = f.paragraphs.iter().map(|p| p.text.as_str()).collect();
                assert_eq!(shown, expected, "{}", f.code);
            }
        }
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

    fn first_figure() -> RawCard {
        raw_cards(SETS[5].0).remove(0)
    }

    #[test]
    #[should_panic(expected = "F001: unknown color")]
    fn unknown_color_names_the_card() {
        let mut card = first_figure();
        card.color = "OCHRE".into();
        card.into_face(Set::Figures);
    }

    #[test]
    #[should_panic(expected = "F001: unknown icon")]
    fn unknown_icon_names_the_card() {
        let mut card = first_figure();
        card.icons.top_left = Some("MYSTERY".into());
        card.into_face(Set::Figures);
    }

    #[test]
    #[should_panic(expected = "F001: unknown karma kind")]
    fn unknown_karma_kind_names_the_card() {
        let mut card = first_figure();
        card.karmas[0].kind = "whenever".into();
        card.into_face(Set::Figures);
    }

    #[test]
    #[should_panic(expected = "F001: expected 4 slots")]
    fn a_short_bonus_row_names_the_card() {
        let mut card = first_figure();
        card.bonuses.pop();
        card.into_face(Set::Figures);
    }
}
