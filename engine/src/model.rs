use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Decree, DecreeLocation, Special, SpecialLocation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CardId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    One,
    Two,
}

impl Player {
    pub const ALL: [Self; 2] = [Self::One, Self::Two];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn opponent(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Red,
    Blue,
    Green,
    Yellow,
    Purple,
}

impl Color {
    pub const ALL: [Self; 5] = [
        Self::Red,
        Self::Blue,
        Self::Green,
        Self::Yellow,
        Self::Purple,
    ];

    /// The color's English name, capitalized.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Yellow => "Yellow",
            Self::Purple => "Purple",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Figure {
    Kalashnikov,
    Michelangelo,
    EmperorMeiji,
    JackieChan,
    Nightingale,
    AlexTrebek,
    YiSunSin,
    Homer,
    ShenKuo,
    DukeEllington,
    AlexanderTheGreat,
    EmmaWatson,
    BearGrylls,
    Gilgamesh,
    JamesClerkMaxwell,
    AlfredNobel,
    Archimedes,
    JohannesKepler,
    HedyLamarr,
    GalileoGalilei,
    JohnVonNeumann,
    TranHuangDao,
    MarieCurie,
    Shennong,
    FuXi,
    HuangDi,
    JohnEricsson,
    Sneferu,
    SuSong,
    SejongTheGreat,
    Tigernmas,
    Daedalus,
    Sinuhe,
    WilliamShakespeare,
    ChristopherPolhem,
    Ptolemy,
    JohnLoudonMcAdam,
    CaiLun,
    AntonieVanLeeuwenhoek,
    NiccoloMachiavelli,
    CatherineTheGreat,
    SergeyBrin,
    NelsonMandela,
    Ptahhotep,
    Plato,
    Avicenna,
    HGWells,
    PedroAlvaresCabral,
    CharlesDarwin,
    Pele,
    ShigeruMiyamoto,
    Hammurabi,
    SunshuAo,
    Hatshepsut,
    Imhotep,
    XimenBao,
    AugustusCaesar,
    ZhangHeng,
    Confucius,
    Saladin,
    Rhazes,
    Alhazen,
    AminaSukhera,
    LeonardoDaVinci,
    TakiAdDin,
    Shivaji,
    PeterTheGreat,
    SamuelDeChamplain,
    BartholomewRoberts,
    IsaacNewton,
    BartolomeoCristofori,
    JohannesVermeer,
    NapoleonBonaparte,
    Tecumseh,
    EdwardJenner,
    AdamSmith,
    JohnHarrison,
    CarlFriedrichGauss,
    BenjaminFranklin,
    LudwigVanBeethoven,
    RobertELee,
    RowlandHill,
    GeorgeStephenson,
    QueenVictoria,
    ErwinRommel,
    CaresseCrosby,
    NikolaTesla,
    JPMorgan,
    EmmyNoether,
    AlbertEinstein,
    RuthHandler,
    CheGuevara,
    TaiichiOhno,
    WernherVonBraun,
    RodSerling,
    MargaretThatcher,
    StephenHawking,
    YunaKim,
    SheikhTamim,
    GretaThunberg,
    ThereseKirongozi,
    GiuseppeScionti,
    KonstantinNovoselov,
    Sargon,
    SunTzu,
    MurasakiShikibu,
    AlKindi,
    ChristiaanHuygens,
    GraceHopper,
    MuhammadYunus,
    SusanBlackmore,
    MartinScorsese,
    MaxVerstappen,
    BoyanSlat,
    LilyHevesh,
}

impl Figure {
    pub const ALL: [Self; 115] = [
        Self::Kalashnikov,
        Self::Michelangelo,
        Self::EmperorMeiji,
        Self::JackieChan,
        Self::Nightingale,
        Self::AlexTrebek,
        Self::YiSunSin,
        Self::Homer,
        Self::ShenKuo,
        Self::DukeEllington,
        Self::AlexanderTheGreat,
        Self::EmmaWatson,
        Self::BearGrylls,
        Self::Gilgamesh,
        Self::JamesClerkMaxwell,
        Self::AlfredNobel,
        Self::Archimedes,
        Self::JohannesKepler,
        Self::HedyLamarr,
        Self::GalileoGalilei,
        Self::JohnVonNeumann,
        Self::TranHuangDao,
        Self::MarieCurie,
        Self::Shennong,
        Self::FuXi,
        Self::HuangDi,
        Self::JohnEricsson,
        Self::Sneferu,
        Self::SuSong,
        Self::SejongTheGreat,
        Self::Tigernmas,
        Self::Daedalus,
        Self::Sinuhe,
        Self::WilliamShakespeare,
        Self::ChristopherPolhem,
        Self::Ptolemy,
        Self::JohnLoudonMcAdam,
        Self::CaiLun,
        Self::AntonieVanLeeuwenhoek,
        Self::NiccoloMachiavelli,
        Self::CatherineTheGreat,
        Self::SergeyBrin,
        Self::NelsonMandela,
        Self::Ptahhotep,
        Self::Plato,
        Self::Avicenna,
        Self::HGWells,
        Self::PedroAlvaresCabral,
        Self::CharlesDarwin,
        Self::Pele,
        Self::ShigeruMiyamoto,
        Self::Hammurabi,
        Self::SunshuAo,
        Self::Hatshepsut,
        Self::Imhotep,
        Self::XimenBao,
        Self::AugustusCaesar,
        Self::ZhangHeng,
        Self::Confucius,
        Self::Saladin,
        Self::Rhazes,
        Self::Alhazen,
        Self::AminaSukhera,
        Self::LeonardoDaVinci,
        Self::TakiAdDin,
        Self::Shivaji,
        Self::PeterTheGreat,
        Self::SamuelDeChamplain,
        Self::BartholomewRoberts,
        Self::IsaacNewton,
        Self::BartolomeoCristofori,
        Self::JohannesVermeer,
        Self::NapoleonBonaparte,
        Self::Tecumseh,
        Self::EdwardJenner,
        Self::AdamSmith,
        Self::JohnHarrison,
        Self::CarlFriedrichGauss,
        Self::BenjaminFranklin,
        Self::LudwigVanBeethoven,
        Self::RobertELee,
        Self::RowlandHill,
        Self::GeorgeStephenson,
        Self::QueenVictoria,
        Self::ErwinRommel,
        Self::CaresseCrosby,
        Self::NikolaTesla,
        Self::JPMorgan,
        Self::EmmyNoether,
        Self::AlbertEinstein,
        Self::RuthHandler,
        Self::CheGuevara,
        Self::TaiichiOhno,
        Self::WernherVonBraun,
        Self::RodSerling,
        Self::MargaretThatcher,
        Self::StephenHawking,
        Self::YunaKim,
        Self::SheikhTamim,
        Self::GretaThunberg,
        Self::ThereseKirongozi,
        Self::GiuseppeScionti,
        Self::KonstantinNovoselov,
        Self::Sargon,
        Self::SunTzu,
        Self::MurasakiShikibu,
        Self::AlKindi,
        Self::ChristiaanHuygens,
        Self::GraceHopper,
        Self::MuhammadYunus,
        Self::SusanBlackmore,
        Self::MartinScorsese,
        Self::MaxVerstappen,
        Self::BoyanSlat,
        Self::LilyHevesh,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Authority,
    Concept,
    Health,
    Prosperity,
    Industry,
    Efficiency,
    Avatar,
    Food,
}

impl Icon {
    /// The icon's English name, capitalized.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Authority => "Authority",
            Self::Concept => "Concept",
            Self::Health => "Health",
            Self::Prosperity => "Prosperity",
            Self::Industry => "Industry",
            Self::Efficiency => "Efficiency",
            Self::Avatar => "Avatar",
            Self::Food => "Food",
        }
    }
}

/// Printed symbol positions: top-left, bottom-left, bottom-center,
/// bottom-right, then the Cities-only top-center and top-right. Other sets
/// leave the last two blank.
pub const SLOTS: usize = 6;

/// A four-position layout (every set but Cities), padded to [`SLOTS`].
pub const fn four_slots(symbols: [Symbol; 4]) -> [Symbol; SLOTS] {
    [
        symbols[0],
        symbols[1],
        symbols[2],
        symbols[3],
        Symbol::Blank,
        Symbol::Blank,
    ]
}

/// The nine City icon kinds other than Search. Each is its own generic
/// icon type, never a standard icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl CityIcon {
    pub const ALL: [Self; 9] = [
        Self::Plus,
        Self::Uplift,
        Self::Junk,
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Unsplay,
        Self::Flag,
        Self::Fountain,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Blank,
    Image,
    Icon(Icon),
    Bonus(u8),
    /// A City's Search icon: its printed standard icon, which counts as one
    /// of that type, marked for the Search action.
    Search(Icon),
    City(CityIcon),
    /// An echo effect's printed position (Echoes): effect text, not an icon
    /// of any type. A card prints at most one; its program is the card's
    /// echo effect.
    Echo,
}

impl Symbol {
    /// The standard icon this symbol counts as, including a Search icon's.
    pub const fn standard(self) -> Option<Icon> {
        match self {
            Self::Icon(icon) | Self::Search(icon) => Some(icon),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Splay {
    #[default]
    None,
    Left,
    Right,
    Up,
    Aslant,
}

impl Splay {
    /// The direction's English name, capitalized ("None" unsplayed).
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Up => "Up",
            Self::Aslant => "Aslant",
        }
    }

    /// Whether a covered card shows this [`SLOTS`] position. Top-center is
    /// never shown; top-right (a City's image) only under Left.
    pub const fn reveals(self, slot: usize) -> bool {
        match self {
            Self::None => false,
            Self::Left => slot == 3 || slot == 5,
            Self::Right => slot < 2,
            Self::Up => slot > 0 && slot < 4,
            Self::Aslant => slot < 4,
        }
    }
}

/// Printed sets in stable identity order. Age 0 cards belong to Base
/// but have their own badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardSet {
    Base,
    Echoes,
    Cities,
    Unseen,
    Artifacts,
    Figures,
}

/// Cards per set, and the publisher's Age 0 cards (codes `Z001`–`Z015`).
pub const SET_SIZE: u16 = 115;
pub const AGE_ZERO_SIZE: u16 = 15;
/// Stable identities across every set: 690 set cards, then Age 0.
pub const STABLE_IDS: u16 = 6 * SET_SIZE + AGE_ZERO_SIZE;

impl CardSet {
    pub const ALL: [Self; 6] = [
        Self::Base,
        Self::Echoes,
        Self::Cities,
        Self::Unseen,
        Self::Artifacts,
        Self::Figures,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The publisher code prefix (`B001`, `E001`, ..., `F001`).
    pub const fn prefix(self) -> char {
        match self {
            Self::Base => 'B',
            Self::Echoes => 'E',
            Self::Cities => 'C',
            Self::Unseen => 'U',
            Self::Artifacts => 'A',
            Self::Figures => 'F',
        }
    }
}

/// A printed card's identity in the reserved ranges shared by every mode:
/// Base 0–114, Echoes 115–229, Cities 230–344, Unseen 345–459 and Artifacts
/// 460–574, then Figures 575–689 and Age 0 690–704.
/// Game-local [`CardId`]s index one game's catalog and are not stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableId(pub u16);

impl StableId {
    /// From a publisher code; `None` for codes outside the printed sets.
    pub fn from_code(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        let prefix = chars.next()?;
        let digits = chars.as_str();
        if digits.len() != 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let number: u16 = digits.parse().ok()?;
        let (first, size) = if prefix == 'Z' {
            (6 * SET_SIZE, AGE_ZERO_SIZE)
        } else {
            let set = CardSet::ALL
                .into_iter()
                .find(|set| set.prefix() == prefix)?;
            (set.index() as u16 * SET_SIZE, SET_SIZE)
        };
        (1..=size)
            .contains(&number)
            .then(|| Self(first + number - 1))
    }
}

/// Physical decks have both an expansion and a printed age. Effective values
/// and draw routing must not conflate these with a card's current location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deck {
    pub set: CardSet,
    pub age: u8,
}

impl Deck {
    /// Ages 0 through 11 per set.
    pub const AGES: usize = 12;

    /// Dense index over every set's decks: `set * 12 + age`.
    pub const fn index(self) -> usize {
        self.set.index() * Self::AGES + self.age as usize
    }

    pub const fn base(age: u8) -> Self {
        Self {
            set: CardSet::Base,
            age,
        }
    }
    pub const fn figures(age: u8) -> Self {
        Self {
            set: CardSet::Figures,
            age,
        }
    }
    pub const fn cities(age: u8) -> Self {
        Self {
            set: CardSet::Cities,
            age,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub code: &'static str,
    /// The printed set: with the printed age, the card's public back.
    pub set: CardSet,
    pub color: Color,
    /// Printed value, which is also the printed age on every current card.
    pub value: u8,
    pub figure: Option<Figure>,
    pub symbols: [Symbol; SLOTS],
    /// Printed featured icon, independent of the action's optional Auspice.
    pub featured: Option<Icon>,
    /// Dogma effects in printed order.
    pub effects: Vec<Effect>,
    /// The printed echo effect (Echoes), always a non-demand effect. It is
    /// effect [`Card::ECHO`], beside the dogma effects' printed indices.
    /// Keep its program out of the common record, including Base catalogs.
    pub echo: Option<Box<Effect>>,
}

impl Card {
    /// The effect index naming a card's echo effect. A card prints at most
    /// one echo; dogma effects keep their printed indices 0, 1 and 2.
    pub const ECHO: u16 = u16::MAX;

    /// Whether this card's echo shows when it is covered in a pile with this
    /// splay (on top, every printed position shows).
    pub(crate) fn echo_shown(&self, splay: Splay) -> bool {
        self.echo.is_some()
            && self
                .printed()
                .iter()
                .position(|&symbol| symbol == Symbol::Echo)
                .is_some_and(|slot| splay.reveals(slot))
    }

    /// A dogma effect by printed index, or the echo effect at [`Card::ECHO`].
    pub fn effect(&self, index: u16) -> Option<&Effect> {
        if index == Self::ECHO {
            self.echo.as_deref()
        } else {
            self.effects.get(usize::from(index))
        }
    }

    /// The printed positions: all six on a City, the first four elsewhere.
    pub fn printed(&self) -> &[Symbol] {
        let slots = if self.set == CardSet::Cities {
            SLOTS
        } else {
            4
        };
        &self.symbols[..slots]
    }

    /// The printed position of the card's image, if it has one.
    pub fn image_slot(&self) -> Option<usize> {
        self.printed()
            .iter()
            .position(|&symbol| symbol == Symbol::Image)
    }

    /// The printed bonus, if any (a card with several repeats one value).
    pub fn bonus(&self) -> Option<u8> {
        self.printed().iter().find_map(|&symbol| match symbol {
            Symbol::Bonus(value) => Some(value),
            _ => None,
        })
    }

    /// Visit every printed symbol: the four shared positions (a fixed loop
    /// the compiler unrolls), then a City's two.
    #[inline]
    fn visit_printed(&self, mut visit: impl FnMut(Symbol)) {
        for slot in 0..4 {
            visit(self.symbols[slot]);
        }
        if self.set == CardSet::Cities {
            for slot in 4..SLOTS {
                visit(self.symbols[slot]);
            }
        }
    }

    /// Visit the symbols this card shows: all of them on top, else those its
    /// pile's splay reveals.
    #[inline]
    pub(crate) fn visit_shown(&self, top: bool, splay: Splay, visit: impl FnMut(Symbol)) {
        if top {
            self.visit_printed(visit);
        } else {
            self.visit_covered(splay, visit);
        }
    }

    /// Visit the symbols a covered card shows through its pile's splay.
    #[inline]
    fn visit_covered(&self, splay: Splay, mut visit: impl FnMut(Symbol)) {
        for slot in 0..4 {
            if splay.reveals(slot) {
                visit(self.symbols[slot]);
            }
        }
        if self.set == CardSet::Cities {
            for slot in 4..SLOTS {
                if splay.reveals(slot) {
                    visit(self.symbols[slot]);
                }
            }
        }
    }

    pub fn printed_icon_count(&self, icon: Icon) -> usize {
        let mut count = 0;
        self.visit_printed(|symbol| count += usize::from(symbol.standard() == Some(icon)));
        count
    }

    /// The value printed on the face. Every card but Battleship Yamato (A077)
    /// prints its age: Yamato's back and deck are age 8, its face says 11.
    /// [`Position::value`] decides which applies where the card lies.
    #[inline]
    pub fn face_value(&self) -> u8 {
        if self.set == CardSet::Artifacts && self.value == 8 && self.code == "A077" {
            11
        } else {
            self.value
        }
    }

    /// Dancing Girl (A004), whose action scope remembers the boards it lies on.
    pub(crate) fn is_dancing_girl(&self) -> bool {
        self.set == CardSet::Artifacts && self.value == 1 && self.code == "A004"
    }

    /// The public back and origin deck: printed set and age.
    pub fn origin_deck(&self) -> Deck {
        Deck {
            set: self.set,
            age: self.value,
        }
    }

    /// The reserved identity of a printed card, if this is one.
    pub fn stable_id(&self) -> Option<StableId> {
        StableId::from_code(self.code)
    }
}

/// A printed dogma effect's kind. A compel (Artifacts) is a demand that
/// eligible sharers execute instead of vulnerable opponents; the owner never
/// executes it and no one earns a sharing reward from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    NonDemand,
    Demand,
    Compel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub kind: EffectKind,
    pub instructions: Vec<crate::Instruction>,
}

impl EffectKind {
    #[inline]
    pub const fn from_demand(demand: bool) -> Self {
        if demand {
            Self::Demand
        } else {
            Self::NonDemand
        }
    }
}

impl Effect {
    /// A demand or compel: never shared, counted by every demand predicate.
    #[inline]
    pub fn demand(&self) -> bool {
        self.kind != EffectKind::NonDemand
    }
}

/// Invocation-scoped modification of printed boxed values. Ordinary counts,
/// control-flow offsets and computed ages do not pass through this scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoxedValues {
    increase: u8,
}

impl BoxedValues {
    pub(crate) const ZERO: Self = Self { increase: 0 };

    pub(crate) fn increase(&mut self, amount: u8) -> Result<(), Error> {
        self.increase = self
            .increase
            .checked_add(amount)
            .ok_or(Error::Unsupported("boxed adjustment overflow"))?;
        Ok(())
    }

    pub(crate) fn value(self, literal: u8) -> Result<u8, Error> {
        literal
            .checked_add(self.increase)
            .ok_or(Error::Unsupported("boxed value overflow"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Meld,
    Score,
    Return,
    Transfer,
    Junk,
    Tuck,
    Achieve,
    /// Echoes: put a card into a Forecast. Capacity can prevent it.
    Foreshadow,
    /// Unseen: put a card into a Safe as a secret. Capacity can prevent it.
    Safeguard,
    /// Place onto the origin deck's top, without performing Return.
    Place,
    /// Artifacts: a Meld action digs the top of an Artifact deck onto the
    /// player's display. A rule consequence, never a draw.
    Dig,
    /// Artifacts: a showcased Artifact rotates from the display into a
    /// Museum, or into hand when no Museum is available.
    Rotate,
    /// Artifacts: instead of digging, take an opponent's Museum together
    /// with the Artifact stored in it.
    Seize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Hand(Player),
    Score(Player),
    Achievements(Player),
    AvailableAchievements,
    /// Outside the game until Bear Grylls introduces this immutable roster.
    AgeZeroReserve,
    Board(Player, Color),
    Deck(Deck),
    Junk,
    /// A player's Forecast (Echoes): an unordered pile only its owner may
    /// look at. Unlike a hand, it is not where Meld actions choose from.
    Forecast(Player),
    /// A player's display (Artifacts): at most one public dug Artifact,
    /// showcased at the start of its owner's next turn.
    Display(Player),
    /// A player's Museums (Artifacts): each stored Artifact occupies one of
    /// the five Museums. Public, unordered; a Meld action may take one.
    Museum(Player),
    /// A player's Safe (Unseen): an ordered pile of secrets hidden from
    /// everyone, its owner included. Capacity follows the owner's best splay.
    Safe(Player),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalZone {
    Hand(Player),
    Score(Player),
    /// Explicit authorization for a selected identity anywhere in this stack.
    BoardDepth(Player, Color),
    /// Standard supply plus this player's live board aliases, never specials.
    AvailableAchievements(Player),
    Physical(Zone),
}

impl LogicalZone {
    /// The physical zone it names; Figure aliases may add others.
    pub(crate) fn home(self) -> Zone {
        match self {
            Self::Hand(p) => Zone::Hand(p),
            Self::Score(p) => Zone::Score(p),
            Self::BoardDepth(p, color) => Zone::Board(p, color),
            Self::AvailableAchievements(_) => Zone::AvailableAchievements,
            Self::Physical(zone) => zone,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub card: CardId,
    pub physical: Zone,
    pub logical: LogicalZone,
}

/// Native action requests, not a policy encoding. Card identities in these
/// requests must not be used to expose hidden achievement fronts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Draw,
    Meld {
        card: CardId,
    },
    Achieve {
        card: CardId,
    },
    Dogma {
        card: CardId,
        auspice: bool,
    },
    /// The specific Decree is chosen only after hand payment finishes.
    Decree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Regular,
    Free,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidSetup(&'static str),
    Unsupported(&'static str),
    Busy,
    NoDecision,
    InvalidChoice,
    InvalidAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Board {
    pub hand: Vec<CardId>,
    pub score: Vec<CardId>,
    pub achievements: Vec<CardId>,
    pub stacks: [Vec<CardId>; 5],
    pub splays: [Splay; 5],
}

impl Default for Board {
    fn default() -> Self {
        Self {
            hand: Vec::new(),
            score: Vec::new(),
            achievements: Vec::new(),
            stacks: std::array::from_fn(|_| Vec::new()),
            splays: [Splay::None; 5],
        }
    }
}

/// The expansions a game plays with, a runtime rules profile: one build
/// serves every combination. Base is always included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expansions(u8);

impl Expansions {
    pub const BASE: Self = Self(0);
    pub const FIGURES: Self = Self(1);
    pub const CITIES: Self = Self(2);
    pub const ECHOES: Self = Self(4);
    pub const ARTIFACTS: Self = Self(8);
    /// Unseen: the Safe, secrets, five specials and Unseen decks.
    pub const UNSEEN: Self = Self(16);
    /// Every combination of the five runtime expansions.
    pub const COMBINATIONS: usize = 32;

    /// A dense index over the combinations.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The combination with this [`Expansions::index`]; `None` past the
    /// five runtime expansion bits.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if (bits as usize) < Self::COMBINATIONS {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// Both profiles' expansions.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Enabled expansions, not counting Base.
    pub const fn count(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Base, plus Figures if `figures`.
    pub const fn with_figures(figures: bool) -> Self {
        if figures { Self::FIGURES } else { Self::BASE }
    }

    pub const fn figures(self) -> bool {
        self.0 & Self::FIGURES.0 != 0
    }

    pub const fn cities(self) -> bool {
        self.0 & Self::CITIES.0 != 0
    }

    pub const fn echoes(self) -> bool {
        self.0 & Self::ECHOES.0 != 0
    }

    pub const fn artifacts(self) -> bool {
        self.0 & Self::ARTIFACTS.0 != 0
    }

    pub const fn unseen(self) -> bool {
        self.0 & Self::UNSEEN.0 != 0
    }

    /// Whether this printed set's cards belong to these rules.
    pub const fn includes(self, set: CardSet) -> bool {
        match set {
            CardSet::Base => true,
            CardSet::Figures => self.figures(),
            CardSet::Cities => self.cities(),
            CardSet::Echoes => self.echoes(),
            CardSet::Artifacts => self.artifacts(),
            CardSet::Unseen => self.unseen(),
        }
    }
}

/// Artifacts' auxiliary state. The five Museums are identical pieces, so
/// each is only counted: stored Artifacts and claimed Museums together never
/// exceed [`Artifacts::MUSEUMS`], and the rest wait in the supply.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Artifacts {
    pub display: [Vec<CardId>; 2],
    pub museums: [Vec<CardId>; 2],
    /// Museums each player claimed as achievements, once the Artifact that
    /// was stored in it has been returned.
    pub claimed: [u8; 2],
    /// Museums junked: with the Artifact stored in them, or as an
    /// eliminated civilization's claimed achievements.
    pub junked: u8,
    /// A Museum Check's award before its returns: one of this player's
    /// occupied Museums already counts as their achievement.
    pub pending: Option<Player>,
    /// An explicitly revealed card whose face differs from its back (Battleship Yamato)
    /// counts its face value until it moves or the revealing program ends.
    pub revealed: Option<CardId>,
}

impl Artifacts {
    pub const MUSEUMS: usize = 5;

    pub(crate) fn is_empty(&self) -> bool {
        self.display.iter().all(Vec::is_empty)
            && self.museums.iter().all(Vec::is_empty)
            && self.claimed == [0; 2]
            && self.junked == 0
            && self.pending.is_none()
            && self.revealed.is_none()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AgeZero {
    pub reserve: Vec<CardId>,
    pub introduced: bool,
}

/// Physical state and shared immutable programs. Use `new` for explicit
/// acceptance positions, or `Game::new_base` for a complete Base deal.
#[derive(Clone, Debug, Eq)]
pub struct Position {
    pub(crate) catalog: Arc<[Card]>,
    /// Set by the game that plays this position. Equality ignores it and
    /// the caches derived under it: the same cards compare equal.
    pub(crate) expansions: Expansions,
    pub(crate) players: [Board; 2],
    pub(crate) decks: [Vec<CardId>; 12],
    /// Each other set's decks, by [`CardSet::index`] minus one. A set
    /// without cards in the catalog allocates none.
    pub(crate) set_decks: [Option<Box<[Vec<CardId>; 12]>>; 5],
    /// Rare expansion state; no reserve allocation in Base positions.
    pub(crate) age_zero: Option<Box<AgeZero>>,
    /// Each player's Forecast, allocated when a card first enters one.
    pub(crate) forecast: Option<Box<[Vec<CardId>; 2]>>,
    /// Each player's Safe, allocated when a card first enters one.
    pub(crate) safe: Option<Box<[Vec<CardId>; 2]>>,
    /// Displays, Museums and claimed Museums, allocated on first use.
    pub(crate) artifacts: Option<Box<Artifacts>>,
    pub(crate) junk: Vec<CardId>,
    pub(crate) available_achievements: Vec<CardId>,
    /// Indexed by [`Special::index`]; only the enabled roster is meaningful.
    pub(crate) specials: [SpecialLocation; Special::RESERVED],
    /// Available Decrees are implicit; allocate ownership only when claimed.
    pub(crate) decrees: Option<Box<[DecreeLocation; 5]>>,
    pub(crate) turn_owner: Player,
    pub(crate) regular_actions_started: u8,
    pub(crate) regular_action_limit: u8,
    /// Cards each player tucked and scored this turn (Sunshu Ao).
    pub(crate) turn_moves: [[u8; 2]; 2],
    /// Each player has drawn a card this turn (Unseen replaces a player's
    /// first draw of a turn, on either player's turn).
    pub(crate) first_drawn: [bool; 2],
    /// The turn's one Cities endorsement has been used.
    pub(crate) endorsed: bool,
    /// Explicit viewing grants, independent of logical membership. Native
    /// knowledge will replace this narrow live-visibility fixture mechanism.
    pub(crate) hand_grants: [[bool; 2]; 2],
    /// Live inspection grants `[viewer][owner]` from running executions
    /// (Hitchhiking): the viewer sees the owner's hand, score and Forecast.
    pub(crate) inspections: [[bool; 2]; 2],
    /// Caches derived from the boards, refreshed by [`Self::refresh_board`]
    /// after any pile or splay change: Figures on top of each board,
    figure_tops: [FigureSet; 2],
    /// The trigger kinds ([`crate::On::bit`]) those Figures' Would clauses
    /// watch, refreshed with them.
    watched: [u32; 2],
    /// and each board's visible symbols.
    symbols: [BoardSymbols; 2],
    /// Where each physical card lies, by [`CardId`] (`None` outside the
    /// game), and its value there. Single movements update it;
    /// [`Self::refresh_boards`] rebuilds it.
    locations: Box<[Located]>,
    /// Dancing Girl's identity when the catalog has it, derived from the
    /// catalog: its action scope remembers the boards it lies on.
    pub(crate) dancing_girl: Option<CardId>,
    /// Cards a suspended parent game holds while this subgame runs (Martian
    /// Internet's printed rule): in no zone here, by [`CardId`]. `None` in an
    /// ordinary game.
    pub(crate) absent: Option<Box<[bool]>>,
    /// Renewed from [`GENERATIONS`] by every [`Self::zone_mut`], so a tracked
    /// position keeps one value only while every zone holds the same cards.
    generation: u64,
}

/// Unique [`Position::generation`] values across every position and thread.
static GENERATIONS: AtomicU64 = AtomicU64::new(0);

fn fresh_generation() -> u64 {
    GENERATIONS.fetch_add(1, Ordering::Relaxed)
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        // Exhaustive, so a new field must be considered here.
        let Self {
            catalog,
            expansions: _,
            players,
            decks,
            set_decks,
            age_zero,
            forecast: _,
            safe: _,
            artifacts: _,
            junk,
            available_achievements,
            specials,
            decrees,
            turn_owner,
            regular_actions_started,
            regular_action_limit,
            turn_moves,
            first_drawn,
            endorsed,
            hand_grants,
            inspections,
            absent,
            // Caches derived from the boards under the rules.
            figure_tops: _,
            watched: _,
            symbols: _,
            locations: _,
            dancing_girl: _,
            generation: _,
        } = self;
        // `Arc` skips comparing shared contents only when they are sized,
        // so a shared catalog of card programs needs its own shortcut.
        (Arc::ptr_eq(catalog, &other.catalog) || *catalog == other.catalog)
            && *players == other.players
            && *decks == other.decks
            && *set_decks == other.set_decks
            && *age_zero == other.age_zero
            // Unallocated storage equals an empty Forecast or Safe.
            && Player::ALL.into_iter().all(|p| {
                self.cards(Zone::Forecast(p)) == other.cards(Zone::Forecast(p))
                    && self.cards(Zone::Safe(p)) == other.cards(Zone::Safe(p))
            })
            // Unallocated Artifacts storage equals empty storage.
            && Player::ALL.into_iter().all(|p| {
                self.cards(Zone::Display(p)) == other.cards(Zone::Display(p))
                    && self.cards(Zone::Museum(p)) == other.cards(Zone::Museum(p))
                    && self.claimed_museums(p) == other.claimed_museums(p)
            })
            && self.junked_museums() == other.junked_museums()
            && *junk == other.junk
            && *available_achievements == other.available_achievements
            && *specials == other.specials
            && *decrees == other.decrees
            && *turn_owner == other.turn_owner
            && *regular_actions_started == other.regular_actions_started
            && *regular_action_limit == other.regular_action_limit
            && *turn_moves == other.turn_moves
            && *first_drawn == other.first_drawn
            && *endorsed == other.endorsed
            && *hand_grants == other.hand_grants
            && *inspections == other.inspections
            && *absent == other.absent
    }
}

impl Position {
    pub fn new(
        catalog: Vec<Card>,
        placements: &[(CardId, Zone)],
        turn_owner: Player,
    ) -> Result<Self, Error> {
        if catalog.len() > usize::from(u16::MAX)
            || catalog
                .iter()
                .any(|c| c.value > 11 || (c.value == 0 && c.set != CardSet::Base))
        {
            return Err(Error::InvalidSetup("catalog domain"));
        }
        let mut position = Self::empty(catalog.into(), turn_owner);
        let mut seen = vec![false; position.catalog.len()];
        for &(id, zone) in placements {
            let i = usize::from(id.0);
            if i >= seen.len() || seen[i] {
                return Err(Error::InvalidSetup("duplicate or unknown physical card"));
            }
            let card = position.card(id);
            if matches!(zone, Zone::Board(_, color) if card.color != color)
                || matches!(zone, Zone::Deck(deck) if deck.age > 11 || deck != card.origin_deck())
                || (zone == Zone::AgeZeroReserve && card.value != 0)
            {
                return Err(Error::InvalidSetup(
                    "wrong board color, origin deck or Age 0 reserve",
                ));
            }
            if card.value == 0 && zone != Zone::AgeZeroReserve {
                position
                    .age_zero
                    .as_mut()
                    .expect("Age 0 storage")
                    .introduced = true;
            }
            seen[i] = true;
            position.zone_mut(zone).push(id);
        }
        if seen.iter().any(|present| !present) {
            return Err(Error::InvalidSetup("unplaced physical card"));
        }
        if position
            .age_zero
            .as_ref()
            .is_some_and(|a| a.introduced && !a.reserve.is_empty())
        {
            return Err(Error::InvalidSetup(
                "mixed introduced and dormant Age 0 cards",
            ));
        }
        position.refresh_boards();
        Ok(position)
    }

    /// An explicit position with this pile already splayed, as play would
    /// leave it: no operation runs, so no rule or karma reacts.
    pub fn with_splay(mut self, player: Player, color: Color, splay: Splay) -> Result<Self, Error> {
        if splay != Splay::None && self.cards(Zone::Board(player, color)).len() < 2 {
            return Err(Error::InvalidSetup("a splayed pile needs two cards"));
        }
        self.players[player.index()].splays[color as usize] = splay;
        self.refresh_board(player);
        Ok(self)
    }

    /// Construct storage before a trusted setup places every catalog identity.
    pub(crate) fn empty(catalog: Arc<[Card]>, turn_owner: Player) -> Self {
        let locations: Box<[Located]> = catalog
            .iter()
            .map(|card| Located {
                zone: None,
                value: card.value,
            })
            .collect();
        let set_decks = std::array::from_fn(|i| {
            let set = CardSet::ALL[i + 1];
            catalog
                .iter()
                .any(|c| c.set == set)
                .then(|| Box::new(std::array::from_fn(|_| Vec::new())))
        });
        let age_zero = catalog
            .iter()
            .any(|c| c.value == 0)
            .then(|| Box::new(AgeZero::default()));
        let dancing_girl = catalog
            .iter()
            .position(Card::is_dancing_girl)
            .map(|index| CardId(index as u16));
        Self {
            catalog,
            expansions: Expansions::BASE,
            players: std::array::from_fn(|_| Board::default()),
            decks: std::array::from_fn(|_| Vec::new()),
            set_decks,
            age_zero,
            forecast: None,
            safe: None,
            artifacts: None,
            junk: Vec::new(),
            available_achievements: Vec::new(),
            specials: [SpecialLocation::Available; Special::RESERVED],
            decrees: None,
            turn_owner,
            regular_actions_started: 0,
            regular_action_limit: 2,
            turn_moves: [[0; 2]; 2],
            first_drawn: [false; 2],
            endorsed: false,
            hand_grants: [[true, false], [false, true]],
            inspections: [[false; 2]; 2],
            figure_tops: [FigureSet::default(); 2],
            watched: [0; 2],
            symbols: [BoardSymbols::default(); 2],
            locations,
            dancing_girl,
            absent: None,
            generation: fresh_generation(),
        }
    }

    pub fn expansions(&self) -> Expansions {
        self.expansions
    }

    /// Every deck of the enabled sets, including empty decks and Base age 0.
    pub(crate) fn enabled_decks(&self) -> impl Iterator<Item = Deck> + use<> {
        let expansions = self.expansions;
        CardSet::ALL
            .into_iter()
            .filter(move |&set| expansions.includes(set))
            .flat_map(|set| (0..Deck::AGES as u8).map(move |age| Deck { set, age }))
    }

    /// This position under those expansions' rules (a game sets this).
    pub fn with_expansions(mut self, expansions: Expansions) -> Self {
        self.expansions = expansions;
        self.refresh_boards();
        self
    }

    /// Figures rules apply (the game enabled the expansion).
    pub(crate) fn figures(&self) -> bool {
        self.expansions.figures()
    }

    pub fn age_zero_introduced(&self) -> bool {
        self.age_zero.as_ref().is_some_and(|a| a.introduced)
    }

    pub fn card(&self, id: CardId) -> &Card {
        &self.catalog[usize::from(id.0)]
    }

    pub fn cards(&self, zone: Zone) -> &[CardId] {
        match zone {
            Zone::Hand(p) => &self.players[p.index()].hand,
            Zone::Score(p) => &self.players[p.index()].score,
            Zone::Achievements(p) => &self.players[p.index()].achievements,
            Zone::AvailableAchievements => &self.available_achievements,
            Zone::AgeZeroReserve => self.age_zero.as_ref().map_or(&[], |a| a.reserve.as_slice()),
            Zone::Board(p, c) => &self.players[p.index()].stacks[c as usize],
            Zone::Deck(Deck {
                set: CardSet::Base,
                age,
            }) => &self.decks[usize::from(age)],
            // Sets without cards in this catalog have no deck storage.
            Zone::Deck(Deck { set, age }) => self.set_decks[set.index() - 1]
                .as_ref()
                .map_or(&[], |decks| &decks[usize::from(age)]),
            Zone::Junk => &self.junk,
            Zone::Forecast(p) => self
                .forecast
                .as_ref()
                .map_or(&[], |forecast| forecast[p.index()].as_slice()),
            Zone::Safe(p) => self
                .safe
                .as_ref()
                .map_or(&[], |safe| safe[p.index()].as_slice()),
            Zone::Display(p) => self
                .artifacts
                .as_ref()
                .map_or(&[], |artifacts| artifacts.display[p.index()].as_slice()),
            Zone::Museum(p) => self
                .artifacts
                .as_ref()
                .map_or(&[], |artifacts| artifacts.museums[p.index()].as_slice()),
        }
    }

    pub(crate) fn zone_mut(&mut self, zone: Zone) -> &mut Vec<CardId> {
        self.generation = fresh_generation();
        match zone {
            Zone::Hand(p) => &mut self.players[p.index()].hand,
            Zone::Score(p) => &mut self.players[p.index()].score,
            Zone::Achievements(p) => &mut self.players[p.index()].achievements,
            Zone::AvailableAchievements => &mut self.available_achievements,
            Zone::AgeZeroReserve => &mut self.age_zero.as_mut().expect("Age 0 storage").reserve,
            Zone::Board(p, c) => &mut self.players[p.index()].stacks[c as usize],
            Zone::Deck(Deck {
                set: CardSet::Base,
                age,
            }) => &mut self.decks[usize::from(age)],
            Zone::Deck(Deck { set, age }) => &mut self
                .set_decks_mut(set)
                .unwrap_or_else(|| unreachable!("{set:?} has no cards in this catalog"))
                [usize::from(age)],
            Zone::Junk => &mut self.junk,
            Zone::Forecast(p) => &mut self.forecast.get_or_insert_default()[p.index()],
            Zone::Safe(p) => &mut self.safe.get_or_insert_default()[p.index()],
            Zone::Display(p) => &mut self.artifacts.get_or_insert_default().display[p.index()],
            Zone::Museum(p) => &mut self.artifacts.get_or_insert_default().museums[p.index()],
        }
    }

    /// The Artifact on this player's display, to showcase at their turn's
    /// start.
    pub fn showcase_card(&self, player: Player) -> Option<CardId> {
        self.cards(Zone::Display(player)).first().copied()
    }

    /// Whether the card can be dogma'd at all: never a City, Figure or
    /// Battleship Yamato, which have no Dogma effects or featured icon.
    pub fn can_dogma(&self, card: CardId) -> bool {
        let card = self.card(card);
        card.figure.is_none() && !card.effects.is_empty() && card.featured.is_some()
    }

    /// Meld-action candidates stored in this player's Museums.
    pub fn museum_melds(&self, player: Player) -> impl Iterator<Item = Target> + '_ {
        let physical = Zone::Museum(player);
        self.cards(physical).iter().map(move |&card| Target {
            card,
            physical,
            logical: LogicalZone::Physical(physical),
        })
    }

    /// A Museum Check may award this player one of their occupied Museums.
    pub fn can_claim_museum(&self, player: Player) -> bool {
        !self.cards(Zone::Museum(player)).is_empty()
            && self.artifacts.as_ref().is_none_or(|a| a.pending.is_none())
    }

    /// Museums this player claimed as achievements, including one awarded
    /// by a Museum Check whose returns are still running.
    #[inline]
    pub fn claimed_museums(&self, player: Player) -> usize {
        self.artifacts.as_ref().map_or(0, |artifacts| {
            usize::from(artifacts.claimed[player.index()])
                + usize::from(artifacts.pending == Some(player))
        })
    }

    /// Museums in the supply: the five less those storing an Artifact,
    /// claimed as an achievement or junked.
    #[inline]
    pub fn available_museums(&self) -> usize {
        self.artifacts
            .as_ref()
            .map_or(Artifacts::MUSEUMS, |artifacts| {
                Artifacts::MUSEUMS
                    - artifacts.museums.iter().map(Vec::len).sum::<usize>()
                    - artifacts
                        .claimed
                        .iter()
                        .map(|&n| usize::from(n))
                        .sum::<usize>()
                    - usize::from(artifacts.junked)
            })
    }

    /// Museums that left the game, with their stored Artifact or with an
    /// eliminated civilization.
    pub fn junked_museums(&self) -> usize {
        self.artifacts
            .as_ref()
            .map_or(0, |artifacts| usize::from(artifacts.junked))
    }

    /// The Museum whose stored Artifact was just junked leaves with it.
    pub(crate) fn junk_emptied_museum(&mut self) {
        self.artifacts.get_or_insert_default().junked += 1;
    }

    /// Elimination: this player's claimed Museums leave the game.
    pub(crate) fn junk_claimed_museums(&mut self, player: Player) {
        if let Some(artifacts) = self.artifacts.as_mut() {
            let claimed = std::mem::take(&mut artifacts.claimed[player.index()]);
            artifacts.junked += claimed;
            if artifacts.pending == Some(player) {
                artifacts.pending = None;
                artifacts.junked += 1;
            }
        }
    }

    /// Award one of this player's occupied Museums (a Museum Check's claim,
    /// before its returns). The stored Artifact stays until returned; the
    /// Museum joins the claimed count when [`Self::settle_museum_claim`] runs.
    pub(crate) fn claim_museum(&mut self, player: Player) -> Result<(), Error> {
        if !self.can_claim_museum(player) {
            return Err(Error::Unsupported("no occupied Museum to claim"));
        }
        self.artifacts.get_or_insert_default().pending = Some(player);
        Ok(())
    }

    /// After a Museum Check's returns: the awarded Museum is now empty and
    /// stays with its claimant.
    pub(crate) fn settle_museum_claim(&mut self) {
        if let Some(artifacts) = self.artifacts.as_mut()
            && let Some(player) = artifacts.pending.take()
        {
            artifacts.claimed[player.index()] += 1;
        }
    }

    /// A card's value where it lies: its face value on a board, display or
    /// in a Museum, otherwise its printed age (only Yamato differs). Cached
    /// with the card's location, so a read costs what the printed value does.
    #[inline]
    pub fn value(&self, card: CardId) -> u8 {
        let located = self.locations[usize::from(card.0)];
        debug_assert_eq!(located.value, self.value_in(card, located.zone));
        located.value
    }

    fn value_in(&self, card: CardId, zone: Option<Zone>) -> u8 {
        let printed = self.card(card);
        match zone {
            Some(Zone::Board(..) | Zone::Display(_) | Zone::Museum(_)) => printed.face_value(),
            _ if self.revealed_card() == Some(card) => printed.face_value(),
            _ => printed.value,
        }
    }

    /// The explicitly revealed card currently counting its face value.
    pub(crate) fn revealed_card(&self) -> Option<CardId> {
        self.artifacts.as_ref()?.revealed
    }

    /// An explicit reveal of a card whose face differs from its back: it
    /// counts its face value until it moves or [`Self::end_reveal`]. Any
    /// other card's reveal changes nothing; the result says whether a
    /// scope began.
    pub(crate) fn reveal_face(&mut self, card: CardId) -> bool {
        let printed = self.card(card);
        let face = printed.face_value();
        if face == printed.value {
            return false;
        }
        self.artifacts.get_or_insert_default().revealed = Some(card);
        self.locations[usize::from(card.0)].value = face;
        true
    }

    /// The reveal's scope ended: the card counts its back again where it lies.
    pub(crate) fn end_reveal(&mut self) {
        if let Some(artifacts) = self.artifacts.as_mut()
            && let Some(card) = artifacts.revealed.take()
        {
            let zone = self.locations[usize::from(card.0)].zone;
            self.locations[usize::from(card.0)].value = self.value_in(card, zone);
        }
    }

    /// An expansion set's decks, if the catalog has its cards.
    pub(crate) fn set_decks_mut(&mut self, set: CardSet) -> Option<&mut [Vec<CardId>; 12]> {
        self.set_decks[set.index().checked_sub(1)?].as_deref_mut()
    }

    pub fn top(&self, player: Player, color: Color) -> Option<CardId> {
        // Hot in every end-of-action check: index the pile, not `cards`.
        self.players[player.index()].stacks[color as usize]
            .last()
            .copied()
    }

    /// The echo effects that run with this card, bottom to top: its pile's visible
    /// echoes when it is a top card on any board, otherwise only its own printed echo.
    pub(crate) fn visible_echoes(&self, card: CardId) -> Vec<CardId> {
        if let Some(Zone::Board(player, color)) = self.locate(card)
            && self.top(player, color) == Some(card)
        {
            let splay = self.splay(player, color);
            let pile = self.cards(Zone::Board(player, color));
            return pile
                .iter()
                .enumerate()
                .filter(|&(depth, &id)| {
                    let card = self.card(id);
                    card.echo.is_some() && (depth + 1 == pile.len() || card.echo_shown(splay))
                })
                .map(|(_, &id)| id)
                .collect();
        }
        if self.card(card).echo.is_some() {
            vec![card]
        } else {
            Vec::new()
        }
    }

    /// How many cards this player's Forecast may hold: 5, 4, 3, 2 or 1 for their best
    /// splay of None, Left, Right, Up or Aslant. A lower limit never ejects cards
    /// already there.
    pub fn forecast_limit(&self, player: Player) -> usize {
        self.splay_capacity(player)
    }

    /// How many secrets this player's Safe may hold: the same best-splay rule as the
    /// Forecast.
    pub fn safe_limit(&self, player: Player) -> usize {
        self.splay_capacity(player)
    }

    fn splay_capacity(&self, player: Player) -> usize {
        let best = Color::ALL
            .into_iter()
            .map(|color| self.splay(player, color) as usize)
            .max()
            .unwrap_or(0);
        5 - best
    }

    /// Whether one more card may enter this zone. Only capacity-limited
    /// zones (Forecast, Safe) can refuse; a refused card stays where it is.
    pub(crate) fn has_room(&self, zone: Zone) -> bool {
        match zone {
            Zone::Forecast(owner) => self.cards(zone).len() < self.forecast_limit(owner),
            Zone::Safe(owner) => self.cards(zone).len() < self.safe_limit(owner),
            Zone::Display(_) => self.cards(zone).is_empty(),
            Zone::Museum(_) => self.available_museums() > 0,
            _ => true,
        }
    }

    pub fn top_figures(&self, player: Player) -> impl Iterator<Item = CardId> + '_ {
        Color::ALL
            .into_iter()
            .filter_map(move |color| self.top(player, color))
            .filter(|&card| self.card(card).figure.is_some())
    }

    pub fn has_figure(&self, player: Player, figure: Figure) -> bool {
        self.top_figure_set(player).contains(figure)
    }

    /// The Figures on top of a player's board.
    pub(crate) fn top_figure_set(&self, player: Player) -> FigureSet {
        let cached = self.figure_tops[player.index()];
        debug_assert_eq!(cached, self.scan_figure_tops(player), "stale Figure tops");
        cached
    }

    fn scan_figure_tops(&self, player: Player) -> FigureSet {
        FigureSet(
            self.top_figures(player)
                .filter_map(|card| self.card(card).figure)
                .fold(0, |set, figure| set | (1 << figure as u8)),
        )
    }

    /// Recompute every derived cache after bulk changes: card locations,
    /// both boards' symbols and top Figures.
    pub(crate) fn refresh_boards(&mut self) {
        self.rebuild_locations();
        for player in Player::ALL {
            self.refresh_board(player);
        }
    }

    /// Refresh one board's symbol tally, top Figures and watched triggers.
    pub(crate) fn refresh_board(&mut self, player: Player) {
        self.symbols[player.index()] = self.scan_board_symbols(player);
        if !self.figures() {
            // Without Figures rules no query reads them.
            return;
        }
        let (set, watched) = self
            .top_figures(player)
            .filter_map(|card| self.card(card).figure)
            .fold((0, 0), |(set, watched), figure| {
                (set | 1 << figure as u8, watched | figure.watched())
            });
        self.figure_tops[player.index()] = FigureSet(set);
        self.watched[player.index()] = watched;
    }

    /// The trigger kinds any top Figure watches.
    pub(crate) fn watched(&self) -> u32 {
        self.watched[0] | self.watched[1]
    }

    /// Figures widen ownership, not the definition of a Dogma card.
    pub fn dogma_source_owner(&self, player: Player, card: CardId) -> Option<Player> {
        let selected = self.catalog.get(usize::from(card.0))?;
        if !self.can_dogma(card) {
            return None;
        }
        if self.top(player, selected.color) == Some(card) {
            Some(player)
        } else if self.figures()
            && (self.has_figure(player, Figure::SergeyBrin)
                || (selected.color == Color::Blue && self.has_figure(player, Figure::Ptolemy)))
            && self.top(player.opponent(), selected.color) == Some(card)
        {
            Some(player.opponent())
        } else {
            None
        }
    }

    pub fn turn_owner(&self) -> Player {
        self.turn_owner
    }

    /// Whether this turn's Cities endorsement has been used.
    pub fn endorsement_used(&self) -> bool {
        self.endorsed
    }

    pub fn regular_actions_started(&self) -> u8 {
        self.regular_actions_started
    }

    pub fn highest_top_value(&self, player: Player) -> u8 {
        Color::ALL
            .into_iter()
            .filter_map(|color| self.top(player, color))
            .map(|card| self.value(card))
            .max()
            .unwrap_or(0)
    }

    /// Exactly one of this player's top cards has the highest value: with
    /// Echoes, their ordinary draws come from the Echoes deck. An empty board
    /// has no such card.
    pub fn unique_highest_top(&self, player: Player) -> bool {
        let highest = self.highest_top_value(player);
        Color::ALL
            .into_iter()
            .filter_map(|color| self.top(player, color))
            .filter(|&card| self.value(card) == highest)
            .count()
            == 1
    }

    /// Start an ordinary turn, or explicitly mark the first one-action turn
    /// after setup. Its single action is still ordinal one for Figures rules.
    pub fn begin_turn(&mut self, owner: Player) {
        self.turn_owner = owner;
        self.turn_moves = [[0; 2]; 2];
        self.first_drawn = [false; 2];
        self.endorsed = false;
        self.regular_actions_started = 0;
        self.regular_action_limit = 2;
    }

    pub fn begin_opening_turn(&mut self, owner: Player) {
        self.begin_turn(owner);
        self.regular_action_limit = 1;
    }

    pub fn begin_action(&mut self, kind: ActionKind) -> Result<(), Error> {
        if kind == ActionKind::Regular {
            if self.regular_actions_started >= self.regular_action_limit {
                return Err(Error::Unsupported("regular action limit reached"));
            }
            self.regular_actions_started += 1;
        }
        Ok(())
    }

    pub fn meiji_active(&self, player: Player) -> bool {
        self.figures()
            && self.turn_owner == player
            && self.regular_actions_started < 2
            && self.has_figure(player, Figure::EmperorMeiji)
    }

    pub fn grant_hand_view(&mut self, viewer: Player, owner: Player) {
        self.hand_grants[viewer.index()][owner.index()] = true;
    }

    /// Admission sees the new action's timing without mutating the clock.
    pub(crate) fn action_hand(&self, kind: ActionKind) -> impl Iterator<Item = Target> + '_ {
        self.targets(LogicalZone::Hand(self.turn_owner))
            .filter(move |target| {
                kind != ActionKind::Regular
                    || self.regular_actions_started == 0
                    || target.physical == Zone::Hand(self.turn_owner)
            })
    }

    /// Borrow live membership without allocating a temporary target collection.
    #[inline]
    pub fn targets(&self, zone: LogicalZone) -> impl Iterator<Item = Target> + '_ {
        let physical = zone.home();
        let player = match zone {
            LogicalZone::Hand(p)
            | LogicalZone::Score(p)
            | LogicalZone::AvailableAchievements(p) => Some(p),
            LogicalZone::BoardDepth(..) | LogicalZone::Physical(_) => None,
        };
        // Aliases come only from Figures on top of that player's board.
        if !self.figures() || player.is_none_or(|p| self.top_figure_set(p).is_empty()) {
            return Lookup::Plain(self.cards(physical).iter().map(move |&card| Target {
                card,
                physical,
                logical: zone,
            }));
        }
        let aliases = if self.figures() {
            match zone {
                LogicalZone::Hand(p) if self.meiji_active(p) => {
                    [Some(Zone::Hand(p.opponent())), None]
                }
                // Independent Each inputs: borrowed hands do not feed
                // Michelangelo, nor junk von Braun.
                LogicalZone::Score(p) => [
                    self.has_figure(p, Figure::Michelangelo)
                        .then_some(Zone::Hand(p)),
                    self.has_figure(p, Figure::WernherVonBraun)
                        .then_some(Zone::Junk),
                ],
                _ => [None, None],
            }
        } else {
            [None, None]
        };
        let board_aliases = match zone {
            LogicalZone::AvailableAchievements(player) if self.figures() => {
                let mcadam = self.has_figure(player, Figure::JohnLoudonMcAdam);
                let cai_lun = self.has_figure(player, Figure::CaiLun);
                let amina = self.has_figure(player, Figure::AminaSukhera);
                (mcadam || cai_lun || amina).then_some((player, mcadam, cai_lun, amina))
            }
            _ => None,
        };
        Lookup::Aliased(
            [Some(physical), aliases[0], aliases[1]]
                .into_iter()
                .flatten()
                .flat_map(move |physical| {
                    self.cards(physical).iter().map(move |&card| Target {
                        card,
                        physical,
                        logical: zone,
                    })
                })
                .chain(board_aliases.into_iter().flat_map(
                    move |(player, mcadam, cai_lun, amina)| {
                        Color::ALL.into_iter().filter_map(move |color| {
                            let card = self.top(player, color)?;
                            // Independent Each predicates read only the physical board.
                            // Their union visits each physical top once, even if both apply.
                            ((mcadam && self.card(card).printed_icon_count(Icon::Industry) > 0)
                                || (amina
                                    && self.card(card).printed_icon_count(Icon::Authority) > 0)
                                || (cai_lun && self.splay(player, color) == Splay::None))
                                .then_some(Target {
                                    card,
                                    physical: Zone::Board(player, color),
                                    logical: zone,
                                })
                        })
                    },
                )),
        )
    }

    /// Owned standard achievements, special achievements and Figures Decrees.
    pub fn claimed_achievement_count(&self, player: Player) -> usize {
        // Only an enabled special is ever claimed, so the whole table counts
        // without the per-expansion roster chain (hot: every claim check).
        self.players[player.index()].achievements.len()
            + self
                .specials
                .iter()
                .filter(|&&location| location == SpecialLocation::Owned(player))
                .count()
            + Decree::ALL
                .into_iter()
                .filter(|&d| self.decree_location(d) == DecreeLocation::Owned(player))
                .count()
            + self.claimed_museums(player)
    }

    /// The basic rules count: claimed achievements plus active Cities flags
    /// and fountains. Figure Each modifiers consume this, not each other.
    pub fn achievement_count(&self, player: Player) -> usize {
        self.claimed_achievement_count(player) + self.city_achievements(player)
    }

    /// Achievements from visible Cities symbols: each visible Fountain, and each
    /// visible Flag whose color no opponent shows more cards of. These are never
    /// tokens: they come and go with the boards.
    // Keep this guard small so disabled Cities needs no counting call.
    #[inline]
    pub fn city_achievements(&self, player: Player) -> usize {
        if !self.expansions.cities() {
            return 0;
        }
        self.count_city_achievements(player)
    }

    fn count_city_achievements(&self, player: Player) -> usize {
        Color::ALL
            .into_iter()
            .map(|color| {
                let (mut flags, mut fountains) = (0, 0);
                self.visible_symbols(player, color, |symbol| match symbol {
                    Symbol::City(CityIcon::Flag) => flags += 1,
                    Symbol::City(CityIcon::Fountain) => fountains += 1,
                    _ => {}
                });
                let held = flags > 0
                    && self.visible_cards_in(player.opponent(), color)
                        <= self.visible_cards_in(player, color);
                fountains + if held { flags } else { 0 }
            })
            .sum()
    }

    /// Every visible symbol of one pile: the top card whole, covered cards
    /// through the pile's splay.
    fn visible_symbols(&self, player: Player, color: Color, mut visit: impl FnMut(Symbol)) {
        let Some((&top, covered)) = self.cards(Zone::Board(player, color)).split_last() else {
            return;
        };
        let splay = self.splay(player, color);
        if splay != Splay::None {
            for &card in covered {
                self.card(card).visit_covered(splay, &mut visit);
            }
        }
        self.card(top).visit_printed(visit);
    }

    /// Visible symbols of one pile that match.
    pub(crate) fn visible_count(
        &self,
        player: Player,
        color: Color,
        matches: impl Fn(Symbol) -> bool,
    ) -> usize {
        let mut count = 0;
        self.visible_symbols(player, color, |symbol| {
            count += usize::from(matches(symbol))
        });
        count
    }

    /// Visible cards in one pile: its top, or every card when splayed.
    pub(crate) fn visible_cards_in(&self, player: Player, color: Color) -> usize {
        let len = self.cards(Zone::Board(player, color)).len();
        if self.splay(player, color) == Splay::None {
            len.min(1)
        } else {
            len
        }
    }

    /// Achievement value including independent Each contributions. Virtual
    /// achievements never become physical tokens or count as same-age claims.
    pub fn effective_achievement_count(&self, player: Player) -> usize {
        let figures = self.figure_rules(player);
        if figures.is_empty() {
            return self.achievement_count(player);
        }
        let own = self.board_symbols(player);
        let extra = if figures.contains(Figure::AlfredNobel) {
            let theirs = self.board_symbols(player.opponent());
            self.standard_icons()
                .filter(|&icon| own.icon(icon) > 0 && own.icon(icon) >= 2 * theirs.icon(icon))
                .count()
        } else {
            0
        };
        let machiavelli = if figures.contains(Figure::NiccoloMachiavelli) {
            Color::ALL
                .into_iter()
                .filter(|&color| {
                    self.splay(player, color) == Splay::Right
                        && self.splay(player.opponent(), color) != Splay::Right
                })
                .count()
        } else {
            0
        };
        let mandela = if figures.contains(Figure::NelsonMandela) {
            own.icon(Icon::Avatar) / 2
        } else {
            0
        };
        let curie = if figures.contains(Figure::MarieCurie) {
            // Each reads only physical score cards, not effective score membership.
            self.cards(Zone::Score(player))
                .iter()
                .map(|&card| self.value(card))
                .filter(|&value| value > 6)
                .fold(0_u16, |values, value| values | (1 << value))
                .count_ones() as usize
        } else {
            0
        };
        let lee = if figures.contains(Figure::RobertELee) {
            own.icon(Icon::Health) / 7
        } else {
            0
        };
        self.achievement_count(player) + extra + machiavelli + mandela + curie + lee
    }

    /// Whether this player has drawn a card this turn, so Unseen's first-draw
    /// replacement no longer applies to them until the next turn.
    pub fn first_drawn(&self, player: Player) -> bool {
        self.first_drawn[player.index()]
    }

    /// Daedalus changes claim eligibility only, never printed values or draws.
    pub fn highest_claim_value(&self, player: Player) -> u8 {
        let extra = if self.figures() && self.has_figure(player, Figure::Daedalus) {
            Color::ALL
                .into_iter()
                .filter(|&color| {
                    self.top(player, color).is_some() && self.splay(player, color) == Splay::None
                })
                .count() as u8
        } else {
            0
        };
        self.highest_top_value(player) + extra
    }

    /// An available standard achievement, or one of the player's own
    /// secrets (Unseen), under the usual score and top-value requirements.
    pub fn eligible_for_achievement(&self, player: Player, card: CardId) -> bool {
        (self
            .targets(LogicalZone::AvailableAchievements(player))
            .any(|target| target.card == card)
            || self.cards(Zone::Safe(player)).contains(&card))
            && self.meets_achievement_cost(
                player,
                card,
                self.score(player),
                self.highest_claim_value(player),
            )
    }

    /// Score and top-value requirements of one available card, given the
    /// player's score and claim value (computed once for a whole supply).
    pub(crate) fn meets_achievement_cost(
        &self,
        player: Player,
        card: CardId,
        score: i64,
        claim_value: u8,
    ) -> bool {
        let age = self.value(card);
        let duplicates = self.players[player.index()]
            .achievements
            .iter()
            .filter(|&&id| self.value(id) == age)
            .count();
        // Claim-only points use BASIC hand membership, independently of Meiji
        // and Michelangelo. They neither change score nor highest-top eligibility.
        let hand_points =
            if self.figures() && self.has_figure(player, Figure::AntonieVanLeeuwenhoek) {
                10 * self
                    .cards(Zone::Hand(player))
                    .iter()
                    .filter(|&&id| self.value(id) == age)
                    .count() as i64
            } else {
                0
            };
        score + hand_points >= 5 * i64::from(age) * (duplicates as i64 + 1) && claim_value >= age
    }

    pub fn splay(&self, player: Player, color: Color) -> Splay {
        self.players[player.index()].splays[color as usize]
    }

    pub fn visible_card_count(&self, player: Player) -> usize {
        Color::ALL
            .into_iter()
            .map(|color| self.visible_cards_in(player, color))
            .sum()
    }

    /// Every visible symbol of a board in one pass: top cards whole, covered
    /// cards through their splay.
    pub(crate) fn board_symbols(&self, player: Player) -> BoardSymbols {
        let cached = self.symbols[player.index()];
        debug_assert_eq!(
            cached,
            self.scan_board_symbols(player),
            "stale board symbols"
        );
        cached
    }

    fn scan_board_symbols(&self, player: Player) -> BoardSymbols {
        let mut out = BoardSymbols::default();
        let board = &self.players[player.index()];
        for (stack, &splay) in board.stacks.iter().zip(&board.splays) {
            let Some((&top, covered)) = stack.split_last() else {
                continue;
            };
            if splay != Splay::None {
                for &card in covered {
                    self.card(card)
                        .visit_covered(splay, |symbol| out.add(symbol));
                }
            }
            self.card(top).visit_printed(|symbol| out.add(symbol));
        }
        out
    }

    /// How many visible bonuses of each value a board shows (index = value),
    /// counting a City's repeated bonus each time.
    pub fn visible_bonuses(&self, player: Player) -> [u16; 16] {
        let mut counts = [0_u16; 16];
        let mut count = |symbol| {
            if let Symbol::Bonus(value) = symbol
                && let Some(slot) = counts.get_mut(usize::from(value))
            {
                *slot += 1;
            }
        };
        let board = &self.players[player.index()];
        for (stack, &splay) in board.stacks.iter().zip(&board.splays) {
            let Some((&top, covered)) = stack.split_last() else {
                continue;
            };
            if splay != Splay::None {
                for &card in covered {
                    self.card(card).visit_covered(splay, &mut count);
                }
            }
            self.card(top).visit_printed(&mut count);
        }
        counts
    }

    /// Basic board state only. Each modifiers must independently consume this
    /// query, not another modifier's effective icon count.
    pub fn basic_icon_count(&self, player: Player, icon: Icon) -> usize {
        self.board_symbols(player).icon(icon)
    }

    /// Whether a symbol of this icon shows in one pile. Figures' Each additions are not
    /// on the board: they change totals, never a color.
    pub fn color_has_icon(&self, player: Player, color: Color, icon: Icon) -> bool {
        self.visible_count(player, color, |symbol| symbol.standard() == Some(icon)) > 0
    }

    /// Avatar is the seventh standard type; Food joins only after Age 0 introduction
    /// (p30).
    pub fn standard_icons(&self) -> impl Iterator<Item = Icon> + '_ {
        [
            Icon::Authority,
            Icon::Concept,
            Icon::Health,
            Icon::Prosperity,
            Icon::Industry,
            Icon::Efficiency,
            Icon::Avatar,
        ]
        .into_iter()
        .chain(self.age_zero_introduced().then_some(Icon::Food))
    }

    pub fn icon_count(&self, player: Player, icon: Icon) -> usize {
        let symbols = self.board_symbols(player);
        let figures = self.figure_rules(player);
        if figures.is_empty() {
            return symbols.icon(icon);
        }
        self.effective_icon(&symbols, figures, player, icon)
    }

    /// The Figures on top of this player's board whose rules apply: none in
    /// a game without Figures.
    pub(crate) fn figure_rules(&self, player: Player) -> FigureSet {
        if self.figures() {
            self.top_figure_set(player)
        } else {
            FigureSet::default()
        }
    }

    /// An effective icon count from one board pass and the player's tops.
    /// Inlined so the common no-Figure case stays a table read.
    #[inline]
    pub(crate) fn effective_icon(
        &self,
        symbols: &BoardSymbols,
        figures: FigureSet,
        player: Player,
        icon: Icon,
    ) -> usize {
        let basic = symbols.icon(icon);
        if figures.is_empty() {
            return basic;
        }
        self.figure_icon(symbols, figures, player, icon, basic)
    }

    #[inline(never)]
    fn figure_icon(
        &self,
        symbols: &BoardSymbols,
        figures: FigureSet,
        player: Player,
        icon: Icon,
        basic: usize,
    ) -> usize {
        let standard = || self.standard_icons().any(|standard| standard == icon);
        let gilgamesh = if matches!(icon, Icon::Authority | Icon::Industry)
            && figures.contains(Figure::Gilgamesh)
        {
            symbols.icon(Icon::Authority)
        } else {
            0
        };
        let maxwell = if basic > 0 && figures.contains(Figure::JamesClerkMaxwell) && standard() {
            // Basic physical hand, not Meiji's effective hand membership.
            self.cards(Zone::Hand(player)).len()
        } else {
            0
        };
        let catherine = if matches!(icon, Icon::Concept | Icon::Efficiency)
            && figures.contains(Figure::CatherineTheGreat)
        {
            symbols.icon(Icon::Concept)
        } else {
            0
        };
        let tran = if basic > 0
            && icon != Icon::Authority
            && figures.contains(Figure::TranHuangDao)
            && standard()
        {
            // Literal "other... type on your board": absent types do not appear, and
            // other Each additions never supply input Authority.
            symbols.icon(Icon::Authority) / 2
        } else {
            0
        };
        let smith = if icon == Icon::Prosperity && figures.contains(Figure::AdamSmith) {
            2 * basic
        } else {
            0
        };
        basic + gilgamesh + maxwell + catherine + tran + smith
    }

    /// The score contributed by visible printed bonuses, before Figure Each modifiers.
    pub fn bonus_score(&self, player: Player) -> i64 {
        self.board_symbols(player).bonus()
    }

    /// Signed products of basic counts must fit the admitted u16 card domain.
    pub fn score(&self, player: Player) -> i64 {
        let symbols = self.board_symbols(player);
        let physical_score: i64 = self
            .cards(Zone::Score(player))
            .iter()
            .map(|&card| i64::from(self.value(card)))
            .sum();
        let mut score = physical_score + symbols.bonus();
        let figures = self.figure_rules(player);
        // The opponent's Gilgamesh also lowers this score.
        if figures.is_empty() && self.figure_rules(player.opponent()).is_empty() {
            return score;
        }
        let hand = || self.cards(Zone::Hand(player));
        if figures.contains(Figure::Michelangelo) {
            score += hand()
                .iter()
                .map(|&card| i64::from(self.value(card)))
                .sum::<i64>();
        }
        if figures.contains(Figure::ShenKuo) {
            score += self.visible_card_count(player) as i64;
        }
        if figures.contains(Figure::Tigernmas) {
            score += hand().len() as i64;
        }
        if figures.contains(Figure::Sinuhe) {
            score += symbols.icon(Icon::Authority) as i64;
        }
        if figures.contains(Figure::ChristopherPolhem) {
            score += symbols.icon(Icon::Industry) as i64 * self.achievement_count(player) as i64;
        }
        if figures.contains(Figure::EmmyNoether) {
            let efficiency = symbols.icon(Icon::Efficiency) as i64;
            score += efficiency * efficiency;
        }
        if figures.contains(Figure::WernherVonBraun) {
            score += self
                .cards(Zone::Junk)
                .iter()
                .map(|&card| i64::from(self.value(card)))
                .sum::<i64>();
        }
        if self.has_figure(player.opponent(), Figure::Gilgamesh) {
            score -= symbols.icon(Icon::Authority) as i64;
        }
        score
    }

    /// Equal generations mean equal cards in every zone, when zones change
    /// only through [`Self::zone_mut`] (dealing aside).
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Every input to [`Self::can_view`] besides the zone and the viewer:
    /// equal values mean every zone is visible to the same players.
    pub(crate) fn visibility(&self) -> u16 {
        Player::ALL.into_iter().fold(0, |bits, viewer| {
            let owners = Player::ALL.into_iter().fold(0, |bits, owner| {
                bits << 2
                    | u16::from(self.hand_grants[viewer.index()][owner.index()]) << 1
                    | u16::from(self.inspections[viewer.index()][owner.index()])
            });
            let junk = self.figures() && self.has_figure(viewer, Figure::WernherVonBraun);
            bits << 6 | owners << 2 | u16::from(self.meiji_active(viewer)) << 1 | u16::from(junk)
        })
    }

    /// Whether `viewer` may look at the fronts in `zone` now: their own
    /// private piles, a hand lent by a Figure, the junk while it counts as
    /// their score pile (Wernher von Braun), or the executor's hand, score
    /// and Forecast under a live inspection grant (Hitchhiking). Keep
    /// [`Self::visibility`] in step with every input read here.
    pub fn can_view(&self, zone: Zone, viewer: Player) -> bool {
        match zone {
            Zone::Hand(owner) => {
                self.hand_grants[viewer.index()][owner.index()]
                    || self.inspections[viewer.index()][owner.index()]
                    || (owner != viewer && self.meiji_active(viewer))
            }
            Zone::Score(owner) | Zone::Forecast(owner) => {
                viewer == owner || self.inspections[viewer.index()][owner.index()]
            }
            // Junk counts as von Braun's owner's score pile, so they may look at it; it
            // stays hidden from everyone else.
            Zone::Junk => self.figures() && self.has_figure(viewer, Figure::WernherVonBraun),
            // Secrets are hidden from their owner too.
            Zone::Safe(_)
            | Zone::Deck(_)
            | Zone::Achievements(_)
            | Zone::AvailableAchievements
            | Zone::AgeZeroReserve => false,
            Zone::Board(..) | Zone::Display(_) | Zone::Museum(_) => true,
        }
    }

    pub fn descriptor(&self, target: Target, viewer: Player) -> TargetDescriptor {
        if self.can_view(target.physical, viewer) {
            TargetDescriptor::Known {
                card: target.card,
                source: target.physical,
            }
        } else {
            // Public slot and back describe the choice without publishing the
            // front held internally by the suspended continuation.
            TargetDescriptor::Hidden {
                source: target.physical,
                slot: self
                    .cards(target.physical)
                    .iter()
                    .position(|&c| c == target.card)
                    .expect("live target"),
                back: self.card(target.card).origin_deck(),
            }
        }
    }

    /// Locate a retained physical reference after intervening effects. No
    /// reserved zone or public reveal is implied by keeping this reference.
    /// Used only by continuations whose rules authorize following the card.
    pub(crate) fn locate(&self, card: CardId) -> Option<Zone> {
        let cached = self.locations[usize::from(card.0)].zone;
        debug_assert_eq!(cached, self.scan_locate(card), "stale card location");
        cached
    }

    /// Record one card's new zone after a single movement.
    pub(crate) fn set_location(&mut self, card: CardId, zone: Zone) {
        // Moving ends an explicit reveal.
        if self.revealed_card() == Some(card) {
            self.end_reveal();
        }
        let zone = (zone != Zone::AgeZeroReserve).then_some(zone);
        self.locations[usize::from(card.0)] = Located {
            zone,
            value: self.value_in(card, zone),
        };
    }

    /// Recompute every card's location after bulk changes.
    pub(crate) fn rebuild_locations(&mut self) {
        let zones = Player::ALL
            .into_iter()
            .flat_map(|player| {
                [
                    Zone::Hand(player),
                    Zone::Score(player),
                    Zone::Achievements(player),
                    Zone::Forecast(player),
                    Zone::Display(player),
                    Zone::Museum(player),
                    Zone::Safe(player),
                ]
                .into_iter()
                .chain(
                    Color::ALL
                        .into_iter()
                        .map(move |color| Zone::Board(player, color)),
                )
            })
            .chain([Zone::AvailableAchievements, Zone::Junk])
            .chain(CardSet::ALL.into_iter().flat_map(|set| {
                (0..Deck::AGES as u8).map(move |age| Zone::Deck(Deck { set, age }))
            }));
        let mut locations = std::mem::take(&mut self.locations);
        for (located, card) in locations.iter_mut().zip(self.catalog.iter()) {
            *located = Located {
                zone: None,
                value: card.value,
            };
        }
        for zone in zones {
            for &card in self.cards(zone) {
                locations[usize::from(card.0)] = Located {
                    zone: Some(zone),
                    value: self.value_in(card, Some(zone)),
                };
            }
        }
        self.locations = locations;
    }

    fn scan_locate(&self, card: CardId) -> Option<Zone> {
        for player in Player::ALL {
            for zone in [
                Zone::Hand(player),
                Zone::Score(player),
                Zone::Achievements(player),
                Zone::Forecast(player),
                Zone::Display(player),
                Zone::Museum(player),
                Zone::Safe(player),
            ] {
                if self.cards(zone).contains(&card) {
                    return Some(zone);
                }
            }
            for color in Color::ALL {
                let zone = Zone::Board(player, color);
                if self.cards(zone).contains(&card) {
                    return Some(zone);
                }
            }
        }
        if self.available_achievements.contains(&card) {
            return Some(Zone::AvailableAchievements);
        }
        let deck = self.card(card).origin_deck();
        if self.cards(Zone::Deck(deck)).contains(&card) {
            return Some(Zone::Deck(deck));
        }
        self.junk.contains(&card).then_some(Zone::Junk)
    }

    /// Oracle normal form: hands, scores, achievements, Forecasts and junk
    /// are unordered. Decks, board stacks and Safes retain their physical
    /// order.
    #[cfg(test)]
    pub(crate) fn sort_unordered_piles(&mut self) {
        for player in &mut self.players {
            player.hand.sort_unstable();
            player.score.sort_unstable();
            player.achievements.sort_unstable();
        }
        for forecast in self.forecast.iter_mut().flat_map(|piles| piles.iter_mut()) {
            forecast.sort_unstable();
        }
        for museums in self.artifacts.iter_mut().flat_map(|a| a.museums.iter_mut()) {
            museums.sort_unstable();
        }
        self.available_achievements.sort_unstable();
        self.junk.sort_unstable();
    }

    /// A subgame's position is missing its suspended parent's cards. After a
    /// sampled world moves hidden identities between the supply and those
    /// set-aside cards, the ones in no zone are the absent ones.
    pub(crate) fn refresh_absent(&mut self) {
        if self.absent.is_none() {
            return;
        }
        let mut absent = vec![true; self.catalog.len()].into_boxed_slice();
        for zone in crate::tracking::zones() {
            for card in self.cards(zone) {
                absent[usize::from(card.0)] = false;
            }
        }
        self.absent = Some(absent);
    }

    pub fn check_conservation(&self) -> bool {
        let mut seen = vec![0_u8; self.catalog.len()];
        for id in self
            .players
            .iter()
            .flat_map(|p| {
                p.hand
                    .iter()
                    .chain(&p.score)
                    .chain(&p.achievements)
                    .chain(p.stacks.iter().flatten())
            })
            .chain(self.decks.iter().flatten())
            .chain(
                self.set_decks
                    .iter()
                    .flatten()
                    .flat_map(|decks| decks.iter().flatten()),
            )
            .chain(&self.junk)
            .chain(&self.available_achievements)
            .chain(self.age_zero.iter().flat_map(|a| &a.reserve))
            .chain(self.forecast.iter().flat_map(|f| f.iter().flatten()))
            .chain(self.safe.iter().flat_map(|s| s.iter().flatten()))
        {
            let Some(count) = seen.get_mut(usize::from(id.0)) else {
                return false;
            };
            *count = count.saturating_add(1);
        }
        // Rare storage stays out of the common chain above.
        if let Some(artifacts) = self.artifacts.as_deref() {
            for id in artifacts.display.iter().chain(&artifacts.museums).flatten() {
                let Some(count) = seen.get_mut(usize::from(id.0)) else {
                    return false;
                };
                *count = count.saturating_add(1);
            }
            let stored = artifacts.museums.iter().map(Vec::len).sum::<usize>();
            let claimed = artifacts
                .claimed
                .iter()
                .map(|&n| usize::from(n))
                .sum::<usize>();
            if stored + claimed > Artifacts::MUSEUMS
                || artifacts.display.iter().any(|display| display.len() > 1)
            {
                return false;
            }
        }
        // A subgame's position leaves its suspended parent's cards out.
        seen.iter().enumerate().all(|(id, &count)| {
            count == 1 || count == 0 && self.absent.as_ref().is_some_and(|absent| absent[id])
        })
    }
}

/// One physical card's cached zone and its value there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Located {
    zone: Option<Zone>,
    value: u8,
}

/// Figures on top of one board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FigureSet(u128);

impl FigureSet {
    pub(crate) fn contains(self, figure: Figure) -> bool {
        self.0 & (1 << figure as u8) != 0
    }

    pub(crate) fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Visible icon counts, bonuses and City icons of one board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BoardSymbols {
    /// Six slots per card over the admitted u16 card domain fit in u32.
    icons: [u32; 8],
    bonus_count: u32,
    largest_bonus: u8,
    city: [u32; 9],
}

impl BoardSymbols {
    fn add(&mut self, symbol: Symbol) {
        match symbol {
            Symbol::Icon(icon) | Symbol::Search(icon) => self.icons[icon as usize] += 1,
            Symbol::Bonus(value) => {
                self.bonus_count += 1;
                self.largest_bonus = self.largest_bonus.max(value);
            }
            Symbol::City(icon) => self.city[icon as usize] += 1,
            Symbol::Blank | Symbol::Image | Symbol::Echo => {}
        }
    }

    pub(crate) fn icon(&self, icon: Icon) -> usize {
        self.icons[icon as usize] as usize
    }

    /// Visible bonus icons, each counted (Fortune Cookie's "any icon").
    pub(crate) fn bonus_count(&self) -> usize {
        self.bonus_count as usize
    }

    pub(crate) fn city(&self, icon: CityIcon) -> usize {
        self.city[icon as usize] as usize
    }

    /// The largest bonus plus one point per further visible bonus.
    fn bonus(&self) -> i64 {
        if self.bonus_count == 0 {
            0
        } else {
            i64::from(self.largest_bonus) + i64::from(self.bonus_count) - 1
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetDescriptor {
    Known {
        card: CardId,
        source: Zone,
    },
    Hidden {
        source: Zone,
        slot: usize,
        /// The public back: printed set and age.
        back: Deck,
    },
}

/// A zone lookup: the physical zone alone, or with Figure aliases.
enum Lookup<P, A> {
    Plain(P),
    Aliased(A),
}

impl<P: Iterator<Item = Target>, A: Iterator<Item = Target>> Iterator for Lookup<P, A> {
    type Item = Target;

    #[inline]
    fn next(&mut self) -> Option<Target> {
        match self {
            Self::Plain(plain) => plain.next(),
            Self::Aliased(aliased) => aliased.next(),
        }
    }

    // Whole-zone consumers (extend, count, for_each) iterate the inner
    // iterator directly instead of dispatching per card.
    #[inline]
    fn fold<B, F: FnMut(B, Target) -> B>(self, init: B, f: F) -> B {
        match self {
            Self::Plain(plain) => plain.fold(init, f),
            Self::Aliased(aliased) => aliased.fold(init, f),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Plain(plain) => plain.size_hint(),
            Self::Aliased(aliased) => aliased.size_hint(),
        }
    }
}
