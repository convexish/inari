//! Age 0 card programs (introduced by Bear Grylls), in code order. Each
//! comment is the printed text. "You lose" is elimination.
use crate::program::build::*;

use Color::{Blue, Green, Purple, Red, Yellow};
use Cond::{Any, Card, Compare, Not};
use Expr::{Const, Count};
use Filter::{Has, Highest, Lacks};
use Icon::Food;
use Selector::{Board, Score as ScoreOf, Tops};
use Test::{Code, ColorOf, ColorOnBoard, Icon as HasIcon, Is, Value as ValueIs, Values};
use Who::{Demander, Executor, Opponent};

type Program = &'static [Instruction];

const BIND_LAST: Instruction = bind(Selector::Reg(Reg::Last));
const LOSE: Instruction = Instruction::Win(WinRule::Loses(Executor));
const DRAWN_PAIR: Selector = Selector::Union(&[BOUND, LAST]);
const BOUND_COLOR: &[Filter] = &[Has(ColorOf(Reg::Bound))];
const ORIGIN: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: true,
};

/// The executor's hand holds at least three cards of that color.
macro_rules! three {
    ($color:expr) => {
        Compare(
            Count(&filtered(&HAND, &[Has(Test::Color($color))])),
            Cmp::AtLeast,
            Const(3),
        )
    };
}

// Splay an unsplayed color on your board left. If you don't, you lose.
const FRESH_WATER: [Program; 1] = [&[
    Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors: ColorSet::Only(&Color::ALL),
            direction: Splay::Left,
            from: Some(Splay::None),
        },
        targets: SplayTargets::Changes,
        optional: false,
        all: false,
    }),
    if_(&Not(DID), 1),
    LOSE,
]];

// I DEMAND you score a card from your hand! If there are fewer cards in your
// score pile than in mine, you lose! / Exchange all cards in your hand with
// two cards in your score pile.
const SHELTER: [Program; 2] = [
    &[
        score(HAND),
        if_(
            &Compare(Count(&SCORE), Cmp::Less, Count(&ScoreOf(Demander))),
            1,
        ),
        LOSE,
    ],
    &[Instruction::Exchange(ExchangeStep {
        left: HAND,
        right: SCORE,
        counts: &[None, Some(Const(2))],
        optional: false,
    })],
];

// Score a card with a [Food] on your board. If you do, meld a card from your
// hand for each [Food] on the scored card.
const SKINNING: [Program; 1] = [&[
    score(filtered(&TOPS, &[Has(HasIcon(Food))])),
    if_(DID, 1),
    meld(HAND)
        .exactly(Expr::IconsOn(Reg::Last, Food))
        .shortcut(Shortcut::ByColor),
]];

// Draw and meld a [0]. Reveal all hands and transfer all revealed cards of the
// drawn card's color to your hand, and if you draw Fresh Water, draw a [1].
// If you have three cards of any one color in your hand, draw a [2].
const FISHING_SHOWN: Slot = Slot::effect(0);
const FISHING: [Program; 1] = [&[
    draw(0),
    BIND_LAST,
    meld(BOUND),
    reveal_all(Selector::Hand(Who::Either)).keep(FISHING_SHOWN),
    // The record names the revealed cards, so a reacting karma (Yi Sun-sin)
    // leaves their order to the executor.
    transfer(
        filtered(
            &filtered(
                &Selector::Hand(Opponent),
                &[Has(Test::InSlot(FISHING_SHOWN))],
            ),
            BOUND_COLOR,
        ),
        Destination::Hand(Executor),
    )
    .all()
    .shortcut(Shortcut::AnyOrder),
    if_(&Card(Reg::Bound, Has(Code("Z001"))), 1),
    draw(1),
    if_(
        &Any(&[
            three!(Red),
            three!(Blue),
            three!(Green),
            three!(Yellow),
            three!(Purple),
        ]),
        1,
    ),
    draw(2),
]];

// Draw a [0]. If it has no [Food], you may reveal it, score it, and draw a
// [0].
const FORAGING: [Program; 1] = [&[
    draw(0),
    BIND_LAST,
    if_(&Card(Reg::Bound, Lacks(HasIcon(Food))), 4),
    reveal(BOUND).optional(),
    if_(DID, 2),
    score(BOUND),
    draw(0),
]];

// I DEMAND you draw and reveal two [0]! Transfer the card of my choice to my
// board!
const HUNTING_DRAWN: Slot = Slot::effect(0);
const HUNTING_PAIR: Selector = filtered(&HAND, &[Has(Test::InSlot(HUNTING_DRAWN))]);
const HUNTING: [Program; 1] = [&[
    draw(0),
    reveal(LAST).keep(HUNTING_DRAWN),
    draw(0),
    reveal(LAST).add(HUNTING_DRAWN),
    choose(Demander, Choice::Card(&HUNTING_PAIR)),
    transfer(
        filtered(&HAND, &[Has(Is(Reg::Chosen))]),
        Destination::Board(Demander),
    ),
]];

// I DEMAND you reveal a card in your hand of color not on my board or of value
// greater than 0! If you don't, you lose! / Meld a card from your hand of
// color on your board. If you don't, you lose.
const FIRE: [Program; 2] = [
    &[
        reveal(Selector::Union(&[
            filtered(&HAND, &[Lacks(ColorOnBoard(Demander))]),
            filtered(&HAND, &[Has(ValueIs(Cmp::Greater, Const(0)))]),
        ])),
        if_(&Not(DID), 1),
        LOSE,
    ],
    &[
        meld(filtered(&HAND, &[Has(ColorOnBoard(Executor))])).reveal_if_empty(&HAND),
        if_(&Not(DID), 1),
        LOSE,
    ],
];

// I DEMAND you draw, reveal, and return a [0]! Transfer your top card of the
// color of the revealed card to my board! / If Skinning is a top card on your
// board, draw a [1].
const STONE_KNIVES: [Program; 2] = [
    &[
        draw(0),
        BIND_LAST,
        reveal(LAST),
        return_(BOUND),
        transfer(filtered(&TOPS, BOUND_COLOR), Destination::Board(Demander)),
    ],
    &[
        if_(
            &Compare(
                Count(&filtered(&TOPS, &[Has(Code("Z003"))])),
                Cmp::AtLeast,
                Const(1),
            ),
            1,
        ),
        draw(1),
    ],
];

// I DEMAND you reveal your hand and all cards in my score pile! Transfer all
// cards in your hand of the colors of the cards in my score pile to my hand!
const TRAPPING: [Program; 1] = [&[
    reveal_all(Selector::Union(&[HAND, ScoreOf(Demander)])),
    transfer(
        filtered(
            &HAND,
            &[Has(Test::ColorIn(ColorSet::OfCards(&ScoreOf(Demander))))],
        ),
        Destination::Hand(Demander),
    )
    .all()
    .shortcut(ORIGIN),
]];

// If you have five top cards of value 0 on your board, you win.
const SPEAK_WITH_BIRD: [Program; 1] = [&[
    if_(
        &Compare(
            Count(&filtered(&TOPS, &[Has(Values(&[0]))])),
            Cmp::Equal,
            Const(5),
        ),
        1,
    ),
    Instruction::Win(WinRule::Executor),
]];

// Score two cards from your hand. / Transfer a card from your score pile to
// another player's board. If you do, meld that player's bottom card of the
// transferred card's color.
const TANNING: [Program; 2] = [
    &[score(HAND).exactly(Const(2)).shortcut(ORIGIN)],
    &[
        transfer(SCORE, Destination::Board(Opponent)),
        if_(DID, 1),
        meld(Board(
            Opponent,
            ColorSet::Of(Reg::Last),
            BoardRegion::Bottom,
        )),
    ],
];

// Draw two [0]. Return one of them.
const TRACKING: [Program; 1] = [&[draw(0), BIND_LAST, draw(0), return_(DRAWN_PAIR)]];

// Score another top card with a [Food] on your board. If you don't, you lose.
// / You may exchange all cards in your hand with all the highest cards in an
// opponent's score pile.
const CURING: [Program; 2] = [
    &[
        score(filtered(
            &TOPS,
            &[Has(HasIcon(Food)), Lacks(Is(Reg::Source))],
        )),
        if_(&Not(DID), 1),
        LOSE,
    ],
    &[Instruction::Exchange(ExchangeStep {
        left: HAND,
        right: filtered(&ScoreOf(Opponent), &[Highest]),
        counts: &[None, None],
        optional: true,
    })],
];

// Return a top card from your board. Draw a [0]. Reveal your hand. If you
// have exactly one card in your hand of the color of the returned card, meld
// it. Otherwise, you lose.
const FIRST_AID: [Program; 1] = [&[
    return_(Tops(Executor)),
    BIND_LAST,
    draw(0),
    reveal_all(HAND),
    if_(
        &Compare(Count(&filtered(&HAND, BOUND_COLOR)), Cmp::Equal, Const(1)),
        2,
    ),
    meld(filtered(&HAND, BOUND_COLOR)),
    Instruction::Jump(1),
    LOSE,
]];

// Reveal a card in your score pile of a color on your board. If you do, draw
// a [1].
const KNOTS: [Program; 1] = [&[
    reveal(filtered(&SCORE, &[Has(ColorOnBoard(Executor))])),
    if_(DID, 1),
    draw(1),
]];

/// The printed effects of an Age 0 card as (demand, program) pairs.
pub(super) fn effects(code: &str) -> Option<&'static [(bool, Program)]> {
    const fn plain<const N: usize>(programs: [Program; N]) -> [(bool, Program); N] {
        let mut out = [(false, programs[0]); N];
        let mut i = 0;
        while i < N {
            out[i] = (false, programs[i]);
            i += 1;
        }
        out
    }
    // The first effect of each listed card is a demand.
    const fn demand<const N: usize>(programs: [Program; N]) -> [(bool, Program); N] {
        let mut out = plain(programs);
        out[0].0 = true;
        out
    }
    Some(match code {
        "Z001" => &const { plain(FRESH_WATER) },
        "Z002" => &const { demand(SHELTER) },
        "Z003" => &const { plain(SKINNING) },
        "Z004" => &const { plain(FISHING) },
        "Z005" => &const { plain(FORAGING) },
        "Z006" => &const { demand(HUNTING) },
        "Z007" => &const { demand(FIRE) },
        "Z008" => &const { demand(STONE_KNIVES) },
        "Z009" => &const { demand(TRAPPING) },
        "Z010" => &const { plain(SPEAK_WITH_BIRD) },
        "Z011" => &const { plain(TANNING) },
        "Z012" => &const { plain(TRACKING) },
        "Z013" => &const { plain(CURING) },
        "Z014" => &const { plain(FIRST_AID) },
        "Z015" => &const { plain(KNOTS) },
        _ => return None,
    })
}
