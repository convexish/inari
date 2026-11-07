//! The card language. A printed effect is a short list of [`Instruction`]s;
//! the interpreter resumes it at every operation or decision boundary.
//!
//! Fifteen instruction kinds cover the Base catalog. Shared operand types
//! do the rest: a [`Selector`] names candidate cards, a [`Filter`] narrows
//! them, an [`Expr`] computes a number and a [`Cond`] guards a skip. Each
//! effect keeps three registers: the result of the previous instruction,
//! a bound card ([`Reg::Bound`]) and a chosen value, icon or color set.
//! Results of reactions never reach these registers.
//!
//! Printed boxed values (`[3]`) are [`Expr::Boxed`] or
//! [`Test::BoxedValues`] and follow the invocation's Archimedes/Kepler
//! scope; ordinary numbers are [`Expr::Const`] and never do.
//!
//! A card that must remember more than one thing names [`Slot`]s: an
//! instruction records what it actually did with `.keep(slot)` or
//! `.add(slot)`, and operands read the slot back.
use crate::model::{CardSet, CityIcon, Color, Icon, Splay, Verb};
use crate::{ExecutionMode, Special};

/// Execution of a retained card with scoped modifiers (Unseen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Execute {
    /// The card to execute, wherever it is now.
    pub target: Reg,
    /// Self, super, or one non-demand dogma effect by index.
    pub mode: ExecutionMode,
    /// Who executes it ("you"; Hammurabi's demander).
    pub who: Who,
    /// Blackmail: every "may" inside becomes mandatory, while quantity stops
    /// and real alternatives remain. Nested executions inherit it.
    pub must: bool,
    /// Hitchhiking: this player makes every decision inside, including those
    /// assigned to another player; nested executions inherit it.
    pub controller: Option<Who>,
    /// The controller may look at what the executor may (hand, score and
    /// Forecast), later arrivals included, and remembers it afterwards.
    pub inspect: bool,
    /// Whether reveals alone count as the execution's activity (The Big
    /// Bang counts them; Teleprompter's variant does not).
    pub count_reveals: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Executor,
    Opponent,
    Demander,
    /// Both players: selections and counts cover both, sums add both.
    Either,
}

/// A card an instruction can refer to after it was chosen or operated on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    /// The card of the previous instruction's successful original operation.
    Last,
    /// The card retained by [`Instruction::Bind`].
    Bound,
    /// The card whose effect is executing.
    Source,
    /// The card picked by a [`Choice::Card`].
    Chosen,
    /// The latest card recorded in the slot.
    Slot(Slot),
}

/// How long a [`Slot`] remembers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    /// Fresh for each effect invocation, sharer and owner separately.
    Effect,
    /// One card execution (a Dogma, or one self- or super-execution): a
    /// demand's record is visible to that execution's later effects.
    Execution,
    /// The enclosing action, across sharing, endorsement and copies; a new
    /// free action starts fresh.
    Action,
}

/// One of a card's named memories. It holds what
/// an instruction recorded: cards (each a retained reference), a number, a
/// color set or an icon. A card numbers its slots from zero; its echo and
/// dogma effects share them. Execution and action slots keep one record
/// per player, written by the executing player; the slot's owner says whose
/// record a read means ("the color I last melded" in a demand is the
/// demander's). A keep `of(Who::Either)` writes every player's record: a
/// history the whole action shares, whoever executed (Brethren of Purity's
/// "last card melded due to Brethren during this action"). Packed into one
/// byte so operands stay small: the index in bits 0-1, the lifetime in bits
/// 2-3 and the owner in bits 4-5.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Slot(u8);

impl Slot {
    /// The number of slots a card may declare.
    pub const COUNT: usize = 4;

    pub const fn effect(index: u8) -> Self {
        Self::new(index, Life::Effect)
    }
    pub const fn execution(index: u8) -> Self {
        Self::new(index, Life::Execution)
    }
    pub const fn action(index: u8) -> Self {
        Self::new(index, Life::Action)
    }
    const fn new(index: u8, life: Life) -> Self {
        assert!((index as usize) < Self::COUNT);
        Self(index | (life as u8) << 2)
    }
    /// Read that player's record.
    pub const fn of(self, owner: Who) -> Self {
        Self(self.0 & 0b1111 | (owner as u8) << 4)
    }
    pub const fn index(self) -> u8 {
        self.0 & 0b11
    }
    pub const fn life(self) -> Life {
        match self.0 >> 2 & 0b11 {
            0 => Life::Effect,
            1 => Life::Execution,
            _ => Life::Action,
        }
    }
    pub const fn owner(self) -> Who {
        match self.0 >> 4 {
            0 => Who::Executor,
            1 => Who::Opponent,
            2 => Who::Demander,
            _ => Who::Either,
        }
    }
    /// This slot's bit in a [`Instruction::Repeat`] keep mask.
    pub const fn bit(self) -> u8 {
        1 << self.index()
    }
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("index", &self.index())
            .field("life", &self.life())
            .field("owner", &self.owner())
            .finish()
    }
}

/// An instruction's record of what it actually did: replace the slot, or
/// append to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keep {
    pub slot: Slot,
    pub append: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Equal,
    Less,
    AtMost,
    Greater,
    AtLeast,
}

impl Cmp {
    pub(crate) fn holds(self, left: i64, right: i64) -> bool {
        match self {
            Self::Equal => left == right,
            Self::Less => left < right,
            Self::AtMost => left <= right,
            Self::Greater => left > right,
            Self::AtLeast => left >= right,
        }
    }
}

/// Player quantities other than card counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// Signed effective score.
    Score,
    /// Effective achievements, including virtual ones.
    Achievements,
    /// Owned special achievements (Hostage's "each special achievement you
    /// have").
    Specials,
    /// Effective icon count.
    Icons(Icon),
    /// Effective count of the chosen icon.
    ChosenIcon,
    /// Visible bonus points: the largest bonus plus one per further bonus.
    BonusPoints,
    /// Visible bonus icons, one icon type.
    Bonuses,
    /// Visible City icons of one kind, each an icon type of its own.
    CityIcons(CityIcon),
    /// How many icon types reach this effective count on the board (Symbology's "four
    /// each of four icons"): standard icons, Food once Age 0 is in play, each City icon
    /// kind, and visible bonuses as one type.
    IconTypesAtLeast(u8),
}

/// Visible bonuses on a board, or on both (Echoes): a multiset of values.
/// A covered bonus counts where its pile's splay shows it; a City's repeated
/// bonus counts each time it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bonuses {
    pub on: Who,
    pub only: BonusValues,
}

/// Which of the visible bonuses a [`Bonuses`] query keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BonusValues {
    All,
    Even,
    Odd,
    /// Values shown more than once.
    Repeated,
    /// That value only.
    Equal(Expr),
}

/// A batch's per-value quota source, sampled once at entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueSource {
    Bonuses(Bonuses),
    /// Printed values of logical card members; duplicates follow multiplicity.
    Cards(&'static Selector),
}

/// [`Amount::PerValue`]'s frozen cap for every printed card value.
/// Barcode uses every bonus; Toilet and Indian Clubs use distinct values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerValue {
    pub source: ValueSource,
    pub multiplicity: Multiplicity,
}

/// What [`Expr::Bonuses`] reads from the multiset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tally {
    /// Every bonus.
    Count,
    /// The highest value, or zero.
    Max,
    /// Different values.
    Distinct,
}

/// A public visibility query over the named boards and colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visibility {
    pub who: Who,
    pub colors: ColorSet,
    pub seen: Seen,
}

/// What [`Expr::Visible`] counts in a pile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    Cards,
    Icon(Icon),
    /// Image hexes.
    Images,
    /// Printed echo-effect hexes.
    Echoes,
    /// The visible icons of the single most common type, over the whole
    /// query (Puffing Billy's "icons of the same type in that color"): a
    /// standard icon, bonuses as one type, or a City icon kind.
    MostCommonIcon,
}

/// One printed symbol whose own slot must be visible on a physical board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Icon(Icon),
    Image,
    Echo,
}

/// A number. Missing registers make an expression undefined, and an
/// instruction with an undefined age or quantity does nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expr {
    Const(i64),
    /// A printed boxed value in this invocation's boxed-value scope.
    Boxed(u8),
    Count(&'static Selector),
    /// Highest printed value among the cards, or zero.
    Max(&'static Selector),
    /// Lowest value among the cards; undefined when there are none.
    Min(&'static Selector),
    /// Number of different colors among the cards.
    DistinctColors(&'static Selector),
    /// Number of different printed values among the cards.
    Distinct(&'static Selector),
    /// Sum of the cards' printed values.
    Total(&'static Selector),
    /// Printed value of a register card.
    Value(Reg),
    /// Printed icons of this kind on a register card.
    IconsOn(Reg, Icon),
    /// Printed icons of this kind on all of these cards (Quackery's
    /// "number of Health and Concept on the returned cards").
    IconsAmong(&'static Selector, Icon),
    /// Different printed icon types among these cards, bonuses one type and
    /// each City icon kind its own (Probability).
    IconTypesAmong(&'static Selector),
    /// The chosen value.
    Chosen,
    /// The number a slot holds.
    Slot(Slot),
    /// Successful original operations of the previous instruction.
    Done,
    /// Different printed values among those operations.
    DoneValues,
    /// Cards the preceding demand of this execution moved with that verb
    /// (Exile's "exactly one card was returned due to the demand").
    DemandMoved(Verb),
    Metric(Who, Metric),
    /// Sum clamped at zero.
    Add(&'static Expr, i64),
    Mul(&'static Expr, i64),
    /// Integer division, rounding down ("for every two").
    Div(&'static Expr, i64),
    /// Undefined when zero ("draw a card of that value" after nothing).
    NonZero(&'static Expr),
    /// Sum of defined terms; undefined if any term is.
    Sum(&'static [Expr]),
    /// Visible cards, icons or images on the requested boards and colors.
    Visible(&'static Visibility),
    /// The most cards any one color shows on that player's board.
    MostVisibleInAColor(Who),
    /// Cards the executor tucked (or scored) this turn.
    ThisTurn(Verb),
    /// Visible bonuses.
    Bonuses(&'static Bonuses, Tally),
    /// Printed bonus of a register card; undefined without one.
    BonusOf(Reg),
}

/// A property of one card, judged where the card currently is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    Icon(Icon),
    /// The public expansion printed on the back.
    Set(CardSet),
    Color(Color),
    Code(&'static str),
    Figure,
    Demand,
    Value(Cmp, Expr),
    /// Ordinary printed values.
    Values(&'static [u8]),
    /// Its printed value occurs among the source cards; expansion sets and
    /// duplicate witnesses do not affect membership.
    ValueIn(&'static Selector),
    /// Printed boxed values, adjusted by the invocation scope.
    BoxedValues(&'static [u8]),
    /// Same color as a register card.
    ColorOf(Reg),
    /// A color in the set.
    ColorIn(ColorSet),
    ChosenIcon,
    /// The chosen standard icon is visible on this card's own board slots.
    ShowsChosenIcon,
    /// This exact card shows the symbol through its current physical splay.
    Shows(Shown),
    /// The standard chosen type is this card's featured icon.
    FeaturedChosenIcon,
    /// That player has a top card of this card's color.
    ColorOnBoard(Who),
    /// Value exceeds that player's top card of the same color (or zero).
    AboveTop(Who),
    Is(Reg),
    /// The card is recorded in the slot.
    InSlot(Slot),
    /// Physical-zone inspection permission, not remembered identity or logical membership.
    Inspectable(Who),
    /// This card's board stack is splayed in that direction (any if None).
    StackSplayed(Option<Splay>),
    /// This card's board stack shows the icon.
    StackShows(Icon),
    /// The executor could claim this card as an achievement (points and top
    /// value), whether or not it is in the supply.
    Eligible {
        score_factor: u8,
    },
    /// Same printed featured icon as a register card.
    FeaturedAs(Reg),
    /// At least one shared printed icon type, counting every bonus as one type.
    SharesIcon(Reg),
    /// At least one printed icon type shared with any of these cards
    /// (Polytheism's "no icon on a card already melded").
    SharesIconWith(&'static Selector),
    /// Without this card's points the executor could still claim both this
    /// card and the register card as achievements.
    EligibleBoth(Reg),
    /// The card has a printed bonus.
    Bonus,
    /// The card's title comes before the register card's in the alphabet
    /// (lowercase alphanumeric comparison, accents folded).
    TitleBefore(Reg),
    /// The card lies on a board with a top card printed in this set.
    BoardHasTopSet(CardSet),
    /// The card's color shows more cards on its own board than on that
    /// player's board (Galley Whydah).
    MoreVisibleCardsThan(Who),
    /// The card's color shows fewer cards on its board than the register
    /// card's color does there (Pride and Prejudice).
    FewerVisibleCardsThanColorOf(Reg),
    /// That player's board holds this many cards of the card's color.
    CardsOfColorOnBoard(Who, Cmp, Expr),
    /// The card's value occurs at least twice among these cards.
    ValueRepeated(&'static Selector),
    /// How often the card's value occurs among these cards, compared to a
    /// count (Red Envelope's "exactly two or three cards altogether").
    ValueCount(&'static Selector, Cmp, u8),
    /// The card prints no effects at all: a City or Battleship Yamato, never
    /// a Figure (Garland's Ruby Slippers appendix).
    NoPrintedEffects,
}

impl Test {
    /// Tests that read a card's face. Selecting among hidden cards with them
    /// would turn a menu into an oracle, so they require visible candidates.
    pub(crate) fn reads_front(self) -> bool {
        matches!(
            self,
            Self::Icon(_)
                | Self::Color(_)
                | Self::Code(_)
                | Self::Demand
                | Self::ColorOf(_)
                | Self::ColorIn(_)
                | Self::ChosenIcon
                | Self::ShowsChosenIcon
                | Self::Shows(_)
                | Self::FeaturedChosenIcon
                | Self::ColorOnBoard(_)
                | Self::AboveTop(_)
                | Self::FeaturedAs(_)
                | Self::SharesIcon(_)
                | Self::SharesIconWith(_)
                | Self::Bonus
                | Self::TitleBefore(_)
                | Self::NoPrintedEffects
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Has(Test),
    Lacks(Test),
    /// Lowest printed value among the cards so far, keeping ties.
    Lowest,
    Highest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardRegion {
    Top,
    Bottom,
    /// Up to the bottom `n` cards of each selected board.
    BottomN(u8),
    /// All cards except the live top `n`.
    BelowTop(u8),
    /// All cards except the bottom one.
    AboveBottom,
    /// Beneath the bound card, wherever it is on a board.
    BeneathBound,
    /// Only the card directly beneath the bound card (Propaganda).
    JustBeneathBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSet {
    /// In printed order.
    Only(&'static [Color]),
    /// The color of a register card.
    Of(Reg),
    /// The chosen colors.
    Chosen,
    /// The colors of these cards.
    OfCards(&'static Selector),
    /// The colors a slot holds.
    Slot(Slot),
    /// Every color tied for the most physical cards across these boards.
    MostCards(Who),
    /// Every color tied for the most visible cards on that player's board
    /// (Jackalope).
    MostVisible(Who),
}

/// Candidate cards, in a stable order. Hands and score piles are logical
/// (Figures may alias other physical cards into them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selector {
    Hand(Who),
    Score(Who),
    Achievements(Who),
    Tops(Who),
    TopOf(Who, Color),
    /// Explicit permission to select covered cards.
    Board(Who, ColorSet, BoardRegion),
    /// Standard supply plus the executor's live board aliases.
    AvailableAchievements,
    /// Only physical standard cards in the supply, excluding Figure-granted
    /// board aliases (printed "standard achievement").
    StandardAchievements,
    /// Physical valued cards in the junk; special tokens are not cards.
    Junk,
    /// The top card of every enabled deck of every set (Dragon's Lair).
    DeckTops,
    /// The physical Base deck of that age; absent ages are empty.
    BaseDeck(Expr),
    /// The physical deck of that set and age (Artifacts' "the Artifact deck
    /// of that value"); disabled sets and absent ages are empty.
    SetDeck(CardSet, Expr),
    /// Only the physical top of the exact Base deck; never a draw.
    BaseDeckTop(Expr),
    /// Hands, score piles and board tops of both players.
    InPlay,
    /// Hands, score piles, Forecasts and entire boards (Fission's
    /// "non-achievement cards").
    Owned(Who),
    /// A register card wherever it is.
    Reg(Reg),
    /// The cards recorded in a slot, wherever they are now. Programs narrow
    /// them to the zone the text means with [`Test::InSlot`].
    Slot(Slot),
    /// That player's Forecast (Echoes).
    Forecast(Who),
    /// That player's secrets (Unseen), in arrival order. Hidden from the
    /// chooser too: menus offer public slots and backs, never fronts.
    Safe(Who),
    /// That player's stored Artifacts (Artifacts), a public pile.
    Museum(Who),
    Union(&'static [Selector]),
    Filtered(&'static Selector, &'static [Filter]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Hand(Who),
    Score(Who),
    /// The pile of the moved card's color.
    Board(Who),
    /// Board of the latest successful card's original owner in this slot.
    Origin(Slot),
    /// The moved card's origin deck.
    Deck,
    Junk,
    Achievements(Who),
    /// The available (standard) achievements.
    Supply,
    /// That player's Forecast (Echoes).
    Forecast(Who),
    /// That player's Safe (Unseen). Capacity can refuse the card.
    Safe(Who),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Never,
    /// Decline before the first card, then do as much as possible.
    Initially,
    /// Stop after any card ("up to", "any number").
    AnyTime,
    /// "You may... up to / any number": stop after any card. Under Blackmail's `must`
    /// the first card is required, then stopping remains.
    MayAnyTime,
}

impl Stop {
    /// Stopping is allowed after any card.
    pub const fn any_time(self) -> bool {
        matches!(self, Self::AnyTime | Self::MayAnyTime)
    }

    /// Whether the chooser may stop now: before the first card (`first`) or
    /// later, with `must` removing a "may" decline.
    pub const fn may_stop(self, first: bool, must: bool) -> bool {
        match self {
            Self::Never => false,
            Self::Initially => first && !must,
            Self::AnyTime => true,
            Self::MayAnyTime => !(first && must),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    /// One card; the result is that operation.
    One { optional: bool },
    /// Card after card, re-selecting from live state and considering each
    /// identity once. `per_color` also considers each color once. The
    /// limit is evaluated once, before the first card.
    Batch {
        limit: Option<Expr>,
        stop: Stop,
        per_color: bool,
        /// Snapshot membership before the first operation; origins stay fixed.
        frozen: bool,
    },
    /// A batch of as many cards of each value as the cap allows, as many
    /// as possible; the caps are counted before the first card.
    PerValue(&'static PerValue),
    /// An exact group committed before any operation or reaction. Unlike a
    /// batch, an incomplete group cannot begin. Optionality belongs to the
    /// group, so declining never permits a partial commitment.
    Group {
        size: u8,
        constraint: GroupConstraint,
        optional: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupConstraint {
    SameValue,
    DifferentValues,
    SameValueDifferentColors,
    /// Myth's "two cards of the same color".
    SameColor,
}

/// A choice reduction. Every semantic operation still happens; only
/// interchangeable orders and selections stop being separate decisions. Each
/// kind checks its own preconditions at every choice and falls back to the
/// full menu whenever a live Figure could observe the difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Take the lowest card first (Oars policy).
    LowestFirst,
    /// Cards bound for different origin decks or piles commute. Same-deck
    /// orders stay real choices. `quota`: also reduce when some cards stay.
    ByOrigin { descending: bool, quota: bool },
    /// Board placements of different colors commute.
    ByColor,
    /// Every order of a whole-deck junk is the same: take the top card.
    DeckTop,
    /// Every order is the same: take the first candidate (or a block's first
    /// item). On a mandatory single checked claim, the author additionally
    /// promises an exhausting repeat after each successful claim. That form
    /// groups only independent ages and preserves all within-age back choices.
    AnyOrder,
    /// A sharer's mandatory pick from at most two candidates that the owner
    /// picks from next with the same effect: both go, so take the first.
    SharedPool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub verb: Verb,
    pub from: Selector,
    pub to: Destination,
    pub amount: Amount,
    pub shortcut: Option<Shortcut>,
    /// Reveal the admitted selected card when this condition holds. Within
    /// this condition only, Bound denotes the selected operand; slots include
    /// the current instruction's successful original records.
    pub verify: bool,
    pub when: Option<&'static Cond>,
    /// For a mandatory single selection with no eligible card, reveal this
    /// completeness proof and retain an inapplicable movement result.
    pub reveal_if_empty: Option<&'static Selector>,
    /// Record the cards actually moved.
    pub keep: Option<Keep>,
    /// The player who performs (and chooses) this single move, when it is
    /// not the program's executor: Hitchhiking's donor transfers a card from
    /// their hand to the executor's board. Selectors and the destination
    /// still read "you" as the program's executor.
    pub by: Option<Who>,
}

/// What happens to each card of a counted draw before the next draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Keep,
    Score,
    Reveal,
    Achieve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw {
    pub age: Expr,
    pub set: CardSet,
    /// The player chooses the set among the enabled ones ("from any set").
    pub any_set: bool,
    /// `None`: one draw whose result is that operation. Otherwise a counted
    /// series; zero or undefined draws nothing.
    pub count: Option<Expr>,
    pub then: Then,
    pub optional: bool,
    /// Record the cards actually drawn (not a karma's extra draws).
    pub keep: Option<Keep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueChoices {
    /// Every legal card value, including zero once Age 0 is introduced.
    Any,
    /// Every legal card value except these printed boxed values.
    AnyExceptBoxed(&'static [u8]),
    /// A printed number range, inclusive (not card values).
    Range(u8, u8),
    /// Every odd card value (Buried Treasure).
    Odd,
    Boxed(&'static [u8]),
    /// The printed values present among these cards, ascending.
    Among(&'static Selector),
    /// Every legal value not present among these cards.
    NotAmong(&'static Selector),
    /// The values among these visible bonuses, ascending.
    Bonuses(&'static Bonuses),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Every standard type plus Bonus (also representing other non-featured
    /// symbol types). The optional public proof merges tied-leader no-ops;
    /// it changes aliases, never which icon choices are legal.
    IconTypes {
        merge_tied_leaders: bool,
    },
    Value(ValueChoices),
    /// Mona Lisa's commitment: how many cards of these colors the hand will
    /// hold after `draws` ordinary draws. With no Figure able to change the
    /// hand during the draws, use the drawn count `0..=draws`
    /// plus one deliberate failure, tested by [`Cond::FinalCount`] against
    /// the recorded draws. Otherwise a literal final count `0..=N` plus one
    /// impossible `N + 1`, `N` being the enabled catalog's cards of those
    /// colors.
    FinalCount {
        colors: ColorSet,
        draws: u8,
    },
    Icon {
        /// None offers every currently standard icon type.
        except: Option<Icon>,
    },
    /// Standard icons tied for that player's highest effective count.
    MostIcons(Who),
    /// A set of exactly this many colors.
    Colors(u8),
    /// This many colors among the set, or all of them when the set is
    /// smaller (Handshake's "two colors of cards in your hand").
    ColorsAmong(u8, ColorSet),
    /// One color from the set ("a color on your board").
    Color(ColorSet),
    /// Forward offsets from the next instruction, paired with player-facing
    /// wording. Declining an optional branch ends the effect. Wording uses
    /// the choosing player's perspective and never depends on hidden fronts.
    Branch(&'static [(u8, &'static str)]),
    /// One branch per listed set, offered only when that set is enabled.
    /// Parallel slices preserve branch identities, offsets and labels when
    /// disabled sets are omitted from the menu.
    SetBranch {
        sets: &'static [CardSet],
        branches: &'static [(u8, &'static str)],
    },
    /// One of these cards; it becomes [`Reg::Chosen`].
    Card(&'static Selector),
    /// An unordered set chosen completely before its consumer runs.
    /// The player may stop after any pick; `optional` permits zero picks.
    /// A constraint restricts which cards go together ("of different
    /// values") without requiring the full count.
    Cards {
        from: &'static Selector,
        up_to: u8,
        constraint: Option<GroupConstraint>,
    },
}

/// A printed splay request. Rare copied alternatives live behind a reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplayStep {
    pub who: Who,
    pub choices: SplayChoices,
    pub targets: SplayTargets,
    pub optional: bool,
    /// Every eligible color (and player), not one of them.
    pub all: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplayChoices {
    Fixed {
        colors: ColorSet,
        direction: Splay,
        from: Option<Splay>,
    },
    Options(&'static [SplayOption]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplayOption {
    pub colors: ColorSet,
    pub direction: SplayDirection,
    /// Only targets currently splayed this way.
    pub from: Option<Splay>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplayDirection {
    Fixed(Splay),
    /// Copy any of these piles' directions, resolved before the chosen splay.
    Copy {
        who: Who,
        colors: ColorSet,
        splayed_only: bool,
    },
    /// Copy this player's direction in the target color, including None.
    Matching(Who),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplayTargets {
    /// Only piles whose splay will change.
    Changes,
    /// Existing piles, including singletons and unchanged splays.
    Nonempty,
    /// Colors whether or not a pile exists.
    AnyColor,
}

impl SplayChoices {
    fn reads_operand_front(self, operand: Operand) -> bool {
        match self {
            Self::Fixed { colors, .. } => colors.reads_operand_front(operand),
            Self::Options(options) => options.iter().any(|option| {
                option.colors.reads_operand_front(operand)
                    || match option.direction {
                        SplayDirection::Copy { colors, .. } => colors.reads_operand_front(operand),
                        _ => false,
                    }
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExchangeStep {
    pub left: Selector,
    pub right: Selector,
    /// `None` exchanges every selected card; a count takes successive
    /// extrema without replacement. Counts are evaluated before either cohort
    /// is selected; limited destinations further cap admission.
    pub counts: &'static [Option<Expr>; 2],
    pub optional: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialStep {
    /// Claim without the token's usual requirement.
    Claim(Special),
    /// Junk one available special or restore one junked special.
    Toggle { optional: bool },
    /// Junk every available special.
    JunkAll,
    /// One quota across available standard cards and special tokens. The
    /// count is evaluated once, before the first junk.
    JunkAvailable {
        count: &'static Expr,
        shortcut: Option<Shortcut>,
    },
    /// Issue the Decree of the color in the set.
    Decree(ColorSet),
    /// Claim one available standard or special achievement, regardless of
    /// eligibility.
    ClaimAny,
    /// The executor has sole majority in every icon for the rest of the
    /// current action.
    Majority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WinRule {
    Executor,
    /// Propose this named player as winner; this is not an elimination.
    Player(Who),
    /// The game ends in a draw (Martian Internet's variant).
    Draw,
    /// That player loses (is eliminated).
    Loses(Who),
    /// The unique leader wins; ties change nothing.
    Most(Metric),
    Fewest(Metric),
}

/// What the previous instruction accomplished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Everything it attempted was performed, and it was more than nothing.
    All,
    Any,
    Exactly(u16),
}

/// Options of a value or colour choice that lead to the same place, so the
/// menu offers one of them (the first, in menu order). Applied only when
/// every player can tell which options those are, so the menu's shape
/// discloses nothing; the random-play oracle checks each annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alias {
    /// With the option as the choice, this condition marks a no-op: an
    /// empty deck to junk, a colour with no cards to move.
    NoOpWhen(&'static Cond),
    /// Options whose key agrees are interchangeable (Clock's visible count).
    SameKey(&'static Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    Compare(Expr, Cmp, Expr),
    /// Every selected card prints a common icon type other than `except`.
    /// Bonuses share one type; each non-Search City icon is its own type.
    /// Holds for an empty selection.
    CommonIcon {
        from: &'static Selector,
        except: Icon,
    },
    /// Equal printed values including multiplicities; two empty sets match.
    EqualValues(&'static Selector, &'static Selector),
    All(&'static [Cond]),
    Any(&'static [Cond]),
    Not(&'static Cond),
    /// False when the register is empty.
    Card(Reg, Filter),
    /// The slot holds nothing (no cards, number, colors or icon).
    Empty(Slot),
    /// Ordinary draws of these two values by the executor would come from
    /// the same deck, after age skipping and set redirection: "a [3] or one
    /// higher" is then no choice (Brethren of Purity).
    SameDraw(Expr, Expr),
    /// Some color in the set on that player's board is (or, if `present` is
    /// false, is not) splayed in that direction.
    Splayed {
        who: Who,
        colors: ColorSet,
        direction: Splay,
        present: bool,
    },
    Did(Outcome),
    /// A [`Choice::FinalCount`] commitment holds: a literal final count
    /// equals the count of `hand`, a drawn count the count of `drawn` (the
    /// recorded draws of the chosen colors), so no held count is retained.
    FinalCount {
        hand: &'static Selector,
        drawn: &'static Selector,
    },
    /// The preceding demand of this execution moved cards with that verb.
    DemandMoved(Verb),
    /// The previous counted reveal drew this color.
    DrawnColor(Color),
    DrawnColorTwice,
    /// This player returned more cards than the opponent through this card
    /// during the current action.
    ReturnedMost,
    /// Dancing Girl has been on the executor's opponent's board during the
    /// current action: where it lay as the action began, or a board it
    /// arrived on by any move since.
    DancingGirlVisited,
    /// The nearest real action is the executor's own regular action of this
    /// ordinal (1 or 2). Free actions have no ordinal.
    Action(u8),
    /// The executing effect's card was promoted for this free Dogma action
    /// (Echoes "if [this card] was foreseen").
    Foreseen,
    /// The executor owns the current turn, including copied effects.
    OwnTurn,
    /// Blackmail's `must` replaces "may" in this execution.
    Must,
}

/// What [`Instruction::Bind`] retains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// A card in the bound register ([`Reg::Bound`]), choosing if several
    /// qualify.
    Card(Selector),
    /// A number in an effect slot.
    Value(Slot, Expr),
    /// A color set in an effect slot.
    Colors(Slot, ColorSet),
}

/// Which values of a group a [`Domain::Values`] block visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Multiplicity {
    /// Each different value once.
    Distinct,
    /// Every occurrence.
    Every,
    /// Values appearing exactly this many times.
    Exactly(u8),
}

impl Multiplicity {
    /// How many items a value appearing `count` times yields.
    pub(crate) fn times(self, count: u16) -> usize {
        match self {
            Self::Distinct => usize::from(count > 0),
            Self::Every => usize::from(count),
            Self::Exactly(n) => usize::from(count == u16::from(n)),
        }
    }
}

/// The items a [`Block`] visits, one pass each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// One pass: groups a compound sentence ("if you do both").
    Once,
    /// That many passes, counted at entry.
    Times(Expr),
    /// The printed values among these cards, frozen at entry.
    Values(&'static Selector, Multiplicity),
    /// These visible bonus values, frozen at entry.
    Bonuses(&'static Bonuses, Multiplicity),
    /// Each value in the range; the body reads live state.
    Range(u8, u8),
    /// Each color in the set, frozen at entry.
    Colors(ColorSet),
    /// These cards (or a slot's records), frozen at entry.
    Cards(&'static Selector),
    /// Reselected before each pass, each card (or color) at most once: a
    /// `Move` batch with a body. `stop` lets the chooser end it.
    Live {
        from: &'static Selector,
        per_color: bool,
        stop: Stop,
    },
}

/// The order a [`Block`] visits its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Printed order: colors as listed, cards as selected, values ascending.
    Printed,
    /// By value, lowest first.
    Ascending,
    /// By value, highest first.
    Descending,
    /// That player picks each next item.
    Chosen(Who),
}

/// Run the next `len` instructions once per item of the domain, then go on
/// after them. Afterwards [`Cond::Did`] and
/// [`Expr::Done`] read the whole block: at least one pass ran and every
/// operation in every pass fully happened, and how many operations did.
/// A block stays inside its effect invocation: it is no new effect, action
/// or karma scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub len: u8,
    pub domain: Domain,
    pub order: Order,
    /// Each pass's item: a value, a color or a card.
    pub item: Option<Slot>,
    /// A reduction of a chosen order: only
    /// [`Shortcut::AnyOrder`], for a frozen domain whose body only splays.
    pub shortcut: Option<Shortcut>,
}

impl Block {
    /// How deep blocks may nest.
    pub const DEPTH: usize = 2;
}

/// Effects whose mechanics are unique to one card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    Fusion,
    Reclamation,
    SpaceTraffic,
    /// Max Verstappen's karma: execute the triggering effect once more for
    /// each card the previous instruction returned.
    MaxVerstappen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
    Move(Move),
    Draw(Draw),
    /// Reveal one selected card, or every one.
    Reveal {
        from: Selector,
        optional: bool,
        all: bool,
        /// Skip a proof packet only when this condition is already known true
        /// to both players. Otherwise reveal the packet before testing it.
        unless_proven: Option<&'static Cond>,
        /// Record the card revealed (one card only).
        keep: Option<Keep>,
        /// The player who performs and chooses the reveal when it is not the
        /// executor: a demander showing a witness from their own hand.
        by: Option<Who>,
    },
    Splay(SplayStep),
    Exchange(ExchangeStep),
    Choose {
        who: Who,
        choice: Choice,
        optional: bool,
        /// Record what was chosen: a value, colors, an icon or a card.
        keep: Option<Keep>,
        /// Options the author proves lead to the same place.
        alias: Option<Alias>,
    },
    /// Retain a card or set a slot without an operation: the previous
    /// result stays readable.
    Bind(Binding),
    /// That player executes a retained card wherever it is: self, super, or
    /// one non-demand dogma effect by index, with scoped modifiers.
    Execute(Execute),
    /// Skip the next `n` instructions unless the condition holds.
    If(&'static Cond, u8),
    Jump(u8),
    /// Restart this effect with empty registers and the same scope. Effect
    /// slots empty too, except those in the `keep` mask ([`Slot::bit`]).
    Repeat {
        keep: u8,
    },
    Block(Block),
    Special(SpecialStep),
    Win(WinRule),
    /// Finish the enclosing Dogma action, keeping normal action end.
    EndAction,
    /// Flip a fair coin (Quantum Computers): one RNG word, low bit set wins.
    /// A won flip is a performed result for `Did`; a lost one is not.
    Flip,
    /// Martian Internet: under the draw variant the game ends in a draw; under the
    /// printed rule a subgame begins.
    Subgame,
    Native(Native),
}

/// When a Figure's karma clause reacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    /// Before the operation, which then continues.
    First,
    /// In place of the operation.
    Instead,
    /// "When you meld this card", after the meld.
    Melded,
}

/// The game event a Would clause watches. Its program starts with the
/// event's card bound ([`Reg::Bound`]): the moved, activated, executed or
/// claimed card, or the top card a draw would take (only its back is
/// public). A Dogma's featured icon (after Auspice) or a splay's color is the
/// chosen value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// Moving a card with one of these verbs.
    Move(&'static [Verb]),
    Draw,
    /// Dogma a card (take a Dogma action), using this featured icon if
    /// one is printed.
    Dogma(Option<Icon>),
    /// The draw of a Draw action, regular or free (sharing's included).
    DrawAction,
    /// The meld of a Meld action.
    MeldAction,
    /// Executing a card's effect: the demander for a demand, otherwise the
    /// player performing its instructions (including a sharing player).
    Effect,
    /// Executing a demand effect against the owner; the actor is the
    /// demanding player.
    DemandAgainst,
    IssueDecree,
    /// Claiming an achievement: a standard card, a special or a Decree.
    Claim,
    /// A splay of a color on the owner's own board; `None` is any direction
    /// but unsplaying.
    Splay(Option<Splay>),
    Exchange,
    /// A player would win.
    Win,
    /// The owner is required to fade a Figure.
    Fade,
    /// The Figure itself is melded (only with [`Reaction::Melded`]).
    ThisMelded,
    /// The actor would not be eligible to share the Dogma action.
    Ineligible,
    /// The Dogma player would draw for sharing (the share bonus); the
    /// activated card is bound and its featured icon chosen.
    ShareDraw,
    /// The Dogma player would not draw for sharing after the action.
    NoShareDraw,
    /// Executing a dogma effect, as [`On::Effect`] but never an echo effect.
    DogmaEffect,
}

impl On {
    /// The trigger's kind as one bit, ignoring its payload.
    pub(crate) const fn bit(self) -> u32 {
        1 << match self {
            Self::Move(_) => 0,
            Self::Draw => 1,
            Self::Dogma(_) => 2,
            Self::DrawAction => 3,
            Self::MeldAction => 4,
            Self::Effect => 5,
            Self::DemandAgainst => 6,
            Self::IssueDecree => 7,
            Self::Claim => 8,
            Self::Splay(_) => 9,
            Self::Exchange => 10,
            Self::Win => 11,
            Self::Fade => 12,
            Self::ThisMelded => 13,
            Self::Ineligible => 14,
            Self::ShareDraw => 15,
            Self::NoShareDraw => 16,
            Self::DogmaEffect => 17,
        }
    }
}

/// What a karma clause does: a program, or one of a few unique mechanics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    Program(&'static [Instruction]),
    Native(KarmaNative),
}

/// Karma mechanics without a sensible program form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KarmaNative {
    /// Raise the executing effect's boxed values by one.
    Archimedes,
    /// Re-run the Dogma action with raised boxed values, owning its end.
    Kepler,
    /// Score or self-execute an opponent's top red card.
    Kalashnikov,
    /// Return the melder's top cards of a color, up to six, else score.
    EmmaWatson,
    /// Junk everything in play, introduce Age 0, draw a [0].
    BearGrylls,
    /// Tuck the transferred or exchanged cards, then draw, tuck and score.
    YiSunSin,
    /// Name a Figure and search a Figures deck for it.
    AlexTrebek,
    /// Execute the demand effect against its demander instead.
    Hammurabi,
    /// Junk the [6] deck, draw and reveal one [7], two [8] or three [9],
    /// returning them if they outnumber the board's [Efficiency].
    Huygens,
    /// Reveal five deck tops; five colors win, else draw one and splay
    /// aslant.
    LilyHevesh,
}

/// One karma clause: a trigger, a reaction and its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Karma {
    pub reaction: Reaction,
    pub on: On,
    /// Whose operation: the owner ([`Who::Executor`]), an opponent, or
    /// either. In the clause, [`Who::Demander`] names that player.
    pub actor: Who,
    /// Further trigger conditions, evaluated with the event's registers.
    pub when: Option<&'static Cond>,
    /// Who performs the body: the owner, or the triggering player.
    pub by: Who,
    pub body: Body,
}

/// Registers that may hold a karma's triggering operand at some point.
#[derive(Clone, Copy)]
struct Operand(u8);

impl Operand {
    const BOUND: Self = Self(Self::bit(Reg::Bound));

    /// Karma programs read no slots, so a slot never holds the operand.
    const fn bit(reg: Reg) -> u8 {
        match reg {
            Reg::Last => 1,
            Reg::Bound => 2,
            Reg::Source => 4,
            Reg::Chosen => 8,
            Reg::Slot(_) => 0,
        }
    }

    fn has(self, reg: Reg) -> bool {
        self.0 & Self::bit(reg) != 0
    }

    fn with(self, reg: Reg) -> Self {
        Self(self.0 | Self::bit(reg))
    }
}

/// Static checks for whether a clause can inspect its operand's front (color,
/// icons, name or effects): through a register that may hold the operand,
/// or by filtering candidates that include such a register by a front test.
/// Value, set and identity are not fronts, because a back shows them and a
/// determinized sample keeps every card's back. Front-reading filters over a
/// player's own candidates (cards with a crown in your hand) read those
/// candidates, not the operand. Executing a register card reads it.
impl Test {
    fn reads_operand_front(self, operand: Operand) -> bool {
        match self {
            Self::ColorOf(reg) | Self::FeaturedAs(reg) | Self::SharesIcon(reg) => operand.has(reg),
            Self::ColorIn(colors) => colors.reads_operand_front(operand),
            Self::Value(_, expr) => expr.reads_operand_front(operand),
            Self::ValueIn(source) | Self::SharesIconWith(source) => {
                source.reads_operand_front(operand)
            }
            _ => false,
        }
    }
}

impl Filter {
    fn test(self) -> Option<Test> {
        match self {
            Self::Has(test) | Self::Lacks(test) => Some(test),
            Self::Lowest | Self::Highest => None,
        }
    }

    /// Whether filtering the operand itself reads its front.
    fn reads_front_of_operand(self, operand: Operand) -> bool {
        self.test()
            .is_some_and(|test| test.reads_front() || test.reads_operand_front(operand))
    }

    fn reads_operand_front(self, operand: Operand) -> bool {
        self.test()
            .is_some_and(|test| test.reads_operand_front(operand))
    }
}

impl ColorSet {
    fn reads_operand_front(self, operand: Operand) -> bool {
        match self {
            Self::Of(reg) => operand.has(reg),
            Self::OfCards(selector) => {
                selector.includes_operand(operand) || selector.reads_operand_front(operand)
            }
            Self::Only(_)
            | Self::Chosen
            | Self::Slot(_)
            | Self::MostCards(_)
            | Self::MostVisible(_) => false,
        }
    }
}

impl Expr {
    fn reads_operand_front(self, operand: Operand) -> bool {
        match self {
            Self::IconsOn(reg, _) => operand.has(reg),
            // Board piles are public; only the operand's color is a front.
            Self::Visible(query) => query.colors.reads_operand_front(operand),
            Self::Count(selector)
            | Self::Max(selector)
            | Self::Min(selector)
            | Self::DistinctColors(selector)
            | Self::Distinct(selector)
            | Self::Total(selector)
            | Self::IconsAmong(selector, _)
            | Self::IconTypesAmong(selector) => selector.reads_operand_front(operand),
            Self::Add(expr, _) | Self::Mul(expr, _) | Self::Div(expr, _) | Self::NonZero(expr) => {
                expr.reads_operand_front(operand)
            }
            Self::Sum(exprs) => exprs.iter().any(|expr| expr.reads_operand_front(operand)),
            Self::BonusOf(reg) => operand.has(reg),
            Self::Bonuses(bonuses, _) => bonuses.reads_operand_front(operand),
            Self::Const(_)
            | Self::Boxed(_)
            | Self::Value(_)
            | Self::Chosen
            | Self::Slot(_)
            | Self::Done
            | Self::DoneValues
            | Self::DemandMoved(_)
            | Self::Metric(..)
            | Self::MostVisibleInAColor(_)
            | Self::ThisTurn(_) => false,
        }
    }
}

impl Bonuses {
    /// Board piles are public; only a value to match may read a front.
    fn reads_operand_front(self, operand: Operand) -> bool {
        match self.only {
            BonusValues::Equal(expr) => expr.reads_operand_front(operand),
            _ => false,
        }
    }
}

impl Selector {
    pub(crate) fn selects_tops(self) -> bool {
        match self {
            Self::Tops(_) | Self::Board(_, _, BoardRegion::Top) | Self::TopOf(_, _) => true,
            Self::Filtered(source, _) => source.selects_tops(),
            Self::Union(sources) => sources.iter().any(|source| source.selects_tops()),
            _ => false,
        }
    }

    pub(crate) fn selects_deck_top(self) -> bool {
        match self {
            Self::BaseDeckTop(_) | Self::DeckTops => true,
            Self::Filtered(source, _) => source.selects_deck_top(),
            _ => false,
        }
    }

    /// Positive eligibility filters also constrain the eventual claim. Other
    /// filters only select its candidates; an eligible board card need not be
    /// part of the available-achievement supply.
    pub(crate) fn claim_score_factor(self) -> Option<u8> {
        match self {
            Self::Filtered(source, filters) => filters
                .iter()
                .find_map(|filter| {
                    if let Filter::Has(Test::Eligible { score_factor }) = filter {
                        Some(*score_factor)
                    } else {
                        None
                    }
                })
                .or_else(|| source.claim_score_factor()),
            _ => None,
        }
    }

    /// A positive identity intersection may select the retained operand even
    /// when its base is a whole zone. This is conservative front-dependency
    /// metadata, not permission to read any actual-card-ID-equal reference.
    fn includes_operand(self, operand: Operand) -> bool {
        match self {
            Self::Reg(reg) => operand.has(reg),
            Self::Union(selectors) => selectors.iter().any(|s| s.includes_operand(operand)),
            Self::Filtered(selector, filters) => {
                selector.includes_operand(operand)
                    || filters.iter().any(
                        |filter| matches!(filter, Filter::Has(Test::Is(reg)) if operand.has(*reg)),
                    )
            }
            _ => false,
        }
    }

    fn reads_operand_front(self, operand: Operand) -> bool {
        match self {
            Self::Filtered(selector, filters) => {
                selector.reads_operand_front(operand)
                    || filters.iter().any(|f| f.reads_operand_front(operand))
                    || self.includes_operand(operand)
                        && filters.iter().any(|f| f.reads_front_of_operand(operand))
            }
            Self::Board(_, colors, _) => colors.reads_operand_front(operand),
            Self::BaseDeck(expr) | Self::SetDeck(_, expr) | Self::BaseDeckTop(expr) => {
                expr.reads_operand_front(operand)
            }
            Self::Union(selectors) => selectors.iter().any(|s| s.reads_operand_front(operand)),
            Self::Hand(_)
            | Self::Score(_)
            | Self::Achievements(_)
            | Self::Tops(_)
            | Self::TopOf(..)
            | Self::AvailableAchievements
            | Self::StandardAchievements
            | Self::Junk
            | Self::DeckTops
            | Self::InPlay
            | Self::Owned(_)
            | Self::Reg(_)
            | Self::Slot(_)
            | Self::Forecast(_)
            | Self::Safe(_)
            | Self::Museum(_) => false,
        }
    }
}

impl Cond {
    /// Whether the condition reads only the operand's own color and the
    /// colors of a slot's records: a witness test (Barometer's first blue,
    /// Globe's first of each color) whose outcome for an unrevealed operand
    /// says nothing beyond that operand.
    pub(crate) fn witness_only(&self) -> bool {
        fn color_test(filter: &Filter, slot_ok: bool) -> bool {
            match filter {
                Filter::Has(test) | Filter::Lacks(test) => match test {
                    Test::Color(_) | Test::ColorIn(ColorSet::Only(_)) => true,
                    Test::ColorOf(Reg::Bound) => slot_ok,
                    _ => false,
                },
                _ => false,
            }
        }
        fn slot_count(expr: &Expr) -> bool {
            match expr {
                Expr::Const(_) => true,
                Expr::Count(Selector::Slot(_)) => true,
                Expr::Count(Selector::Filtered(Selector::Slot(_), filters)) => {
                    filters.iter().all(|filter| color_test(filter, true))
                }
                _ => false,
            }
        }
        match self {
            Self::Compare(left, _, right) => slot_count(left) && slot_count(right),
            Self::All(conds) | Self::Any(conds) => conds.iter().all(Self::witness_only),
            Self::Not(cond) => cond.witness_only(),
            Self::Card(Reg::Bound, filter) => color_test(filter, false),
            _ => false,
        }
    }

    /// Whether a karma condition, evaluated with its operand bound, reads the
    /// operand's front.
    pub(crate) fn reads_card_front(&self) -> bool {
        self.reads_operand_front(Operand::BOUND)
    }

    fn reads_operand_front(&self, operand: Operand) -> bool {
        match *self {
            Self::Compare(left, _, right) | Self::SameDraw(left, right) => {
                left.reads_operand_front(operand) || right.reads_operand_front(operand)
            }
            Self::CommonIcon { from, .. } => {
                from.includes_operand(operand) || from.reads_operand_front(operand)
            }
            Self::EqualValues(left, right) => {
                left.reads_operand_front(operand) || right.reads_operand_front(operand)
            }
            Self::All(conds) | Self::Any(conds) => {
                conds.iter().any(|cond| cond.reads_operand_front(operand))
            }
            Self::Not(cond) => cond.reads_operand_front(operand),
            Self::Card(reg, filter) => {
                operand.has(reg) && filter.reads_front_of_operand(operand)
                    || filter.reads_operand_front(operand)
            }
            Self::Splayed { colors, .. } => colors.reads_operand_front(operand),
            Self::FinalCount { hand, drawn } => {
                hand.reads_operand_front(operand) || drawn.reads_operand_front(operand)
            }
            Self::Empty(_)
            | Self::Did(_)
            | Self::DemandMoved(_)
            | Self::DrawnColor(_)
            | Self::DrawnColorTwice
            | Self::ReturnedMost
            | Self::DancingGirlVisited
            | Self::Action(_)
            | Self::Foreseen
            | Self::OwnTurn
            | Self::Must => false,
        }
    }
}

impl Instruction {
    /// The retained interrupted card is read by this reached instruction.
    pub(crate) fn reads_bound_front(&self) -> bool {
        self.reads_operand_front(Operand::BOUND)
    }

    /// Running this instruction may write an execution or action record of
    /// this lifetime: it records into or binds such a slot, or starts another
    /// program (an execution, or a native effect) that might.
    pub(crate) fn may_record(&self, life: Life) -> bool {
        match *self {
            Self::Move(Move { keep, .. })
            | Self::Draw(Draw { keep, .. })
            | Self::Reveal { keep, .. }
            | Self::Choose { keep, .. } => keep.is_some_and(|keep| keep.slot.life() == life),
            Self::Bind(Binding::Value(slot, _) | Binding::Colors(slot, _)) => slot.life() == life,
            Self::Execute(..) | Self::Native(_) => true,
            Self::Bind(Binding::Card(_))
            | Self::Splay(_)
            | Self::Exchange(_)
            | Self::If(..)
            | Self::Jump(_)
            | Self::Repeat { .. }
            | Self::Block(_)
            | Self::Special(_)
            | Self::Win(_)
            | Self::Flip
            | Self::Subgame
            | Self::EndAction => false,
        }
    }

    fn reads_operand_front(&self, operand: Operand) -> bool {
        match *self {
            Self::Move(Move {
                from,
                amount,
                when,
                reveal_if_empty,
                ..
            }) => {
                let predicate_operand = if from.includes_operand(operand) {
                    operand.with(Reg::Bound)
                } else {
                    Operand(operand.0 & !Operand::bit(Reg::Bound))
                };
                from.reads_operand_front(operand)
                    || reveal_if_empty.is_some_and(|proof| proof.reads_operand_front(operand))
                    || when
                        .is_some_and(|condition| condition.reads_operand_front(predicate_operand))
                    || match amount {
                        Amount::PerValue(quota) => match quota.source {
                            ValueSource::Bonuses(bonuses) => bonuses.reads_operand_front(operand),
                            ValueSource::Cards(source) => source.reads_operand_front(operand),
                        },
                        Amount::Batch { limit, .. } => {
                            limit.is_some_and(|limit| limit.reads_operand_front(operand))
                        }
                        Amount::One { .. } | Amount::Group { .. } => false,
                    }
            }
            Self::Bind(Binding::Card(from)) => from.reads_operand_front(operand),
            Self::Bind(Binding::Value(_, expr)) => expr.reads_operand_front(operand),
            Self::Bind(Binding::Colors(_, colors)) => colors.reads_operand_front(operand),
            Self::Draw(Draw { age, count, .. }) => {
                age.reads_operand_front(operand)
                    || count.is_some_and(|count| count.reads_operand_front(operand))
            }
            Self::Reveal {
                from,
                unless_proven,
                ..
            } => {
                from.reads_operand_front(operand)
                    || unless_proven.is_some_and(|condition| condition.reads_operand_front(operand))
            }
            Self::Splay(step) => step.choices.reads_operand_front(operand),
            Self::Exchange(step) => {
                step.left.reads_operand_front(operand)
                    || step.right.reads_operand_front(operand)
                    || step
                        .counts
                        .iter()
                        .flatten()
                        .any(|expr| expr.reads_operand_front(operand))
            }
            Self::Choose { choice, .. } => match choice {
                Choice::Value(ValueChoices::Among(selector) | ValueChoices::NotAmong(selector))
                | Choice::Card(selector)
                | Choice::Cards { from: selector, .. } => selector.reads_operand_front(operand),
                Choice::Color(colors) | Choice::FinalCount { colors, .. } => {
                    colors.reads_operand_front(operand)
                }
                Choice::Value(ValueChoices::Bonuses(bonuses)) => {
                    bonuses.reads_operand_front(operand)
                }
                Choice::ColorsAmong(_, colors) => colors.reads_operand_front(operand),
                Choice::Value(_)
                | Choice::Icon { .. }
                | Choice::MostIcons(_)
                | Choice::IconTypes { .. }
                | Choice::Colors(_)
                | Choice::Branch(_)
                | Choice::SetBranch { .. } => false,
            },
            Self::If(cond, _) => cond.reads_operand_front(operand),
            Self::Block(block) => match block.domain {
                Domain::Times(expr) => expr.reads_operand_front(operand),
                Domain::Values(from, _) | Domain::Cards(from) | Domain::Live { from, .. } => {
                    from.reads_operand_front(operand) || from.includes_operand(operand)
                }
                Domain::Colors(colors) => colors.reads_operand_front(operand),
                Domain::Bonuses(bonuses, _) => bonuses.reads_operand_front(operand),
                Domain::Once | Domain::Range(..) => false,
            },
            Self::Special(SpecialStep::Decree(colors)) => colors.reads_operand_front(operand),
            // Executing the retained card reads it; a native effect may read anything.
            Self::Execute(execute) => operand.has(execute.target),
            Self::Native(_) => true,
            Self::Jump(_)
            | Self::Repeat { .. }
            | Self::Special(_)
            | Self::Win(_)
            | Self::Flip
            | Self::Subgame
            | Self::EndAction => false,
        }
    }

    /// Registers that may hold the operand after this instruction: its
    /// result or choice may be the operand only if it selects from it.
    fn carry(&self, operand: Operand) -> Operand {
        match *self {
            Self::Move(Move { from, .. }) | Self::Reveal { from, .. }
                if from.includes_operand(operand) =>
            {
                operand.with(Reg::Last)
            }
            Self::Choose {
                choice: Choice::Card(selector),
                ..
            } if selector.includes_operand(operand) => operand.with(Reg::Chosen),
            _ => operand,
        }
    }
}

impl Karma {
    /// Whether an entered body may already have read its operand's front.
    /// Registers carry the operand cumulatively, so jumps stay conservative.
    /// Among native bodies only Emma Watson reads the interrupted card (its
    /// color); Yi Sun-sin tucks the cards publicly before anything reads them.
    pub(crate) fn body_reads_card_front(&self) -> bool {
        match self.body {
            Body::Program(instructions) => {
                let mut operand = Operand::BOUND;
                instructions.iter().any(|instruction| {
                    let reads = instruction.reads_operand_front(operand);
                    operand = instruction.carry(operand);
                    reads
                })
            }
            Body::Native(native) => native == KarmaNative::EmmaWatson,
        }
    }
}

/// Continuous "Each" modifiers, by the queries they change. The
/// computations live with those queries; the category drives shortcut guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Each {
    Icons,
    Score,
    Achievements,
    /// Achievement claim cost or eligibility.
    Claims,
    /// Cards that count as available achievements.
    AvailableAchievements,
    /// Cards that can be activated with a Dogma action.
    DogmaSources,
    HandMembership,
    ScoreMembership,
    SpecialRequirements,
}

/// A Figure's complete printed behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FigureDef {
    pub code: &'static str,
    /// Decree permissions: a Decree (`None`: any) with this many Figures.
    pub decrees: &'static [(Option<crate::Decree>, u8)],
    pub each: &'static [Each],
    pub karma: &'static [Karma],
}

/// Authoring helpers. Programs are `const` slices built from these.
pub mod build {
    pub use super::{
        Alias, Amount, Binding, Block, BoardRegion, Body, BonusValues, Bonuses, Choice, Cmp,
        ColorSet, Cond, Destination, Domain, Draw, Each, ExchangeStep, Expr, FigureDef, Filter,
        GroupConstraint, Instruction, Karma, KarmaNative, Keep, Life, Metric, Move, Multiplicity,
        Native, On, Order, Outcome, PerValue, Reaction, Reg, Seen, Selector, Shortcut, Shown, Slot,
        SpecialStep, SplayChoices, SplayDirection, SplayOption, SplayStep, SplayTargets, Stop,
        Tally, Test, Then, ValueChoices, ValueSource, Visibility, Who, WinRule,
    };
    pub use crate::model::{CardSet, Color, Icon, Splay, Verb};
    pub use crate::{ExecutionMode, Special};

    pub const HAND: Selector = Selector::Hand(Who::Executor);
    pub const SCORE: Selector = Selector::Score(Who::Executor);
    pub const TOPS: Selector = Selector::Tops(Who::Executor);
    pub const LAST: Selector = Selector::Filtered(&HAND, &[Filter::Has(Test::Is(Reg::Last))]);
    pub const BOUND: Selector = Selector::Filtered(&HAND, &[Filter::Has(Test::Is(Reg::Bound))]);
    pub const LAST_VALUE: Expr = Expr::Value(Reg::Last);

    pub const DID: &Cond = &Cond::Did(Outcome::All);
    /// Restart the effect with every slot empty.
    pub const REPEAT: Instruction = Instruction::Repeat { keep: 0 };

    /// Restart the effect, keeping these effect slots.
    pub const fn repeat_keeping(slot: Slot) -> Instruction {
        Instruction::Repeat { keep: slot.bit() }
    }
    /// Retain a card in the bound register.
    pub const fn bind(from: Selector) -> Instruction {
        Instruction::Bind(Binding::Card(from))
    }
    pub const DID_ANY: &Cond = &Cond::Did(Outcome::Any);

    pub const fn filtered(source: &'static Selector, filters: &'static [Filter]) -> Selector {
        Selector::Filtered(source, filters)
    }

    const fn to_for(verb: Verb) -> Destination {
        match verb {
            Verb::Meld | Verb::Tuck => Destination::Board(Who::Executor),
            Verb::Score => Destination::Score(Who::Executor),
            Verb::Return | Verb::Place => Destination::Deck,
            Verb::Junk => Destination::Junk,
            Verb::Transfer => Destination::Hand(Who::Executor),
            Verb::Achieve => Destination::Achievements(Who::Executor),
            Verb::Foreshadow => Destination::Forecast(Who::Executor),
            Verb::Safeguard => Destination::Safe(Who::Executor),
            // Rules procedures, never printed verbs; their destinations are
            // decided by the procedure.
            Verb::Dig | Verb::Rotate | Verb::Seize => Destination::Hand(Who::Executor),
        }
    }

    /// Move one card with the verb's usual destination.
    pub const fn verb(verb: Verb, from: Selector) -> Instruction {
        Instruction::Move(Move {
            verb,
            from,
            to: to_for(verb),
            amount: Amount::One { optional: false },
            shortcut: None,
            verify: false,
            when: None,
            reveal_if_empty: None,
            keep: None,
            by: None,
        })
    }
    pub const fn meld(from: Selector) -> Instruction {
        verb(Verb::Meld, from)
    }
    pub const fn score(from: Selector) -> Instruction {
        verb(Verb::Score, from)
    }
    pub const fn tuck(from: Selector) -> Instruction {
        verb(Verb::Tuck, from)
    }
    pub const fn return_(from: Selector) -> Instruction {
        verb(Verb::Return, from)
    }
    pub const fn junk(from: Selector) -> Instruction {
        verb(Verb::Junk, from)
    }
    pub const fn achieve(from: Selector) -> Instruction {
        verb(Verb::Achieve, from)
    }
    pub const fn foreshadow(from: Selector) -> Instruction {
        verb(Verb::Foreshadow, from)
    }
    pub const fn safeguard(from: Selector) -> Instruction {
        verb(Verb::Safeguard, from)
    }
    pub const fn transfer(from: Selector, to: Destination) -> Instruction {
        verb(Verb::Transfer, from).to(to)
    }
    /// That player executes the bound card: self, super, or one non-demand
    /// dogma effect. Modifiers are added with [`Instruction::must`],
    /// [`Instruction::controlled_by`], [`Instruction::inspecting`] and
    /// [`Instruction::ignore_reveals`]; [`Instruction::executing`] retargets.
    pub const fn execute(mode: ExecutionMode, who: Who) -> Instruction {
        Instruction::Execute(super::Execute {
            target: Reg::Bound,
            mode,
            who,
            must: false,
            controller: None,
            inspect: false,
            count_reveals: true,
        })
    }

    pub const fn draw(age: u8) -> Instruction {
        draw_age(Expr::Boxed(age))
    }
    pub const fn draw_age(age: Expr) -> Instruction {
        Instruction::Draw(Draw {
            age,
            set: CardSet::Base,
            any_set: false,
            count: None,
            then: Then::Keep,
            optional: false,
            keep: None,
        })
    }
    /// Draw `count` boxed `age`s, finishing each before the next.
    pub const fn draws(count: Expr, age: u8, then: Then) -> Instruction {
        draws_age(count, Expr::Boxed(age), then)
    }
    /// Draw `count` cards of a computed age, finishing each before the next.
    pub const fn draws_age(count: Expr, age: Expr, then: Then) -> Instruction {
        Instruction::Draw(Draw {
            age,
            set: CardSet::Base,
            any_set: false,
            count: Some(count),
            then,
            optional: false,
            keep: None,
        })
    }

    pub const fn reveal(from: Selector) -> Instruction {
        Instruction::Reveal {
            from,
            optional: false,
            all: false,
            unless_proven: None,
            keep: None,
            by: None,
        }
    }
    /// Reveal every selected card ("reveal your hand").
    pub const fn reveal_all(from: Selector) -> Instruction {
        Instruction::Reveal {
            from,
            optional: false,
            all: true,
            unless_proven: None,
            keep: None,
            by: None,
        }
    }

    pub const fn splay(colors: &'static [Color], direction: Splay) -> Instruction {
        Instruction::Splay(SplayStep {
            who: Who::Executor,
            choices: SplayChoices::Fixed {
                colors: ColorSet::Only(colors),
                direction,
                from: None,
            },
            targets: SplayTargets::Changes,
            optional: true,
            all: false,
        })
    }
    /// A mandatory splay of one color of the set. Any nonempty pile is a target: a
    /// choice that changes nothing is allowed unless the card says otherwise, so a
    /// sharer can deny the share draw. An optional splay's decline is that no-op.
    pub const fn splay_set(colors: ColorSet, direction: Splay) -> Instruction {
        Instruction::Splay(SplayStep {
            who: Who::Executor,
            choices: SplayChoices::Fixed {
                colors,
                direction,
                from: None,
            },
            targets: SplayTargets::Nonempty,
            optional: false,
            all: false,
        })
    }

    pub const fn choose(who: Who, choice: Choice) -> Instruction {
        Instruction::Choose {
            who,
            choice,
            optional: false,
            keep: None,
            alias: None,
        }
    }

    pub const fn if_(cond: &'static Cond, skip: u8) -> Instruction {
        Instruction::If(cond, skip)
    }

    /// Skip the next two instructions, "choose [a] or [b]" and junking that
    /// Base deck, when both decks are empty: that choice changes nothing. It
    /// reads the boxed values the choice offers (Archimedes, Kepler).
    macro_rules! unless_decks_empty {
        ($a:literal, $b:literal) => {
            $crate::program::build::if_(
                &$crate::program::Cond::Any(&[
                    $crate::program::Cond::Compare(
                        $crate::program::Expr::Count(&$crate::program::Selector::BaseDeck(
                            $crate::program::Expr::Boxed($a),
                        )),
                        $crate::program::Cmp::Greater,
                        $crate::program::Expr::Const(0),
                    ),
                    $crate::program::Cond::Compare(
                        $crate::program::Expr::Count(&$crate::program::Selector::BaseDeck(
                            $crate::program::Expr::Boxed($b),
                        )),
                        $crate::program::Cmp::Greater,
                        $crate::program::Expr::Const(0),
                    ),
                ]),
                2,
            )
        };
    }
    pub(crate) use unless_decks_empty;

    /// Run the next `len` instructions as one sentence.
    pub const fn once(len: u8) -> Instruction {
        for_each(Domain::Once, Order::Printed, None, len)
    }
    /// Run the next `len` instructions once per item of the domain.
    pub const fn for_each(
        domain: Domain,
        order: Order,
        item: Option<Slot>,
        len: u8,
    ) -> Instruction {
        Instruction::Block(Block {
            len,
            domain,
            order,
            item,
            shortcut: None,
        })
    }

    pub const fn claim(special: Special) -> Instruction {
        Instruction::Special(SpecialStep::Claim(special))
    }

    /// A Would clause reacting before the event, performed by the owner.
    pub const fn first(on: On, actor: Who, body: &'static [Instruction]) -> Karma {
        Karma {
            reaction: Reaction::First,
            on,
            actor,
            when: None,
            by: Who::Executor,
            body: Body::Program(body),
        }
    }
    /// A Would clause replacing the event, performed by the owner.
    pub const fn instead(on: On, actor: Who, body: &'static [Instruction]) -> Karma {
        Karma {
            reaction: Reaction::Instead,
            ..first(on, actor, body)
        }
    }
    /// "When you meld this card".
    pub const fn melded(body: &'static [Instruction]) -> Karma {
        Karma {
            reaction: Reaction::Melded,
            ..first(On::ThisMelded, Who::Executor, body)
        }
    }

    impl Karma {
        pub const fn when(mut self, cond: &'static Cond) -> Self {
            self.when = Some(cond);
            self
        }
        /// The triggering player performs the body.
        pub const fn by_actor(mut self) -> Self {
            self.by = Who::Demander;
            self
        }
        pub const fn native(mut self, native: KarmaNative) -> Self {
            self.body = Body::Native(native);
            self
        }
    }

    impl Instruction {
        /// Reveal the complete proof unless public witnesses already prove
        /// Execute another retained card than the bound one.
        pub const fn executing(mut self, target: Reg) -> Self {
            match &mut self {
                Self::Execute(execute) => execute.target = target,
                _ => panic!("not an execution"),
            }
            self
        }
        /// Blackmail: "may" becomes "must" throughout the execution.
        pub const fn must(mut self) -> Self {
            match &mut self {
                Self::Execute(execute) => execute.must = true,
                _ => panic!("not an execution"),
            }
            self
        }
        /// Hitchhiking: that player makes every decision in the execution.
        pub const fn controlled_by(mut self, who: Who) -> Self {
            match &mut self {
                Self::Execute(execute) => execute.controller = Some(who),
                _ => panic!("not an execution"),
            }
            self
        }
        /// The controller may look at what the executor may.
        pub const fn inspecting(mut self) -> Self {
            match &mut self {
                Self::Execute(execute) => execute.inspect = true,
                _ => panic!("not an execution"),
            }
            self
        }
        /// Reveals alone are not activity ("if you do" of Teleprompter).
        pub const fn ignore_reveals(mut self) -> Self {
            match &mut self {
                Self::Execute(execute) => execute.count_reveals = false,
                _ => panic!("not an execution"),
            }
            self
        }
        /// the condition. An optional reveal cannot establish an absence.
        pub const fn unless_proven(mut self, condition: &'static Cond) -> Self {
            match &mut self {
                Self::Reveal {
                    all: true,
                    optional: false,
                    unless_proven,
                    ..
                } => {
                    *unless_proven = Some(condition);
                }
                _ => panic!("conditional proof requires a mandatory reveal packet"),
            }
            self
        }
        /// Add "you may": an optional single move, batch, draw, reveal,
        /// splay, exchange or choice.
        pub const fn optional(self) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.amount = match m.amount {
                        Amount::One { .. } => Amount::One { optional: true },
                        // "May... up to / any number" keeps its stops.
                        Amount::Batch {
                            limit,
                            stop: Stop::AnyTime | Stop::MayAnyTime,
                            per_color,
                            frozen,
                        } => Amount::Batch {
                            limit,
                            stop: Stop::MayAnyTime,
                            per_color,
                            frozen,
                        },
                        Amount::Batch {
                            limit,
                            per_color,
                            frozen,
                            ..
                        } => Amount::Batch {
                            limit,
                            stop: Stop::Initially,
                            per_color,
                            frozen,
                        },
                        Amount::PerValue(_) => panic!("optional per-value batch"),
                        Amount::Group {
                            size, constraint, ..
                        } => Amount::Group {
                            size,
                            constraint,
                            optional: true,
                        },
                    };
                    Self::Move(m)
                }
                Self::Draw(mut d) => {
                    d.optional = true;
                    Self::Draw(d)
                }
                Self::Reveal {
                    from,
                    all,
                    keep,
                    unless_proven,
                    by,
                    ..
                } => Self::Reveal {
                    from,
                    optional: true,
                    all,
                    unless_proven,
                    keep,
                    by,
                },
                Self::Splay(mut s) => {
                    s.optional = true;
                    // The decline is the no-op: offer only splays that change.
                    if matches!(s.targets, SplayTargets::Nonempty) {
                        s.targets = SplayTargets::Changes;
                    }
                    Self::Splay(s)
                }
                Self::Exchange(mut e) => {
                    e.optional = true;
                    Self::Exchange(e)
                }
                Self::Choose {
                    who,
                    choice,
                    keep,
                    alias,
                    ..
                } => Self::Choose {
                    who,
                    choice,
                    optional: true,
                    keep,
                    alias,
                },
                _ => panic!("not optional"),
            }
        }

        const fn batch(self, limit: Option<Expr>, stop: Stop, per_color: bool) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.amount = Amount::Batch {
                        limit,
                        stop,
                        per_color,
                        frozen: false,
                    };
                    Self::Move(m)
                }
                _ => panic!("not a move"),
            }
        }

        /// Freeze an ordinary batch's full selection before the first move.
        /// Reactions cannot add members or change its original source zones.
        pub const fn frozen(self) -> Self {
            match self {
                Self::Move(mut step) => {
                    match &mut step.amount {
                        Amount::Batch { frozen, .. } => *frozen = true,
                        _ => panic!("not an ordinary batch"),
                    }
                    Self::Move(step)
                }
                _ => panic!("not a move"),
            }
        }

        /// As many cards of each value as `quota` allows, as many as
        /// possible.
        pub const fn per_value(self, quota: &'static PerValue) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.amount = Amount::PerValue(quota);
                    Self::Move(m)
                }
                _ => panic!("not a move"),
            }
        }

        /// Every qualifying card, one at a time.
        pub const fn all(self) -> Self {
            self.batch(None, Stop::Never, false)
        }
        /// Exactly `n` cards (as many as possible).
        pub const fn exactly(self, n: Expr) -> Self {
            self.batch(Some(n), Stop::Never, false)
        }
        /// Up to `n` cards (any number if `None`), stopping at will.
        pub const fn up_to(self, n: Option<Expr>) -> Self {
            self.batch(n, Stop::AnyTime, false)
        }
        /// One card of each color, `n` at most.
        pub const fn per_color(self, n: Option<Expr>) -> Self {
            self.batch(n, Stop::Never, true)
        }
        /// At most one card of each color, stopping at any time.
        pub const fn any_per_color(self) -> Self {
            self.batch(None, Stop::AnyTime, true)
        }
        pub const fn group(self, size: u8, constraint: GroupConstraint) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.amount = Amount::Group {
                        size,
                        constraint,
                        optional: false,
                    };
                    Self::Move(m)
                }
                _ => panic!("not a move"),
            }
        }
        pub const fn to(self, to: Destination) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.to = to;
                    Self::Move(m)
                }
                _ => panic!("not a move"),
            }
        }
        /// That player performs and chooses this single move or reveal.
        pub const fn by(self, who: Who) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.by = Some(who);
                    Self::Move(m)
                }
                Self::Reveal {
                    from,
                    optional,
                    all,
                    unless_proven,
                    keep,
                    ..
                } => Self::Reveal {
                    from,
                    optional,
                    all,
                    unless_proven,
                    keep,
                    by: Some(who),
                },
                _ => panic!("not a move or reveal"),
            }
        }
        pub const fn verify(self) -> Self {
            match self {
                Self::Move(mut step) => {
                    step.verify = true;
                    step.when = None;
                    Self::Move(step)
                }
                _ => panic!("verification requires a move"),
            }
        }

        /// Reveal a completeness proof only when a mandatory single-card
        /// selection has no eligible target, before reporting it inapplicable.
        pub const fn reveal_if_empty(self, proof: &'static Selector) -> Self {
            match self {
                Self::Move(mut step) => {
                    assert!(matches!(step.amount, Amount::One { optional: false }));
                    step.reveal_if_empty = Some(proof);
                    Self::Move(step)
                }
                _ => panic!("empty-selection proof requires a move"),
            }
        }

        /// Conditional verification, after admission and before First karma.
        /// The selected card temporarily shadows Bound in this condition only.
        pub const fn verify_when(self, condition: &'static Cond) -> Self {
            match self {
                Self::Move(mut step) => {
                    step.verify = true;
                    step.when = Some(condition);
                    Self::Move(step)
                }
                _ => panic!("verification requires a move"),
            }
        }

        pub const fn shortcut(self, shortcut: Shortcut) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.shortcut = Some(shortcut);
                    Self::Move(m)
                }
                Self::Block(mut b) => {
                    b.shortcut = Some(shortcut);
                    Self::Block(b)
                }
                Self::Special(SpecialStep::JunkAvailable { count, .. }) => {
                    Self::Special(SpecialStep::JunkAvailable {
                        count,
                        shortcut: Some(shortcut),
                    })
                }
                _ => panic!("not a move or block"),
            }
        }
        /// Record what this instruction actually did in the slot, replacing
        /// what it held.
        /// Mark options of a value or colour choice that lead to the same
        /// place; see [`Alias`].
        pub const fn alias(self, alias: Alias) -> Self {
            match self {
                Self::Choose {
                    who,
                    choice,
                    optional,
                    keep,
                    ..
                } => Self::Choose {
                    who,
                    choice,
                    optional,
                    keep,
                    alias: Some(alias),
                },
                _ => panic!("only a choice has aliases"),
            }
        }

        pub const fn keep(self, slot: Slot) -> Self {
            self.recording(Keep {
                slot,
                append: false,
            })
        }
        /// Append what this instruction actually did to the slot.
        pub const fn add(self, slot: Slot) -> Self {
            self.recording(Keep { slot, append: true })
        }
        const fn recording(self, record: Keep) -> Self {
            match self {
                Self::Move(mut m) => {
                    m.keep = Some(record);
                    Self::Move(m)
                }
                Self::Draw(mut d) => {
                    d.keep = Some(record);
                    Self::Draw(d)
                }
                Self::Reveal {
                    from,
                    optional,
                    all,
                    unless_proven,
                    by,
                    ..
                } => Self::Reveal {
                    from,
                    optional,
                    all,
                    unless_proven,
                    keep: Some(record),
                    by,
                },
                Self::Choose {
                    who,
                    choice,
                    optional,
                    alias,
                    ..
                } => Self::Choose {
                    who,
                    choice,
                    optional,
                    keep: Some(record),
                    alias,
                },
                _ => panic!("nothing to record"),
            }
        }

        /// A counted draw's follow-up, or a single draw's set.
        pub const fn set(self, set: CardSet) -> Self {
            match self {
                Self::Draw(mut d) => {
                    d.set = set;
                    Self::Draw(d)
                }
                _ => panic!("not a draw"),
            }
        }
        /// "From any set": the player chooses among the enabled sets.
        pub const fn any_set(self) -> Self {
            match self {
                Self::Draw(mut d) => {
                    d.any_set = true;
                    Self::Draw(d)
                }
                _ => panic!("not a draw"),
            }
        }
    }
}
