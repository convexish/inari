//! Echoes card programs in code order. Each comment describes the printed
//! text, echo effect first. Complete setup requires every declaration.
use crate::program::build::*;

use Color::{Blue, Green, Purple, Red, Yellow};
use Cond::{All, Card, Compare, Did, Empty, Foreseen, Not};
use Expr::{Add, Const, Count, Div, Max, Total, Value};
use Filter::{Has, Lacks};
use Icon::{Efficiency, Health};
use Selector::{
    AvailableAchievements, BaseDeck, Board, Forecast, StandardAchievements, TopOf, Tops,
};
use Test::{
    BoxedValues, ColorIn, ColorOf, Eligible, Icon as HasIcon, InSlot, Is, StackShows,
    Value as ValueIs, Values,
};
use Who::{Demander, Either, Executor, Opponent};

use BonusValues::{Equal, Even, Odd, Repeated};

type Program = &'static [Instruction];

/// A card's echo effect (always non-demand) and its dogma effects, each
/// marked demand or not.
pub(super) struct Programs {
    pub(super) echo: Option<Program>,
    pub(super) dogmas: &'static [(bool, Program)],
}

const FORECAST: Selector = Forecast(Executor);
const ORIGIN: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: true,
};
const ORIGIN_ALL: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: false,
};
const WIN: Instruction = Instruction::Win(WinRule::Executor);
const SELF_EXECUTE: Instruction = execute(ExecutionMode::SelfExecute, Executor);

/// The cards an instruction drew, kept for later instructions.
const DRAWN: Slot = Slot::effect(0);
/// Those still in your hand: a karma may have moved one elsewhere.
const DRAWN_IN_HAND: Selector = filtered(&HAND, &[Has(InSlot(DRAWN))]);
/// The cards an instruction returned or transferred.
const MOVED: Slot = Slot::effect(0);
/// Verification disclosures remain named while live logical score members
/// may arrive later. Naming grants no access to cards outside that score pile.
const PROOF: Slot = Slot::effect(3);
const VERIFIED_SCORE: Selector = Selector::Union(&[filtered(&SCORE, &[Has(InSlot(PROOF))]), SCORE]);
/// A block's item: a value, color or card each pass.
const ITEM: Slot = Slot::effect(1);
/// What a card's echo did, for its dogma effects in the same action. Cards
/// are appended, so an endorsed or copied echo that does nothing leaves the
/// earlier record; `Reg::Slot` reads the latest.
const ECHOED: Slot = Slot::action(2);
/// The last card a player drew or melded due to its card's echo.
const LAST_ECHOED: Test = Is(Reg::Slot(ECHOED));
const ITEM_IN_HAND: Selector = filtered(&HAND, &[Has(Is(Reg::Slot(ITEM)))]);

const fn deck_junk(age: u8) -> Instruction {
    junk(BaseDeck(Expr::Boxed(age)))
        .all()
        .shortcut(Shortcut::DeckTop)
}

/// Take the deck top: all junk orders are equivalent except for an unobservable
/// modulo bias in a later blind claim.
macro_rules! junk_then_highest {
    ($age:expr $(, $prefix:expr)* $(,)?) => {{
        const AGE: Expr = $age;
        &[
            $($prefix,)*
            junk(BaseDeck(AGE)).all().optional().shortcut(Shortcut::DeckTop),
            if_(DID, 1),
            achieve(filtered(&Selector::Junk, &[
                Filter::Highest, Has(Eligible { score_factor: 1 }),
            ])),
        ]
    }};
}

/// The chosen value's Base deck is empty: junking it changes nothing, so
/// every empty deck is the same choice.
const EMPTY_CHOSEN_DECK: Cond = Compare(Count(&BaseDeck(Expr::Chosen)), Cmp::Equal, Const(0));

/// Colors on the executor's board showing the icon.
macro_rules! colors_with {
    ($icon:expr) => {
        Count(&filtered(&TOPS, &[Has(StackShows($icon))]))
    };
}

// Echo: Tuck a [1] from your hand. / Choose to either draw and foreshadow a
// [2], or tuck a [2] from your forecast. / If you have no cards in your
// forecast, draw and foreshadow a [3].
const BANGLE: Programs = Programs {
    echo: Some(&[tuck(filtered(&HAND, &[Has(BoxedValues(&[1]))]))]),
    dogmas: &[
        (
            false,
            &[
                choose(
                    Executor,
                    Choice::Branch(&[
                        (0, "Draw and foreshadow a [2]"),
                        (3, "Tuck a [2] from your forecast"),
                    ]),
                ),
                draw(2),
                foreshadow(LAST),
                Instruction::Jump(1),
                tuck(filtered(&FORECAST, &[Has(BoxedValues(&[2]))])),
            ],
        ),
        (
            false,
            &[
                if_(&Compare(Count(&FORECAST), Cmp::Equal, Const(0)), 2),
                draw(3),
                foreshadow(LAST),
            ],
        ),
    ],
};

const VERIFIED_HAND: Selector = Selector::Union(&[filtered(&HAND, &[Has(InSlot(PROOF))]), HAND]);
const CANDLE_CARDS: Selector = Selector::Union(&[
    filtered(&VERIFIED_HAND, &[Has(HasIcon(Icon::Authority))]),
    filtered(&VERIFIED_HAND, &[Has(HasIcon(Icon::Concept))]),
]);

// Echo: If no player has fewer points than you, draw a [3]. / I DEMAND
// you transfer a card with Authority or Concept from your hand to my hand!
// If you do, draw a [1]!
const CANDLES: Programs = Programs {
    echo: Some(&[
        if_(
            &Compare(
                Expr::Metric(Executor, Metric::Score),
                Cmp::AtMost,
                Expr::Metric(Opponent, Metric::Score),
            ),
            1,
        ),
        draw(3),
    ]),
    dogmas: &[(
        true,
        &[
            reveal_all(filtered(&HAND, &[Lacks(Test::Inspectable(Executor))])).keep(PROOF),
            // The hand proves an empty selection, as Bandage's does, so no
            // branch on the private hand survives into the draw.
            transfer(CANDLE_CARDS, Destination::Hand(Demander))
                .verify()
                .reveal_if_empty(&HAND),
            if_(DID, 1),
            draw(1),
        ],
    )],
};

// Echo: Score the bottom blue card from your board. / Junk all cards in
// the [1] deck.
const PLUMBING: Programs = Programs {
    echo: Some(&[score(Board(
        Executor,
        ColorSet::Only(&[Blue]),
        BoardRegion::Bottom,
    ))]),
    dogmas: &[(false, &[deck_junk(1)])],
};

// Echo: You may draw and foreshadow a [1]. / You may junk all cards in
// the [1] deck. If you do, achieve the highest card in the junk if eligible.
const CHOPSTICKS: Programs = Programs {
    echo: Some(&[draw(1).optional(), foreshadow(LAST)]),
    dogmas: &[(false, junk_then_highest!(Expr::Boxed(1)))],
};

// If you have more [1] in your hand than every opponent, draw and score a
// [2]. / Draw and reveal a [1]. If it is yellow, score all [1] from your hand.
const NOODLES: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                if_(
                    &Compare(
                        Count(&filtered(&HAND, &[Has(BoxedValues(&[1]))])),
                        Cmp::Greater,
                        Count(&filtered(
                            &Selector::Hand(Opponent),
                            &[Has(BoxedValues(&[1]))],
                        )),
                    ),
                    2,
                ),
                draw(2),
                score(LAST),
            ],
        ),
        (
            false,
            &[
                draw(1),
                reveal(LAST),
                if_(&Card(Reg::Last, Has(Test::Color(Yellow))), 1),
                score(filtered(&HAND, &[Has(BoxedValues(&[1]))]))
                    .all()
                    .shortcut(ORIGIN_ALL),
            ],
        ),
    ],
};

// Choose a color. You may tuck any number of cards of that color from your
// hand. If you do, and your top card of that color is higher than each
// opponent's, you may achieve (if eligible) a card from your hand.
const SOAP: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Colors(1)),
            tuck(filtered(&HAND, &[Has(ColorIn(ColorSet::Chosen))]))
                .up_to(None)
                .optional(),
            if_(
                &All(&[
                    Did(Outcome::All),
                    Compare(
                        Max(&Board(Executor, ColorSet::Chosen, BoardRegion::Top)),
                        Cmp::Greater,
                        Max(&Board(Opponent, ColorSet::Chosen, BoardRegion::Top)),
                    ),
                ]),
                1,
            ),
            achieve(filtered(&HAND, &[Has(Eligible { score_factor: 1 })])).optional(),
        ],
    )],
};

const COMB_COLOR: Slot = Slot::effect(2);
// Choose a color, then draw and reveal five [1]. Return the drawn cards
// that do not match the chosen color. If Comb was foreseen, return all
// cards of the chosen color from all boards.
const COMB: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Colors(1)).keep(COMB_COLOR),
            for_each(Domain::Times(Const(5)), Order::Printed, None, 2),
            draw(1).add(DRAWN),
            reveal(LAST),
            return_(filtered(
                &DRAWN_IN_HAND,
                &[Lacks(ColorIn(ColorSet::Slot(COMB_COLOR)))],
            ))
            .all()
            .shortcut(ORIGIN_ALL),
            if_(&Foreseen, 1),
            return_(Board(
                Either,
                ColorSet::Slot(COMB_COLOR),
                BoardRegion::BelowTop(0),
            ))
            .all()
            .shortcut(ORIGIN_ALL),
        ],
    )],
};

// Echo: You may meld a card from your hand. / Return any number of cards
// from your hand. Score two cards from your hand for every card you return.
const UMBRELLA: Programs = Programs {
    echo: Some(&[meld(HAND).optional()]),
    dogmas: &[(
        false,
        &[
            return_(HAND).up_to(None).shortcut(ORIGIN),
            score(HAND)
                .exactly(Expr::Mul(&Expr::Done, 2))
                .shortcut(ORIGIN_ALL),
        ],
    )],
};

// An original draw can stay hidden if its tuck was replaced. Its face must
// become public before a later condition inspects it; ordinary tucks already
// provide that witness and need no extra reveal.
const HIDDEN_DRAWN: Selector = Selector::Union(&[
    filtered(
        &Selector::Slot(DRAWN),
        &[Lacks(Test::Inspectable(Executor))],
    ),
    filtered(
        &Selector::Slot(DRAWN),
        &[Lacks(Test::Inspectable(Opponent))],
    ),
]);

// Echo: Draw and tuck a [1]. If it has Authority, repeat this effect. /
// I DEMAND you transfer a top card of different value from any top card on
// my board from your board to mine! If you do, draw and meld a card of equal value!
const PERFUME: Programs = Programs {
    echo: Some(&[
        draw(1).keep(DRAWN),
        tuck(LAST),
        reveal_all(HIDDEN_DRAWN),
        if_(&Card(Reg::Slot(DRAWN), Has(HasIcon(Icon::Authority))), 1),
        REPEAT,
    ]),
    dogmas: &[(
        true,
        &[
            transfer(
                filtered(&TOPS, &[Lacks(Test::ValueIn(&Selector::Tops(Demander)))]),
                Destination::Board(Demander),
            )
            .keep(MOVED),
            if_(DID, 2),
            draw_age(Value(Reg::Slot(MOVED))),
            meld(LAST),
        ],
    )],
};

const PUPPET_ACHIEVEMENTS: Selector =
    filtered(&AvailableAchievements, &[Has(Test::ValueIn(&SCORE))]);
// Junk an available achievement of value equal to the value of a card in
// your score pile. Choose by public back; identical backs sample natively.
const PUPPET: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Card(&PUPPET_ACHIEVEMENTS)),
            junk(filtered(&PUPPET_ACHIEVEMENTS, &[Has(Is(Reg::Chosen))])),
        ],
    )],
};

// If Ice Skates was foreseen, junk all cards in the [1] deck and [2] deck. /
// Return up to two cards from your hand. For each card you return, either
// draw and meld a [2], or draw and foreshadow a [3].
const ICE_SKATES: Programs = Programs {
    echo: None,
    dogmas: &[
        (false, &[if_(&Foreseen, 2), deck_junk(1), deck_junk(2)]),
        (
            false,
            &[
                return_(HAND).up_to(Some(Const(2))).shortcut(ORIGIN),
                for_each(Domain::Times(Expr::Done), Order::Printed, None, 6),
                choose(
                    Executor,
                    Choice::Branch(&[(0, "Draw and meld a [2]"), (3, "Draw and foreshadow a [3]")]),
                ),
                draw(2),
                meld(LAST),
                Instruction::Jump(2),
                draw(3),
                foreshadow(LAST),
            ],
        ),
    ],
};

// Echo: Draw a [2]. / Draw two Echoes [1]. Foreshadow one of them and return
// the other.
const RULER: Programs = Programs {
    echo: Some(&[draw(2)]),
    dogmas: &[(
        false,
        &[
            draws(Const(2), 1, Then::Keep)
                .set(CardSet::Echoes)
                .keep(DRAWN),
            // Choose first: the other card is returned even if the Forecast
            // refuses the chosen one.
            choose(Executor, Choice::Card(&DRAWN_IN_HAND)),
            foreshadow(filtered(&HAND, &[Has(Is(Reg::Chosen))])),
            return_(filtered(&DRAWN_IN_HAND, &[Lacks(Is(Reg::Chosen))])),
        ],
    )],
};

// Echo: You may score a card from your hand. / Draw and foreshadow a [1] and
// a [2]. / If Bell was foreseen, return all cards from all hands.
const BELL: Programs = Programs {
    echo: Some(&[score(HAND).optional()]),
    dogmas: &[
        (
            false,
            &[draw(1), foreshadow(LAST), draw(2), foreshadow(LAST)],
        ),
        (
            false,
            &[
                if_(&Foreseen, 1),
                return_(Selector::Hand(Either)).all().shortcut(ORIGIN_ALL),
            ],
        ),
    ],
};

// Echo: You may draw and foreshadow a [2] or [3]. / Transfer a top
// card with neither Authority nor Industry, then draw and meld a [2].
const HORSESHOES: Programs = Programs {
    echo: Some(&[
        choose(Executor, Choice::Value(ValueChoices::Boxed(&[2, 3]))).optional(),
        draw_age(Expr::Chosen),
        foreshadow(LAST),
    ]),
    dogmas: &[(
        true,
        &[
            transfer(
                filtered(
                    &TOPS,
                    &[
                        Lacks(HasIcon(Icon::Authority)),
                        Lacks(HasIcon(Icon::Industry)),
                    ],
                ),
                Destination::Board(Demander),
            ),
            if_(&Did(Outcome::All), 2),
            draw(2),
            meld(LAST),
        ],
    )],
};

// Echo: Tuck every hand card of one present value. / Optional left splay.
// Optionally junk the [2] deck, then achieve a highest junk card if eligible.
const TOOTHBRUSH: Programs = Programs {
    echo: Some(&[
        choose(Executor, Choice::Value(ValueChoices::Among(&HAND))),
        tuck(filtered(&HAND, &[Has(ValueIs(Cmp::Equal, Expr::Chosen))]))
            .all()
            .shortcut(Shortcut::ByColor),
    ]),
    dogmas: &[
        (false, &[splay(&Color::ALL, Splay::Left)]),
        (false, junk_then_highest!(Expr::Boxed(2))),
    ],
};

const GLASSBLOWING: Programs = Programs {
    echo: Some(&[score(filtered(&HAND, &[Lacks(Test::Set(CardSet::Base))]))]),
    dogmas: &[
        (
            false,
            &[
                draw_age(Add(
                    &Max(&filtered(
                        &TOPS,
                        &[Lacks(Test::Color(Green)), Filter::Lowest],
                    )),
                    3,
                )),
                foreshadow(LAST),
            ],
        ),
        (
            false,
            &[
                unless_decks_empty!(2, 3),
                choose(Executor, Choice::Value(ValueChoices::Boxed(&[2, 3]))),
                junk(BaseDeck(Expr::Chosen))
                    .all()
                    .shortcut(Shortcut::DeckTop),
            ],
        ),
    ],
};

const CHATURANGA: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                meld(HAND),
                // This draw is independent: no successful meld means value zero.
                // LAST selects a hand card; the successful meld is now on the board.
                draw_age(Max(&Selector::Reg(Reg::Last))),
                foreshadow(LAST),
            ],
        ),
        (
            false,
            &[if_(&Foreseen, 2), draw_age(Count(&TOPS)), foreshadow(LAST)],
        ),
    ],
};

// Echo: Draw two [2]. / You may return any number of cards from your hand.
// For every two cards of matching value you return, draw a card of value one
// higher.
const LEVER: Programs = Programs {
    echo: Some(&[draws(Const(2), 2, Then::Keep)]),
    dogmas: &[(
        false,
        &[
            return_(HAND)
                .up_to(None)
                .optional()
                .shortcut(ORIGIN)
                .keep(MOVED),
            // The returned values, from the records wherever the cards are
            // now; the player orders the rewards (empty decks interact).
            for_each(
                Domain::Values(&Selector::Slot(MOVED), Multiplicity::Distinct),
                Order::Chosen(Executor),
                Some(ITEM),
                1,
            ),
            draws_age(
                Div(
                    &Count(&filtered(
                        &Selector::Slot(MOVED),
                        &[Has(ValueIs(Cmp::Equal, Expr::Slot(ITEM)))],
                    )),
                    2,
                ),
                Add(&Expr::Slot(ITEM), 1),
                Then::Keep,
            ),
        ],
    )],
};

// Echo: Draw a [3] or [4]. / You may meld the last card you drew due to
// Charitable Trust's echo effect. If you meld a [3], achieve your top green
// card, if eligible. If you meld a [4], return your top green card.
const CHARITABLE_TRUST: Programs = Programs {
    echo: Some(&[
        choose(Executor, Choice::Value(ValueChoices::Boxed(&[3, 4]))),
        draw_age(Expr::Chosen).add(ECHOED),
    ]),
    dogmas: &[(
        false,
        &[
            meld(filtered(&HAND, &[Has(LAST_ECHOED)]))
                .optional()
                .keep(MOVED),
            if_(&Card(Reg::Slot(MOVED), Has(BoxedValues(&[3]))), 1),
            achieve(filtered(
                &TopOf(Executor, Green),
                &[Has(Eligible { score_factor: 1 })],
            )),
            if_(&Card(Reg::Slot(MOVED), Has(BoxedValues(&[4]))), 1),
            return_(TopOf(Executor, Green)),
        ],
    )],
};

// Draw and foreshadow a [3]. You may tuck another card of matching color from
// your hand. If you do, meld the drawn card. If you do, and Pagoda was
// foreseen, meld all cards of that color from all other boards.
const PAGODA: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            draw(3).keep(DRAWN),
            foreshadow(DRAWN_IN_HAND),
            tuck(filtered(
                &HAND,
                &[Lacks(InSlot(DRAWN)), Has(ColorOf(Reg::Slot(DRAWN)))],
            ))
            .optional(),
            if_(DID, 3),
            // From the Forecast, or from hand if the Forecast was full.
            meld(Selector::Union(&[
                filtered(&FORECAST, &[Has(InSlot(DRAWN))]),
                DRAWN_IN_HAND,
            ])),
            if_(&All(&[Did(Outcome::All), Foreseen]), 1),
            meld(Board(
                Opponent,
                ColorSet::Of(Reg::Slot(DRAWN)),
                BoardRegion::BelowTop(0),
            ))
            .all(),
        ],
    )],
};

/// No colour on the executor's board shows [Health]: X-Ray draws nothing
/// whatever value is named.
const NO_HEALTH_COLORS: Cond = Compare(colors_with!(Health), Cmp::Equal, Const(0));

// Echo: Draw and tuck an [8]. / Choose a value. For every color on your board
// with [Health], draw a card of that value. Foreshadow any number of them. /
// Return all cards from your hand. / You may splay your yellow cards up.
const X_RAY: Programs = Programs {
    echo: Some(&[draw(8), tuck(LAST)]),
    dogmas: &[
        (
            false,
            &[
                choose(Executor, Choice::Value(ValueChoices::Any))
                    .alias(Alias::NoOpWhen(&NO_HEALTH_COLORS)),
                draws_age(colors_with!(Health), Expr::Chosen, Then::Keep).keep(DRAWN),
                foreshadow(DRAWN_IN_HAND).up_to(None).shortcut(ORIGIN),
            ],
        ),
        (false, &[return_(HAND).all().shortcut(ORIGIN_ALL)]),
        (false, &[splay(&[Yellow], Splay::Up)]),
    ],
};

// Echo: Draw and foreshadow a [9]. / You may transfer a top non-green card
// from your board to your hand. If you do, draw and score a card of equal
// value. If you do, and Credit Card was foreseen, repeat this effect. / You
// may splay your green cards up.
const CREDIT_CARD: Programs = Programs {
    echo: Some(&[draw(9), foreshadow(LAST)]),
    dogmas: &[
        (
            false,
            &[
                transfer(
                    filtered(&TOPS, &[Lacks(Test::Color(Green))]),
                    Destination::Hand(Executor),
                )
                .optional()
                .keep(MOVED),
                if_(DID, 5),
                once(2),
                draw_age(Value(Reg::Slot(MOVED))),
                score(LAST),
                if_(&All(&[Did(Outcome::All), Foreseen]), 1),
                REPEAT,
            ],
        ),
        (false, &[splay(&[Green], Splay::Up)]),
    ],
};

// Echo: Draw and foreshadow an [11]. / Draw a [10] for every color on your
// board with [Efficiency]. / You may splay your green cards up. / You may
// tuck any number of cards with [Efficiency] from your hand, splaying up each
// color into which you tuck.
const CELL_PHONE: Programs = Programs {
    echo: Some(&[draw(11), foreshadow(LAST)]),
    dogmas: &[
        (false, &[draws(colors_with!(Efficiency), 10, Then::Keep)]),
        (false, &[splay(&[Green], Splay::Up)]),
        (
            false,
            &[
                // Each tuck's splay comes before the next choice.
                for_each(
                    Domain::Live {
                        from: &filtered(&HAND, &[Has(HasIcon(Efficiency))]),
                        per_color: false,
                        stop: Stop::MayAnyTime,
                    },
                    Order::Chosen(Executor),
                    Some(ITEM),
                    3,
                ),
                tuck(filtered(&HAND, &[Has(Is(Reg::Slot(ITEM)))])),
                if_(DID, 1),
                splay_set(ColorSet::Of(Reg::Slot(ITEM)), Splay::Up),
            ],
        ),
    ],
};

// For each non-yellow top card on your board, draw and score a card of equal
// value, in ascending order. If you do, and Esports was foreseen, you win.
const ESPORTS: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            for_each(
                Domain::Cards(&filtered(&TOPS, &[Lacks(Test::Color(Yellow))])),
                Order::Ascending,
                Some(ITEM),
                2,
            ),
            draw_age(Value(Reg::Slot(ITEM))),
            score(LAST),
            if_(&All(&[Did(Outcome::All), Foreseen]), 1),
            WIN,
        ],
    )],
};

/// The executor's pile of the block's color, all of it.
const PILE: Expr = Count(&Board(
    Executor,
    ColorSet::Slot(ITEM),
    BoardRegion::BelowTop(0),
));

/// The pile has between `low` and `high` cards.
macro_rules! pile_between {
    ($low:expr, $high:expr) => {
        &All(&[
            Compare(PILE, Cmp::AtLeast, Const($low)),
            Compare(PILE, Cmp::AtMost, Const($high)),
        ])
    };
}

// Splay each color on your board in a direction according to the exact
// number of cards of that color on your board: two or three, aslant; four or
// five, up; six or seven, right; eight or more, left. If you splay five
// colors, you win.
const TOUCHSCREEN: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            // Splays of different piles commute.
            for_each(
                Domain::Colors(ColorSet::OfCards(&TOPS)),
                Order::Chosen(Executor),
                Some(ITEM),
                11,
            )
            .shortcut(Shortcut::AnyOrder),
            if_(pile_between!(2, 3), 2),
            splay_set(ColorSet::Slot(ITEM), Splay::Aslant),
            Instruction::Jump(8),
            if_(pile_between!(4, 5), 2),
            splay_set(ColorSet::Slot(ITEM), Splay::Up),
            Instruction::Jump(5),
            if_(pile_between!(6, 7), 2),
            splay_set(ColorSet::Slot(ITEM), Splay::Right),
            Instruction::Jump(2),
            if_(&Compare(PILE, Cmp::AtLeast, Const(8)), 1),
            splay_set(ColorSet::Slot(ITEM), Splay::Left),
            // Only splays that changed a pile count.
            if_(&Compare(Expr::Done, Cmp::Equal, Const(5)), 1),
            WIN,
        ],
    )],
};

const ODD: Selector = filtered(&HAND, &[Has(Values(&[1, 3, 5, 7, 9, 11]))]);
/// The cards a demand returned, for the same execution's dogma: both runs of
/// an endorsed demand.
const DEMANDED: Slot = Slot::execution(0);

// Echo: Score an odd-valued card from your hand. / I DEMAND you return all
// odd-valued cards in your hand! Draw a [6]! / Draw a card of value one
// higher than the highest card returned due to the demand. / You may splay
// your red cards right.
const MORPHINE: Programs = Programs {
    echo: Some(&[score(ODD)]),
    dogmas: &[
        (
            true,
            &[
                return_(ODD).all().shortcut(ORIGIN_ALL).add(DEMANDED),
                draw(6),
            ],
        ),
        (
            false,
            &[draw_age(Add(
                &Max(&Selector::Slot(DEMANDED.of(Opponent))),
                1,
            ))],
        ),
        (false, &[splay(&[Red], Splay::Right)]),
    ],
};

// Echo: Meld a blue or yellow card from your hand. / Draw a [7]. If you
// melded a blue card due to Stethoscope's echo effect, draw an [8], and if
// Stethoscope was foreseen, also draw a [9]. / You may splay your yellow
// cards right.
const STETHOSCOPE: Programs = Programs {
    echo: Some(&[
        meld(filtered(
            &HAND,
            &[Has(ColorIn(ColorSet::Only(&[Blue, Yellow])))],
        ))
        .reveal_if_empty(&HAND),
        // The color is judged as it is melded, in public.
        if_(
            &All(&[Did(Outcome::All), Card(Reg::Last, Has(Test::Color(Blue)))]),
            1,
        ),
        Instruction::Bind(Binding::Value(ECHOED, Const(1))),
    ]),
    dogmas: &[
        (
            false,
            &[
                draw(7),
                if_(&Not(&Empty(ECHOED)), 3),
                draw(8),
                if_(&Foreseen, 1),
                draw(9),
            ],
        ),
        (false, &[splay(&[Yellow], Splay::Right)]),
    ],
};

/// The color of the demander's latest Jet echo meld this action.
const JET_MELD: Slot = ECHOED.of(Demander);

// Echo: Meld a card from your hand. / I DEMAND you return your top card of
// the color I last melded due to Jet's echo effect during this action! Junk
// all available achievements of values equal to the melded card and the
// returned card! / Draw and foreshadow a [10].
const JET: Programs = Programs {
    echo: Some(&[meld(HAND).add(ECHOED)]),
    dogmas: &[
        (
            true,
            &[
                return_(Board(
                    Executor,
                    ColorSet::Of(Reg::Slot(JET_MELD)),
                    BoardRegion::Top,
                )),
                junk(Selector::Union(&[
                    filtered(
                        &AvailableAchievements,
                        &[Has(ValueIs(Cmp::Equal, Value(Reg::Slot(JET_MELD))))],
                    ),
                    filtered(
                        &AvailableAchievements,
                        &[Has(ValueIs(Cmp::Equal, Value(Reg::Last)))],
                    ),
                ]))
                .all()
                .shortcut(Shortcut::AnyOrder),
            ],
        ),
        (false, &[draw(10), foreshadow(LAST)]),
    ],
};

// Echo: Draw and meld a card of a value present in your hand. / Transfer your
// bottom card of each color to your hand. / Claim an available achievement
// of value equal to the card you last melded due to Karaoke's echo effect
// during this action, regardless of eligibility. If you do, self-execute the
// melded card.
const KARAOKE: Programs = Programs {
    echo: Some(&[
        // With no value to choose, draw a 0.
        if_(&Compare(Count(&HAND), Cmp::Equal, Const(0)), 2),
        draw_age(Const(0)),
        Instruction::Jump(2),
        choose(Executor, Choice::Value(ValueChoices::Among(&HAND))),
        draw_age(Expr::Chosen),
        meld(LAST).add(ECHOED),
    ]),
    dogmas: &[
        (
            false,
            &[transfer(
                Board(Executor, ColorSet::Only(&Color::ALL), BoardRegion::Bottom),
                Destination::Hand(Executor),
            )
            .per_color(None)
            .shortcut(ORIGIN_ALL)],
        ),
        (
            false,
            &[
                achieve(filtered(
                    &AvailableAchievements,
                    &[Has(ValueIs(Cmp::Equal, Value(Reg::Slot(ECHOED))))],
                )),
                if_(DID, 2),
                bind(Selector::Reg(Reg::Slot(ECHOED))),
                SELF_EXECUTE,
            ],
        ),
    ],
};

/// The chosen color, kept when the effect repeats.
const COLOR: Slot = Slot::effect(0);

// Choose a color on your board. Choose to either achieve the top card of that
// color on your board, if eligible, or score it. If you do either, and
// Streaming was foreseen, repeat this effect using the same color.
const STREAMING: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            if_(&Empty(COLOR), 1),
            choose(Executor, Choice::Color(ColorSet::OfCards(&TOPS))).keep(COLOR),
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Achieve the top card, if eligible"),
                    (2, "Score the top card"),
                ]),
            ),
            achieve(filtered(
                &Board(Executor, ColorSet::Slot(COLOR), BoardRegion::Top),
                &[Has(Eligible { score_factor: 1 })],
            )),
            Instruction::Jump(1),
            score(Board(Executor, ColorSet::Slot(COLOR), BoardRegion::Top)),
            if_(&All(&[Did(Outcome::All), Foreseen]), 1),
            repeat_keeping(COLOR),
        ],
    )],
};

/// Set once the effect has repeated.
const REPEATED: Slot = Slot::effect(1);
const SCORED: Selector = Selector::Slot(MOVED);

// Score two bottom non-blue cards of different colors on your board. If you
// score two and they have a total value less than 12, draw a card of that
// total value and repeat this effect (once only). / You may splay your blue
// cards up.
const CALCULATOR: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                score(Board(
                    Executor,
                    ColorSet::Only(&[Red, Green, Yellow, Purple]),
                    BoardRegion::Bottom,
                ))
                .per_color(Some(Const(2)))
                // Only when both remaining colors must go:
                // which two of three or four is a real choice.
                .shortcut(ORIGIN_ALL)
                .keep(MOVED),
                if_(
                    &All(&[
                        Compare(Count(&SCORED), Cmp::Equal, Const(2)),
                        Compare(Total(&SCORED), Cmp::Less, Const(12)),
                    ]),
                    4,
                ),
                draw_age(Total(&SCORED)),
                if_(&Empty(REPEATED), 2),
                Instruction::Bind(Binding::Value(REPEATED, Const(1))),
                repeat_keeping(REPEATED),
            ],
        ),
        (false, &[splay(&[Blue], Splay::Up)]),
    ],
};

/// Visible bonuses on the executor's board (Echoes bonus queries).
const MY_BONUSES: Bonuses = Bonuses {
    on: Executor,
    only: BonusValues::All,
};
const fn bonus_count(bonuses: &'static Bonuses) -> Expr {
    Expr::Bonuses(bonuses, Tally::Count)
}
const ACHIEVEMENTS_OF_CHOSEN: Selector = filtered(
    &AvailableAchievements,
    &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
);

// Draw and reveal an Echoes [1]. If the card has a bonus, draw and meld a card
// of value equal to its bonus. / If Dice was foreseen, draw a [4], then
// transfer it to the hand of an opponent with more bonus points than you.
const DICE: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                draw(1).set(CardSet::Echoes),
                reveal(LAST),
                if_(&Card(Reg::Last, Has(Test::Bonus)), 2),
                draw_age(Expr::BonusOf(Reg::Last)),
                meld(LAST),
            ],
        ),
        (
            false,
            &[
                if_(&Foreseen, 3),
                draw(4),
                if_(
                    &Compare(
                        Expr::Metric(Opponent, Metric::BonusPoints),
                        Cmp::Greater,
                        Expr::Metric(Executor, Metric::BonusPoints),
                    ),
                    1,
                ),
                transfer(LAST, Destination::Hand(Opponent)),
            ],
        ),
    ],
};

// Echo: Draw a [3] or Draw and foreshadow a [4]. / Draw a card of value equal
// to a bonus on any board, if there is one. If you do, and Linguistics was
// foreseen, junk all available achievements of that value.
const LINGUISTICS: Programs = Programs {
    echo: Some(&[
        choose(
            Executor,
            Choice::Branch(&[(0, "Draw a [3]"), (2, "Draw and foreshadow a [4]")]),
        ),
        draw(3),
        Instruction::Jump(2),
        draw(4),
        foreshadow(LAST),
    ]),
    dogmas: &[(
        false,
        &[
            choose(
                Executor,
                Choice::Value(ValueChoices::Bonuses(&Bonuses {
                    on: Either,
                    only: BonusValues::All,
                })),
            ),
            draw_age(Expr::Chosen),
            // Only a draw of the chosen value (provisional ruling).
            if_(&All(&[Did(Outcome::All), Foreseen]), 1),
            junk(ACHIEVEMENTS_OF_CHOSEN)
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    )],
};

// I DEMAND you draw a card of value equal to the highest bonus on your board!
// Transfer it to my forecast! If it is red, transfer all cards from your hand
// to my score pile!
const LIQUID_FIRE: Programs = Programs {
    echo: None,
    dogmas: &[(
        true,
        &[
            draw_age(Expr::Bonuses(&MY_BONUSES, Tally::Max)).keep(DRAWN),
            transfer(DRAWN_IN_HAND, Destination::Forecast(Demander)),
            // A full Forecast leaves it in hand: reveal it, since its color decides the
            // rest.
            reveal(DRAWN_IN_HAND),
            if_(&Card(Reg::Slot(DRAWN), Has(Test::Color(Red))), 1),
            transfer(HAND, Destination::Score(Demander))
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    )],
};

// Echo: Draw and foreshadow an Echoes [5]. / You may place a card from
// your forecast on top of its deck. If you do, exchange all cards in your
// forecast with an equal number of available standard achievements, and
// junk all the cards in the [5] deck.
const TELESCOPE: Programs = Programs {
    echo: Some(&[draw(5).set(CardSet::Echoes), foreshadow(LAST)]),
    dogmas: &[(
        false,
        &[
            verb(Verb::Place, FORECAST).optional(),
            if_(DID, 2),
            Instruction::Exchange(ExchangeStep {
                left: FORECAST,
                right: StandardAchievements,
                counts: &[None, Some(Count(&FORECAST))],
                optional: false,
            }),
            deck_junk(5),
        ],
    )],
};

// Echo: Draw and tuck a [4]. / I DEMAND you return a card from your score
// pile matching each different bonus value on my board! / You may return a
// card from your hand and draw a card of the same value.
const TOILET: Programs = Programs {
    echo: Some(&[draw(4), tuck(LAST)]),
    dogmas: &[
        (
            true,
            &[return_(SCORE)
                .per_value(&PerValue {
                    source: ValueSource::Bonuses(Bonuses {
                        on: Demander,
                        only: BonusValues::All,
                    }),
                    multiplicity: Multiplicity::Distinct,
                })
                .shortcut(ORIGIN)],
        ),
        (
            false,
            &[return_(HAND).optional(), if_(DID, 1), draw_age(LAST_VALUE)],
        ),
    ],
};

// Transfer another top card to its owner's score pile, then optionally
// return a hand card sharing a printed icon type and repeat on actual return.
// The branch fixes the original owner before any transfer reaction runs.
const HELICOPTER_TOPS: Selector = filtered(&Tops(Either), &[Lacks(Test::Code("E086"))]);
const HELICOPTER_OWN: Selector = filtered(&TOPS, &[Has(Is(Reg::Chosen))]);
const HELICOPTER: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Card(&HELICOPTER_TOPS)),
            if_(&Compare(Count(&HELICOPTER_OWN), Cmp::Greater, Const(0)), 2),
            transfer(HELICOPTER_OWN, Destination::Score(Executor)).keep(MOVED),
            Instruction::Jump(1),
            transfer(
                filtered(&Tops(Opponent), &[Has(Is(Reg::Chosen))]),
                Destination::Score(Opponent),
            )
            .keep(MOVED),
            if_(DID, 3),
            return_(filtered(&HAND, &[Has(Test::SharesIcon(Reg::Slot(MOVED)))]))
                .optional()
                .verify(),
            if_(DID, 1),
            REPEAT,
        ],
    )],
};

// Echo: Draw and score any value. / I DEMAND your highest eligible non-yellow
// top without Prosperity! / You may splay purple up. / Junk the [10] deck.
const ATM: Programs = Programs {
    echo: Some(&[
        choose(Executor, Choice::Value(ValueChoices::Any)),
        draw_age(Expr::Chosen),
        score(LAST),
    ]),
    dogmas: &[
        (
            true,
            &[transfer(
                filtered(
                    &TOPS,
                    &[
                        Lacks(Test::Color(Yellow)),
                        Lacks(HasIcon(Icon::Prosperity)),
                        Filter::Highest,
                    ],
                ),
                Destination::Board(Demander),
            )],
        ),
        (false, &[splay(&[Purple], Splay::Up)]),
        (false, &[deck_junk(10)]),
    ],
};

// Echo: Draw and foreshadow [10]. / Draw and foreshadow [9], also [11] if
// foreseen. / Self-execute a lowest top of a non-green color.
const EMAIL: Programs = Programs {
    echo: Some(&[draw(10), foreshadow(LAST)]),
    dogmas: &[
        (
            false,
            &[
                draw(9),
                foreshadow(LAST),
                if_(&Foreseen, 2),
                draw(11),
                foreshadow(LAST),
            ],
        ),
        (
            false,
            &[
                bind(filtered(
                    &TOPS,
                    &[Lacks(Test::Color(Green)), Filter::Lowest],
                )),
                SELF_EXECUTE,
            ],
        ),
    ],
};

// Return four score cards, then all physical standard achievements. / Draw
// and foreshadow [11], draw and meld [10], then [11] if foreseen.
const LASER: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                return_(SCORE).exactly(Const(4)).shortcut(ORIGIN_ALL),
                return_(Selector::StandardAchievements)
                    .all()
                    .shortcut(ORIGIN_ALL),
            ],
        ),
        (
            false,
            &[
                draw(11),
                foreshadow(LAST),
                draw(10),
                meld(LAST),
                if_(&Foreseen, 2),
                draw(11),
                meld(LAST),
            ],
        ),
    ],
};

// I DEMAND your top green card into my hand! If its new top is Scissors,
// I win. / You may score a top; if Paper is your green top, you win.
const ROCK: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[
                transfer(TopOf(Executor, Green), Destination::Hand(Demander)),
                if_(
                    &Compare(
                        Count(&filtered(
                            &TopOf(Executor, Green),
                            &[Has(Test::Code("E021"))],
                        )),
                        Cmp::Greater,
                        Const(0),
                    ),
                    1,
                ),
                Instruction::Win(WinRule::Player(Demander)),
            ],
        ),
        (
            false,
            &[
                score(TOPS).optional(),
                if_(
                    &Compare(
                        Count(&filtered(
                            &TopOf(Executor, Green),
                            &[Has(Test::Code("B031"))],
                        )),
                        Cmp::Greater,
                        Const(0),
                    ),
                    1,
                ),
                WIN,
            ],
        ),
    ],
};

/// Top cards with a bonus.
const BONUS_TOPS: Selector = filtered(&TOPS, &[Has(Test::Bonus)]);
const ITEM_BONUS: Expr = Expr::BonusOf(Reg::Slot(ITEM));

// Echo: Draw and tuck a [4]. / Draw and reveal a [6]. If it has Industry,
// meld it and repeat; otherwise foreshadow it. / If foreseen, score your top
// card of each non-red color.
const COKE: Programs = Programs {
    echo: Some(&[draw(4), tuck(LAST)]),
    dogmas: &[
        (
            false,
            &[
                draw(6),
                reveal(LAST),
                if_(&Card(Reg::Last, Has(HasIcon(Icon::Industry))), 2),
                meld(LAST),
                REPEAT,
                foreshadow(LAST),
            ],
        ),
        (
            false,
            &[
                if_(&Foreseen, 1),
                score(filtered(&TOPS, &[Lacks(Test::Color(Red))]))
                    .per_color(None)
                    .shortcut(Shortcut::AnyOrder),
            ],
        ),
    ],
};

const OCTANT_TOPS: Selector = Selector::Union(&[
    filtered(&TOPS, &[Lacks(Test::Color(Red)), Has(HasIcon(Health))]),
    filtered(
        &TOPS,
        &[Lacks(Test::Color(Red)), Has(HasIcon(Icon::Industry))],
    ),
]);
// I DEMAND you transfer a qualifying Health or Industry top of each non-red
// color to my board! If at least one transfers and this wasn't foreseen,
// draw and foreshadow a [6]! / Draw and foreshadow a [6].
const OCTANT: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[
                transfer(OCTANT_TOPS, Destination::Board(Demander))
                    .per_color(None)
                    .shortcut(Shortcut::AnyOrder),
                if_(&All(&[Did(Outcome::Any), Not(&Foreseen)]), 2),
                draw(6),
                foreshadow(LAST),
            ],
        ),
        (false, &[draw(6), foreshadow(LAST)]),
    ],
};

// Echo: Score a top card without Industry. / Draw and tuck a [4]. If your
// live top of the successfully tucked card's color is below 4, draw and
// score a [4]. / You may splay your green cards right.
const STOVE: Programs = Programs {
    echo: Some(&[score(filtered(&TOPS, &[Lacks(HasIcon(Icon::Industry))]))]),
    dogmas: &[
        (
            false,
            &[
                draw(4),
                tuck(LAST).keep(MOVED),
                if_(DID, 3),
                if_(
                    &Compare(
                        Max(&Board(
                            Executor,
                            ColorSet::Of(Reg::Slot(MOVED)),
                            BoardRegion::Top,
                        )),
                        Cmp::Less,
                        Const(4),
                    ),
                    2,
                ),
                draw(4),
                score(LAST),
            ],
        ),
        (false, &[splay(&[Green], Splay::Right)]),
    ],
};

// I DEMAND you return a top card below value 3! / Choose the [3], [4], or
// [5] Base deck. You may junk it all, then achieve the highest junk if eligible.
const SEED_DRILL: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[return_(filtered(
                &TOPS,
                &[Has(ValueIs(Cmp::Less, Const(3)))],
            ))],
        ),
        (
            false,
            junk_then_highest!(
                Expr::Chosen,
                choose(Executor, Choice::Value(ValueChoices::Boxed(&[3, 4, 5])))
                    .alias(Alias::NoOpWhen(&EMPTY_CHOSEN_DECK)),
            ),
        ),
    ],
};

// Echo: Draw and tuck a [5]. / I DEMAND you draw and tuck a [5], then return
// your top card of the tucked card's color! / Return a top card. / Junk [6].
const LIGHTNING_ROD: Programs = Programs {
    echo: Some(&[draw(5), tuck(LAST)]),
    dogmas: &[
        (
            true,
            &[
                draw(5),
                tuck(LAST).keep(MOVED),
                return_(Board(
                    Executor,
                    ColorSet::Of(Reg::Slot(MOVED)),
                    BoardRegion::Top,
                )),
            ],
        ),
        (false, &[return_(TOPS)]),
        (false, &[deck_junk(6)]),
    ],
};

// Echo: Meld your bottom green card. / Draw and meld one higher than your
// live yellow top. Repeat for a successful yellow meld, or foreseen red/purple.
const THERMOMETER: Programs = Programs {
    echo: Some(&[meld(Board(
        Executor,
        ColorSet::Only(&[Green]),
        BoardRegion::Bottom,
    ))]),
    dogmas: &[(
        false,
        &[
            draw_age(Add(&Max(&TopOf(Executor, Yellow)), 1)),
            meld(LAST),
            if_(DID, 2),
            if_(
                &Cond::Any(&[
                    Card(Reg::Last, Has(Test::Color(Yellow))),
                    All(&[
                        Foreseen,
                        Cond::Any(&[
                            Card(Reg::Last, Has(Test::Color(Red))),
                            Card(Reg::Last, Has(Test::Color(Purple))),
                        ]),
                    ]),
                ]),
                1,
            ),
            REPEAT,
        ],
    )],
};

// Echo: Draw a value in either hand. / If your five tops have distinct values,
// return five score cards, then draw/score the current top values ascending.
const PIANO: Programs = Programs {
    echo: Some(&[
        if_(
            &Compare(Count(&Selector::Hand(Either)), Cmp::Equal, Const(0)),
            2,
        ),
        draw_age(Const(0)),
        Instruction::Jump(2),
        choose(
            Executor,
            Choice::Value(ValueChoices::Among(&Selector::Hand(Either))),
        ),
        draw_age(Expr::Chosen),
    ]),
    dogmas: &[(
        false,
        &[
            if_(
                &All(&[
                    Compare(Count(&TOPS), Cmp::Equal, Const(5)),
                    Compare(Expr::Distinct(&TOPS), Cmp::Equal, Const(5)),
                ]),
                4,
            ),
            return_(SCORE).exactly(Const(5)).shortcut(ORIGIN_ALL),
            for_each(
                Domain::Values(&TOPS, Multiplicity::Distinct),
                Order::Ascending,
                Some(ITEM),
                2,
            ),
            draw_age(Expr::Slot(ITEM)),
            score(LAST),
        ],
    )],
};

// Echo: Draw a value in either score pile. / Foreshadow, then draw and reveal
// that value. Meld the draw if above its same-color top; otherwise return the
// original draw still in hand and optionally repeat.
const TUNING_FORK: Programs = Programs {
    echo: Some(&[
        if_(
            &Compare(Count(&Selector::Score(Either)), Cmp::Equal, Const(0)),
            2,
        ),
        draw_age(Const(0)),
        Instruction::Jump(2),
        choose(
            Executor,
            Choice::Value(ValueChoices::Among(&Selector::Score(Either))),
        ),
        draw_age(Expr::Chosen),
    ]),
    dogmas: &[(
        false,
        &[
            foreshadow(HAND),
            if_(DID, 7),
            draw_age(Value(Reg::Last)).keep(DRAWN),
            reveal(LAST),
            meld(filtered(&DRAWN_IN_HAND, &[Has(Test::AboveTop(Executor))])),
            if_(&Not(DID), 3),
            return_(DRAWN_IN_HAND),
            choose(Executor, Choice::Branch(&[(0, "Repeat")])).optional(),
            REPEAT,
        ],
    )],
};

// If Pressure Cooker was foreseen, meld all cards from your hand. / Return all
// cards from your hand. For each top card of different color on your board
// with a bonus, both draw a card and junk an available achievement of value
// equal to that bonus.
const PRESSURE_COOKER: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                if_(&Foreseen, 1),
                meld(HAND).all().shortcut(Shortcut::ByColor),
            ],
        ),
        (
            false,
            &[
                return_(HAND).all().shortcut(ORIGIN_ALL),
                // Reselected live, one top per color, in the player's order:
                // a draw can reveal what the next junk chooses among.
                for_each(
                    Domain::Live {
                        from: &BONUS_TOPS,
                        per_color: true,
                        stop: Stop::Never,
                    },
                    Order::Chosen(Executor),
                    Some(ITEM),
                    2,
                ),
                draw_age(ITEM_BONUS),
                junk(filtered(
                    &AvailableAchievements,
                    &[Has(ValueIs(Cmp::Equal, ITEM_BONUS))],
                )),
            ],
        ),
    ],
};

// Draw and score a card of value equal to a bonus that occurs more than once
// on your board, if there is one. / You may splay your purple cards right. /
// If you have at least six bonuses on your board, claim the Wealth
// achievement.
const PALAMPORE: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                choose(
                    Executor,
                    Choice::Value(ValueChoices::Bonuses(&Bonuses {
                        on: Executor,
                        only: Repeated,
                    })),
                ),
                draw_age(Expr::Chosen),
                score(LAST),
            ],
        ),
        (false, &[splay(&[Purple], Splay::Right)]),
        (
            false,
            &[
                if_(
                    &Compare(bonus_count(&MY_BONUSES), Cmp::AtLeast, Const(6)),
                    1,
                ),
                claim(Special::Wealth),
            ],
        ),
    ],
};

// For each even bonus on your board, draw a card of that value. If Crossword
// was foreseen, transfer the drawn cards to the available achievements. / For
// each odd bonus on your board, return the lowest card from your hand.
const CROSSWORD: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                for_each(
                    Domain::Bonuses(
                        &Bonuses {
                            on: Executor,
                            only: Even,
                        },
                        Multiplicity::Every,
                    ),
                    Order::Chosen(Executor),
                    Some(ITEM),
                    1,
                ),
                // A skipped age still drew that card, and it is kept.
                draw_age(Expr::Slot(ITEM)).add(DRAWN),
                if_(&Foreseen, 1),
                transfer(DRAWN_IN_HAND, Destination::Supply)
                    .all()
                    .shortcut(Shortcut::AnyOrder),
            ],
        ),
        (
            false,
            &[return_(filtered(&HAND, &[Filter::Lowest]))
                .exactly(bonus_count(&Bonuses {
                    on: Executor,
                    only: Odd,
                }))
                .shortcut(ORIGIN)],
        ),
    ],
};

// Echo: Tuck a top card from your board. / If Wristwatch was foreseen, return
// all non-bottom cards from your board. / For each value in ascending order,
// if you have a bonus on your board of that value, draw and meld a card of
// that value.
const WRISTWATCH: Programs = Programs {
    echo: Some(&[tuck(TOPS)]),
    dogmas: &[
        (
            false,
            &[
                if_(&Foreseen, 1),
                return_(Board(
                    Executor,
                    ColorSet::Only(&Color::ALL),
                    BoardRegion::AboveBottom,
                ))
                .all()
                .shortcut(ORIGIN_ALL),
            ],
        ),
        (
            false,
            &[
                for_each(Domain::Range(1, 11), Order::Ascending, Some(ITEM), 3),
                // Tested live: an earlier meld can add or cover a bonus.
                if_(
                    &Compare(
                        bonus_count(&Bonuses {
                            on: Executor,
                            only: Equal(Expr::Slot(ITEM)),
                        }),
                        Cmp::AtLeast,
                        Const(1),
                    ),
                    2,
                ),
                draw_age(Expr::Slot(ITEM)),
                meld(LAST),
            ],
        ),
    ],
};

// I DEMAND you return a card of matching value from your score pile for each
// bonus on your board! / You may splay any one color of your cards up. If
// Barcode was foreseen, splay all your colors up.
const BARCODE: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[return_(SCORE)
                .per_value(&PerValue {
                    source: ValueSource::Bonuses(MY_BONUSES),
                    multiplicity: Multiplicity::Every,
                })
                .shortcut(ORIGIN)],
        ),
        (
            false,
            &[
                splay(&Color::ALL, Splay::Up),
                if_(&Foreseen, 1),
                Instruction::Splay(SplayStep {
                    who: Executor,
                    choices: SplayChoices::Fixed {
                        colors: ColorSet::Only(&Color::ALL),
                        direction: Splay::Up,
                        from: None,
                    },
                    targets: SplayTargets::Changes,
                    optional: false,
                    all: true,
                }),
            ],
        ),
    ],
};

// Claim one available standard achievement, if eligible, doubling your
// current score for the purpose of checking eligibility. If you do, and
// Artificial Heart was foreseen, repeat this effect.
const ARTIFICIAL_HEART: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            if_(&Foreseen, 2),
            achieve(filtered(
                &StandardAchievements,
                &[Has(Eligible { score_factor: 2 })],
            ))
            .shortcut(Shortcut::AnyOrder),
            Instruction::Jump(1),
            achieve(filtered(
                &StandardAchievements,
                &[Has(Eligible { score_factor: 2 })],
            )),
            if_(&All(&[Did(Outcome::All), Foreseen]), 1),
            REPEAT,
        ],
    )],
};

// Echo: You may tuck any number of cards from your hand. / Draw and meld a
// card of any value. If you have exactly nine different bonus values on your
// board, you win. Otherwise, self-execute the melded card.
const SUDOKU: Programs = Programs {
    echo: Some(&[tuck(HAND)
        .up_to(None)
        .optional()
        .shortcut(Shortcut::ByColor)]),
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Value(ValueChoices::Any)),
            // The drawn card: a replaced meld still executes it.
            draw_age(Expr::Chosen).keep(DRAWN),
            meld(LAST),
            if_(
                &Compare(
                    Expr::Bonuses(&MY_BONUSES, Tally::Distinct),
                    Cmp::Equal,
                    Const(9),
                ),
                1,
            ),
            WIN,
            bind(Selector::Reg(Reg::Slot(DRAWN))),
            SELF_EXECUTE,
        ],
    )],
};

// Echo: Score your bottom yellow card. / You may choose up to two cards
// from your hand. For each card chosen, either meld it or score it. / If
// Paper is a top card on any player's board, score it.
const SCISSORS: Programs = Programs {
    echo: Some(&[score(Board(
        Executor,
        ColorSet::Only(&[Yellow]),
        BoardRegion::Bottom,
    ))]),
    dogmas: &[
        (
            false,
            &[
                choose(
                    Executor,
                    Choice::Cards {
                        from: &HAND,
                        up_to: 2,
                        constraint: None,
                    },
                )
                .optional()
                .keep(MOVED),
                for_each(
                    Domain::Cards(&Selector::Slot(MOVED)),
                    Order::Chosen(Executor),
                    Some(ITEM),
                    4,
                ),
                choose(
                    Executor,
                    Choice::Branch(&[(0, "Meld the chosen card"), (2, "Score the chosen card")]),
                ),
                meld(ITEM_IN_HAND),
                Instruction::Jump(1),
                score(ITEM_IN_HAND),
            ],
        ),
        (
            false,
            &[score(filtered(
                &Selector::Tops(Either),
                &[Has(Test::Code("B031"))],
            ))],
        ),
    ],
};

// Echo: Draw a [4] then return a card from your hand. / You may return
// exactly three cards of equal value from your hand. If you do, draw a
// card of value two higher than the cards you return. / You may splay
// your yellow or blue cards left.
const MAGNIFYING_GLASS: Programs = Programs {
    echo: Some(&[draw(4), return_(HAND)]),
    dogmas: &[
        (
            false,
            &[
                return_(HAND)
                    .group(3, GroupConstraint::SameValue)
                    .optional()
                    .shortcut(ORIGIN_ALL)
                    .keep(MOVED),
                if_(&Did(Outcome::All), 1),
                draw_age(Add(&Max(&Selector::Slot(MOVED)), 2)),
            ],
        ),
        (false, &[splay(&[Yellow, Blue], Splay::Left)]),
    ],
};

// Echo: Score your lowest top card. / You may return exactly two cards
// of different value from your score pile. If you do, draw and tuck three
// [6]. / If you have at least five images on your board in one color,
// claim the Heritage achievement.
const LOOM: Programs = Programs {
    echo: Some(&[score(filtered(&TOPS, &[Filter::Lowest]))]),
    dogmas: &[
        (
            false,
            &[
                return_(SCORE)
                    .group(2, GroupConstraint::DifferentValues)
                    .optional()
                    .shortcut(ORIGIN_ALL),
                if_(&Did(Outcome::All), 3),
                for_each(Domain::Times(Const(3)), Order::Printed, None, 2),
                draw(6),
                tuck(LAST),
            ],
        ),
        (
            false,
            &[
                for_each(
                    Domain::Colors(ColorSet::Only(&Color::ALL)),
                    Order::Printed,
                    Some(ITEM),
                    2,
                ),
                if_(
                    &Compare(
                        Expr::Visible(&Visibility {
                            who: Who::Executor,
                            colors: ColorSet::Slot(ITEM),
                            seen: Seen::Images,
                        }),
                        Cmp::AtLeast,
                        Const(5),
                    ),
                    1,
                ),
                claim(Special::Heritage),
            ],
        ),
    ],
};

// Echo: Return all cards from your forecast. / Draw a [3]. / You may splay
// your purple cards left. / If all your non-purple top cards share a common
// icon other than [Prosperity], claim the Supremacy achievement.
const NOVEL: Programs = Programs {
    echo: Some(&[return_(FORECAST).all().shortcut(ORIGIN_ALL)]),
    dogmas: &[
        (false, &[draw(3)]),
        (false, &[splay(&[Purple], Splay::Left)]),
        (
            false,
            &[
                if_(
                    &Cond::CommonIcon {
                        from: &filtered(&TOPS, &[Lacks(Test::Color(Purple))]),
                        except: Icon::Prosperity,
                    },
                    1,
                ),
                claim(Special::Supremacy),
            ],
        ),
    ],
};

// Echo: Draw an [11]. / You may draw and score a card of any value. Transfer
// your bottom red card to your hand. If the values of all the cards in your
// hand match the values of all the cards in your score pile exactly, you win.
const HUMAN_GENOME: Programs = Programs {
    echo: Some(&[draw(11)]),
    dogmas: &[(
        false,
        &[
            choose(
                Executor,
                Choice::Branch(&[(0, "Draw and score"), (3, "Skip")]),
            ),
            choose(Executor, Choice::Value(ValueChoices::Any)),
            draw_age(Expr::Chosen),
            score(LAST),
            transfer(
                Board(Executor, ColorSet::Only(&[Red]), BoardRegion::Bottom),
                Destination::Hand(Executor),
            ),
            if_(&Cond::EqualValues(&HAND, &SCORE), 1),
            WIN,
        ],
    )],
};

const SHOWING_CHOSEN: Selector = filtered(
    &Board(
        Executor,
        ColorSet::Only(&Color::ALL),
        BoardRegion::BelowTop(0),
    ),
    &[Has(Test::ShowsChosenIcon)],
);

// I DEMAND you choose the standard icon of which I have the most among my
// icon types! Transfer all cards with that visible icon from your board to
// mine, or if Meritocracy was foreseen, to my achievements!
const MERITOCRACY: Programs = Programs {
    echo: None,
    dogmas: &[(
        true,
        &[
            choose(Executor, Choice::MostIcons(Demander)),
            if_(&Foreseen, 2),
            // Removals can only expose slots: the last single card is unsplayed
            // but also becomes fully visible. Thus the removal closure is unique.
            transfer(SHOWING_CHOSEN, Destination::Achievements(Demander))
                .all()
                .shortcut(Shortcut::AnyOrder),
            Instruction::Jump(1),
            // Cross-color arrivals commute; same-color order remains a choice.
            // Both batches reselect live, including newly exposed matches.
            transfer(SHOWING_CHOSEN, Destination::Board(Demander))
                .all()
                .shortcut(Shortcut::ByColor),
        ],
    )],
};

// Choose an opponent. That player chooses a card (unrevealed) in your hand.
// Meld the chosen card. If you do, and it is your turn, self-execute the card,
// then repeat this effect.
const ROBOCAR: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            choose(Opponent, Choice::Card(&HAND)),
            meld(Selector::Reg(Reg::Chosen)),
            if_(&All(&[Did(Outcome::All), Cond::OwnTurn]), 3),
            bind(Selector::Reg(Reg::Last)),
            SELF_EXECUTE,
            REPEAT,
        ],
    )],
};

/// Deepfake's chosen top card, still a top card.
const DEEPFAKE_CHOSEN: Selector = filtered(&Selector::Tops(Either), &[Has(Is(Reg::Slot(ITEM)))]);
/// One when the chosen card was already a top card on your board.
const DEEPFAKE_OWN: Slot = Slot::effect(2);
// If it is your turn, transfer a top card from any board to your board, then
// super-execute a top card on your board other than Deepfake. If the transferred card
// is still a top card, transfer it to its original board. A card chosen from your own
// board is still the transferred card: if it moved away and is still a top card, it
// comes back.
const DEEPFAKE: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            if_(&Cond::OwnTurn, 8),
            choose(Executor, Choice::Card(&Selector::Tops(Either))).keep(ITEM),
            Instruction::Bind(Binding::Value(
                DEEPFAKE_OWN,
                Count(&filtered(&TOPS, &[Has(Is(Reg::Slot(ITEM)))])),
            )),
            transfer(DEEPFAKE_CHOSEN, Destination::Board(Executor)).keep(MOVED),
            bind(filtered(&TOPS, &[Lacks(Test::Code("E114"))])),
            execute(ExecutionMode::SuperExecute, Executor),
            transfer(
                filtered(&Selector::Tops(Either), &[Has(InSlot(MOVED))]),
                Destination::Origin(MOVED),
            ),
            if_(&Compare(Expr::Slot(DEEPFAKE_OWN), Cmp::Equal, Const(1)), 1),
            transfer(DEEPFAKE_CHOSEN, Destination::Board(Executor)),
        ],
    )],
};

// If you have five top cards, draw and score a [7]. / Transfer a bonus
// top of each color, then junk four available achievements and draw [7]. /
// Return the initial top of every non-red color.
const MACHINE_GUN: Programs = Programs {
    echo: Some(&[
        if_(&Compare(Count(&TOPS), Cmp::Equal, Const(5)), 1),
        draws(Const(1), 7, Then::Score),
    ]),
    dogmas: &[
        (
            true,
            &[
                transfer(
                    filtered(&TOPS, &[Has(Test::Bonus)]),
                    Destination::Score(Demander),
                )
                .per_color(None)
                .shortcut(Shortcut::AnyOrder),
                if_(DID_ANY, 2),
                Instruction::Special(SpecialStep::JunkAvailable {
                    count: &Const(4),
                    shortcut: Some(Shortcut::AnyOrder),
                }),
                draw(7),
            ],
        ),
        (
            false,
            &[return_(filtered(&TOPS, &[Lacks(Test::Color(Red))]))
                .per_color(None)
                .shortcut(ORIGIN_ALL)],
        ),
    ],
};

// Meld a Health card, or reveal why no such hand card exists. / Both
// Efficiency returns must happen before junking the achievement quota.
const BANDAGE: Programs = Programs {
    echo: Some(&[meld(filtered(&HAND, &[Has(HasIcon(Health))])).reveal_if_empty(&HAND)]),
    dogmas: &[(
        true,
        &[
            reveal_all(filtered(&SCORE, &[Lacks(Test::Inspectable(Executor))])).keep(PROOF),
            return_(filtered(&VERIFIED_SCORE, &[Has(HasIcon(Efficiency))]))
                .verify()
                .reveal_if_empty(&SCORE)
                .keep(MOVED),
            return_(filtered(&TOPS, &[Has(HasIcon(Efficiency))])),
            if_(&All(&[Not(&Empty(MOVED)), Did(Outcome::All)]), 1),
            Instruction::Special(SpecialStep::JunkAvailable {
                count: &Expr::Metric(Executor, Metric::Achievements),
                shortcut: Some(Shortcut::AnyOrder),
            }),
        ],
    )],
};

// Reveal the hand, then transfer every card lacking Efficiency; a card
// that arrived unseen meanwhile is shown after. Foreseen: junk every board
// card.
const PARACHUTE: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[
                reveal_all(HAND).keep(PROOF),
                transfer(
                    filtered(&HAND, &[Lacks(HasIcon(Efficiency))]),
                    Destination::Hand(Demander),
                )
                .all()
                .shortcut(Shortcut::AnyOrder),
                reveal_all(filtered(&HAND, &[Lacks(InSlot(PROOF))])),
            ],
        ),
        (
            false,
            &[
                if_(&Foreseen, 1),
                junk(Board(
                    Either,
                    ColorSet::Only(&Color::ALL),
                    BoardRegion::BelowTop(0),
                ))
                .all()
                .shortcut(Shortcut::AnyOrder),
            ],
        ),
    ],
};

// Draw, then reveal the actual card. Return the live matching board/score
// set, proving each hidden return and the final remainder, including cards
// introduced by a reaction after the initial score reveal.
const DIGITAL_PET: Programs = Programs {
    echo: None,
    dogmas: &[(
        true,
        &[
            draw(11).keep(DRAWN),
            reveal(LAST),
            reveal_all(SCORE).keep(PROOF),
            return_(filtered(
                &Selector::Union(&[
                    VERIFIED_SCORE,
                    Board(
                        Executor,
                        ColorSet::Only(&Color::ALL),
                        BoardRegion::BelowTop(0),
                    ),
                ]),
                &[Has(ColorOf(Reg::Slot(DRAWN)))],
            ))
            .all()
            .verify()
            .shortcut(ORIGIN_ALL),
            reveal_all(SCORE),
        ],
    )],
};

// Choose any icon type. The unique leader is determined once, before
// revealing the affected piles and before any transfer reaction. Each branch
// has a fixed destination and a frozen, explicitly revealed packet.
const ALGOCRACY: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            choose(
                Executor,
                Choice::IconTypes {
                    merge_tied_leaders: true,
                },
            ),
            if_(
                &Compare(
                    Expr::Metric(Executor, Metric::ChosenIcon),
                    Cmp::Greater,
                    Expr::Metric(Opponent, Metric::ChosenIcon),
                ),
                3,
            ),
            reveal_all(Selector::Union(&[
                Selector::Hand(Opponent),
                Selector::Score(Either),
            ]))
            .keep(MOVED),
            transfer(
                filtered(
                    &Selector::Union(&[Selector::Hand(Either), Selector::Score(Either)]),
                    &[Has(InSlot(MOVED)), Has(Test::FeaturedChosenIcon)],
                ),
                Destination::Hand(Executor),
            )
            .all()
            .shortcut(Shortcut::AnyOrder),
            Instruction::Jump(3),
            if_(
                &Compare(
                    Expr::Metric(Opponent, Metric::ChosenIcon),
                    Cmp::Greater,
                    Expr::Metric(Executor, Metric::ChosenIcon),
                ),
                2,
            ),
            reveal_all(Selector::Union(&[HAND, Selector::Score(Either)])).keep(MOVED),
            transfer(
                filtered(
                    &Selector::Union(&[Selector::Hand(Either), Selector::Score(Either)]),
                    &[Has(InSlot(MOVED)), Has(Test::FeaturedChosenIcon)],
                ),
                Destination::Hand(Opponent),
            )
            .all()
            .shortcut(Shortcut::AnyOrder),
        ],
    )],
};

// Echo: You may splay one color of your cards left. / I DEMAND you return
// an expansion card from your hand! / Draw and reveal an Echoes [1]. If it
// has a bonus, draw a [1].
const FLUTE: Programs = Programs {
    echo: Some(&[splay(&Color::ALL, Splay::Left)]),
    dogmas: &[
        (
            true,
            &[return_(filtered(&HAND, &[Lacks(Test::Set(CardSet::Base))]))],
        ),
        (
            false,
            &[
                draw(1).set(CardSet::Echoes),
                reveal(LAST),
                if_(&Card(Reg::Last, Has(Test::Bonus)), 1),
                draw(1),
            ],
        ),
    ],
};

// I DEMAND you transfer an expansion card from your hand to my score pile!
// / Transfer a card from your hand to any opponent's board.
const CROSSBOW: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[transfer(
                filtered(&HAND, &[Lacks(Test::Set(CardSet::Base))]),
                Destination::Score(Demander),
            )],
        ),
        (false, &[transfer(HAND, Destination::Board(Opponent))]),
    ],
};

// Draw a card of value equal to a bonus on your board, if you have one. /
// Tuck a card from your hand. If Watermill was foreseen, tuck all cards from
// the deck of value equal to the tucked card.
const WATERMILL: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                choose(Executor, Choice::Value(ValueChoices::Bonuses(&MY_BONUSES))),
                draw_age(Expr::Chosen),
            ],
        ),
        (
            false,
            &[
                tuck(HAND).keep(MOVED),
                if_(&Foreseen, 1),
                // The specified Base deck is swept in physical top-down order,
                // without lookahead, ordinary draw routing, or an order menu.
                tuck(Selector::BaseDeckTop(Value(Reg::Slot(MOVED)))).all(),
            ],
        ),
    ],
};

/// Dentures' remembered color, evaluated from the last original tuck this action.
const DENTURES_PILE: Selector = Board(
    Executor,
    ColorSet::Of(Reg::Slot(ECHOED)),
    BoardRegion::BelowTop(0),
);
// Echo: Draw and tuck a [6]. / Score your top two non-bottom cards of the
// color of the last card you tucked due to Dentures. If there are none to
// score, draw and tuck a [6], then repeat this effect. / You may splay your
// blue cards right.
const DENTURES: Programs = Programs {
    echo: Some(&[draw(6), tuck(LAST).add(ECHOED)]),
    dogmas: &[
        (
            false,
            &[
                if_(&Compare(Count(&DENTURES_PILE), Cmp::AtMost, Const(1)), 3),
                draw(6),
                tuck(LAST).add(ECHOED),
                REPEAT,
                for_each(Domain::Times(Const(2)), Order::Printed, None, 2),
                if_(&Compare(Count(&DENTURES_PILE), Cmp::AtLeast, Const(2)), 1),
                score(Board(
                    Executor,
                    ColorSet::Of(Reg::Slot(ECHOED)),
                    BoardRegion::Top,
                )),
            ],
        ),
        (false, &[splay(&[Blue], Splay::Right)]),
    ],
};

// I DEMAND you transfer two top cards with [AUTHORITY] from your board to
// my score pile! If you transfer exactly one and Katana was foreseen, junk
// all available standard achievements!
const KATANA: Programs = Programs {
    echo: None,
    dogmas: &[(
        true,
        &[
            transfer(
                filtered(&TOPS, &[Has(HasIcon(Icon::Authority))]),
                Destination::Score(Demander),
            )
            .exactly(Const(2))
            .shortcut(ORIGIN_ALL),
            if_(&All(&[Did(Outcome::Exactly(1)), Foreseen]), 1),
            junk(Selector::StandardAchievements)
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    )],
};

// Echo: Draw and meld a [3]. / If you have a top card with [AUTHORITY],
// draw and meld a [3]. Otherwise, draw a [4]. / If you have a top card
// with [INDUSTRY], junk all cards in the [4] deck.
const DEODORANT: Programs = Programs {
    echo: Some(&[draw(3), meld(LAST)]),
    dogmas: &[
        (
            false,
            &[
                if_(
                    &Compare(
                        Count(&filtered(&TOPS, &[Has(HasIcon(Icon::Authority))])),
                        Cmp::Greater,
                        Const(0),
                    ),
                    3,
                ),
                draw(3),
                meld(LAST),
                Instruction::Jump(1),
                draw(4),
            ],
        ),
        (
            false,
            &[
                if_(
                    &Compare(
                        Count(&filtered(&TOPS, &[Has(HasIcon(Icon::Industry))])),
                        Cmp::Greater,
                        Const(0),
                    ),
                    1,
                ),
                deck_junk(4),
            ],
        ),
    ],
};

// You may return any number of cards from your hand. Draw a [3] for each
// card you return. / Meld a card from your hand. / If Sandpaper was
// foreseen, foreshadow all cards in your hand.
const SANDPAPER: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                return_(HAND).up_to(None).optional().shortcut(ORIGIN),
                draws(Expr::Done, 3, Then::Keep),
            ],
        ),
        (false, &[meld(HAND)]),
        (
            false,
            &[
                if_(&Foreseen, 1),
                foreshadow(HAND).all().shortcut(ORIGIN_ALL),
            ],
        ),
    ],
};

// I DEMAND you return two cards from your score pile whose values each
// match a card in my hand! / You may splay your red or green cards left.
// If Homing Pigeons was foreseen, splay all your colors left.
const HOMING_PIGEONS: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[return_(filtered(
                &SCORE,
                &[Has(Test::ValueIn(&Selector::Hand(Demander)))],
            ))
            .exactly(Const(2))
            .shortcut(ORIGIN_ALL)],
        ),
        (
            false,
            &[
                splay(&[Red, Green], Splay::Left),
                if_(&Foreseen, 2),
                for_each(
                    Domain::Colors(ColorSet::Only(&Color::ALL)),
                    Order::Chosen(Executor),
                    Some(ITEM),
                    1,
                )
                .shortcut(Shortcut::AnyOrder),
                splay_set(ColorSet::Slot(ITEM), Splay::Left),
            ],
        ),
    ],
};

// Echo: Draw and foreshadow an Echoes [4]. / You may return a card from
// your forecast with a bonus. If you do, draw and score a card of value
// one higher than that bonus. / If Almanac was foreseen, foreshadow all
// cards in another player's forecast.
const ALMANAC: Programs = Programs {
    echo: Some(&[draw(4).set(CardSet::Echoes), foreshadow(LAST)]),
    dogmas: &[
        (
            false,
            &[
                return_(filtered(&FORECAST, &[Has(Test::Bonus)]))
                    .optional()
                    .verify()
                    .keep(MOVED),
                if_(DID, 2),
                draw_age(Add(&Expr::BonusOf(Reg::Slot(MOVED)), 1)),
                score(LAST),
            ],
        ),
        (
            false,
            &[
                if_(&Foreseen, 1),
                foreshadow(Forecast(Opponent)).all().shortcut(ORIGIN_ALL),
            ],
        ),
    ],
};

const SPLAYED_TOPS: Selector = filtered(&TOPS, &[Has(Test::StackSplayed(None))]);
const SPLAYED_HAND: Selector = filtered(
    &Selector::Union(&[filtered(&HAND, &[Has(InSlot(PROOF))]), HAND]),
    &[Has(ColorIn(ColorSet::OfCards(&filtered(
        &TOPS,
        &[Has(Test::StackSplayed(None))],
    ))))],
);

// Echo: Score a card from your hand of a color you have splayed. / You
// may either splay your purple cards in the direction one of your other
// colors is splayed, or you may splay one of your other colors in the
// direction that your purple cards are splayed.
const SUNGLASSES_TO_PURPLE: SplayOption = SplayOption {
    colors: ColorSet::Only(&[Purple]),
    direction: SplayDirection::Copy {
        who: Executor,
        colors: ColorSet::Only(&[Red, Yellow, Green, Blue]),
        splayed_only: true,
    },
    from: None,
};
const SUNGLASSES_FROM_PURPLE: SplayOption = SplayOption {
    colors: ColorSet::Only(&[Red, Yellow, Green, Blue]),
    direction: SplayDirection::Copy {
        who: Executor,
        colors: ColorSet::Only(&[Purple]),
        splayed_only: true,
    },
    from: None,
};
const SUNGLASSES: Programs = Programs {
    echo: Some(&[
        // With no splayed color, the board alone proves nothing can score.
        if_(&Compare(Count(&SPLAYED_TOPS), Cmp::AtLeast, Const(1)), 2),
        reveal_all(filtered(&HAND, &[Lacks(Test::Inspectable(Executor))])).keep(PROOF),
        score(SPLAYED_HAND).verify().reveal_if_empty(&HAND),
    ]),
    dogmas: &[(
        false,
        &[
            if_(&Not(&Cond::Must), 2),
            Instruction::Splay(SplayStep {
                who: Executor,
                choices: SplayChoices::Options(&[SUNGLASSES_TO_PURPLE, SUNGLASSES_FROM_PURPLE]),
                targets: SplayTargets::Changes,
                optional: true,
                all: false,
            }),
            Instruction::Jump(4),
            // Under Blackmail's `must` the alternative itself is chosen, and one that
            // splays nothing is a legal choice.
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Splay purple like another color"),
                    (2, "Splay another color like purple"),
                ]),
            ),
            Instruction::Splay(SplayStep {
                who: Executor,
                choices: SplayChoices::Options(&[SUNGLASSES_TO_PURPLE]),
                targets: SplayTargets::Changes,
                optional: false,
                all: false,
            }),
            Instruction::Jump(1),
            Instruction::Splay(SplayStep {
                who: Executor,
                choices: SplayChoices::Options(&[SUNGLASSES_FROM_PURPLE]),
                targets: SplayTargets::Changes,
                optional: false,
                all: false,
            }),
        ],
    )],
};

// Echo: Splay left one color on any player's board. / I DEMAND you return
// a top card with Authority of each color! Draw and tuck a [4]! /
// Draw and tuck a [4]. / If Kobukson was foreseen, draw and meld a [5].
const KOBUKSON: Programs = Programs {
    echo: Some(&[Instruction::Splay(SplayStep {
        who: Either,
        choices: SplayChoices::Fixed {
            colors: ColorSet::Only(&Color::ALL),
            direction: Splay::Left,
            from: None,
        },
        targets: SplayTargets::Nonempty,
        optional: false,
        all: false,
    })]),
    dogmas: &[
        (
            true,
            &[
                return_(filtered(&TOPS, &[Has(HasIcon(Icon::Authority))]))
                    .per_color(None)
                    .shortcut(ORIGIN_ALL),
                draw(4),
                tuck(LAST),
            ],
        ),
        (false, &[draw(4), tuck(LAST)]),
        (false, &[if_(&Foreseen, 2), draw(5), meld(LAST)]),
    ],
};

// I DEMAND you transfer two non-red top cards with Authority or Avatar of
// different colors to my board! If you do, and Shuriken was foreseen,
// transfer them to my achievements! / You may splay your purple cards right.
const SHURIKEN: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[
                transfer(
                    Selector::Union(&[
                        filtered(
                            &TOPS,
                            &[Lacks(Test::Color(Red)), Has(HasIcon(Icon::Authority))],
                        ),
                        filtered(
                            &TOPS,
                            &[Lacks(Test::Color(Red)), Has(HasIcon(Icon::Avatar))],
                        ),
                    ]),
                    Destination::Board(Demander),
                )
                .per_color(Some(Const(2)))
                .shortcut(Shortcut::ByColor)
                .keep(MOVED),
                if_(
                    &All(&[
                        Did(Outcome::All),
                        Foreseen,
                        Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(2)),
                    ]),
                    1,
                ),
                transfer(
                    filtered(
                        &Board(
                            Demander,
                            ColorSet::Only(&Color::ALL),
                            BoardRegion::BelowTop(0),
                        ),
                        &[Has(InSlot(MOVED))],
                    ),
                    Destination::Achievements(Demander),
                )
                .all()
                .shortcut(Shortcut::AnyOrder),
            ],
        ),
        (false, &[splay(&[Purple], Splay::Right)]),
    ],
};

const PENCIL_DRAWS: Slot = Slot::effect(2);
const PENCIL_PACKET: Selector = filtered(&HAND, &[Has(InSlot(PENCIL_DRAWS))]);
// Echo: Draw a [5]. / You may return up to three cards from your hand.
// If you do, draw that many cards one higher than the highest you return.
// Return all but one of them, and foreshadow the remaining card.
const PENCIL: Programs = Programs {
    echo: Some(&[draw(5)]),
    dogmas: &[(
        false,
        &[
            return_(HAND)
                .up_to(Some(Const(3)))
                .optional()
                .shortcut(ORIGIN)
                .keep(MOVED),
            if_(&Did(Outcome::All), 3),
            draws_age(
                Count(&Selector::Slot(MOVED)),
                Add(&Max(&Selector::Slot(MOVED)), 1),
                Then::Keep,
            )
            .keep(PENCIL_DRAWS),
            // Keep the survivor undecided: a return reaction may disclose
            // new information before the next return choice.
            return_(PENCIL_PACKET)
                .exactly(Add(&Count(&PENCIL_PACKET), -1))
                .shortcut(ORIGIN),
            foreshadow(PENCIL_PACKET),
        ],
    )],
};

// Draw a [4]. / If you have exactly one card in your hand, draw a [4],
// then draw and score a [4]. / If Chintz was foreseen, transfer your whole
// hand to the available achievements.
const CHINTZ: Programs = Programs {
    echo: None,
    dogmas: &[
        (false, &[draw(4)]),
        (
            false,
            &[
                if_(&Compare(Count(&HAND), Cmp::Equal, Const(1)), 3),
                draw(4),
                draw(4),
                score(LAST),
            ],
        ),
        (
            false,
            &[
                if_(&Foreseen, 1),
                transfer(HAND, Destination::Supply)
                    .all()
                    .shortcut(Shortcut::AnyOrder),
            ],
        ),
    ],
};

// You may splay your yellow cards right. / Draw a card of value equal to
// your lowest top card plus the number of colors you have splayed.
const SLIDE_RULE: Programs = Programs {
    echo: None,
    dogmas: &[
        (false, &[splay(&[Yellow], Splay::Right)]),
        (
            false,
            &[draw_age(Expr::Sum(&[
                Max(&filtered(&TOPS, &[Filter::Lowest])),
                Count(&filtered(&TOPS, &[Has(Test::StackSplayed(None))])),
            ]))],
        ),
    ],
};

const CLOCK_VALUE: Slot = Slot::effect(1);
/// The cards visible in the chosen colour on the demander's board: colours
/// with the same count are the same choice.
const CLOCK_VISIBLE: Expr = Expr::Visible(&Visibility {
    who: Demander,
    colors: ColorSet::Chosen,
    seen: Seen::Cards,
});
// Echo: You may splay your color with the most cards right. / I DEMAND
// you transfer every hand and score card matching the number of visible
// cards in a color of my choice on my board to my score pile! Junk an
// available achievement of that value!
const CLOCK: Programs = Programs {
    echo: Some(&[Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors: ColorSet::MostCards(Executor),
            direction: Splay::Right,
            from: None,
        },
        targets: SplayTargets::Changes,
        optional: true,
        all: false,
    })]),
    dogmas: &[(
        true,
        &[
            choose(Demander, Choice::Color(ColorSet::Only(&Color::ALL)))
                .alias(Alias::SameKey(&CLOCK_VISIBLE)),
            Instruction::Bind(Binding::Value(CLOCK_VALUE, CLOCK_VISIBLE)),
            transfer(
                filtered(
                    &Selector::Union(&[HAND, SCORE]),
                    &[Has(ValueIs(Cmp::Equal, Expr::Slot(CLOCK_VALUE)))],
                ),
                Destination::Score(Demander),
            )
            .all()
            .shortcut(ORIGIN_ALL),
            junk(filtered(
                &AvailableAchievements,
                &[Has(ValueIs(Cmp::Equal, Expr::Slot(CLOCK_VALUE)))],
            )),
        ],
    )],
};

const RETURNED: Selector = Selector::Slot(MOVED);
const RETURNED_BLUE: Selector = filtered(&RETURNED, &[Has(Test::Color(Blue))]);
const RETURNED_GREEN: Selector = filtered(&RETURNED, &[Has(Test::Color(Green))]);
const RETURNED_YELLOW: Selector = filtered(&RETURNED, &[Has(Test::Color(Yellow))]);
// Reveal only the first blue returned; failure is proven once, after the returns.
const BAROMETER_VERIFY: Cond = All(&[
    Compare(Count(&RETURNED_BLUE), Cmp::Equal, Const(0)),
    Card(Reg::Bound, Has(Test::Color(Blue))),
]);
// Echo: Transfer a [5] from your forecast to your hand. / Draw and
// foreshadow two higher than a bonus on any board, if any. / You may return
// all Forecast cards. If any are blue, claim Destiny.
const BAROMETER: Programs = Programs {
    echo: Some(&[transfer(
        filtered(&FORECAST, &[Has(BoxedValues(&[5]))]),
        Destination::Hand(Executor),
    )]),
    dogmas: &[
        (
            false,
            &[
                choose(
                    Executor,
                    Choice::Value(ValueChoices::Bonuses(&Bonuses {
                        on: Either,
                        only: BonusValues::All,
                    })),
                ),
                draw_age(Add(&Expr::Chosen, 2)),
                foreshadow(LAST),
            ],
        ),
        (
            false,
            &[
                return_(FORECAST)
                    .all()
                    .optional()
                    .shortcut(ORIGIN_ALL)
                    .verify_when(&BAROMETER_VERIFY)
                    .keep(MOVED),
                reveal_all(RETURNED).unless_proven(&Compare(
                    Count(&RETURNED_BLUE),
                    Cmp::Greater,
                    Const(0),
                )),
                if_(&Compare(Count(&RETURNED_BLUE), Cmp::Greater, Const(0)), 1),
                Instruction::Special(SpecialStep::Claim(Special::Destiny)),
            ],
        ),
    ],
};

const GLOBE_COLORS: Cond = All(&[
    Compare(Count(&RETURNED_BLUE), Cmp::Greater, Const(0)),
    Compare(Count(&RETURNED_GREEN), Cmp::Greater, Const(0)),
    Compare(Count(&RETURNED_YELLOW), Cmp::Greater, Const(0)),
]);
// Reveal only the first returned card of each required color; failure is proven once,
// after the returns.
const GLOBE_VERIFY: Cond = All(&[
    Card(
        Reg::Bound,
        Has(ColorIn(ColorSet::Only(&[Blue, Green, Yellow]))),
    ),
    Compare(
        Count(&filtered(&RETURNED, &[Has(ColorOf(Reg::Bound))])),
        Cmp::Equal,
        Const(0),
    ),
]);
// You may return your whole hand. If you return blue, green and yellow,
// draw and foreshadow a [6], [7], [8], then splay any color right. /
// If Globe was foreseen, foreshadow a top card from any board.
const GLOBE: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                return_(HAND)
                    .all()
                    .optional()
                    .shortcut(ORIGIN_ALL)
                    .verify_when(&GLOBE_VERIFY)
                    .keep(MOVED),
                // A First may prevent a predicted witness. Complete any
                // missing proof before the condition enters control flow.
                reveal_all(RETURNED).unless_proven(&GLOBE_COLORS),
                if_(&GLOBE_COLORS, 7),
                draw(6),
                foreshadow(LAST),
                draw(7),
                foreshadow(LAST),
                draw(8),
                foreshadow(LAST),
                splay_set(ColorSet::Only(&Color::ALL), Splay::Right),
            ],
        ),
        (
            false,
            &[if_(&Foreseen, 1), foreshadow(Selector::Tops(Either))],
        ),
    ],
};

// I DEMAND you draw and tuck a [6]! Transfer your top two cards of its
// color to my score pile! Score the bottom card of its color on my board!
const SHRAPNEL: Programs = Programs {
    echo: None,
    dogmas: &[(
        true,
        &[
            draw(6),
            tuck(LAST).keep(MOVED),
            transfer(
                Board(Executor, ColorSet::Of(Reg::Slot(MOVED)), BoardRegion::Top),
                Destination::Score(Demander),
            )
            .exactly(Const(2)),
            score(Board(
                Demander,
                ColorSet::Of(Reg::Slot(MOVED)),
                BoardRegion::Bottom,
            )),
        ],
    )],
};

// Echo: Draw and score a [7]. / You may achieve (if eligible) a top card
// from an opponent's board if they have an achievement of matching value.
// If you do, transfer your top green card to that player's board.
// Otherwise, draw and meld a [7].
const HOT_AIR_BALLOON: Programs = Programs {
    echo: Some(&[draw(7), score(LAST)]),
    dogmas: &[(
        false,
        &[
            achieve(filtered(
                &Selector::Tops(Opponent),
                &[
                    Has(Test::ValueIn(&Selector::Achievements(Opponent))),
                    Has(Eligible { score_factor: 1 }),
                ],
            ))
            .optional(),
            if_(DID, 2),
            transfer(TopOf(Executor, Green), Destination::Board(Opponent)),
            Instruction::Jump(2),
            draw(7),
            meld(LAST),
        ],
    )],
};

// I DEMAND you draw and reveal a [6]! If it is blue or yellow, transfer
// it and all cards in your hand to my hand! If it is red or green,
// transfer two cards from your score pile to mine!
const STEAMBOAT: Programs = Programs {
    echo: None,
    dogmas: &[(
        true,
        &[
            draw(6).keep(DRAWN),
            reveal(LAST),
            if_(
                &Card(
                    Reg::Slot(DRAWN),
                    Has(ColorIn(ColorSet::Only(&[Blue, Yellow]))),
                ),
                1,
            ),
            transfer(HAND, Destination::Hand(Demander))
                .all()
                .shortcut(ORIGIN_ALL),
            if_(
                &Card(
                    Reg::Slot(DRAWN),
                    Has(ColorIn(ColorSet::Only(&[Red, Green]))),
                ),
                1,
            ),
            transfer(SCORE, Destination::Score(Demander))
                .exactly(Const(2))
                .shortcut(ORIGIN_ALL),
        ],
    )],
};

const LOWEST_STANDARD: Selector = filtered(&StandardAchievements, &[Filter::Lowest]);

// Echo: Return a card from your forecast. / Draw and foreshadow a [7].
// If Bifocals was foreseen, draw and foreshadow a card of value equal to
// the lowest available standard achievement. / You may splay your green
// cards right. If you do, splay any color of your cards up.
const BIFOCALS: Programs = Programs {
    echo: Some(&[return_(FORECAST)]),
    dogmas: &[
        (
            false,
            &[
                draw(7),
                foreshadow(LAST),
                if_(&Foreseen, 2),
                draw_age(Max(&LOWEST_STANDARD)),
                foreshadow(LAST),
            ],
        ),
        (
            false,
            &[
                splay(&[Green], Splay::Right),
                if_(DID, 1),
                splay_set(ColorSet::Only(&Color::ALL), Splay::Up),
            ],
        ),
    ],
};

// I DEMAND you return two cards from your score pile! If Indian Clubs
// was foreseen, return all cards from your score pile! / For every value
// of card in your score pile, score a card from your hand of that value.
const INDIAN_CLUBS: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[
                return_(SCORE).exactly(Const(2)).shortcut(ORIGIN),
                if_(&Foreseen, 1),
                return_(SCORE).all().shortcut(ORIGIN_ALL),
            ],
        ),
        (
            false,
            &[score(HAND)
                .per_value(&PerValue {
                    source: ValueSource::Cards(&SCORE),
                    multiplicity: Multiplicity::Distinct,
                })
                .shortcut(ORIGIN)],
        ),
    ],
};

// Draw and meld a [7]. You may splay your cards of that color right. /
// Junk an available achievement of value equal to the number of [CONCEPT]
// on your board. If Kaleidoscope was foreseen, junk all available
// achievements of lower value.
const KALEIDOSCOPE: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                draw(7),
                meld(LAST).keep(MOVED),
                splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Right).optional(),
            ],
        ),
        (
            false,
            &[
                // All printed values are at most 11. Twelve preserves both
                // comparisons while keeping a large Figure icon count out of
                // the small value register. Capture before either junk clause.
                if_(
                    &Compare(
                        Expr::Metric(Executor, Metric::Icons(Icon::Concept)),
                        Cmp::Greater,
                        Const(11),
                    ),
                    2,
                ),
                Instruction::Bind(Binding::Value(ITEM, Const(12))),
                Instruction::Jump(1),
                Instruction::Bind(Binding::Value(
                    ITEM,
                    Expr::Metric(Executor, Metric::Icons(Icon::Concept)),
                )),
                choose(
                    Executor,
                    Choice::Card(&filtered(
                        &AvailableAchievements,
                        &[Has(ValueIs(Cmp::Equal, Expr::Slot(ITEM)))],
                    )),
                ),
                junk(filtered(&AvailableAchievements, &[Has(Is(Reg::Chosen))])),
                if_(&Foreseen, 1),
                junk(filtered(
                    &AvailableAchievements,
                    &[Has(ValueIs(Cmp::Less, Expr::Slot(ITEM)))],
                ))
                .all()
                .shortcut(Shortcut::AnyOrder),
            ],
        ),
    ],
};

// Echo: Draw and tuck two [8]. / Score a top card without a bonus. /
// You may splay red up. / If Rubber was foreseen, foreshadow a top card.
const RUBBER: Programs = Programs {
    echo: Some(&[
        for_each(Domain::Times(Const(2)), Order::Printed, None, 2),
        draw(8),
        tuck(LAST),
    ]),
    dogmas: &[
        (false, &[score(filtered(&TOPS, &[Lacks(Test::Bonus)]))]),
        (false, &[splay(&[Red], Splay::Up)]),
        (false, &[if_(&Foreseen, 1), foreshadow(TOPS)]),
    ],
};

const ELEVATOR_HAND: Selector = filtered(
    &Selector::Hand(Opponent),
    &[Has(ValueIs(Cmp::Equal, Expr::Slot(ITEM)))],
);
const ELEVATOR_SCORE: Selector = filtered(
    &Selector::Score(Opponent),
    &[Has(ValueIs(Cmp::Equal, Expr::Slot(ITEM)))],
);
// Echo: Score your top or bottom green card. / Choose a value in your
// score pile, score that value from opposing hands or opposing score piles,
// then draw and foreshadow a card of the chosen value.
const ELEVATOR: Programs = Programs {
    echo: Some(&[score(Selector::Union(&[
        Board(Executor, ColorSet::Only(&[Green]), BoardRegion::Top),
        Board(Executor, ColorSet::Only(&[Green]), BoardRegion::Bottom),
    ]))]),
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Value(ValueChoices::Among(&SCORE))).keep(ITEM),
            if_(&Not(&Empty(ITEM)), 6),
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Score from opposing hands"),
                    (2, "Score from opposing score piles"),
                ]),
            ),
            score(ELEVATOR_HAND).all().shortcut(Shortcut::AnyOrder),
            Instruction::Jump(1),
            score(ELEVATOR_SCORE).all().shortcut(Shortcut::AnyOrder),
            draw_age(Expr::Slot(ITEM)),
            foreshadow(LAST),
        ],
    )],
};

// You may return a card from your hand. If you do, transfer all cards of
// its value from all score piles to your hand. / Draw and foreshadow a
// card of value equal to the number of cards in your hand.
const FERTILIZER: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                return_(HAND).optional().keep(MOVED),
                if_(DID, 1),
                transfer(
                    filtered(
                        &Selector::Score(Either),
                        &[Has(ValueIs(Cmp::Equal, Value(Reg::Slot(MOVED))))],
                    ),
                    Destination::Hand(Executor),
                )
                .all()
                .shortcut(Shortcut::AnyOrder),
            ],
        ),
        (false, &[draw_age(Count(&HAND)), foreshadow(LAST)]),
    ],
};

const JEANS_SELECTED: Selector = filtered(&DRAWN_IN_HAND, &[Has(Is(Reg::Slot(ITEM)))]);
const JEANS_OTHER: Selector = filtered(&DRAWN_IN_HAND, &[Lacks(Is(Reg::Slot(ITEM)))]);
// Echo: Draw two [8]. Return one, foreshadow the other. / Draw and reveal
// two cards of your top blue card's value. Meld one and return the other. /
// Choose [7] or [8], junk that Base deck. / Foreseen: transfer a valued
// card from junk to your hand.
const JEANS: Programs = Programs {
    echo: Some(&[
        draw(8).keep(DRAWN),
        draw(8).add(DRAWN),
        choose(Executor, Choice::Card(&DRAWN_IN_HAND)).keep(ITEM),
        return_(JEANS_SELECTED),
        foreshadow(JEANS_OTHER),
    ]),
    dogmas: &[
        (
            false,
            &[
                Instruction::Bind(Binding::Value(
                    ITEM,
                    Max(&Board(Executor, ColorSet::Only(&[Blue]), BoardRegion::Top)),
                )),
                draw_age(Expr::Slot(ITEM)),
                reveal(LAST).keep(DRAWN),
                draw_age(Expr::Slot(ITEM)),
                reveal(LAST).add(DRAWN),
                choose(Executor, Choice::Card(&DRAWN_IN_HAND)).keep(ITEM),
                meld(JEANS_SELECTED),
                return_(JEANS_OTHER),
            ],
        ),
        (
            false,
            &[
                unless_decks_empty!(7, 8),
                choose(Executor, Choice::Value(ValueChoices::Boxed(&[7, 8]))),
                junk(BaseDeck(Expr::Chosen))
                    .all()
                    .shortcut(Shortcut::DeckTop),
            ],
        ),
        (
            false,
            &[
                if_(&Foreseen, 2),
                choose(Executor, Choice::Card(&Selector::Junk)),
                transfer(Selector::Reg(Reg::Chosen), Destination::Hand(Executor)),
            ],
        ),
    ],
};

// You may match one color's splay to the other player's. / You may splay
// blue up. / If Telegraph was foreseen, splay all your splayed colors up.
const TELEGRAPH: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[Instruction::Splay(SplayStep {
                who: Executor,
                choices: SplayChoices::Options(&[SplayOption {
                    colors: ColorSet::Only(&Color::ALL),
                    direction: SplayDirection::Matching(Opponent),
                    from: None,
                }]),
                targets: SplayTargets::AnyColor,
                optional: true,
                all: false,
            })],
        ),
        (false, &[splay(&[Blue], Splay::Up)]),
        (
            false,
            &[
                if_(&Foreseen, 2),
                for_each(
                    Domain::Colors(ColorSet::OfCards(&filtered(
                        &TOPS,
                        &[Has(Test::StackSplayed(None))],
                    ))),
                    Order::Chosen(Executor),
                    Some(ITEM),
                    1,
                )
                .shortcut(Shortcut::AnyOrder),
                splay_set(ColorSet::Slot(ITEM), Splay::Up),
            ],
        ),
    ],
};

macro_rules! three_echoes {
    ($color:expr) => {
        Compare(
            Expr::Visible(&Visibility {
                who: Executor,
                colors: ColorSet::Only(&[$color]),
                seen: Seen::Echoes,
            }),
            Cmp::AtLeast,
            Const(3),
        )
    };
}
// Echo: Meld a card from your forecast. / Demand: transfer your highest
// top card to your hand. / Three visible echoes in one color claim History;
// if that claim succeeds and Photography was foreseen, win.
const PHOTOGRAPHY: Programs = Programs {
    echo: Some(&[meld(FORECAST)]),
    dogmas: &[
        (
            true,
            &[transfer(
                filtered(&TOPS, &[Filter::Highest]),
                Destination::Hand(Executor),
            )],
        ),
        (
            false,
            &[
                if_(
                    &Cond::Any(&[
                        three_echoes!(Red),
                        three_echoes!(Yellow),
                        three_echoes!(Green),
                        three_echoes!(Blue),
                        three_echoes!(Purple),
                    ]),
                    3,
                ),
                claim(Special::History),
                if_(&All(&[Did(Outcome::All), Foreseen]), 1),
                WIN,
            ],
        ),
    ],
};

// Return your hand, draw a [6], then draw a card one higher than your
// live highest hand card for each distinct color successfully returned.
const TYPEWRITER: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            return_(HAND)
                .all()
                .verify()
                .shortcut(ORIGIN_ALL)
                .keep(MOVED),
            draw(6),
            for_each(
                Domain::Colors(ColorSet::OfCards(&Selector::Slot(MOVED))),
                Order::Printed,
                None,
                1,
            ),
            draw_age(Add(&Max(&HAND), 1)),
        ],
    )],
};

// Echo: Score a non-purple top card without a bonus. / Demand: draw and
// meld a [1]. / Choose [6], [7], [8] or [9], optionally junk its Base deck,
// and if you do, achieve the highest card in junk if eligible.
const ICE_CREAM: Programs = Programs {
    echo: Some(&[score(filtered(
        &TOPS,
        &[Lacks(Test::Color(Purple)), Lacks(Test::Bonus)],
    ))]),
    dogmas: &[
        (true, &[draw(1), meld(LAST)]),
        (
            false,
            junk_then_highest!(
                Expr::Chosen,
                choose(Executor, Choice::Value(ValueChoices::Boxed(&[6, 7, 8, 9])))
                    .alias(Alias::NoOpWhen(&EMPTY_CHOSEN_DECK)),
            ),
        ),
    ],
};

const SAXOPHONE_IMAGES: Selector = filtered(
    &Selector::Union(&[
        filtered(
            &Board(
                Either,
                ColorSet::Only(&Color::ALL),
                BoardRegion::BelowTop(0),
            ),
            &[Has(Test::Code("E013"))],
        ),
        filtered(
            &Board(
                Either,
                ColorSet::Only(&Color::ALL),
                BoardRegion::BelowTop(0),
            ),
            &[Has(Test::Code("E014"))],
        ),
        filtered(
            &Board(
                Either,
                ColorSet::Only(&Color::ALL),
                BoardRegion::BelowTop(0),
            ),
            &[Has(Test::Code("E054"))],
        ),
        filtered(
            &Board(
                Either,
                ColorSet::Only(&Color::ALL),
                BoardRegion::BelowTop(0),
            ),
            &[Has(Test::Code("E075"))],
        ),
    ]),
    &[Has(Test::Shows(Shown::Image))],
);
// You may splay purple up. / Win if all four marked instrument images
// show anywhere; otherwise draw a [7] for each of those images showing.
const SAXOPHONE: Programs = Programs {
    echo: None,
    dogmas: &[
        (false, &[splay(&[Purple], Splay::Up)]),
        (
            false,
            &[
                Instruction::Bind(Binding::Value(ITEM, Count(&SAXOPHONE_IMAGES))),
                if_(&Compare(Expr::Slot(ITEM), Cmp::Equal, Const(4)), 2),
                WIN,
                Instruction::Jump(1),
                draws(Expr::Slot(ITEM), 7, Then::Keep),
            ],
        ),
    ],
};

// Echo: You may score a card from your hand. / I DEMAND you return all
// cards from your score pile of value matching any of your top cards! /
// Junk all cards in the [9] deck.
const AIR_CONDITIONER: Programs = Programs {
    echo: Some(&[score(HAND).optional()]),
    dogmas: &[
        (
            true,
            &[return_(filtered(&SCORE, &[Has(Test::ValueIn(&TOPS))]))
                .all()
                .shortcut(ORIGIN_ALL)],
        ),
        (false, &[deck_junk(9)]),
    ],
};

// Echo: Draw a [7]. / Draw and score a [7]. / Draw a [7]. /
// If Tractor was foreseen, draw and score seven [7].
const TRACTOR: Programs = Programs {
    echo: Some(&[draw(7)]),
    dogmas: &[
        (false, &[draw(7), score(LAST)]),
        (false, &[draw(7)]),
        (false, &[if_(&Foreseen, 1), draws(Const(7), 7, Then::Score)]),
    ],
};

// Draw and tuck three [8]. If any of the tucked cards are green, repeat
// this effect. / You may splay your red cards up.
const NYLON: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            false,
            &[
                for_each(Domain::Times(Const(3)), Order::Printed, None, 2),
                draw(8),
                tuck(LAST).add(MOVED),
                if_(
                    &Compare(
                        Count(&filtered(
                            &Selector::Slot(MOVED),
                            &[Has(Test::Color(Green))],
                        )),
                        Cmp::AtLeast,
                        Const(1),
                    ),
                    1,
                ),
                REPEAT,
            ],
        ),
        (false, &[splay(&[Red], Splay::Up)]),
    ],
};

// Echo: Return all cards from your hand and draw two [8]. / Return a card
// from your score pile. Draw and score two cards of value one less than
// the value of the card you return.
const SLICED_BREAD: Programs = Programs {
    echo: Some(&[
        return_(HAND).all().shortcut(ORIGIN_ALL),
        draws(Const(2), 8, Then::Keep),
    ]),
    dogmas: &[(
        false,
        &[
            return_(SCORE).keep(MOVED),
            // No successful return still gives undefined-value zero.
            draws_age(Const(2), Add(&Max(&Selector::Slot(MOVED)), -1), Then::Score),
        ],
    )],
};

const RADIO_NINES: Slot = Slot::effect(1);
const RADIO_TENS: Slot = Slot::effect(2);
const RADIO_MELDED: Slot = Slot::effect(3);
// For every color on your board with Concept, draw a [9], and if Radio Telescope was
// foreseen, for every color also draw a [10]. Meld one of the drawn cards and return
// the rest. If you meld A.I. due to this effect, you win. Foreseen tens count every
// occupied color.
const RADIO_TELESCOPE: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            Instruction::Bind(Binding::Value(RADIO_NINES, colors_with!(Icon::Concept))),
            Instruction::Bind(Binding::Value(RADIO_TENS, Count(&TOPS))),
            draws(Expr::Slot(RADIO_NINES), 9, Then::Keep).keep(DRAWN),
            if_(&Foreseen, 1),
            draws(Expr::Slot(RADIO_TENS), 10, Then::Keep).add(DRAWN),
            meld(DRAWN_IN_HAND).keep(RADIO_MELDED),
            return_(filtered(&DRAWN_IN_HAND, &[Lacks(InSlot(RADIO_MELDED))]))
                .all()
                .shortcut(ORIGIN_ALL),
            if_(&Card(Reg::Slot(RADIO_MELDED), Has(Test::Code("B104"))), 1),
            WIN,
        ],
    )],
};

const TELEVISION_VALUE: Slot = Slot::effect(0);
const TELEVISION_SCORE: Selector = filtered(
    &Selector::Score(Opponent),
    &[Has(ValueIs(Cmp::Equal, Expr::Slot(TELEVISION_VALUE)))],
);
/// The opponent's score pile holds no card of the chosen value (backs show
/// values): nothing to transfer or achieve.
const NO_TELEVISION_SCORE: Cond = Compare(
    Count(&filtered(
        &Selector::Score(Opponent),
        &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
    )),
    Cmp::Equal,
    Const(0),
);
// Echo: Draw and meld an [8]. / Choose a value and an opponent. Transfer
// a card of that value from their score pile to their board. If they have
// an achievement of the same value, achieve (if eligible) a card of that
// value from their score pile.
const TELEVISION: Programs = Programs {
    echo: Some(&[draw(8), meld(LAST)]),
    dogmas: &[(
        false,
        &[
            choose(Executor, Choice::Value(ValueChoices::Any))
                .alias(Alias::NoOpWhen(&NO_TELEVISION_SCORE))
                .keep(TELEVISION_VALUE),
            choose(Executor, Choice::Card(&TELEVISION_SCORE)),
            transfer(
                filtered(&TELEVISION_SCORE, &[Has(Is(Reg::Chosen))]),
                Destination::Board(Opponent),
            ),
            if_(
                &Compare(
                    Count(&filtered(
                        &Selector::Achievements(Opponent),
                        &[Has(ValueIs(Cmp::Equal, Expr::Slot(TELEVISION_VALUE)))],
                    )),
                    Cmp::AtLeast,
                    Const(1),
                ),
                1,
            ),
            achieve(filtered(
                &TELEVISION_SCORE,
                &[Has(Eligible { score_factor: 1 })],
            )),
        ],
    )],
};

// I DEMAND you transfer all cards in your hand to my hand! Draw a [9]! /
// Meld all [9] from your hand. If Camcorder wasn't foreseen, return all
// other cards from your hand, and draw three [9].
const CAMCORDER: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[
                transfer(HAND, Destination::Hand(Demander))
                    .all()
                    .shortcut(ORIGIN_ALL),
                draw(9),
            ],
        ),
        (
            false,
            &[
                meld(filtered(&HAND, &[Has(BoxedValues(&[9]))]))
                    .all()
                    .shortcut(Shortcut::ByColor),
                if_(&Not(&Foreseen), 2),
                return_(HAND).all().shortcut(ORIGIN_ALL),
                draws(Const(3), 9, Then::Keep),
            ],
        ),
    ],
};

// Echo: Score a top non-red card from your board. / I DEMAND you choose
// a standard icon type! Transfer all top cards without that icon from
// your board to my score pile! / If you have fewer Industry, fewer
// Prosperity, and fewer Authority than each opponent, you win.
const SOCIAL_NETWORKING: Programs = Programs {
    echo: Some(&[score(filtered(&TOPS, &[Lacks(Test::Color(Red))]))]),
    dogmas: &[
        (
            true,
            &[
                choose(Executor, Choice::Icon { except: None }),
                transfer(
                    filtered(&TOPS, &[Lacks(Test::ChosenIcon)]),
                    Destination::Score(Demander),
                )
                .all()
                .shortcut(Shortcut::AnyOrder),
            ],
        ),
        (
            false,
            &[
                if_(
                    &All(&[
                        Compare(
                            Expr::Metric(Executor, Metric::Icons(Icon::Industry)),
                            Cmp::Less,
                            Expr::Metric(Opponent, Metric::Icons(Icon::Industry)),
                        ),
                        Compare(
                            Expr::Metric(Executor, Metric::Icons(Icon::Prosperity)),
                            Cmp::Less,
                            Expr::Metric(Opponent, Metric::Icons(Icon::Prosperity)),
                        ),
                        Compare(
                            Expr::Metric(Executor, Metric::Icons(Icon::Authority)),
                            Cmp::Less,
                            Expr::Metric(Opponent, Metric::Icons(Icon::Authority)),
                        ),
                    ]),
                    1,
                ),
                WIN,
            ],
        ),
    ],
};

// Echo: Draw and score a [10]. / Draw and score a card of value equal
// to a bonus on your board, if there is one. / Return any number of
// cards from your hand. For each card returned, claim two available
// standard achievements for which you are eligible.
const MP3_ELIGIBLE: Selector =
    filtered(&StandardAchievements, &[Has(Eligible { score_factor: 1 })]);
const MP3: Programs = Programs {
    echo: Some(&[draw(10), score(LAST)]),
    dogmas: &[
        (
            false,
            &[
                choose(Executor, Choice::Value(ValueChoices::Bonuses(&MY_BONUSES))),
                draw_age(Expr::Chosen),
                score(LAST),
            ],
        ),
        (
            false,
            &[
                return_(HAND).up_to(None).shortcut(ORIGIN),
                for_each(
                    Domain::Times(Expr::Mul(&Expr::Done, 2)),
                    Order::Printed,
                    None,
                    2,
                ),
                choose(Executor, Choice::Card(&MP3_ELIGIBLE)),
                achieve(filtered(&MP3_ELIGIBLE, &[Has(Is(Reg::Chosen))])),
            ],
        ),
    ],
};

// I DEMAND you return all cards from your forecast! / You may splay
// your yellow cards up. / Draw three [11]. If GPS was foreseen,
// foreshadow them.
const GPS: Programs = Programs {
    echo: None,
    dogmas: &[
        (true, &[return_(FORECAST).all().shortcut(ORIGIN_ALL)]),
        (false, &[splay(&[Yellow], Splay::Up)]),
        (
            false,
            &[
                draws(Const(3), 11, Then::Keep).keep(DRAWN),
                if_(&Foreseen, 1),
                foreshadow(DRAWN_IN_HAND).all().shortcut(ORIGIN_ALL),
            ],
        ),
    ],
};

const VISIBLE_COLOR: Expr = Expr::Visible(&Visibility {
    who: Executor,
    colors: ColorSet::Slot(ITEM),
    seen: Seen::Cards,
});
const VISIBLE_FIRST: Slot = Slot::effect(2);
const VISIBLE_MISMATCH: Slot = Slot::effect(3);

// Echo: Meld a card from your score pile. / You may score the bottom
// card or two bottom cards of one color from your board. If all colors
// on your board contain the same number of visible cards, you win. /
// Draw and meld a [10].
const PUZZLE_CUBE: Programs = Programs {
    echo: Some(&[meld(SCORE)]),
    dogmas: &[
        (
            false,
            &[
                choose(Executor, Choice::Color(ColorSet::OfCards(&TOPS))),
                score(Board(Executor, ColorSet::Chosen, BoardRegion::Bottom))
                    .up_to(Some(Const(2)))
                    .optional()
                    .shortcut(ORIGIN),
                Instruction::Bind(Binding::Value(VISIBLE_FIRST, Const(0))),
                Instruction::Bind(Binding::Value(VISIBLE_MISMATCH, Const(0))),
                for_each(
                    Domain::Colors(ColorSet::OfCards(&TOPS)),
                    Order::Printed,
                    Some(ITEM),
                    4,
                ),
                if_(&Compare(Expr::Slot(VISIBLE_FIRST), Cmp::Equal, Const(0)), 1),
                Instruction::Bind(Binding::Value(VISIBLE_FIRST, VISIBLE_COLOR)),
                if_(
                    &Not(&Compare(
                        VISIBLE_COLOR,
                        Cmp::Equal,
                        Expr::Slot(VISIBLE_FIRST),
                    )),
                    1,
                ),
                Instruction::Bind(Binding::Value(VISIBLE_MISMATCH, Const(1))),
                // An empty board wins vacuously.
                if_(
                    &Compare(Expr::Slot(VISIBLE_MISMATCH), Cmp::Equal, Const(0)),
                    1,
                ),
                WIN,
            ],
        ),
        (false, &[draw(10), meld(LAST)]),
    ],
};

// Draw and reveal a [10]. If you have fewer than six cards of its color on
// your board, splay that color aslant on your board. Otherwise, return the
// bottom five cards of that color from all boards. If you do, repeat this effect.
const DRONE: Programs = Programs {
    echo: None,
    dogmas: &[(
        false,
        &[
            draw(10),
            reveal(LAST).keep(ITEM),
            if_(
                &Compare(
                    Count(&Board(
                        Executor,
                        ColorSet::Of(Reg::Slot(ITEM)),
                        BoardRegion::BelowTop(0),
                    )),
                    Cmp::Less,
                    Const(6),
                ),
                2,
            ),
            splay_set(ColorSet::Of(Reg::Slot(ITEM)), Splay::Aslant),
            Instruction::Jump(3),
            return_(Board(
                Either,
                ColorSet::Of(Reg::Slot(ITEM)),
                BoardRegion::BottomN(5),
            ))
            .all()
            .frozen()
            .shortcut(ORIGIN_ALL)
            .keep(MOVED),
            if_(
                &All(&[
                    Did(Outcome::All),
                    Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(10)),
                ]),
                1,
            ),
            REPEAT,
        ],
    )],
};

// I DEMAND you transfer all but the lowest cards in your hand to my score pile!
// You may achieve a card from any player's hand, if eligible. If you do, and
// Exoskeleton was foreseen, repeat this effect.
const EXOSKELETON: Programs = Programs {
    echo: None,
    dogmas: &[
        (
            true,
            &[transfer(
                filtered(
                    &HAND,
                    &[Lacks(Test::ValueIn(&filtered(&HAND, &[Filter::Lowest])))],
                ),
                Destination::Score(Demander),
            )
            .all()
            .shortcut(Shortcut::AnyOrder)],
        ),
        (
            false,
            &[
                achieve(filtered(
                    &Selector::Hand(Either),
                    &[Has(Eligible { score_factor: 1 })],
                ))
                .optional(),
                if_(&All(&[Did(Outcome::All), Foreseen]), 1),
                REPEAT,
            ],
        ),
    ],
};

pub(super) fn programs(code: &str) -> Option<&'static Programs> {
    Some(match code {
        "E001" => &BANGLE,
        "E002" => &CANDLES,
        "E003" => &PLUMBING,
        "E004" => &CHOPSTICKS,
        "E005" => &NOODLES,
        "E006" => &SOAP,
        "E007" => &COMB,
        "E008" => &ICE_SKATES,
        "E009" => &UMBRELLA,
        "E010" => &DICE,
        "E011" => &PERFUME,
        "E012" => &RULER,
        "E013" => &BELL,
        "E014" => &FLUTE,
        "E015" => &PUPPET,
        "E016" => &CROSSBOW,
        "E017" => &HORSESHOES,
        "E018" => &TOOTHBRUSH,
        "E019" => &WATERMILL,
        "E020" => &GLASSBLOWING,
        "E021" => &SCISSORS,
        "E022" => &LEVER,
        "E023" => &LINGUISTICS,
        "E024" => &PAGODA,
        "E025" => &CHATURANGA,
        "E026" => &LIQUID_FIRE,
        "E027" => &KATANA,
        "E028" => &DEODORANT,
        "E029" => &SANDPAPER,
        "E030" => &CHARITABLE_TRUST,
        "E031" => &HOMING_PIGEONS,
        "E032" => &ALMANAC,
        "E033" => &MAGNIFYING_GLASS,
        "E034" => &NOVEL,
        "E035" => &SUNGLASSES,
        "E036" => &KOBUKSON,
        "E037" => &SHURIKEN,
        "E038" => &BAROMETER,
        "E039" => &PENCIL,
        "E040" => &CHINTZ,
        "E041" => &GLOBE,
        "E042" => &SLIDE_RULE,
        "E043" => &TELESCOPE,
        "E044" => &CLOCK,
        "E045" => &TOILET,
        "E046" => &COKE,
        "E047" => &OCTANT,
        "E048" => &PRESSURE_COOKER,
        "E049" => &STOVE,
        "E050" => &PALAMPORE,
        "E051" => &SEED_DRILL,
        "E052" => &LIGHTNING_ROD,
        "E053" => &THERMOMETER,
        "E054" => &PIANO,
        "E055" => &TUNING_FORK,
        "E056" => &LOOM,
        "E057" => &SHRAPNEL,
        "E058" => &DENTURES,
        "E059" => &MORPHINE,
        "E060" => &HOT_AIR_BALLOON,
        "E061" => &STEAMBOAT,
        "E062" => &BIFOCALS,
        "E063" => &STETHOSCOPE,
        "E064" => &INDIAN_CLUBS,
        "E065" => &KALEIDOSCOPE,
        "E066" => &MACHINE_GUN,
        "E067" => &RUBBER,
        "E068" => &ELEVATOR,
        "E069" => &FERTILIZER,
        "E070" => &JEANS,
        "E071" => &TELEGRAPH,
        "E072" => &PHOTOGRAPHY,
        "E073" => &TYPEWRITER,
        "E074" => &ICE_CREAM,
        "E075" => &SAXOPHONE,
        "E076" => &BANDAGE,
        "E077" => &PARACHUTE,
        "E078" => &AIR_CONDITIONER,
        "E079" => &TRACTOR,
        "E080" => &NYLON,
        "E081" => &SLICED_BREAD,
        "E082" => &RADIO_TELESCOPE,
        "E083" => &X_RAY,
        "E084" => &CROSSWORD,
        "E085" => &TELEVISION,
        "E086" => &HELICOPTER,
        "E087" => &JET,
        "E088" => &ATM,
        "E089" => &WRISTWATCH,
        "E090" => &CREDIT_CARD,
        "E091" => &EMAIL,
        "E092" => &CALCULATOR,
        "E093" => &LASER,
        "E094" => &KARAOKE,
        "E095" => &ROCK,
        "E096" => &CAMCORDER,
        "E097" => &SOCIAL_NETWORKING,
        "E098" => &CELL_PHONE,
        "E099" => &MP3,
        "E100" => &GPS,
        "E101" => &BARCODE,
        "E102" => &ARTIFICIAL_HEART,
        "E103" => &HUMAN_GENOME,
        "E104" => &PUZZLE_CUBE,
        "E105" => &SUDOKU,
        "E106" => &DRONE,
        "E107" => &EXOSKELETON,
        "E108" => &DIGITAL_PET,
        "E109" => &ESPORTS,
        "E110" => &ALGOCRACY,
        "E111" => &ROBOCAR,
        "E112" => &STREAMING,
        "E113" => &TOUCHSCREEN,
        "E114" => &DEEPFAKE,
        "E115" => &MERITOCRACY,
        _ => return None,
    })
}
