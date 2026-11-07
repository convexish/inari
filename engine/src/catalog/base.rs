//! Base card programs, in catalog order. Each comment is the printed text.
use crate::program::build::*;

use Color::{Blue, Green, Purple, Red, Yellow};
use Cond::{Card, Compare, DemandMoved};
use Expr::{Add, Boxed, Const, Count, Max, Metric as M};
use Filter::{Has, Highest, Lacks, Lowest};
use Icon::{Authority, Concept, Efficiency, Health, Industry, Prosperity};
use Metric::{Achievements, Icons};
use Selector::{AvailableAchievements, BaseDeck, Board, Score as ScoreOf, TopOf, Tops};
use Test::{Code, ColorOnBoard, Icon as HasIcon, StackShows, StackSplayed, Value as ValueIs};
use Who::{Demander, Either, Executor, Opponent};

type Program = &'static [Instruction];

const DEMANDER_HAND: Destination = Destination::Hand(Demander);
const DEMANDER_SCORE: Destination = Destination::Score(Demander);
const DEMANDER_BOARD: Destination = Destination::Board(Demander);
const BIND_LAST: Instruction = bind(Selector::Reg(Reg::Last));
const SELF_EXECUTE: Instruction = execute(ExecutionMode::SelfExecute, Who::Executor);
const CHOSEN_SCORE: Selector = filtered(&SCORE, &[Has(ValueIs(Cmp::Equal, Expr::Chosen))]);
/// The executor's score pile holds no card of the chosen value: card backs
/// show values, so both players can tell, and every such value is the same
/// (empty) choice.
const NO_CHOSEN_SCORE: Cond = Compare(Count(&CHOSEN_SCORE), Cmp::Equal, Const(0));
/// Neither score pile holds a card of the chosen value.
const NO_CHOSEN_SCORE_EITHER: Cond = Compare(
    Count(&filtered(
        &ScoreOf(Either),
        &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
    )),
    Cmp::Equal,
    Const(0),
);
const ORIGIN: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: true,
};
const ORIGIN_ALL: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: false,
};
const COLOR: Shortcut = Shortcut::ByColor;

/// Colors on the executor's board showing the icon.
macro_rules! colors_with {
    ($icon:expr) => {
        Count(&filtered(&TOPS, &[Has(StackShows($icon))]))
    };
}

const fn deck_junk(age: u8) -> Instruction {
    junk(BaseDeck(Boxed(age))).all().shortcut(Shortcut::DeckTop)
}

const fn exchange(
    left: Selector,
    right: Selector,
    counts: &'static [Option<Expr>; 2],
) -> Instruction {
    Instruction::Exchange(ExchangeStep {
        left,
        right,
        counts,
        optional: false,
    })
}

// I DEMAND you draw a [1], then transfer the highest card in your hand to my
// hand! / Junk an available achievement of value 1 or 2.
const ARCHERY: [Program; 2] = [
    &[
        draw(1),
        transfer(filtered(&HAND, &[Highest]), DEMANDER_HAND),
    ],
    &[junk(filtered(
        &AvailableAchievements,
        &[Has(Test::Values(&[1, 2]))],
    ))
    .shortcut(Shortcut::SharedPool)],
];

// Draw and reveal a [1]. If it has [Authority], score it and repeat.
const METALWORKING: [Program; 1] = [&[
    draw(1),
    reveal(LAST),
    if_(&Card(Reg::Last, Has(HasIcon(Authority))), 2),
    score(LAST),
    REPEAT,
]];

// I DEMAND you transfer a card with [Prosperity] from your hand to my score
// pile! If you do, draw a [1] and repeat! / If no cards were transferred due
// to the demand, draw a [1].
const OARS: [Program; 2] = [
    &[
        transfer(filtered(&HAND, &[Has(HasIcon(Prosperity))]), DEMANDER_SCORE)
            .reveal_if_empty(&HAND)
            .shortcut(Shortcut::LowestFirst),
        if_(DID, 2),
        draw(1),
        REPEAT,
    ],
    &[if_(&Cond::Not(&DemandMoved(Verb::Transfer)), 1), draw(1)],
];

// You may return a card from your hand. If you do, draw and score a card of
// value one higher than the card you returned.
const AGRICULTURE: [Program; 1] = [&[
    return_(HAND).optional(),
    if_(DID, 2),
    draw_age(Add(&LAST_VALUE, 1)),
    score(LAST),
]];

// Meld the lowest card in your hand. Draw a [1].
const DOMESTICATION: [Program; 1] = [&[meld(filtered(&HAND, &[Lowest])), draw(1)]];

// You may meld any number of cards from your hand, each with [Authority]. /
// If you have exactly three red cards on your board, claim Monument.
const MASONRY: [Program; 2] = [
    &[meld(filtered(&HAND, &[Has(HasIcon(Authority))]))
        .up_to(None)
        .optional()
        .shortcut(COLOR)],
    &[
        if_(
            &Compare(
                Count(&Board(
                    Executor,
                    ColorSet::Only(&[Red]),
                    BoardRegion::BelowTop(0),
                )),
                Cmp::Equal,
                Const(3),
            ),
            1,
        ),
        claim(Special::Monument),
    ],
];

// Meld a card from your hand of different color from any card on your board.
// / Draw and score a [1] for each color on your board not present on any
// other player's board.
const CLOTHING: [Program; 2] = [
    &[
        // With all five colors on the board, nothing can qualify publicly.
        if_(
            &Compare(Expr::DistinctColors(&TOPS), Cmp::Less, Const(5)),
            1,
        ),
        meld(filtered(&HAND, &[Lacks(ColorOnBoard(Executor))])).reveal_if_empty(&HAND),
    ],
    &[draws(
        Count(&filtered(&TOPS, &[Lacks(ColorOnBoard(Opponent))])),
        1,
        Then::Score,
    )],
];

// Draw and meld a [1].
const SAILING: [Program; 1] = [&[draw(1), meld(LAST)]];

// Draw two [1].
const THE_WHEEL: [Program; 1] = [&[draw(1), draw(1)]];

// You may return up to three cards from your hand. If you return any, draw
// and score a card of value equal to the number you returned. / Draw a [1].
const POTTERY: [Program; 2] = [
    &[
        return_(HAND)
            .up_to(Some(Const(3)))
            .optional()
            .shortcut(ORIGIN),
        if_(DID_ANY, 2),
        draw_age(Expr::NonZero(&Expr::Done)),
        score(LAST),
    ],
    &[draw(1)],
];

// You may return three cards from your hand. If you do, draw and meld a [3].
// / You may return a [3] from your hand. If you do, draw three [1].
const TOOLS: [Program; 2] = [
    &[
        return_(HAND)
            .exactly(Const(3))
            .optional()
            .shortcut(ORIGIN_ALL),
        if_(&Cond::Did(Outcome::Exactly(3)), 2),
        draw(3),
        meld(LAST),
    ],
    &[
        return_(filtered(&HAND, &[Has(Test::BoxedValues(&[3]))])).optional(),
        if_(DID, 3),
        draw(1),
        draw(1),
        draw(1),
    ],
];

// Draw a [2].
const WRITING: [Program; 1] = [&[draw(2)]];

// I DEMAND you transfer a top card with [Authority] from your board to my
// board if you have at least four [Authority] on your board! If you do, draw
// a [1]!
const CITY_STATES: [Program; 1] = [&[
    if_(
        &Compare(M(Executor, Icons(Authority)), Cmp::AtLeast, Const(4)),
        3,
    ),
    transfer(filtered(&TOPS, &[Has(HasIcon(Authority))]), DEMANDER_BOARD),
    if_(DID, 1),
    draw(1),
]];

// You may tuck a card from your hand of the same color as any card on your
// board. If you do, you may splay that color of your cards left.
const CODE_OF_LAWS: [Program; 1] = [&[
    tuck(filtered(&HAND, &[Has(ColorOnBoard(Executor))])).optional(),
    if_(DID, 1),
    Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors: ColorSet::Of(Reg::Last),
            direction: Splay::Left,
            from: None,
        },
        targets: SplayTargets::Changes,
        optional: true,
        all: false,
    }),
]];

// Draw and reveal a [1]. If it is the same color as any card on your board,
// meld it and draw a [1].
const MYSTICISM: [Program; 1] = [&[
    draw(1),
    reveal(LAST),
    if_(&Card(Reg::Last, Has(ColorOnBoard(Executor))), 2),
    meld(LAST),
    draw(1),
]];

// I DEMAND you transfer two cards from your hand to my hand! Then draw a
// [2]! / If you are the only player with five top cards, claim Empire.
const CONSTRUCTION: [Program; 2] = [
    &[
        transfer(HAND, DEMANDER_HAND)
            .exactly(Const(2))
            .shortcut(ORIGIN_ALL),
        draw(2),
    ],
    &[
        if_(
            &Cond::All(&[
                Compare(Count(&TOPS), Cmp::Equal, Const(5)),
                Compare(Count(&Tops(Opponent)), Cmp::Less, Const(5)),
            ]),
            1,
        ),
        claim(Special::Empire),
    ],
];

// Meld one or two cards from your hand. If you meld two, you may transfer your
// top red card to another player's board. If you do, meld that player's top
// green card.
const ROAD_BUILDING: [Program; 1] = [&[
    meld(HAND),
    // "One or two" is a quantity, not a "may": Blackmail's `must` keeps the stop.
    meld(HAND).up_to(Some(Const(1))),
    if_(DID_ANY, 3),
    transfer(TopOf(Executor, Red), Destination::Board(Opponent)).optional(),
    if_(DID, 1),
    meld(TopOf(Opponent, Green)),
]];

// You may choose to either exchange all the highest cards in your hand with
// all the highest cards in your score pile, or junk all cards in the [3]
// deck.
const CANAL_BUILDING: [Program; 1] = [&[
    Instruction::Choose {
        who: Executor,
        choice: Choice::Branch(&[
            (0, "Exchange all highest cards in your hand and score pile"),
            (2, "Junk all cards in the [3] deck"),
        ]),
        optional: true,
        keep: None,
        alias: None,
    },
    exchange(
        filtered(&HAND, &[Highest]),
        filtered(&SCORE, &[Highest]),
        &[None, None],
    ),
    Instruction::Jump(1),
    deck_junk(3),
]];

// Draw a [2] for every color on your board with [Health]. / You may tuck a
// green card from your hand. If you don't, junk all cards in the [2] deck,
// and junk Fermenting if it is a top card on any board.
const FERMENTING: [Program; 2] = [
    &[draws(colors_with!(Health), 2, Then::Keep)],
    &[
        tuck(filtered(&HAND, &[Has(Test::Color(Green))])).optional(),
        if_(DID, 1),
        Instruction::Jump(2),
        deck_junk(2),
        junk(filtered(&Tops(Either), &[Has(Code("B019"))])),
    ],
];

// You may return any number of cards from your hand. If you do, draw and
// score a [2] for every different value of card you returned.
const CURRENCY: [Program; 1] = [&[
    return_(HAND).up_to(None).optional().shortcut(ORIGIN),
    draws(Expr::DoneValues, 2, Then::Score),
]];

// I DEMAND you transfer a [1] from your score pile to my score pile! / If
// any card was transferred due to the demand, draw and score a [1].
const MAPMAKING: [Program; 2] = [
    &[transfer(
        filtered(&SCORE, &[Has(Test::BoxedValues(&[1]))]),
        DEMANDER_SCORE,
    )],
    &[if_(&DemandMoved(Verb::Transfer), 2), draw(1), score(LAST)],
];

// If you have more cards in your hand than in your score pile, draw two [3].
const CALENDAR: [Program; 1] = [&[
    if_(&Compare(Count(&SCORE), Cmp::Greater, Count(&HAND)), 2),
    draw(3),
    draw(3),
]];

// You may return a card from your hand. If you do, draw and meld a card of
// value one higher than the card you returned.
const MATHEMATICS: [Program; 1] = [&[
    return_(HAND).optional(),
    if_(DID, 2),
    draw_age(Add(&LAST_VALUE, 1)),
    meld(LAST),
]];

// I DEMAND you transfer a top card on your board of a different color from
// any card on my board to my score pile! If you do, draw and tuck a [1]! /
// Draw and tuck a [1].
const MONOTHEISM: [Program; 2] = [
    &[
        transfer(
            filtered(&TOPS, &[Lacks(ColorOnBoard(Demander))]),
            DEMANDER_SCORE,
        ),
        if_(DID, 2),
        draw(1),
        tuck(LAST),
    ],
    &[draw(1), tuck(LAST)],
];

// You may splay left any one color of your cards. / You may score a card from
// your hand.
const PHILOSOPHY: [Program; 2] = [
    &[splay(&Color::ALL, Splay::Left)],
    &[score(HAND).optional()],
];

// I DEMAND you transfer all top cards with [Authority] from your board to my
// score pile! / You may splay your red cards left.
const ENGINEERING: [Program; 2] = [
    &[
        transfer(filtered(&TOPS, &[Has(HasIcon(Authority))]), DEMANDER_SCORE)
            .per_color(None)
            .shortcut(ORIGIN_ALL),
    ],
    &[splay(&[Red], Splay::Left)],
];

// Draw and meld a [3]. If it has [Prosperity], draw and score a [4]. Otherwise,
// if you have more points than another player, transfer a card from your
// score pile to that player's score pile.
const OPTICS: [Program; 1] = [&[
    draw(3),
    BIND_LAST,
    meld(LAST),
    if_(&Card(Reg::Bound, Has(HasIcon(Prosperity))), 2),
    draw(4),
    score(LAST),
    if_(&Card(Reg::Bound, Lacks(HasIcon(Prosperity))), 2),
    if_(
        &Compare(
            M(Opponent, Metric::Score),
            Cmp::Less,
            M(Executor, Metric::Score),
        ),
        1,
    ),
    transfer(SCORE, Destination::Score(Opponent)),
]];

// I DEMAND you exchange all cards in your hand with all the highest cards in
// my hand! / Score a card from your hand with [Authority]. / You may splay
// your red cards left.
const MACHINERY: [Program; 3] = [
    &[exchange(
        HAND,
        filtered(&Selector::Hand(Demander), &[Highest]),
        &[None, None],
    )],
    &[score(filtered(&HAND, &[Has(HasIcon(Authority))]))
        .verify()
        .reveal_if_empty(&HAND)],
    &[splay(&[Red], Splay::Left)],
];

// I DEMAND you exchange the highest card in your score pile with the lowest
// card in my score pile! / Junk an available achievement of value 3 or 4.
const MEDICINE: [Program; 2] = [
    &[exchange(
        filtered(&SCORE, &[Highest]),
        filtered(&ScoreOf(Demander), &[Lowest]),
        &[Some(Const(1)), Some(Const(1))],
    )],
    &[junk(filtered(
        &AvailableAchievements,
        &[Has(Test::Values(&[3, 4]))],
    ))
    .shortcut(Shortcut::SharedPool)],
];

// I DEMAND you transfer a top non-green card with [Health] from your board to
// my board, and then meld a top card without [Health] from my board!
const COMPASS: [Program; 1] = [&[
    transfer(
        filtered(&TOPS, &[Lacks(Test::Color(Green)), Has(HasIcon(Health))]),
        DEMANDER_BOARD,
    ),
    meld(filtered(&Tops(Demander), &[Lacks(HasIcon(Health))])),
]];

// You may splay your green or blue cards left. / Score a top card with
// [Health]. If you do, draw a [4] for every color you have splayed left.
const PAPER: [Program; 2] = [
    &[splay(&[Green, Blue], Splay::Left)],
    &[
        score(filtered(&TOPS, &[Has(HasIcon(Health))])),
        if_(DID, 1),
        draws(
            Count(&filtered(&TOPS, &[Has(StackSplayed(Some(Splay::Left)))])),
            4,
            Then::Keep,
        ),
    ],
];

// Draw and reveal a [4] for every color on your board with [Authority]. If
// any of the drawn cards are red, return all cards from your hand. / Meld a
// card from your hand, then score a card from your hand.
const ALCHEMY: [Program; 2] = [
    &[
        draws(colors_with!(Authority), 4, Then::Reveal),
        if_(&Cond::DrawnColor(Red), 1),
        return_(HAND).all().shortcut(ORIGIN_ALL),
    ],
    &[meld(HAND), score(HAND)],
];

// You may meld all the cards in your score pile. / If each top card on your
// board has [Prosperity], claim World.
const TRANSLATION: [Program; 2] = [
    &[meld(SCORE).all().optional().shortcut(COLOR)],
    &[
        if_(
            &Compare(
                Count(&filtered(&TOPS, &[Lacks(HasIcon(Prosperity))])),
                Cmp::Equal,
                Const(0),
            ),
            1,
        ),
        claim(Special::World),
    ],
];

// You may return the highest card from your score pile. If you do, draw a
// card of value two higher than the highest card in your score pile.
const EDUCATION: [Program; 1] = [&[
    return_(filtered(&SCORE, &[Highest])).optional(),
    if_(DID, 1),
    draw_age(Add(&Max(&SCORE), 2)),
]];

// I DEMAND you transfer a card with [Authority] from your hand to my hand! If
// you do, junk all available special achievements! / You may splay your
// yellow or purple cards left. If you do, draw a [3].
const FEUDALISM: [Program; 2] = [
    &[
        transfer(filtered(&HAND, &[Has(HasIcon(Authority))]), DEMANDER_HAND).reveal_if_empty(&HAND),
        if_(DID, 1),
        Instruction::Special(SpecialStep::JunkAll),
    ],
    &[splay(&[Yellow, Purple], Splay::Left), if_(DID, 1), draw(3)],
];

// Draw and tuck a [3]. If it is green, junk all cards in the [5] deck. If it
// has [Prosperity], repeat this effect.
const COLONIALISM: [Program; 1] = [&[
    draw(3),
    BIND_LAST,
    tuck(BOUND),
    if_(&Card(Reg::Bound, Has(Test::Color(Green))), 1),
    deck_junk(5),
    if_(&Card(Reg::Bound, Has(HasIcon(Prosperity))), 1),
    REPEAT,
]];

// I DEMAND you transfer a top card with [Authority] from your board to my
// score pile! / If any card was transferred due to the demand, draw and score
// a [2].
const GUNPOWDER: [Program; 2] = [
    &[transfer(
        filtered(&TOPS, &[Has(HasIcon(Authority))]),
        DEMANDER_SCORE,
    )],
    &[if_(&DemandMoved(Verb::Transfer), 2), draw(2), score(LAST)],
];

// I DEMAND you return a card from your score pile! If you do, return a top
// card of equal value from your board! If you do, junk all cards in the [4]
// deck!
const ANATOMY: [Program; 1] = [&[
    return_(SCORE),
    if_(DID, 3),
    return_(filtered(&TOPS, &[Has(ValueIs(Cmp::Equal, LAST_VALUE))])),
    if_(DID, 1),
    deck_junk(4),
]];

// You may return a card from your hand. If you do, score a card from your
// hand for every color on your board with [Concept].
const PERSPECTIVE: [Program; 1] = [&[
    return_(HAND).optional(),
    if_(DID, 1),
    score(HAND).exactly(colors_with!(Concept)).shortcut(ORIGIN),
]];

// You may splay right any one color of your cards currently splayed left. If
// you do, draw and score a [4]. / If you have five colors splayed, each in
// any direction, claim Wonder.
const INVENTION: [Program; 2] = [
    &[
        Instruction::Splay(SplayStep {
            who: Executor,
            choices: SplayChoices::Fixed {
                colors: ColorSet::Only(&Color::ALL),
                direction: Splay::Right,
                from: Some(Splay::Left),
            },
            targets: SplayTargets::Changes,
            optional: true,
            all: false,
        }),
        if_(DID, 2),
        draw(4),
        score(LAST),
    ],
    &[
        if_(
            &Compare(
                Count(&filtered(&TOPS, &[Has(StackSplayed(None))])),
                Cmp::Equal,
                Const(5),
            ),
            1,
        ),
        claim(Special::Wonder),
    ],
];

// I DEMAND you transfer a [2] or [3] from your score pile to my score pile!
const NAVIGATION: [Program; 1] = [&[transfer(
    filtered(&SCORE, &[Has(Test::BoxedValues(&[2, 3]))]),
    DEMANDER_SCORE,
)]];

// Draw and meld a [5].
const EXPERIMENTATION: [Program; 1] = [&[draw(5), meld(LAST)]];

// You may return a card from your score pile. If you do, draw a card of value
// two higher than the top purple card on your board. / You may splay your
// blue cards right.
const PRINTING_PRESS: [Program; 2] = [
    &[
        return_(SCORE).optional(),
        if_(DID, 1),
        draw_age(Add(&Max(&TopOf(Executor, Purple)), 2)),
    ],
    &[splay(&[Blue], Splay::Right)],
];

// I DEMAND you transfer a top non-purple card with [Prosperity] from your
// board to my board! If you do, draw and meld a [4]! / You may splay your
// green cards right.
const ENTERPRISE: [Program; 2] = [
    &[
        transfer(
            filtered(
                &TOPS,
                &[Lacks(Test::Color(Purple)), Has(HasIcon(Prosperity))],
            ),
            DEMANDER_BOARD,
        ),
        if_(DID, 2),
        draw(4),
        meld(LAST),
    ],
    &[splay(&[Green], Splay::Right)],
];

// You may splay your yellow or purple cards right. / You may tuck a card from
// your hand for every color on your board splayed in any direction.
const REFORMATION: [Program; 2] = [
    &[splay(&[Yellow, Purple], Splay::Right)],
    &[tuck(HAND)
        .up_to(Some(Count(&filtered(&TOPS, &[Has(StackSplayed(None))]))))
        .optional()
        .shortcut(COLOR)],
];

/// The executor has no cards of the chosen colour: scoring its tops changes
/// nothing, so every such colour is the same choice.
const NO_CARDS_OF_CHOSEN_COLOR: Cond = Cond::Compare(
    Expr::Count(&Selector::Board(
        Who::Executor,
        ColorSet::Chosen,
        BoardRegion::BelowTop(0),
    )),
    Cmp::Equal,
    Expr::Const(0),
);

// Draw and tuck a [5]. / You may splay your red cards right. / You may choose
// a color. If you do, score your top two cards of that color.
const COAL: [Program; 3] = [
    &[draw(5), tuck(LAST)],
    &[splay(&[Red], Splay::Right)],
    &[
        Instruction::Choose {
            who: Executor,
            choice: Choice::Colors(1),
            optional: true,
            keep: None,
            alias: Some(Alias::NoOpWhen(&NO_CARDS_OF_CHOSEN_COLOR)),
        },
        score(Board(Executor, ColorSet::Chosen, BoardRegion::Top)).exactly(Const(2)),
    ],
];

// I DEMAND you transfer two cards of value 4 or less from your score pile to
// my score pile! / If any card was transferred due to the demand, score your
// lowest top card with [Prosperity].
const THE_PIRATE_CODE: [Program; 2] = [
    &[transfer(
        filtered(&SCORE, &[Has(Test::Values(&[0, 1, 2, 3, 4]))]),
        DEMANDER_SCORE,
    )
    .exactly(Const(2))
    .shortcut(ORIGIN)],
    &[
        if_(&DemandMoved(Verb::Transfer), 1),
        score(filtered(&TOPS, &[Has(HasIcon(Prosperity)), Lowest])),
    ],
];

// I DEMAND you transfer all the cards of the value of my choice from your
// score pile to your hand! / You may splay your yellow cards right.
const STATISTICS: [Program; 2] = [
    &[
        choose(Demander, Choice::Value(ValueChoices::Any)).alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE)),
        transfer(CHOSEN_SCORE, Destination::Hand(Executor))
            .all()
            .shortcut(ORIGIN_ALL),
    ],
    &[splay(&[Yellow], Splay::Right)],
];

// Draw and tuck two [4]. Score your bottom yellow card. If it is Steam
// Engine, junk all cards in the [6] deck.
const STEAM_ENGINE: [Program; 1] = [&[
    draw(4),
    tuck(LAST),
    draw(4),
    tuck(LAST),
    score(Board(
        Executor,
        ColorSet::Only(&[Yellow]),
        BoardRegion::Bottom,
    )),
    BIND_LAST,
    if_(&Card(Reg::Bound, Has(Code("B049"))), 1),
    deck_junk(6),
]];

// I DEMAND you transfer a top non-green card with [Industry] from your board to
// my board! If you do, draw and score a [5]! / You may splay your green cards
// right.
const BANKING: [Program; 2] = [
    &[
        transfer(
            filtered(&TOPS, &[Lacks(Test::Color(Green)), Has(HasIcon(Industry))]),
            DEMANDER_BOARD,
        ),
        if_(DID, 2),
        draw(5),
        score(LAST),
    ],
    &[splay(&[Green], Splay::Right)],
];

// You may reveal and return a card from your hand. If you do, splay that
// color of your cards right, and then draw a card of value equal to the
// number of cards of that color on your board.
const MEASUREMENT: [Program; 1] = [&[
    reveal(HAND).optional(),
    BIND_LAST,
    return_(BOUND),
    if_(DID, 2),
    splay_set(ColorSet::Of(Reg::Bound), Splay::Right),
    draw_age(Count(&Board(
        Executor,
        ColorSet::Of(Reg::Bound),
        BoardRegion::BelowTop(0),
    ))),
]];

// You may splay your blue cards right. / Draw and score a card of value one
// higher than the highest top card on your board, and then return a card from
// your score pile.
const CHEMISTRY: [Program; 2] = [
    &[splay(&[Blue], Splay::Right)],
    &[draw_age(Add(&Max(&TOPS), 1)), score(LAST), return_(SCORE)],
];

// Draw three [6] and reveal them. If at least two of the drawn cards are the
// same color, return all cards in your hand.
const PHYSICS: [Program; 1] = [&[
    draws(Const(3), 6, Then::Reveal),
    if_(&Cond::DrawnColorTwice, 1),
    return_(HAND).all().shortcut(ORIGIN_ALL),
]];

// Draw and reveal a [6]. If the card is green or blue, meld it and repeat this
// effect. / If all non-purple top cards on your board are value 6 or higher,
// claim Universe.
const ASTRONOMY: [Program; 2] = [
    &[
        draw(6),
        BIND_LAST,
        reveal(LAST),
        if_(
            &Cond::Any(&[
                Card(Reg::Bound, Has(Test::Color(Green))),
                Card(Reg::Bound, Has(Test::Color(Blue))),
            ]),
            2,
        ),
        meld(BOUND),
        REPEAT,
    ],
    &[
        if_(
            &Compare(
                Count(&filtered(
                    &TOPS,
                    &[
                        Lacks(Test::Color(Purple)),
                        Has(ValueIs(Cmp::Less, Const(6))),
                    ],
                )),
                Cmp::Equal,
                Const(0),
            ),
            1,
        ),
        claim(Special::Universe),
    ],
];

// I DEMAND you transfer a top card with [Concept] from your board to my board
// that is higher than my top card of its color! If you do, draw a [5]!
const SOCIETIES: [Program; 1] = [&[
    transfer(
        filtered(
            &TOPS,
            &[Has(HasIcon(Concept)), Has(Test::AboveTop(Demander))],
        ),
        DEMANDER_BOARD,
    ),
    if_(DID, 1),
    draw(5),
]];

// Draw and tuck three [6]. If you have more [Efficiency] than each other
// player, return your top red card. / You may splay your red or purple cards
// right.
const INDUSTRIALIZATION: [Program; 2] = [
    &[
        draw(6),
        tuck(LAST),
        draw(6),
        tuck(LAST),
        draw(6),
        tuck(LAST),
        if_(
            &Compare(
                M(Executor, Icons(Efficiency)),
                Cmp::Greater,
                M(Opponent, Icons(Efficiency)),
            ),
            1,
        ),
        return_(TopOf(Executor, Red)),
    ],
    &[splay(&[Red, Purple], Splay::Right)],
];

// Draw and score a card of value equal to the highest card in your score pile.
const MACHINE_TOOLS: [Program; 1] = [&[draw_age(Max(&SCORE)), score(LAST)]];

// You may draw and tuck a [6]. If you tuck a card, score a top card without
// [Industry] of each color on your board. / You may splay your yellow cards
// right.
const CANNING: [Program; 2] = [
    &[
        draw(6).optional(),
        // Whether the optional draw was taken: a [7] is still tucked.
        if_(DID_ANY, 3),
        tuck(LAST),
        // "If you tuck a card": a replaced tuck scores nothing.
        if_(DID, 1),
        score(filtered(&TOPS, &[Lacks(HasIcon(Industry))]))
            .per_color(None)
            .shortcut(ORIGIN_ALL),
    ],
    &[splay(&[Yellow], Splay::Right)],
];

// I DEMAND you choose a card in your score pile! Return all the cards from your
// score pile of its value! If you do, draw and meld a [6]! / If any card was
// returned as a result of the demand, draw and meld a [7].
const VACCINATION: [Program; 2] = [
    &[
        // Only the chosen card's value matters: choose among present values.
        choose(Executor, Choice::Value(ValueChoices::Among(&SCORE))),
        return_(CHOSEN_SCORE).all().shortcut(ORIGIN),
        if_(DID, 2),
        draw(6),
        meld(LAST),
    ],
    &[if_(&DemandMoved(Verb::Return), 2), draw(7), meld(LAST)],
];

// Reveal a card from your hand. Transfer to your hand all cards of that card's
// color from all opponents' hands. Then, meld all cards of that color from your
// hand.
const CLASSIFICATION_SHOWN: Slot = Slot::effect(0);
const CLASSIFICATION_PREY: Selector = Selector::Union(&[
    filtered(
        &filtered(
            &Selector::Hand(Opponent),
            &[Has(Test::InSlot(CLASSIFICATION_SHOWN))],
        ),
        &[Has(Test::ColorOf(Reg::Bound))],
    ),
    // A card that arrived during the sweep is still taken, blind.
    filtered(
        &Selector::Hand(Opponent),
        &[
            Lacks(Test::InSlot(CLASSIFICATION_SHOWN)),
            Has(Test::ColorOf(Reg::Bound)),
        ],
    ),
]);
const CLASSIFICATION: [Program; 1] = [&[
    reveal(HAND),
    if_(DID, 6),
    BIND_LAST,
    // Hands are revealed before the cards they affect move, so the executor orders
    // known cards when a karma reacts.
    reveal_all(Selector::Hand(Opponent)).keep(CLASSIFICATION_SHOWN),
    transfer(CLASSIFICATION_PREY, Destination::Hand(Executor))
        .all()
        .shortcut(Shortcut::AnyOrder),
    // The rest of the executor's hand, before its melds.
    reveal_all(filtered(
        &HAND,
        &[
            Lacks(Test::InSlot(CLASSIFICATION_SHOWN)),
            Lacks(Test::Is(Reg::Bound)),
        ],
    ))
    .add(CLASSIFICATION_SHOWN),
    meld(filtered(&HAND, &[Has(Test::ColorOf(Reg::Bound))])).all(),
    // Show any card that arrived unseen in either hand during the sweeps.
    reveal_all(filtered(
        &Selector::Hand(Either),
        &[
            Lacks(Test::InSlot(CLASSIFICATION_SHOWN)),
            Lacks(Test::Is(Reg::Bound)),
        ],
    )),
]];

// If your green cards are splayed right, you may splay any one color of your
// cards right. / You may splay your green cards right.
const METRIC_SYSTEM: [Program; 2] = [
    &[
        if_(
            &Cond::Splayed {
                who: Executor,
                colors: ColorSet::Only(&[Green]),
                direction: Splay::Right,
                present: true,
            },
            1,
        ),
        splay(&Color::ALL, Splay::Right),
    ],
    &[splay(&[Green], Splay::Right)],
];

// You may splay your blue cards right. / Draw and meld a [7].
const ATOMIC_THEORY: [Program; 2] = [&[splay(&[Blue], Splay::Right)], &[draw(7), meld(LAST)]];

// Choose a value. You may meld all the cards of that value in your score
// pile. / You may junk an available achievement of value 5, 6, or 7.
const ENCYCLOPEDIA: [Program; 2] = [
    &[
        choose(Executor, Choice::Value(ValueChoices::Among(&SCORE))),
        meld(CHOSEN_SCORE).all().optional().shortcut(COLOR),
    ],
    &[junk(filtered(
        &AvailableAchievements,
        &[Has(Test::Values(&[5, 6, 7]))],
    ))
    .optional()],
];

// You may return any number of cards from your hand. If you have returned
// more cards than any opponent due to Democracy so far during this action,
// draw and score an [8].
const DEMOCRACY: [Program; 1] = [&[
    return_(HAND).up_to(None).optional().shortcut(ORIGIN),
    if_(&Cond::ReturnedMost, 2),
    draw(8),
    score(LAST),
]];

// I DEMAND you transfer a card from your hand to my score pile! If you do,
// draw a [6]! / You may splay your red or purple cards right.
const EMANCIPATION: [Program; 2] = [
    &[transfer(HAND, DEMANDER_SCORE), if_(DID, 1), draw(6)],
    &[splay(&[Red, Purple], Splay::Right)],
];

// I DEMAND you transfer a card from your score pile to my score pile for each
// color with [Prosperity] on my board! / Return your bottom red card.
const COMBUSTION: [Program; 2] = [
    &[transfer(SCORE, DEMANDER_SCORE)
        .exactly(Count(&filtered(
            &Tops(Demander),
            &[Has(StackShows(Prosperity))],
        )))
        .shortcut(ORIGIN)],
    &[return_(Board(
        Executor,
        ColorSet::Only(&[Red]),
        BoardRegion::Bottom,
    ))],
];

// I DEMAND you transfer the three highest cards from your hand to my hand! If
// you transfer any, and then have no cards in hand, draw a [7]!
const EXPLOSIVES: [Program; 1] = [&[
    transfer(filtered(&HAND, &[Highest]), DEMANDER_HAND)
        .exactly(Const(3))
        .shortcut(Shortcut::ByOrigin {
            descending: true,
            quota: true,
        }),
    if_(DID_ANY, 2),
    if_(&Compare(Count(&HAND), Cmp::Equal, Const(0)), 1),
    draw(7),
]];

// I DEMAND you return all but one card in your hand! / You may score a card
// from your hand.
const REFRIGERATION: [Program; 2] = [
    &[return_(HAND)
        .exactly(Add(&Count(&HAND), -1))
        .shortcut(ORIGIN)],
    &[score(HAND).optional()],
];

// I DEMAND you exchange the two highest cards in your hand with the lowest
// card in my hand! / Choose [7] or [8]. Junk all cards in that deck.
const SANITATION: [Program; 2] = [
    &[exchange(
        filtered(&HAND, &[Highest]),
        filtered(&Selector::Hand(Demander), &[Lowest]),
        &[Some(Const(2)), Some(Const(1))],
    )],
    &[
        unless_decks_empty!(7, 8),
        choose(Executor, Choice::Value(ValueChoices::Boxed(&[7, 8]))),
        junk(BaseDeck(Expr::Chosen))
            .all()
            .shortcut(Shortcut::DeckTop),
    ],
];

// You may exchange all the cards in your hand with all the cards in your
// score pile.
const BICYCLE: [Program; 1] = [&[exchange(HAND, SCORE, &[None, None]).optional()]];

// Return all your top cards without [Industry], one per color. Draw an [8] for
// each card you return.
const ELECTRICITY: [Program; 1] = [&[
    return_(filtered(&TOPS, &[Lacks(HasIcon(Industry))]))
        .per_color(None)
        .shortcut(ORIGIN_ALL),
    draws(Expr::Done, 8, Then::Keep),
]];

// You may choose to either draw and score an [8] and then return a card from
// your score pile, or draw a card of value one higher than the highest card
// in your score pile.
const EVOLUTION: [Program; 1] = [&[
    Instruction::Choose {
        who: Executor,
        choice: Choice::Branch(&[
            (
                0,
                "Draw and score an [8], then return a card from your score pile",
            ),
            (4, "Draw one value higher than your highest score card"),
        ]),
        optional: true,
        keep: None,
        alias: None,
    },
    draw(8),
    score(LAST),
    return_(SCORE),
    Instruction::Jump(1),
    draw_age(Add(&Max(&SCORE), 1)),
]];

// You may splay your yellow or blue cards up. / You may junk an available
// special achievement, or make a junked special achievement available.
const PUBLICATIONS: [Program; 2] = [
    &[splay(&[Yellow, Blue], Splay::Up)],
    &[Instruction::Special(SpecialStep::Toggle { optional: true })],
];

// You may tuck up to three cards from your hand. If you do, draw and score a
// [7] for every different value of card you tucked.
const LIGHTING: [Program; 1] = [&[
    tuck(HAND).up_to(Some(Const(3))).optional().shortcut(COLOR),
    draws(Expr::DoneValues, 7, Then::Score),
]];

// Return all cards from your hand. / Draw three [6]. / You may splay up any one
// color of your cards currently splayed right.
const RAILROAD: [Program; 3] = [
    &[return_(HAND).all().shortcut(ORIGIN_ALL)],
    &[draw(6), draw(6), draw(6)],
    &[Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors: ColorSet::Only(&Color::ALL),
            direction: Splay::Up,
            from: Some(Splay::Right),
        },
        targets: SplayTargets::Changes,
        optional: true,
        all: false,
    })],
];

// If your red cards are splayed up, you may splay any one color of your cards
// up. / You may splay your red cards up.
const FLIGHT: [Program; 2] = [
    &[
        if_(
            &Cond::Splayed {
                who: Executor,
                colors: ColorSet::Only(&[Red]),
                direction: Splay::Up,
                present: true,
            },
            1,
        ),
        splay(&Color::ALL, Splay::Up),
    ],
    &[splay(&[Red], Splay::Up)],
];

// I DEMAND you transfer your two highest non-red top cards without [Industry]
// of different colors to my score pile! If you transfer any cards, draw an [8]!
const MOBILITY: [Program; 1] = [&[
    transfer(
        filtered(
            &TOPS,
            &[Lacks(Test::Color(Red)), Lacks(HasIcon(Industry)), Highest],
        ),
        DEMANDER_SCORE,
    )
    .per_color(Some(Const(2)))
    .shortcut(ORIGIN),
    if_(DID_ANY, 1),
    draw(8),
]];

// You may return up to three cards from your hand. Draw two [8] for every
// different value of card you returned.
const ANTIBIOTICS: [Program; 1] = [&[
    return_(HAND)
        .up_to(Some(Const(3)))
        .optional()
        .shortcut(ORIGIN),
    draws(Expr::Mul(&Expr::DoneValues, 2), 8, Then::Keep),
]];

// I DEMAND you transfer a top non-yellow card with [Efficiency] from your
// board to my board! If you do, score the top card of its color, return the
// rest of that color, and transfer Skyscrapers to my hand!
const SKYSCRAPERS: [Program; 1] = [&[
    transfer(
        filtered(
            &TOPS,
            &[Lacks(Test::Color(Yellow)), Has(HasIcon(Efficiency))],
        ),
        DEMANDER_BOARD,
    ),
    BIND_LAST,
    if_(DID, 3),
    score(Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top)),
    return_(Board(
        Executor,
        ColorSet::Of(Reg::Bound),
        BoardRegion::BelowTop(0),
    ))
    .all()
    .shortcut(ORIGIN),
    transfer(filtered(&Tops(Either), &[Has(Code("B079"))]), DEMANDER_HAND),
]];

// I DEMAND you transfer a top non-green card with [Industry] from your board to
// my score pile! If you do, draw and meld an [8]! / Draw and meld an [8].
const CORPORATIONS: [Program; 2] = [
    &[
        transfer(
            filtered(&TOPS, &[Lacks(Test::Color(Green)), Has(HasIcon(Industry))]),
            DEMANDER_SCORE,
        ),
        if_(DID, 2),
        draw(8),
        meld(LAST),
    ],
    &[draw(8), meld(LAST)],
];

// You may return a card from your hand. If you do, choose a value, and return
// all cards of that value from all score piles. / You may splay your purple
// cards up.
const MASS_MEDIA: [Program; 2] = [
    &[
        return_(HAND).optional(),
        if_(DID, 2),
        choose(Executor, Choice::Value(ValueChoices::Any))
            .alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE_EITHER)),
        return_(filtered(
            &ScoreOf(Either),
            &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
        ))
        .all()
        .shortcut(ORIGIN_ALL),
    ],
    &[splay(&[Purple], Splay::Up)],
];

// You may return up to two cards from your hand. If you return two, draw a
// [10] and then draw and score a [10].
const QUANTUM_THEORY: [Program; 1] = [&[
    return_(HAND)
        .up_to(Some(Const(2)))
        .optional()
        .shortcut(ORIGIN),
    if_(&Cond::Did(Outcome::Exactly(2)), 3),
    draw(10),
    draw(10),
    score(LAST),
]];

// Return a card in any opponent's score pile for every color on your board
// with [Efficiency].
const ROCKETRY: [Program; 1] = [&[return_(ScoreOf(Opponent))
    .exactly(colors_with!(Efficiency))
    .shortcut(ORIGIN)]];

// Choose two colors, then draw and reveal a [9]. If the drawn card is one of
// those colors, meld it and splay your cards of its color up, otherwise
// unsplay that color. / If you have at least twenty [Concept] on your board,
// you win.
const EMPIRICISM: [Program; 2] = [
    &[
        choose(Executor, Choice::Colors(2)),
        draw(9),
        BIND_LAST,
        reveal(LAST),
        if_(&Card(Reg::Bound, Has(Test::ColorIn(ColorSet::Chosen))), 3),
        meld(BOUND),
        splay_set(ColorSet::Of(Reg::Bound), Splay::Up),
        Instruction::Jump(1),
        splay_set(ColorSet::Of(Reg::Bound), Splay::None),
    ],
    &[
        if_(
            &Compare(M(Executor, Icons(Concept)), Cmp::AtLeast, Const(20)),
            1,
        ),
        Instruction::Win(WinRule::Executor),
    ],
];

// You may tuck a top card from your board. If you do, tuck all cards from
// your hand. / You may junk an available achievement of value 8, 9, or 10.
const SOCIALISM: [Program; 2] = [
    &[
        tuck(TOPS).optional(),
        if_(DID, 1),
        tuck(HAND).all().shortcut(COLOR),
    ],
    &[junk(filtered(
        &AvailableAchievements,
        &[Has(Test::Values(&[8, 9, 10]))],
    ))
    .optional()],
];

// I DEMAND you transfer all but one card from your hand to my hand! Also
// transfer the highest card from your score pile to my score pile!
const COMPOSITES: [Program; 1] = [&[
    transfer(HAND, DEMANDER_HAND)
        .exactly(Add(&Count(&HAND), -1))
        .shortcut(ORIGIN),
    transfer(filtered(&SCORE, &[Highest]), DEMANDER_SCORE),
]];

// I DEMAND you draw a [10]! If it is red, junk each player's non-achievement
// cards, and the Dogma action is complete! / Return a top card other than
// Fission from any player's board. Draw a [10].
const FISSION: [Program; 2] = [
    &[
        draw(10),
        reveal(LAST),
        BIND_LAST,
        if_(&Card(Reg::Bound, Has(Test::Color(Red))), 2),
        junk(Selector::Owned(Either))
            .all()
            .shortcut(Shortcut::AnyOrder),
        Instruction::EndAction,
    ],
    &[
        return_(filtered(&Tops(Either), &[Lacks(Code("B087"))])),
        draw(10),
    ],
];

// You may return a card from your hand. If you do, score a card from your hand
// and draw two [10]. / You may junk all cards in the [10] deck.
const ECOLOGY: [Program; 2] = [
    &[
        return_(HAND).optional(),
        if_(DID, 3),
        score(HAND),
        draw(10),
        draw(10),
    ],
    &[junk(BaseDeck(Boxed(10)))
        .all()
        .optional()
        .shortcut(Shortcut::DeckTop)],
];

// You may tuck any number of cards from your hand. Draw and score a [1] for
// each card you tucked. / You may junk all cards in the [9] deck.
const SUBURBIA: [Program; 2] = [
    &[
        tuck(HAND).up_to(None).optional().shortcut(COLOR),
        draws(Expr::Done, 1, Then::Score),
    ],
    &[junk(BaseDeck(Boxed(9)))
        .all()
        .optional()
        .shortcut(Shortcut::DeckTop)],
];

// I DEMAND you draw two [9] and reveal them! Transfer the card of my choice to
// my board, and meld the other! / If you have at least ten green cards on your
// board, you win.
const COLLABORATION_DRAWN: Slot = Slot::effect(0);
const DRAWN_PAIR: Selector = filtered(&HAND, &[Has(Test::InSlot(COLLABORATION_DRAWN))]);
const COLLABORATION: [Program; 2] = [
    &[
        draw(9),
        reveal(LAST).keep(COLLABORATION_DRAWN),
        draw(9),
        reveal(LAST).add(COLLABORATION_DRAWN),
        choose(Demander, Choice::Card(&DRAWN_PAIR)),
        bind(filtered(&DRAWN_PAIR, &[Lacks(Test::Is(Reg::Chosen))])),
        transfer(
            filtered(&HAND, &[Has(Test::Is(Reg::Chosen))]),
            DEMANDER_BOARD,
        ),
        meld(BOUND),
    ],
    &[
        if_(
            &Compare(
                Count(&Board(
                    Executor,
                    ColorSet::Only(&[Green]),
                    BoardRegion::BelowTop(0),
                )),
                Cmp::AtLeast,
                Const(10),
            ),
            1,
        ),
        Instruction::Win(WinRule::Executor),
    ],
];

// Return all cards from your hand, and you may splay your purple cards up. /
// Draw three [8]. / Choose a card in your hand. Meld it, and self-execute it.
const SATELLITES: [Program; 3] = [
    &[
        return_(HAND).all().shortcut(ORIGIN_ALL),
        splay(&[Purple], Splay::Up),
    ],
    &[draw(8), draw(8), draw(8)],
    &[bind(HAND), meld(BOUND), SELF_EXECUTE],
];

// You may splay your red or green cards up. / Draw and meld a [10], then
// self-execute it.
const COMPUTERS: [Program; 2] = [
    &[splay(&[Red, Green], Splay::Up)],
    &[draw(10), BIND_LAST, meld(BOUND), SELF_EXECUTE],
];

// Draw and meld an [11]. Score all cards beneath it.
const GENETICS: [Program; 1] = [&[
    draw(11),
    BIND_LAST,
    meld(BOUND),
    score(Board(
        Either,
        ColorSet::Of(Reg::Bound),
        BoardRegion::BeneathBound,
    ))
    .all()
    .shortcut(ORIGIN_ALL),
]];

// I DEMAND you transfer all the cards of the value of my choice from your
// score pile to my hand! If you transfer any, transfer a top card without
// [Health] from my board to your hand!
const SERVICES: [Program; 1] = [&[
    choose(Demander, Choice::Value(ValueChoices::Any)).alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE)),
    transfer(CHOSEN_SCORE, DEMANDER_HAND)
        .all()
        .shortcut(ORIGIN_ALL),
    if_(DID, 1),
    transfer(
        filtered(&Tops(Demander), &[Lacks(HasIcon(Health))]),
        Destination::Hand(Executor),
    ),
]];

// Reveal a card from your hand. Take into your hand the top card of that
// color from all opponents' boards. / You may splay your yellow or blue cards
// up.
const SPECIALIZATION: [Program; 2] = [
    &[
        reveal(HAND),
        BIND_LAST,
        transfer(
            filtered(&Tops(Opponent), &[Has(Test::ColorOf(Reg::Bound))]),
            Destination::Hand(Executor),
        ),
    ],
    &[splay(&[Yellow, Blue], Splay::Up)],
];

// Score your top green card. / Draw and meld a [10], then self-execute it if
// it has [Industry] or [Efficiency].
const ROBOTICS: [Program; 2] = [
    &[score(TopOf(Executor, Green))],
    &[
        draw(10),
        BIND_LAST,
        meld(BOUND),
        if_(
            &Cond::Any(&[
                Card(Reg::Bound, Has(HasIcon(Industry))),
                Card(Reg::Bound, Has(HasIcon(Efficiency))),
            ]),
            1,
        ),
        SELF_EXECUTE,
    ],
];

// Return a card from your hand. If you return a [10], draw a [10] for every
// different value of card in your score pile. If you return an [11], junk all
// cards in the [11] deck.
const MINIATURIZATION: [Program; 1] = [&[
    return_(HAND),
    BIND_LAST,
    if_(&Card(Reg::Bound, Has(Test::BoxedValues(&[10]))), 1),
    draws(Expr::Distinct(&SCORE), 10, Then::Keep),
    if_(&Card(Reg::Bound, Has(Test::BoxedValues(&[11]))), 1),
    deck_junk(11),
]];

// I DEMAND you return a top card with [Health] from your board! / Draw and
// meld an [11]. If no player has fewer [Industry] than [Health] on their board,
// the single player with the most points wins.
const GLOBALIZATION: [Program; 2] = [
    &[return_(filtered(&TOPS, &[Has(HasIcon(Health))]))],
    &[
        draw(11),
        meld(LAST),
        if_(
            &Cond::All(&[
                Compare(
                    M(Executor, Icons(Health)),
                    Cmp::AtMost,
                    M(Executor, Icons(Industry)),
                ),
                Compare(
                    M(Opponent, Icons(Health)),
                    Cmp::AtMost,
                    M(Opponent, Icons(Industry)),
                ),
            ]),
            1,
        ),
        Instruction::Win(WinRule::Most(Metric::Score)),
    ],
];

// You may score all cards in your hand. / Draw an [11].
const STEM_CELLS: [Program; 2] = [
    &[score(HAND).all().optional().shortcut(ORIGIN_ALL)],
    &[draw(11)],
];

// I DEMAND you return a number of cards from your score pile equal to the
// value of your highest achievement!
const DATABASES: [Program; 1] = [&[return_(SCORE)
    .exactly(Max(&Selector::Achievements(Executor)))
    .shortcut(ORIGIN)]];

// If you have at least twice as many achievements as each opponent, you win.
// / Self-execute any top card other than Self Service on your board.
const SELF_SERVICE: [Program; 2] = [
    &[
        if_(
            &Compare(
                M(Executor, Achievements),
                Cmp::AtLeast,
                Expr::Mul(&M(Opponent, Achievements), 2),
            ),
            1,
        ),
        Instruction::Win(WinRule::Executor),
    ],
    &[
        bind(filtered(&TOPS, &[Lacks(Test::Is(Reg::Source))])),
        SELF_EXECUTE,
    ],
];

// Score a top card with [Health] from any opponent's board. / If any player
// has fewer than two [Health] on their board, the single player with the most
// [Health] on their board wins.
const BIOENGINEERING: [Program; 2] = [
    &[score(filtered(&Tops(Opponent), &[Has(HasIcon(Health))]))],
    &[
        if_(
            &Cond::Any(&[
                Compare(M(Executor, Icons(Health)), Cmp::Less, Const(2)),
                Compare(M(Opponent, Icons(Health)), Cmp::Less, Const(2)),
            ]),
            1,
        ),
        Instruction::Win(WinRule::Most(Icons(Health))),
    ],
];

// Draw and score a [10]. / Draw and meld two [9], then self-execute the second
// card.
const SOFTWARE: [Program; 2] = [
    &[draw(10), score(LAST)],
    &[
        draw(9),
        meld(LAST),
        draw(9),
        BIND_LAST,
        meld(BOUND),
        SELF_EXECUTE,
    ],
];

// Draw and score a [10]. / If Robotics and Software are top cards on any board,
// the single player with the lowest score wins.
const AI: [Program; 2] = [
    &[draw(10), score(LAST)],
    &[
        if_(
            &Cond::All(&[
                Compare(
                    Count(&filtered(&Tops(Either), &[Has(Code("B097"))])),
                    Cmp::AtLeast,
                    Const(1),
                ),
                Compare(
                    Count(&filtered(&Tops(Either), &[Has(Code("B103"))])),
                    Cmp::AtLeast,
                    Const(1),
                ),
            ]),
            1,
        ),
        Instruction::Win(WinRule::Fewest(Metric::Score)),
    ],
];

// You may splay your green cards up. / Draw and score a [10]. / Draw and meld
// two [10].
const THE_INTERNET: [Program; 3] = [
    &[splay(&[Green], Splay::Up)],
    &[draw(10), score(LAST)],
    &[draw(10), meld(LAST), draw(10), meld(LAST)],
];

// Draw and reveal an [11]. Splay its color on your board aslant. If you do,
// transfer all but your top four cards of that color into your hand. / If you
// have at least eight cards in your hand, you win.
const ASTROGEOLOGY: [Program; 2] = [
    &[
        draw(11),
        BIND_LAST,
        reveal(LAST),
        splay_set(ColorSet::Of(Reg::Bound), Splay::Aslant),
        if_(DID, 1),
        transfer(
            Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::BelowTop(4)),
            Destination::Hand(Executor),
        )
        .all()
        .shortcut(ORIGIN_ALL),
    ],
    &[
        if_(&Compare(Count(&HAND), Cmp::AtLeast, Const(8)), 1),
        Instruction::Win(WinRule::Executor),
    ],
];

// Score a top card of value 11 on your board. If you do, choose a value one or
// two lower than the scored card, then repeat this dogma effect using the
// chosen value.
const FUSION: [Program; 1] = [&[Instruction::Native(Native::Fusion)]];

// I DEMAND you choose a value! Transfer all cards of that value from your
// score pile to my score pile! / Reveal your highest score card, then
// self-execute it.
const NEAR_FIELD_COMM: [Program; 2] = [
    &[
        choose(Demander, Choice::Value(ValueChoices::Any)).alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE)),
        transfer(CHOSEN_SCORE, DEMANDER_SCORE)
            .all()
            .shortcut(ORIGIN_ALL),
    ],
    &[
        reveal(filtered(&SCORE, &[Highest])),
        BIND_LAST,
        SELF_EXECUTE,
    ],
];

// Return your three bottom red cards. Draw and meld a card of value equal to
// half the total sum value of the returned cards, rounded up. If you return
// three cards, repeat this effect using the color of the melded card.
const RECLAMATION: [Program; 1] = [&[Instruction::Native(Native::Reclamation)]];

// I DEMAND you return exactly two top cards of different colors from your
// board of the same value! If you do, return all cards of that value or less
// in your hand and score pile!
const HYPERSONICS: [Program; 1] = [&[
    return_(TOPS).group(2, GroupConstraint::SameValueDifferentColors),
    if_(&Cond::Did(Outcome::Exactly(2)), 1),
    return_(filtered(
        &Selector::Union(&[HAND, SCORE]),
        &[Has(ValueIs(Cmp::AtMost, Expr::Chosen))],
    ))
    .all()
    .shortcut(ORIGIN_ALL),
]];

// Draw and tuck an [11]. If you tuck directly under an [11], you lose.
// Otherwise, score all but your top five cards of the color of the tucked
// card, splay that color aslant, and if you do not have the highest score,
// repeat this effect.
const SPACE_TRAFFIC: [Program; 1] = [&[Instruction::Native(Native::SpaceTraffic)]];

// I DEMAND you return two top cards from your board each with the icon of my
// choice other than [Health]! / Return a top card on your board. Return all
// cards in your score pile of equal or higher value than the returned card.
const CLIMATOLOGY: [Program; 2] = [
    &[
        choose(
            Demander,
            Choice::Icon {
                except: Some(Health),
            },
        ),
        return_(filtered(&TOPS, &[Has(Test::ChosenIcon)]))
            .exactly(Const(2))
            .shortcut(ORIGIN),
    ],
    &[
        return_(TOPS),
        BIND_LAST,
        return_(filtered(
            &SCORE,
            &[Has(ValueIs(Cmp::AtLeast, Expr::Value(Reg::Bound)))],
        ))
        .all()
        .shortcut(ORIGIN_ALL),
    ],
];

// Draw and meld an [11]. If its color is not splayed aslant on your board,
// return all but your top four cards of that color, and splay that color
// aslant. If there are six or more cards of that color on your board, you win.
const SOLAR_SAILING: [Program; 1] = [&[
    draw(11),
    BIND_LAST,
    meld(BOUND),
    if_(
        &Cond::Splayed {
            who: Executor,
            colors: ColorSet::Of(Reg::Bound),
            direction: Splay::Aslant,
            present: false,
        },
        2,
    ),
    return_(Board(
        Executor,
        ColorSet::Of(Reg::Bound),
        BoardRegion::BelowTop(4),
    ))
    .all()
    .shortcut(ORIGIN),
    splay_set(ColorSet::Of(Reg::Bound), Splay::Aslant),
    if_(
        &Compare(
            Count(&Board(
                Executor,
                ColorSet::Of(Reg::Bound),
                BoardRegion::BelowTop(0),
            )),
            Cmp::AtLeast,
            Const(6),
        ),
        1,
    ),
    Instruction::Win(WinRule::Executor),
]];

// Reveal and junk a card in your hand. Return from your hand all cards of value
// equal to the value of the junked card. Draw three cards of that value.
// Self-execute the junked card.
const ESCAPISM: [Program; 1] = [&[
    reveal(HAND),
    junk(LAST),
    if_(DID, 6),
    BIND_LAST,
    return_(filtered(
        &HAND,
        &[Has(ValueIs(Cmp::Equal, Expr::Value(Reg::Bound)))],
    ))
    .all()
    .shortcut(ORIGIN),
    draw_age(Expr::Value(Reg::Bound)),
    draw_age(Expr::Value(Reg::Bound)),
    draw_age(Expr::Value(Reg::Bound)),
    SELF_EXECUTE,
]];

// I DEMAND you transfer all your top cards with a demand effect to my board,
// one per color! If you transfer any, exchange all cards in your score pile
// with all cards in my score pile!
const WHATABOUTISM: [Program; 1] = [&[
    transfer(filtered(&TOPS, &[Has(Test::Demand)]), DEMANDER_BOARD)
        .per_color(None)
        .shortcut(Shortcut::AnyOrder),
    if_(DID_ANY, 1),
    exchange(SCORE, ScoreOf(Demander), &[None, None]),
]];

/// The printed effects of a Base card as (demand, program) pairs, or `None`
/// for a card that is not yet authored.
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
        "B001" => &const { demand(ARCHERY) },
        "B002" => &const { plain(METALWORKING) },
        "B003" => &const { demand(OARS) },
        "B004" => &const { plain(AGRICULTURE) },
        "B005" => &const { plain(DOMESTICATION) },
        "B006" => &const { plain(MASONRY) },
        "B007" => &const { plain(CLOTHING) },
        "B008" => &const { plain(SAILING) },
        "B009" => &const { plain(THE_WHEEL) },
        "B010" => &const { plain(POTTERY) },
        "B011" => &const { plain(TOOLS) },
        "B012" => &const { plain(WRITING) },
        "B013" => &const { demand(CITY_STATES) },
        "B014" => &const { plain(CODE_OF_LAWS) },
        "B015" => &const { plain(MYSTICISM) },
        "B016" => &const { demand(CONSTRUCTION) },
        "B017" => &const { plain(ROAD_BUILDING) },
        "B018" => &const { plain(CANAL_BUILDING) },
        "B019" => &const { plain(FERMENTING) },
        "B020" => &const { plain(CURRENCY) },
        "B021" => &const { demand(MAPMAKING) },
        "B022" => &const { plain(CALENDAR) },
        "B023" => &const { plain(MATHEMATICS) },
        "B024" => &const { demand(MONOTHEISM) },
        "B025" => &const { plain(PHILOSOPHY) },
        "B026" => &const { demand(ENGINEERING) },
        "B027" => &const { plain(OPTICS) },
        "B028" => &const { demand(MACHINERY) },
        "B029" => &const { demand(MEDICINE) },
        "B030" => &const { demand(COMPASS) },
        "B031" => &const { plain(PAPER) },
        "B032" => &const { plain(ALCHEMY) },
        "B033" => &const { plain(TRANSLATION) },
        "B034" => &const { plain(EDUCATION) },
        "B035" => &const { demand(FEUDALISM) },
        "B036" => &const { plain(COLONIALISM) },
        "B037" => &const { demand(GUNPOWDER) },
        "B038" => &const { demand(ANATOMY) },
        "B039" => &const { plain(PERSPECTIVE) },
        "B040" => &const { plain(INVENTION) },
        "B041" => &const { demand(NAVIGATION) },
        "B042" => &const { plain(EXPERIMENTATION) },
        "B043" => &const { plain(PRINTING_PRESS) },
        "B044" => &const { demand(ENTERPRISE) },
        "B045" => &const { plain(REFORMATION) },
        "B046" => &const { plain(COAL) },
        "B047" => &const { demand(THE_PIRATE_CODE) },
        "B048" => &const { demand(STATISTICS) },
        "B049" => &const { plain(STEAM_ENGINE) },
        "B050" => &const { demand(BANKING) },
        "B051" => &const { plain(MEASUREMENT) },
        "B052" => &const { plain(CHEMISTRY) },
        "B053" => &const { plain(PHYSICS) },
        "B054" => &const { plain(ASTRONOMY) },
        "B055" => &const { demand(SOCIETIES) },
        "B056" => &const { plain(INDUSTRIALIZATION) },
        "B057" => &const { plain(MACHINE_TOOLS) },
        "B058" => &const { plain(CANNING) },
        "B059" => &const { demand(VACCINATION) },
        "B060" => &const { plain(CLASSIFICATION) },
        "B061" => &const { plain(METRIC_SYSTEM) },
        "B062" => &const { plain(ATOMIC_THEORY) },
        "B063" => &const { plain(ENCYCLOPEDIA) },
        "B064" => &const { plain(DEMOCRACY) },
        "B065" => &const { demand(EMANCIPATION) },
        "B066" => &const { demand(COMBUSTION) },
        "B067" => &const { demand(EXPLOSIVES) },
        "B068" => &const { demand(REFRIGERATION) },
        "B069" => &const { demand(SANITATION) },
        "B070" => &const { plain(BICYCLE) },
        "B071" => &const { plain(ELECTRICITY) },
        "B072" => &const { plain(EVOLUTION) },
        "B073" => &const { plain(PUBLICATIONS) },
        "B074" => &const { plain(LIGHTING) },
        "B075" => &const { plain(RAILROAD) },
        "B076" => &const { plain(FLIGHT) },
        "B077" => &const { demand(MOBILITY) },
        "B078" => &const { plain(ANTIBIOTICS) },
        "B079" => &const { demand(SKYSCRAPERS) },
        "B080" => &const { demand(CORPORATIONS) },
        "B081" => &const { plain(MASS_MEDIA) },
        "B082" => &const { plain(QUANTUM_THEORY) },
        "B083" => &const { plain(ROCKETRY) },
        "B084" => &const { plain(EMPIRICISM) },
        "B085" => &const { plain(SOCIALISM) },
        "B086" => &const { demand(COMPOSITES) },
        "B087" => &const { demand(FISSION) },
        "B088" => &const { plain(ECOLOGY) },
        "B089" => &const { plain(SUBURBIA) },
        "B090" => &const { demand(COLLABORATION) },
        "B091" => &const { plain(SATELLITES) },
        "B092" => &const { plain(COMPUTERS) },
        "B093" => &const { plain(GENETICS) },
        "B094" => &const { demand(SERVICES) },
        "B095" => &const { plain(SPECIALIZATION) },
        "B096" => &const { plain(MINIATURIZATION) },
        "B097" => &const { plain(ROBOTICS) },
        "B098" => &const { demand(GLOBALIZATION) },
        "B099" => &const { plain(STEM_CELLS) },
        "B100" => &const { demand(DATABASES) },
        "B101" => &const { plain(SELF_SERVICE) },
        "B102" => &const { plain(BIOENGINEERING) },
        "B103" => &const { plain(SOFTWARE) },
        "B104" => &const { plain(AI) },
        "B105" => &const { plain(THE_INTERNET) },
        "B106" => &const { plain(ASTROGEOLOGY) },
        "B107" => &const { plain(FUSION) },
        "B108" => &const { demand(NEAR_FIELD_COMM) },
        "B109" => &const { plain(RECLAMATION) },
        "B110" => &const { demand(HYPERSONICS) },
        "B111" => &const { plain(SPACE_TRAFFIC) },
        "B112" => &const { demand(CLIMATOLOGY) },
        "B113" => &const { plain(SOLAR_SAILING) },
        "B114" => &const { plain(ESCAPISM) },
        "B115" => &const { demand(WHATABOUTISM) },
        _ => return None,
    })
}
