use inno_engine::*;

mod common;
use common::*;

const E: Expansions = Expansions::ECHOES;
const UE: Expansions = Expansions::UNSEEN.union(Expansions::ECHOES);

fn tech(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, Vec::new())
}

/// A blank card in `set`'s deck of `value`.
fn in_deck(f: &mut Fixture, set: CardSet, color: Color, value: u8) -> CardId {
    let card = tech(f, color, value, Zone::Deck(Deck { set, age: value }));
    f.cards[usize::from(card.0)].set = set;
    card
}

fn symbols(f: &mut Fixture, card: CardId, symbols: [Symbol; 4]) {
    f.cards[usize::from(card.0)].symbols = four_slots(symbols);
}

/// Advance to the end, picking each decision with `pick`.
fn finish(game: &mut Game, mut pick: impl FnMut(&Decision) -> usize) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    for _ in 0..200 {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            status => {
                assert!(game.position().check_conservation());
                return (status, events);
            }
        }
    }
    panic!("did not finish");
}

/// One printed effect of `card` executed by `executor`, with A demanding.
fn execute(position: Position, card: CardId, effect: u16, executor: Player) -> Game {
    let mut game = Game::new(position, E).unwrap();
    let mut context = Context::new(executor);
    context.demander = A;
    context.source_owner = A;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    game
}

/// A's Meld action of a blue 11 promotes `source` from A's Forecast, keeping
/// only its dogma effect `effect`. A purple 10 showing the card's featured
/// icon in every position keeps B from sharing.
fn promote(mut f: Fixture, source: CardId, effect: usize) -> Game {
    let card = &mut f.cards[usize::from(source.0)];
    card.effects = vec![card.effects[effect].clone()];
    card.echo = None;
    let featured = card.featured.unwrap();
    f.placements
        .iter_mut()
        .find(|(card, _)| *card == source)
        .unwrap()
        .1 = Zone::Forecast(A);
    let boost = tech(&mut f, Color::Purple, 10, Zone::Board(A, Color::Purple));
    f.cards[usize::from(boost.0)].symbols = [Symbol::Icon(featured); SLOTS];
    let action = tech(&mut f, Color::Blue, 11, Zone::Hand(A));
    let mut game = Game::new(f.position(), E).unwrap();
    game.take_action(Action::Meld { card: action }, ActionKind::Regular)
        .unwrap();
    game
}

fn option(menu: &Decision, wanted: &Alternative) -> Option<usize> {
    menu.options.iter().position(|o| o == wanted)
}

fn moves_card(option: &Alternative, card: CardId) -> bool {
    matches!(
        option,
        Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
        | Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
        if *c == card
    )
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|o| moves_card(o, card))
}

/// Linguistics: "If you do, and Linguistics was foreseen, junk all available
/// achievements of that value." A same-value Base card drawn because the Echoes deck is
/// empty satisfies "if you do"; only an age skip fails it.
#[test]
fn foreseen_linguistics_junks_after_a_same_value_base_fallback() {
    for echoes_two in [false, true] {
        let mut f = Fixture::default();
        let linguistics = f.add(echoes_card("E023").unwrap(), Zone::Junk);
        let bonus = tech(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
        symbols(
            &mut f,
            bonus,
            [
                Symbol::Bonus(2),
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
            ],
        );
        let base = in_deck(&mut f, CardSet::Base, Color::Green, 2);
        let echo = echoes_two.then(|| in_deck(&mut f, CardSet::Echoes, Color::Green, 2));
        let twos = [
            tech(&mut f, Color::Red, 2, Zone::AvailableAchievements),
            tech(&mut f, Color::Yellow, 2, Zone::AvailableAchievements),
        ];
        let three = tech(&mut f, Color::Green, 3, Zone::AvailableAchievements);
        // A's single highest top (the purple 10) routes the draw to Echoes.
        let mut game = promote(f, linguistics, 0);
        let (status, _) = finish(&mut game, |menu| {
            option(menu, &Alternative::Value(2)).unwrap_or(0)
        });
        assert!(matches!(status, Status::Complete(_)), "{status:?}");
        let position = game.position();
        assert_eq!(position.cards(Zone::Hand(A)), [echo.unwrap_or(base)]);
        for card in twos {
            assert!(position.cards(Zone::Junk).contains(&card), "{echoes_two}");
        }
        assert_eq!(position.cards(Zone::AvailableAchievements), [three]);
    }
}

/// An achievement win at the Meld action's end comes before promotion, so nothing is
/// promoted.
#[test]
fn an_achievement_win_at_the_meld_actions_end_prevents_promotion() {
    let mut f = Fixture::default();
    for value in 1..=6 {
        tech(&mut f, Color::Red, value, Zone::Achievements(A));
    }
    // Five Forecast cards earn Destiny, A's seventh achievement.
    let forecast: Vec<_> = (0..5)
        .map(|_| tech(&mut f, Color::Green, 1, Zone::Forecast(A)))
        .collect();
    let melded = tech(&mut f, Color::Blue, 5, Zone::Hand(A));
    let mut game = Game::new(f.position(), E).unwrap();
    game.take_action(Action::Meld { card: melded }, ActionKind::Regular)
        .unwrap();
    let (status, _) = finish(&mut game, |menu| panic!("no promotion: {menu:?}"));
    assert_eq!(status, Status::Winner(A));
    let position = game.position();
    assert_eq!(
        position.special_location(Special::Destiny),
        SpecialLocation::Owned(A)
    );
    assert_eq!(position.cards(Zone::Forecast(A)), forecast);
    assert!(position.cards(Zone::Board(A, Color::Green)).is_empty());
}

/// A special first reached by the promotion meld is claimed at the end of the promoted
/// card's Dogma, with no check between that meld and the Dogma.
#[test]
fn a_special_reached_by_the_promotion_meld_waits_for_the_promoted_dogma() {
    let mut f = Fixture::default();
    // Seven visible bonuses before promotion; Wealth needs eight.
    let red = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    symbols(&mut f, red, [Symbol::Bonus(1); 4]);
    let yellow = tech(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow));
    symbols(
        &mut f,
        yellow,
        [
            Symbol::Bonus(1),
            Symbol::Bonus(1),
            Symbol::Bonus(1),
            Symbol::Blank,
        ],
    );
    let promoted = f.tech(Color::Green, 1, Zone::Forecast(A), vec![build::draw(1)]);
    symbols(
        &mut f,
        promoted,
        [
            Symbol::Bonus(1),
            Symbol::Icon(Icon::Authority),
            Symbol::Image,
            Symbol::Blank,
        ],
    );
    let reward = tech(&mut f, Color::Purple, 1, Zone::Deck(Deck::base(1)));
    let melded = tech(&mut f, Color::Blue, 5, Zone::Hand(A));
    let mut game = Game::new(f.position(), E).unwrap();
    game.take_action(Action::Meld { card: melded }, ActionKind::Regular)
        .unwrap();
    let (status, events) = finish(&mut game, |menu| pick_known(menu, promoted).unwrap_or(0));
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    assert_eq!(game.position().top(A, Color::Green), Some(promoted));
    let drew = events
        .iter()
        .position(|e| matches!(e, Event::Moved(m) if m.card == reward))
        .expect("the promoted Dogma drew");
    let claimed = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::SpecialMoved {
                    special: Special::Wealth,
                    destination: SpecialLocation::Owned(A),
                    ..
                }
            )
        })
        .expect("Wealth was claimed");
    assert!(drew < claimed);
}

/// Foreseen Ice Skates junks "the [1] deck and [2] deck": the Base decks. The Echoes
/// decks stay (old test
/// ice_skates_finishes_returns_before_draws_and_chooses_each_reward).
#[test]
fn foreseen_ice_skates_junks_only_the_base_one_and_two_decks() {
    let mut f = Fixture::default();
    let skates = f.add(echoes_card("E008").unwrap(), Zone::Junk);
    let base = [1, 1, 2].map(|value| in_deck(&mut f, CardSet::Base, Color::Red, value));
    let echoes = [1, 2].map(|value| in_deck(&mut f, CardSet::Echoes, Color::Red, value));
    let mut game = promote(f, skates, 0);
    let (status, _) = finish(&mut game, |_| 0);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    let position = game.position();
    for card in base {
        assert!(position.cards(Zone::Junk).contains(&card));
    }
    for (age, card) in [(1, echoes[0]), (2, echoes[1])] {
        let deck = Zone::Deck(Deck {
            set: CardSet::Echoes,
            age,
        });
        assert_eq!(position.cards(deck), [card]);
    }
}

/// Liquid Fire's victim with no bonus draws a card of value zero, a [1]; it goes to the
/// demander's Forecast, and only a red draw takes the victim's hand.
#[test]
fn liquid_fire_with_no_bonus_draws_a_one() {
    for red in [false, true] {
        let mut f = Fixture::default();
        let fire = f.add(echoes_card("E026").unwrap(), Zone::Junk);
        let held = tech(&mut f, Color::Purple, 6, Zone::Hand(B));
        let color = if red { Color::Red } else { Color::Yellow };
        let one = tech(&mut f, color, 1, Zone::Deck(Deck::base(1)));
        let two = tech(&mut f, Color::Green, 2, Zone::Deck(Deck::base(2)));
        let mut game = execute(f.position(), fire, 0, B);
        let (status, _) = finish(&mut game, |_| 0);
        assert!(matches!(status, Status::Complete(_)), "{status:?}");
        let position = game.position();
        assert_eq!(position.cards(Zone::Forecast(A)), [one]);
        assert_eq!(position.cards(Zone::Deck(Deck::base(2))), [two]);
        assert_eq!(position.cards(Zone::Hand(B)).contains(&held), !red);
        assert_eq!(position.cards(Zone::Score(A)).contains(&held), red);
    }
}

/// Foreseen Pressure Cooker melds every card from the hand (card text,
/// E048; old test pressure_cooker_foreseen_melds_the_entire_hand_in_chosen_order).
#[test]
fn foreseen_pressure_cooker_melds_the_whole_hand() {
    let mut f = Fixture::default();
    let cooker = f.add(echoes_card("E048").unwrap(), Zone::Junk);
    let reds = [
        tech(&mut f, Color::Red, 3, Zone::Hand(A)),
        tech(&mut f, Color::Red, 2, Zone::Hand(A)),
    ];
    let green = tech(&mut f, Color::Green, 4, Zone::Hand(A));
    let mut game = promote(f, cooker, 0);
    let (status, _) = finish(&mut game, |_| 0);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    let position = game.position();
    assert!(position.cards(Zone::Hand(A)).is_empty());
    let mut red_pile = position.cards(Zone::Board(A, Color::Red)).to_vec();
    red_pile.sort();
    assert_eq!(red_pile, reds);
    assert_eq!(position.top(A, Color::Green), Some(green));
}

/// Pressure Cooker's second effect with an empty hand: each top card with a
/// bonus gives a draw at the bonus value and junks an available achievement
/// of that value. Returns the available [2] and [3] achievements and the
/// card the draw should take.
fn pressure_cooker(covered_bonus: bool, two_in_deck: bool) -> (Game, [CardId; 2], CardId) {
    let mut f = Fixture::default();
    let cooker = f.add(echoes_card("E048").unwrap(), Zone::Junk);
    if covered_bonus {
        // An aslant splay shows the covered card's bonus 3, but the red top
        // card has none.
        let covered = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        symbols(
            &mut f,
            covered,
            [
                Symbol::Bonus(3),
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
            ],
        );
        tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
    }
    let green = tech(&mut f, Color::Green, 2, Zone::Board(A, Color::Green));
    symbols(
        &mut f,
        green,
        [
            Symbol::Bonus(2),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ],
    );
    let achievements =
        [2, 3].map(|value| tech(&mut f, Color::Blue, value, Zone::AvailableAchievements));
    let two = two_in_deck.then(|| tech(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2))));
    let three = tech(&mut f, Color::Yellow, 3, Zone::Deck(Deck::base(3)));
    let mut position = f.position();
    if covered_bonus {
        position = position.with_splay(A, Color::Red, Splay::Aslant).unwrap();
    }
    let mut game = execute(position, cooker, 1, A);
    let (status, _) = finish(&mut game, |_| 0);
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    (game, achievements, two.unwrap_or(three))
}

/// Pressure Cooker counts "each top card... with a bonus": a covered bonus shown by a
/// splay does not count (card text, E048; old test
/// pressure_cooker_uses_top_bonuses_and_prompts_for_junked_set).
#[test]
fn pressure_cooker_counts_only_top_card_bonuses() {
    let (game, [junked, kept], drawn) = pressure_cooker(true, true);
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), [drawn]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(3))).len(), 1);
    assert!(position.cards(Zone::Junk).contains(&junked));
    assert_eq!(position.cards(Zone::AvailableAchievements), [kept]);
}

/// Pressure Cooker's junk uses the bonus value even when the draw skips an
/// empty age: the two instructions are independent (old test
/// pressure_cooker_finishes_returns_then_draws_and_junks_independently).
#[test]
fn pressure_cooker_junks_at_the_bonus_value_after_its_draw_skips_an_age() {
    let (game, [junked, kept], drawn) = pressure_cooker(false, false);
    let position = game.position();
    assert_eq!(position.card(drawn).value, 3);
    assert_eq!(position.cards(Zone::Hand(A)), [drawn]);
    assert!(position.cards(Zone::Junk).contains(&junked));
    assert_eq!(position.cards(Zone::AvailableAchievements), [kept]);
}

/// Toilet: "You may return a card from your hand and draw a card of the
/// same value." Declining draws nothing (card text, E045).
#[test]
fn toilet_returns_a_hand_card_and_draws_one_of_its_value() {
    for accept in [true, false] {
        let mut f = Fixture::default();
        let toilet = f.add(echoes_card("E045").unwrap(), Zone::Junk);
        let returned = tech(&mut f, Color::Red, 3, Zone::Hand(A));
        let three = tech(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
        tech(&mut f, Color::Green, 4, Zone::Deck(Deck::base(4)));
        let mut game = execute(f.position(), toilet, 1, A);
        let (status, _) = finish(&mut game, |menu| {
            if accept {
                pick_known(menu, returned).unwrap()
            } else {
                option(menu, &Alternative::Decline).unwrap()
            }
        });
        assert!(matches!(status, Status::Complete(_)), "{status:?}");
        let position = game.position();
        let (hand, deck) = if accept {
            (three, returned)
        } else {
            (returned, three)
        };
        assert_eq!(position.cards(Zone::Hand(A)), [hand]);
        assert_eq!(position.cards(Zone::Deck(Deck::base(3))), [deck]);
        assert_eq!(position.cards(Zone::Deck(Deck::base(4))).len(), 1);
    }
}

/// Kobukson's echo may name a pile already splayed left, which changes nothing.
#[test]
fn kobukson_echo_may_choose_a_pile_already_splayed_left() {
    let mut f = Fixture::default();
    let kobukson = f.add(echoes_card("E036").unwrap(), Zone::Junk);
    for _ in 0..2 {
        tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        tech(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
    }
    let position = f.position().with_splay(A, Color::Red, Splay::Left).unwrap();
    let mut game = execute(position, kobukson, Card::ECHO, A);
    let left = |player, color| Alternative::Splay {
        player,
        color,
        direction: Splay::Left,
    };
    let mut offered = Vec::new();
    let (status, events) = finish(&mut game, |menu| {
        offered = menu.options.clone();
        option(menu, &left(A, Color::Red)).unwrap()
    });
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    assert!(offered.contains(&left(B, Color::Blue)));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
    assert_eq!(game.position().splay(B, Color::Blue), Splay::None);
    assert!(!events.iter().any(|e| matches!(e, Event::Splayed { .. })));
}

/// Blackmail on A's board: "Self-execute a card revealed due to this effect
/// of my choice, replacing 'may' with 'must'!"
fn blackmail(f: &mut Fixture) -> CardId {
    f.add(unseen_card("U040").unwrap(), Zone::Board(A, Color::Green))
}

/// A demands Blackmail of B, who melds and self-executes the card A picks.
fn blackmailed(f: Fixture, blackmail: CardId) -> Game {
    let mut game = Game::new(f.position(), UE).unwrap();
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    game.start(
        Operation::ExecuteEffect {
            card: blackmail,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    game
}

/// Play the blackmailed `card`'s effects for B. A move with `verb` must be
/// offered without a decline until `done`; then stopping is chosen.
fn moves_at_least_one(
    game: &mut Game,
    card: CardId,
    verb: Verb,
    color: Option<Color>,
    done: impl Fn(&Position) -> bool,
) {
    let mut events = Vec::new();
    let mut stopped = false;
    for _ in 0..200 {
        let menu = match game.advance(1000, &mut events).unwrap() {
            Status::Decision(menu) => menu,
            status => {
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                assert!(game.position().check_conservation());
                assert!(stopped, "a further {verb:?} was offered and declined");
                return;
            }
        };
        let decline = option(&menu, &Alternative::Decline);
        let moving = menu
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Move { verb: v, .. } if *v == verb));
        let chosen = color.and_then(|color| option(&menu, &Alternative::Colors(1 << color as u8)));
        let index = match (moving, chosen) {
            (Some(index), _) if !done(game.position()) => {
                assert_eq!(decline, None, "the first {verb:?} is required");
                index
            }
            (Some(_), _) => {
                stopped = true;
                decline.expect("stopping after one remains")
            }
            (None, Some(index)) => index,
            (None, None) => pick_known(&menu, card).unwrap_or(0),
        };
        game.choose(index).unwrap();
    }
    panic!("did not finish");
}

/// Blackmailed Soap must tuck at least one card of the chosen color, then may stop.
#[test]
fn soap_tucks_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = blackmail(&mut f);
    let soap = f.add(echoes_card("E006").unwrap(), Zone::Hand(B));
    tech(&mut f, Color::Red, 1, Zone::Hand(B));
    tech(&mut f, Color::Red, 2, Zone::Hand(B));
    let mut game = blackmailed(f, blackmail);
    moves_at_least_one(&mut game, soap, Verb::Tuck, Some(Color::Red), |position| {
        !position.cards(Zone::Board(B, Color::Red)).is_empty()
    });
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Red)).len(), 1);
    assert_eq!(position.cards(Zone::Hand(B)).len(), 1);
}

/// Blackmailed Lever must return at least one card, then may stop.
#[test]
fn lever_returns_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = blackmail(&mut f);
    let lever = f.add(echoes_card("E022").unwrap(), Zone::Hand(B));
    tech(&mut f, Color::Red, 5, Zone::Hand(B));
    tech(&mut f, Color::Green, 6, Zone::Hand(B));
    // Lever's echo draws two [2] first: four cards in hand.
    for _ in 0..2 {
        tech(&mut f, Color::Yellow, 2, Zone::Deck(Deck::base(2)));
    }
    let mut game = blackmailed(f, blackmail);
    moves_at_least_one(&mut game, lever, Verb::Return, None, |position| {
        position.cards(Zone::Hand(B)).len() < 4
    });
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 3);
}

/// Blackmailed Sandpaper must return at least one card, then may stop, and draws a [3]
/// for it.
#[test]
fn sandpaper_returns_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = blackmail(&mut f);
    let sandpaper = f.add(echoes_card("E029").unwrap(), Zone::Hand(B));
    tech(&mut f, Color::Red, 5, Zone::Hand(B));
    tech(&mut f, Color::Green, 6, Zone::Hand(B));
    tech(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
    let mut game = blackmailed(f, blackmail);
    moves_at_least_one(&mut game, sandpaper, Verb::Return, None, |position| {
        position.cards(Zone::Hand(B)).len() < 2
    });
    let position = game.position();
    let returned = position.cards(Zone::Deck(Deck::base(5))).len()
        + position.cards(Zone::Deck(Deck::base(6))).len();
    assert_eq!(returned, 1);
    assert!(position.cards(Zone::Deck(Deck::base(3))).is_empty());
    // The [3] and the kept card: one is melded, one stays.
    assert_eq!(position.cards(Zone::Hand(B)).len(), 1);
}

/// Blackmailed Scissors must choose at least one card, then may stop choosing.
#[test]
fn scissors_chooses_at_least_one_card_under_blackmail() {
    let mut f = Fixture::default();
    let blackmail = blackmail(&mut f);
    let scissors = f.add(echoes_card("E021").unwrap(), Zone::Hand(B));
    let red = tech(&mut f, Color::Red, 1, Zone::Hand(B));
    let blue = tech(&mut f, Color::Blue, 1, Zone::Hand(B));
    let mut game = blackmailed(f, blackmail);
    let mut choices = 0;
    let (status, _) = finish(&mut game, |menu| {
        if menu.player == A {
            return pick_known(menu, scissors).unwrap();
        }
        if let Some(index) = pick_known(menu, red) {
            assert_eq!(option(menu, &Alternative::Decline), None);
            choices += 1;
            return index;
        }
        if pick_known(menu, blue).is_some() {
            choices += 1;
            return option(menu, &Alternative::Decline).expect("may stop after one");
        }
        // Meld the chosen card.
        option(menu, &Alternative::Branch(0)).unwrap_or(0)
    });
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
    assert_eq!(choices, 2);
    let position = game.position();
    assert_eq!(position.top(B, Color::Red), Some(red));
    assert_eq!(position.cards(Zone::Hand(B)), [blue]);
}
