use crate::model::{CityIcon, EffectKind};
use crate::program::build::*;

use crate::achievements::Special;

use Color::{Blue, Green, Purple, Red, Yellow};
use Cond::{All, Any, Card, Compare, DemandMoved, Did, Empty, Not, OwnTurn, Splayed};
use Expr::{
    Add, Const, Count, DistinctColors, IconTypesAmong, IconsAmong, Max, Metric as M, Sum, Total,
    Value, Visible,
};
use Filter::{Has, Highest, Lacks, Lowest};
use Icon::{Authority, Avatar, Concept, Efficiency, Food, Health, Industry, Prosperity};
use Selector::{AvailableAchievements, BaseDeck, Board, StandardAchievements, TopOf, Tops};
use Test::{
    Code, Color as ColorIs, ColorIn, ColorOf, ColorOnBoard, Icon as HasIcon, InSlot, Is,
    SharesIconWith, StackShows, StackSplayed, Value as ValueIs, ValueCount, ValueIn,
};
use Who::{Demander, Either, Executor, Opponent};

type Program = &'static [Instruction];

const D: EffectKind = EffectKind::Demand;
const N: EffectKind = EffectKind::NonDemand;

const SAFE: Selector = Selector::Safe(Executor);
const SELF_EXECUTE: Instruction = execute(ExecutionMode::SelfExecute, Executor);
const SUPER_EXECUTE: Instruction = execute(ExecutionMode::SuperExecute, Executor);
const WIN: Instruction = Instruction::Win(WinRule::Executor);
const LOSE: Instruction = Instruction::Win(WinRule::Loses(Executor));
const FLIP: Instruction = Instruction::Flip;
const REPEAT: Instruction = Instruction::Repeat { keep: 0 };
const ALL_COLORS: ColorSet = ColorSet::Only(&Color::ALL);
const ORIGIN_ALL: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: false,
};
const ORIGIN: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: true,
};

/// The cards an instruction moved.
const MOVED: Slot = Slot::effect(0);
/// The cards an instruction drew, or the cards an effect revealed.
const DRAWN: Slot = Slot::effect(1);
const REVEALED: Slot = Slot::effect(1);
/// A block's item: a color each pass.
const ITEM: Slot = Slot::effect(2);
/// Score members nobody can inspect (Wernher von Braun's junk), revealed as
/// the completeness proof before a front-filtered score selection, as in
/// Bandage. Naming grants no access to cards outside that score pile.
const PROOF: Slot = Slot::effect(2);
const VERIFIED_SCORE: Selector = Selector::Union(&[filtered(&SCORE, &[Has(InSlot(PROOF))]), SCORE]);
/// A color chosen once and kept across repeats.
const COLOR: Slot = Slot::effect(0);
/// Legend: the points a player scored due to Legend during this action.
const LEGEND_POINTS: Slot = Slot::action(3);
/// Legend: the card it just scored.
const LEGEND_LAST: Slot = Slot::effect(1);
/// Pilgrimage's rising value, kept across repeats.
const VALUE: Slot = Slot::effect(0);
/// Polytheism: the cards a player melded due to it during this action.
const POLYTHEISM_MELDED: Slot = Slot::action(3);
/// Brethren of Purity: the cards melded due to it this action, by anyone;
/// each meld is recorded for both players (`of(Either)`).
const BRETHREN_MELDED: Slot = Slot::action(3);
/// One higher than the last card melded due to Brethren of Purity, or a [1]
/// before any.
const BRETHREN_NEXT: Expr = Add(&Max(&Selector::Reg(Reg::Slot(BRETHREN_MELDED))), 1);
/// Cards returned by a later instruction of the same effect.
const RETURNED: Slot = Slot::effect(2);
/// The Prophecies' reference secret, kept apart from the drawn card.
const REFERENCE: Slot = Slot::effect(2);
const HAND_AND_SCORE: Selector = Selector::Union(&[HAND, SCORE]);
const DRAWN_IN_HAND: Selector = filtered(&HAND, &[Has(InSlot(DRAWN))]);
const DEMANDER_HAND: Selector = Selector::Hand(Demander);
const ALL_OF: BoardRegion = BoardRegion::BelowTop(0);
/// The executor has no cards of the chosen colour: moving them all changes
/// nothing, so every such colour is the same choice.
const NO_CARDS_OF_CHOSEN_COLOR: Cond = Compare(
    Count(&Board(Executor, ColorSet::Chosen, ALL_OF)),
    Cmp::Equal,
    Const(0),
);
/// The executor has no top card of the chosen colour.
const NO_TOP_OF_CHOSEN_COLOR: Cond = Compare(
    Count(&Board(Executor, ColorSet::Chosen, BoardRegion::Top)),
    Cmp::Equal,
    Const(0),
);
/// The number of splayed colors on the executor's board.
const SPLAYED_COLORS: Expr = Count(&filtered(&TOPS, &[Has(StackSplayed(None))]));

/// "Splay your X and Y cards...": every pile of the set.
const fn splay_all(colors: ColorSet, direction: Splay) -> Instruction {
    Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors,
            direction,
            from: None,
        },
        targets: SplayTargets::Changes,
        optional: false,
        all: true,
    })
}

/// "You may splay..." one color of the set.
const fn may_splay(colors: ColorSet, direction: Splay) -> Instruction {
    Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors,
            direction,
            from: None,
        },
        targets: SplayTargets::Changes,
        optional: true,
        all: false,
    })
}

/// "You may splay left..." one color of the set.
const fn may_splay_left(colors: ColorSet) -> Instruction {
    may_splay(colors, Splay::Left)
}

const fn same_value(value: Expr) -> Filter {
    Has(ValueIs(Cmp::Equal, value))
}

const fn exchange(left: Selector, right: Selector) -> Instruction {
    Instruction::Exchange(ExchangeStep {
        left,
        right,
        counts: &[None, None],
        optional: false,
    })
}

// ----------------------------------------------------------------- Age 1

// I DEMAND you draw and reveal a [1]! If it has [Authority], transfer it and
// the top card on your board of its color to my score pile! / If no player
// has a top green card, claim the Confidence achievement.
const ASSASSINATION: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            draw(1),
            reveal(LAST).keep(DRAWN),
            if_(&Card(Reg::Slot(DRAWN), Has(HasIcon(Authority))), 2),
            transfer(DRAWN_IN_HAND, Destination::Score(Demander)),
            // The matching top is read live: the drawn card is in hand.
            transfer(
                Board(Executor, ColorSet::Of(Reg::Slot(DRAWN)), BoardRegion::Top),
                Destination::Score(Demander),
            ),
        ],
    ),
    (
        N,
        &[
            if_(
                &Compare(
                    Count(&filtered(&Tops(Either), &[Has(ColorIs(Green))])),
                    Cmp::Equal,
                    Const(0),
                ),
                1,
            ),
            claim(Special::Confidence),
        ],
    ),
];

// I DEMAND you score a card from your hand of matching color for each card
// in my hand! If you don't, and I have a card in my hand, exchange all cards
// in your hand with all cards in my score pile!
const MAZE: &[(EffectKind, Program)] = &[(
    D,
    &[
        // The demanding hand sets the color quota, so it is shown (proof).
        reveal_all(DEMANDER_HAND),
        for_each(Domain::Cards(&DEMANDER_HAND), Order::Printed, Some(ITEM), 3),
        score(filtered(&HAND, &[Has(ColorOf(Reg::Slot(ITEM)))])).verify(),
        if_(&Not(&Did(Outcome::All)), 1),
        reveal_all(filtered(&HAND, &[Lacks(InSlot(REVEALED))])).add(REVEALED),
        if_(
            &All(&[
                Not(&Did(Outcome::All)),
                Compare(Count(&DEMANDER_HAND), Cmp::AtLeast, Const(1)),
            ]),
            1,
        ),
        exchange(HAND, Selector::Score(Demander)),
    ],
)];

// You may return a card of value 1 from your hand. If you do, safeguard an
// available achievement of value equal to the returned card, then repeat
// this effect using a value one higher. / You may junk all cards in the [1]
// deck.
const PILGRIMAGE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            if_(&Empty(VALUE), 1),
            Instruction::Bind(Binding::Value(VALUE, Const(1))),
            return_(filtered(&HAND, &[same_value(Expr::Slot(VALUE))])).optional(),
            if_(&Did(Outcome::All), 3),
            safeguard(filtered(
                &AvailableAchievements,
                &[same_value(Expr::Slot(VALUE))],
            )),
            Instruction::Bind(Binding::Value(VALUE, Add(&Expr::Slot(VALUE), 1))),
            repeat_keeping(VALUE),
        ],
    ),
    (
        N,
        &[junk(BaseDeck(Expr::Boxed(1)))
            .all()
            .optional()
            .shortcut(Shortcut::DeckTop)],
    ),
];

// I DEMAND you transfer all cards from my hand to your hand! Choose two
// colors of cards in your hand! Transfer all cards in your hand of those
// colors to my hand!
const HANDSHAKE: &[(EffectKind, Program)] = &[(
    D,
    &[
        transfer(DEMANDER_HAND, Destination::Hand(Executor))
            .all()
            .shortcut(ORIGIN_ALL)
            .keep(MOVED),
        // The victim's own cards are revealed before the colors are chosen;
        // the demander's arrivals are already known.
        reveal_all(filtered(&HAND, &[Lacks(InSlot(MOVED))])).keep(PROOF),
        choose(Executor, Choice::ColorsAmong(2, ColorSet::OfCards(&HAND))),
        transfer(
            filtered(&HAND, &[Has(ColorIn(ColorSet::Chosen))]),
            Destination::Hand(Demander),
        )
        .all()
        .shortcut(ORIGIN_ALL),
        // A card that arrived unseen meanwhile is shown too.
        reveal_all(filtered(
            &HAND,
            &[Lacks(InSlot(MOVED)), Lacks(InSlot(PROOF))],
        )),
    ],
)];

// Safeguard an available achievement of value 1 plus the number of
// achievements you have. / You may transfer the lowest available achievement
// to your hand. If you do, return all yellow cards and all blue cards on your
// board.
const TOMB: &[(EffectKind, Program)] = &[
    (
        N,
        &[safeguard(filtered(
            &AvailableAchievements,
            &[same_value(Add(&M(Executor, Metric::Achievements), 1))],
        ))],
    ),
    (
        N,
        &[
            transfer(
                filtered(&AvailableAchievements, &[Lowest]),
                Destination::Hand(Executor),
            )
            .optional(),
            if_(&Did(Outcome::All), 1),
            return_(Board(Executor, ColorSet::Only(&[Yellow, Blue]), ALL_OF))
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    ),
];

// Draw and meld a [2]. If the melded card is a bottom card on your board,
// score it.
const WOODWORKING: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(2),
        meld(LAST).keep(MOVED),
        if_(
            &Compare(
                Count(&filtered(
                    &Board(Executor, ALL_COLORS, BoardRegion::Bottom),
                    &[Has(InSlot(MOVED))],
                )),
                Cmp::AtLeast,
                Const(1),
            ),
            1,
        ),
        score(Selector::Slot(MOVED)),
    ],
)];

// Transfer a top card on your board with [Authority] to the board of any
// other player. If you do, meld the lowest top card without [Authority] from
// that player's board.
const DANCE: &[(EffectKind, Program)] = &[(
    N,
    &[
        transfer(
            filtered(&TOPS, &[Has(HasIcon(Authority))]),
            Destination::Board(Opponent),
        ),
        if_(&Did(Outcome::All), 1),
        meld(filtered(
            &Tops(Opponent),
            &[Lacks(HasIcon(Authority)), Lowest],
        )),
    ],
)];

// Return a card from your score pile. If you do, draw a card of value one
// higher than the card you return. / Transfer a card from your hand to the
// hand of the player on your left.
const RUMOR: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            return_(SCORE),
            if_(&Did(Outcome::All), 1),
            draw_age(Add(&Value(Reg::Last), 1)),
        ],
    ),
    (N, &[transfer(HAND, Destination::Hand(Opponent))]),
];

/// Silk scored something but fewer cards than colors on the board.
const SILK_COLOR_UNSCORED: Cond = All(&[
    Compare(Expr::Done, Cmp::Greater, Const(0)),
    Compare(Expr::Done, Cmp::Less, Count(&TOPS)),
]);

// Meld a card from your hand. / You may score a card from your hand of each
// color on your board.
const SILK: &[(EffectKind, Program)] = &[
    (N, &[meld(HAND)]),
    (
        N,
        &[
            score(filtered(&HAND, &[Has(ColorOnBoard(Executor))]))
                .per_color(None)
                .optional()
                .verify()
                .shortcut(ORIGIN),
            // All or nothing once accepted: a board color left unscored is proven
            // absent by the remaining hand.
            if_(&SILK_COLOR_UNSCORED, 1),
            reveal_all(HAND),
        ],
    ),
];

/// The demander's hand cards of the revealed card's color.
const ESPIONAGE_MATCHES: Selector = filtered(&DEMANDER_HAND, &[Has(ColorOf(Reg::Slot(REVEALED)))]);
const ESPIONAGE_NO_MATCH: Cond = Compare(Count(&ESPIONAGE_MATCHES), Cmp::Equal, Const(0));

// I DEMAND you reveal a card in your hand! If you do, and I have no card in
// my hand of the same color, transfer it to my hand, then repeat this
// effect!
const ESPIONAGE: &[(EffectKind, Program)] = &[(
    D,
    &[
        reveal(HAND).keep(REVEALED),
        if_(&Did(Outcome::All), 5),
        // The demander shows one matching card as the witness that stops
        // the transfer; with none to show, their hand proves the absence
        // unless it is already known. The victim never filters that hand.
        reveal(ESPIONAGE_MATCHES).by(Demander),
        if_(&Not(&Did(Outcome::All)), 3),
        reveal_all(DEMANDER_HAND)
            .unless_proven(&ESPIONAGE_NO_MATCH)
            .by(Demander),
        transfer(
            filtered(&HAND, &[Has(InSlot(REVEALED))]),
            Destination::Hand(Demander),
        ),
        REPEAT,
    ],
)];

// Draw and meld a [1]. / Return two cards from your hand. If you do, draw and
// score a [2].
const PALMISTRY: &[(EffectKind, Program)] = &[
    (N, &[draw(1), meld(LAST)]),
    (
        N,
        &[
            return_(HAND).exactly(Const(2)).shortcut(ORIGIN).keep(MOVED),
            if_(
                &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(2)),
                2,
            ),
            draw(2),
            score(LAST),
        ],
    ),
];

// Draw, reveal, and return a [1]. If the color of the returned card is yellow
// or purple, safeguard an available achievement of value equal to a card in
// your hand, then return all cards from your hand. Otherwise, draw two [1].
const PROVERB: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(1),
        reveal(LAST).keep(DRAWN),
        return_(DRAWN_IN_HAND),
        if_(
            &Not(&Card(
                Reg::Slot(DRAWN),
                Has(ColorIn(ColorSet::Only(&[Yellow, Purple]))),
            )),
            3,
        ),
        draw(1),
        draw(1),
        Instruction::Jump(2),
        safeguard(filtered(&AvailableAchievements, &[Has(ValueIn(&HAND))])),
        return_(HAND).all().shortcut(ORIGIN_ALL),
    ],
)];

// If you have two cards of the same color in your hand, tuck them both. If
// you do, splay left that color, and draw and safeguard a card of value
// equal to the value of your bottom card of that color.
const MYTH: &[(EffectKind, Program)] = &[(
    N,
    &[
        tuck(HAND).group(2, GroupConstraint::SameColor).keep(MOVED),
        if_(&Did(Outcome::All), 4),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Left),
        draw_age(Max(&Board(
            Executor,
            ColorSet::Of(Reg::Slot(MOVED)),
            BoardRegion::Bottom,
        ))),
        safeguard(LAST),
        Instruction::Jump(2),
        // No pair: with two or more cards, the hand proves it.
        if_(&Compare(Count(&HAND), Cmp::AtLeast, Const(2)), 1),
        reveal_all(HAND),
    ],
)];

// Meld a card from your hand with no icon on a card already melded by you
// during this action due to Polytheism. If you do, repeat this effect. / Draw
// and tuck a [1].
const POLYTHEISM: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            meld(filtered(
                &HAND,
                &[Lacks(SharesIconWith(&Selector::Slot(POLYTHEISM_MELDED)))],
            ))
            .reveal_if_empty(&HAND)
            .add(POLYTHEISM_MELDED),
            if_(&Did(Outcome::All), 1),
            REPEAT,
        ],
    ),
    (N, &[draw(1), tuck(LAST)]),
];

// If you have at least four each of at least four icons on your board, draw a
// [4]. Otherwise, if you have three of three icons, draw a [3]. Otherwise, if
// you have two of two icons, draw a [2].
const SYMBOLOGY: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&icon_types(4), 2),
        draw(4),
        Instruction::Jump(6),
        if_(&icon_types(3), 2),
        draw(3),
        Instruction::Jump(2),
        if_(&icon_types(2), 1),
        draw(2),
    ],
)];

/// At least `n` each of at least `n` icon types on the executor's board.
const fn icon_types(n: u8) -> Cond {
    Compare(
        M(Executor, Metric::IconTypesAtLeast(n)),
        Cmp::AtLeast,
        Const(n as i64),
    )
}

// ----------------------------------------------------------------- Age 2

// I DEMAND you transfer one of your secrets to the available achievements!
// / If no card was transferred due to the demand, you may score up to three
// cards from your hand of different values.
const PADLOCK: &[(EffectKind, Program)] = &[
    (D, &[transfer(SAFE, Destination::Supply)]),
    (
        N,
        &[
            if_(&Not(&DemandMoved(Verb::Transfer)), 2),
            choose(
                Executor,
                Choice::Cards {
                    from: &HAND,
                    up_to: 3,
                    constraint: Some(GroupConstraint::DifferentValues),
                },
            )
            .optional()
            .keep(MOVED),
            score(filtered(&HAND, &[Has(InSlot(MOVED))]))
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    ),
];

// Draw and reveal a [2]. You may safeguard another card from your hand of
// the color of the drawn card. If you do, score the drawn card. Otherwise,
// return all cards from your hand except the drawn card.
const PASSWORD: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(2),
        reveal(LAST).keep(DRAWN),
        safeguard(filtered(
            &HAND,
            &[Lacks(InSlot(DRAWN)), Has(ColorOf(Reg::Slot(DRAWN)))],
        ))
        .optional()
        .verify(),
        if_(&Did(Outcome::All), 2),
        score(DRAWN_IN_HAND),
        Instruction::Jump(1),
        return_(filtered(&HAND, &[Lacks(InSlot(DRAWN))]))
            .all()
            .shortcut(ORIGIN_ALL),
    ],
)];

// I DEMAND you return a top card without [Health] from your board! Return
// all cards of the returned card's value from your score pile! / If exactly
// one card was returned due to the demand, return Exile if it is a top card
// on any board and draw a [3].
const EXILE: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            return_(filtered(&TOPS, &[Lacks(HasIcon(Health))])).keep(MOVED),
            return_(filtered(&SCORE, &[same_value(Value(Reg::Slot(MOVED)))]))
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    ),
    (
        N,
        &[
            if_(
                &Compare(Expr::DemandMoved(Verb::Return), Cmp::Equal, Const(1)),
                2,
            ),
            return_(filtered(&Tops(Either), &[Has(Code("U018"))])),
            draw(3),
        ],
    ),
];

// You may splay your red or yellow cards left. / Safeguard an available
// achievement of value equal to the number of splayed colors on your board.
// Transfer a card of that value in your hand to any board.
const FINGERPRINTS: &[(EffectKind, Program)] = &[
    (N, &[splay(&[Red, Yellow], Splay::Left)]),
    (
        N,
        &[
            safeguard(filtered(
                &AvailableAchievements,
                &[same_value(SPLAYED_COLORS)],
            )),
            // The card first, then the board: no board choice without one.
            choose(
                Executor,
                Choice::Card(&filtered(&HAND, &[same_value(SPLAYED_COLORS)])),
            )
            .keep(MOVED),
            if_(&Not(&Empty(MOVED)), 4),
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Transfer to your board"),
                    (2, "Transfer to the other board"),
                ]),
            ),
            transfer(Selector::Slot(MOVED), Destination::Board(Executor)),
            Instruction::Jump(1),
            transfer(Selector::Slot(MOVED), Destination::Board(Opponent)),
        ],
    ),
];

// Return all cards from your hand. If you return at least two, draw a card of
// value one higher than the highest value of card you return. / Draw a [2].
// You may splay your blue cards left.
const CIPHER: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            return_(HAND).all().shortcut(ORIGIN_ALL).keep(MOVED),
            if_(
                &Compare(Count(&Selector::Slot(MOVED)), Cmp::AtLeast, Const(2)),
                1,
            ),
            draw_age(Add(&Max(&Selector::Slot(MOVED)), 1)),
        ],
    ),
    (N, &[draw(2), splay(&[Blue], Splay::Left)]),
];

// Score a top card from your board of a value not in your score pile. If you
// do, repeat this effect. / You may splay your green or purple cards left.
const COUNTERFEITING: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            score(filtered(&TOPS, &[Lacks(ValueIn(&SCORE))])),
            if_(&Did(Outcome::All), 1),
            REPEAT,
        ],
    ),
    (N, &[splay(&[Green, Purple], Splay::Left)]),
];

// You may splay left the color of which you have the most cards on your
// board. / Draw and meld a card of value equal to the number of visible
// purple cards on your board. If the melded card has no [Prosperity], tuck
// it.
const ASTROLOGY: &[(EffectKind, Program)] = &[
    (N, &[may_splay_left(ColorSet::MostCards(Executor))]),
    (
        N,
        &[
            draw_age(Expr::Visible(&Visibility {
                who: Executor,
                colors: ColorSet::Only(&[Purple]),
                seen: Seen::Cards,
            })),
            meld(LAST).keep(MOVED),
            if_(&Card(Reg::Slot(MOVED), Lacks(HasIcon(Prosperity))), 1),
            tuck(Selector::Slot(MOVED)),
        ],
    ),
];

// Draw and reveal a [3]. If it has [Health], score it. Otherwise, if it has
// [Prosperity], return it and draw two [3]. Otherwise, tuck it. / If you have
// no [Authority], claim the Zen achievement.
const METEOROLOGY: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            draw(3),
            reveal(LAST).keep(DRAWN),
            if_(&Card(Reg::Slot(DRAWN), Has(HasIcon(Health))), 2),
            score(DRAWN_IN_HAND),
            Instruction::Jump(6),
            if_(&Card(Reg::Slot(DRAWN), Has(HasIcon(Prosperity))), 4),
            return_(DRAWN_IN_HAND),
            draw(3),
            draw(3),
            Instruction::Jump(1),
            tuck(DRAWN_IN_HAND),
        ],
    ),
    (
        N,
        &[
            if_(
                &Compare(M(Executor, Metric::Icons(Authority)), Cmp::Equal, Const(0)),
                1,
            ),
            claim(Special::Zen),
        ],
    ),
];

// I DEMAND you meld a card of the color of my choice from your hand! If you
// do, transfer the card beneath it to my board! / Meld a card from your
// hand.
const PROPAGANDA: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            choose(Demander, Choice::Colors(1)),
            meld(filtered(&HAND, &[Has(ColorIn(ColorSet::Chosen))]))
                .reveal_if_empty(&HAND)
                .keep(MOVED),
            if_(&Did(Outcome::All), 2),
            bind(Selector::Reg(Reg::Slot(MOVED))),
            transfer(
                Board(
                    Executor,
                    ColorSet::Of(Reg::Bound),
                    BoardRegion::JustBeneathBound,
                ),
                Destination::Board(Demander),
            ),
        ],
    ),
    (N, &[meld(HAND)]),
];

// You may splay left a color on your board with [Concept]. If you do,
// safeguard an available achievement of value equal to the number of cards
// of that color on your board. Otherwise, draw and tuck a [3].
const STEGANOGRAPHY: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Color(ColorSet::OfCards(&filtered(
                &TOPS,
                &[Has(StackShows(Concept))],
            ))),
        )
        .optional()
        .keep(COLOR),
        splay_set(ColorSet::Slot(COLOR), Splay::Left),
        if_(&Did(Outcome::All), 2),
        safeguard(filtered(
            &AvailableAchievements,
            &[same_value(Count(&Board(
                Executor,
                ColorSet::Slot(COLOR),
                ALL_OF,
            )))],
        )),
        Instruction::Jump(2),
        draw(3),
        tuck(LAST),
    ],
)];

// ----------------------------------------------------------------- Age 3

/// The splayed colors on the executor's board.
const SPLAYED: Selector = filtered(&TOPS, &[Has(StackSplayed(None))]);

// I DEMAND you unsplay a splayed color on your board! If you do, transfer
// the top card on your board of that color to my score pile! / You may splay
// your red or green cards left.
const KNIGHTS_TEMPLAR: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            choose(Executor, Choice::Color(ColorSet::OfCards(&SPLAYED))).keep(COLOR),
            splay_set(ColorSet::Slot(COLOR), Splay::None),
            if_(&Did(Outcome::All), 1),
            transfer(
                Board(Executor, ColorSet::Slot(COLOR), BoardRegion::Top),
                Destination::Score(Demander),
            ),
        ],
    ),
    (N, &[splay(&[Red, Green], Splay::Left)]),
];

/// Hand and score cards whose value occurs two or three times there.
const ENVELOPE_VALUES: Selector = filtered(
    &HAND_AND_SCORE,
    &[
        Has(ValueCount(&HAND_AND_SCORE, Cmp::AtLeast, 2)),
        Has(ValueCount(&HAND_AND_SCORE, Cmp::AtMost, 3)),
    ],
);

// Choose a value of which you have exactly two or three cards altogether in
// your hand and score pile. Transfer those cards to the score pile of the
// player on your right. / You may score exactly two or three cards from your
// hand.
const RED_ENVELOPE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(
                Executor,
                Choice::Value(ValueChoices::Among(&ENVELOPE_VALUES)),
            ),
            transfer(
                filtered(&HAND_AND_SCORE, &[same_value(Expr::Chosen)]),
                Destination::Score(Opponent),
            )
            .all()
            .shortcut(ORIGIN_ALL),
        ],
    ),
    (
        N,
        &[
            // Three only when possible: with two cards it would be a second
            // way to decline.
            if_(&Compare(Count(&HAND), Cmp::AtLeast, Const(3)), 5),
            choose(
                Executor,
                Choice::Branch(&[(0, "Score two cards"), (2, "Score three cards")]),
            )
            .optional(),
            score(HAND).exactly(Const(2)).shortcut(ORIGIN),
            Instruction::Jump(5),
            score(HAND).exactly(Const(3)).shortcut(ORIGIN),
            Instruction::Jump(3),
            if_(&Compare(Count(&HAND), Cmp::Equal, Const(2)), 2),
            choose(Executor, Choice::Branch(&[(0, "Score two cards")])).optional(),
            score(HAND).exactly(Const(2)).shortcut(ORIGIN),
        ],
    ),
];

/// Tucked cards that are yellow or from an expansion.
const FREEMASON_TRIGGERS: Selector = Selector::Union(&[
    filtered(&Selector::Slot(MOVED), &[Has(ColorIs(Yellow))]),
    filtered(&Selector::Slot(MOVED), &[Lacks(Test::Set(CardSet::Base))]),
]);

// For each color, you may tuck a card from your hand of that color. If you
// tuck any yellow cards or expansion cards, draw two [3]. / You may splay
// your yellow or blue cards left.
const FREEMASONS: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            // "For each color, you may tuck": any one card per color, in
            // any order; under Blackmail one of every color in hand.
            if_(&Cond::Must, 2),
            tuck(HAND)
                .per_color(None)
                .shortcut(Shortcut::ByColor)
                .add(MOVED),
            Instruction::Jump(1),
            tuck(HAND)
                .any_per_color()
                .shortcut(Shortcut::ByColor)
                .add(MOVED),
            if_(
                &Compare(Count(&FREEMASON_TRIGGERS), Cmp::AtLeast, Const(1)),
                2,
            ),
            draw(3),
            draw(3),
        ],
    ),
    (N, &[splay(&[Yellow, Blue], Splay::Left)]),
];

// I DEMAND you tuck a card in your hand, then return your top card of its
// color! If you do, repeat this effect! Otherwise, draw a [3]! / You may tuck
// any number of cards of any one color from your hand.
const SECRET_POLICE: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            once(2),
            tuck(HAND).keep(MOVED),
            return_(Board(
                Executor,
                ColorSet::Of(Reg::Slot(MOVED)),
                BoardRegion::Top,
            )),
            if_(&Did(Outcome::All), 1),
            REPEAT,
            draw(3),
        ],
    ),
    (
        N,
        &[
            choose(Executor, Choice::Color(ColorSet::OfCards(&HAND)))
                .optional()
                .keep(COLOR),
            tuck(filtered(&HAND, &[Has(ColorIn(ColorSet::Slot(COLOR)))]))
                .up_to(None)
                .optional(),
        ],
    ),
];

// Reveal a [4] in your safe. If it is: green, tuck it; purple, meld it; red,
// achieve it regardless of eligibility; yellow, score it; blue, draw a [5].
// Otherwise, safeguard the top card of the [4] deck.
const CLIFFHANGER: &[(EffectKind, Program)] = &[(
    N,
    &[
        reveal(filtered(&SAFE, &[same_value(Const(4))])).keep(DRAWN),
        if_(&Not(&Did(Outcome::All)), 2),
        safeguard(Selector::BaseDeckTop(Expr::Boxed(4))),
        Instruction::Jump(13),
        if_(&Card(Reg::Slot(DRAWN), Has(ColorIs(Green))), 2),
        tuck(Selector::Slot(DRAWN)),
        Instruction::Jump(10),
        if_(&Card(Reg::Slot(DRAWN), Has(ColorIs(Purple))), 2),
        meld(Selector::Slot(DRAWN)),
        Instruction::Jump(7),
        if_(&Card(Reg::Slot(DRAWN), Has(ColorIs(Red))), 2),
        achieve(Selector::Slot(DRAWN)),
        Instruction::Jump(4),
        if_(&Card(Reg::Slot(DRAWN), Has(ColorIs(Yellow))), 2),
        score(Selector::Slot(DRAWN)),
        Instruction::Jump(1),
        draw(5),
    ],
)];

// I DEMAND you transfer a card of value equal to your top yellow card and a
// card of value equal to my top yellow card from your score pile to my score
// pile!
const SMUGGLING: &[(EffectKind, Program)] = &[(
    D,
    &[
        transfer(
            filtered(&SCORE, &[same_value(Max(&TopOf(Executor, Yellow)))]),
            Destination::Score(Demander),
        ),
        transfer(
            filtered(&SCORE, &[same_value(Max(&TopOf(Demander, Yellow)))]),
            Destination::Score(Demander),
        ),
    ],
)];

// Draw and meld a [3] or a card of value one higher than the last card melded due to
// Brethren of Purity during this action. If you meld over a card with a [Concept],
// repeat this effect. The last meld is the action's, whoever executed: a sharer's meld
// counts for the owner. With no such meld yet, one higher is a [1]. The choice is
// skipped only when both requests draw from the same deck.
const BRETHREN_OF_PURITY: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&Not(&Cond::SameDraw(Expr::Boxed(3), BRETHREN_NEXT)), 1),
        choose(
            Executor,
            Choice::Branch(&[
                (0, "Draw and meld a [3]"),
                (
                    2,
                    "Draw and meld one higher than the last Brethren meld (a [1] if none)",
                ),
            ]),
        ),
        draw(3),
        Instruction::Jump(1),
        draw_age(BRETHREN_NEXT),
        meld(LAST).add(BRETHREN_MELDED.of(Either)),
        bind(Selector::Reg(Reg::Slot(BRETHREN_MELDED))),
        if_(
            &Compare(
                Count(&filtered(
                    &Board(
                        Executor,
                        ColorSet::Of(Reg::Bound),
                        BoardRegion::JustBeneathBound,
                    ),
                    &[Has(HasIcon(Concept))],
                )),
                Cmp::AtLeast,
                Const(1),
            ),
            1,
        ),
        REPEAT,
    ],
)];

// Return five cards from your hand and/or score pile. Draw two cards of
// value equal to the number of different colors of cards you return. Meld one
// of the drawn cards and score the other.
const SECRETUM_SECRETORUM: &[(EffectKind, Program)] = &[(
    N,
    &[
        // The returned colors set the draw value, so each one is shown.
        return_(HAND_AND_SCORE)
            .exactly(Const(5))
            .verify()
            .shortcut(ORIGIN)
            .keep(MOVED),
        draws_age(Const(2), DistinctColors(&Selector::Slot(MOVED)), Then::Keep).keep(DRAWN),
        meld(DRAWN_IN_HAND),
        score(DRAWN_IN_HAND),
    ],
)];

// Safeguard an available achievement of value equal to the number of cards in
// your hand. If you do, return all cards of that value from your hand. If you
// return a [4], claim the Anonymity achievement. / You may splay your purple
// cards left.
const MASQUERADE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            safeguard(filtered(
                &AvailableAchievements,
                &[same_value(Count(&HAND))],
            ))
            .keep(MOVED),
            if_(&Did(Outcome::All), 3),
            return_(filtered(&HAND, &[same_value(Value(Reg::Slot(MOVED)))]))
                .all()
                .shortcut(ORIGIN_ALL)
                .keep(RETURNED),
            if_(
                &Compare(
                    Count(&filtered(
                        &Selector::Slot(RETURNED),
                        &[Has(Test::BoxedValues(&[4]))],
                    )),
                    Cmp::AtLeast,
                    Const(1),
                ),
                1,
            ),
            claim(Special::Anonymity),
        ],
    ),
    (N, &[splay(&[Purple], Splay::Left)]),
];

// Choose a color. Transfer all cards of that color on your board into your
// hand. / Draw and meld a [3]. If the melded card is a bottom card on your
// board, score it and any number of cards of its color in your hand.
const TAQIYYA: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(Executor, Choice::Colors(1)).alias(Alias::NoOpWhen(&NO_CARDS_OF_CHOSEN_COLOR)),
            transfer(
                Board(Executor, ColorSet::Chosen, ALL_OF),
                Destination::Hand(Executor),
            )
            .all()
            .shortcut(ORIGIN_ALL),
        ],
    ),
    (
        N,
        &[
            draw(3),
            meld(LAST).keep(MOVED),
            if_(
                &Compare(
                    Count(&filtered(
                        &Board(Executor, ALL_COLORS, BoardRegion::Bottom),
                        &[Has(InSlot(MOVED))],
                    )),
                    Cmp::AtLeast,
                    Const(1),
                ),
                2,
            ),
            score(Selector::Slot(MOVED)),
            score(filtered(&HAND, &[Has(ColorOf(Reg::Slot(MOVED)))]))
                .up_to(None)
                .verify()
                .shortcut(ORIGIN),
        ],
    ),
];

// ----------------------------------------------------------------- Age 4

// I DEMAND you return a card of the color of my choice from your hand! If
// you do, transfer the top card of that color from your board to mine! / You
// may splay your red cards right.
const NINJA: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            choose(Demander, Choice::Colors(1)),
            return_(filtered(&HAND, &[Has(ColorIn(ColorSet::Chosen))]))
                .verify()
                .reveal_if_empty(&HAND),
            if_(&Did(Outcome::All), 1),
            transfer(
                Board(Executor, ColorSet::Chosen, BoardRegion::Top),
                Destination::Board(Demander),
            ),
        ],
    ),
    (N, &[splay(&[Red], Splay::Right)]),
];

// I DEMAND you return all but the highest cards from your hand and all but
// the highest cards from your score pile! / If Spanish Inquisition is a top
// card on your board, return all red cards from your board.
const SPANISH_INQUISITION: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            return_(filtered(&HAND, &[Has(ValueIs(Cmp::Less, Max(&HAND)))]))
                .all()
                .shortcut(ORIGIN_ALL),
            return_(filtered(&SCORE, &[Has(ValueIs(Cmp::Less, Max(&SCORE)))]))
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    ),
    (
        N,
        &[
            if_(
                &Compare(
                    Count(&filtered(&TOPS, &[Has(Code("U037"))])),
                    Cmp::AtLeast,
                    Const(1),
                ),
                1,
            ),
            return_(Board(Executor, ColorSet::Only(&[Red]), ALL_OF))
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    ),
];

// Transfer a card from your hand or score pile to the board of the player on
// your right. If you don't, claim the Folklore achievement. / Splay your
// yellow cards right, and unsplay your purple cards, or vice versa.
const APRIL_FOOLS_DAY: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            transfer(HAND_AND_SCORE, Destination::Board(Opponent)),
            if_(&Not(&Did(Outcome::All)), 1),
            claim(Special::Folklore),
        ],
    ),
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Splay yellow right and unsplay purple"),
                    (3, "Splay purple right and unsplay yellow"),
                ]),
            ),
            splay_set(ColorSet::Only(&[Yellow]), Splay::Right),
            splay_set(ColorSet::Only(&[Purple]), Splay::None),
            Instruction::Jump(2),
            splay_set(ColorSet::Only(&[Purple]), Splay::Right),
            splay_set(ColorSet::Only(&[Yellow]), Splay::None),
        ],
    ),
];

/// The melded cards showing Prosperity.
const PROSPEROUS_MELDS: Selector = filtered(&Selector::Slot(MOVED), &[Has(HasIcon(Prosperity))]);

// Draw and meld a [3], a [2], and a [1]. If all three cards have
// [Prosperity], score all cards in the [5] deck. If at least two have
// [Prosperity], splay your green and blue cards right.
const EL_DORADO: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(3),
        meld(LAST).add(MOVED),
        draw(2),
        meld(LAST).add(MOVED),
        draw(1),
        meld(LAST).add(MOVED),
        if_(&Compare(Count(&PROSPEROUS_MELDS), Cmp::Equal, Const(3)), 1),
        score(BaseDeck(Expr::Boxed(5)))
            .all()
            .shortcut(Shortcut::DeckTop),
        if_(
            &Compare(Count(&PROSPEROUS_MELDS), Cmp::AtLeast, Const(2)),
            1,
        ),
        splay_all(ColorSet::Only(&[Green, Blue]), Splay::Right),
    ],
)];

// Choose to either score a card from your hand, or draw a [4]. / Return
// exactly two cards in your hand. If you do, draw a card of value equal to
// the sum number of [Health] and [Concept] on the returned cards.
const QUACKERY: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[(0, "Score a card from your hand"), (2, "Draw a [4]")]),
            ),
            score(HAND),
            Instruction::Jump(1),
            draw(4),
        ],
    ),
    (
        N,
        &[
            // "Exactly two" needs two cards in hand.
            if_(&Compare(Count(&HAND), Cmp::AtLeast, Const(2)), 3),
            return_(HAND)
                .exactly(Const(2))
                .verify()
                .shortcut(ORIGIN)
                .keep(MOVED),
            if_(
                &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(2)),
                1,
            ),
            draw_age(Sum(&[
                IconsAmong(&Selector::Slot(MOVED), Health),
                IconsAmong(&Selector::Slot(MOVED), Concept),
            ])),
        ],
    ),
];

// Choose to either draw and safeguard a [4], or draw and reveal a card of
// value one higher than one of your secrets. If you reveal a red or purple
// card, meld one of your other secrets. If you do, safeguard the drawn card.
// The reference secret is revealed in place and excluded from "other".
const THE_PROPHECIES: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Branch(&[
                (0, "Draw and safeguard a [4]"),
                (3, "Draw and reveal a card one higher than a secret"),
            ]),
        ),
        draw(4),
        safeguard(LAST),
        Instruction::Jump(11),
        // With no secret, draw and reveal a 0, as Heirloom draws a 0.
        if_(&Compare(Count(&SAFE), Cmp::Equal, Const(0)), 3),
        draw_age(Const(0)),
        reveal(LAST),
        Instruction::Jump(7),
        reveal(SAFE).keep(REFERENCE),
        draw_age(Add(&Value(Reg::Slot(REFERENCE)), 1)),
        reveal(LAST).keep(DRAWN),
        if_(
            &Card(
                Reg::Slot(DRAWN),
                Has(ColorIn(ColorSet::Only(&[Red, Purple]))),
            ),
            3,
        ),
        meld(filtered(&SAFE, &[Lacks(InSlot(REFERENCE))])),
        if_(&Did(Outcome::All), 1),
        safeguard(DRAWN_IN_HAND),
    ],
)];

// Return a top card with [Authority] of each color from your board. If you
// return none, meld a card from your score pile, then draw and score a [4].
// / Draw a [4] for each [4] in your score pile.
const CONFESSION: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            return_(filtered(&TOPS, &[Has(HasIcon(Authority))]))
                .per_color(None)
                .shortcut(ORIGIN_ALL)
                .keep(MOVED),
            if_(
                &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(0)),
                3,
            ),
            meld(SCORE),
            draw(4),
            score(LAST),
        ],
    ),
    (
        N,
        &[draws(
            Count(&filtered(&SCORE, &[Has(Test::BoxedValues(&[4]))])),
            4,
            Then::Keep,
        )],
    ),
];

// Transfer one of your secrets to the available achievements and draw a card
// of value one higher than the transferred card. If you don't, safeguard an
// available achievement of value equal to the value of your top red card.
const HEIRLOOM: &[(EffectKind, Program)] = &[(
    N,
    &[
        transfer(SAFE, Destination::Supply).keep(MOVED),
        // With no transfer, draw a 0, which never matches.
        if_(
            &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(0)),
            2,
        ),
        draw_age(Const(0)),
        Instruction::Jump(2),
        draw_age(Add(&Max(&Selector::Slot(MOVED)), 1)),
        // "If you don't": the drawn card is not exactly one higher.
        if_(
            &Not(&Compare(
                Expr::Value(Reg::Last),
                Cmp::Equal,
                Add(&Max(&Selector::Slot(MOVED)), 1),
            )),
            1,
        ),
        // No top red card: value zero, which no achievement has.
        safeguard(filtered(
            &AvailableAchievements,
            &[same_value(Max(&TopOf(Executor, Red)))],
        )),
    ],
)];

// I DEMAND you reveal your hand! Meld a revealed card of my choice! Reveal
// your score pile! Self-execute a card revealed due to this effect of my
// choice, replacing 'may' with 'must'!
const BLACKMAIL: &[(EffectKind, Program)] = &[(
    D,
    &[
        reveal_all(HAND).keep(REVEALED),
        choose(
            Demander,
            Choice::Card(&filtered(&HAND, &[Has(InSlot(REVEALED))])),
        ),
        meld(filtered(&HAND, &[Has(Is(Reg::Chosen))])),
        reveal_all(SCORE).add(REVEALED),
        // Any card this effect revealed, wherever it is now, the melded
        // card included; the victim executes it under must.
        choose(Demander, Choice::Card(&Selector::Slot(REVEALED))),
        bind(Selector::Reg(Reg::Chosen)),
        SELF_EXECUTE.must(),
    ],
)];

// Choose a non-purple color. Self-execute your top card of that color. Score
// your top card of that color. If you do, repeat this effect with the same
// color if you have scored fewer than nine points due to Legend during this
// action.
const LEGEND: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&Empty(COLOR), 1),
        choose(
            Executor,
            Choice::Color(ColorSet::Only(&[Red, Blue, Green, Yellow])),
        )
        .alias(Alias::NoOpWhen(&NO_TOP_OF_CHOSEN_COLOR))
        .keep(COLOR),
        bind(Board(Executor, ColorSet::Slot(COLOR), BoardRegion::Top)),
        SELF_EXECUTE,
        // Only Legend's own scores count, not the executed card's, and a
        // card scored again counts again: the points are a running total.
        score(Board(Executor, ColorSet::Slot(COLOR), BoardRegion::Top)).keep(LEGEND_LAST),
        if_(&Did(Outcome::All), 5),
        if_(&Empty(LEGEND_POINTS), 1),
        Instruction::Bind(Binding::Value(LEGEND_POINTS, Const(0))),
        Instruction::Bind(Binding::Value(
            LEGEND_POINTS,
            Sum(&[
                Expr::Slot(LEGEND_POINTS),
                Total(&Selector::Slot(LEGEND_LAST)),
            ]),
        )),
        if_(&Compare(Expr::Slot(LEGEND_POINTS), Cmp::Less, Const(9)), 1),
        repeat_keeping(COLOR),
    ],
)];

// ----------------------------------------------------------------- Age 5

// I DEMAND you transfer all cards from your hand that have a value matching
// any of my secrets to my score pile! Draw a [5]! / Safeguard an available
// achievement of value equal to a top card on your board.
// Values are public backs, so the sweep needs no proof reveal.
const CABAL: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            transfer(
                filtered(&HAND, &[Has(ValueIn(&Selector::Safe(Demander)))]),
                Destination::Score(Demander),
            )
            .all()
            .shortcut(ORIGIN_ALL),
            draw(5),
        ],
    ),
    (
        N,
        &[safeguard(filtered(
            &AvailableAchievements,
            &[Has(ValueIn(&TOPS))],
        ))],
    ),
];

// I DEMAND you draw and reveal a [5]! Return your top card of the color of
// the drawn card, another card of that color from your hand, and a card from
// your score pile! If you do, repeat this effect!
const WITCH_TRIAL: &[(EffectKind, Program)] = &[(
    D,
    &[
        draw(5),
        reveal(LAST).keep(DRAWN),
        once(3),
        return_(Board(
            Executor,
            ColorSet::Of(Reg::Slot(DRAWN)),
            BoardRegion::Top,
        )),
        return_(filtered(
            &HAND,
            &[Has(ColorOf(Reg::Slot(DRAWN))), Lacks(InSlot(DRAWN))],
        ))
        .verify()
        .reveal_if_empty(&HAND),
        return_(SCORE),
        if_(&Did(Outcome::All), 1),
        REPEAT,
    ],
)];

/// A [2], or a [1], in the score pile.
const SCORED_TWO: Cond = Compare(
    Count(&filtered(&SCORE, &[Has(Test::BoxedValues(&[2]))])),
    Cmp::AtLeast,
    Const(1),
);
const SCORED_ONE: Cond = Compare(
    Count(&filtered(&SCORE, &[Has(Test::BoxedValues(&[1]))])),
    Cmp::AtLeast,
    Const(1),
);

// If you have a [2] in your score pile, draw a [6]. / If you have a [1] in
// your score pile, draw a [7]. Otherwise, draw a [5].
const GALLERY: &[(EffectKind, Program)] = &[
    (N, &[if_(&SCORED_TWO, 1), draw(6)]),
    (
        N,
        &[if_(&SCORED_ONE, 2), draw(7), Instruction::Jump(1), draw(5)],
    ),
];

// Choose an odd value. Transfer all cards of that value from all score piles
// to the available achievements. If you transfer at least four cards, draw
// and safeguard a card of that value, and score three available standard
// achievements.
/// Neither score pile holds a card of the chosen value (backs show values):
/// nothing to transfer.
const NO_CHOSEN_SCORE_EITHER: Cond = Compare(
    Count(&filtered(
        &Selector::Score(Either),
        &[same_value(Expr::Chosen)],
    )),
    Cmp::Equal,
    Const(0),
);
const BURIED_TREASURE: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Value(ValueChoices::Odd))
            .alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE_EITHER)),
        // Every match goes to the unordered supply: any order is the same.
        transfer(
            filtered(&Selector::Score(Either), &[same_value(Expr::Chosen)]),
            Destination::Supply,
        )
        .all()
        .shortcut(Shortcut::AnyOrder)
        .keep(MOVED),
        if_(
            &Compare(Count(&Selector::Slot(MOVED)), Cmp::AtLeast, Const(4)),
            3,
        ),
        draw_age(Expr::Chosen),
        safeguard(LAST),
        // Which three is a real choice; only their order is not.
        score(StandardAchievements)
            .exactly(Const(3))
            .shortcut(ORIGIN),
    ],
)];

const RED_RIGHT: Cond = Splayed {
    who: Executor,
    colors: ColorSet::Only(&[Red]),
    direction: Splay::Right,
    present: true,
};
const PURPLE_RIGHT: Cond = Splayed {
    who: Executor,
    colors: ColorSet::Only(&[Purple]),
    direction: Splay::Right,
    present: true,
};

// I DEMAND you transfer one of your secrets to my safe! / If your red and
// purple cards are splayed right, claim the Mystery achievement. Otherwise,
// splay your red or purple cards right.
const SECRET_HISTORY: &[(EffectKind, Program)] = &[
    (D, &[transfer(SAFE, Destination::Safe(Demander))]),
    (
        N,
        &[
            if_(&All(&[RED_RIGHT, PURPLE_RIGHT]), 2),
            claim(Special::Mystery),
            Instruction::Jump(1),
            splay_set(ColorSet::Only(&[Red, Purple]), Splay::Right),
        ],
    ),
];

const GREEN_TOPS: Selector = filtered(&Tops(Either), &[Has(ColorIs(Green))]);

// Draw and meld a card of value equal to the value of a top green card
// anywhere. / Draw and meld a card of value one higher than the value of
// your top yellow card. / You may splay your blue cards right.
const POPULAR_SCIENCE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            // No top green card has value 0: draw a 0.
            if_(&Compare(Count(&GREEN_TOPS), Cmp::Equal, Const(0)), 2),
            draw_age(Const(0)),
            Instruction::Jump(2),
            choose(Executor, Choice::Value(ValueChoices::Among(&GREEN_TOPS))),
            draw_age(Expr::Chosen),
            meld(LAST),
        ],
    ),
    (
        N,
        &[draw_age(Add(&Max(&TopOf(Executor, Yellow)), 1)), meld(LAST)],
    ),
    (N, &[splay(&[Blue], Splay::Right)]),
];

const fn drawn_icon_types(count: i64) -> Cond {
    Compare(
        IconTypesAmong(&Selector::Slot(DRAWN)),
        Cmp::Equal,
        Const(count),
    )
}

// Return all cards from your hand. / Draw and reveal two [6], then return
// them. If exactly two different icon types appear on the drawn cards, draw
// and score two [6]. If exactly four different icon types appear, draw a
// [7]. Draw a [6].
const PROBABILITY: &[(EffectKind, Program)] = &[
    (N, &[return_(HAND).all().shortcut(ORIGIN_ALL)]),
    (
        N,
        &[
            draw(6),
            reveal(LAST).add(DRAWN),
            draw(6),
            reveal(LAST).add(DRAWN),
            return_(DRAWN_IN_HAND).all().shortcut(ORIGIN_ALL),
            if_(&drawn_icon_types(2), 4),
            draw(6),
            score(LAST),
            draw(6),
            score(LAST),
            if_(&drawn_icon_types(4), 1),
            draw(7),
            draw(6),
        ],
    ),
];

// Tuck a card from your hand. If you do, draw and tuck a [4], score all cards
// on your board of the color of one of the tucked cards, and splay right the
// color on your board of the other tucked card. / Draw and tuck a [4].
const PANTHEISM: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            tuck(HAND).keep(MOVED),
            if_(&Did(Outcome::All), 5),
            draw(4),
            tuck(LAST).add(MOVED),
            choose(Executor, Choice::Card(&Selector::Slot(MOVED))),
            score(Board(Executor, ColorSet::Of(Reg::Chosen), ALL_OF))
                .all()
                .shortcut(ORIGIN_ALL),
            splay_set(
                ColorSet::OfCards(&filtered(&Selector::Slot(MOVED), &[Lacks(Is(Reg::Chosen))])),
                Splay::Right,
            ),
        ],
    ),
    (N, &[draw(4), tuck(LAST)]),
];

/// Unsplayed non-purple colors on the executor's board.
const PEN_NAME_COLORS: Selector = filtered(
    &TOPS,
    &[Has(StackSplayed(Some(Splay::None))), Lacks(ColorIs(Purple))],
);

// Choose to either splay an unsplayed non-purple color on your board left
// and self-execute its top card, or meld a card from your hand and splay its
// color on your board right.
const PEN_NAME: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Branch(&[
                (
                    0,
                    "Splay an unsplayed non-purple color left and self-execute its top",
                ),
                (5, "Meld a card from your hand and splay its color right"),
            ]),
        ),
        choose(Executor, Choice::Color(ColorSet::OfCards(&PEN_NAME_COLORS))).keep(COLOR),
        splay_set(ColorSet::Slot(COLOR), Splay::Left),
        bind(Board(Executor, ColorSet::Slot(COLOR), BoardRegion::Top)),
        SELF_EXECUTE,
        Instruction::Jump(2),
        meld(HAND).keep(MOVED),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Right),
    ],
)];

// Draw and reveal a [3], a [4], a [5], and a [6]. Meld each drawn green card
// and each drawn yellow card, in any order. Return the other drawn cards. /
// You may splay your green or yellow cards right.
const CHARTREUSE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            draw(3),
            reveal(LAST).add(DRAWN),
            draw(4),
            reveal(LAST).add(DRAWN),
            draw(5),
            reveal(LAST).add(DRAWN),
            draw(6),
            reveal(LAST).add(DRAWN),
            meld(filtered(
                &DRAWN_IN_HAND,
                &[Has(ColorIn(ColorSet::Only(&[Green, Yellow])))],
            ))
            .all()
            .shortcut(Shortcut::ByColor),
            return_(DRAWN_IN_HAND).all().shortcut(ORIGIN_ALL),
        ],
    ),
    (N, &[splay(&[Green, Yellow], Splay::Right)]),
];

// ----------------------------------------------------------------- Age 6

/// The tucked card's pile and the red pile share this splay.
macro_rules! same_as_red {
    ($name:ident, $direction:expr) => {
        const $name: Cond = All(&[
            Splayed {
                who: Executor,
                colors: ColorSet::Only(&[Red]),
                direction: $direction,
                present: true,
            },
            Splayed {
                who: Executor,
                colors: ColorSet::Of(Reg::Slot(MOVED)),
                direction: $direction,
                present: true,
            },
        ]);
    };
}
same_as_red!(BOTH_LEFT, Splay::Left);
same_as_red!(BOTH_RIGHT, Splay::Right);
same_as_red!(BOTH_UP, Splay::Up);
same_as_red!(BOTH_ASLANT, Splay::Aslant);
// Unsplayed is not a direction: two unsplayed colors do not match.
const SAME_AS_RED: Cond = Any(&[BOTH_LEFT, BOTH_RIGHT, BOTH_UP, BOTH_ASLANT]);

const fn red_option(direction: Splay) -> SplayOption {
    SplayOption {
        colors: ColorSet::Only(&[Red]),
        direction: SplayDirection::Fixed(direction),
        from: None,
    }
}

// Splay your red cards left, right, or up. / Draw and tuck a [6]. If the
// color on your board of the card you tuck is splayed in the same direction
// as your red cards, splay that color up. Otherwise, unsplay that color.
const RED_HERRING: &[(EffectKind, Program)] = &[
    (
        N,
        &[Instruction::Splay(SplayStep {
            who: Executor,
            choices: SplayChoices::Options(&[
                red_option(Splay::Left),
                red_option(Splay::Right),
                red_option(Splay::Up),
            ]),
            // The current direction is a legal no-op.
            targets: SplayTargets::Nonempty,
            optional: false,
            all: false,
        })],
    ),
    (
        N,
        &[
            draw(6),
            tuck(LAST).keep(MOVED),
            if_(&SAME_AS_RED, 2),
            splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Up),
            Instruction::Jump(1),
            splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::None),
        ],
    ),
];

// I DEMAND you unsplay the color on your board of my choice! Meld your
// bottom card of that color! Transfer your bottom non-top card of that color
// to my board!
const SNIPING: &[(EffectKind, Program)] = &[(
    D,
    &[
        choose(Demander, Choice::Color(ColorSet::OfCards(&TOPS))),
        splay_set(ColorSet::Chosen, Splay::None),
        meld(Board(Executor, ColorSet::Chosen, BoardRegion::Bottom)),
        if_(
            &Compare(
                Count(&Board(Executor, ColorSet::Chosen, ALL_OF)),
                Cmp::AtLeast,
                Const(2),
            ),
            1,
        ),
        transfer(
            Board(Executor, ColorSet::Chosen, BoardRegion::Bottom),
            Destination::Board(Demander),
        ),
    ],
)];

// You may score or safeguard a card from your hand. / Return a card from
// your score pile. / Draw and score a card of value equal to a card in your
// score pile.
const ATTIC: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Score a card from your hand"),
                    (2, "Safeguard a card from your hand"),
                ]),
            )
            .optional(),
            score(HAND),
            Instruction::Jump(1),
            safeguard(HAND),
        ],
    ),
    (N, &[return_(SCORE)]),
    (
        N,
        &[
            // With an empty score pile, draw a 0.
            if_(&Compare(Count(&SCORE), Cmp::Equal, Const(0)), 2),
            draw_age(Const(0)),
            Instruction::Jump(2),
            choose(Executor, Choice::Value(ValueChoices::Among(&SCORE))),
            draw_age(Expr::Chosen),
            score(LAST),
        ],
    ),
];

// I DEMAND you draw a [6]! Reveal the cards in your hand! Return the card of
// my choice from your hand! Tuck your top card and all cards from your score
// pile of the same color as the returned card!
const SABOTAGE: &[(EffectKind, Program)] = &[(
    D,
    &[
        draw(6),
        reveal_all(HAND).keep(REVEALED),
        choose(
            Demander,
            Choice::Card(&filtered(&HAND, &[Has(InSlot(REVEALED))])),
        ),
        return_(filtered(&HAND, &[Has(Is(Reg::Chosen))])).keep(MOVED),
        tuck(Board(
            Executor,
            ColorSet::Of(Reg::Slot(MOVED)),
            BoardRegion::Top,
        )),
        // The whole score pile proves which cards share the color.
        reveal_all(SCORE).keep(PROOF),
        tuck(filtered(&VERIFIED_SCORE, &[Has(ColorOf(Reg::Slot(MOVED)))])).all(),
    ],
)];

// Draw and reveal a [6]. If the top card on your board of the drawn card's
// color has [Industry], tuck the drawn card and draw and reveal a [7]. If the
// second drawn card has [Health], meld it and draw an [8].
const HIKING: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(6),
        reveal(LAST).keep(DRAWN),
        if_(
            &Compare(
                Count(&filtered(
                    &Board(Executor, ColorSet::Of(Reg::Slot(DRAWN)), BoardRegion::Top),
                    &[Has(HasIcon(Industry))],
                )),
                Cmp::AtLeast,
                Const(1),
            ),
            6,
        ),
        tuck(DRAWN_IN_HAND),
        draw(7),
        reveal(LAST).keep(DRAWN),
        if_(&Card(Reg::Slot(DRAWN), Has(HasIcon(Health))), 2),
        meld(DRAWN_IN_HAND),
        draw(8),
    ],
)];

// Safeguard an available achievement of value equal to the number of cards
// in your score pile. If you do, score all cards in your hand of its value. /
// Draw a [6] for each secret in your safe.
const SWISS_BANK_ACCOUNT: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            safeguard(filtered(
                &AvailableAchievements,
                &[same_value(Count(&SCORE))],
            ))
            .keep(MOVED),
            if_(&Did(Outcome::All), 1),
            score(filtered(&HAND, &[same_value(Value(Reg::Slot(MOVED)))]))
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    ),
    (N, &[draws(Count(&SAFE), 6, Then::Keep)]),
];

// Return a top card on your board, then you may repeat as many times as you
// want with the same color. Draw a [7] for each card you return. If you
// return exactly one [7], draw an [8].
// Each pass returns one top card; the returned cards accumulate in MOVED.
const PLACEBO_NEXT: Selector = Board(Executor, ColorSet::Of(Reg::Slot(MOVED)), BoardRegion::Top);
const PLACEBO: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&Empty(MOVED), 2),
        return_(TOPS).keep(MOVED),
        Instruction::Jump(6),
        if_(&Compare(Count(&PLACEBO_NEXT), Cmp::AtLeast, Const(1)), 7),
        // The first repeat is the "may": Blackmail's `must` requires it. Later ones are
        // a quantity it keeps.
        if_(
            &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(1)),
            2,
        ),
        return_(PLACEBO_NEXT).add(MOVED).optional(),
        Instruction::Jump(2),
        choose(
            Executor,
            Choice::Branch(&[(0, "Return another card of that color"), (3, "Stop")]),
        ),
        return_(PLACEBO_NEXT).add(MOVED),
        if_(&Did(Outcome::All), 1),
        repeat_keeping(MOVED),
        draws(Count(&Selector::Slot(MOVED)), 7, Then::Keep),
        if_(
            &Compare(
                Count(&filtered(
                    &Selector::Slot(MOVED),
                    &[Has(Test::BoxedValues(&[7]))],
                )),
                Cmp::Equal,
                Const(1),
            ),
            1,
        ),
        draw(8),
    ],
)];

// I DEMAND you reveal your hand! / Draw and reveal three [7]. Return two of
// the drawn cards. You may splay the color of the card not returned right.
const RECONNAISSANCE: &[(EffectKind, Program)] = &[
    (D, &[reveal_all(HAND)]),
    (
        N,
        &[
            draw(7),
            reveal(LAST).add(DRAWN),
            draw(7),
            reveal(LAST).add(DRAWN),
            draw(7),
            reveal(LAST).add(DRAWN),
            return_(DRAWN_IN_HAND).exactly(Const(2)).shortcut(ORIGIN),
            may_splay(ColorSet::OfCards(&DRAWN_IN_HAND), Splay::Right),
        ],
    ),
];

// Reveal a card in your hand. Splay the card's color on your board right. Safeguard the
// top card on your board of that color. Safeguard an available achievement of value one
// higher than the secret. A failed first safeguard leaves no secret to reference.
const ILLUMINATI: &[(EffectKind, Program)] = &[(
    N,
    &[
        reveal(HAND).keep(REVEALED),
        splay_set(ColorSet::Of(Reg::Slot(REVEALED)), Splay::Right),
        safeguard(Board(
            Executor,
            ColorSet::Of(Reg::Slot(REVEALED)),
            BoardRegion::Top,
        ))
        .keep(MOVED),
        safeguard(filtered(
            &AvailableAchievements,
            &[same_value(Add(&Value(Reg::Slot(MOVED)), 1))],
        )),
    ],
)];

// If you have at least three cards in your hand, return a card from your
// hand and splay the color of the returned card right, tuck a card from your
// hand, and score a card from your hand.
const TRIAD: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&Compare(Count(&HAND), Cmp::AtLeast, Const(3)), 4),
        // Its color picks the splay, so it is shown.
        return_(HAND).verify().keep(MOVED),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Right),
        tuck(HAND),
        score(HAND),
    ],
)];

// ----------------------------------------------------------------- Age 7

const JUNKED: Selector = filtered(&Selector::Junk, &[Has(InSlot(MOVED))]);

// Choose to either junk exactly two top cards of different colors and equal
// value on your board, then safeguard them, or score exactly two of your
// secrets of equal value. / Draw a [7] for each special achievement you have.
const CAMOUFLAGE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[
                    (
                        0,
                        "Junk two top cards of equal value and different colors, then safeguard them",
                    ),
                    (3, "Score two secrets of equal value"),
                ]),
            ),
            junk(TOPS)
                .group(2, GroupConstraint::SameValueDifferentColors)
                .keep(MOVED)
                .shortcut(Shortcut::AnyOrder),
            safeguard(JUNKED).all(),
            Instruction::Jump(1),
            score(SAFE).group(2, GroupConstraint::SameValue),
        ],
    ),
    (N, &[draws(M(Executor, Metric::Specials), 7, Then::Keep)]),
];

// You may choose to either draw and junk two [7], or exchange all cards in
// your score pile with all valued cards in the junk.
const SAFE_DEPOSIT_BOX: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Branch(&[
                (0, "Draw and junk two [7]"),
                (
                    5,
                    "Exchange your score pile with the valued cards in the junk",
                ),
            ]),
        )
        .optional(),
        // Each draw is junked before the next.
        draw(7),
        junk(LAST),
        draw(7),
        junk(LAST),
        Instruction::Jump(1),
        exchange(SCORE, Selector::Junk),
    ],
)];

// I DEMAND you transfer your lowest secret to my safe! / Tuck a card from
// any score pile. / You may splay your red or yellow cards right.
const MAFIA: &[(EffectKind, Program)] = &[
    (
        D,
        &[transfer(
            filtered(&SAFE, &[Lowest]),
            Destination::Safe(Demander),
        )],
    ),
    (N, &[tuck(Selector::Score(Either))]),
    (
        N,
        &[may_splay(ColorSet::Only(&[Red, Yellow]), Splay::Right)],
    ),
];

const TUCKED_COLOR_VISIBLE: Visibility = Visibility {
    who: Executor,
    colors: ColorSet::Of(Reg::Slot(MOVED)),
    seen: Seen::Cards,
};

// Draw and tuck a [7]. If you have at least seven visible cards on your
// board of the color of the tucked card, draw a [9]. Otherwise, junk all
// cards on your board of that color, and draw an [8].
const SUBWAY: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(7),
        tuck(LAST).keep(MOVED),
        if_(
            &Compare(Visible(&TUCKED_COLOR_VISIBLE), Cmp::AtLeast, Const(7)),
            2,
        ),
        draw(9),
        Instruction::Jump(2),
        junk(Board(Executor, ColorSet::Of(Reg::Slot(MOVED)), ALL_OF))
            .all()
            .shortcut(Shortcut::AnyOrder),
        draw(8),
    ],
)];

const REVEALED_ACHIEVEMENTS: Selector = filtered(&StandardAchievements, &[Has(InSlot(REVEALED))]);

// You may safeguard a card from your hand. If you do, reveal two available
// standard achievements. You may meld a revealed card with no [Efficiency]
// or [Avatar]. Return each revealed card you do not meld.
const BLACK_MARKET: &[(EffectKind, Program)] = &[(
    N,
    &[
        safeguard(HAND).optional(),
        if_(&Did(Outcome::All), 4),
        reveal(StandardAchievements).keep(REVEALED),
        reveal(filtered(&StandardAchievements, &[Lacks(InSlot(REVEALED))])).add(REVEALED),
        meld(filtered(
            &REVEALED_ACHIEVEMENTS,
            &[Lacks(HasIcon(Efficiency)), Lacks(HasIcon(Avatar))],
        ))
        .optional(),
        return_(REVEALED_ACHIEVEMENTS).all().shortcut(ORIGIN_ALL),
    ],
)];

// Meld a card from your score pile. Safeguard the lowest available standard
// achievement. If you do, super-execute the melded card if it is your turn,
// or if it is not your turn self-execute it.
const PLOT_VOUCHER: &[(EffectKind, Program)] = &[(
    N,
    &[
        meld(SCORE).keep(MOVED),
        safeguard(filtered(&StandardAchievements, &[Lowest])),
        if_(&All(&[Did(Outcome::All), Not(&Empty(MOVED))]), 4),
        if_(&OwnTurn, 2),
        SUPER_EXECUTE.executing(Reg::Slot(MOVED)),
        Instruction::Jump(1),
        SELF_EXECUTE.executing(Reg::Slot(MOVED)),
    ],
)];

// I DEMAND you tuck a top card on your board with [Concept]! If you do,
// transfer your top card of color matching the tucked card to my board, and
// draw a [7]! / Draw an [8].
const COUNTERINTELLIGENCE: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            tuck(filtered(&TOPS, &[Has(HasIcon(Concept))])).keep(MOVED),
            if_(&Did(Outcome::All), 2),
            transfer(
                Board(Executor, ColorSet::Of(Reg::Slot(MOVED)), BoardRegion::Top),
                Destination::Board(Demander),
            ),
            draw(7),
        ],
    ),
    (N, &[draw(8)]),
];

// I DEMAND you reveal your hand! Transfer the card in your hand of my choice
// to my board! Draw a [7]! / Score one of your secrets. / You may splay your
// blue cards right.
const PRIVATE_EYE: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            reveal_all(HAND).keep(REVEALED),
            choose(
                Demander,
                Choice::Card(&filtered(&HAND, &[Has(InSlot(REVEALED))])),
            ),
            transfer(
                filtered(&HAND, &[Has(Is(Reg::Chosen))]),
                Destination::Board(Demander),
            ),
            draw(7),
        ],
    ),
    (N, &[score(SAFE)]),
    (N, &[may_splay(ColorSet::Only(&[Blue]), Splay::Right)]),
];

const fn icon_exactly(icon: Icon, count: i64) -> Cond {
    Compare(M(Executor, Metric::Icons(icon)), Cmp::Equal, Const(count))
}

const fn metric_exactly(metric: Metric, count: i64) -> Cond {
    Compare(M(Executor, metric), Cmp::Equal, Const(count))
}

/// Exactly this many of some icon type on the executor's board. Bonuses are one type
/// and each City icon its own.
macro_rules! any_icon_exactly {
    ($name:ident, $count:expr) => {
        const $name: Cond = Any(&[
            icon_exactly(Authority, $count),
            icon_exactly(Concept, $count),
            icon_exactly(Health, $count),
            icon_exactly(Prosperity, $count),
            icon_exactly(Industry, $count),
            icon_exactly(Efficiency, $count),
            icon_exactly(Avatar, $count),
            icon_exactly(Food, $count),
            metric_exactly(Metric::Bonuses, $count),
            metric_exactly(Metric::CityIcons(CityIcon::Plus), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Uplift), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Junk), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Left), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Right), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Up), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Unsplay), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Flag), $count),
            metric_exactly(Metric::CityIcons(CityIcon::Fountain), $count),
        ]);
    };
}
any_icon_exactly!(EXACTLY_SEVEN, 7);
any_icon_exactly!(EXACTLY_EIGHT, 8);
any_icon_exactly!(EXACTLY_NINE, 9);

// If you have exactly seven of any icon on your board, draw and score a
// [7]; exactly eight, splay your green or purple cards right and draw an
// [8]; exactly nine, draw a [9].
const FORTUNE_COOKIE: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&EXACTLY_SEVEN, 2),
        draw(7),
        score(LAST),
        if_(&EXACTLY_EIGHT, 2),
        splay_set(ColorSet::Only(&[Green, Purple]), Splay::Right),
        draw(8),
        if_(&EXACTLY_NINE, 1),
        draw(9),
    ],
)];

const GREEN_DRAWN: Expr = Count(&filtered(&Selector::Slot(DRAWN), &[Has(ColorIs(Green))]));

// Draw and reveal a [1], [2], [3], [4], and [5]. If at least one drawn card
// is green, splay your green or purple cards right. If at least two are,
// score all drawn cards, otherwise return them. If at least three are, you
// win.
const SLOT_MACHINE: &[(EffectKind, Program)] = &[(
    N,
    &[
        for_each(Domain::Range(1, 5), Order::Printed, Some(ITEM), 2),
        draw_age(Expr::Slot(ITEM)),
        reveal(LAST).add(DRAWN),
        if_(&Compare(GREEN_DRAWN, Cmp::AtLeast, Const(1)), 1),
        splay_set(ColorSet::Only(&[Green, Purple]), Splay::Right),
        if_(&Compare(GREEN_DRAWN, Cmp::AtLeast, Const(2)), 2),
        score(DRAWN_IN_HAND).all().shortcut(ORIGIN_ALL),
        Instruction::Jump(1),
        return_(DRAWN_IN_HAND).all().shortcut(ORIGIN_ALL),
        if_(&Compare(GREEN_DRAWN, Cmp::AtLeast, Const(3)), 1),
        WIN,
    ],
)];

// ----------------------------------------------------------------- Age 8

const PURPLE_BOTTOM: Selector = Board(Executor, ColorSet::Only(&[Purple]), BoardRegion::Bottom);

// I DEMAND you tuck all your secrets! / Safeguard your bottom purple card.
const CONCEALMENT: &[(EffectKind, Program)] =
    &[(D, &[tuck(SAFE).all()]), (N, &[safeguard(PURPLE_BOTTOM)])];

/// A mandatory splay of one color currently splayed `from`.
const fn resplay(from: Splay, direction: Splay) -> Instruction {
    Instruction::Splay(SplayStep {
        who: Executor,
        choices: SplayChoices::Fixed {
            colors: ALL_COLORS,
            direction,
            from: Some(from),
        },
        targets: SplayTargets::Changes,
        optional: false,
        all: false,
    })
}

// Choose to either safeguard all available standard achievements, transfer
// all your secrets to your hand, or transfer all cards in your hand to the
// available achievements. / Choose a color you have splayed left and splay
// it up.
const ENIGMA_MACHINE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Safeguard all available standard achievements"),
                    (2, "Transfer all your secrets to your hand"),
                    (
                        4,
                        "Transfer all cards in your hand to the available achievements",
                    ),
                ]),
            ),
            // Safe arrival order is the player's: with less room than
            // achievements, the order decides which ones enter.
            safeguard(StandardAchievements).all(),
            Instruction::Jump(3),
            transfer(SAFE, Destination::Hand(Executor))
                .all()
                .shortcut(ORIGIN_ALL),
            Instruction::Jump(1),
            transfer(HAND, Destination::Supply)
                .all()
                .shortcut(ORIGIN_ALL),
        ],
    ),
    (N, &[resplay(Splay::Left, Splay::Up)]),
];

// I DEMAND you transfer the highest card on your board without [Efficiency]
// to my board! If you do, unsplay the transferred card's color on your
// board! / Unsplay the color on your board with the most visible cards.
const JACKALOPE: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            transfer(
                filtered(&TOPS, &[Lacks(HasIcon(Efficiency)), Highest]),
                Destination::Board(Demander),
            )
            .keep(MOVED),
            if_(&Did(Outcome::All), 1),
            splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::None),
        ],
    ),
    (
        N,
        &[splay_set(ColorSet::MostVisible(Executor), Splay::None)],
    ),
];

const TUCKED_ON_BOARD: Selector = filtered(
    &Board(Executor, ColorSet::Of(Reg::Slot(MOVED)), ALL_OF),
    &[Has(InSlot(MOVED))],
);

// Draw and tuck an [8]. If it has [Industry], score it. Otherwise, draw and meld an
// [8]. If it is an [8], repeat this effect. "It" is the most recently drawn card of the
// branch.
const SHANGRI_LA: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(8),
        tuck(LAST).keep(MOVED),
        if_(&Card(Reg::Slot(MOVED), Has(HasIcon(Industry))), 2),
        score(TUCKED_ON_BOARD),
        Instruction::Jump(2),
        draw(8),
        meld(LAST).keep(MOVED),
        if_(&Card(Reg::Slot(MOVED), Has(Test::BoxedValues(&[8]))), 1),
        REPEAT,
    ],
)];

// You may choose to either transfer your bottom card of each color to your
// hand, or tuck all cards from your score pile, or choose a value and score
// all cards from your hand of that value.
const HANDBAG: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Branch(&[
                (0, "Transfer your bottom card of each color to your hand"),
                (2, "Tuck all cards from your score pile"),
                (
                    4,
                    "Choose a value and score all cards from your hand of that value",
                ),
            ]),
        )
        .optional(),
        transfer(
            Board(Executor, ALL_COLORS, BoardRegion::Bottom),
            Destination::Hand(Executor),
        )
        .per_color(None)
        .shortcut(ORIGIN_ALL),
        Instruction::Jump(4),
        tuck(SCORE).all().shortcut(ORIGIN_ALL),
        Instruction::Jump(2),
        choose(Executor, Choice::Value(ValueChoices::Among(&HAND))),
        score(filtered(&HAND, &[same_value(Expr::Chosen)]))
            .all()
            .shortcut(ORIGIN_ALL),
    ],
)];

// Choose another player. They transfer a card from their hand to your
// board. If they do, self-execute the card, with that player making all
// decisions and allowed to look at any card that you can.
const HITCHHIKING: &[(EffectKind, Program)] = &[(
    N,
    &[
        transfer(Selector::Hand(Opponent), Destination::Board(Executor))
            .by(Opponent)
            .keep(MOVED),
        if_(&Did(Outcome::All), 1),
        SELF_EXECUTE
            .executing(Reg::Slot(MOVED))
            .controlled_by(Opponent)
            .inspecting(),
    ],
)];

// Choose to either unsplay one color of your cards, or splay up an
// unsplayed color on your board and draw a [9].
const BLACKLIGHT: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Branch(&[
                (0, "Unsplay one color"),
                (2, "Splay up an unsplayed color and draw a [9]"),
            ]),
        ),
        // Any nonempty color, a no-op if already unsplayed.
        Instruction::Splay(SplayStep {
            who: Executor,
            choices: SplayChoices::Fixed {
                colors: ALL_COLORS,
                direction: Splay::None,
                from: None,
            },
            targets: SplayTargets::Nonempty,
            optional: false,
            all: false,
        }),
        Instruction::Jump(2),
        // Any unsplayed nonempty color, a lone card included, and the draw even with
        // none.
        Instruction::Splay(SplayStep {
            who: Executor,
            choices: SplayChoices::Fixed {
                colors: ALL_COLORS,
                direction: Splay::Up,
                from: Some(Splay::None),
            },
            targets: SplayTargets::Nonempty,
            optional: false,
            all: false,
        }),
        draw(9),
    ],
)];

// Draw and reveal two [9]. Return at least one of the drawn cards. If you
// return two cards, reveal the top card of the [10] deck. If the color of
// the revealed card matches the color of at least one of the returned
// cards, draw a [10].
const SCOUTING: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(9),
        reveal(LAST).add(DRAWN),
        draw(9),
        reveal(LAST).add(DRAWN),
        return_(DRAWN_IN_HAND).keep(RETURNED),
        return_(DRAWN_IN_HAND).add(RETURNED).optional(),
        if_(&Did(Outcome::All), 5),
        reveal(Selector::BaseDeckTop(Expr::Boxed(10))).keep(REVEALED),
        if_(
            &Compare(
                Count(&filtered(
                    &Selector::Slot(RETURNED),
                    &[Has(ColorOf(Reg::Slot(REVEALED)))],
                )),
                Cmp::AtLeast,
                Const(1),
            ),
            1,
        ),
        draw(10),
    ],
)];

// I DEMAND you exchange all cards in your hand with all the lowest cards in
// my hand! / You may choose a value and score all the cards in your hand of
// that value. If you do, score your top purple card.
const JOY_BUZZER: &[(EffectKind, Program)] = &[
    (
        D,
        &[exchange(
            HAND,
            filtered(&Selector::Hand(Demander), &[Lowest]),
        )],
    ),
    (
        N,
        &[
            choose(Executor, Choice::Value(ValueChoices::Among(&HAND))).optional(),
            score(filtered(&HAND, &[same_value(Expr::Chosen)]))
                .all()
                .shortcut(ORIGIN_ALL),
            if_(&Did(Outcome::Any), 1),
            score(Board(Executor, ColorSet::Only(&[Purple]), BoardRegion::Top)),
        ],
    ),
];

// Reveal the highest card in your score pile. If you do, splay your cards
// of the revealed card's color up, and safeguard the revealed card. / Draw
// an [8] for every color on your board splayed up.
const OPUS_DEI: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            reveal(filtered(&SCORE, &[Highest])).keep(REVEALED),
            if_(&Did(Outcome::All), 2),
            splay_set(ColorSet::Of(Reg::Slot(REVEALED)), Splay::Up),
            safeguard(filtered(&SCORE, &[Has(InSlot(REVEALED))])),
        ],
    ),
    (
        N,
        &[draws(
            Count(&filtered(&TOPS, &[Has(StackSplayed(Some(Splay::Up)))])),
            8,
            Then::Keep,
        )],
    ),
];

// ----------------------------------------------------------------- Age 9

// I DEMAND you draw and meld an [8]! If Socialism is a top card on your
// board, you lose! / Score your top purple card. / You may splay your red
// or blue cards up.
const MCCARTHYISM: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            draw(8),
            meld(LAST),
            if_(
                &Compare(
                    Count(&filtered(&TOPS, &[Has(Code("B085"))])),
                    Cmp::AtLeast,
                    Const(1),
                ),
                1,
            ),
            Instruction::Win(WinRule::Loses(Executor)),
        ],
    ),
    (
        N,
        &[score(Board(
            Executor,
            ColorSet::Only(&[Purple]),
            BoardRegion::Top,
        ))],
    ),
    (N, &[may_splay(ColorSet::Only(&[Red, Blue]), Splay::Up)]),
];

/// Magic 8-Ball's wish, kept apart from the tucked card.
const WISH: Slot = Slot::effect(2);

const fn wish_is(wish: i64) -> Cond {
    Compare(Expr::Slot(WISH), Cmp::Equal, Const(wish))
}

// Choose whether you wish to draw two [10], draw and score two [8], or safeguard two
// available standard achievements. Draw and tuck an [8]. If it has [Concept], do as you
// wish. If it is red or purple, repeat this effect. The wish is chosen again on each
// repeat.
const MAGIC_8_BALL: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Branch(&[
                (0, "Wish to draw two [10]"),
                (2, "Wish to draw and score two [8]"),
                (4, "Wish to safeguard two available standard achievements"),
            ]),
        ),
        Instruction::Bind(Binding::Value(WISH, Const(1))),
        Instruction::Jump(3),
        Instruction::Bind(Binding::Value(WISH, Const(2))),
        Instruction::Jump(1),
        Instruction::Bind(Binding::Value(WISH, Const(3))),
        draw(8),
        tuck(LAST).keep(MOVED),
        if_(&Card(Reg::Slot(MOVED), Has(HasIcon(Concept))), 6),
        if_(&wish_is(1), 1),
        draws(Const(2), 10, Then::Keep),
        if_(&wish_is(2), 1),
        draws(Const(2), 8, Then::Score),
        if_(&wish_is(3), 1),
        safeguard(StandardAchievements).exactly(Const(2)),
        if_(
            &Card(
                Reg::Slot(MOVED),
                Has(ColorIn(ColorSet::Only(&[Red, Purple]))),
            ),
            1,
        ),
        REPEAT,
    ],
)];

const VICTIM_COLORS_OUTSIDE: Selector =
    filtered(&HAND, &[Lacks(ColorIn(ColorSet::OfCards(&DEMANDER_HAND)))]);
/// Read by the demander as performer: their hand against the victim's.
const DEMANDER_COLORS_OUTSIDE: Selector = filtered(
    &HAND,
    &[Lacks(ColorIn(ColorSet::OfCards(&Selector::Hand(Opponent))))],
);

// I DEMAND you reveal your hand! If each color present in my hand is present in yours,
// and vice versa, and your hand is not empty, I win! / Draw a [10]. Proof: the demander
// shows one card of a color the victim lacks, or else their whole hand, which decides
// the win.
const SURVEILLANCE: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            reveal_all(HAND),
            if_(&Compare(Count(&HAND), Cmp::AtLeast, Const(1)), 5),
            reveal(DEMANDER_COLORS_OUTSIDE).by(Demander),
            if_(&Not(&Did(Outcome::All)), 3),
            reveal_all(HAND).by(Demander),
            if_(
                &Compare(Count(&VICTIM_COLORS_OUTSIDE), Cmp::Equal, Const(0)),
                1,
            ),
            Instruction::Win(WinRule::Player(Demander)),
        ],
    ),
    (N, &[draw(10)]),
];

// You may splay your green cards up. / Choose to either draw an [11], or
// safeguard an available standard achievement. / Reveal one of your
// secrets, and super-execute it if it is your turn.
const AREA_51: &[(EffectKind, Program)] = &[
    (N, &[may_splay(ColorSet::Only(&[Green]), Splay::Up)]),
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Draw an [11]"),
                    (2, "Safeguard an available standard achievement"),
                ]),
            ),
            draw(11),
            Instruction::Jump(1),
            safeguard(StandardAchievements),
        ],
    ),
    (
        N,
        &[
            reveal(SAFE).keep(REVEALED),
            if_(&All(&[Did(Outcome::All), OwnTurn]), 1),
            SUPER_EXECUTE.executing(Reg::Slot(REVEALED)),
        ],
    ),
];

const fn ordinal_is(ordinal: i64) -> Cond {
    Compare(Expr::Slot(VALUE), Cmp::Equal, Const(ordinal))
}

const fn teleprompt(ordinal: u8) -> Instruction {
    execute(ExecutionMode::Effect(ordinal), Executor)
        .executing(Reg::Slot(REVEALED))
        .ignore_reveals()
}

// Reveal the top card of any value deck from any set. Execute the first non-demand
// dogma effect on the card. If you do, return the revealed card and repeat this effect
// using the next non-demand effect. The effect-based variant: the ordinal counts
// non-demand effects; only non-reveal activity continues the loop; the remembered card
// returns from wherever it now lies.
const TELEPROMPTER: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&Empty(VALUE), 1),
        Instruction::Bind(Binding::Value(VALUE, Const(0))),
        reveal(Selector::DeckTops).keep(REVEALED),
        if_(&ordinal_is(0), 1),
        teleprompt(0),
        if_(&ordinal_is(1), 1),
        teleprompt(1),
        if_(&ordinal_is(2), 1),
        teleprompt(2),
        if_(&ordinal_is(3), 1),
        teleprompt(3),
        if_(&Did(Outcome::All), 3),
        return_(Selector::Slot(REVEALED)),
        Instruction::Bind(Binding::Value(VALUE, Add(&Expr::Slot(VALUE), 1))),
        repeat_keeping(VALUE),
    ],
)];

// Choose an opponent. Draw and meld two [10]. Self-execute the top card on
// your board of that player's choice.
const CONSULTING: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(10),
        meld(LAST),
        draw(10),
        meld(LAST),
        choose(Opponent, Choice::Card(&TOPS)),
        SELF_EXECUTE.executing(Reg::Chosen),
    ],
)];

const NINE_OR_TEN_TOP: Selector = Selector::Union(&[
    Selector::BaseDeckTop(Expr::Boxed(9)),
    Selector::BaseDeckTop(Expr::Boxed(10)),
]);

// Reveal the top card of the [9] deck and the [10] deck. Return the top
// card of the [9] deck or the [10] deck. / If you have no cards on your
// board, you win. Otherwise, transfer all valued cards in the junk to your
// hand.
const FERMI_PARADOX: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            reveal(Selector::BaseDeckTop(Expr::Boxed(9))),
            reveal(Selector::BaseDeckTop(Expr::Boxed(10))),
            return_(NINE_OR_TEN_TOP),
        ],
    ),
    (
        N,
        &[
            if_(&Compare(Count(&TOPS), Cmp::Equal, Const(0)), 2),
            WIN,
            Instruction::Jump(1),
            transfer(Selector::Junk, Destination::Hand(Executor))
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    ),
];

// I DEMAND you meld a card from my score pile! If the melded card has no
// [Prosperity], repeat this effect!
const CLOWN_CAR: &[(EffectKind, Program)] = &[(
    D,
    &[
        meld(Selector::Score(Demander)).keep(MOVED),
        if_(
            &All(&[
                Did(Outcome::All),
                Card(Reg::Slot(MOVED), Lacks(HasIcon(Prosperity))),
            ]),
            1,
        ),
        REPEAT,
    ],
)];

// For every color on your board with [Industry], draw a [9]. If you draw
// five cards, you win. / You may splay your yellow or purple cards up.
const URBAN_LEGEND: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            draws(
                Count(&filtered(&TOPS, &[Has(StackShows(Industry))])),
                9,
                Then::Keep,
            ),
            if_(&Did(Outcome::Exactly(5)), 1),
            WIN,
        ],
    ),
    (
        N,
        &[may_splay(ColorSet::Only(&[Yellow, Purple]), Splay::Up)],
    ),
];

// ------------------------------------------------- Iron Curtain (Age 9)

// ---------------------------------------------------------------- Age 10

// I DEMAND you transfer one of your secrets to my achievements! / You may
// splay your yellow cards up.
const FIGHT_CLUB: &[(EffectKind, Program)] = &[
    (D, &[transfer(SAFE, Destination::Achievements(Demander))]),
    (N, &[may_splay(ColorSet::Only(&[Yellow]), Splay::Up)]),
];

// I DEMAND you meld a card from my score pile! / Draw and score three [10].
const SECRET_SANTA: &[(EffectKind, Program)] = &[
    (D, &[meld(Selector::Score(Demander))]),
    (N, &[draws(Const(3), 10, Then::Score)]),
];

// You may splay your green cards up. / Meld a top non-yellow card with
// [Efficiency] from another player's board. If you do, self-execute it.
// Otherwise, draw an [11].
const RIDE_HAILING: &[(EffectKind, Program)] = &[
    (N, &[may_splay(ColorSet::Only(&[Green]), Splay::Up)]),
    (
        N,
        &[
            meld(filtered(
                &Tops(Opponent),
                &[Lacks(ColorIs(Yellow)), Has(HasIcon(Efficiency))],
            ))
            .keep(MOVED),
            if_(&Did(Outcome::All), 2),
            SELF_EXECUTE.executing(Reg::Slot(MOVED)),
            Instruction::Jump(1),
            draw(11),
        ],
    ),
];

const TOPS_AND_SCORE: Selector = Selector::Union(&[TOPS, SCORE]);

// For each value, in ascending order, if that value is not a value of a top
// card on your board or a card in your score pile, draw and score a card of
// that value.
const WHATCHAMACALLIT: &[(EffectKind, Program)] = &[(
    N,
    &[
        for_each(Domain::Range(1, 11), Order::Printed, Some(ITEM), 3),
        if_(
            &Compare(
                Count(&filtered(&TOPS_AND_SCORE, &[same_value(Expr::Slot(ITEM))])),
                Cmp::Equal,
                Const(0),
            ),
            2,
        ),
        draw_age(Expr::Slot(ITEM)),
        score(LAST),
    ],
)];

// Return all cards from your score pile. For each different value of card
// you return, draw and score a [10]. / You may splay your red cards up.
const CRYPTOCURRENCY: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            return_(SCORE).all().shortcut(ORIGIN_ALL),
            draws(Expr::DoneValues, 10, Then::Score),
        ],
    ),
    (N, &[may_splay(ColorSet::Only(&[Red]), Splay::Up)]),
];

// You may achieve one of your secrets regardless of eligibility. / You may
// splay your purple cards up.
const DENVER_AIRPORT: &[(EffectKind, Program)] = &[
    (N, &[achieve(SAFE).optional()]),
    (N, &[may_splay(ColorSet::Only(&[Purple]), Splay::Up)]),
];

// I DEMAND you transfer your two highest secrets to my safe! Transfer the
// two highest cards in your score pile to my score pile! Meld the two lowest
// cards from your score pile!
const HACKING: &[(EffectKind, Program)] = &[(
    D,
    &[
        transfer(filtered(&SAFE, &[Highest]), Destination::Safe(Demander)).exactly(Const(2)),
        transfer(filtered(&SCORE, &[Highest]), Destination::Score(Demander))
            .exactly(Const(2))
            .shortcut(ORIGIN),
        meld(filtered(&SCORE, &[Lowest]))
            .exactly(Const(2))
            .shortcut(Shortcut::ByColor),
    ],
)];

// You may place a top card from your board on top of its deck. You may meld
// a card from your hand. If you do either, repeat this effect. / Draw an
// [11] for every color not on your board.
const INHOMOGENEOUS_COSMOLOGY: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            verb(Verb::Place, TOPS).optional().keep(MOVED),
            meld(HAND).optional().add(MOVED),
            if_(&Not(&Empty(MOVED)), 1),
            REPEAT,
        ],
    ),
    (
        N,
        &[draws(Add(&Expr::Mul(&Count(&TOPS), -1), 5), 11, Then::Keep)],
    ),
];

// If you have a [3] in your score pile, you lose. / If you have a [7] in
// your hand, you win. / Meld two cards from your hand. Score four cards from
// your hand. Splay your blue cards up.
const ORDER_OF_THE_OCCULT_HAND: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            if_(
                &Compare(
                    Count(&filtered(&SCORE, &[Has(Test::BoxedValues(&[3]))])),
                    Cmp::AtLeast,
                    Const(1),
                ),
                1,
            ),
            LOSE,
        ],
    ),
    (
        N,
        &[
            if_(
                &Compare(
                    Count(&filtered(&HAND, &[Has(Test::BoxedValues(&[7]))])),
                    Cmp::AtLeast,
                    Const(1),
                ),
                1,
            ),
            WIN,
        ],
    ),
    (
        N,
        &[
            meld(HAND).exactly(Const(2)).shortcut(Shortcut::ByColor),
            score(HAND).exactly(Const(4)).shortcut(ORIGIN),
            splay_set(ColorSet::Only(&[Blue]), Splay::Up),
        ],
    ),
];

// Unsplay each splayed color on your board. For each color you unsplay,
// return your top card of that color and safeguard an available standard
// achievement.
// Each splayed color is unsplayed in the player's order, and the return and
// safeguard follow for the colors actually unsplayed.
const IRON_CURTAIN: &[(EffectKind, Program)] = &[(
    N,
    &[
        // Unsplay every splayed color first: the Safe's capacity follows the
        // best remaining splay, so the safeguards see the unsplayed board.
        Instruction::Bind(Binding::Colors(
            COLOR,
            ColorSet::OfCards(&filtered(&Tops(Executor), &[Has(StackSplayed(None))])),
        )),
        for_each(
            Domain::Colors(ColorSet::Slot(COLOR)),
            Order::Printed,
            Some(ITEM),
            1,
        ),
        splay_set(ColorSet::Slot(ITEM), Splay::None),
        for_each(
            Domain::Colors(ColorSet::Slot(COLOR)),
            Order::Chosen(Executor),
            Some(ITEM),
            2,
        ),
        return_(Board(Executor, ColorSet::Slot(ITEM), BoardRegion::Top)),
        safeguard(StandardAchievements),
    ],
)];

// Return a top or bottom card on your board. Achieve one of your secrets of
// value equal to the returned card regardless of eligibility, then safeguard
// an available standard achievement. If you do, repeat this effect.
const THREE_D_PRINTING: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(Selector::Union(&[
            Board(Executor, ALL_COLORS, BoardRegion::Top),
            Board(Executor, ALL_COLORS, BoardRegion::Bottom),
        ]))
        .keep(MOVED),
        // Both the achieve and the safeguard are needed to repeat; the
        // safeguard is attempted even after a failed achieve.
        once(2),
        achieve(filtered(&SAFE, &[same_value(Value(Reg::Slot(MOVED)))])),
        safeguard(StandardAchievements),
        if_(&Did(Outcome::All), 1),
        REPEAT,
    ],
)];

// ---------------------------------------------------------------- Age 11

// I DEMAND you transfer one of your claimed standard achievements to my
// safe!
const CLOAKING: &[(EffectKind, Program)] = &[(
    D,
    &[transfer(
        Selector::Achievements(Executor),
        Destination::Safe(Demander),
    )],
)];

// Unsplay any color on any board. / Choose to either safeguard any number of available
// standard achievements, or achieve any number of secrets from your safe regardless of
// eligibility. Any nonempty color on any board, splayed or not.
const DARK_WEB: &[(EffectKind, Program)] = &[
    (
        N,
        &[Instruction::Splay(SplayStep {
            who: Either,
            choices: SplayChoices::Fixed {
                colors: ALL_COLORS,
                direction: Splay::None,
                from: None,
            },
            targets: SplayTargets::Nonempty,
            optional: false,
            all: false,
        })],
    ),
    (
        N,
        &[
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Safeguard any number of available standard achievements"),
                    (2, "Achieve any number of secrets regardless of eligibility"),
                ]),
            ),
            safeguard(StandardAchievements).up_to(None),
            Instruction::Jump(1),
            achieve(SAFE).up_to(None),
        ],
    ),
];

// I DEMAND you draw, reveal, and score an [11]! Score a card from your hand
// of the same color as the drawn card! If you don't, you lose! / Score four
// top non-yellow cards each with [Efficiency] of different colors on your
// board.
const ESCAPE_ROOM: &[(EffectKind, Program)] = &[
    (
        D,
        &[
            draw(11),
            reveal(LAST).keep(DRAWN),
            score(DRAWN_IN_HAND),
            score(filtered(&HAND, &[Has(ColorOf(Reg::Slot(DRAWN)))]))
                .verify()
                .reveal_if_empty(&HAND),
            if_(&Not(&Did(Outcome::All)), 1),
            LOSE,
        ],
    ),
    (
        N,
        &[score(filtered(
            &TOPS,
            &[Lacks(ColorIs(Yellow)), Has(HasIcon(Efficiency))],
        ))
        .per_color(Some(Const(4)))],
    ),
];

const PROSPEROUS_OR_INDUSTRIOUS_TOPS: Selector = Selector::Union(&[
    filtered(&TOPS, &[Has(HasIcon(Prosperity))]),
    filtered(&TOPS, &[Has(HasIcon(Industry))]),
]);

// Tuck a top card with [Prosperity] or [Industry] of each color on your
// board. You may safeguard one of the tucked cards. / Score all but the top
// five each of your yellow and purple cards. Splay those colors aslant.
const FASHION_MASK: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            tuck(PROSPEROUS_OR_INDUSTRIOUS_TOPS)
                .per_color(None)
                .shortcut(ORIGIN_ALL)
                .keep(MOVED),
            safeguard(filtered(
                &Board(Executor, ALL_COLORS, ALL_OF),
                &[Has(InSlot(MOVED))],
            ))
            .optional(),
        ],
    ),
    (
        N,
        &[
            score(Board(
                Executor,
                ColorSet::Only(&[Yellow, Purple]),
                BoardRegion::BelowTop(5),
            ))
            .all()
            .shortcut(ORIGIN_ALL),
            splay_all(ColorSet::Only(&[Yellow, Purple]), Splay::Aslant),
        ],
    ),
];

// Score all non-top green cards on your board. Draw and tuck an [11] for
// each card scored.
const GREEN_HYDROGEN: &[(EffectKind, Program)] = &[(
    N,
    &[
        score(Board(
            Executor,
            ColorSet::Only(&[Green]),
            BoardRegion::BelowTop(1),
        ))
        .all()
        .shortcut(ORIGIN_ALL),
        // Each draw is tucked before the next: an exhausted draw ends the
        // game with the earlier cards already tucked.
        for_each(Domain::Times(Expr::Done), Order::Printed, None, 2),
        draw(11),
        tuck(LAST),
    ],
)];

// Claim an available standard achievement, regardless of eligibility.
// Self-execute it.
// The claimed card is revealed where it lies and executes from there.
const MYSTERY_BOX: &[(EffectKind, Program)] = &[(
    N,
    &[
        achieve(StandardAchievements).keep(MOVED),
        reveal(filtered(
            &Selector::Achievements(Executor),
            &[Has(InSlot(MOVED))],
        )),
        if_(&Did(Outcome::All), 1),
        SELF_EXECUTE.executing(Reg::Slot(MOVED)),
    ],
)];

// Return a bottom card from your board. Splay that color on your board
// aslant. Score all cards on your board of that color without [Health].
const ASTROBIOLOGY: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(Board(Executor, ALL_COLORS, BoardRegion::Bottom)).keep(MOVED),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Aslant),
        score(filtered(
            &Board(Executor, ColorSet::Of(Reg::Slot(MOVED)), ALL_OF),
            &[Lacks(HasIcon(Health))],
        ))
        .all()
        .shortcut(ORIGIN_ALL),
    ],
)];

/// Claimed standard achievements worth less than the lowest score card.
const LOWER_CLAIMED: Selector = filtered(
    &Selector::Achievements(Executor),
    &[Has(ValueIs(Cmp::Less, Expr::Min(&SCORE)))],
);

// Choose red, blue, or green. Score all but your top five cards of that
// color, then splay it aslant. If you do both, exchange all the lowest cards
// in your score pile with all your claimed standard achievements of lower
// value.
const HOLOGRAPHY: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Color(ColorSet::Only(&[Red, Blue, Green]))),
        once(2),
        score(Board(Executor, ColorSet::Chosen, BoardRegion::BelowTop(5)))
            .all()
            .shortcut(ORIGIN_ALL),
        splay_set(ColorSet::Chosen, Splay::Aslant),
        if_(&Did(Outcome::All), 1),
        exchange(filtered(&SCORE, &[Lowest]), LOWER_CLAIMED),
    ],
)];

// For each splayed color on your board, score its top card. If you score
// fewer than three cards, you lose.
const METAVERSE: &[(EffectKind, Program)] = &[(
    N,
    &[
        score(filtered(&TOPS, &[Has(StackSplayed(None))]))
            .per_color(None)
            .shortcut(ORIGIN_ALL),
        if_(&Compare(Expr::Done, Cmp::Less, Const(3)), 1),
        LOSE,
    ],
)];

// I DEMAND you flip a coin! If you lose the flip, you lose! / Flip a coin.
// If you win the flip, this effect is complete. If you lose the flip, return
// one of your secrets. If you don't, you lose. Otherwise, repeat this effect.
const QUANTUM_COMPUTERS: &[(EffectKind, Program)] = &[
    (D, &[FLIP, if_(&Not(&Did(Outcome::All)), 1), LOSE]),
    (
        N,
        &[
            FLIP,
            if_(&Not(&Did(Outcome::All)), 4),
            return_(SAFE),
            if_(&Did(Outcome::All), 1),
            REPEAT,
            LOSE,
        ],
    ),
];

/// The authored effects of an Unseen card, by publisher code.
pub fn effects(code: &str) -> Option<&'static [(EffectKind, Program)]> {
    Some(match code {
        "U001" => ASSASSINATION,
        "U002" => MAZE,
        "U003" => PILGRIMAGE,
        "U004" => HANDSHAKE,
        "U005" => TOMB,
        "U006" => WOODWORKING,
        "U007" => DANCE,
        "U008" => RUMOR,
        "U009" => SILK,
        "U010" => ESPIONAGE,
        "U011" => PALMISTRY,
        "U012" => PROVERB,
        "U013" => MYTH,
        "U014" => POLYTHEISM,
        "U015" => SYMBOLOGY,
        "U016" => PADLOCK,
        "U017" => PASSWORD,
        "U018" => EXILE,
        "U019" => FINGERPRINTS,
        "U020" => CIPHER,
        "U021" => COUNTERFEITING,
        "U022" => ASTROLOGY,
        "U023" => METEOROLOGY,
        "U024" => PROPAGANDA,
        "U025" => STEGANOGRAPHY,
        "U026" => KNIGHTS_TEMPLAR,
        "U027" => RED_ENVELOPE,
        "U028" => FREEMASONS,
        "U029" => SECRET_POLICE,
        "U030" => CLIFFHANGER,
        "U031" => SMUGGLING,
        "U032" => BRETHREN_OF_PURITY,
        "U033" => SECRETUM_SECRETORUM,
        "U034" => MASQUERADE,
        "U035" => TAQIYYA,
        "U036" => NINJA,
        "U037" => SPANISH_INQUISITION,
        "U038" => APRIL_FOOLS_DAY,
        "U039" => HEIRLOOM,
        "U040" => BLACKMAIL,
        "U041" => EL_DORADO,
        "U042" => QUACKERY,
        "U043" => THE_PROPHECIES,
        "U044" => CONFESSION,
        "U045" => LEGEND,
        "U046" => CABAL,
        "U047" => WITCH_TRIAL,
        "U048" => CHARTREUSE,
        "U049" => GALLERY,
        "U050" => BURIED_TREASURE,
        "U051" => SECRET_HISTORY,
        "U052" => POPULAR_SCIENCE,
        "U053" => PROBABILITY,
        "U054" => PANTHEISM,
        "U055" => PEN_NAME,
        "U056" => RED_HERRING,
        "U057" => SNIPING,
        "U058" => ATTIC,
        "U059" => SABOTAGE,
        "U060" => HIKING,
        "U061" => SWISS_BANK_ACCOUNT,
        "U062" => PLACEBO,
        "U063" => RECONNAISSANCE,
        "U064" => ILLUMINATI,
        "U065" => TRIAD,
        "U066" => CAMOUFLAGE,
        "U067" => SAFE_DEPOSIT_BOX,
        "U068" => MAFIA,
        "U069" => SUBWAY,
        "U070" => BLACK_MARKET,
        "U071" => PLOT_VOUCHER,
        "U072" => COUNTERINTELLIGENCE,
        "U073" => PRIVATE_EYE,
        "U074" => FORTUNE_COOKIE,
        "U075" => SLOT_MACHINE,
        "U076" => CONCEALMENT,
        "U077" => ENIGMA_MACHINE,
        "U078" => JACKALOPE,
        "U079" => SHANGRI_LA,
        "U080" => HANDBAG,
        "U081" => HITCHHIKING,
        "U082" => BLACKLIGHT,
        "U083" => SCOUTING,
        "U084" => JOY_BUZZER,
        "U085" => OPUS_DEI,
        "U087" => MCCARTHYISM,
        "U088" => MAGIC_8_BALL,
        "U089" => SURVEILLANCE,
        "U090" => AREA_51,
        "U091" => TELEPROMPTER,
        "U092" => CONSULTING,
        "U093" => FERMI_PARADOX,
        "U094" => CLOWN_CAR,
        "U095" => URBAN_LEGEND,
        "U096" => FIGHT_CLUB,
        "U097" => SECRET_SANTA,
        "U098" => RIDE_HAILING,
        "U099" => WHATCHAMACALLIT,
        "U100" => CRYPTOCURRENCY,
        "U101" => DENVER_AIRPORT,
        "U102" => HACKING,
        "U103" => INHOMOGENEOUS_COSMOLOGY,
        "U105" => ORDER_OF_THE_OCCULT_HAND,
        "U106" => CLOAKING,
        "U107" => DARK_WEB,
        "U108" => ESCAPE_ROOM,
        "U109" => FASHION_MASK,
        "U110" => GREEN_HYDROGEN,
        "U111" => MYSTERY_BOX,
        "U112" => ASTROBIOLOGY,
        "U113" => QUANTUM_COMPUTERS,
        "U114" => HOLOGRAPHY,
        "U115" => METAVERSE,
        "U086" => IRON_CURTAIN,
        "U104" => THREE_D_PRINTING,
        _ => return None,
    })
}
