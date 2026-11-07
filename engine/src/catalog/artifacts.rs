//! Artifacts card programs in code order: each card's compels and
//! non-demand effects, each comment giving the printed text. Cards without
//! an entry are explicitly unsupported. Rules procedures (dig, showcase,
//! Museums) are not card effects and live in `execution::artifacts`.
use crate::model::{CardSet, EffectKind};
use crate::program::build::*;

use Color::{Blue, Green, Purple, Red, Yellow};
use Cond::{All, Any, Card, Compare, Did, Empty, Not, OwnTurn};
use Expr::{Add, Const, Count, DistinctColors, Max, Min, Value};
use Filter::{Has, Highest, Lacks};
use Icon::Authority;
use Selector::{AvailableAchievements, BaseDeck, Board, TopOf, Tops};
use Test::{
    BoardHasTopSet, Color as ColorIs, ColorIn, ColorOf, ColorOnBoard, Icon as HasIcon, InSlot, Is,
    TitleBefore, Value as ValueIs, ValueIn,
};
use Who::{Demander, Either, Executor, Opponent};

type Program = &'static [Instruction];

const C: EffectKind = EffectKind::Compel;
const N: EffectKind = EffectKind::NonDemand;

const DEMANDER_HAND: Destination = Destination::Hand(Demander);
const SOURCE: Selector = Selector::Reg(Reg::Source);
const BIND_LAST: Instruction = bind(Selector::Reg(Reg::Last));
const SELF_EXECUTE: Instruction = execute(ExecutionMode::SelfExecute, Executor);
const SUPER_EXECUTE: Instruction = execute(ExecutionMode::SuperExecute, Executor);
const WIN: Instruction = Instruction::Win(WinRule::Executor);
/// "Claim an available achievement, ignoring eligibility": a standard card or a
/// special.
const CLAIM_ANY: Instruction = Instruction::Special(SpecialStep::ClaimAny);
const ORIGIN: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: true,
};
const ORIGIN_ALL: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: false,
};
const ALL_COLORS: ColorSet = ColorSet::Only(&Color::ALL);

/// The cards an instruction moved.
const MOVED: Slot = Slot::effect(0);
/// The cards an instruction drew.
const DRAWN: Slot = Slot::effect(1);
/// The cards revealed so far.
const SHOWN: Slot = Slot::effect(2);
/// The cards scored.
const SCORED: Slot = Slot::effect(3);
/// Score members nobody can inspect (Wernher von Braun's junk), revealed as
/// the completeness proof before a front-filtered score selection, as in
/// Bandage. Naming grants no access to cards outside that score pile.
const PROOF: Slot = Slot::effect(2);
const VERIFIED_SCORE: Selector = Selector::Union(&[filtered(&SCORE, &[Has(InSlot(PROOF))]), SCORE]);
const DRAWN_IN_HAND: Selector = filtered(&HAND, &[Has(InSlot(DRAWN))]);
const SHOWN_IN_HAND: Selector = filtered(&HAND, &[Has(InSlot(SHOWN))]);
const MOVED_VALUE: Expr = Value(Reg::Slot(MOVED));

const fn holds(selector: &'static Selector) -> Cond {
    Compare(Count(selector), Cmp::AtLeast, Const(1))
}

/// Junk all cards in the Base deck of a computed value.
const fn deck_junk(value: Expr) -> Instruction {
    junk(BaseDeck(value)).all().shortcut(Shortcut::DeckTop)
}

/// The chosen value's Base deck is empty: junking it changes nothing, so
/// every empty deck is the same choice.
const EMPTY_CHOSEN_DECK: Cond = Compare(Count(&BaseDeck(Expr::Chosen)), Cmp::Equal, Const(0));

const fn same_value(value: Expr) -> Filter {
    Has(ValueIs(Cmp::Equal, value))
}

// I COMPEL you to transfer your highest top card with [Authority] to my hand!
// If you don't, junk all cards in the deck of value equal to the value of
// your lowest top card! / Draw a [2].
const HOLMEGAARD_BOWS: &[(EffectKind, Program)] = &[
    (
        C,
        &[
            transfer(
                filtered(&TOPS, &[Has(HasIcon(Authority)), Highest]),
                DEMANDER_HAND,
            ),
            if_(&Not(DID), 1),
            deck_junk(Min(&TOPS)),
        ],
    ),
    (N, &[draw(2)]),
];

// Return a purple card from your hand. If you do, draw and reveal a card
// from any set of value two higher. If the drawn card is purple, meld it
// and self-execute it.
const PAPYRUS_OF_ANI: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(filtered(&HAND, &[Has(ColorIs(Purple))]))
            .verify()
            .reveal_if_empty(&HAND)
            .keep(MOVED),
        if_(DID, 6),
        draw_age(Add(&MOVED_VALUE, 2)).any_set(),
        reveal(LAST),
        if_(&Card(Reg::Last, Has(ColorIs(Purple))), 3),
        meld(LAST),
        BIND_LAST,
        SELF_EXECUTE,
    ],
)];

// Draw three cards of value equal to your top green card. Return one of the
// drawn cards. Score one of the drawn cards.
const PAVLOVIAN_TUSK: &[(EffectKind, Program)] = &[(
    N,
    &[
        draws_age(Const(3), Max(&TopOf(Executor, Green)), Then::Keep).keep(DRAWN),
        return_(DRAWN_IN_HAND),
        score(DRAWN_IN_HAND),
    ],
)];

// I COMPEL you to transfer Dancing Girl to your board! / If Dancing Girl has
// been on every opponent's board during this action, and it is your turn,
// you win.
const DANCING_GIRL: &[(EffectKind, Program)] = &[
    (C, &[transfer(SOURCE, Destination::Board(Executor))]),
    (
        N,
        &[if_(&All(&[OwnTurn, Cond::DancingGirlVisited]), 1), WIN],
    ),
];

// Meld a card from your hand. Score all other cards of the same color from
// your board. If you score a card, repeat this effect.
const LURGAN_CANOE: &[(EffectKind, Program)] = &[(
    N,
    &[
        meld(HAND).keep(MOVED),
        if_(DID, 3),
        score(filtered(
            &Board(
                Executor,
                ColorSet::Of(Reg::Slot(MOVED)),
                BoardRegion::BelowTop(0),
            ),
            &[Lacks(Is(Reg::Slot(MOVED)))],
        ))
        .all()
        .shortcut(Shortcut::AnyOrder),
        if_(DID_ANY, 1),
        REPEAT,
    ],
)];

// Reveal three cards from your hand. Score two, then tuck the other. If you
// score two cards of the same color, draw three [1].
const XIANRENDONG_SHARDS: &[(EffectKind, Program)] = &[(
    N,
    &[
        for_each(Domain::Times(Const(3)), Order::Printed, None, 1),
        reveal(filtered(&HAND, &[Lacks(InSlot(SHOWN))])).add(SHOWN),
        score(SHOWN_IN_HAND)
            .exactly(Const(2))
            .shortcut(ORIGIN)
            .keep(SCORED),
        tuck(SHOWN_IN_HAND),
        if_(
            &All(&[
                Compare(Count(&Selector::Slot(SCORED)), Cmp::Equal, Const(2)),
                Compare(
                    DistinctColors(&Selector::Slot(SCORED)),
                    Cmp::Equal,
                    Const(1),
                ),
            ]),
            1,
        ),
        draws(Const(3), 1, Then::Keep),
    ],
)];

// Score a card from your hand. If you have a top card matching its color,
// super-execute that top card if it is your turn, otherwise self-execute it.
const PRIEST_KING: &[(EffectKind, Program)] = &[(
    N,
    &[
        // The scored card's color decides the execution: it is revealed.
        score(HAND).verify().keep(MOVED),
        if_(DID, 5),
        bind(filtered(&TOPS, &[Has(ColorOf(Reg::Slot(MOVED)))])),
        if_(&OwnTurn, 2),
        SUPER_EXECUTE,
        Instruction::Jump(1),
        SELF_EXECUTE,
    ],
)];

// Draw and reveal a [3]. If you do not have a top card of the drawn card's
// color, meld it and repeat this effect.
const ELECTRUM_STATER_OF_EPHESUS: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(3),
        reveal(LAST),
        if_(&Not(&Card(Reg::Last, Has(ColorOnBoard(Executor)))), 2),
        meld(LAST),
        REPEAT,
    ],
)];

// I COMPEL you to return a card from your score pile! If you do, transfer an
// achievement of the same value from your achievements to mine, and junk
// all cards in the deck of that value!
const JISKAIRUMOKO_NECKLACE: &[(EffectKind, Program)] = &[(
    C,
    &[
        return_(SCORE).keep(MOVED),
        if_(DID, 2),
        transfer(
            filtered(
                &Selector::Achievements(Executor),
                &[same_value(MOVED_VALUE)],
            ),
            Destination::Achievements(Demander),
        ),
        deck_junk(MOVED_VALUE),
    ],
)];

// I COMPEL you to return a top card with a demand effect of each color from
// your board! / Score a top, non-blue card with a demand effect on your
// board.
const TREATY_OF_KADESH: &[(EffectKind, Program)] = &[
    (
        C,
        &[return_(filtered(&TOPS, &[Has(Test::Demand)]))
            .per_color(None)
            .shortcut(ORIGIN_ALL)],
    ),
    (
        N,
        &[score(filtered(
            &TOPS,
            &[Has(Test::Demand), Lacks(ColorIs(Blue))],
        ))],
    ),
];

// Draw and reveal a [1]. If you have a top card of matching color and value
// to the drawn card, score the drawn card and repeat this effect.
const SIBIDU_NEEDLE: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(1),
        reveal(LAST),
        if_(
            &holds(&filtered(
                &TOPS,
                &[Has(ColorOf(Reg::Last)), same_value(Value(Reg::Last))],
            )),
            2,
        ),
        score(LAST),
        REPEAT,
    ],
)];

// Draw and reveal a [4]. If you have a top card of the drawn card's color
// that comes before it in the alphabet, return the drawn card and all cards
// from your score pile.
const BASUR_HOYUK_TOKENS: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(4),
        reveal(LAST),
        if_(
            &holds(&filtered(
                &TOPS,
                &[Has(ColorOf(Reg::Last)), Has(TitleBefore(Reg::Last))],
            )),
            2,
        ),
        return_(LAST),
        return_(SCORE).all().shortcut(ORIGIN_ALL),
    ],
)];

const CHOSEN_IN_HAND: Selector = filtered(&HAND, &[Has(ColorIn(ColorSet::Chosen))]);
/// The opponent's revealed hand cards of the chosen color: the reveal's
/// record names them, so their fronts may be read.
const CHOSEN_IN_REVEALED_OPPONENT_HAND: Selector = filtered(
    &filtered(&Selector::Hand(Opponent), &[Has(InSlot(SHOWN))]),
    &[Has(ColorIn(ColorSet::Chosen))],
);
// Choose a color. Each player reveals their hand. If you are the only player
// to reveal at least one card of that color, return them and claim all
// available achievements of value matching those cards, ignoring eligibility.
const MASK_OF_WARKA: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Colors(1)),
        reveal_all(HAND),
        reveal_all(Selector::Hand(Opponent)).keep(SHOWN),
        if_(
            &All(&[
                holds(&CHOSEN_IN_HAND),
                Not(&holds(&CHOSEN_IN_REVEALED_OPPONENT_HAND)),
            ]),
            2,
        ),
        return_(CHOSEN_IN_HAND)
            .all()
            .shortcut(ORIGIN_ALL)
            .keep(MOVED),
        achieve(filtered(
            &AvailableAchievements,
            &[Has(ValueIn(&Selector::Slot(MOVED)))],
        ))
        .all()
        .shortcut(Shortcut::AnyOrder),
    ],
)];

// Return a card from your hand. Score all cards of the same color on the
// boards of all players with no top Artifacts. / If Ark of the Covenant is
// a top card on any board, transfer it to your hand.
const ARK_OF_THE_COVENANT: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            return_(HAND).verify().keep(MOVED),
            if_(DID, 1),
            score(filtered(
                &Board(
                    Either,
                    ColorSet::Of(Reg::Slot(MOVED)),
                    BoardRegion::BelowTop(0),
                ),
                &[Lacks(BoardHasTopSet(CardSet::Artifacts))],
            ))
            .all()
            .frozen()
            .shortcut(ORIGIN_ALL),
        ],
    ),
    (
        N,
        &[
            if_(&holds(&filtered(&Tops(Either), &[Has(Is(Reg::Source))])), 1),
            transfer(SOURCE, Destination::Hand(Executor)),
        ],
    ),
];

// Choose a color. Draw a [1]. Meld a card of the chosen color from your
// hand. If you do, splay that color left, and junk an available achievement
// of value equal to the value of the melded card.
const TALE_OF_THE_SHIPWRECKED_SAILOR: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Colors(1)),
        draw(1),
        meld(CHOSEN_IN_HAND).reveal_if_empty(&HAND).keep(MOVED),
        if_(DID, 2),
        splay_set(ColorSet::Chosen, Splay::Left),
        junk(filtered(&AvailableAchievements, &[same_value(MOVED_VALUE)])),
    ],
)];

// If you have the least [Authority] or the least [Prosperity], draw a [3]. /
// If you have the least [Concept], draw a [4]. (Ties qualify.)
const fn least(icon: Icon) -> Cond {
    Compare(
        Expr::Metric(Executor, Metric::Icons(icon)),
        Cmp::AtMost,
        Expr::Metric(Opponent, Metric::Icons(icon)),
    )
}
const CHRONICLE_OF_ZUO: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            if_(&Any(&[least(Authority), least(Icon::Prosperity)]), 1),
            draw(3),
        ],
    ),
    (N, &[if_(&least(Icon::Concept), 1), draw(4)]),
];

// I COMPEL you to transfer a top non-red card with [Authority] from your
// board to my board! / Draw and score a [3].
const BABYLONIAN_CHRONICLES: &[(EffectKind, Program)] = &[
    (
        C,
        &[transfer(
            filtered(&TOPS, &[Has(HasIcon(Authority)), Lacks(ColorIs(Red))]),
            Destination::Board(Demander),
        )],
    ),
    (N, &[draw(3), score(LAST)]),
];

// Return a card from your hand. Claim an available achievement of matching
// value ignoring eligibility.
const HOLY_GRAIL: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).keep(MOVED),
        if_(DID, 1),
        achieve(filtered(&AvailableAchievements, &[same_value(MOVED_VALUE)])),
    ],
)];

// I COMPEL you to return a top card with no [Authority] from your board! /
// Score a card from your hand with no [Authority]. If you do, junk all cards
// in the deck of value equal to the scored card. Otherwise, tuck Terracotta
// Army.
const TERRACOTTA_ARMY: &[(EffectKind, Program)] = &[
    (C, &[return_(filtered(&TOPS, &[Lacks(HasIcon(Authority))]))]),
    (
        N,
        &[
            score(filtered(&HAND, &[Lacks(HasIcon(Authority))]))
                .verify()
                .reveal_if_empty(&HAND)
                .keep(MOVED),
            if_(DID, 2),
            deck_junk(MOVED_VALUE),
            Instruction::Jump(1),
            tuck(SOURCE),
        ],
    ),
];

// I COMPEL you to transfer a top Artifact from your board to my board! / If
// Holy Grail is a top card on your board, you win.
const HOLY_LANCE: &[(EffectKind, Program)] = &[
    (
        C,
        &[transfer(
            filtered(&TOPS, &[Has(Test::Set(CardSet::Artifacts))]),
            Destination::Board(Demander),
        )],
    ),
    (
        N,
        &[
            if_(&holds(&filtered(&TOPS, &[Has(Test::Code("A018"))])), 1),
            WIN,
        ],
    ),
];

// Meld a card from your hand. Score a card from your hand. If you do both,
// and the cards have different values, junk all cards in the decks of both
// values.
const BAGHDAD_BATTERY: &[(EffectKind, Program)] = &[(
    N,
    &[
        meld(HAND).keep(MOVED),
        score(HAND).keep(SCORED),
        if_(
            &All(&[
                Not(&Empty(MOVED)),
                Not(&Empty(SCORED)),
                Not(&Compare(MOVED_VALUE, Cmp::Equal, Value(Reg::Slot(SCORED)))),
            ]),
            2,
        ),
        deck_junk(MOVED_VALUE),
        deck_junk(Value(Reg::Slot(SCORED))),
    ],
)];

// Draw and meld a [3]. Meld your bottom card of the drawn card's color, then
// self-execute it.
const SEIKILOS_EPITAPH: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(3),
        meld(LAST).keep(MOVED),
        if_(DID, 3),
        meld(Board(
            Executor,
            ColorSet::Of(Reg::Slot(MOVED)),
            BoardRegion::Bottom,
        )),
        BIND_LAST,
        SELF_EXECUTE,
    ],
)];

// Choose a set. Draw two [2] from that set. Meld one and transfer the other
// to an opponent's board.
const ROSETTA_STONE: &[(EffectKind, Program)] = &[(
    N,
    &[
        draws(Const(2), 2, Then::Keep).any_set().keep(DRAWN),
        meld(DRAWN_IN_HAND),
        transfer(DRAWN_IN_HAND, Destination::Board(Opponent)),
    ],
)];

// Draw an Artifact of value equal to the value of your highest top card.
// Junk the Artifact deck of that value. / Choose a player. Junk an
// available achievement of value equal to the highest top card on that
// player's board.
const DEAD_SEA_SCROLLS: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            draw_age(Max(&TOPS)).set(CardSet::Artifacts),
            junk(Selector::SetDeck(CardSet::Artifacts, Max(&TOPS)))
                .all()
                .shortcut(Shortcut::DeckTop),
        ],
    ),
    (
        N,
        &[
            // Only the chosen player's highest top value matters, so the value is
            // chosen instead. With equal values, or no available achievement of either,
            // both answers junk the same card or none: nothing is asked.
            if_(&Not(&DEAD_SEA_CHOICE_MATTERS), 2),
            junk(DEAD_SEA_MINE),
            Instruction::Jump(8),
            // An empty board's player stands for value 0 (an Age 0
            // achievement can be available with Figures).
            if_(&Any(&[Not(&holds(&TOPS)), Not(&holds(&Tops(Opponent)))]), 5),
            choose(
                Executor,
                Choice::Branch(&[(0, "Value 0"), (2, "The highest top value")]),
            ),
            junk(filtered(&AvailableAchievements, &[same_value(Const(0))])),
            Instruction::Jump(1),
            junk(filtered(
                &AvailableAchievements,
                &[same_value(Max(&Tops(Either)))],
            )),
            Instruction::Jump(2),
            choose(
                Executor,
                Choice::Value(ValueChoices::Among(&DEAD_SEA_HIGHEST_TOPS)),
            )
            .alias(Alias::NoOpWhen(&NO_CHOSEN_ACHIEVEMENT)),
            junk(filtered(&AvailableAchievements, &[CHOSEN_VALUE])),
        ],
    ),
];
/// No available achievement of the chosen value: nothing to junk.
const NO_CHOSEN_ACHIEVEMENT: Cond = Compare(
    Count(&filtered(&AvailableAchievements, &[CHOSEN_VALUE])),
    Cmp::Equal,
    Const(0),
);
const DEAD_SEA_HIGHEST_TOPS: Selector = Selector::Union(&[
    filtered(&TOPS, &[Highest]),
    filtered(&Tops(Opponent), &[Highest]),
]);
const DEAD_SEA_MINE: Selector = filtered(&AvailableAchievements, &[same_value(Max(&TOPS))]);
const DEAD_SEA_THEIRS: Selector =
    filtered(&AvailableAchievements, &[same_value(Max(&Tops(Opponent)))]);
const DEAD_SEA_CHOICE_MATTERS: Cond = All(&[
    Not(&Compare(Max(&TOPS), Cmp::Equal, Max(&Tops(Opponent)))),
    Any(&[holds(&DEAD_SEA_MINE), holds(&DEAD_SEA_THEIRS)]),
]);

// Splay left a color on any player's board. / Choose any top purple card
// other than Cyrus Cylinder on any player's board. Self-execute it.
const CYRUS_CYLINDER: &[(EffectKind, Program)] = &[
    (
        N,
        &[Instruction::Splay(SplayStep {
            who: Either,
            choices: SplayChoices::Fixed {
                colors: ALL_COLORS,
                direction: Splay::Left,
                from: None,
            },
            // Any nonempty pile: a no-op choice is allowed.
            targets: SplayTargets::Nonempty,
            optional: false,
            all: false,
        })],
    ),
    (
        N,
        &[
            bind(filtered(
                &Tops(Either),
                &[Has(ColorIs(Purple)), Lacks(Is(Reg::Source))],
            )),
            SELF_EXECUTE,
        ],
    ),
];

// I COMPEL you to transfer a top card of higher value than my top card of
// the same color from your board to my board!
const EXCALIBUR: &[(EffectKind, Program)] = &[(
    C,
    &[transfer(
        filtered(&TOPS, &[Has(Test::AboveTop(Demander))]),
        Destination::Board(Demander),
    )],
)];

// I COMPEL you to choose a top card on your board! Transfer all cards of
// that card's color from your board to my score pile!
const MJOLNIR_AMULET: &[(EffectKind, Program)] = &[(
    C,
    &[
        choose(Executor, Choice::Card(&TOPS)),
        transfer(
            Board(
                Executor,
                ColorSet::Of(Reg::Chosen),
                BoardRegion::BelowTop(0),
            ),
            Destination::Score(Demander),
        )
        .all()
        .shortcut(Shortcut::AnyOrder),
    ],
)];

// I COMPEL you to reveal all cards in your hand and transfer the card of my
// choice to my board! If you do, junk all cards in the deck of the chosen
// card's value!
const MOYLOUGH_BELT_SHRINE: &[(EffectKind, Program)] = &[(
    C,
    &[
        reveal_all(HAND).keep(SHOWN),
        choose(Demander, Choice::Card(&SHOWN_IN_HAND)),
        transfer(Selector::Reg(Reg::Chosen), Destination::Board(Demander)).keep(MOVED),
        if_(DID, 1),
        deck_junk(MOVED_VALUE),
    ],
)];

// Draw and reveal a [4]. If it is yellow, tuck it. If it is purple, score
// it. Otherwise, repeat this effect.
const ALONG_THE_RIVER: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(4),
        reveal(LAST),
        BIND_LAST,
        if_(&Card(Reg::Bound, Has(ColorIs(Yellow))), 2),
        tuck(BOUND),
        // "Otherwise" is neither yellow nor purple.
        Instruction::Jump(4),
        if_(&Card(Reg::Bound, Has(ColorIs(Purple))), 2),
        score(BOUND),
        Instruction::Jump(1),
        REPEAT,
    ],
)];

// Return a card from your hand. Score a number of cards from your hand equal
// to the value of the card you return. Junk all cards in the deck of value
// equal to the total value of the cards you score.
const PHILOSOPHERS_STONE: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).keep(MOVED),
        if_(DID, 2),
        score(HAND)
            .exactly(MOVED_VALUE)
            .shortcut(ORIGIN)
            .keep(SCORED),
        deck_junk(Expr::Total(&Selector::Slot(SCORED))),
    ],
)];

/// Colors of unsplayed piles on the executor's board.
const UNSPLAYED_TOPS: Selector = filtered(&TOPS, &[Lacks(Test::StackSplayed(None))]);
const CHOSEN_STACK: Selector = Board(Executor, ColorSet::Chosen, BoardRegion::BelowTop(0));
// Splay right an unsplayed color on your board. Junk an available
// achievement of value equal to the number of cards of that color on your
// board. If you don't, draw a card of that value.
const IFE_HEAD: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Color(ColorSet::OfCards(&UNSPLAYED_TOPS))),
        splay_set(ColorSet::Chosen, Splay::Right),
        junk(filtered(
            &AvailableAchievements,
            &[same_value(Count(&CHOSEN_STACK))],
        )),
        if_(&Not(DID), 1),
        draw_age(Count(&CHOSEN_STACK)),
    ],
)];

// Return all cards in your hand. Draw a card of value equal to the number of
// cards you return.
const DUNHUANG_STAR_CHART: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).all().shortcut(ORIGIN_ALL),
        draw_age(Expr::Done),
    ],
)];

// Reveal a card from your hand. If you do, splay left its color on your
// board, then choose a top card other than Yata No Kagami of that color on
// any board and self-execute it.
const YATA_NO_KAGAMI: &[(EffectKind, Program)] = &[(
    N,
    &[
        reveal(HAND).keep(SHOWN),
        if_(DID, 3),
        splay_set(ColorSet::Of(Reg::Slot(SHOWN)), Splay::Left),
        bind(filtered(
            &Tops(Either),
            &[Has(ColorOf(Reg::Slot(SHOWN))), Lacks(Is(Reg::Source))],
        )),
        SELF_EXECUTE,
    ],
)];

const UNSPLAY_ALL: Instruction = Instruction::Splay(SplayStep {
    who: Executor,
    choices: SplayChoices::Fixed {
        colors: ALL_COLORS,
        direction: Splay::None,
        from: None,
    },
    targets: SplayTargets::Changes,
    optional: false,
    all: true,
});
// Draw and reveal a [3]. If it is: Yellow: Return all cards in your hand.
// Green: Unsplay all your colors. Red: Return all cards in your score pile.
// Blue: Draw a [9].
const NECRONOMICON: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(3),
        reveal(LAST),
        BIND_LAST,
        if_(&Card(Reg::Bound, Has(ColorIs(Yellow))), 1),
        return_(HAND).all().shortcut(ORIGIN_ALL),
        if_(&Card(Reg::Bound, Has(ColorIs(Green))), 1),
        UNSPLAY_ALL,
        if_(&Card(Reg::Bound, Has(ColorIs(Red))), 1),
        return_(SCORE).all().shortcut(ORIGIN_ALL),
        if_(&Card(Reg::Bound, Has(ColorIs(Blue))), 1),
        draw(9),
    ],
)];

// Return a card from your hand. If you do, return a top card of the same
// color from your board and a card of the same color from your score pile.
// If you do all three, claim an available achievement ignoring eligibility.
const SHROUD_OF_TURIN: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).verify().keep(MOVED),
        if_(DID, 5),
        return_(filtered(&TOPS, &[Has(ColorOf(Reg::Slot(MOVED)))])).add(MOVED),
        reveal_all(filtered(&SCORE, &[Lacks(Test::Inspectable(Executor))])).keep(PROOF),
        return_(filtered(&VERIFIED_SCORE, &[Has(ColorOf(Reg::Slot(MOVED)))]))
            .verify()
            .reveal_if_empty(&SCORE)
            .add(MOVED),
        if_(
            &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(3)),
            1,
        ),
        CLAIM_ANY,
    ],
)];

/// Values recorded before a return, so later draws know which score piles
/// contributed.
const MINE_BEFORE: Slot = Slot::effect(1);
const THEIRS_BEFORE: Slot = Slot::effect(2);
const CHOSEN_VALUE: Filter = Has(ValueIs(Cmp::Equal, Expr::Chosen));
/// Neither score pile holds a card of the chosen value (public: backs show
/// values), so every such value is the same empty choice.
const NO_CHOSEN_SCORE_EITHER: Cond = Compare(
    Count(&filtered(&Selector::Score(Either), &[CHOSEN_VALUE])),
    Cmp::Equal,
    Const(0),
);
/// The executor's score pile holds no card of the chosen value.
const NO_CHOSEN_SCORE: Cond = Compare(
    Count(&filtered(&SCORE, &[CHOSEN_VALUE])),
    Cmp::Equal,
    Const(0),
);
// Choose a value other than [5]. Return all cards of that value from all
// score piles. For each score pile from which cards are returned, draw and
// score a [5]. If you score only [5]s, junk all cards in the deck of the
// chosen value.
const EAST_INDIA_COMPANY_CHARTER: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Value(ValueChoices::AnyExceptBoxed(&[5])))
            .alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE_EITHER)),
        // A pile contributes when it holds cards of the value: the whole
        // return is mandatory, so this is what the returns will take.
        Instruction::Bind(Binding::Value(
            MINE_BEFORE,
            Count(&filtered(&SCORE, &[CHOSEN_VALUE])),
        )),
        Instruction::Bind(Binding::Value(
            THEIRS_BEFORE,
            Count(&filtered(&Selector::Score(Opponent), &[CHOSEN_VALUE])),
        )),
        return_(filtered(&Selector::Score(Either), &[CHOSEN_VALUE]))
            .all()
            .shortcut(ORIGIN_ALL),
        if_(&Compare(Expr::Slot(MINE_BEFORE), Cmp::Greater, Const(0)), 2),
        draw(5),
        score(LAST).add(SCORED),
        if_(
            &Compare(Expr::Slot(THEIRS_BEFORE), Cmp::Greater, Const(0)),
            2,
        ),
        draw(5),
        score(LAST).add(SCORED),
        if_(
            &All(&[
                Not(&Empty(SCORED)),
                Not(&holds(&filtered(
                    &Selector::Slot(SCORED),
                    &[Lacks(Test::BoxedValues(&[5]))],
                ))),
            ]),
            1,
        ),
        deck_junk(Expr::Chosen),
    ],
)];

// I COMPEL you to transfer all the cards of the value of my choice from
// your score pile to my score pile! If you do, transfer a top card on your
// board of that value to my board!
const GALLEON_ATOCHA: &[(EffectKind, Program)] = &[(
    C,
    &[
        choose(Demander, Choice::Value(ValueChoices::Any)).alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE)),
        transfer(
            filtered(&SCORE, &[CHOSEN_VALUE]),
            Destination::Score(Demander),
        )
        .all()
        .shortcut(ORIGIN_ALL),
        if_(DID_ANY, 1),
        transfer(
            filtered(&TOPS, &[CHOSEN_VALUE]),
            Destination::Board(Demander),
        ),
    ],
)];

// I COMPEL you to transfer a top card with [Prosperity] of each color from
// your board to my score pile! / Score a top card on your board with
// [Prosperity].
const MOSES: &[(EffectKind, Program)] = &[
    (
        C,
        &[transfer(
            filtered(&TOPS, &[Has(HasIcon(Icon::Prosperity))]),
            Destination::Score(Demander),
        )
        .per_color(None)
        .shortcut(Shortcut::AnyOrder)],
    ),
    (
        N,
        &[score(filtered(&TOPS, &[Has(HasIcon(Icon::Prosperity))]))],
    ),
];

/// Mona Lisa's chosen color and the number of that color held when the
/// number was chosen.
const LISA_COLOR: Slot = Slot::effect(2);
const LISA_CARDS: Selector = filtered(&HAND, &[Has(ColorIn(ColorSet::Slot(LISA_COLOR)))]);
const LISA_DRAWN: Selector = filtered(
    &Selector::Slot(DRAWN),
    &[Has(ColorIn(ColorSet::Slot(LISA_COLOR)))],
);
// Choose a number and a color. Draw five [4], then reveal your hand. If you have
// exactly that many cards of that color, score them, and splay right your cards of that
// color. Otherwise, return all cards from your hand. Choose the
// color, then how many of the five draws will have it (0-5, or 6 to fail deliberately),
// tested against the recorded draws so no private held count is retained. When a Figure
// could change the hand during the draws, the literal final count is chosen instead.
const MONA_LISA: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Colors(1)).keep(LISA_COLOR),
        choose(
            Executor,
            Choice::FinalCount {
                colors: ColorSet::Slot(LISA_COLOR),
                draws: 5,
            },
        ),
        draws(Const(5), 4, Then::Keep).keep(DRAWN),
        reveal_all(HAND),
        if_(
            &Cond::FinalCount {
                hand: &LISA_CARDS,
                drawn: &LISA_DRAWN,
            },
            3,
        ),
        score(LISA_CARDS).all().shortcut(ORIGIN_ALL),
        splay_set(ColorSet::Slot(LISA_COLOR), Splay::Right),
        Instruction::Jump(1),
        return_(HAND).all().shortcut(ORIGIN_ALL),
    ],
)];

// Return all cards from your hand. / Draw three [4]. Meld a green card from
// your hand. Junk all cards in the deck of value equal to your top green
// card.
const MOLASSES_REEF_CARAVEL: &[(EffectKind, Program)] = &[
    (N, &[return_(HAND).all().shortcut(ORIGIN_ALL)]),
    (
        N,
        &[
            draws(Const(3), 4, Then::Keep),
            meld(filtered(&HAND, &[Has(ColorIs(Green))])).reveal_if_empty(&HAND),
            deck_junk(Max(&TopOf(Executor, Green))),
        ],
    ),
];

// If you have fewer than four cards in your hand, return your top card of
// each non-green color. Draw a [5] for each card you return. / Meld a card
// from your hand.
const HUNT_LENOX_GLOBE: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            if_(&Compare(Count(&HAND), Cmp::Less, Const(4)), 2),
            return_(filtered(&TOPS, &[Lacks(ColorIs(Green))]))
                .per_color(None)
                .shortcut(ORIGIN_ALL),
            draws(Expr::Done, 5, Then::Keep),
        ],
    ),
    (N, &[meld(HAND)]),
];

// I COMPEL you to transfer a card from your score pile to my score pile for
// every color with [Authority] on your board! / Junk an available
// achievement of value equal to the number of [Authority] on your board.
const PETITION_OF_RIGHT: &[(EffectKind, Program)] = &[
    (
        C,
        &[transfer(SCORE, Destination::Score(Demander))
            .exactly(Count(&filtered(&TOPS, &[Has(Test::StackShows(Authority))])))
            .shortcut(ORIGIN)],
    ),
    (
        N,
        &[junk(filtered(
            &AvailableAchievements,
            &[same_value(Expr::Metric(Executor, Metric::Icons(Authority)))],
        ))],
    ),
];

// Return a card from your score pile. If you do, draw a [5] and a [6], then
// reveal one of the drawn cards that has an icon in common with the
// returned card. If you cannot, return the drawn cards and repeat this
// effect.
const DELFT_POCKET_TELESCOPE: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(SCORE).verify().keep(MOVED),
        if_(DID, 6),
        draw(5).add(DRAWN),
        draw(6).add(DRAWN),
        reveal(filtered(
            &DRAWN_IN_HAND,
            &[Has(Test::SharesIcon(Reg::Slot(MOVED)))],
        )),
        if_(&Not(DID), 2),
        // Returning both proves neither shares an icon.
        return_(DRAWN_IN_HAND).all().verify().shortcut(ORIGIN_ALL),
        REPEAT,
    ],
)];

// Reveal your hand. If you have exactly five cards and five colors in your
// hand, you win.
const CROSS_OF_CORONADO: &[(EffectKind, Program)] = &[(
    N,
    &[
        reveal_all(HAND),
        if_(
            &All(&[
                Compare(Count(&HAND), Cmp::Equal, Const(5)),
                Compare(DistinctColors(&HAND), Cmp::Equal, Const(5)),
            ]),
            1,
        ),
        WIN,
    ],
)];

/// A block's item: a value each pass.
const ITEM: Slot = Slot::effect(1);
// For each value of top card on your board appearing exactly once, draw and
// score a card of that value in ascending order.
const ABELL_GALLERY_HARPSICHORD: &[(EffectKind, Program)] = &[(
    N,
    &[
        for_each(
            Domain::Values(&TOPS, Multiplicity::Exactly(1)),
            Order::Ascending,
            Some(ITEM),
            2,
        ),
        draw_age(Expr::Slot(ITEM)),
        score(LAST),
    ],
)];

const CHOSEN_OWN_STACK: Selector = Board(Executor, ColorSet::Chosen, BoardRegion::BelowTop(0));
// I COMPEL you to choose a color of which there are more visible cards on
// your board than on my board! From the bottom up, transfer all cards of
// that color from my board to my score pile, then from your board to my
// board!
const GALLEY_WHYDAH: &[(EffectKind, Program)] = &[(
    C,
    &[
        choose(
            Executor,
            Choice::Color(ColorSet::OfCards(&filtered(
                &TOPS,
                &[Has(Test::MoreVisibleCardsThan(Demander))],
            ))),
        ),
        transfer(
            Board(Demander, ColorSet::Chosen, BoardRegion::BelowTop(0)),
            Destination::Score(Demander),
        )
        .all()
        .shortcut(Shortcut::AnyOrder),
        // Bottom up: each card lands on top of the one before it.
        for_each(
            Domain::Cards(&CHOSEN_OWN_STACK),
            Order::Printed,
            Some(ITEM),
            1,
        ),
        transfer(
            filtered(&CHOSEN_OWN_STACK, &[Has(Is(Reg::Slot(ITEM)))]),
            Destination::Board(Demander),
        ),
    ],
)];

// If you have no cards in your score pile, choose a color and score all
// cards of that color on your board. Otherwise, return all cards from your
// score pile.
const SHIP_OF_THE_LINE_SUSSEX: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&Compare(Count(&SCORE), Cmp::Equal, Const(0)), 3),
        choose(Executor, Choice::Colors(1)),
        score(CHOSEN_OWN_STACK).all().shortcut(Shortcut::AnyOrder),
        Instruction::Jump(1),
        return_(SCORE).all().shortcut(ORIGIN_ALL),
    ],
)];

// If it is your turn, choose any other top card on any other board and
// super-execute it.
const GUJIN_TUSHU_JICHENG: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&OwnTurn, 2),
        bind(Selector::Tops(Opponent)),
        SUPER_EXECUTE,
    ],
)];

// Draw, reveal, and return a [6]. Draw and meld a card of value equal to
// the value of your top card of the same color as the returned card.
// Self-execute the melded card.
const THE_DAILY_COURANT: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(6),
        reveal(LAST),
        return_(LAST).keep(MOVED),
        draw_age(Max(&filtered(&TOPS, &[Has(ColorOf(Reg::Slot(MOVED)))]))),
        meld(LAST),
        BIND_LAST,
        SELF_EXECUTE,
    ],
)];

// Draw and meld a [5]. If the drawn card is not green, repeat this effect.
const BARQUE_LONGUE_LA_BELLE: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(5),
        meld(LAST),
        if_(&Not(&Card(Reg::Last, Has(ColorIs(Green)))), 1),
        REPEAT,
    ],
)];

// Score the bottom card of every color on your board. Meld a card from your
// score pile. Splay right the color of the melded card. Junk all cards in
// the deck of value equal to the melded card.
const HUDSONS_BAY_COMPANY_ARCHIVES: &[(EffectKind, Program)] = &[(
    N,
    &[
        score(Board(Executor, ALL_COLORS, BoardRegion::Bottom))
            .per_color(None)
            .shortcut(Shortcut::AnyOrder),
        meld(SCORE).keep(MOVED),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Right),
        deck_junk(MOVED_VALUE),
    ],
)];

// Return the lowest card in your hand and the lowest top card on your
// board. Draw and score a card of value equal to the sum of the values of
// the cards you return.
const BOERHAAVE_MICROSCOPE: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(filtered(&HAND, &[Filter::Lowest])).keep(MOVED),
        return_(filtered(&TOPS, &[Filter::Lowest])).add(MOVED),
        draw_age(Expr::Total(&Selector::Slot(MOVED))),
        score(LAST),
    ],
)];

// Return your top card of each non-blue color. For each card you return,
// draw and meld a card of value one higher than the value of the returned
// card, in ascending order.
const PRINCIPIA: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(filtered(&TOPS, &[Lacks(ColorIs(Blue))]))
            .per_color(None)
            .shortcut(ORIGIN_ALL)
            .keep(MOVED),
        for_each(
            Domain::Values(&Selector::Slot(MOVED), Multiplicity::Every),
            Order::Ascending,
            Some(ITEM),
            2,
        ),
        draw_age(Add(&Expr::Slot(ITEM), 1)),
        meld(LAST),
    ],
)];

// Draw and reveal a [6]. If it is red, claim an available standard
// achievement, ignoring eligibility. Otherwise, junk an available standard
// achievement.
const SANDHAM_ROOM_CRICKET_BAT: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(6),
        reveal(LAST),
        if_(&Card(Reg::Last, Has(ColorIs(Red))), 2),
        achieve(Selector::StandardAchievements),
        Instruction::Jump(1),
        junk(Selector::StandardAchievements),
    ],
)];

// Meld a card from your hand. If you do, claim an available achievement of
// matching value, ignoring eligibility. Otherwise, junk all cards in the
// deck of value equal to the lowest available achievement, if there is one.
const ALMIRA_QUEEN_OF_CASTILE: &[(EffectKind, Program)] = &[(
    N,
    &[
        meld(HAND).keep(MOVED),
        if_(DID, 2),
        achieve(filtered(&AvailableAchievements, &[same_value(MOVED_VALUE)])),
        Instruction::Jump(1),
        deck_junk(Min(&Selector::StandardAchievements)),
    ],
)];

// I COMPEL you to reveal a card in your hand! If you do, and its value is
// equal to the value of any of my top cards, return it and all cards of its
// color from your board!
const FRIGATE_CONSTITUTION: &[(EffectKind, Program)] = &[(
    C,
    &[
        reveal(HAND).keep(SHOWN),
        if_(
            &Card(Reg::Slot(SHOWN), Has(ValueIn(&Selector::Tops(Demander)))),
            2,
        ),
        return_(SHOWN_IN_HAND),
        return_(Board(
            Executor,
            ColorSet::Of(Reg::Slot(SHOWN)),
            BoardRegion::BelowTop(0),
        ))
        .all()
        .shortcut(ORIGIN_ALL),
    ],
)];

// I COMPEL you to transfer the highest card in your hand to my hand, the
// highest card in your score pile to my score pile, and the highest top
// card with [Industry] from your board to my board!
const US_DECLARATION_OF_INDEPENDENCE: &[(EffectKind, Program)] = &[(
    C,
    &[
        transfer(filtered(&HAND, &[Highest]), DEMANDER_HAND),
        transfer(filtered(&SCORE, &[Highest]), Destination::Score(Demander)),
        transfer(
            filtered(&TOPS, &[Has(HasIcon(Icon::Industry)), Highest]),
            Destination::Board(Demander),
        ),
    ],
)];

// I COMPEL you to transfer your top red card to my score pile! Transfer
// your top green card to my board! Transfer a yellow card from your score
// pile to mine! Transfer a purple card from your score pile to my hand!
const QIANLONGS_DRAGON_ROBE: &[(EffectKind, Program)] = &[(
    C,
    &[
        transfer(TopOf(Executor, Red), Destination::Score(Demander)),
        transfer(TopOf(Executor, Green), Destination::Board(Demander)),
        transfer(
            filtered(&SCORE, &[Has(ColorIs(Yellow))]),
            Destination::Score(Demander),
        )
        .verify()
        .reveal_if_empty(&SCORE),
        transfer(filtered(&SCORE, &[Has(ColorIs(Purple))]), DEMANDER_HAND)
            .verify()
            .reveal_if_empty(&SCORE),
    ],
)];

// Draw and meld a [6]. If the drawn card's color is the color with the
// fewest (or tied) number of visible cards on your board, score the melded
// card, and repeat this effect.
const PRIDE_AND_PREJUDICE: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(6),
        meld(LAST).keep(MOVED),
        if_(
            &Not(&holds(&filtered(
                &TOPS,
                &[Has(Test::FewerVisibleCardsThanColorOf(Reg::Slot(MOVED)))],
            ))),
            2,
        ),
        score(Selector::Reg(Reg::Slot(MOVED))),
        REPEAT,
    ],
)];

/// The Wealth of Nations' computed value: the score pile's total value
/// divided by five, rounded up.
const WEALTH: Slot = Slot::effect(1);
// Draw and score a [1]. Add up the values of all cards in your score pile,
// divide by five, and round up. Draw and score a card of value equal to the
// result. Junk all cards in the deck of that value.
const THE_WEALTH_OF_NATIONS: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(1),
        score(LAST),
        Instruction::Bind(Binding::Value(
            WEALTH,
            Expr::Div(&Add(&Expr::Total(&SCORE), 4), 5),
        )),
        draw_age(Expr::Slot(WEALTH)),
        score(LAST),
        deck_junk(Expr::Slot(WEALTH)),
    ],
)];

const BUTTONWOOD_COLORS: Slot = Slot::effect(1);
// Choose three colors. Draw and reveal an [8]. If the drawn card is one of
// the chosen colors, score it and splay up that color on your board.
// Otherwise, return all cards of that color from your score pile, and
// unsplay that color.
const BUTTONWOOD_AGREEMENT: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Colors(3)).keep(BUTTONWOOD_COLORS),
        draw(8),
        reveal(LAST),
        BIND_LAST,
        if_(
            &Card(Reg::Bound, Has(ColorIn(ColorSet::Slot(BUTTONWOOD_COLORS)))),
            3,
        ),
        score(BOUND),
        splay_set(ColorSet::Of(Reg::Bound), Splay::Up),
        Instruction::Jump(3),
        return_(filtered(&SCORE, &[Has(ColorOf(Reg::Bound))]))
            .all()
            .verify()
            .shortcut(ORIGIN_ALL),
        // The rest of the score pile proves no card of that color remains.
        reveal_all(SCORE),
        splay_set(ColorSet::Of(Reg::Bound), Splay::None),
    ],
)];

const RETURNED_PAIR: Cond = Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(2));
// Return a card from your hand. Return a top card from your board. If you
// return two cards and their values sum to ten, draw and score a [10].
// Otherwise, junk all cards in the deck of value equal to the sum.
const KILOGRAM_OF_THE_ARCHIVES: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).keep(MOVED),
        return_(TOPS).add(MOVED),
        if_(
            &All(&[
                RETURNED_PAIR,
                Compare(Expr::Total(&Selector::Slot(MOVED)), Cmp::Equal, Const(10)),
            ]),
            3,
        ),
        draw(10),
        score(LAST),
        Instruction::Jump(1),
        deck_junk(Expr::Total(&Selector::Slot(MOVED))),
    ],
)];

// Tuck a card from your hand. Splay right its color on your board. Draw a
// card of value equal to the highest number of icons of the same type in
// that color on your board.
const PUFFING_BILLY: &[(EffectKind, Program)] = &[(
    N,
    &[
        tuck(HAND).keep(MOVED),
        if_(DID, 2),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Right),
        draw_age(Expr::Visible(&Visibility {
            who: Executor,
            colors: ColorSet::Of(Reg::Slot(MOVED)),
            seen: Seen::MostCommonIcon,
        })),
    ],
)];

// Choose a color on your board having the highest top card. Meld your
// bottom card of that color. / Claim an available standard achievement,
// ignoring eligibility. Junk an available standard achievement.
const MOONLIGHT_SONATA: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(
                Executor,
                Choice::Color(ColorSet::OfCards(&filtered(&TOPS, &[Highest]))),
            ),
            meld(Board(Executor, ColorSet::Chosen, BoardRegion::Bottom)),
        ],
    ),
    (
        N,
        &[
            achieve(Selector::StandardAchievements),
            junk(Selector::StandardAchievements),
        ],
    ),
];

// Reveal and return two cards from your hand. If they have the same value,
// draw a card of value one higher. If they have the same color, claim an
// available achievement, ignoring eligibility.
const MARCHA_REAL: &[(EffectKind, Program)] = &[(
    N,
    &[
        reveal(HAND).add(SHOWN),
        reveal(filtered(&HAND, &[Lacks(InSlot(SHOWN))])).add(SHOWN),
        return_(SHOWN_IN_HAND)
            .all()
            .shortcut(ORIGIN_ALL)
            .keep(MOVED),
        if_(
            &All(&[
                RETURNED_PAIR,
                Compare(Expr::Distinct(&Selector::Slot(MOVED)), Cmp::Equal, Const(1)),
            ]),
            1,
        ),
        draw_age(Add(&Max(&Selector::Slot(MOVED)), 1)),
        if_(
            &All(&[
                RETURNED_PAIR,
                Compare(DistinctColors(&Selector::Slot(MOVED)), Cmp::Equal, Const(1)),
            ]),
            1,
        ),
        CLAIM_ANY,
    ],
)];

const EVEN: Test = Test::Values(&[0, 2, 4, 6, 8, 10]);
// I COMPEL you to draw and meld a [7]! Reveal your bottom card of the
// melded card's color! If the revealed card is even-valued, return all
// cards of its color from your board!
const SUBMARINE_HUNLEY: &[(EffectKind, Program)] = &[(
    C,
    &[
        draw(7),
        meld(LAST).keep(MOVED),
        if_(DID, 4),
        reveal(Board(
            Executor,
            ColorSet::Of(Reg::Slot(MOVED)),
            BoardRegion::Bottom,
        ))
        .keep(SHOWN),
        if_(&Card(Reg::Slot(SHOWN), Has(EVEN)), 1),
        return_(Board(
            Executor,
            ColorSet::Of(Reg::Slot(SHOWN)),
            BoardRegion::BelowTop(0),
        ))
        .all()
        .shortcut(ORIGIN_ALL),
    ],
)];

// Draw and score an [8]. / Draw and meld an [8]. If it is an [8], choose a
// value, and junk all cards in the deck of that value.
const JEDLIKS_SELF_ROTOR: &[(EffectKind, Program)] = &[
    (N, &[draw(8), score(LAST)]),
    (
        N,
        &[
            draw(8),
            meld(LAST).keep(MOVED),
            if_(&Card(Reg::Slot(MOVED), Has(Test::BoxedValues(&[8]))), 2),
            choose(Executor, Choice::Value(ValueChoices::Any))
                .alias(Alias::NoOpWhen(&EMPTY_CHOSEN_DECK)),
            deck_junk(Expr::Chosen),
        ],
    ),
];

// I COMPEL you to reveal your hand! Draw a [7]! If the color of the drawn
// card matches the color of any other card in your hand, return all cards
// in your hand and all cards in your score pile!
const COLT_PATERSON_REVOLVER: &[(EffectKind, Program)] = &[(
    C,
    &[
        reveal_all(HAND).keep(SHOWN),
        draw(7).keep(DRAWN),
        // Its color decides the returns, so it is shown.
        reveal(LAST),
        if_(
            &Card(
                Reg::Slot(DRAWN),
                Has(ColorIn(ColorSet::OfCards(&SHOWN_IN_HAND))),
            ),
            2,
        ),
        return_(HAND).all().shortcut(ORIGIN_ALL),
        return_(SCORE).all().shortcut(ORIGIN_ALL),
    ],
)];

// Tuck a card from your hand. If you do, splay up its color, and then tuck
// all cards from your score pile of that color. If you do, junk an
// available standard achievement.
// A splay that changes nothing (already up, or a lone card) is not done.
const SPLAYED_UP: Slot = Slot::effect(1);
const SINGER_MODEL_27: &[(EffectKind, Program)] = &[(
    N,
    &[
        tuck(HAND).keep(MOVED),
        if_(DID, 7),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Up),
        if_(DID, 1),
        Instruction::Bind(Binding::Value(SPLAYED_UP, Const(1))),
        tuck(filtered(&SCORE, &[Has(ColorOf(Reg::Slot(MOVED)))]))
            .all()
            .keep(SCORED),
        // The rest of the score pile proves no card of that color remains.
        reveal_all(SCORE),
        if_(
            &All(&[
                Not(&Empty(SCORED)),
                Compare(Expr::Slot(SPLAYED_UP), Cmp::Equal, Const(1)),
            ]),
            1,
        ),
        junk(Selector::StandardAchievements),
    ],
)];

// Choose a value. Draw and reveal three cards of that value. Splay up the
// colors of the cards. If the number of cards of each of those colors on
// your board is equal to that value, you win. Otherwise, return the drawn
// cards.
const METRE_BAR: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Value(ValueChoices::Any)),
        draws_age(Const(3), Expr::Chosen, Then::Reveal).keep(DRAWN),
        Instruction::Splay(SplayStep {
            who: Executor,
            choices: SplayChoices::Fixed {
                colors: ColorSet::OfCards(&Selector::Slot(DRAWN)),
                direction: Splay::Up,
                from: None,
            },
            targets: SplayTargets::Changes,
            optional: false,
            all: true,
        }),
        if_(
            &All(&[
                Not(&Empty(DRAWN)),
                Not(&holds(&filtered(
                    &Selector::Slot(DRAWN),
                    &[Lacks(Test::CardsOfColorOnBoard(
                        Executor,
                        Cmp::Equal,
                        Expr::Chosen,
                    ))],
                ))),
            ]),
            2,
        ),
        WIN,
        Instruction::Jump(1),
        return_(DRAWN_IN_HAND).all().shortcut(ORIGIN_ALL),
    ],
)];

// I COMPEL you to draw four [7]! Meld a blue card, then transfer all cards
// in your hand to my hand! / Draw and reveal a [7]. If it has no
// [Efficiency], tuck it and repeat this effect.
const HANSEN_WRITING_BALL: &[(EffectKind, Program)] = &[
    (
        C,
        &[
            draws(Const(4), 7, Then::Keep),
            meld(filtered(&HAND, &[Has(ColorIs(Blue))])).reveal_if_empty(&HAND),
            transfer(HAND, DEMANDER_HAND).all().shortcut(ORIGIN_ALL),
        ],
    ),
    (
        N,
        &[
            draw(7),
            reveal(LAST),
            if_(&Card(Reg::Last, Lacks(HasIcon(Icon::Efficiency))), 2),
            tuck(LAST),
            REPEAT,
        ],
    ),
];

/// Periodic Table's chosen pair of equal-value tops.
const PAIR: Slot = Slot::effect(1);
// Choose two top cards on your board of the same value. If you do, draw a
// card of value one higher and meld it. If you meld it over one of the
// chosen cards, repeat this effect.
const PERIODIC_TABLE: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(
            Executor,
            Choice::Card(&filtered(&TOPS, &[Has(Test::ValueRepeated(&TOPS))])),
        )
        .keep(PAIR),
        choose(
            Executor,
            Choice::Card(&filtered(
                &TOPS,
                &[
                    same_value(Value(Reg::Slot(PAIR))),
                    Lacks(Is(Reg::Slot(PAIR))),
                ],
            )),
        )
        .add(PAIR),
        if_(
            &Compare(Count(&Selector::Slot(PAIR)), Cmp::Equal, Const(2)),
            4,
        ),
        draw_age(Add(&Value(Reg::Slot(PAIR)), 1)),
        meld(LAST).keep(MOVED),
        if_(
            &holds(&filtered(
                &Board(
                    Executor,
                    ColorSet::Of(Reg::Slot(MOVED)),
                    BoardRegion::BelowTop(1),
                ),
                &[Has(InSlot(PAIR))],
            )),
            1,
        ),
        REPEAT,
    ],
)];

const CHALLENGER_COUNT: Slot = Slot::effect(1);
// Draw and tuck an [8]. Splay up the color of the tucked card. Draw and
// score a card of value equal to the number of cards of that color on your
// board. Junk all cards in the deck of that value.
const CORVETTE_CHALLENGER: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(8),
        tuck(LAST).keep(MOVED),
        if_(DID, 5),
        splay_set(ColorSet::Of(Reg::Slot(MOVED)), Splay::Up),
        Instruction::Bind(Binding::Value(
            CHALLENGER_COUNT,
            Count(&Board(
                Executor,
                ColorSet::Of(Reg::Slot(MOVED)),
                BoardRegion::BelowTop(0),
            )),
        )),
        draw_age(Expr::Slot(CHALLENGER_COUNT)),
        score(LAST),
        deck_junk(Expr::Slot(CHALLENGER_COUNT)),
    ],
)];

// Meld the highest card from your score pile. Draw and score two cards of
// value equal to the melded card. Self-execute the melded card.
const ROUNDHAY_GARDEN_SCENE: &[(EffectKind, Program)] = &[(
    N,
    &[
        meld(filtered(&SCORE, &[Highest])).keep(MOVED),
        if_(DID, 3),
        draws_age(Const(2), MOVED_VALUE, Then::Score),
        bind(Selector::Reg(Reg::Slot(MOVED))),
        SELF_EXECUTE,
    ],
)];

// For each player, draw and reveal a [7]. Transfer one of the drawn cards
// to each other player's board. Meld the last, and self-execute it.
const THE_COMMUNIST_MANIFESTO: &[(EffectKind, Program)] = &[(
    N,
    &[
        draws(Const(2), 7, Then::Reveal).keep(DRAWN),
        transfer(DRAWN_IN_HAND, Destination::Board(Opponent)),
        meld(DRAWN_IN_HAND),
        BIND_LAST,
        SELF_EXECUTE,
    ],
)];

// I COMPEL you to draw and reveal an [8]! Return all cards of the drawn
// card's color from your board!
const BATTLESHIP_BISMARCK: &[(EffectKind, Program)] = &[(
    C,
    &[
        draw(8),
        reveal(LAST),
        BIND_LAST,
        return_(Board(
            Executor,
            ColorSet::Of(Reg::Bound),
            BoardRegion::BelowTop(0),
        ))
        .all()
        .shortcut(ORIGIN_ALL),
    ],
)];

// Choose a value. Splay up each color on your board with a top card of the
// chosen value. Return all cards of the chosen value from all score piles.
/// No top card of the chosen value and no score card of it anywhere: the
/// value changes nothing.
const PLUSCH_IDLE: Cond = All(&[
    Compare(
        Count(&filtered(&TOPS, &[CHOSEN_VALUE])),
        Cmp::Equal,
        Const(0),
    ),
    NO_CHOSEN_SCORE_EITHER,
]);
const PLUSCH_ROD_BEAR: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Value(ValueChoices::Any)).alias(Alias::NoOpWhen(&PLUSCH_IDLE)),
        Instruction::Splay(SplayStep {
            who: Executor,
            choices: SplayChoices::Fixed {
                colors: ColorSet::OfCards(&filtered(&TOPS, &[CHOSEN_VALUE])),
                direction: Splay::Up,
                from: None,
            },
            targets: SplayTargets::Changes,
            optional: false,
            all: true,
        }),
        return_(filtered(&Selector::Score(Either), &[CHOSEN_VALUE]))
            .all()
            .shortcut(ORIGIN_ALL),
    ],
)];

const DRAWN_COLOR_EFFICIENCY_TOP: Selector = filtered(
    &TOPS,
    &[
        Has(ColorOf(Reg::Slot(DRAWN))),
        Has(HasIcon(Icon::Efficiency)),
    ],
);
// I COMPEL you to draw an [8]! If it is green, achieve Action Comics #1 if
// it is a top card! Otherwise, if it has [Efficiency], return it, and if
// your top card of its color has [Efficiency], transfer it to my
// achievements, and repeat this effect!
const ACTION_COMICS: &[(EffectKind, Program)] = &[(
    C,
    &[
        draw(8).keep(DRAWN),
        // Its color and icons decide the branch, so it is shown.
        reveal(LAST),
        if_(&Card(Reg::Slot(DRAWN), Has(ColorIs(Green))), 2),
        achieve(filtered(
            &Tops(Either),
            &[
                Has(Is(Reg::Source)),
                Has(Test::Eligible { score_factor: 1 }),
            ],
        )),
        Instruction::Jump(5),
        if_(&Card(Reg::Slot(DRAWN), Has(HasIcon(Icon::Efficiency))), 4),
        return_(DRAWN_IN_HAND),
        if_(&holds(&DRAWN_COLOR_EFFICIENCY_TOP), 2),
        transfer(
            DRAWN_COLOR_EFFICIENCY_TOP,
            Destination::Achievements(Demander),
        ),
        REPEAT,
    ],
)];

const LOWEST_DECK: Slot = Slot::effect(1);
/// Every Base deck, for "the deck of lowest value with a card".
const BASE_DECKS: Selector = Selector::Union(&[
    BaseDeck(Const(0)),
    BaseDeck(Const(1)),
    BaseDeck(Const(2)),
    BaseDeck(Const(3)),
    BaseDeck(Const(4)),
    BaseDeck(Const(5)),
    BaseDeck(Const(6)),
    BaseDeck(Const(7)),
    BaseDeck(Const(8)),
    BaseDeck(Const(9)),
    BaseDeck(Const(10)),
    BaseDeck(Const(11)),
]);
// Score your bottom card of each color. / Junk all cards from the deck of
// lowest value with a card.
const LINER_TITANIC: &[(EffectKind, Program)] = &[
    (
        N,
        &[score(Board(Executor, ALL_COLORS, BoardRegion::Bottom))
            .per_color(None)
            .shortcut(Shortcut::AnyOrder)],
    ),
    (
        N,
        &[
            Instruction::Bind(Binding::Value(LOWEST_DECK, Min(&BASE_DECKS))),
            deck_junk(Expr::Slot(LOWEST_DECK)),
        ],
    ),
];

// Return a card from your hand. Draw and score three cards of the returned
// card's value. If you don't, junk all cards in the deck of value equal to
// the highest scored card.
const DID_RETURN: Cond = Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(1));
const RETURNED_VALUE: Expr = Max(&Selector::Slot(MOVED));
const MEIJI_MURA: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).keep(MOVED),
        // With nothing returned, three 0s are drawn and the 0 deck, if it exists, is
        // junked: junking does not skip ahead like drawing.
        draws_age(Const(3), RETURNED_VALUE, Then::Score).keep(SCORED),
        if_(&Not(&DID_RETURN), 1),
        deck_junk(Const(0)),
        // "If you don't": fewer than three scores, or one of another value.
        if_(
            &All(&[
                DID_RETURN,
                Not(&All(&[
                    Compare(Count(&Selector::Slot(SCORED)), Cmp::Equal, Const(3)),
                    Not(&holds(&filtered(
                        &Selector::Slot(SCORED),
                        &[Lacks(ValueIs(Cmp::Equal, RETURNED_VALUE))],
                    ))),
                ])),
            ]),
            1,
        ),
        deck_junk(Max(&Selector::Slot(SCORED))),
    ],
)];

// Draw and meld a card of value one higher than the highest top card on
// your board. Junk an available standard achievement. If you don't, and
// the melded card has at least three [Efficiency], you win.
const PARNELL_PITCH_DROP: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw_age(Add(&Max(&TOPS), 1)),
        meld(LAST).keep(MOVED),
        junk(Selector::StandardAchievements),
        if_(
            &All(&[
                Not(DID),
                Compare(
                    Expr::IconsOn(Reg::Slot(MOVED), Icon::Efficiency),
                    Cmp::AtLeast,
                    Const(3),
                ),
            ]),
            1,
        ),
        WIN,
    ],
)];

// For each value below 9, junk a top card of that value from your board, in
// descending order. Then, if there is a junked card of each value below 9,
// you win. / Claim an available achievement, ignoring eligibility.
const EARHARTS_ELECTRA: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            for_each(Domain::Range(1, 8), Order::Descending, Some(ITEM), 1),
            junk(filtered(&TOPS, &[same_value(Expr::Slot(ITEM))])),
            if_(
                &Compare(
                    Expr::Distinct(&filtered(
                        &Selector::Junk,
                        &[
                            Has(ValueIs(Cmp::AtLeast, Const(1))),
                            Has(ValueIs(Cmp::AtMost, Const(8))),
                        ],
                    )),
                    Cmp::Equal,
                    Const(8),
                ),
                1,
            ),
            WIN,
        ],
    ),
    (N, &[CLAIM_ANY]),
];

// Meld an [8] from your hand. If the melded card has no effects, you win.
// Otherwise, self-execute it.
const GARLANDS_RUBY_SLIPPERS: &[(EffectKind, Program)] = &[(
    N,
    &[
        meld(filtered(&HAND, &[Has(Test::BoxedValues(&[8]))])).keep(MOVED),
        if_(DID, 5),
        if_(&Card(Reg::Slot(MOVED), Has(Test::NoPrintedEffects)), 2),
        WIN,
        Instruction::Jump(2),
        bind(Selector::Reg(Reg::Slot(MOVED))),
        SELF_EXECUTE,
    ],
)];

const DRAWN_COLOR_TOPS: Selector = filtered(&TOPS, &[Has(ColorOf(Reg::Bound))]);
const DRAWN_COLOR_OPPONENT_TOPS: Selector =
    filtered(&Selector::Tops(Opponent), &[Has(ColorOf(Reg::Bound))]);
// I COMPEL you to return one of your claimed standard achievements! / Draw
// and reveal an [8]. The single player with the highest top card of the
// drawn card's color achieves the top card, ignoring eligibility. If they
// do, repeat this effect.
const WORLD_CUP_FINAL_BALL: &[(EffectKind, Program)] = &[
    (C, &[return_(Selector::Achievements(Executor))]),
    (
        N,
        &[
            draw(8),
            reveal(LAST),
            BIND_LAST,
            if_(
                &Compare(
                    Max(&DRAWN_COLOR_TOPS),
                    Cmp::Greater,
                    Max(&DRAWN_COLOR_OPPONENT_TOPS),
                ),
                2,
            ),
            achieve(DRAWN_COLOR_TOPS).keep(MOVED),
            Instruction::Jump(2),
            if_(
                &Compare(
                    Max(&DRAWN_COLOR_OPPONENT_TOPS),
                    Cmp::Greater,
                    Max(&DRAWN_COLOR_TOPS),
                ),
                1,
            ),
            // The opponent's own claim, ignoring eligibility.
            achieve(DRAWN_COLOR_OPPONENT_TOPS)
                .to(Destination::Achievements(Opponent))
                .keep(MOVED),
            if_(&Not(&Empty(MOVED)), 1),
            REPEAT,
        ],
    ),
];

const DEMAND_TOPS: Selector = filtered(&TOPS, &[Has(Test::Demand)]);
// I COMPEL you to transfer the two bottom cards of each color which has a
// top card with a demand effect to my score pile! / If you have a top card
// on your board with a demand effect, score it, and draw a [10].
const CRUSADER_RABBIT: &[(EffectKind, Program)] = &[
    (
        C,
        &[transfer(
            Board(
                Executor,
                ColorSet::OfCards(&DEMAND_TOPS),
                BoardRegion::BottomN(2),
            ),
            Destination::Score(Demander),
        )
        .all()
        .frozen()
        .shortcut(Shortcut::AnyOrder)],
    ),
    (
        N,
        &[if_(&holds(&DEMAND_TOPS), 2), score(DEMAND_TOPS), draw(10)],
    ),
];

// I COMPEL you to transfer a [9] from your hand to my hand! If you don't,
// transfer a [9] from your score pile to my score pile! If you do neither,
// I win! / Score your highest top card.
const VELCRO_SHOES: &[(EffectKind, Program)] = &[
    (
        C,
        &[
            transfer(
                filtered(&HAND, &[Has(Test::BoxedValues(&[9]))]),
                DEMANDER_HAND,
            )
            .verify(),
            // Backs prove the values: a missing [9] needs no reveal.
            if_(&Not(DID), 3),
            transfer(
                filtered(&SCORE, &[Has(Test::BoxedValues(&[9]))]),
                Destination::Score(Demander),
            )
            .verify(),
            if_(&Not(DID), 1),
            Instruction::Win(WinRule::Player(Demander)),
        ],
    ),
    (N, &[score(filtered(&TOPS, &[Highest]))]),
];

// For each top card on your board with [Efficiency], draw and score a [9].
// / Junk all cards in the deck of value equal to the number of cards in
// your score pile.
const ROCK_AROUND_THE_CLOCK: &[(EffectKind, Program)] = &[
    (
        N,
        &[draws(
            Count(&filtered(&TOPS, &[Has(HasIcon(Icon::Efficiency))])),
            9,
            Then::Score,
        )],
    ),
    (N, &[deck_junk(Count(&SCORE))]),
];

// Draw and meld two [10]. If they are the same color, you win.
const MAGNAVOX_ODYSSEY: &[(EffectKind, Program)] = &[(
    N,
    &[
        for_each(Domain::Times(Const(2)), Order::Printed, None, 2),
        draw(10).add(DRAWN),
        meld(LAST),
        if_(
            &All(&[
                Compare(Count(&Selector::Slot(DRAWN)), Cmp::Equal, Const(2)),
                Compare(DistinctColors(&Selector::Slot(DRAWN)), Cmp::Equal, Const(1)),
            ]),
            1,
        ),
        WIN,
    ],
)];

// I COMPEL you to unsplay all splayed colors on your board! / Splay up two colors on
// your board. Any two colors on the board, a pile the splay leaves as it is included.
const PHILIPS_COMPACT_CASSETTE: &[(EffectKind, Program)] = &[
    (C, &[UNSPLAY_ALL]),
    (
        N,
        &[
            choose(Executor, Choice::ColorsAmong(2, ColorSet::OfCards(&TOPS))),
            Instruction::Splay(SplayStep {
                who: Executor,
                choices: SplayChoices::Fixed {
                    colors: ColorSet::Chosen,
                    direction: Splay::Up,
                    from: None,
                },
                targets: SplayTargets::Changes,
                optional: false,
                all: true,
            }),
        ],
    ),
];

// Return all cards from your hand. Draw and reveal five [9]. If you reveal
// five colors, you win.
const SYNCOM_3: &[(EffectKind, Program)] = &[(
    N,
    &[
        return_(HAND).all().shortcut(ORIGIN_ALL),
        draws(Const(5), 9, Then::Reveal).keep(DRAWN),
        if_(
            &Compare(DistinctColors(&Selector::Slot(DRAWN)), Cmp::Equal, Const(5)),
            1,
        ),
        WIN,
    ],
)];

// Draw and meld a [9], and self-execute it. If that card has [Efficiency],
// repeat this effect.
const YEAGERS_BELL_X1A: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(9),
        meld(LAST).keep(MOVED),
        BIND_LAST,
        SELF_EXECUTE,
        if_(&Card(Reg::Slot(MOVED), Has(HasIcon(Icon::Efficiency))), 1),
        REPEAT,
    ],
)];

// Return all cards from your score pile. Draw and score a card of value
// equal to the number of cards you return. / Choose a value. Junk all cards
// in that deck.
const LUNA_3: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            return_(SCORE).all().shortcut(ORIGIN_ALL),
            draw_age(Expr::Done),
            score(LAST),
        ],
    ),
    (
        N,
        &[
            choose(Executor, Choice::Value(ValueChoices::Any))
                .alias(Alias::NoOpWhen(&EMPTY_CHOSEN_DECK)),
            deck_junk(Expr::Chosen),
        ],
    ),
];

// Self-execute your top blue card. If this causes any change to occur,
// draw and junk a [10], then repeat this effect.
const THE_BIG_BANG: &[(EffectKind, Program)] = &[(
    N,
    &[
        bind(TopOf(Executor, Blue)),
        SELF_EXECUTE,
        if_(DID_ANY, 3),
        draw(10),
        junk(LAST),
        REPEAT,
    ],
)];

// You may score a card from your hand. You may transfer any card from your
// score pile to your hand. If you have exactly 25 points, you win. / Junk
// an available standard achievement.
const MARILYN_DIPTYCH: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            score(HAND).optional(),
            transfer(SCORE, Destination::Hand(Executor)).optional(),
            if_(
                &Compare(Expr::Metric(Executor, Metric::Score), Cmp::Equal, Const(25)),
                1,
            ),
            WIN,
        ],
    ),
    (N, &[junk(Selector::StandardAchievements)]),
];

// ---------------------------------------------------------------- Age 10

// I COMPEL you to junk all your cards! You lose! The junk is dropped: with two players
// the loss ends the game, and the achievement check waits for the end of the action, so
// nothing the junk could trigger (a City claim from the hand) changes the result.
const TANKER_EXXON_VALDEZ: &[(EffectKind, Program)] =
    &[(C, &[Instruction::Win(WinRule::Loses(Executor))])];

// I COMPEL you to return all cards in your hand but two! Return all cards in
// your score pile but two! / Return all cards in your score pile but four.
const MALDIVES: &[(EffectKind, Program)] = &[
    (
        C,
        &[
            return_(HAND)
                .exactly(Add(&Count(&HAND), -2))
                .shortcut(ORIGIN),
            return_(SCORE)
                .exactly(Add(&Count(&SCORE), -2))
                .shortcut(ORIGIN),
        ],
    ),
    (
        N,
        &[return_(SCORE)
            .exactly(Add(&Count(&SCORE), -4))
            .shortcut(ORIGIN)],
    ),
];

const BOTTOM_YELLOW: Selector = Board(Executor, ColorSet::Only(&[Yellow]), BoardRegion::Bottom);
// You may score your bottom yellow card. You may draw and tuck a [1]. If
// your bottom yellow card is Domestication, you win. Otherwise, meld the
// highest card in your hand, then draw an [11]. / Junk all available
// achievements.
const DOLLY_THE_SHEEP: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            score(BOTTOM_YELLOW).optional(),
            draw(1).optional(),
            tuck(LAST),
            if_(
                &holds(&filtered(&BOTTOM_YELLOW, &[Has(Test::Code("B005"))])),
                2,
            ),
            WIN,
            Instruction::Jump(2),
            meld(filtered(&HAND, &[Highest])),
            draw(11),
        ],
    ),
    (
        N,
        &[
            junk(Selector::StandardAchievements)
                .all()
                .shortcut(Shortcut::AnyOrder),
            Instruction::Special(SpecialStep::JunkAll),
        ],
    ),
];

// You win.
const WHERES_WALDO: &[(EffectKind, Program)] = &[(N, &[WIN])];

// If you have the most cards in your score pile, you win. Otherwise, draw and
// reveal an [11]. Score all cards on your board of the color of the drawn
// card. If you do, return the drawn card. Otherwise, meld it.
const MUSKS_TESLA_ROADSTER: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(
            &Compare(
                Count(&SCORE),
                Cmp::AtLeast,
                Count(&Selector::Score(Opponent)),
            ),
            2,
        ),
        WIN,
        Instruction::Jump(7),
        draw(11).keep(DRAWN),
        reveal(LAST),
        score(Board(
            Executor,
            ColorSet::Of(Reg::Slot(DRAWN)),
            BoardRegion::BelowTop(0),
        ))
        .all()
        .shortcut(Shortcut::AnyOrder),
        if_(DID_ANY, 2),
        return_(DRAWN_IN_HAND),
        Instruction::Jump(1),
        meld(DRAWN_IN_HAND),
    ],
)];

// If you have the most cards of a color visible on your board out of all
// colors on all boards, you win.
const SEIKAN_TUNNEL: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(
            &Compare(
                Expr::MostVisibleInAColor(Executor),
                Cmp::AtLeast,
                Expr::MostVisibleInAColor(Opponent),
            ),
            1,
        ),
        WIN,
    ],
)];

// Draw and meld an Artifact [10], then self-execute it.
const ROVER_CURIOSITY: &[(EffectKind, Program)] = &[(
    N,
    &[
        draw(10).set(CardSet::Artifacts),
        meld(LAST),
        BIND_LAST,
        SELF_EXECUTE,
    ],
)];

const WHOLE_BOARD: Selector = Board(Executor, ALL_COLORS, BoardRegion::BelowTop(0));
// Score all cards on your board.
const HIGGS_BOSON: &[(EffectKind, Program)] =
    &[(N, &[score(WHOLE_BOARD).all().shortcut(Shortcut::AnyOrder)])];

// If Delorean DMC-12 is a top card on any board, junk a top card of each
// color from each board and all cards in all hands.
const DELOREAN_DMC_12: &[(EffectKind, Program)] = &[(
    N,
    &[
        if_(&holds(&filtered(&Tops(Either), &[Has(Is(Reg::Source))])), 2),
        junk(Tops(Either))
            .all()
            .frozen()
            .shortcut(Shortcut::AnyOrder),
        junk(Selector::Hand(Either))
            .all()
            .shortcut(Shortcut::AnyOrder),
    ],
)];

// I COMPEL you to meld the lowest card in your score pile with a unique
// value! If you do, meld a card of the same color from your hand! If you
// meld one card total, you lose! Otherwise, junk the top card of each deck!
const DRAGONS_LAIR: &[(EffectKind, Program)] = &[(
    C,
    &[
        meld(filtered(
            &SCORE,
            &[Lacks(Test::ValueRepeated(&SCORE)), Filter::Lowest],
        ))
        .keep(MOVED),
        if_(DID, 1),
        meld(filtered(&HAND, &[Has(ColorOf(Reg::Slot(MOVED)))]))
            .reveal_if_empty(&HAND)
            .add(MOVED),
        if_(
            &Compare(Count(&Selector::Slot(MOVED)), Cmp::Equal, Const(1)),
            2,
        ),
        Instruction::Win(WinRule::Loses(Executor)),
        Instruction::Jump(1),
        // "Each deck" is every enabled set's decks, junked directly.
        junk(Selector::DeckTops)
            .all()
            .frozen()
            .shortcut(Shortcut::AnyOrder),
    ],
)];

// ---------------------------------------------------------------- Age 11

// Choose a value different from any top card on your board. Score all cards
// in the deck of that value. Score all cards in the junk of that value.
/// An empty deck and no junk of the chosen value: nothing to score.
const PSYCHE_IDLE: Cond = All(&[
    EMPTY_CHOSEN_DECK,
    Compare(
        Count(&filtered(&Selector::Junk, &[same_value(Expr::Chosen)])),
        Cmp::Equal,
        Const(0),
    ),
]);
const SIXTEEN_PSYCHE: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Value(ValueChoices::NotAmong(&TOPS)))
            .alias(Alias::NoOpWhen(&PSYCHE_IDLE)),
        score(BaseDeck(Expr::Chosen))
            .all()
            .shortcut(Shortcut::DeckTop),
        score(filtered(&Selector::Junk, &[same_value(Expr::Chosen)]))
            .all()
            .shortcut(Shortcut::AnyOrder),
    ],
)];

// I COMPEL you to junk all cards from your board! Draw and meld a card of
// each value in ascending order!
const PIZZA_PACAYA: &[(EffectKind, Program)] = &[(
    C,
    &[
        junk(WHOLE_BOARD).all().shortcut(Shortcut::AnyOrder),
        for_each(Domain::Range(1, 11), Order::Printed, Some(ITEM), 2),
        draw_age(Expr::Slot(ITEM)),
        meld(LAST),
    ],
)];

// Draw two [11]. Meld one of them, then meld the other and if it is your
// turn, super-execute it, otherwise self-execute it.
const WHAT_DOES_THE_FOX_SAY: &[(EffectKind, Program)] = &[(
    N,
    &[
        draws(Const(2), 11, Then::Keep).keep(DRAWN),
        meld(DRAWN_IN_HAND),
        meld(DRAWN_IN_HAND),
        BIND_LAST,
        if_(&OwnTurn, 2),
        SUPER_EXECUTE,
        Instruction::Jump(1),
        SELF_EXECUTE,
    ],
)];

const fn exchange(left: Selector, right: Selector) -> Instruction {
    Instruction::Exchange(ExchangeStep {
        left,
        right,
        counts: &[None, None],
        optional: false,
    })
}
/// Tasmanian Tiger's chosen score card and chosen top card.
const TIGER_SCORED: Slot = Slot::effect(0);
const TIGER_TOP: Slot = Slot::effect(1);
// Choose a card in your score pile. Choose a top card of the same color on any player's
// board. Exchange the two cards. You may return two cards from your hand. If you do,
// repeat this effect. With no top card of that color, the chosen score card is
// revealed: its front proves the mandatory exchange impossible.
const TASMANIAN_TIGER: &[(EffectKind, Program)] = &[(
    N,
    &[
        choose(Executor, Choice::Card(&SCORE)).keep(TIGER_SCORED),
        choose(
            Executor,
            Choice::Card(&filtered(
                &Tops(Either),
                &[Has(ColorOf(Reg::Slot(TIGER_SCORED)))],
            )),
        )
        .keep(TIGER_TOP),
        if_(&Empty(TIGER_TOP), 1),
        reveal(Selector::Slot(TIGER_SCORED)),
        if_(&Not(&Empty(TIGER_TOP)), 1),
        // The chosen card is a score pile card even where a Figure aliases
        // it (Wernher von Braun's junk, Michelangelo's hand).
        exchange(
            filtered(&SCORE, &[Has(InSlot(TIGER_SCORED))]),
            Selector::Slot(TIGER_TOP),
        ),
        return_(HAND).exactly(Const(2)).optional().shortcut(ORIGIN),
        if_(&Did(Outcome::Exactly(2)), 1),
        REPEAT,
    ],
)];

const EFFICIENCY_TOPS: Selector = filtered(&TOPS, &[Has(HasIcon(Icon::Efficiency))]);
const PROSPERITY_TOPS: Selector = filtered(&TOPS, &[Has(HasIcon(Icon::Prosperity))]);
// I COMPEL you to return one of your top cards with [Efficiency]! If you
// do, repeat this effect! / Return one of your top cards with [Prosperity].
// If you do, repeat this effect.
const GREENLAND: &[(EffectKind, Program)] = &[
    (C, &[return_(EFFICIENCY_TOPS), if_(DID, 1), REPEAT]),
    (N, &[return_(PROSPERITY_TOPS), if_(DID, 1), REPEAT]),
];

// Junk all cards in all score piles. If you don't, for each player, choose
// their highest top card, then for each other different color, transfer
// their top card of that color to their score pile.
const SANSKRIT: &[(EffectKind, Program)] = &[(
    N,
    &[
        junk(Selector::Score(Either))
            .all()
            .shortcut(Shortcut::AnyOrder),
        if_(&Not(DID_ANY), 6),
        if_(&holds(&TOPS), 2),
        choose(Executor, Choice::Card(&filtered(&TOPS, &[Highest]))),
        transfer(
            filtered(&TOPS, &[Lacks(ColorOf(Reg::Chosen))]),
            Destination::Score(Executor),
        )
        .all()
        .frozen()
        .shortcut(Shortcut::AnyOrder),
        if_(&holds(&Tops(Opponent)), 2),
        choose(
            Executor,
            Choice::Card(&filtered(&Tops(Opponent), &[Highest])),
        ),
        transfer(
            filtered(&Tops(Opponent), &[Lacks(ColorOf(Reg::Chosen))]),
            Destination::Score(Opponent),
        )
        .all()
        .frozen()
        .shortcut(Shortcut::AnyOrder),
    ],
)];

const SPLAY_ASLANT_ALL: Instruction = Instruction::Splay(SplayStep {
    who: Executor,
    choices: SplayChoices::Fixed {
        colors: ALL_COLORS,
        direction: Splay::Aslant,
        from: None,
    },
    targets: SplayTargets::Changes,
    optional: false,
    all: true,
});
// Junk all cards on your board other than the top five of each color. /
// Splay each color on your board aslant.
const GREAT_BARRIER_REEF: &[(EffectKind, Program)] = &[
    (
        N,
        &[junk(Board(Executor, ALL_COLORS, BoardRegion::BelowTop(5)))
            .all()
            .shortcut(Shortcut::AnyOrder)],
    ),
    (N, &[SPLAY_ASLANT_ALL]),
];

// Start a new game, ignoring all players' current cards and without shuffling, each
// player drawing and melding a 9 to begin, the winner then transferring their cards to
// this game and junking each player's other cards from that game.
const MARTIAN_INTERNET: &[(EffectKind, Program)] = &[(N, &[Instruction::Subgame])];

// Choose a color on your board. Junk all cards of that color from all
// boards. / Choose a valued card in the junk. Score all cards of the chosen
// card's value in the junk. If you do, and you score fewer than eleven
// points, repeat this effect.
const JUMBO_KINGDOM: &[(EffectKind, Program)] = &[
    (
        N,
        &[
            choose(Executor, Choice::Color(ColorSet::OfCards(&TOPS))),
            junk(Board(Either, ColorSet::Chosen, BoardRegion::BelowTop(0)))
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    ),
    (
        N,
        &[
            choose(Executor, Choice::Card(&Selector::Junk)),
            score(filtered(&Selector::Junk, &[same_value(Value(Reg::Chosen))]))
                .all()
                .shortcut(Shortcut::AnyOrder)
                .keep(SCORED),
            if_(
                &All(&[
                    Did(Outcome::Any),
                    Compare(Expr::Total(&Selector::Slot(SCORED)), Cmp::Less, Const(11)),
                ]),
                1,
            ),
            REPEAT,
        ],
    ),
];

// I COMPEL you to transfer all cards on your board to your hand!
const OCULUS_QUEST: &[(EffectKind, Program)] = &[(
    C,
    &[transfer(WHOLE_BOARD, Destination::Hand(Executor))
        .all()
        .shortcut(Shortcut::AnyOrder)],
)];

pub(super) fn effects(code: &str) -> Option<&'static [(EffectKind, Program)]> {
    Some(match code {
        "A001" => HOLMEGAARD_BOWS,
        "A002" => PAPYRUS_OF_ANI,
        "A003" => PAVLOVIAN_TUSK,
        "A004" => DANCING_GIRL,
        "A005" => LURGAN_CANOE,
        "A006" => XIANRENDONG_SHARDS,
        "A007" => PRIEST_KING,
        "A008" => ELECTRUM_STATER_OF_EPHESUS,
        "A009" => JISKAIRUMOKO_NECKLACE,
        "A010" => TREATY_OF_KADESH,
        "A011" => SIBIDU_NEEDLE,
        "A012" => BASUR_HOYUK_TOKENS,
        "A013" => MASK_OF_WARKA,
        "A014" => ARK_OF_THE_COVENANT,
        "A015" => TALE_OF_THE_SHIPWRECKED_SAILOR,
        "A016" => CHRONICLE_OF_ZUO,
        "A017" => BABYLONIAN_CHRONICLES,
        "A018" => HOLY_GRAIL,
        "A019" => TERRACOTTA_ARMY,
        "A020" => HOLY_LANCE,
        "A021" => BAGHDAD_BATTERY,
        "A022" => SEIKILOS_EPITAPH,
        "A023" => ROSETTA_STONE,
        "A024" => DEAD_SEA_SCROLLS,
        "A025" => CYRUS_CYLINDER,
        "A026" => EXCALIBUR,
        "A027" => MJOLNIR_AMULET,
        "A028" => MOYLOUGH_BELT_SHRINE,
        "A029" => ALONG_THE_RIVER,
        "A030" => PHILOSOPHERS_STONE,
        "A031" => IFE_HEAD,
        "A032" => DUNHUANG_STAR_CHART,
        "A033" => YATA_NO_KAGAMI,
        "A034" => NECRONOMICON,
        "A035" => SHROUD_OF_TURIN,
        "A036" => EAST_INDIA_COMPANY_CHARTER,
        "A037" => GALLEON_ATOCHA,
        "A038" => MOSES,
        "A039" => MONA_LISA,
        "A040" => MOLASSES_REEF_CARAVEL,
        "A041" => HUNT_LENOX_GLOBE,
        "A042" => PETITION_OF_RIGHT,
        "A043" => DELFT_POCKET_TELESCOPE,
        "A044" => CROSS_OF_CORONADO,
        "A045" => ABELL_GALLERY_HARPSICHORD,
        "A046" => GALLEY_WHYDAH,
        "A047" => SHIP_OF_THE_LINE_SUSSEX,
        "A048" => GUJIN_TUSHU_JICHENG,
        "A049" => THE_DAILY_COURANT,
        "A050" => BARQUE_LONGUE_LA_BELLE,
        "A051" => HUDSONS_BAY_COMPANY_ARCHIVES,
        "A052" => BOERHAAVE_MICROSCOPE,
        "A053" => PRINCIPIA,
        "A054" => SANDHAM_ROOM_CRICKET_BAT,
        "A055" => ALMIRA_QUEEN_OF_CASTILE,
        "A056" => FRIGATE_CONSTITUTION,
        "A057" => US_DECLARATION_OF_INDEPENDENCE,
        "A058" => QIANLONGS_DRAGON_ROBE,
        "A059" => PRIDE_AND_PREJUDICE,
        "A060" => THE_WEALTH_OF_NATIONS,
        "A061" => BUTTONWOOD_AGREEMENT,
        "A062" => KILOGRAM_OF_THE_ARCHIVES,
        "A063" => PUFFING_BILLY,
        "A064" => MOONLIGHT_SONATA,
        "A065" => MARCHA_REAL,
        "A066" => SUBMARINE_HUNLEY,
        "A067" => JEDLIKS_SELF_ROTOR,
        "A068" => COLT_PATERSON_REVOLVER,
        "A069" => SINGER_MODEL_27,
        "A070" => METRE_BAR,
        "A071" => HANSEN_WRITING_BALL,
        "A072" => PERIODIC_TABLE,
        "A073" => CORVETTE_CHALLENGER,
        "A074" => ROUNDHAY_GARDEN_SCENE,
        "A075" => THE_COMMUNIST_MANIFESTO,
        "A076" => BATTLESHIP_BISMARCK,
        "A078" => PLUSCH_ROD_BEAR,
        "A079" => ACTION_COMICS,
        "A080" => LINER_TITANIC,
        "A081" => MEIJI_MURA,
        "A082" => PARNELL_PITCH_DROP,
        "A083" => EARHARTS_ELECTRA,
        "A084" => GARLANDS_RUBY_SLIPPERS,
        "A085" => WORLD_CUP_FINAL_BALL,
        "A086" => CRUSADER_RABBIT,
        "A087" => VELCRO_SHOES,
        "A088" => ROCK_AROUND_THE_CLOCK,
        "A089" => MAGNAVOX_ODYSSEY,
        "A090" => PHILIPS_COMPACT_CASSETTE,
        "A091" => SYNCOM_3,
        "A092" => YEAGERS_BELL_X1A,
        "A093" => LUNA_3,
        "A094" => THE_BIG_BANG,
        "A095" => MARILYN_DIPTYCH,
        "A096" => TANKER_EXXON_VALDEZ,
        "A097" => MALDIVES,
        "A098" => DOLLY_THE_SHEEP,
        "A099" => WHERES_WALDO,
        "A100" => MUSKS_TESLA_ROADSTER,
        "A101" => SEIKAN_TUNNEL,
        "A102" => ROVER_CURIOSITY,
        "A103" => HIGGS_BOSON,
        "A104" => DELOREAN_DMC_12,
        "A105" => DRAGONS_LAIR,
        "A106" => SIXTEEN_PSYCHE,
        "A107" => PIZZA_PACAYA,
        "A108" => WHAT_DOES_THE_FOX_SAY,
        "A109" => TASMANIAN_TIGER,
        "A110" => GREENLAND,
        "A111" => SANSKRIT,
        "A112" => GREAT_BARRIER_REEF,
        "A113" => MARTIAN_INTERNET,
        "A114" => JUMBO_KINGDOM,
        "A115" => OCULUS_QUEST,
        // Battleship Yamato prints no effects.
        "A077" => &[],
        _ => return None,
    })
}
