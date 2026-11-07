//! Observation and action contract `next-1`: one variable-length float row
//! per viewer and prompt:
//!
//! | Part | Size | Contents |
//! |---|---:|---|
//! | Header | 8 | family+generation, revision, scalar, token and option counts, viewer seat, enabled sets, achievement target |
//! | Scalars | [`SCALARS`] | global and per-player state, laid out for all six sets |
//! | Tokens | 5 each | card, group, weight, code, value |
//! | Options | [`OPTION_FIELDS`] each | one per legal step, in the engine's order |
//!
//! Only the viewer's information is encoded: public zones, the viewer's
//! tracker beliefs (candidates weighted by expected count) and continuation
//! references as the viewer may see them. A determinized world therefore
//! encodes identically for its sampling viewer, which the tests check at
//! every root of seeded games. Groups and scalars of a disabled set stay
//! empty. Cards are [`StableId`]s plus one (zero: no identity), so a network
//! sees the same card under every mode.
//!
//! Every categorical field and token code is an integer below 2^24, so it
//! survives the `f32` row exactly; OR'd codes (deck depths, Safe slots,
//! option classes) stay within that range by construction.

pub mod context;
pub mod pack;

use crate::execution::{PromptView, Sight};
use crate::{
    Action, Alternative, CardId, CardSet, Color, Deck, Decree, DecreeLocation, Error, Game, Icon,
    Player, Special, SpecialLocation, Splay, TargetDescriptor, Zone,
};

/// Rows start with `FAMILY + GENERATION` to distinguish incompatible layouts.
pub const FAMILY: u32 = 100;
pub const GENERATION: u32 = 1;
/// Exported networks must match this layout revision.
pub const REVISION: u32 = 7;
pub const HEADER: usize = 8;
pub const TOKEN_FIELDS: usize = 5;
pub const OPTION_FIELDS: usize = 13;

/// Scalar offsets. Player sections are viewer-relative: "me", then "them".
pub mod scalar {
    /// One flag per printed set (Base always set).
    pub const SETS: usize = 0;
    pub const TARGET: usize = SETS + 6;
    pub const MY_TURN: usize = TARGET + 1;
    pub const ACTIONS_STARTED: usize = MY_TURN + 1;
    pub const ACTION_LIMIT: usize = ACTIONS_STARTED + 1;
    /// The viewer is choosing a decision option.
    pub const DECIDING: usize = ACTION_LIMIT + 1;
    /// The viewer is choosing an action.
    pub const ACTING: usize = DECIDING + 1;
    pub const AGE_ZERO: usize = ACTING + 1;
    /// Cards per printed back (set × 12 ages), / 15.
    pub const DECKS: usize = AGE_ZERO + 1;
    /// Twenty token slots × available, mine, theirs, junked.
    pub const SPECIALS: usize = DECKS + 72;
    /// Five Decrees × supply, mine, theirs, junked.
    pub const DECREES: usize = SPECIALS + 80;
    /// Cities: the turn's endorsement is used; the running Dogma action is
    /// endorsed.
    pub const CITIES: usize = DECREES + 20;
    pub const PLAYERS: usize = CITIES + 2;

    /// Offsets within one player's section.
    pub const SCORE: usize = 0;
    pub const BONUS: usize = 1;
    pub const ACHIEVEMENTS: usize = 2;
    pub const CLAIMED: usize = 3;
    pub const HAND: usize = 4;
    pub const SCORE_PILE: usize = 5;
    pub const TOP_VALUE: usize = 6;
    pub const VISIBLE: usize = 7;
    pub const TUCKED: usize = 8;
    pub const SCORED: usize = 9;
    pub const INSPECTS: usize = 10;
    /// Eight effective icon counts (seven standard and Food), four reserved.
    pub const ICONS: usize = 11;
    /// Per color: splay one-hot (5), stack size, top value, top is a Figure.
    pub const COLORS: usize = ICONS + 12;
    pub const COLOR_STRIDE: usize = 8;
    /// Reserved for expansion zones (Forecast, Safe, display, Museums).
    pub const RESERVED: usize = COLORS + 5 * COLOR_STRIDE;
    pub const FORECAST_LIMIT: usize = RESERVED;
    pub const FORECAST_ROOM: usize = RESERVED + 1;
    /// Unseen: Safe capacity / 10, remaining room / 10, and whether the
    /// player's first draw of the turn has happened.
    pub const SAFE_LIMIT: usize = RESERVED + 2;
    pub const SAFE_ROOM: usize = RESERVED + 3;
    pub const FIRST_DRAWN: usize = RESERVED + 4;
    pub const PLAYER: usize = RESERVED + 16;
}

pub const SCALARS: usize = scalar::PLAYERS + 2 * scalar::PLAYER;

/// Token groups, laid out for every expansion; each pools into one summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Decks,
    Junk,
    /// Available standard achievements.
    Supply,
    AgeZeroReserve,
    MyBoard,
    TheirBoard,
    MyHand,
    TheirHand,
    MyScore,
    TheirScore,
    MyAchievements,
    TheirAchievements,
    MyForecast,
    TheirForecast,
    MySafe,
    TheirSafe,
    MyDisplay,
    TheirDisplay,
    MyMuseums,
    TheirMuseums,
    /// Cards the legal options name.
    Options,
    /// Cards the running effects, karma and pending operations refer to.
    Context,
}

pub const GROUPS: usize = 22;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Token {
    /// [`StableId`] plus one; zero when the token names no identity.
    pub card: u16,
    pub group: Group,
    /// Expected count: sums of `1 / candidates` over the slots it may fill.
    pub weight: f32,
    /// Group-specific bits: deck depths, Safe slots, board position and
    /// visible slots, option classes, or a context role and back.
    pub code: u32,
    pub value: f32,
}

/// Option kinds: native actions, then decision alternatives.
pub mod kind {
    pub const DRAW: u8 = 1;
    pub const MELD: u8 = 2;
    pub const ACHIEVE: u8 = 3;
    pub const DOGMA: u8 = 4;
    pub const DECREE_ACTION: u8 = 5;
    pub const VALUE: u8 = 6;
    pub const ICON: u8 = 7;
    pub const COLORS: u8 = 8;
    pub const MATCHING_PAIR: u8 = 9;
    pub const BRANCH: u8 = 10;
    pub const BLIND_SELECT: u8 = 11;
    pub const DECREE: u8 = 12;
    pub const DRAW_CHOICE: u8 = 13;
    pub const SELECT: u8 = 14;
    pub const SPLAY: u8 = 15;
    pub const EXCHANGE: u8 = 16;
    pub const BLIND_MOVE: u8 = 17;
    pub const DECK: u8 = 18;
    pub const NAME_FIGURE: u8 = 19;
    pub const PASS: u8 = 20;
    pub const DECLINE: u8 = 21;
    pub const JUNK_SPECIAL: u8 = 22;
    pub const RESTORE_SPECIAL: u8 = 23;
    pub const CLAIM_SPECIAL: u8 = 24;
    pub const MOVE: u8 = 25;
    pub const REVEAL: u8 = 26;
    pub const SELF_EXECUTE: u8 = 27;
    pub const KARMA: u8 = 28;
    pub const SHOWCASE: u8 = 29;
    pub const COUNT: u8 = 30;
}

/// The class bit an option kind contributes to the [`Group::Options`] token
/// of the card it names: 0 Meld, 1 Achieve, 2 Dogma, 3 Select, Reveal and
/// BlindSelect, 4 Move and BlindMove, 5 MatchingPair, 6 SelfExecute,
/// Showcase and Karma, 7 NameFigure. Kinds that name no card have no bit, so
/// an OR of classes fits eight bits.
pub fn option_class(kind: u8) -> u32 {
    match kind {
        kind::MELD => 1 << 0,
        kind::ACHIEVE => 1 << 1,
        kind::DOGMA => 1 << 2,
        kind::SELECT | kind::REVEAL | kind::BLIND_SELECT => 1 << 3,
        kind::MOVE | kind::BLIND_MOVE => 1 << 4,
        kind::MATCHING_PAIR => 1 << 5,
        kind::SELF_EXECUTE | kind::SHOWCASE | kind::KARMA => 1 << 6,
        kind::NAME_FIGURE => 1 << 7,
        _ => 0,
    }
}

/// Option fields; categorical ones are zero when absent. Every field is an
/// integer below 2^24.
pub mod field {
    pub const KIND: usize = 0;
    /// The named card's identity plus one, if the viewer may know it; a
    /// named Figure is a card identity too.
    pub const CARD: usize = 1;
    /// Viewer-relative zone code ([`super::zone_code`]); an exchange's left
    /// zone.
    pub const ZONE: usize = 2;
    /// Printed back (`Deck::index` plus one).
    pub const BACK: usize = 3;
    /// A value, age, branch index, splay direction or count.
    pub const NUMBER: usize = 4;
    pub const COLORS: usize = 5;
    pub const ICON: usize = 6;
    pub const VERB: usize = 7;
    /// A special (1–20, index plus one) or a Decree (21–25).
    pub const TOKEN: usize = 8;
    /// A hidden slot plus one (a deck or Safe position), or the Auspice flag
    /// of a Dogma action or Showcase.
    pub const SLOT: usize = 9;
    pub const OWNER: usize = 10;
    /// A second card (matching pairs).
    pub const SECOND: usize = 11;
    /// An exchange's right zone.
    pub const SECOND_ZONE: usize = 12;
    /// First Decree token code; Decrees follow the twenty special slots.
    pub const DECREE_TOKENS: u8 = 21;
}

/// What one viewer knows at a settled prompt, and its legal options.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub viewer: Player,
    pub sets: u8,
    pub target: u8,
    pub scalars: Vec<f32>,
    pub tokens: Vec<Token>,
    pub options: Vec<[f32; OPTION_FIELDS]>,
}

impl Observation {
    /// The flat row: header, scalars, tokens, options.
    pub fn row(&self) -> Vec<f32> {
        let mut row = Vec::with_capacity(
            HEADER
                + self.scalars.len()
                + TOKEN_FIELDS * self.tokens.len()
                + OPTION_FIELDS * self.options.len(),
        );
        row.extend([
            (FAMILY + GENERATION) as f32,
            REVISION as f32,
            self.scalars.len() as f32,
            self.tokens.len() as f32,
            self.options.len() as f32,
            self.viewer.index() as f32,
            f32::from(self.sets),
            f32::from(self.target),
        ]);
        row.extend(&self.scalars);
        for token in &self.tokens {
            row.extend([
                f32::from(token.card),
                token.group as u8 as f32,
                token.weight,
                token.code as f32,
                token.value,
            ]);
        }
        for option in &self.options {
            row.extend(option);
        }
        row
    }
}

fn stable(game: &Game, card: CardId) -> u16 {
    game.position()
        .card(card)
        .stable_id()
        .map_or(0, |id| id.0 + 1)
}

fn owner(player: Player, viewer: Player) -> usize {
    usize::from(player != viewer)
}

/// Viewer-relative zone codes; 13 and up are reserved for expansion zones.
pub fn zone_code(zone: Zone, viewer: Player) -> u8 {
    let mine = |p: Player| u8::from(p != viewer);
    match zone {
        Zone::Hand(p) => 1 + mine(p),
        Zone::Score(p) => 3 + mine(p),
        Zone::Achievements(p) => 5 + mine(p),
        Zone::AvailableAchievements => 7,
        Zone::AgeZeroReserve => 8,
        Zone::Board(p, _) => 9 + mine(p),
        Zone::Deck(_) => 11,
        Zone::Junk => 12,
        Zone::Forecast(p) => 13 + mine(p),
        Zone::Display(p) => 15 + mine(p),
        Zone::Museum(p) => 17 + mine(p),
        Zone::Safe(p) => 19 + mine(p),
    }
}

/// Token accumulation: entries are merged by (group, card, code) at the end,
/// summing weights and keeping the first value, in key order.
/// (group, card, code) with (weight, value).
type Entry = ((Group, u16, u32), (f32, f32));

struct Tokens(Vec<Entry>);

impl Tokens {
    fn add(&mut self, group: Group, card: u16, weight: f32, code: u32, value: f32) {
        self.0.push(((group, card, code), (weight, value)));
    }

    /// A hidden or private zone from the viewer's beliefs: each slot spreads
    /// one card over its candidates. Positional zones give each slot a code
    /// bit: decks by depth from the top, Safes by slot index (the index an
    /// option's `SLOT` field names, plus one); bit 8 collects the rest.
    /// Unordered zones have code 0.
    fn beliefs(&mut self, game: &Game, viewer: Player, zone: Zone, group: Group) {
        let tracker = game.tracker(viewer).expect("tracked game");
        let slots = tracker.belief_masks(zone);
        let len = slots.len();
        let mut merged: Vec<(u16, f32, u32)> = Vec::new();
        for (index, &(back, mask)) in slots.iter().enumerate() {
            let weight = 1.0 / mask.count_ones() as f32;
            let code = match zone {
                // Decks are positional: index 0 is the bottom.
                Zone::Deck(_) => 1 << (len - 1 - index).min(8),
                // Safe slots are oldest first, in option slot order.
                Zone::Safe(_) => 1 << index.min(8),
                _ => 0,
            };
            for (i, &card) in tracker.roster(back).iter().enumerate() {
                if mask & (1 << i) != 0 {
                    merged.push((stable(game, card), weight, code));
                }
            }
        }
        merged.sort_by_key(|&(card, _, _)| card);
        let mut merged = merged.into_iter().peekable();
        while let Some((card, mut weight, mut code)) = merged.next() {
            while let Some(&(next, w, c)) = merged.peek() {
                if next != card {
                    break;
                }
                weight += w;
                code |= c;
                merged.next();
            }
            self.add(group, card, weight, code, 0.0);
        }
    }

    /// Merged tokens in (group, card, code) order.
    fn finish(mut self) -> Vec<Token> {
        self.0.sort_by_key(|&(key, _)| key);
        let mut tokens: Vec<Token> = Vec::with_capacity(self.0.len());
        for ((group, card, code), (weight, value)) in self.0 {
            match tokens.last_mut() {
                Some(last) if last.group == group && last.card == card && last.code == code => {
                    last.weight += weight;
                }
                _ => tokens.push(Token {
                    card,
                    group,
                    weight,
                    code,
                    value,
                }),
            }
        }
        tokens
    }
}

fn kind_code(alternative: &Alternative) -> u8 {
    match alternative {
        Alternative::Value(_) => kind::VALUE,
        Alternative::Count(_) => kind::COUNT,
        Alternative::Icon(_) | Alternative::BonusIcon => kind::ICON,
        Alternative::Colors(_) => kind::COLORS,
        Alternative::MatchingPair { .. } => kind::MATCHING_PAIR,
        Alternative::Branch(_) => kind::BRANCH,
        Alternative::BlindSelect { .. } => kind::BLIND_SELECT,
        Alternative::Decree(_) => kind::DECREE,
        Alternative::Draw { .. } => kind::DRAW_CHOICE,
        Alternative::Select { .. } => kind::SELECT,
        Alternative::Splay { .. } => kind::SPLAY,
        Alternative::Exchange { .. } => kind::EXCHANGE,
        Alternative::BlindMove { .. } => kind::BLIND_MOVE,
        Alternative::Deck(_) => kind::DECK,
        Alternative::NameFigure(_) => kind::NAME_FIGURE,
        Alternative::Pass => kind::PASS,
        Alternative::Decline => kind::DECLINE,
        Alternative::JunkSpecial(_) => kind::JUNK_SPECIAL,
        Alternative::RestoreSpecial(_) => kind::RESTORE_SPECIAL,
        Alternative::ClaimSpecial(_) => kind::CLAIM_SPECIAL,
        Alternative::Move { .. } => kind::MOVE,
        Alternative::Reveal { .. } => kind::REVEAL,
        Alternative::SelfExecute(_) => kind::SELF_EXECUTE,
        Alternative::Karma { .. } => kind::KARMA,
        Alternative::Showcase { .. } => kind::SHOWCASE,
    }
}

/// The eight icon counts in the per-player scalars.
const ICONS: [Icon; 8] = [
    Icon::Authority,
    Icon::Concept,
    Icon::Health,
    Icon::Prosperity,
    Icon::Industry,
    Icon::Efficiency,
    Icon::Avatar,
    Icon::Food,
];

fn back_code(back: Deck) -> f32 {
    (back.index() + 1) as f32
}

impl Game {
    /// Encode what `viewer` knows at a settled prompt: a decision or action
    /// boundary (options only when the viewer chooses), or a finished game.
    /// Requires tracking from setup.
    pub fn observe(&self, viewer: Player) -> Result<Observation, Error> {
        if self.tracker(viewer).is_none() {
            return Err(Error::Unsupported(
                "observation requires tracking from setup",
            ));
        }
        let prompt = self.prompt_view();
        if matches!(prompt, PromptView::Busy) {
            return Err(Error::Busy);
        }
        let position = self.position();
        let them = viewer.opponent();
        let expansions = position.expansions();
        let sets = CardSet::ALL
            .into_iter()
            .filter(|&set| expansions.includes(set))
            .fold(0u8, |mask, set| mask | 1 << set.index());
        let target = 6 + expansions.count() as u8;

        let mut s = vec![0.0f32; SCALARS];
        for set in CardSet::ALL {
            s[scalar::SETS + set.index()] = f32::from(sets >> set.index() & 1);
        }
        s[scalar::TARGET] = f32::from(target) / 12.0;
        s[scalar::MY_TURN] = f32::from(u8::from(position.turn_owner() == viewer));
        s[scalar::ACTIONS_STARTED] = f32::from(position.regular_actions_started()) / 2.0;
        s[scalar::ACTION_LIMIT] = f32::from(position.regular_action_limit) / 2.0;
        s[scalar::DECIDING] = f32::from(u8::from(
            matches!(&prompt, PromptView::Decision(d) if d.player == viewer),
        ));
        s[scalar::ACTING] = f32::from(u8::from(
            matches!(prompt, PromptView::Turn(p) if p == viewer),
        ));
        s[scalar::AGE_ZERO] = f32::from(u8::from(position.age_zero_introduced()));
        for set in CardSet::ALL {
            for age in 0..Deck::AGES as u8 {
                let deck = Deck { set, age };
                s[scalar::DECKS + deck.index()] =
                    position.cards(Zone::Deck(deck)).len() as f32 / 15.0;
            }
        }
        for special in position.specials() {
            let slot = match position.special_location(special) {
                SpecialLocation::Available => 0,
                SpecialLocation::Owned(p) => 1 + owner(p, viewer),
                SpecialLocation::Junk => 3,
            };
            s[scalar::SPECIALS + 4 * special.index() + slot] = 1.0;
        }
        if expansions.figures() {
            for decree in Decree::ALL {
                let slot = match position.decree_location(decree) {
                    DecreeLocation::Supply => 0,
                    DecreeLocation::Owned(p) => 1 + owner(p, viewer),
                    DecreeLocation::Junk => 3,
                };
                s[scalar::DECREES + 4 * decree as usize + slot] = 1.0;
            }
        }
        if expansions.cities() {
            s[scalar::CITIES] = f32::from(u8::from(position.endorsement_used()));
            s[scalar::CITIES + 1] = f32::from(u8::from(self.endorsed_action()));
        }
        for player in [viewer, them] {
            let base = scalar::PLAYERS + owner(player, viewer) * scalar::PLAYER;
            s[base + scalar::SCORE] = position.score(player) as f32 / 50.0;
            s[base + scalar::BONUS] = position.bonus_score(player) as f32 / 20.0;
            s[base + scalar::ACHIEVEMENTS] =
                position.effective_achievement_count(player) as f32 / 12.0;
            s[base + scalar::CLAIMED] = position.achievement_count(player) as f32 / 12.0;
            s[base + scalar::HAND] = position.cards(Zone::Hand(player)).len() as f32 / 10.0;
            s[base + scalar::SCORE_PILE] = position.cards(Zone::Score(player)).len() as f32 / 20.0;
            s[base + scalar::TOP_VALUE] = f32::from(position.highest_top_value(player)) / 11.0;
            s[base + scalar::VISIBLE] = position.visible_card_count(player) as f32 / 20.0;
            let moves = position.turn_moves[player.index()];
            s[base + scalar::TUCKED] = f32::from(moves[0]) / 6.0;
            s[base + scalar::SCORED] = f32::from(moves[1]) / 6.0;
            s[base + scalar::INSPECTS] = f32::from(u8::from(
                position.can_view(Zone::Hand(player.opponent()), player),
            ));
            if expansions.echoes() {
                let limit = position.forecast_limit(player);
                s[base + scalar::FORECAST_LIMIT] = limit as f32 / 10.0;
                s[base + scalar::FORECAST_ROOM] =
                    limit.saturating_sub(position.cards(Zone::Forecast(player)).len()) as f32
                        / 10.0;
            }
            if expansions.unseen() {
                let limit = position.safe_limit(player);
                s[base + scalar::SAFE_LIMIT] = limit as f32 / 10.0;
                s[base + scalar::SAFE_ROOM] =
                    limit.saturating_sub(position.cards(Zone::Safe(player)).len()) as f32 / 10.0;
                s[base + scalar::FIRST_DRAWN] = f32::from(u8::from(position.first_drawn(player)));
            }
            for (i, icon) in ICONS.into_iter().enumerate() {
                s[base + scalar::ICONS + i] = position.icon_count(player, icon) as f32 / 20.0;
            }
            for color in Color::ALL {
                let at = base + scalar::COLORS + color as usize * scalar::COLOR_STRIDE;
                let splay = position.splay(player, color);
                s[at + splay as usize] = 1.0;
                let stack = position.cards(Zone::Board(player, color));
                s[at + 5] = stack.len() as f32 / 10.0;
                if let Some(&top) = stack.last() {
                    let card = position.card(top);
                    s[at + 6] = f32::from(position.value(top)) / 11.0;
                    s[at + 7] = f32::from(u8::from(card.figure.is_some()));
                }
            }
        }

        let mut tokens = Tokens(Vec::new());
        for set in CardSet::ALL {
            for age in 0..Deck::AGES as u8 {
                let zone = Zone::Deck(Deck { set, age });
                if !position.cards(zone).is_empty() {
                    tokens.beliefs(self, viewer, zone, Group::Decks);
                }
            }
        }
        tokens.beliefs(self, viewer, Zone::Junk, Group::Junk);
        tokens.beliefs(self, viewer, Zone::AvailableAchievements, Group::Supply);
        tokens.beliefs(self, viewer, Zone::AgeZeroReserve, Group::AgeZeroReserve);
        for player in [viewer, them] {
            let mine = player == viewer;
            let pick = |a, b| if mine { a } else { b };
            tokens.beliefs(
                self,
                viewer,
                Zone::Hand(player),
                pick(Group::MyHand, Group::TheirHand),
            );
            tokens.beliefs(
                self,
                viewer,
                Zone::Score(player),
                pick(Group::MyScore, Group::TheirScore),
            );
            tokens.beliefs(
                self,
                viewer,
                Zone::Achievements(player),
                pick(Group::MyAchievements, Group::TheirAchievements),
            );
            if expansions.echoes() {
                tokens.beliefs(
                    self,
                    viewer,
                    Zone::Forecast(player),
                    pick(Group::MyForecast, Group::TheirForecast),
                );
            }
            if expansions.unseen() {
                tokens.beliefs(
                    self,
                    viewer,
                    Zone::Safe(player),
                    pick(Group::MySafe, Group::TheirSafe),
                );
            }
            if expansions.artifacts() {
                tokens.beliefs(
                    self,
                    viewer,
                    Zone::Display(player),
                    pick(Group::MyDisplay, Group::TheirDisplay),
                );
                tokens.beliefs(
                    self,
                    viewer,
                    Zone::Museum(player),
                    pick(Group::MyMuseums, Group::TheirMuseums),
                );
            }
            let group = pick(Group::MyBoard, Group::TheirBoard);
            for color in Color::ALL {
                let stack = position.cards(Zone::Board(player, color));
                let splay = position.splay(player, color);
                for (index, &card) in stack.iter().enumerate() {
                    let above = stack.len() - 1 - index;
                    // Visibility bits for the card's printed positions (six on
                    // a City, four elsewhere), then top/second/third and
                    // bottom/second/third.
                    let mut code = 0u32;
                    for slot in 0..position.card(card).printed().len() {
                        if above == 0 || splay.reveals(slot) {
                            code |= 1 << slot;
                        }
                    }
                    if above < 3 {
                        code |= 1 << (6 + above);
                    }
                    if index < 3 {
                        code |= 1 << (9 + index);
                    }
                    tokens.add(group, stable(self, card), 1.0, code, above as f32 / 5.0);
                }
            }
        }
        let mut context_tokens = Vec::new();
        for record in self.context_records(viewer) {
            let (card, back) = match record.sight {
                Some(Sight::Card(card)) => (stable(self, card), None),
                Some(Sight::Back(back)) => (0, Some(back.index() as u64 + 1)),
                None => (0, None),
            };
            context_tokens.push(Token {
                card,
                group: Group::Context,
                weight: 1.0,
                code: context::HEADER + record.kind as u32,
                value: 0.0,
            });
            for (field, value) in std::iter::once((context::Field::Parent, record.parent as u64))
                .chain(back.map(|back| (context::Field::Back, back)))
                .chain(record.fields)
            {
                let limbs = if value == 0 {
                    1
                } else {
                    (64 - value.leading_zeros()).div_ceil(16)
                };
                for limb in 0..limbs {
                    context_tokens.push(Token {
                        card: 0,
                        group: Group::Context,
                        weight: 1.0,
                        code: context::FIELD + 4 * field as u32 + limb,
                        value: ((value >> (16 * limb)) & 0xffff) as f32,
                    });
                }
            }
        }

        let mut options = Vec::new();
        match &prompt {
            PromptView::Decision(decision) if decision.player == viewer => {
                for alternative in &decision.options {
                    options.push(self.option_row(alternative, viewer));
                }
            }
            PromptView::Turn(player) if *player == viewer => {
                for action in self.legal_actions() {
                    options.push(self.action_row(action, viewer));
                }
            }
            _ => {}
        }
        for option in &options {
            let card = option[field::CARD] as u16;
            if card != 0 {
                let class = option_class(option[field::KIND] as u8);
                tokens.add(Group::Options, card, 1.0, class, 0.0);
            }
        }
        Ok(Observation {
            viewer,
            sets,
            target,
            scalars: s,
            tokens: tokens.finish().into_iter().chain(context_tokens).collect(),
            options,
        })
    }

    fn action_row(&self, action: Action, viewer: Player) -> [f32; OPTION_FIELDS] {
        let position = self.position();
        let mut row = [0.0; OPTION_FIELDS];
        let card = |row: &mut [f32; OPTION_FIELDS], id: CardId| {
            let zone = position.locate(id).expect("legal action card");
            row[field::ZONE] = f32::from(zone_code(zone, viewer));
            row[field::BACK] = back_code(position.card(id).origin_deck());
            if position.can_view(zone, viewer) || matches!(zone, Zone::Board(..)) {
                row[field::CARD] = f32::from(stable(self, id));
                row[field::COLORS] = f32::from(1u8 << position.card(id).color as u8);
            }
            // A secret is named by its public Safe slot and back.
            if matches!(zone, Zone::Safe(_)) {
                let slot = position
                    .cards(zone)
                    .iter()
                    .position(|&c| c == id)
                    .expect("located secret");
                row[field::SLOT] = (slot + 1) as f32;
            }
        };
        match action {
            Action::Draw => {
                row[field::KIND] = f32::from(kind::DRAW);
                row[field::NUMBER] = f32::from(position.highest_top_value(viewer));
            }
            Action::Meld { card: id } => {
                row[field::KIND] = f32::from(kind::MELD);
                card(&mut row, id);
            }
            // A standard achievement is face down: only its back is named.
            Action::Achieve { card: id } => {
                row[field::KIND] = f32::from(kind::ACHIEVE);
                card(&mut row, id);
            }
            Action::Dogma { card: id, auspice } => {
                row[field::KIND] = f32::from(kind::DOGMA);
                card(&mut row, id);
                row[field::SLOT] = f32::from(u8::from(auspice));
            }
            Action::Decree => row[field::KIND] = f32::from(kind::DECREE_ACTION),
        }
        row
    }

    fn target_fields(
        &self,
        row: &mut [f32; OPTION_FIELDS],
        target: &TargetDescriptor,
        viewer: Player,
    ) {
        match *target {
            TargetDescriptor::Known { card, source } => {
                row[field::CARD] = f32::from(stable(self, card));
                row[field::ZONE] = f32::from(zone_code(source, viewer));
                row[field::BACK] = back_code(self.position().card(card).origin_deck());
                row[field::COLORS] = f32::from(1u8 << self.position().card(card).color as u8);
            }
            TargetDescriptor::Hidden { source, slot, back } => {
                row[field::ZONE] = f32::from(zone_code(source, viewer));
                row[field::BACK] = back_code(back);
                row[field::SLOT] = (slot + 1) as f32;
            }
        }
    }

    fn option_row(&self, alternative: &Alternative, viewer: Player) -> [f32; OPTION_FIELDS] {
        let mut row = [0.0; OPTION_FIELDS];
        row[field::KIND] = f32::from(kind_code(alternative));
        let zone_owner = |zone: Zone| match zone {
            Zone::Hand(p)
            | Zone::Score(p)
            | Zone::Achievements(p)
            | Zone::Board(p, _)
            | Zone::Forecast(p)
            | Zone::Safe(p) => (1 + owner(p, viewer)) as f32,
            _ => 0.0,
        };
        match alternative {
            Alternative::Value(value) => row[field::NUMBER] = f32::from(*value),
            Alternative::Count(count) => row[field::NUMBER] = f32::from(*count),
            Alternative::Icon(icon) => row[field::ICON] = (*icon as u8 + 1) as f32,
            Alternative::BonusIcon => row[field::ICON] = 9.0,
            Alternative::Colors(mask) => row[field::COLORS] = f32::from(*mask),
            Alternative::MatchingPair { first, second } => {
                self.target_fields(&mut row, first, viewer);
                if let TargetDescriptor::Known { card, .. } = second {
                    row[field::SECOND] = f32::from(stable(self, *card));
                }
            }
            Alternative::Branch(branch) => row[field::NUMBER] = *branch as f32,
            Alternative::BlindSelect { source, back } => {
                row[field::ZONE] = f32::from(zone_code(*source, viewer));
                row[field::BACK] = back_code(*back);
                row[field::OWNER] = zone_owner(*source);
            }
            Alternative::Decree(decree) => {
                row[field::TOKEN] = f32::from(field::DECREE_TOKENS + *decree as u8);
            }
            Alternative::Draw { value, set } => {
                row[field::NUMBER] = f32::from(*value);
                row[field::BACK] = back_code(Deck {
                    set: *set,
                    age: *value,
                });
            }
            Alternative::Select { target } | Alternative::Reveal { target } => {
                self.target_fields(&mut row, target, viewer);
            }
            Alternative::Splay {
                player,
                color,
                direction,
            } => {
                row[field::OWNER] = (1 + owner(*player, viewer)) as f32;
                row[field::COLORS] = f32::from(1u8 << *color as u8);
                row[field::NUMBER] = *direction as u8 as f32;
            }
            Alternative::Exchange { left, right } => {
                row[field::ZONE] = f32::from(zone_code(left.home(), viewer));
                row[field::SECOND_ZONE] = f32::from(zone_code(right.home(), viewer));
            }
            Alternative::BlindMove { verb, source, back } => {
                row[field::VERB] = (*verb as u8 + 1) as f32;
                row[field::ZONE] = f32::from(zone_code(*source, viewer));
                row[field::BACK] = back_code(*back);
                row[field::OWNER] = zone_owner(*source);
            }
            Alternative::Deck(deck) => row[field::BACK] = back_code(*deck),
            // A named Figure is a card identity, like any named card.
            Alternative::NameFigure(name) => {
                row[field::CARD] =
                    crate::StableId::from_code(name.code).map_or(0.0, |id| f32::from(id.0 + 1));
                row[field::NUMBER] = f32::from(name.age);
            }
            Alternative::Pass | Alternative::Decline => {}
            Alternative::JunkSpecial(special)
            | Alternative::RestoreSpecial(special)
            | Alternative::ClaimSpecial(special) => {
                row[field::TOKEN] = (special.index() + 1) as f32;
            }
            Alternative::Move { verb, target } => {
                row[field::VERB] = (*verb as u8 + 1) as f32;
                self.target_fields(&mut row, target, viewer);
            }
            Alternative::SelfExecute(card) => {
                row[field::CARD] = f32::from(stable(self, *card));
            }
            Alternative::Showcase { card, auspice } => {
                row[field::CARD] = f32::from(stable(self, *card));
                row[field::SLOT] = f32::from(u8::from(*auspice));
            }
            Alternative::Karma { source, .. } => {
                row[field::CARD] = f32::from(stable(self, *source));
            }
        }
        if let Alternative::Move { target, .. }
        | Alternative::Select { target }
        | Alternative::Reveal { target } = alternative
        {
            let source = match target {
                TargetDescriptor::Known { source, .. }
                | TargetDescriptor::Hidden { source, .. } => *source,
            };
            row[field::OWNER] = zone_owner(source);
        }
        row
    }
}

/// Splay one-hot order in the color scalars.
const _: () = assert!(Splay::Aslant as usize == 4);
/// Twenty reserved special slots fit the scalar section, and the Decree
/// token codes follow them.
const _: () = assert!(Special::RESERVED == 20);
const _: () = assert!(field::DECREE_TOKENS as usize == Special::RESERVED + 1);
