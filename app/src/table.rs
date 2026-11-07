//! What the table draws: an engine-independent snapshot from one viewer's
//! seat. A backend rebuilds it after every state change; the UI reads only
//! this and the face table, so layout work serves every engine.
use inno_engine::{Color as CardColor, Icon, Special, Splay};

use super::faces::{FaceId, Set, face};

/// One run of rich text in a prompt, option or log line.
#[derive(Clone, PartialEq, Debug)]
pub enum Seg {
    Text(String),
    Strong(String),
    /// A player, named from the viewer's seat when drawn.
    Name(usize),
    Card(FaceId),
    /// A face-down card: only its set and age are public.
    Back(Set, u8),
    Hue(CardColor),
    Icon(Icon),
}

/// A log entry: a turn header (current or not), a gap between actions, or a
/// line indented by level, folded `count` times.
#[derive(Clone, PartialEq, Debug)]
pub enum Entry {
    Header(String, bool),
    Gap,
    Line(u8, Vec<Seg>, usize),
}

/// A Decree's printed text, by its name.
pub fn decree_text(name: &str) -> &'static str {
    match name {
        "War" => {
            "Choose a value. Return a top card of that value of each color from all opponents' boards."
        }
        "Expansion" => "Splay any one of your colors up.",
        "Trade" => "Draw three cards of value one higher than your highest top card.",
        "Advancement" => "Draw a card of value two higher than your highest top card.",
        "Rivalry" => {
            "Choose an opponent. Return three cards from their score pile. Return a top figure of each color from their board."
        }
        _ => "",
    }
}

/// A pile on the board, bottom card first.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct StackView {
    pub cards: Vec<FaceId>,
    pub splay: Splay,
}

impl StackView {
    pub fn top(&self) -> Option<FaceId> {
        self.cards.last().copied()
    }

    /// Whether slot `slot` of the card `depth` from the bottom shows: every
    /// slot of the top card, otherwise the slots its splay reveals, and a
    /// City's top-right image under a left splay.
    pub fn slot_visible(&self, depth: usize, slot: usize) -> bool {
        depth + 1 == self.cards.len()
            || self.splay.reveals(slot)
            || (slot == 5 && self.splay == Splay::Left)
    }
}

/// A claimed achievement.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Held {
    Card(FaceId),
    Special(Special),
    /// A claimed Museum, by its number.
    Museum(u8),
}

/// One secret in a Safe, oldest first: its back, the front if this viewer
/// has learned it, and whether clicking it is a legal choice now.
#[derive(Clone, PartialEq, Debug)]
pub struct Secret {
    pub set: Set,
    pub age: u8,
    pub known: Option<FaceId>,
    pub legal: bool,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct PlayerView {
    /// By `COLORS` order.
    pub stacks: [StackView; 5],
    pub hand: Vec<FaceId>,
    pub score: Vec<FaceId>,
    pub forecast: Vec<FaceId>,
    /// Whether the viewing seat may see these piles' fronts now: its own,
    /// or the other player's under a rule (Meiji's borrowed hand, a
    /// Hitchhiking controller's inspection).
    pub hand_visible: bool,
    pub score_visible: bool,
    pub forecast_visible: bool,
    pub safe: Vec<Secret>,
    pub display: Option<FaceId>,
    /// Stored Artifacts, with their Museum's number.
    pub museums: Vec<(u8, FaceId)>,
    pub achievements: Vec<Held>,
    /// Achievements counted toward victory, including flags and fountains.
    pub achievement_count: usize,
    /// Signed: Figures modifiers can make an engine-next score negative.
    pub score_total: i64,
    pub bonus_score: i64,
    /// Visible bonus icons on the board, highest first: the first scores
    /// its value and each other one point.
    pub bonuses: Vec<u8>,
    pub icons: [usize; 7],
    /// Food icons, which count only once Age 0 is in play.
    pub food: usize,
    pub forecast_limit: usize,
    pub safe_limit: usize,
    /// Figures Decrees this player has issued, by name and the color of the
    /// Figures that issue it. Shown with the achievements they count as.
    pub decrees: Vec<(String, CardColor)>,
}

/// One set's deck sizes, by age 0 to 11.
#[derive(Clone, PartialEq, Debug)]
pub struct DeckRow {
    pub set: Set,
    pub counts: [usize; 12],
}

/// An available standard achievement, by its public back.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Claimable {
    pub set: Set,
    pub age: u8,
    /// Clicking it claims it now.
    pub legal: bool,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct SupplyView {
    /// Base first, then the enabled expansions.
    pub decks: Vec<DeckRow>,
    /// Sorted by age, then set.
    pub achievements: Vec<Claimable>,
    pub museums_available: usize,
    pub museums_junked: usize,
    pub junk: Vec<FaceId>,
    /// Whether the viewing seat may see the junk's fronts (Wernher von Braun).
    pub junk_visible: bool,
    pub junk_specials: Vec<Special>,
    /// Available special achievements, one group per expansion that has any.
    pub specials: Vec<Vec<Special>>,
    /// Figures Decrees still in the supply, by name and issuing color.
    pub decrees: Vec<(String, CardColor)>,
}

/// One option of a decision: its label and the card it names, if any, so a
/// click on that card on the table picks it.
#[derive(Clone, PartialEq, Debug)]
pub struct OptionView {
    pub segs: Vec<Seg>,
    pub card: Option<FaceId>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct DecisionView {
    pub player: usize,
    pub prompt: Vec<Seg>,
    pub options: Vec<OptionView>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    Winner(usize),
    Draw,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct TableView {
    pub mode: String,
    pub figures: bool,
    /// Age 0 has been introduced: show its deck and the Food icon.
    pub age_zero: bool,
    pub echoes: bool,
    pub cities: bool,
    pub unseen: bool,
    pub artifacts: bool,
    /// The outcome and how the game ended, once it is over.
    pub over: Option<(Outcome, String)>,
    pub current_player: usize,
    /// One-based turn number, action within the turn, and actions allowed.
    pub turn: usize,
    pub action: usize,
    pub actions: usize,
    pub achievements_needed: usize,
    /// Martian Internet's printed rule: how many suspended games this one is
    /// nested in (0 ordinarily), and whether the boards shown are the
    /// immediate parent's rather than the subgame's.
    pub subgame: usize,
    pub peeking: bool,
    /// The match plays Martian Internet's printed subgame, so its card reads
    /// as printed rather than as the draw variant.
    pub martian_internet_printed: bool,
    pub players: [PlayerView; 2],
    /// Whose hand and piles the viewer sees: themselves, or the opponent
    /// whose execution they control.
    pub private_owner: usize,
    pub supply: SupplyView,
    pub decision: Option<DecisionView>,
    pub log: Vec<Entry>,
}

impl TableView {
    /// Cards the pending decision names, for glowing and clicking.
    pub fn option_cards(&self) -> impl Iterator<Item = FaceId> + '_ {
        self.decision
            .iter()
            .flat_map(|d| d.options.iter().filter_map(|o| o.card))
    }
}

/// A player as the viewer reads them.
pub fn name(player: usize, human: usize, two_player: bool) -> &'static str {
    if player == human {
        "You"
    } else if two_player {
        ["P1", "P2"][player]
    } else {
        "AI"
    }
}

/// Card data spells titles in capitals. A letter after a hyphen, full stop
/// or digit starts a new part ("Self-Rotor", "U.S.", "X-1A"), and joining
/// words after the first stay lowercase.
pub fn title_case(s: &str) -> String {
    s.split(' ')
        .enumerate()
        .map(|(i, word)| {
            let lower = word.to_lowercase();
            match lower.as_str() {
                "of" | "the" | "and" | "a" | "an" | "in" | "to" if i > 0 => return lower,
                "atm" | "gps" | "mp3" | "dmc-12" => return lower.to_uppercase(),
                "delorean" => return "DeLorean".to_owned(),
                _ => {}
            }
            let mut out = String::with_capacity(lower.len());
            let mut starts_part = true;
            for c in lower.chars() {
                if starts_part {
                    out.extend(c.to_uppercase());
                } else {
                    out.push(c);
                }
                starts_part = matches!(c, '-' | '.') || c.is_ascii_digit();
            }
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rich text as plain text, for the clipboard.
pub fn plain(segs: &[Seg], human: usize, two_player: bool) -> String {
    segs.iter()
        .map(|seg| match seg {
            Seg::Text(t) | Seg::Strong(t) => t.clone(),
            Seg::Name(p) => name(*p, human, two_player).into(),
            Seg::Card(id) => {
                let c = face(*id);
                format!("{} [{}]", title_case(&c.title), c.age)
            }
            Seg::Back(set, age) => format!("[{} {age}]", set.name()),
            Seg::Hue(c) => c.name().to_lowercase(),
            Seg::Icon(i) => title_case(i.name()),
        })
        .collect()
}
