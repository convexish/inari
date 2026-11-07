//! Figure declarations, in code order. Each comment is the printed karma.
//! A Would clause's program starts with the event's card bound and, for a
//! Dogma, the featured icon (or for a splay, the color) chosen; in it,
//! `Demander` names the player whose operation triggered it.
use super::{CardMetadata, FIGURE_METADATA};
use crate::Decree::{self, Advancement, Expansion, Rivalry, Trade, War};
use crate::model::{Card, Error, Figure};
use crate::program::build::*;

use Color::{Green, Red, Yellow};
use Cond::{All, Any, Card as CardIs, Compare, Not};
use Expr::{Const, Count, Distinct, Max, Metric as M, Value};
use Filter::{Has, Lacks, Lowest};
use Icon::{Authority, Concept, Efficiency, Health, Industry, Prosperity};
use Selector::{AvailableAchievements, BaseDeck, Board, StandardAchievements, TopOf, Tops};
use Test::{Code, Figure as IsFigure, Icon as HasIcon, InSlot, Is, Value as ValueIs, Values};
use Verb::{Junk, Meld, Return, Score, Transfer, Tuck};
use Who::{Either, Executor, Opponent};

/// The event's card, wherever it is.
const THE: Selector = Selector::Reg(Reg::Bound);
/// The previous operation's card, wherever it is.
const THE_LAST: Selector = Selector::Reg(Reg::Last);
const BIND_LAST: Instruction = bind(Selector::Reg(Reg::Last));
const SELF_EXECUTE: Instruction = execute(ExecutionMode::SelfExecute, Who::Executor);
const SUPER_EXECUTE: Instruction = execute(ExecutionMode::SuperExecute, Who::Executor);
const WIN: Instruction = Instruction::Win(WinRule::Executor);
const ALL_COLORS: ColorSet = ColorSet::Only(&Color::ALL);

const fn figure(code: &'static str, karma: &'static [Karma]) -> FigureDef {
    FigureDef {
        code,
        decrees: &[],
        each: &[],
        karma,
    }
}

/// "You may issue a <Decree> Decree with any two figures."
const fn grant(decree: Decree) -> &'static [(Option<Decree>, u8)] {
    match decree {
        War => &[(Some(War), 2)],
        Expansion => &[(Some(Expansion), 2)],
        Trade => &[(Some(Trade), 2)],
        Advancement => &[(Some(Advancement), 2)],
        Rivalry => &[(Some(Rivalry), 2)],
    }
}

impl FigureDef {
    const fn decree(mut self, decree: Decree) -> Self {
        self.decrees = grant(decree);
        self
    }
    const fn each(mut self, each: &'static [Each]) -> Self {
        self.each = each;
        self
    }
}

/// The selection is not empty.
const fn holds(selector: &'static Selector) -> Cond {
    Compare(Count(selector), Cmp::AtLeast, Const(1))
}

/// A draw has a card to take (an exhausted draw has none).
const DRAWS: Cond = CardIs(Reg::Bound, Has(ValueIs(Cmp::AtLeast, Const(0))));

/// A mandatory splay of one color: any nonempty pile, since a no-op choice is allowed.
const fn splay_color(who: Who, colors: ColorSet, direction: Splay) -> Instruction {
    Instruction::Splay(SplayStep {
        who,
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

/// Splay every eligible color of those players.
const fn splay_all(who: Who, colors: ColorSet, direction: Splay) -> Instruction {
    Instruction::Splay(SplayStep {
        who,
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

const fn deck_junk(age: i64) -> Instruction {
    junk(BaseDeck(Const(age))).all().shortcut(Shortcut::DeckTop)
}

// Each [Authority] on your board provides one additional [Authority] and one
// additional [Industry]. / Each [Authority] on an opponent's board subtracts
// one point from that opponent's score.
const GILGAMESH: FigureDef = figure("F002", &[]).each(&[Each::Icons, Each::Score]);

// You may issue a War Decree with any two figures. / Each card in your hand
// provides one additional point towards your score.
const TIGERNMAS: FigureDef = figure("F003", &[]).decree(War).each(&[Each::Score]);

// If you would draw a [1] during your first action, first draw and score a
// card of value equal to the number of [1] in your hand.
const SHENNONG: FigureDef = figure(
    "F005",
    &[first(
        On::Draw,
        Executor,
        &[
            draw_age(Count(&filtered(&HAND, &[Has(Values(&[1]))]))),
            score(LAST),
        ],
    )
    .when(&All(&[
        Cond::Action(1),
        CardIs(Reg::Bound, Has(Values(&[1]))),
    ]))],
);

// You may issue an Expansion Decree with any two figures. / If you would meld
// a card, first draw and tuck a [2]. If the tucked card has no [Authority],
// return it.
const SNEFERU: FigureDef = figure(
    "F006",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            draw(2),
            tuck(LAST),
            if_(&CardIs(Reg::Last, Lacks(HasIcon(Authority))), 1),
            return_(THE_LAST),
        ],
    )],
)
.decree(Expansion);

// You may issue a Trade Decree with any two figures. / If you would draw a
// card during your first action, first draw and score a [2].
const FU_XI: FigureDef = figure(
    "F008",
    &[first(On::Draw, Executor, &[draw(2), score(LAST)]).when(&Cond::Action(1))],
)
.decree(Trade);

// Each unsplayed color on your board adds one to the value of your highest
// top card for the purpose of claiming achievements.
const DAEDALUS: FigureDef = figure("F010", &[]).each(&[Each::Claims]);

// You may issue an Advancement Decree with any two figures. / If you would
// dogma a card using [Health] as a featured icon, first draw a [2].
const HUANG_DI: FigureDef = figure(
    "F012",
    &[first(On::Dogma(Some(Health)), Executor, &[draw(2)])],
)
.decree(Advancement);

// If you would dogma a card as your second action, instead score a card from
// your hand. If you do, super-execute the scored card.
const PTAHHOTEP: FigureDef = figure(
    "F013",
    &[instead(
        On::Dogma(None),
        Executor,
        &[score(HAND), if_(DID, 2), BIND_LAST, SUPER_EXECUTE],
    )
    .when(&Cond::Action(2))],
);

// You may issue a Rivalry Decree with any two figures. / Each [Authority] on
// your board provides one additional point towards your score.
const SINUHE: FigureDef = figure("F014", &[]).decree(Rivalry).each(&[Each::Score]);

// If you would dogma a card of a color, first draw and tuck a [2], then
// return your top card of the drawn card's color if it is different. / If
// you would junk or return a figure from your hand, instead tuck it.
const HOMER: FigureDef = figure(
    "F015",
    &[
        first(
            On::Dogma(None),
            Executor,
            &[
                draw(2),
                BIND_LAST,
                tuck(BOUND),
                return_(filtered(
                    &Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top),
                    &[Lacks(Is(Reg::Bound))],
                )),
            ],
        ),
        instead(On::Move(&[Junk, Return]), Executor, &[tuck(THE)]).when(&All(&[
            CardIs(Reg::Bound, Has(IsFigure)),
            holds(&filtered(&HAND, &[Has(Is(Reg::Bound))])),
        ])),
    ],
);

// When you meld this card, score all opponents' top figures. / If you would
// take a Dogma action, first draw and score a [2]. If it isn't a [2], score
// Alexander the Great.
const ALEXANDER_THE_GREAT: FigureDef = figure(
    "F017",
    &[
        melded(&[score(filtered(&Tops(Opponent), &[Has(IsFigure)]))
            .all()
            .shortcut(ORIGIN_ALL)]),
        first(
            On::Dogma(None),
            Executor,
            &[
                draw(2),
                BIND_LAST,
                score(BOUND),
                if_(&CardIs(Reg::Bound, Lacks(Values(&[2]))), 1),
                score(Selector::Reg(Reg::Source)),
            ],
        ),
    ],
);

// If a player would score a card, first splay all colors on your board left.
// / Each top card on your board of a color not splayed counts as an
// available achievement for you.
const CAI_LUN: FigureDef = figure(
    "F019",
    &[first(
        On::Move(&[Score]),
        Either,
        &[splay_all(Executor, ALL_COLORS, Splay::Left)],
    )],
)
.each(&[Each::AvailableAchievements]);

// You may issue a Trade Decree with any two figures. / Each top blue card on
// every player's board counts as a card you can activate with a Dogma action.
const PTOLEMY: FigureDef = figure("F020", &[])
    .decree(Trade)
    .each(&[Each::DogmaSources]);

// You may issue an Advancement Decree with any two figures. / If you would
// execute an effect, first increase every [ ] value in the effect by one for
// the duration of the effect.
const ARCHIMEDES: FigureDef = figure(
    "F022",
    &[first(On::Effect, Executor, &[]).native(KarmaNative::Archimedes)],
)
.decree(Advancement);

// You may issue a Rivalry Decree with any two figures. / If you would dogma a
// card as your first action, first splay the card's color left. If you
// don't, junk an available achievement of value 2.
const PLATO: FigureDef = figure(
    "F025",
    &[first(
        On::Dogma(None),
        Executor,
        &[
            splay_color(Executor, ColorSet::Of(Reg::Bound), Splay::Left),
            if_(&Not(DID), 1),
            junk(filtered(&AvailableAchievements, &[Has(Values(&[2]))])),
        ],
    )
    .when(&Cond::Action(1))],
)
.decree(Rivalry);

// If you would draw a [3], first score a top red card of value less than 4
// from anywhere. / Each two [Authority] on your board provides one additional
// icon of every other standard icon type on your board.
const TRAN_HUANG_DAO: FigureDef = figure(
    "F026",
    &[first(
        On::Draw,
        Executor,
        &[score(filtered(
            &TopOf(Either, Red),
            &[Has(ValueIs(Cmp::Less, Const(4)))],
        ))],
    )
    .when(&CardIs(Reg::Bound, Has(Values(&[3]))))],
)
.each(&[Each::Icons]);

// You may issue an Expansion Decree with any two figures. / If you would
// dogma a card as your first action, instead junk the lowest available
// standard achievement, then super-execute the junked card.
const AVICENNA: FigureDef = figure(
    "F028",
    &[instead(
        On::Dogma(None),
        Executor,
        &[
            junk(filtered(&StandardAchievements, &[Lowest])),
            BIND_LAST,
            SUPER_EXECUTE,
        ],
    )
    .when(&Cond::Action(1))],
)
.decree(Expansion);

// If you would score a non-figure of a color or splay a color of your cards
// left, instead splay that color right. / Each visible card on your board
// provides an additional point towards your score.
const SHEN_KUO: FigureDef = figure(
    "F030",
    &[
        instead(
            On::Move(&[Score]),
            Executor,
            &[splay_color(
                Executor,
                ColorSet::Of(Reg::Bound),
                Splay::Right,
            )],
        )
        .when(&CardIs(Reg::Bound, Lacks(IsFigure))),
        instead(
            On::Splay(Some(Splay::Left)),
            Executor,
            &[splay_color(Executor, ColorSet::Chosen, Splay::Right)],
        ),
    ],
)
.each(&[Each::Score]);

// You may issue a Trade Decree with any two figures. / If you would meld a
// card, first draw and meld a [3]. If the drawn card has no [Health], return
// it.
const SU_SONG: FigureDef = figure(
    "F031",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            draw(3),
            meld(LAST),
            if_(&CardIs(Reg::Last, Lacks(HasIcon(Health))), 1),
            return_(THE_LAST),
        ],
    )],
)
.decree(Trade);

// You may issue an Advancement Decree with any two figures. / If you would
// meld a card of value equal to your top red card, instead return it and
// draw and meld a card of value one higher.
const SEJONG_THE_GREAT: FigureDef = figure(
    "F033",
    &[instead(
        On::Move(&[Meld]),
        Executor,
        &[
            return_(THE),
            draw_age(Expr::Add(&Value(Reg::Bound), 1)),
            meld(LAST),
        ],
    )
    .when(&Compare(
        Value(Reg::Bound),
        Cmp::Equal,
        Max(&TopOf(Executor, Red)),
    ))],
)
.decree(Advancement);

// If any player would transfer a card or exchange any number of cards,
// instead tuck that card or those cards, draw and tuck a [4], and score a
// top card with [Authority] from anywhere.
const YI_SUN_SIN: FigureDef = figure(
    "F037",
    &[
        instead(On::Move(&[Transfer]), Either, &[]).native(KarmaNative::YiSunSin),
        instead(On::Exchange, Either, &[]).native(KarmaNative::YiSunSin),
    ],
);

// Each card in your hand is also considered part of your score pile.
const MICHELANGELO: FigureDef = figure("F039", &[]).each(&[Each::ScoreMembership, Each::Score]);

// If you would draw a card, first junk all cards in either the [4] or [5]
// deck. / If you would meld a card, first junk an available achievement of
// value 3, 4, or 5.
const GALILEO_GALILEI: FigureDef = figure(
    "F040",
    &[
        first(
            On::Draw,
            Executor,
            &[
                unless_decks_empty!(4, 5),
                choose_deck(&[4, 5]),
                CHOSEN_DECK_JUNK,
            ],
        ),
        first(
            On::Move(&[Meld]),
            Executor,
            &[junk(filtered(
                &AvailableAchievements,
                &[Has(Values(&[3, 4, 5]))],
            ))],
        ),
    ],
);

// You may issue a Trade Decree with any two figures. / If you would dogma a
// [1], instead claim the World achievement. If you don't, and Monotheism is a
// top card on your board, you win.
const PEDRO_ALVARES_CABRAL: FigureDef = figure(
    "F041",
    &[instead(
        On::Dogma(None),
        Executor,
        &[
            claim(Special::World),
            if_(
                &All(&[Not(DID), holds(&filtered(&TOPS, &[Has(Code("B024"))]))]),
                1,
            ),
            WIN,
        ],
    )
    .when(&CardIs(Reg::Bound, Has(Values(&[1]))))],
)
.decree(Trade);

// If you would take a Dogma action, instead do so while increasing each [ ]
// value in every effect during this action by the number of different top
// card values on your board greater than 3.
const JOHANNES_KEPLER: FigureDef = figure(
    "F043",
    &[instead(On::Dogma(None), Executor, &[]).native(KarmaNative::Kepler)],
);

// Each color splayed right on only your board counts as an achievement for
// you. / If you would splay a color on your board, first unsplay that color
// on all opponents' boards.
const NICCOLO_MACHIAVELLI: FigureDef = figure(
    "F044",
    &[first(
        On::Splay(None),
        Executor,
        &[splay_color(Opponent, ColorSet::Chosen, Splay::None)],
    )],
)
.each(&[Each::Achievements]);

// You may issue a Rivalry Decree with any one figure. / You may issue any
// Decree with any three figures.
const WILLIAM_SHAKESPEARE: FigureDef = FigureDef {
    decrees: &[(Some(Rivalry), 1), (None, 3)],
    ..figure("F045", &[])
};

// Each card in your hand counts as ten points towards the cost of claiming an
// achievement of that card's value. / If you would draw a card, first you may
// return a [5] from your hand. If you do, draw a [6].
const ANTONIE_VAN_LEEUWENHOEK: FigureDef = figure(
    "F048",
    &[first(
        On::Draw,
        Executor,
        &[
            return_(filtered(&HAND, &[Has(Values(&[5]))])).optional(),
            if_(DID, 1),
            draw(6),
        ],
    )],
)
.each(&[Each::Claims]);

// You may issue an Expansion Decree with any two figures. / Each [Industry]
// on your board provides one additional point towards your score for every
// achievement you have.
const CHRISTOPHER_POLHEM: FigureDef = figure("F049", &[]).decree(Expansion).each(&[Each::Score]);

// You may issue an Expansion Decree with any two figures. / Each top card
// with [Industry] on your board counts as an available achievement for you.
const JOHN_LOUDON_MCADAM: FigureDef = figure("F058", &[])
    .decree(Expansion)
    .each(&[Each::AvailableAchievements]);

// Each [Concept] on your board provides one additional [Concept] and one
// additional [Efficiency]. / If a player would meld a card, first that player
// transfers their top card of the same color into their hand.
const CATHERINE_THE_GREAT: FigureDef = figure(
    "F064",
    &[first(
        On::Move(&[Meld]),
        Either,
        &[transfer(
            Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top),
            Destination::Hand(Executor),
        )],
    )
    .by_actor()],
)
.each(&[Each::Icons]);

// If you would dogma a card of a color an opponent has splayed right, first
// unsplay that color on an opponent's board. / If an opponent would draw a
// card, first draw and tuck a [7].
const JOHN_ERICSSON: FigureDef = figure(
    "F067",
    &[
        first(
            On::Dogma(None),
            Executor,
            &[splay_color(Opponent, ColorSet::Of(Reg::Bound), Splay::None)],
        )
        .when(&Cond::Splayed {
            who: Opponent,
            colors: ColorSet::Of(Reg::Bound),
            direction: Splay::Right,
            present: true,
        }),
        first(On::Draw, Opponent, &[draw(7), tuck(LAST)]),
    ],
);

// You may issue an Expansion Decree with any two figures. / If a player would
// transfer, return, score, meld, or junk a card from your score pile, instead
// leave it there.
const FLORENCE_NIGHTINGALE: FigureDef = figure(
    "F069",
    &[instead(
        On::Move(&[Transfer, Return, Score, Meld, Junk]),
        Either,
        &[],
    )
    .when(&holds(&filtered(&SCORE, &[Has(Is(Reg::Bound))])))],
)
.decree(Expansion);

// Each standard icon type on your board counts as an achievement, if you have
// at least twice as many of that icon as every opponent. / If a player would
// transfer a card, first junk all cards in the [7] deck.
const ALFRED_NOBEL: FigureDef = figure(
    "F071",
    &[first(On::Move(&[Transfer]), Either, &[deck_junk(7)])],
)
.each(&[Each::Achievements]);

// You may issue an Advancement Decree with any two figures. / If you would
// claim an achievement, first if no opponent has more achievements than you,
// you win. Otherwise, draw an [8].
const CHARLES_DARWIN: FigureDef = figure(
    "F072",
    &[first(
        On::Claim,
        Executor,
        &[
            if_(
                &Compare(
                    M(Executor, Metric::Achievements),
                    Cmp::AtLeast,
                    M(Opponent, Metric::Achievements),
                ),
                2,
            ),
            WIN,
            Instruction::Jump(1),
            draw(8),
        ],
    )],
)
.decree(Advancement);

// Each card in your hand provides one additional standard icon of every type
// on your board. / If a player would return a card of value lower than 8,
// first draw an [8].
const JAMES_CLERK_MAXWELL: FigureDef = figure(
    "F073",
    &[first(On::Move(&[Return]), Either, &[draw(8)])
        .when(&CardIs(Reg::Bound, Has(ValueIs(Cmp::Less, Const(8)))))],
)
.each(&[Each::Icons]);

// If you would meld a [10] and you have top cards of five unique values on
// your board, instead you win. / Until your second action on your turn, each
// card in an opponent's hand counts as being in your hand.
const EMPEROR_MEIJI: FigureDef = figure(
    "F074",
    &[instead(On::Move(&[Meld]), Executor, &[WIN]).when(&All(&[
        CardIs(Reg::Bound, Has(Values(&[10]))),
        Compare(Distinct(&TOPS), Cmp::Equal, Const(5)),
    ]))],
)
.each(&[Each::HandMembership]);

// If you would draw an [8], first junk all cards in the [8] deck. / If you
// would meld a card, instead meld it and self-execute it.
const JOHN_VON_NEUMANN: FigureDef = figure(
    "F077",
    &[
        first(On::Draw, Executor, &[deck_junk(8)]).when(&CardIs(Reg::Bound, Has(Values(&[8])))),
        instead(On::Move(&[Meld]), Executor, &[meld(THE), SELF_EXECUTE]),
    ],
);

// Each different value present in your score pile above 6 counts as an
// achievement. / If you would draw a card of a value not present in your
// hand, first draw a [9].
const MARIE_CURIE: FigureDef = figure(
    "F082",
    &[first(On::Draw, Executor, &[draw(9)]).when(&All(&[
        DRAWS,
        Compare(
            Count(&filtered(
                &HAND,
                &[Has(ValueIs(Cmp::Equal, Value(Reg::Bound)))],
            )),
            Cmp::Equal,
            Const(0),
        ),
    ]))],
)
.each(&[Each::Achievements]);

// If you would dogma a card using [Concept] as a featured icon, instead draw
// and junk a [10], then super-execute the junked card.
const H_G_WELLS: FigureDef = figure(
    "F084",
    &[instead(
        On::Dogma(Some(Concept)),
        Executor,
        &[draw(10), BIND_LAST, junk(LAST), SUPER_EXECUTE],
    )],
);

// If you are required to fade a figure, instead do nothing. / If you would
// meld a figure and have at least four top figures already, instead you win.
const DUKE_ELLINGTON: FigureDef = figure(
    "F085",
    &[
        instead(On::Fade, Executor, &[]),
        instead(On::Move(&[Meld]), Executor, &[WIN]).when(&All(&[
            CardIs(Reg::Bound, Has(IsFigure)),
            Compare(
                Count(&filtered(&TOPS, &[Has(IsFigure)])),
                Cmp::AtLeast,
                Const(4),
            ),
        ])),
    ],
);

// If you would dogma a card as your second action, instead super-execute the
// card. / Each two [Avatar] on your board counts as an achievement.
const NELSON_MANDELA: FigureDef = figure(
    "F086",
    &[instead(On::Dogma(None), Executor, &[SUPER_EXECUTE]).when(&Cond::Action(2))],
)
.each(&[Each::Achievements]);

// If an opponent would meld a card, first choose a top red card on an
// opponent's board. Choose to either score it, or self-execute it.
const MIKHAIL_KALASHNIKOV: FigureDef = figure(
    "F087",
    &[first(On::Move(&[Meld]), Opponent, &[]).native(KarmaNative::Kalashnikov)],
);

// Each special achievement is claimed with one fewer or one lower of each
// requirement listed for you. / If you would claim an achievement, first
// splay a color of your cards up.
const HEDY_LAMARR: FigureDef = figure(
    "F090",
    &[first(
        On::Claim,
        Executor,
        &[splay_color(Executor, ALL_COLORS, Splay::Up)],
    )],
)
.each(&[Each::SpecialRequirements]);

// You may issue a Rivalry Decree with any two figures. / If you would score a
// card, first if both your top yellow card and your top green card are
// highest top cards on your board, you win.
const PELE: FigureDef = figure(
    "F094",
    &[first(
        On::Move(&[Score]),
        Executor,
        &[
            if_(
                &All(&[
                    holds(&TopOf(Executor, Yellow)),
                    holds(&TopOf(Executor, Green)),
                    Compare(Max(&TopOf(Executor, Yellow)), Cmp::Equal, Max(&TOPS)),
                    Compare(Max(&TopOf(Executor, Green)), Cmp::Equal, Max(&TOPS)),
                ]),
                1,
            ),
            WIN,
        ],
    )],
)
.decree(Rivalry);

// If an opponent would win, first score all other top figures anywhere. If
// you now have the most points, you win instead. / If you would draw a card,
// first score a figure from your hand.
const JACKIE_CHAN: FigureDef = figure(
    "F096",
    &[
        first(
            On::Win,
            Opponent,
            &[
                score(filtered(
                    &Tops(Either),
                    &[Has(IsFigure), Lacks(Is(Reg::Source))],
                ))
                .all(),
                if_(
                    &Compare(
                        M(Executor, Metric::Score),
                        Cmp::AtLeast,
                        M(Opponent, Metric::Score),
                    ),
                    1,
                ),
                WIN,
            ],
        ),
        first(
            On::Draw,
            Executor,
            &[score(filtered(&HAND, &[Has(IsFigure)]))],
        ),
    ],
);

// If you would draw a card and have no figures in hand, first say "Who is"
// and name a figure. Search the figures deck of any age for the named figure
// and transfer it into your hand if present. Then shuffle that deck.
const ALEX_TREBEK: FigureDef = figure(
    "F098",
    &[first(On::Draw, Executor, &[])
        .when(&Compare(
            Count(&filtered(&HAND, &[Has(IsFigure)])),
            Cmp::Equal,
            Const(0),
        ))
        .native(KarmaNative::AlexTrebek)],
);

// If you would dogma a card using [Efficiency] as the featured icon, first if
// you have exactly one, three, or six [Efficiency] on your board, you win.
const SHIGERU_MIYAMOTO: FigureDef = figure(
    "F099",
    &[first(
        On::Dogma(Some(Efficiency)),
        Executor,
        &[
            if_(
                &Any(&[
                    Compare(M(Executor, Metric::Icons(Efficiency)), Cmp::Equal, Const(1)),
                    Compare(M(Executor, Metric::Icons(Efficiency)), Cmp::Equal, Const(3)),
                    Compare(M(Executor, Metric::Icons(Efficiency)), Cmp::Equal, Const(6)),
                ]),
                1,
            ),
            WIN,
        ],
    )],
);

// Each top card on every player's board counts as a card you can activate
// with a Dogma action. / If you would dogma a card on another player's board,
// first splay its color up on your board.
const SERGEY_BRIN: FigureDef = figure(
    "F101",
    &[first(
        On::Dogma(None),
        Executor,
        &[splay_color(Executor, ColorSet::Of(Reg::Bound), Splay::Up)],
    )
    .when(&holds(&filtered(&Tops(Opponent), &[Has(Is(Reg::Bound))])))],
)
.each(&[Each::DogmaSources]);

// When you meld this card, junk all cards that are not achievements or in
// decks. Introduce the [0] deck. Draw a [0].
const BEAR_GRYLLS: FigureDef = figure("F110", &[melded(&[]).native(KarmaNative::BearGrylls)]);

// When you meld this card, unsplay all colors on all boards. / If a player
// would meld a card, first return their top six cards of its color. If you
// don't, that player scores all cards of its color from all boards.
const EMMA_WATSON: FigureDef = figure(
    "F114",
    &[
        melded(&[splay_all(Either, ALL_COLORS, Splay::None)]),
        first(On::Move(&[Meld]), Either, &[]).native(KarmaNative::EmmaWatson),
    ],
);

const ORIGIN: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: true,
};
const ORIGIN_ALL: Shortcut = Shortcut::ByOrigin {
    descending: false,
    quota: false,
};
const OPPONENT_HAND: Selector = Selector::Hand(Opponent);
const LOSES: Instruction = Instruction::Win(WinRule::Loses(Who::Demander));
/// The claimed card is a standard (supply) achievement.
const STANDARD: Cond = holds(&filtered(&StandardAchievements, &[Has(Is(Reg::Bound))]));

const fn same_value(reg: Reg) -> Filter {
    Has(ValueIs(Cmp::Equal, Value(reg)))
}

/// "Junk all cards in the [a] or [b] deck", after `unless_decks_empty!`.
const fn choose_deck(ages: &'static [u8]) -> Instruction {
    choose(Executor, Choice::Value(ValueChoices::Boxed(ages)))
}
const CHOSEN_DECK_JUNK: Instruction = junk(BaseDeck(Expr::Chosen))
    .all()
    .shortcut(Shortcut::DeckTop);
/// The previous instruction did everything it attempted.
const DONE: Cond = Cond::Did(Outcome::All);

// If a player would execute a demand effect against you, first execute that
// demand effect yourself on that player.
const HAMMURABI: FigureDef = figure(
    "F001",
    &[first(On::DemandAgainst, Either, &[]).native(KarmaNative::Hammurabi)],
);

// If you would meld a non-figure with a Meld action, instead self-execute it.
// / If you would tuck a sixth card or score a sixth card this turn, first draw
// and achieve a [1].
const SUNSHU_AO: FigureDef = figure(
    "F004",
    &[
        instead(On::MeldAction, Executor, &[SELF_EXECUTE])
            .when(&CardIs(Reg::Bound, Lacks(IsFigure))),
        first(On::Move(&[Tuck]), Executor, &[draw(1), achieve(LAST)]).when(&Compare(
            Expr::ThisTurn(Tuck),
            Cmp::Equal,
            Const(5),
        )),
        first(On::Move(&[Score]), Executor, &[draw(1), achieve(LAST)]).when(&Compare(
            Expr::ThisTurn(Score),
            Cmp::Equal,
            Const(5),
        )),
    ],
);

// If you would draw a card of value higher than 1 and you have a [1] in your
// hand, first return all cards from your hand and draw a card of that value
// for each card you return.
const HATSHEPSUT: FigureDef = figure(
    "F007",
    &[first(
        On::Draw,
        Executor,
        &[
            return_(HAND).all().shortcut(ORIGIN_ALL),
            Instruction::Draw(Draw {
                age: Value(Reg::Bound),
                set: CardSet::Base,
                any_set: false,
                count: Some(Expr::Done),
                then: Then::Keep,
                optional: false,
                keep: None,
            }),
        ],
    )
    .when(&All(&[
        CardIs(Reg::Bound, Has(ValueIs(Cmp::Greater, Const(1)))),
        holds(&filtered(&HAND, &[Has(Values(&[1]))])),
    ]))],
);

// If you would dogma a [1] as your second action, first draw and meld a [2],
// then splay left the color of the melded card on your board. If you do both,
// and the melded card was a [2], return the top card of that color from all
// boards.
const IMHOTEP: FigureDef = figure(
    "F011",
    &[first(
        On::Dogma(None),
        Executor,
        &[
            draw(2),
            BIND_LAST,
            meld(BOUND),
            if_(DID, 4),
            splay_color(Executor, ColorSet::Of(Reg::Bound), Splay::Left),
            if_(&All(&[DONE, CardIs(Reg::Bound, Has(Values(&[2])))]), 2),
            return_(Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top)),
            return_(Board(Opponent, ColorSet::Of(Reg::Bound), BoardRegion::Top)),
        ],
    )
    .when(&All(&[
        Cond::Action(2),
        CardIs(Reg::Bound, Has(Values(&[1]))),
    ]))],
);

// You may issue an Expansion Decree with any two figures. / If you would
// issue a Decree, first junk all cards from all opponents' hands.
const XIMEN_BAO: FigureDef = figure(
    "F018",
    &[first(
        On::IssueDecree,
        Executor,
        &[junk(OPPONENT_HAND).all().shortcut(Shortcut::AnyOrder)],
    )],
)
.decree(Expansion);

// If a player would dogma a card with [Authority], instead draw and reveal a
// card of the same value. If it has [Authority] or [Concept], meld it.
// Otherwise, the player super-executes the original card.
const AUGUSTUS_CAESAR: FigureDef = figure(
    "F021",
    &[instead(
        On::Dogma(None),
        Either,
        &[
            draw_age(Value(Reg::Bound)),
            reveal(LAST),
            if_(
                &Any(&[
                    CardIs(Reg::Last, Has(HasIcon(Authority))),
                    CardIs(Reg::Last, Has(HasIcon(Concept))),
                ]),
                2,
            ),
            meld(LAST),
            Instruction::Jump(1),
            execute(ExecutionMode::SuperExecute, Who::Demander),
        ],
    )
    .when(&CardIs(Reg::Bound, Has(HasIcon(Authority))))],
);

// If a player would claim an achievement, first draw and tuck a [3], then
// score all cards above the tucked card.
const ZHANG_HENG: FigureDef = figure(
    "F023",
    &[first(
        On::Claim,
        Either,
        &[
            draw(3),
            tuck(LAST),
            score(filtered(
                &Board(Executor, ColorSet::Of(Reg::Last), BoardRegion::BelowTop(0)),
                &[Lacks(Is(Reg::Last))],
            ))
            .all()
            .shortcut(ORIGIN_ALL),
        ],
    )],
);

// If you would dogma a card using [Authority] as a featured icon, instead
// score the card and junk all cards in the [2] deck or the [3] deck.
const CONFUCIUS: FigureDef = figure(
    "F024",
    &[instead(
        On::Dogma(Some(Authority)),
        Executor,
        &[
            score(THE),
            unless_decks_empty!(2, 3),
            choose_deck(&[2, 3]),
            CHOSEN_DECK_JUNK,
        ],
    )],
);

// You may issue a War Decree with any two figures. / If you would dogma a
// card with a demand effect, instead score a top card with [Authority] of
// each color on each opponent's board.
const SALADIN: FigureDef = figure(
    "F027",
    &[instead(
        On::Dogma(None),
        Executor,
        &[score(filtered(&Tops(Opponent), &[Has(HasIcon(Authority))]))
            .per_color(None)
            .shortcut(ORIGIN_ALL)],
    )
    .when(&CardIs(Reg::Bound, Has(Test::Demand)))],
)
.decree(War);

// If you would draw a card, first tuck a card of the same value from your
// hand or from an opponent's score pile.
const RHAZES: FigureDef = figure(
    "F029",
    &[first(
        On::Draw,
        Executor,
        &[tuck(filtered(
            &Selector::Union(&[HAND, Selector::Score(Opponent)]),
            &[same_value(Reg::Bound)],
        ))],
    )],
);

// If you would take a Draw action, instead tuck a top card with [Authority]
// from anywhere. Then draw a card of value equal to the number of
// [Authority] or [Concept] in a color on your board.
const ALHAZEN: FigureDef = figure(
    "F032",
    &[instead(
        On::DrawAction,
        Executor,
        &[
            tuck(filtered(&Tops(Either), &[Has(HasIcon(Authority))])),
            choose(Executor, Choice::Color(ColorSet::OfCards(&Tops(Executor)))),
            draw_age(Expr::Sum(&[
                Expr::Visible(&Visibility {
                    who: Who::Executor,
                    colors: ColorSet::Chosen,
                    seen: Seen::Icon(Authority),
                }),
                Expr::Visible(&Visibility {
                    who: Who::Executor,
                    colors: ColorSet::Chosen,
                    seen: Seen::Icon(Concept),
                }),
            ])),
        ],
    )],
);

// If you would dogma a card as your second action, first score a figure from
// an opponent's hand. / Each top card with [Authority] on your board counts
// as an available achievement for you.
const AMINA_SUKHERA: FigureDef = figure(
    "F036",
    &[first(
        On::Dogma(None),
        Executor,
        &[score(filtered(&OPPONENT_HAND, &[Has(IsFigure)]))],
    )
    .when(&Cond::Action(2))],
)
.each(&[Each::AvailableAchievements]);

// If you would meld a card, first meld another card in your hand. If the
// other card is purple, draw three [4], then score a top figure from
// anywhere.
const LEONARDO_DA_VINCI: FigureDef = figure(
    "F038",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            meld(filtered(&HAND, &[Lacks(Is(Reg::Bound))])),
            if_(&CardIs(Reg::Last, Has(Test::Color(Color::Purple))), 4),
            draw(4),
            draw(4),
            draw(4),
            score(filtered(&Tops(Either), &[Has(IsFigure)])),
        ],
    )],
);

// You may issue an Advancement Decree with any two figures. / If you would
// take a Draw action, instead draw and reveal a [5], and splay its color on
// your board right. If the drawn card has [Concept], meld it.
const TAKI_AD_DIN: FigureDef = figure(
    "F042",
    &[instead(
        On::DrawAction,
        Executor,
        &[
            draw(5),
            reveal(LAST),
            BIND_LAST,
            splay_color(Executor, ColorSet::Of(Reg::Bound), Splay::Right),
            if_(&CardIs(Reg::Bound, Has(HasIcon(Concept))), 1),
            meld(BOUND),
        ],
    )],
)
.decree(Advancement);

// If an opponent would claim an achievement, first you claim another
// available standard achievement if eligible. / If you would dogma a yellow
// card, first transfer all purple cards on your board to the available
// achievements.
const SHIVAJI: FigureDef = figure(
    "F046",
    &[
        first(
            On::Claim,
            Opponent,
            &[achieve(filtered(
                &StandardAchievements,
                &[
                    Lacks(Is(Reg::Bound)),
                    Has(Test::Eligible { score_factor: 1 }),
                ],
            ))],
        ),
        first(
            On::Dogma(None),
            Executor,
            &[transfer(
                Board(
                    Executor,
                    ColorSet::Only(&[Color::Purple]),
                    BoardRegion::BelowTop(0),
                ),
                Destination::Supply,
            )
            .all()
            .shortcut(Shortcut::AnyOrder)],
        )
        .when(&CardIs(Reg::Bound, Has(Test::Color(Yellow)))),
    ],
);

// You may issue a War Decree with any two figures. / If you would return a
// card, first score your bottom green card. If the scored card has
// [Prosperity], achieve your bottom green card, if eligible.
const PETER_THE_GREAT: FigureDef = figure(
    "F047",
    &[first(
        On::Move(&[Return]),
        Executor,
        &[
            score(Board(
                Executor,
                ColorSet::Only(&[Green]),
                BoardRegion::Bottom,
            )),
            if_(&CardIs(Reg::Last, Has(HasIcon(Prosperity))), 1),
            achieve(filtered(
                &Board(Executor, ColorSet::Only(&[Green]), BoardRegion::Bottom),
                &[Has(Test::Eligible { score_factor: 1 })],
            )),
        ],
    )],
)
.decree(War);

// If you would draw a fifth card into your hand, first claim an available
// achievement of that card's value or below, regardless of eligibility. / If
// you would draw a first card into your hand, first draw a [6].
const SAMUEL_DE_CHAMPLAIN: FigureDef = figure(
    "F050",
    &[
        first(
            On::Draw,
            Executor,
            &[achieve(filtered(
                &AvailableAchievements,
                &[Has(ValueIs(Cmp::AtMost, Value(Reg::Bound)))],
            ))],
        )
        .when(&All(&[DRAWS, Compare(Count(&HAND), Cmp::Equal, Const(4))])),
        first(On::Draw, Executor, &[draw(6)])
            .when(&All(&[DRAWS, Compare(Count(&HAND), Cmp::Equal, Const(0))])),
    ],
);

// If you would take a Draw action, instead score a top card with
// [Prosperity] from anywhere. / If you would draw a card, first claim an
// available achievement matching that card's value regardless of
// eligibility.
const BARTHOLOMEW_ROBERTS: FigureDef = figure(
    "F051",
    &[
        instead(
            On::DrawAction,
            Executor,
            &[score(filtered(&Tops(Either), &[Has(HasIcon(Prosperity))]))],
        ),
        first(
            On::Draw,
            Executor,
            &[achieve(filtered(
                &AvailableAchievements,
                &[same_value(Reg::Bound)],
            ))],
        )
        .when(&DRAWS),
    ],
);

// If you would take a Draw action, first draw a [1] and transfer it to any
// player's board. / If you would dogma a card, first splay right every color
// on your board with a top card of that card's value.
const ISAAC_NEWTON: FigureDef = figure(
    "F052",
    &[
        first(
            On::DrawAction,
            Executor,
            &[
                draw(1),
                choose(
                    Executor,
                    Choice::Branch(&[
                        (0, "Transfer the drawn card to your board"),
                        (2, "Transfer the drawn card to your opponent's board"),
                    ]),
                ),
                transfer(LAST, Destination::Board(Executor)),
                Instruction::Jump(1),
                transfer(LAST, Destination::Board(Opponent)),
            ],
        ),
        first(
            On::Dogma(None),
            Executor,
            &[splay_all(
                Executor,
                ColorSet::OfCards(&filtered(&TOPS, &[same_value(Reg::Bound)])),
                Splay::Right,
            )],
        ),
    ],
);

// If you would meld a fifth visible card of a color on your board, first
// claim an available achievement regardless of eligibility.
const BARTOLOMEO_CRISTOFORI: FigureDef = figure(
    "F054",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[achieve(AvailableAchievements)],
    )
    .when(&Compare(
        Expr::Visible(&Visibility {
            who: Who::Executor,
            colors: ColorSet::Of(Reg::Bound),
            seen: Seen::Cards,
        }),
        Cmp::Equal,
        Const(4),
    ))],
);

// If you would claim a standard achievement, first claim an available
// achievement of value one higher, regardless of eligibility. / If you would
// meld a card, first score all cards of a color on your board.
const JOHANNES_VERMEER: FigureDef = figure(
    "F055",
    &[
        first(
            On::Claim,
            Executor,
            &[achieve(filtered(
                &AvailableAchievements,
                &[Has(ValueIs(Cmp::Equal, Expr::Add(&Value(Reg::Bound), 1)))],
            ))],
        )
        .when(&STANDARD),
        first(
            On::Move(&[Meld]),
            Executor,
            &[
                choose(Executor, Choice::Color(ColorSet::OfCards(&Tops(Executor)))),
                score(Board(Executor, ColorSet::Chosen, BoardRegion::BelowTop(0)))
                    .all()
                    .shortcut(ORIGIN_ALL),
            ],
        ),
    ],
);

// You may issue a War Decree with any two figures. / If you would meld, tuck,
// score, or return a card, instead meld it, then if the melded card is red or
// blue, tuck it. Score a top card of value 5 or 6 from anywhere.
const NAPOLEON_BONAPARTE: FigureDef = figure(
    "F056",
    &[instead(
        On::Move(&[Meld, Tuck, Score, Return]),
        Executor,
        &[
            meld(THE),
            if_(
                &Any(&[
                    CardIs(Reg::Last, Has(Test::Color(Red))),
                    CardIs(Reg::Last, Has(Test::Color(Color::Blue))),
                ]),
                1,
            ),
            tuck(THE_LAST),
            score(filtered(&Tops(Either), &[Has(Values(&[5, 6]))])),
        ],
    )],
)
.decree(War);

/// No top card with [Industry] of the chosen value on any board and no
/// available achievement of it: Tecumseh's value changes nothing.
const TECUMSEH_IDLE: Cond = Cond::All(&[
    Cond::Compare(
        Expr::Count(&filtered(
            &Selector::Tops(Who::Either),
            &[
                Filter::Has(HasIcon(Industry)),
                Filter::Has(ValueIs(Cmp::Equal, Expr::Chosen)),
            ],
        )),
        Cmp::Equal,
        Expr::Const(0),
    ),
    Cond::Compare(
        Expr::Count(&filtered(
            &Selector::AvailableAchievements,
            &[Filter::Has(ValueIs(Cmp::Equal, Expr::Chosen))],
        )),
        Cmp::Equal,
        Expr::Const(0),
    ),
]);

// If you would meld a card, first choose a value. Return a top card with
// [Industry] of the chosen value from each player's board. Junk an available
// achievement of the chosen value.
const TECUMSEH: FigureDef = figure(
    "F057",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            choose(Executor, Choice::Value(ValueChoices::Any))
                .alias(Alias::NoOpWhen(&TECUMSEH_IDLE)),
            return_(filtered(
                &TOPS,
                &[
                    Has(HasIcon(Industry)),
                    Has(ValueIs(Cmp::Equal, Expr::Chosen)),
                ],
            )),
            return_(filtered(
                &Tops(Opponent),
                &[
                    Has(HasIcon(Industry)),
                    Has(ValueIs(Cmp::Equal, Expr::Chosen)),
                ],
            )),
            junk(filtered(
                &AvailableAchievements,
                &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
            )),
        ],
    )],
);

// If a player would execute a demand effect against you, instead return a
// card from your hand. / If an opponent would score a card, first junk an
// available achievement of the same value.
const EDWARD_JENNER: FigureDef = figure(
    "F059",
    &[
        instead(On::DemandAgainst, Either, &[return_(HAND)]),
        first(
            On::Move(&[Score]),
            Opponent,
            &[junk(filtered(
                &AvailableAchievements,
                &[same_value(Reg::Bound)],
            ))],
        ),
    ],
);

// Each [Prosperity] on your board provides two additional [Prosperity]. / If
// you would take a Dogma action with no player eligible to share, first junk
// all cards in the [6] deck. If you don't, splay one color of your cards
// right.
const ADAM_SMITH: FigureDef = figure(
    "F060",
    &[first(
        On::Dogma(None),
        Executor,
        &[
            junk(BaseDeck(Const(6))).all().shortcut(Shortcut::DeckTop),
            if_(&Not(DID_ANY), 1),
            splay_color(Executor, ALL_COLORS, Splay::Right),
        ],
    )
    .when(&Compare(
        M(Opponent, Metric::ChosenIcon),
        Cmp::Less,
        M(Executor, Metric::ChosenIcon),
    ))],
)
.each(&[Each::Icons]);

// You may issue a Trade Decree with any two figures. / If you would take a
// Draw action, first return a card from your hand. If you do, draw a card
// from any set of value equal to the returned card.
const JOHN_HARRISON: FigureDef = figure(
    "F061",
    &[first(
        On::DrawAction,
        Executor,
        &[
            return_(HAND),
            if_(DID, 1),
            draw_age(Value(Reg::Last)).any_set(),
        ],
    )],
)
.decree(Trade);

// If you would meld a card, first choose a value and meld all other cards of
// that value from your hand and score pile. / If you would take a Draw
// action, first draw a [7].
/// Neither the hand nor the score pile holds another card of the chosen
/// value (backs show values): nothing to meld.
const GAUSS_IDLE: Cond = Compare(
    Count(&filtered(
        &Selector::Union(&[HAND, SCORE]),
        &[
            Has(ValueIs(Cmp::Equal, Expr::Chosen)),
            Lacks(Is(Reg::Bound)),
        ],
    )),
    Cmp::Equal,
    Const(0),
);
const CARL_FRIEDRICH_GAUSS: FigureDef = figure(
    "F062",
    &[
        first(
            On::Move(&[Meld]),
            Executor,
            &[
                choose(Executor, Choice::Value(ValueChoices::Any))
                    .alias(Alias::NoOpWhen(&GAUSS_IDLE)),
                meld(filtered(
                    &Selector::Union(&[HAND, SCORE]),
                    &[
                        Has(ValueIs(Cmp::Equal, Expr::Chosen)),
                        Lacks(Is(Reg::Bound)),
                    ],
                ))
                .all()
                .shortcut(Shortcut::ByColor),
            ],
        ),
        first(On::DrawAction, Executor, &[draw(7)]),
    ],
);

// You may issue an Advancement Decree with any two figures. / If you would
// meld a card, first if there is a top figure of the same color on any
// opponent's board, transfer that figure to your hand.
const BENJAMIN_FRANKLIN: FigureDef = figure(
    "F063",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[transfer(
            filtered(
                &Board(Opponent, ColorSet::Of(Reg::Bound), BoardRegion::Top),
                &[Has(IsFigure)],
            ),
            Destination::Hand(Executor),
        )],
    )],
)
.decree(Advancement);

// You may issue a Rivalry Decree with any two figures. / If you would score a
// card, first junk all cards from your score pile, then draw and score four
// [5]. If you score only [5]s, draw and score four [5].
const LUDWIG_VAN_BEETHOVEN: FigureDef = figure(
    "F065",
    &[first(
        On::Move(&[Score]),
        Executor,
        &[
            junk(SCORE).all().shortcut(Shortcut::AnyOrder),
            draws(Const(4), 5, Then::Score),
            if_(
                &All(&[
                    holds(&SCORE),
                    Compare(
                        Count(&filtered(&SCORE, &[Lacks(Values(&[5]))])),
                        Cmp::Equal,
                        Const(0),
                    ),
                ]),
                1,
            ),
            draws(Const(4), 5, Then::Score),
        ],
    )],
)
.decree(Rivalry);

const LEE_TOPS: Selector = filtered(
    &Tops(Either),
    &[Has(HasIcon(Health)), Lacks(Test::ColorOf(Reg::Bound))],
);

// If a player would dogma a card with a demand effect, first transfer a top
// card of another color with [Health] from anywhere to any player's board. /
// Each seven [Health] on your board counts as an achievement.
const ROBERT_E_LEE: FigureDef = figure(
    "F066",
    &[first(
        On::Dogma(None),
        Either,
        &[
            if_(&holds(&LEE_TOPS), 5),
            choose(Executor, Choice::Card(&LEE_TOPS)),
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Transfer the selected card to your board"),
                    (2, "Transfer the selected card to your opponent's board"),
                ]),
            ),
            transfer(Selector::Reg(Reg::Chosen), Destination::Board(Executor)),
            Instruction::Jump(1),
            transfer(Selector::Reg(Reg::Chosen), Destination::Board(Opponent)),
        ],
    )
    .when(&CardIs(Reg::Bound, Has(Test::Demand)))],
)
.each(&[Each::Achievements]);

// If you would claim an achievement, first return three cards from your
// hand. If you do, achieve all other cards in your hand, ignoring
// eligibility.
const ROWLAND_HILL: FigureDef = figure(
    "F068",
    &[first(
        On::Claim,
        Executor,
        &[
            return_(HAND).exactly(Const(3)).shortcut(ORIGIN),
            if_(&Cond::Did(Outcome::Exactly(3)), 1),
            achieve(HAND).all().shortcut(Shortcut::AnyOrder),
        ],
    )],
);

// If you would dogma a card of a color you have splayed right, first splay
// that color on your board up. / If you would claim an achievement, first
// transfer the bottom yellow card on each board to the available
// achievements.
const GEORGE_STEPHENSON: FigureDef = figure(
    "F070",
    &[
        first(
            On::Dogma(None),
            Executor,
            &[splay_color(Executor, ColorSet::Of(Reg::Bound), Splay::Up)],
        )
        .when(&Cond::Splayed {
            who: Executor,
            colors: ColorSet::Of(Reg::Bound),
            direction: Splay::Right,
            present: true,
        }),
        first(
            On::Claim,
            Executor,
            &[
                transfer(
                    Board(Executor, ColorSet::Only(&[Yellow]), BoardRegion::Bottom),
                    Destination::Supply,
                ),
                transfer(
                    Board(Opponent, ColorSet::Only(&[Yellow]), BoardRegion::Bottom),
                    Destination::Supply,
                ),
            ],
        ),
    ],
);

// You may issue a Rivalry Decree with any two figures. / If you would score a
// card, first choose a figure in any score pile. Choose to either score it,
// or transfer it to the available achievements.
const QUEEN_VICTORIA: FigureDef = figure(
    "F075",
    &[first(
        On::Move(&[Score]),
        Executor,
        &[
            choose(
                Executor,
                Choice::Card(&filtered(&Selector::Score(Either), &[Has(IsFigure)])),
            ),
            if_(&holds(&Selector::Reg(Reg::Chosen)), 4),
            choose(
                Executor,
                Choice::Branch(&[
                    (0, "Score the selected Figure"),
                    (
                        2,
                        "Transfer the selected Figure to the available achievements",
                    ),
                ]),
            ),
            score(Selector::Reg(Reg::Chosen)),
            Instruction::Jump(1),
            transfer(Selector::Reg(Reg::Chosen), Destination::Supply),
        ],
    )],
)
.decree(Rivalry);

// You may issue a War Decree with any two figures. / If you would score a
// non-figure, instead score the top card of its color from all boards, and
// score a card in any score pile.
const ERWIN_ROMMEL: FigureDef = figure(
    "F076",
    &[instead(
        On::Move(&[Score]),
        Executor,
        &[
            score(Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top)),
            score(Board(Opponent, ColorSet::Of(Reg::Bound), BoardRegion::Top)),
            score(Selector::Score(Either)),
        ],
    )
    .when(&CardIs(Reg::Bound, Lacks(IsFigure)))],
)
.decree(War);

// If a player would dogma a card of a color you do not have splayed left,
// first splay that color on any board left and draw two [2]. If you splay a
// fifth color left on your board this way, you win.
const CARESSE_CROSBY: FigureDef = figure(
    "F078",
    &[first(
        On::Dogma(None),
        Either,
        &[
            Instruction::Splay(SplayStep {
                who: Either,
                choices: SplayChoices::Fixed {
                    colors: ColorSet::Of(Reg::Bound),
                    direction: Splay::Left,
                    from: None,
                },
                // Either board's pile, a no-op included.
                targets: SplayTargets::Nonempty,
                optional: false,
                all: false,
            }),
            draw(2),
            draw(2),
            if_(
                &Compare(
                    Count(&filtered(
                        &TOPS,
                        &[Has(Test::StackSplayed(Some(Splay::Left)))],
                    )),
                    Cmp::Equal,
                    Const(5),
                ),
                1,
            ),
            WIN,
        ],
    )
    .when(&Cond::Splayed {
        who: Executor,
        colors: ColorSet::Of(Reg::Bound),
        direction: Splay::Left,
        present: false,
    })],
);

// You may issue an Expansion Decree with any two figures. / If you would meld
// a card, first score an opponent's top card with neither [Concept] nor
// [Efficiency]. If the scored card is the highest in your score pile, draw a
// [9].
const NIKOLA_TESLA: FigureDef = figure(
    "F079",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            score(filtered(
                &Tops(Opponent),
                &[Lacks(HasIcon(Concept)), Lacks(HasIcon(Efficiency))],
            )),
            if_(
                &All(&[DONE, Compare(Value(Reg::Last), Cmp::AtLeast, Max(&SCORE))]),
                1,
            ),
            draw(9),
        ],
    )],
)
.decree(Expansion);

// You may issue a Trade Decree with any two figures. / If a player would
// dogma a card, first splay the color of the card up on your board.
const J_P_MORGAN: FigureDef = figure(
    "F080",
    &[first(
        On::Dogma(None),
        Either,
        &[splay_color(Executor, ColorSet::Of(Reg::Bound), Splay::Up)],
    )],
)
.decree(Trade);

// Each [Efficiency] on your board provides an additional number of points
// equal to the number of [Efficiency] on your board. / If a player would
// score a card, first junk a card from that player's score pile.
const EMMY_NOETHER: FigureDef = figure(
    "F081",
    &[first(
        On::Move(&[Score]),
        Either,
        &[junk(Selector::Score(Who::Demander))],
    )],
)
.each(&[Each::Score]);

// You may issue an Advancement Decree with any two figures. / If you would
// take a Draw action, first meld all cards with [Concept] or [Efficiency]
// from each player's hand.
/// Both hands as the karma body revealed them (the hands are shown before
/// their cards move), and the cards that arrived unseen since.
const SHOWN_HANDS: Slot = Slot::effect(0);
const SHOWN_HAND_CARDS: Selector = filtered(&Selector::Hand(Either), &[Has(InSlot(SHOWN_HANDS))]);
const UNSHOWN_HAND_CARDS: Selector =
    filtered(&Selector::Hand(Either), &[Lacks(InSlot(SHOWN_HANDS))]);
const ALBERT_EINSTEIN: FigureDef = figure(
    "F083",
    &[first(
        On::DrawAction,
        Executor,
        &[
            reveal_all(Selector::Hand(Either)).keep(SHOWN_HANDS),
            meld(Selector::Union(&[
                filtered(&SHOWN_HAND_CARDS, &[Has(HasIcon(Concept))]),
                filtered(&SHOWN_HAND_CARDS, &[Has(HasIcon(Efficiency))]),
                filtered(&UNSHOWN_HAND_CARDS, &[Has(HasIcon(Concept))]),
                filtered(&UNSHOWN_HAND_CARDS, &[Has(HasIcon(Efficiency))]),
            ]))
            .all()
            .shortcut(Shortcut::ByColor),
            // Show any card that arrived unseen in either hand meanwhile.
            reveal_all(UNSHOWN_HAND_CARDS),
        ],
    )],
)
.decree(Advancement);

// If you would meld a card, first meld all other cards of that color from
// each player's hand, then draw and achieve a [9] for each card you meld in
// this way, regardless of eligibility.
const RUTH_MELDED: Slot = Slot::effect(1);
const RUTH_HANDLER: FigureDef = figure(
    "F088",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            // The card being melded is shown when its color is first read.
            reveal_all(filtered(&Selector::Hand(Either), &[Lacks(Is(Reg::Bound))]))
                .keep(SHOWN_HANDS),
            meld(Selector::Union(&[
                filtered(
                    &SHOWN_HAND_CARDS,
                    &[Has(Test::ColorOf(Reg::Bound)), Lacks(Is(Reg::Bound))],
                ),
                filtered(
                    &UNSHOWN_HAND_CARDS,
                    &[Has(Test::ColorOf(Reg::Bound)), Lacks(Is(Reg::Bound))],
                ),
            ]))
            .all()
            .shortcut(Shortcut::ByColor)
            .keep(RUTH_MELDED),
            // Show any card that arrived unseen in either hand meanwhile.
            reveal_all(filtered(&UNSHOWN_HAND_CARDS, &[Lacks(Is(Reg::Bound))])),
            draws(Count(&Selector::Slot(RUTH_MELDED)), 9, Then::Achieve),
        ],
    )],
);

// If you would meld a card, first draw and score a [9]. If the scored card is
// yellow, junk all cards in all score piles and all available achievements.
const CHE_GUEVARA: FigureDef = figure(
    "F089",
    &[first(
        On::Move(&[Meld]),
        Executor,
        &[
            draw(9),
            // Its color decides the junking, so it is shown.
            reveal(LAST),
            score(LAST),
            if_(&CardIs(Reg::Last, Has(Test::Color(Yellow))), 2),
            junk(Selector::Score(Either))
                .all()
                .shortcut(Shortcut::AnyOrder),
            junk(AvailableAchievements)
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    )],
);

// If you would dogma a card, first you may achieve a card from your hand with
// featured icon matching that card's featured icon, regardless of
// eligibility. If you don't, draw an [11].
const TAIICHI_OHNO: FigureDef = figure(
    "F091",
    &[first(
        On::Dogma(None),
        Executor,
        &[
            achieve(filtered(&HAND, &[Has(Test::FeaturedAs(Reg::Bound))]))
                .optional()
                .verify(),
            if_(&Not(DID), 1),
            draw(11),
        ],
    )],
);

// If a player would draw a figure, first junk all cards in the [9] or [10]
// deck. / Each valued card in the junk counts as being in your score pile.
const WERNHER_VON_BRAUN: FigureDef = figure(
    "F093",
    &[first(
        On::Draw,
        Either,
        &[
            unless_decks_empty!(9, 10),
            choose_deck(&[9, 10]),
            CHOSEN_DECK_JUNK,
        ],
    )
    .when(&CardIs(Reg::Bound, Has(IsFigure)))],
)
.each(&[Each::ScoreMembership, Each::Score]);

const LOW_VALUE: Cond = CardIs(Reg::Bound, Has(ValueIs(Cmp::Less, Const(4))));

// If a player would transfer, return, draw, score, meld, tuck, or junk a card
// of value less than 4, instead that player loses.
const ROD_SERLING: FigureDef = figure(
    "F095",
    &[
        instead(
            On::Move(&[Transfer, Return, Score, Meld, Tuck, Junk]),
            Either,
            &[LOSES],
        )
        .when(&LOW_VALUE),
        instead(On::Draw, Either, &[LOSES]).when(&LOW_VALUE),
    ],
);

// You may issue a War Decree with any two figures. / If you would take a
// Dogma action, first score any top card with [Prosperity] or [Industry] from
// anywhere.
const MARGARET_THATCHER: FigureDef = figure(
    "F097",
    &[first(
        On::Dogma(None),
        Executor,
        &[score(Selector::Union(&[
            filtered(&Tops(Either), &[Has(HasIcon(Prosperity))]),
            filtered(&Tops(Either), &[Has(HasIcon(Industry))]),
        ]))],
    )],
)
.decree(War);

// If you would dogma a card, first score your bottom card of its color for
// each [HEX] in that color on your board. / If you would draw a card, first
// draw and tuck a card of the same value.
const STEPHEN_HAWKING: FigureDef = figure(
    "F103",
    &[
        first(
            On::Dogma(None),
            Executor,
            &[score(Board(
                Executor,
                ColorSet::Of(Reg::Bound),
                BoardRegion::Bottom,
            ))
            .exactly(Expr::Visible(&Visibility {
                who: Who::Executor,
                colors: ColorSet::Of(Reg::Bound),
                seen: Seen::Images,
            }))],
        ),
        first(
            On::Draw,
            Executor,
            &[draw_age(Value(Reg::Bound)), tuck(LAST)],
        )
        .when(&DRAWS),
    ],
);

// If you would score a card, first if your top card of the same color has
// [Authority], you win. / If a player would meld a card, instead return the
// top three cards of its color from all boards. Then that player tucks the
// card.
const YUNA_KIM: FigureDef = figure(
    "F105",
    &[
        first(On::Move(&[Score]), Executor, &[WIN]).when(&holds(&filtered(
            &Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top),
            &[Has(HasIcon(Authority))],
        ))),
        instead(
            On::Move(&[Meld]),
            Either,
            &[
                return_(Board(Executor, ColorSet::Of(Reg::Bound), BoardRegion::Top))
                    .exactly(Const(3)),
                return_(Board(Opponent, ColorSet::Of(Reg::Bound), BoardRegion::Top))
                    .exactly(Const(3)),
                tuck(THE)
                    .to(Destination::Board(Who::Demander))
                    .by(Who::Demander),
            ],
        ),
    ],
);

// If you would dogma a card using a featured icon, first you may return a
// card from your hand with that icon. If you do, issue a Decree of color
// matching the color of the returned card.
const SHEIKH_TAMIM: FigureDef = figure(
    "F107",
    &[first(
        On::Dogma(None),
        Executor,
        &[
            return_(filtered(&HAND, &[Has(Test::ChosenIcon)]))
                .optional()
                .verify(),
            if_(DID, 1),
            Instruction::Special(SpecialStep::Decree(ColorSet::Of(Reg::Last))),
        ],
    )],
);

// If a player would dogma a card, first junk another top card with
// [Industry] from any board. Then if there is no top card with [Industry] on
// any board, you win.
const GRETA_THUNBERG: FigureDef = figure(
    "F108",
    &[first(
        On::Dogma(None),
        Either,
        &[
            junk(filtered(
                &Tops(Either),
                &[Has(HasIcon(Industry)), Lacks(Is(Reg::Bound))],
            )),
            if_(
                &Not(&holds(&filtered(&Tops(Either), &[Has(HasIcon(Industry))]))),
                1,
            ),
            WIN,
        ],
    )],
);

// If an opponent would take a Dogma action, instead the opponent draws,
// reveals, and returns a card of any value. The opponent super-executes their
// top card of the revealed card's color, if there is one.
const THERESE_KIRONGOZI: FigureDef = figure(
    "F109",
    &[instead(
        On::Dogma(None),
        Opponent,
        &[
            choose(Executor, Choice::Value(ValueChoices::Any)),
            draw_age(Expr::Chosen),
            reveal(LAST),
            return_(LAST),
            bind(Board(Executor, ColorSet::Of(Reg::Last), BoardRegion::Top)),
            SUPER_EXECUTE,
        ],
    )
    .by_actor()],
);

// If you would execute a dogma effect, first draw and score a [9].
const GIUSEPPE_SCIONTI: FigureDef = figure(
    "F112",
    &[first(On::DogmaEffect, Executor, &[draw(9), score(LAST)])],
);

// If you would take a Dogma action as your second action, instead
// super-execute each of your top cards, if you have one, in red, blue,
// green, yellow, purple order.
const KONSTANTIN_NOVOSELOV: FigureDef = figure(
    "F113",
    &[instead(
        On::Dogma(None),
        Executor,
        &[
            bind(TopOf(Executor, Red)),
            SUPER_EXECUTE,
            bind(TopOf(Executor, Color::Blue)),
            SUPER_EXECUTE,
            bind(TopOf(Executor, Green)),
            SUPER_EXECUTE,
            bind(TopOf(Executor, Yellow)),
            SUPER_EXECUTE,
            bind(TopOf(Executor, Color::Purple)),
            SUPER_EXECUTE,
        ],
    )
    .when(&Cond::Action(2))],
);

// If you would not be eligible for sharing, instead become eligible for
// sharing and junk an available achievement of value 1.
const SARGON_OF_AKKAD: FigureDef = figure(
    "F009",
    &[instead(
        On::Ineligible,
        Executor,
        &[junk(filtered(&AvailableAchievements, &[Has(Values(&[1]))]))],
    )],
);

// You may issue a War Decree with any two figures. / If you would draw for a
// share bonus, first meld any number of cards from your hand having the
// Dogma action's featured icon.
const SUN_TZU: FigureDef = figure(
    "F016",
    &[first(
        On::ShareDraw,
        Executor,
        &[meld(filtered(&HAND, &[Has(Test::ChosenIcon)]))
            .up_to(None)
            .shortcut(Shortcut::ByColor)],
    )],
)
.decree(War);

// You may issue a Rivalry Decree with any two figures. / If you would claim
// a standard achievement, first you may achieve a card from your score pile
// if eligible for both without its points.
const MURASAKI_SHIKIBU: FigureDef = figure(
    "F034",
    &[first(
        On::Claim,
        Executor,
        &[achieve(filtered(&SCORE, &[Has(Test::EligibleBoth(Reg::Bound))])).optional()],
    )
    .when(&STANDARD)],
)
.decree(Rivalry);

// If you would draw a card for sharing, first draw two cards of the same
// value. / If another player would draw a card for sharing, first score a
// card from your hand.
const AL_KINDI: FigureDef = figure(
    "F035",
    &[
        first(
            On::ShareDraw,
            Executor,
            &[Instruction::Draw(Draw {
                age: Max(&TOPS),
                set: CardSet::Base,
                any_set: false,
                count: Some(Const(2)),
                then: Then::Keep,
                optional: false,
                keep: None,
            })],
        ),
        first(On::ShareDraw, Opponent, &[score(HAND)]),
    ],
);

// If you would dogma a card as your first action, instead junk all cards in
// the [6] deck. Then draw and reveal one [7], two [8], or three [9]. If the
// drawn cards have more [Efficiency] than you have on your board, return
// them.
const CHRISTIAAN_HUYGENS: FigureDef = figure(
    "F053",
    &[instead(On::Dogma(None), Executor, &[])
        .when(&Cond::Action(1))
        .native(KarmaNative::Huygens)],
);

// If an opponent would not draw a card for sharing after a Dogma action,
// first draw and reveal a [10]. If the revealed card is blue, you win.
const GRACE_HOPPER: FigureDef = figure(
    "F092",
    &[first(
        On::NoShareDraw,
        Opponent,
        &[
            draws(Const(1), 10, Then::Reveal),
            if_(&Cond::DrawnColor(Color::Blue), 1),
            WIN,
        ],
    )],
);

// You may issue a Trade Decree with any two figures. / If a player would
// take a Dogma action, first you may return your bottom green card. If you
// do, you have sole majority in every icon during the action.
const MUHAMMAD_YUNUS: FigureDef = figure(
    "F100",
    &[first(
        On::Dogma(None),
        Either,
        &[
            return_(Board(
                Executor,
                ColorSet::Only(&[Green]),
                BoardRegion::Bottom,
            ))
            .optional(),
            if_(DID, 1),
            Instruction::Special(SpecialStep::Majority),
        ],
    )],
)
.decree(Trade);

// If another player would not draw a card for sharing after a Dogma action,
// or you would draw a card for sharing, first self-execute the card
// activated by the Dogma action.
const SUSAN_BLACKMORE: FigureDef = figure(
    "F102",
    &[
        first(On::NoShareDraw, Opponent, &[SELF_EXECUTE]),
        first(On::ShareDraw, Executor, &[SELF_EXECUTE]),
    ],
);

// If you would meld a figure, instead tuck the figure and claim an
// achievement, regardless of eligibility.
const MARTIN_SCORSESE: FigureDef = figure(
    "F104",
    &[instead(
        On::Move(&[Meld]),
        Executor,
        &[tuck(THE), Instruction::Special(SpecialStep::ClaimAny)],
    )
    .when(&CardIs(Reg::Bound, Has(IsFigure)))],
);

// If you would execute a dogma effect, first return any number of cards from
// your hand. If you do, execute that dogma effect an additional time for
// every card you return, then draw an [11].
const MAX_VERSTAPPEN: FigureDef = figure(
    "F106",
    &[first(
        On::DogmaEffect,
        Executor,
        &[
            return_(HAND).up_to(None).shortcut(ORIGIN),
            if_(DID_ANY, 2),
            Instruction::Native(Native::MaxVerstappen),
            draw(11),
        ],
    )],
);

// If you would draw a card, first choose a value and return all cards of that
// value from all score piles. If you return exactly one card, achieve it,
// regardless of eligibility.
/// Neither score pile holds a card of the chosen value: nothing to return.
const NO_CHOSEN_SCORE_EITHER: Cond = Compare(
    Count(&filtered(
        &Selector::Score(Either),
        &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
    )),
    Cmp::Equal,
    Const(0),
);
const BOYAN_SLAT: FigureDef = figure(
    "F111",
    &[first(
        On::Draw,
        Executor,
        &[
            choose(Executor, Choice::Value(ValueChoices::Any))
                .alias(Alias::NoOpWhen(&NO_CHOSEN_SCORE_EITHER)),
            return_(filtered(
                &Selector::Score(Either),
                &[Has(ValueIs(Cmp::Equal, Expr::Chosen))],
            ))
            .all()
            .shortcut(ORIGIN_ALL),
            if_(&Cond::Did(Outcome::Exactly(1)), 1),
            achieve(THE_LAST),
        ],
    )
    .when(&DRAWS)],
);

// If you would take a Draw action, instead reveal the top card of five
// decks, one at a time. If you reveal five colors, you win. Otherwise, draw
// one of the revealed cards, and splay aslant a color on your board.
const LILY_HEVESH: FigureDef = figure(
    "F115",
    &[instead(On::DrawAction, Executor, &[]).native(KarmaNative::LilyHevesh)],
);

impl Figure {
    /// The Figure's complete printed declaration.
    pub const fn def(self) -> &'static FigureDef {
        match self {
            Self::Gilgamesh => &GILGAMESH,
            Self::Tigernmas => &TIGERNMAS,
            Self::Shennong => &SHENNONG,
            Self::Sneferu => &SNEFERU,
            Self::FuXi => &FU_XI,
            Self::Daedalus => &DAEDALUS,
            Self::HuangDi => &HUANG_DI,
            Self::Ptahhotep => &PTAHHOTEP,
            Self::Sinuhe => &SINUHE,
            Self::Homer => &HOMER,
            Self::AlexanderTheGreat => &ALEXANDER_THE_GREAT,
            Self::CaiLun => &CAI_LUN,
            Self::Ptolemy => &PTOLEMY,
            Self::Archimedes => &ARCHIMEDES,
            Self::Plato => &PLATO,
            Self::TranHuangDao => &TRAN_HUANG_DAO,
            Self::Avicenna => &AVICENNA,
            Self::ShenKuo => &SHEN_KUO,
            Self::SuSong => &SU_SONG,
            Self::SejongTheGreat => &SEJONG_THE_GREAT,
            Self::YiSunSin => &YI_SUN_SIN,
            Self::Michelangelo => &MICHELANGELO,
            Self::GalileoGalilei => &GALILEO_GALILEI,
            Self::PedroAlvaresCabral => &PEDRO_ALVARES_CABRAL,
            Self::JohannesKepler => &JOHANNES_KEPLER,
            Self::NiccoloMachiavelli => &NICCOLO_MACHIAVELLI,
            Self::WilliamShakespeare => &WILLIAM_SHAKESPEARE,
            Self::AntonieVanLeeuwenhoek => &ANTONIE_VAN_LEEUWENHOEK,
            Self::ChristopherPolhem => &CHRISTOPHER_POLHEM,
            Self::JohnLoudonMcAdam => &JOHN_LOUDON_MCADAM,
            Self::CatherineTheGreat => &CATHERINE_THE_GREAT,
            Self::JohnEricsson => &JOHN_ERICSSON,
            Self::Nightingale => &FLORENCE_NIGHTINGALE,
            Self::AlfredNobel => &ALFRED_NOBEL,
            Self::CharlesDarwin => &CHARLES_DARWIN,
            Self::JamesClerkMaxwell => &JAMES_CLERK_MAXWELL,
            Self::EmperorMeiji => &EMPEROR_MEIJI,
            Self::JohnVonNeumann => &JOHN_VON_NEUMANN,
            Self::MarieCurie => &MARIE_CURIE,
            Self::HGWells => &H_G_WELLS,
            Self::DukeEllington => &DUKE_ELLINGTON,
            Self::NelsonMandela => &NELSON_MANDELA,
            Self::Kalashnikov => &MIKHAIL_KALASHNIKOV,
            Self::HedyLamarr => &HEDY_LAMARR,
            Self::Pele => &PELE,
            Self::JackieChan => &JACKIE_CHAN,
            Self::AlexTrebek => &ALEX_TREBEK,
            Self::ShigeruMiyamoto => &SHIGERU_MIYAMOTO,
            Self::SergeyBrin => &SERGEY_BRIN,
            Self::BearGrylls => &BEAR_GRYLLS,
            Self::EmmaWatson => &EMMA_WATSON,
            Self::Hammurabi => &HAMMURABI,
            Self::SunshuAo => &SUNSHU_AO,
            Self::Hatshepsut => &HATSHEPSUT,
            Self::Imhotep => &IMHOTEP,
            Self::XimenBao => &XIMEN_BAO,
            Self::AugustusCaesar => &AUGUSTUS_CAESAR,
            Self::ZhangHeng => &ZHANG_HENG,
            Self::Confucius => &CONFUCIUS,
            Self::Saladin => &SALADIN,
            Self::Rhazes => &RHAZES,
            Self::Alhazen => &ALHAZEN,
            Self::AminaSukhera => &AMINA_SUKHERA,
            Self::LeonardoDaVinci => &LEONARDO_DA_VINCI,
            Self::TakiAdDin => &TAKI_AD_DIN,
            Self::Shivaji => &SHIVAJI,
            Self::PeterTheGreat => &PETER_THE_GREAT,
            Self::SamuelDeChamplain => &SAMUEL_DE_CHAMPLAIN,
            Self::BartholomewRoberts => &BARTHOLOMEW_ROBERTS,
            Self::IsaacNewton => &ISAAC_NEWTON,
            Self::BartolomeoCristofori => &BARTOLOMEO_CRISTOFORI,
            Self::JohannesVermeer => &JOHANNES_VERMEER,
            Self::NapoleonBonaparte => &NAPOLEON_BONAPARTE,
            Self::Tecumseh => &TECUMSEH,
            Self::EdwardJenner => &EDWARD_JENNER,
            Self::AdamSmith => &ADAM_SMITH,
            Self::JohnHarrison => &JOHN_HARRISON,
            Self::CarlFriedrichGauss => &CARL_FRIEDRICH_GAUSS,
            Self::BenjaminFranklin => &BENJAMIN_FRANKLIN,
            Self::LudwigVanBeethoven => &LUDWIG_VAN_BEETHOVEN,
            Self::RobertELee => &ROBERT_E_LEE,
            Self::RowlandHill => &ROWLAND_HILL,
            Self::GeorgeStephenson => &GEORGE_STEPHENSON,
            Self::QueenVictoria => &QUEEN_VICTORIA,
            Self::ErwinRommel => &ERWIN_ROMMEL,
            Self::CaresseCrosby => &CARESSE_CROSBY,
            Self::NikolaTesla => &NIKOLA_TESLA,
            Self::JPMorgan => &J_P_MORGAN,
            Self::EmmyNoether => &EMMY_NOETHER,
            Self::AlbertEinstein => &ALBERT_EINSTEIN,
            Self::RuthHandler => &RUTH_HANDLER,
            Self::CheGuevara => &CHE_GUEVARA,
            Self::TaiichiOhno => &TAIICHI_OHNO,
            Self::WernherVonBraun => &WERNHER_VON_BRAUN,
            Self::RodSerling => &ROD_SERLING,
            Self::MargaretThatcher => &MARGARET_THATCHER,
            Self::StephenHawking => &STEPHEN_HAWKING,
            Self::YunaKim => &YUNA_KIM,
            Self::SheikhTamim => &SHEIKH_TAMIM,
            Self::GretaThunberg => &GRETA_THUNBERG,
            Self::ThereseKirongozi => &THERESE_KIRONGOZI,
            Self::GiuseppeScionti => &GIUSEPPE_SCIONTI,
            Self::KonstantinNovoselov => &KONSTANTIN_NOVOSELOV,
            Self::Sargon => &SARGON_OF_AKKAD,
            Self::SunTzu => &SUN_TZU,
            Self::MurasakiShikibu => &MURASAKI_SHIKIBU,
            Self::AlKindi => &AL_KINDI,
            Self::ChristiaanHuygens => &CHRISTIAAN_HUYGENS,
            Self::GraceHopper => &GRACE_HOPPER,
            Self::MuhammadYunus => &MUHAMMAD_YUNUS,
            Self::SusanBlackmore => &SUSAN_BLACKMORE,
            Self::MartinScorsese => &MARTIN_SCORSESE,
            Self::MaxVerstappen => &MAX_VERSTAPPEN,
            Self::BoyanSlat => &BOYAN_SLAT,
            Self::LilyHevesh => &LILY_HEVESH,
        }
    }

    pub const fn code(self) -> &'static str {
        self.def().code
    }

    /// The trigger kinds ([`On::bit`]) of its Would clauses; "when melded"
    /// is dispatched separately.
    pub(crate) fn watched(self) -> u32 {
        self.def()
            .karma
            .iter()
            .filter(|karma| karma.reaction != Reaction::Melded)
            .fold(0, |watched, karma| watched | karma.on.bit())
    }
}

pub fn figure_metadata(code: &str) -> Option<&'static CardMetadata> {
    FIGURE_METADATA
        .binary_search_by_key(&code, |metadata| metadata.code)
        .ok()
        .map(|index| &FIGURE_METADATA[index])
}

/// Construct a declared Figure; physical metadata alone is insufficient.
pub fn figure_card(code: &str) -> Result<Card, Error> {
    let metadata = figure_metadata(code).ok_or(Error::InvalidSetup("unknown Figure card"))?;
    let figure = Figure::ALL
        .into_iter()
        .find(|figure| figure.code() == code)
        .ok_or(Error::Unsupported("Figure program not authored"))?;
    Ok(Card {
        code: metadata.code,
        set: CardSet::Figures,
        color: metadata.color,
        value: metadata.value,
        figure: Some(figure),
        echo: None,
        symbols: crate::four_slots(metadata.symbols),
        featured: metadata.featured,
        // Karma lives in the Figure's declaration, not in Dogma effects.
        effects: Vec::new(),
    })
}

#[cfg(test)]
mod operand_front_tests {
    use super::*;

    /// Clauses whose triggering card can be hidden from an observer (moves,
    /// draws, claims) and that read its front. A hidden operand is revealed
    /// first: before the hooks for a condition, before the body's first read
    /// otherwise. Every one reads the operand's color; Napoleon, von Neumann
    /// and Sunshu Ao do so only after melding or executing it, when it is
    /// public.
    #[test]
    fn only_these_hidden_operand_clauses_read_a_front() {
        let mut flagged = Vec::new();
        for figure in Figure::ALL {
            for (clause, karma) in figure.def().karma.iter().enumerate() {
                let hidden = matches!(
                    karma.on,
                    On::Move(_) | On::Draw | On::Claim | On::MeldAction
                );
                if hidden
                    && (karma.when.is_some_and(Cond::reads_card_front)
                        || karma.body_reads_card_front())
                {
                    flagged.push((figure, clause));
                }
            }
        }
        assert_eq!(
            flagged,
            [
                (Figure::ShenKuo, 0),
                (Figure::EmmaWatson, 1),
                (Figure::JohnVonNeumann, 1),
                (Figure::CatherineTheGreat, 0),
                (Figure::SunshuAo, 0),
                (Figure::BartolomeoCristofori, 0),
                (Figure::NapoleonBonaparte, 0),
                (Figure::BenjaminFranklin, 0),
                (Figure::ErwinRommel, 0),
                (Figure::RuthHandler, 0),
                (Figure::YunaKim, 0),
                (Figure::YunaKim, 1),
            ]
        );
    }
}
