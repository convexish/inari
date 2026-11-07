//! Authored Echoes cards and the slots they need (port step 3): the first
//! declarations prove the scheduler on real cards, and slots on packets of
//! drawn cards.
use super::*;
use program::build::{self, HAND, filtered};

pub(super) fn card(code: &str) -> Card {
    echoes_card(code).unwrap()
}

/// Advance to the end, answering each decision with `pick`; whether A won.
pub(super) fn settle(game: &mut Game, mut pick: impl FnMut(&Decision) -> usize) -> bool {
    let mut events = Vec::new();
    let won = loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(decision) => {
                let choice = pick(&decision);
                game.choose(choice).unwrap();
            }
            Status::Complete(_) => break false,
            Status::Winner(winner) => break winner == A,
            status => panic!("unexpected {status:?}"),
        }
    };
    assert!(game.position().check_conservation());
    won
}

/// A runs one effect of `card`.
pub(super) fn run_effect(
    layout: Layout,
    expansions: Expansions,
    card: CardId,
    effect: u16,
    pick: impl FnMut(&Decision) -> usize,
) -> Game {
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, pick);
    game
}

/// The cards a menu names, in option order.
pub(super) fn offered(decision: &Decision) -> Vec<CardId> {
    decision
        .options
        .iter()
        .filter_map(|option| match option {
            Alternative::Select { target }
            | Alternative::Move { target, .. }
            | Alternative::Reveal { target } => match *target {
                TargetDescriptor::Known { card, .. } => Some(card),
                TargetDescriptor::Hidden { .. } => None,
            },
            _ => None,
        })
        .collect()
}

/// A blank card of this color and value in a player's hand.
pub(super) fn in_hand(layout: &mut Layout, player: Player, color: Color, value: u8) -> CardId {
    let mut card = printed(color, [Symbol::Blank; 4]);
    card.value = value;
    layout.add(card, Zone::Hand(player))
}

pub(super) fn deck_card(layout: &mut Layout, set: CardSet, color: Color, age: u8) -> CardId {
    let mut card = printed(color, [Symbol::Blank; 4]);
    card.set = set;
    card.value = age;
    layout.add(card, Zone::Deck(Deck { set, age }))
}

#[test]
fn authored_echoes_cards_match_their_printed_echoes() {
    for code in ["E001", "E012", "E013", "E024", "E083"] {
        let card = card(code);
        assert_eq!(card.set, CardSet::Echoes);
        let printed = card.symbols.contains(&Symbol::Echo);
        assert_eq!(card.echo.is_some(), printed, "{code}");
        let metadata = echoes_metadata(code).unwrap();
        assert_eq!(
            card.effects.len(),
            echoes_effect_count(metadata.code),
            "{code}"
        );
    }
}

/// Printed dogma effect counts per card.
fn echoes_effect_count(code: &str) -> usize {
    match code {
        "E001" | "E013" => 2,
        "E012" | "E024" => 1,
        "E083" => 3,
        _ => unreachable!("not authored"),
    }
}

#[test]
fn ruler_chooses_between_its_own_two_draws() {
    for full in [false, true] {
        let mut layout = Layout::default();
        let ruler = layout.add(card("E012"), Zone::Board(A, Color::Blue));
        // The same back as the draws, already in hand: never one of them.
        let held = deck_card(&mut layout, CardSet::Echoes, Color::Red, 1);
        layout.placements.last_mut().unwrap().1 = Zone::Hand(A);
        let drawn = [
            deck_card(&mut layout, CardSet::Echoes, Color::Green, 1),
            deck_card(&mut layout, CardSet::Echoes, Color::Yellow, 1),
        ];
        if full {
            layout.forecast(A, 5);
        }
        // The age comes from the Base decks; the set is Echoes.
        deck_card(&mut layout, CardSet::Base, Color::Blue, 1);
        let mut menu = Vec::new();
        let game = run_effect(layout, Expansions::ECHOES, ruler, 0, |decision| {
            menu = offered(decision);
            menu.len() - 1
        });
        let mut sorted = menu.clone();
        sorted.sort();
        assert_eq!(sorted, drawn, "only the two drawn cards are offered");
        let (chosen, other) = (menu[1], menu[0]);
        let position = game.position();
        let deck = Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 1,
        });
        // The other card goes back even when the Forecast refuses the chosen one.
        assert_eq!(position.cards(deck), [other]);
        assert_eq!(position.cards(Zone::Forecast(A)).contains(&chosen), !full);
        let hand = position.cards(Zone::Hand(A));
        assert_eq!(hand, if full { vec![held, chosen] } else { vec![held] });
    }
}

#[test]
fn pagoda_melds_its_drawn_card_after_a_matching_tuck() {
    #[derive(Clone, Copy, PartialEq)]
    enum Case {
        Tuck,
        Decline,
        FullForecast,
    }
    for case in [Case::Tuck, Case::Decline, Case::FullForecast] {
        let mut layout = Layout::default();
        let pagoda = layout.add(card("E024"), Zone::Board(A, Color::Purple));
        let green = in_hand(&mut layout, A, Color::Green, 1);
        let red = in_hand(&mut layout, A, Color::Red, 1);
        let drawn = deck_card(&mut layout, CardSet::Base, Color::Green, 3);
        if case == Case::FullForecast {
            layout.forecast(A, 5);
        }
        let mut menu = Vec::new();
        let game = run_effect(layout, Expansions::ECHOES, pagoda, 0, |decision| {
            menu = offered(decision);
            if case == Case::Decline {
                decision.options.len() - 1
            } else {
                0
            }
        });
        // Only another card of the drawn card's color, wherever that is.
        assert_eq!(menu, [green]);
        let position = game.position();
        assert!(position.cards(Zone::Hand(A)).contains(&red));
        let board = position.cards(Zone::Board(A, Color::Green));
        match case {
            Case::Decline => {
                assert!(board.is_empty());
                assert!(position.cards(Zone::Forecast(A)).contains(&drawn));
            }
            Case::Tuck | Case::FullForecast => {
                // From the Forecast, or from hand when it was full.
                assert_eq!(board, [green, drawn]);
                assert!(!position.cards(Zone::Forecast(A)).contains(&drawn));
            }
        }
    }
}

#[test]
fn foreseen_pagoda_melds_the_color_from_other_boards() {
    let mut layout = Layout::default();
    let melded = layout.add(dogma_card(Color::Red, 5, vec![Vec::new()]), Zone::Hand(A));
    layout.add(card("E024"), Zone::Forecast(A));
    let green = in_hand(&mut layout, A, Color::Green, 1);
    let theirs = [
        top(&mut layout, B, Color::Green, 4),
        top(&mut layout, B, Color::Green, 6),
    ];
    let drawn = deck_card(&mut layout, CardSet::Base, Color::Green, 3);
    let (game, _) = meld_action(layout, Expansions::ECHOES, melded);
    let position = game.position();
    assert!(position.cards(Zone::Board(B, Color::Green)).is_empty());
    let board = position.cards(Zone::Board(A, Color::Green));
    assert_eq!(&board[..2], [green, drawn]);
    let mut melded_theirs = board[2..].to_vec();
    melded_theirs.sort();
    assert_eq!(melded_theirs, theirs);
}

#[test]
fn x_ray_foreshadows_only_its_own_draws() {
    // Marie Curie draws a [9] before a draw of a value not in hand: that
    // card is the karma's, not one X-Ray drew.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    layout.figure(Figure::MarieCurie, A);
    let x_ray = layout.add(card("E083"), Zone::Hand(A));
    for color in [Color::Red, Color::Yellow] {
        layout.add(
            printed(color, [Symbol::Icon(Icon::Health); 4]),
            Zone::Board(A, color),
        );
    }
    let held = in_hand(&mut layout, A, Color::Green, 4);
    let ones = [
        deck_card(&mut layout, CardSet::Base, Color::Green, 1),
        deck_card(&mut layout, CardSet::Base, Color::Purple, 1),
    ];
    let nine = deck_card(&mut layout, CardSet::Base, Color::Blue, 9);
    let mut menus = Vec::new();
    // Choose the value 1, then foreshadow each offered card in turn.
    let game = run_effect(layout, both, x_ray, 0, |decision| {
        menus.push(offered(decision));
        0
    });
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), [x_ray, held, nine]);
    let mut forecast = position.cards(Zone::Forecast(A)).to_vec();
    forecast.sort();
    assert_eq!(forecast, ones);
    // The foreshadow menus offered only the drawn cards (and stopping).
    let foreshadows: Vec<_> = menus.into_iter().filter(|menu| !menu.is_empty()).collect();
    assert_eq!(foreshadows.len(), 2);
    assert!(foreshadows.iter().flatten().all(|card| ones.contains(card)));
}

#[test]
fn bangle_branches_then_draws_into_an_empty_forecast() {
    let mut layout = Layout::default();
    let bangle = layout.add(card("E001"), Zone::Board(A, Color::Red));
    let two = layout.add(
        dogma_card(Color::Blue, 2, vec![Vec::new()]),
        Zone::Forecast(A),
    );
    // The first effect: tuck the [2] from the Forecast.
    let game = run_effect(layout, Expansions::ECHOES, bangle, 0, |decision| {
        assert_eq!(decision.options.len(), 2, "a mandatory choice");
        1
    });
    assert_eq!(game.position().cards(Zone::Board(A, Color::Blue)), [two]);
    // The second, with the Forecast now empty, draws and foreshadows a [3].
    let mut layout = Layout::default();
    let bangle = layout.add(card("E001"), Zone::Board(A, Color::Red));
    let three_again = deck_card(&mut layout, CardSet::Base, Color::Yellow, 3);
    let game = run_effect(layout, Expansions::ECHOES, bangle, 1, |_| 0);
    assert_eq!(game.position().cards(Zone::Forecast(A)), [three_again]);
}

#[test]
fn bangle_echo_runs_before_its_dogmas() {
    let mut layout = Layout::default();
    let bangle = layout.add(card("E001"), Zone::Board(A, Color::Red));
    let one = in_hand(&mut layout, A, Color::Yellow, 1);
    let two = deck_card(&mut layout, CardSet::Base, Color::Green, 2);
    let (game, _) = dogma_draws(layout, Expansions::ECHOES, bangle);
    let position = game.position();
    // Echo: tuck the [1]. First dogma: draw and foreshadow a [2]. Second:
    // the Forecast isn't empty, so nothing.
    assert_eq!(position.cards(Zone::Board(A, Color::Yellow)), [one]);
    assert_eq!(position.cards(Zone::Forecast(A)), [two]);
}

#[test]
fn foreseen_bell_returns_every_hand() {
    for promoted in [true, false] {
        let mut layout = Layout::default();
        let bell = card("E013");
        let melded = layout.add(dogma_card(Color::Red, 5, vec![Vec::new()]), Zone::Hand(A));
        let bell = if promoted {
            layout.add(bell, Zone::Forecast(A))
        } else {
            layout.add(bell, Zone::Board(A, Color::Purple))
        };
        let theirs = in_hand(&mut layout, B, Color::Green, 3);
        let mine = in_hand(&mut layout, A, Color::Blue, 3);
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 1);
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 2);
        let game = if promoted {
            meld_action(layout, Expansions::ECHOES, melded).0
        } else {
            dogma_draws(layout, Expansions::ECHOES, bell).0
        };
        let position = game.position();
        assert_eq!(
            position.cards(Zone::Forecast(A)).len(),
            2,
            "two foreshadowed"
        );
        // The echo scored A's first hand card.
        let scored = position.cards(Zone::Score(A));
        assert_eq!(scored.len(), 1);
        assert_eq!(
            position.cards(Zone::Hand(B)).is_empty(),
            promoted,
            "{promoted}"
        );
        if promoted {
            assert!(position.cards(Zone::Hand(A)).is_empty());
        } else {
            assert!(position.cards(Zone::Hand(B)).contains(&theirs));
        }
        let _ = mine;
    }
}

const S: Slot = Slot::effect(0);
const T: Slot = Slot::effect(1);
const UNUSED: Slot = Slot::effect(2);

fn slot_program(layout: &mut Layout, instructions: Vec<Instruction>) -> CardId {
    program(layout, instructions)
}

#[test]
fn keep_replaces_and_add_appends() {
    let mut layout = Layout::default();
    let held = in_hand(&mut layout, A, Color::Red, 1);
    let card = slot_program(
        &mut layout,
        vec![
            build::draw(1).keep(S),
            build::draw(1).keep(S),
            build::draw(1).add(S),
            build::foreshadow(filtered(&HAND, &[Filter::Has(Test::InSlot(S))])).all(),
        ],
    );
    let drawn: Vec<_> = (0..3)
        .map(|_| deck_card(&mut layout, CardSet::Base, Color::Green, 1))
        .collect();
    let (game, _) = run_program(layout, card);
    let position = game.position();
    // Draws take the deck's last card first: the second and third draws
    // stay recorded; the first was replaced.
    let mut forecast = position.cards(Zone::Forecast(A)).to_vec();
    forecast.sort();
    assert_eq!(forecast, [drawn[0], drawn[1]]);
    assert_eq!(position.cards(Zone::Hand(A)), [held, drawn[2]]);
}

#[test]
fn choices_record_values_and_colors() {
    let mut layout = Layout::default();
    let card = slot_program(
        &mut layout,
        vec![
            build::choose(Who::Executor, Choice::Value(ValueChoices::Any)).keep(S),
            build::choose(Who::Executor, Choice::Colors(1)).keep(T),
            // Choosing again changes the register, not the slots.
            build::choose(Who::Executor, Choice::Value(ValueChoices::Any)),
            build::draw_age(Expr::Slot(S)),
            build::if_(&Cond::Not(&Cond::Empty(UNUSED)), 1),
            Instruction::Win(WinRule::Executor),
            build::splay_set(ColorSet::Slot(T), Splay::Left),
        ],
    );
    let two = deck_card(&mut layout, CardSet::Base, Color::Green, 2);
    layout.pile(A, Color::Blue, 2, [Symbol::Blank; 4]);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let mut answers = vec![
        1, // value 2
        1, // blue: single colors in mask order, red first
        5, // the register's value, unused
    ]
    .into_iter();
    let won = settle(&mut game, |_| answers.next().unwrap());
    assert!(!won, "an unused slot is empty");
    assert_eq!(game.position().cards(Zone::Hand(A)), [two]);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Left);
}

#[test]
fn repeat_starts_with_empty_slots() {
    // Wins only if a slot survives the repeat; draws until two in hand.
    let mut layout = Layout::default();
    let card = slot_program(
        &mut layout,
        vec![
            build::if_(&Cond::Not(&Cond::Empty(S)), 1),
            Instruction::Win(WinRule::Executor),
            build::if_(
                &Cond::Compare(Expr::Count(&HAND), Cmp::Less, Expr::Const(2)),
                2,
            ),
            build::draw(1).keep(S),
            Instruction::Repeat { keep: 0 },
        ],
    );
    for _ in 0..2 {
        deck_card(&mut layout, CardSet::Base, Color::Green, 1);
    }
    let (game, won) = run_program(layout, card);
    assert!(!won);
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
}

/// A draws a [1] into a slot and then a [2], so only the slot remembers the
/// [1]; B makes a choice; A foreshadows the recorded card. B has seen none of
/// the [1]s, in A's hand or the deck.
fn hidden_packet() -> Game {
    let mut layout = Layout::default();
    for color in [Color::Red, Color::Yellow] {
        in_hand(&mut layout, A, color, 1);
    }
    let card = slot_program(
        &mut layout,
        vec![
            build::draw(1).keep(S),
            build::draw(2),
            build::choose(Who::Opponent, Choice::Value(ValueChoices::Any)),
            build::foreshadow(filtered(&HAND, &[Filter::Has(Test::InSlot(S))])),
        ],
    );
    deck_card(&mut layout, CardSet::Base, Color::Blue, 2);
    for color in [Color::Red, Color::Yellow, Color::Purple, Color::Green] {
        deck_card(&mut layout, CardSet::Base, color, 1);
    }
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let Status::Decision(decision) = game.advance(1000, &mut Vec::new()).unwrap() else {
        panic!("B's choice");
    };
    assert_eq!(decision.player, B);
    game
}

#[test]
fn recorded_cards_survive_cloning_and_hidden_sampling() {
    let game = hidden_packet();
    // A clone resumes exactly like the original.
    let mut original = game.clone();
    let mut clone = game.clone();
    settle(&mut original, |_| 0);
    settle(&mut clone, |_| 0);
    assert_eq!(original, clone);
    let drawn = original.position().cards(Zone::Forecast(A))[0];
    assert_eq!(original.position().card(drawn).color, Color::Green);
    // B can't tell which [1] A drew: each sampled world follows the recorded
    // card's pick to a [1] in A's hand, never the real card wherever the
    // sample put it.
    for seed in 0..32 {
        let mut world = game.determinize(B, seed).unwrap();
        settle(&mut world, |_| 0);
        let position = world.position();
        assert_eq!(position.cards(Zone::Forecast(A)).len(), 1, "seed {seed}");
        assert_eq!(position.cards(Zone::Hand(A)).len(), 3);
        let foreshadowed = position.card(position.cards(Zone::Forecast(A))[0]);
        assert_eq!((foreshadowed.set, foreshadowed.value), (CardSet::Base, 1));
    }
}

#[test]
fn authored_block_cards_match_their_printed_echoes() {
    for (code, dogmas) in [
        ("E008", 2),
        ("E022", 1),
        ("E090", 2),
        ("E098", 3),
        ("E109", 1),
        ("E113", 1),
    ] {
        let card = card(code);
        assert_eq!(
            card.echo.is_some(),
            card.symbols.contains(&Symbol::Echo),
            "{code}"
        );
        assert_eq!(card.effects.len(), dogmas, "{code}");
    }
}

#[test]
fn ice_skates_rewards_each_returned_card() {
    for returns in [0, 2] {
        let mut layout = Layout::default();
        let skates = layout.add(card("E008"), Zone::Board(A, Color::Green));
        for color in [Color::Red, Color::Blue, Color::Yellow] {
            in_hand(&mut layout, A, color, 1);
        }
        let two = deck_card(&mut layout, CardSet::Base, Color::Purple, 2);
        let three = deck_card(&mut layout, CardSet::Base, Color::Red, 3);
        let mut returned = 0;
        let mut branches = 0;
        let game = run_effect(layout, Expansions::ECHOES, skates, 1, |decision| {
            if matches!(decision.options[0], Alternative::Branch(_)) {
                // The first reward melds, the second foreshadows.
                branches += 1;
                branches - 1
            } else if returned < returns {
                returned += 1;
                0
            } else {
                decision.options.len() - 1
            }
        });
        let position = game.position();
        assert_eq!(branches, returns);
        assert_eq!(position.cards(Zone::Hand(A)).len(), 3 - returns);
        if returns == 2 {
            assert_eq!(position.top(A, Color::Purple), Some(two));
            assert_eq!(position.cards(Zone::Forecast(A)), [three]);
        } else {
            assert!(position.cards(Zone::Forecast(A)).is_empty());
        }
    }
}

#[test]
fn esports_draws_in_ascending_order_and_wins_only_when_foreseen() {
    for promoted in [false, true] {
        let mut layout = Layout::default();
        let esports = card("E109");
        top(&mut layout, A, Color::Red, 3);
        top(&mut layout, A, Color::Blue, 1);
        top(&mut layout, A, Color::Purple, 2);
        let drawn: Vec<_> = (1..=3)
            .map(|age| deck_card(&mut layout, CardSet::Base, Color::Green, age))
            .collect();
        let (game, won) = if promoted {
            let melded = layout.add(
                dogma_card(Color::Green, 11, vec![Vec::new()]),
                Zone::Hand(A),
            );
            layout.add(esports, Zone::Forecast(A));
            let eleven = deck_card(&mut layout, CardSet::Base, Color::Red, 11);
            let (game, won) = meld_action(layout, Expansions::ECHOES, melded);
            assert!(game.position().cards(Zone::Score(A)).contains(&eleven));
            (game, won)
        } else {
            let esports = layout.add(esports, Zone::Board(A, Color::Yellow));
            let mut game =
                Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
            game.take_action(
                Action::Dogma {
                    card: esports,
                    auspice: false,
                },
                ActionKind::Regular,
            )
            .unwrap();
            let won = settle(&mut game, |_| 0);
            (game, won)
        };
        assert_eq!(won, promoted);
        // Scored lowest first: 1, 2, 3 (then the promoting [11]).
        assert_eq!(&game.position().cards(Zone::Score(A))[..3], drawn);
    }
}

#[test]
fn lever_draws_once_per_returned_pair_in_the_players_order() {
    let mut layout = Layout::default();
    let lever = layout.add(card("E022"), Zone::Board(A, Color::Blue));
    for (color, value) in [
        (Color::Red, 2),
        (Color::Green, 2),
        (Color::Red, 3),
        (Color::Green, 3),
        (Color::Yellow, 3),
        (Color::Purple, 4),
    ] {
        in_hand(&mut layout, A, color, value);
    }
    let three = deck_card(&mut layout, CardSet::Base, Color::Blue, 3);
    let four = deck_card(&mut layout, CardSet::Base, Color::Blue, 4);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: lever,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    // Return the whole hand, one card at a time.
    let reward_order = loop {
        let Status::Decision(decision) = game.advance(1000, &mut Vec::new()).unwrap() else {
            panic!("a decision");
        };
        if let Alternative::Value(_) = decision.options[0] {
            break decision;
        }
        game.choose(0).unwrap();
    };
    // The player orders the rewards for each returned value.
    let values = [2, 3, 4].map(Alternative::Value);
    assert_eq!(reward_order.options, values);
    let mut clone = game.clone();
    for game in [&mut game, &mut clone] {
        settle(game, |decision| decision.options.len() - 1);
    }
    assert_eq!(game, clone);
    // Two 2s and three 3s each make one pair; a lone 4 makes none.
    let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
    hand.sort();
    assert_eq!(hand, [three, four]);
}

#[test]
fn cell_phone_splays_each_tucked_color_before_the_next_choice() {
    let mut layout = Layout::default();
    let phone = layout.add(card("E098"), Zone::Board(A, Color::Yellow));
    let efficient = [Symbol::Icon(Icon::Efficiency); 4];
    let red = layout.add(printed(Color::Red, efficient), Zone::Hand(A));
    let blue = layout.add(printed(Color::Blue, efficient), Zone::Hand(A));
    in_hand(&mut layout, A, Color::Green, 1);
    layout.pile(A, Color::Red, 1, [Symbol::Blank; 4]);
    layout.pile(A, Color::Blue, 1, [Symbol::Blank; 4]);
    let mut menus = Vec::new();
    let game = run_effect(layout, Expansions::ECHOES, phone, 2, |decision| {
        menus.push(offered(decision));
        // Tuck the red card, then stop.
        if menus.len() == 1 {
            0
        } else {
            decision.options.len() - 1
        }
    });
    assert_eq!(menus, [vec![red, blue], vec![blue]]);
    let position = game.position();
    assert_eq!(position.splay(A, Color::Red), Splay::Up);
    assert_eq!(position.splay(A, Color::Blue), Splay::None);
    assert!(position.cards(Zone::Hand(A)).contains(&blue));
}

#[test]
fn touchscreen_wins_when_it_splays_five_colors() {
    for already in [false, true] {
        let mut layout = Layout::default();
        let screen = layout.add(card("E113"), Zone::Hand(A));
        for (color, size) in [
            (Color::Red, 2),
            (Color::Blue, 4),
            (Color::Green, 6),
            (Color::Yellow, 8),
            (Color::Purple, 3),
        ] {
            layout.pile(A, color, size, [Symbol::Blank; 4]);
        }
        if already {
            layout.splay(A, Color::Purple, Splay::Aslant);
        }
        let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: screen,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        let won = settle(&mut game, |_| 0);
        let position = game.position();
        for (color, splay) in [
            (Color::Red, Splay::Aslant),
            (Color::Blue, Splay::Up),
            (Color::Green, Splay::Right),
            (Color::Yellow, Splay::Left),
            (Color::Purple, Splay::Aslant),
        ] {
            assert_eq!(position.splay(A, color), splay);
        }
        // An unchanged splay isn't one of the five.
        assert_eq!(won, !already);
    }
}

#[test]
fn credit_card_repeats_only_when_foreseen() {
    for promoted in [false, true] {
        let mut layout = Layout::default();
        let credit = card("E090");
        let blue = top(&mut layout, A, Color::Blue, 2);
        let two = deck_card(&mut layout, CardSet::Base, Color::Yellow, 2);
        let nines = [
            deck_card(&mut layout, CardSet::Base, Color::Yellow, 9),
            deck_card(&mut layout, CardSet::Base, Color::Yellow, 9),
        ];
        let game = if promoted {
            let red = layout.add(dogma_card(Color::Red, 9, vec![Vec::new()]), Zone::Hand(A));
            layout.add(credit, Zone::Forecast(A));
            let (game, _) = meld_action(layout, Expansions::ECHOES, red);
            // The echo foreshadowed a [9]; the dogma took both non-green tops
            // in turn, scoring a card of each one's value.
            assert_eq!(game.position().cards(Zone::Hand(A)), [red, blue]);
            assert_eq!(game.position().cards(Zone::Score(A)), [nines[0], two]);
            game
        } else {
            let credit = layout.add(credit, Zone::Hand(A));
            let game = run_effect(layout, Expansions::ECHOES, credit, 0, |_| 0);
            assert_eq!(game.position().cards(Zone::Score(A)), [two]);
            game
        };
        assert!(game.position().check_conservation());
    }
}

/// A program that runs `body` as a block over `domain`, then wins if the
/// block did.
fn block_program(domain: Domain, body: Vec<Instruction>) -> Vec<Instruction> {
    let mut program = vec![build::for_each(
        domain,
        Order::Printed,
        None,
        body.len() as u8,
    )];
    program.extend(body);
    program.extend([
        build::if_(build::DID, 1),
        Instruction::Win(WinRule::Executor),
    ]);
    program
}

#[test]
fn a_block_did_only_if_every_pass_fully_happened() {
    const ONES: Selector = filtered(&HAND, &[Filter::Has(Test::Values(&[1]))]);
    for (passes, ones, wins) in [(0, 2, false), (2, 2, true), (2, 1, false)] {
        let mut layout = Layout::default();
        for _ in 0..ones {
            in_hand(&mut layout, A, Color::Red, 1);
        }
        let card = slot_program(
            &mut layout,
            block_program(
                Domain::Times(Expr::Const(passes)),
                vec![build::return_(ONES)],
            ),
        );
        let (_, won) = run_program(layout, card);
        assert_eq!(won, wins, "{passes} passes, {ones} ones");
    }
}

#[test]
fn nested_blocks_count_every_operation() {
    const FOUR: Cond = Cond::Compare(Expr::Done, Cmp::Equal, Expr::Const(4));
    let mut layout = Layout::default();
    let card = slot_program(
        &mut layout,
        vec![
            build::for_each(Domain::Times(Expr::Const(2)), Order::Printed, None, 2),
            build::for_each(Domain::Times(Expr::Const(2)), Order::Printed, None, 1),
            build::draw(1),
            build::if_(&FOUR, 1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    for _ in 0..4 {
        deck_card(&mut layout, CardSet::Base, Color::Green, 1);
    }
    let (game, won) = run_program(layout, card);
    assert!(won);
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 4);
}

#[test]
fn a_jump_out_of_a_block_is_refused() {
    let mut layout = Layout::default();
    let card = slot_program(
        &mut layout,
        vec![
            build::once(1),
            Instruction::Jump(1),
            Instruction::Win(WinRule::Executor),
        ],
    );
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    assert!(matches!(
        game.advance(1000, &mut Vec::new()),
        Err(Error::Unsupported("a jump out of a block"))
    ));
}

/// A's Dogma action on `card`, answering each decision with `pick`.
pub(super) fn dogma(
    layout: Layout,
    expansions: Expansions,
    card: CardId,
    pick: impl FnMut(&Decision) -> usize,
) -> (Game, bool) {
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let won = settle(&mut game, pick);
    (game, won)
}

#[test]
fn authored_memory_cards_match_their_printed_echoes() {
    for (code, dogmas) in [
        ("E030", 1),
        ("E059", 3),
        ("E063", 2),
        ("E087", 2),
        ("E092", 2),
        ("E094", 2),
        ("E112", 1),
    ] {
        let card = card(code);
        assert_eq!(
            card.echo.is_some(),
            card.symbols.contains(&Symbol::Echo),
            "{code}"
        );
        assert_eq!(card.effects.len(), dogmas, "{code}");
    }
}

#[test]
fn morphine_draws_above_the_highest_card_its_demand_returned() {
    for odd in [vec![3, 5], vec![]] {
        let mut layout = Layout::default();
        let morphine = layout.add(card("E059"), Zone::Board(A, Color::Yellow));
        let mut theirs = Vec::new();
        for &value in odd.iter().chain(&[4]) {
            theirs.push(in_hand(&mut layout, B, Color::Red, value));
        }
        let draws: Vec<_> = [1, 6, 6]
            .iter()
            .map(|&age| deck_card(&mut layout, CardSet::Base, Color::Green, age))
            .collect();
        let (game, _) = dogma(layout, Expansions::ECHOES, morphine, |_| 0);
        let position = game.position();
        // B returned its odd cards and drew a [6] (the deck's top one); A
        // drew one above the highest returned, or a [1] when nothing was.
        let expected = if odd.is_empty() { draws[0] } else { draws[1] };
        assert!(position.cards(Zone::Hand(A)).contains(&expected), "{odd:?}");
        assert_eq!(position.cards(Zone::Hand(B)).len(), 2);
        assert!(
            position
                .cards(Zone::Hand(B))
                .contains(theirs.last().unwrap())
        );
    }
}

#[test]
fn charitable_trust_melds_its_echo_draw() {
    for (pick, value) in [(0, 3), (1, 4)] {
        let mut layout = Layout::default();
        let trust = layout.add(card("E030"), Zone::Board(A, Color::Green));
        // A card of the drawn value already in hand is never melded.
        let held = in_hand(&mut layout, A, Color::Purple, value);
        // Fifteen points: eligible for a [3].
        for _ in 0..3 {
            layout.add(
                {
                    let mut card = printed(Color::Blue, [Symbol::Blank; 4]);
                    card.value = 5;
                    card
                },
                Zone::Score(A),
            );
        }
        let drawn = deck_card(&mut layout, CardSet::Base, Color::Red, value);
        let mut menus = Vec::new();
        let (game, _) = dogma(layout, Expansions::ECHOES, trust, |decision| {
            menus.push(offered(decision));
            if matches!(decision.options[0], Alternative::Value(_)) {
                pick
            } else {
                0
            }
        });
        let position = game.position();
        assert_eq!(position.top(A, Color::Red), Some(drawn));
        assert!(position.cards(Zone::Hand(A)).contains(&held));
        assert!(menus.iter().all(|menu| !menu.contains(&held)));
        // A [3] achieves the top green card (Charitable Trust itself); a [4]
        // returns it.
        assert_eq!(
            position.cards(Zone::Achievements(A)).contains(&trust),
            value == 3
        );
        assert_eq!(position.top(A, Color::Green), None);
    }
}

#[test]
fn stethoscope_remembers_a_blue_echo_meld() {
    for (color, promoted, drawn) in [
        (Color::Blue, false, vec![7, 8]),
        (Color::Yellow, false, vec![7]),
        (Color::Blue, true, vec![7, 8, 9]),
    ] {
        let mut layout = Layout::default();
        let stethoscope = card("E063");
        let melded = in_hand(&mut layout, A, color, 1);
        ages(&mut layout, 7..=9);
        let game = if promoted {
            let action = layout.add(dogma_card(Color::Red, 6, vec![Vec::new()]), Zone::Hand(A));
            layout.add(stethoscope, Zone::Forecast(A));
            meld_action(layout, Expansions::ECHOES, action).0
        } else {
            let stethoscope = layout.add(stethoscope, Zone::Board(A, Color::Blue));
            dogma(layout, Expansions::ECHOES, stethoscope, |_| 0).0
        };
        let position = game.position();
        assert_eq!(position.top(A, color), Some(melded));
        let mut hand: Vec<_> = position
            .cards(Zone::Hand(A))
            .iter()
            .map(|&card| position.card(card).value)
            .collect();
        hand.sort();
        assert_eq!(hand, drawn, "{color:?} promoted {promoted}");
    }
}

#[test]
fn jets_demand_reads_the_demanders_echo_meld() {
    let mut layout = Layout::default();
    let jet = layout.add(card("E087"), Zone::Board(A, Color::Red));
    let melded = in_hand(&mut layout, A, Color::Purple, 4);
    top(&mut layout, B, Color::Purple, 1);
    let returned = top(&mut layout, B, Color::Purple, 2);
    let achievements: Vec<_> = [2, 4, 5]
        .iter()
        .map(|&value| {
            let mut card = printed(Color::Green, [Symbol::Blank; 4]);
            card.value = value;
            layout.add(card, Zone::AvailableAchievements)
        })
        .collect();
    ages(&mut layout, 10..=10);
    let (game, _) = dogma(layout, Expansions::ECHOES, jet, |_| 0);
    let position = game.position();
    assert_eq!(position.top(A, Color::Purple), Some(melded));
    // B returned its top purple card, the color A melded; the [2] and [4]
    // achievements are junked, the [5] stays.
    assert!(
        !position
            .cards(Zone::Board(B, Color::Purple))
            .contains(&returned)
    );
    assert_eq!(
        position.cards(Zone::AvailableAchievements),
        [achievements[2]]
    );
    let mut junked = position.cards(Zone::Junk).to_vec();
    junked.sort();
    assert_eq!(junked, [achievements[0], achievements[1]]);
}

#[test]
fn karaoke_self_executes_its_echo_meld_after_claiming() {
    let mut layout = Layout::default();
    let karaoke = layout.add(card("E094"), Zone::Board(A, Color::Purple));
    top(&mut layout, A, Color::Red, 1);
    in_hand(&mut layout, A, Color::Green, 2);
    // The echo draws a [2] and melds it: a red card that draws a [1].
    let mut red = dogma_card(Color::Red, 2, vec![vec![draw_box(1)]]);
    red.featured = None;
    let drawn = layout.add(
        red,
        Zone::Deck(Deck {
            set: CardSet::Base,
            age: 2,
        }),
    );
    let mut claim = printed(Color::Blue, [Symbol::Blank; 4]);
    claim.value = 2;
    let claim = layout.add(claim, Zone::AvailableAchievements);
    let one = deck_card(&mut layout, CardSet::Base, Color::Yellow, 1);
    let (game, _) = dogma(layout, Expansions::ECHOES, karaoke, |_| 0);
    let position = game.position();
    assert!(position.cards(Zone::Achievements(A)).contains(&claim));
    // The melded card was executed: it drew the [1].
    assert!(position.cards(Zone::Hand(A)).contains(&one));
    assert!(position.locate(drawn).is_some());
}

#[test]
fn streaming_repeats_with_the_same_color_when_foreseen() {
    for promoted in [false, true] {
        let mut layout = Layout::default();
        let streaming = card("E112");
        let purple = [
            top(&mut layout, A, Color::Purple, 1),
            top(&mut layout, A, Color::Purple, 2),
        ];
        let mut colors = 0;
        let mut pick = |decision: &Decision| match decision.options[0] {
            Alternative::Colors(_) => {
                colors += 1;
                decision
                    .options
                    .iter()
                    .position(|option| *option == Alternative::Colors(1 << Color::Purple as u8))
                    .unwrap()
            }
            // Score, never achieve.
            Alternative::Branch(_) => 1,
            _ => 0,
        };
        let game = if promoted {
            let action = layout.add(dogma_card(Color::Red, 11, vec![Vec::new()]), Zone::Hand(A));
            layout.add(streaming, Zone::Forecast(A));
            let mut game =
                Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
            game.take_action(Action::Meld { card: action }, ActionKind::Regular)
                .unwrap();
            settle(&mut game, &mut pick);
            game
        } else {
            let streaming = layout.add(streaming, Zone::Board(A, Color::Blue));
            dogma(layout, Expansions::ECHOES, streaming, &mut pick).0
        };
        let scored = game.position().cards(Zone::Score(A));
        // One color choice; the repeat keeps it and scores the next top.
        assert_eq!(colors, 1);
        if promoted {
            assert_eq!(scored, [purple[1], purple[0]]);
        } else {
            assert_eq!(scored, [purple[1]]);
        }
    }
}

#[test]
fn calculator_repeats_once_only() {
    let mut layout = Layout::default();
    let calculator = layout.add(card("E092"), Zone::Hand(A));
    for _ in 0..3 {
        top(&mut layout, A, Color::Red, 1);
        top(&mut layout, A, Color::Green, 2);
    }
    let threes = [
        deck_card(&mut layout, CardSet::Base, Color::Purple, 3),
        deck_card(&mut layout, CardSet::Base, Color::Purple, 3),
        deck_card(&mut layout, CardSet::Base, Color::Purple, 3),
    ];
    let game = run_effect(layout, Expansions::ECHOES, calculator, 0, |_| 0);
    let position = game.position();
    // Two rounds of a red [1] and a green [2]: two [3]s, then no third round.
    assert_eq!(position.cards(Zone::Score(A)).len(), 4);
    let mut hand: Vec<_> = position.cards(Zone::Hand(A)).to_vec();
    hand.retain(|&card| card != calculator);
    hand.sort();
    assert_eq!(hand, [threes[1], threes[2]]);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)).len(), 1);
}

#[test]
fn an_execution_slot_needs_a_card_execution() {
    // Morphine's demand, run alone, has no execution to remember in (an
    // action slot is fine: every invocation runs inside an action scope).
    let mut layout = Layout::default();
    let morphine = layout.add(card("E059"), Zone::Hand(B));
    in_hand(&mut layout, A, Color::Red, 3);
    deck_card(&mut layout, CardSet::Base, Color::Red, 6);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: morphine,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let error = loop {
        match game.advance(1000, &mut events) {
            Ok(Status::Decision(_)) => game.choose(0).unwrap(),
            Ok(status) => panic!("unexpected {status:?}"),
            Err(error) => break error,
        }
    };
    assert_eq!(
        error,
        Error::Unsupported("a slot outside its execution or action")
    );
}

#[test]
fn charitable_trust_remembers_each_players_echo_draw() {
    // B shares: B's echo, A's echo, B's dogma, A's dogma. Each dogma melds
    // the card its own player's echo drew.
    let mut layout = Layout::default();
    let trust = layout.add(card("E030"), Zone::Board(A, Color::Green));
    layout.add(
        printed(Color::Red, [Symbol::Icon(Icon::Prosperity); 4]),
        Zone::Board(B, Color::Red),
    );
    let three = deck_card(&mut layout, CardSet::Base, Color::Purple, 3);
    let four = deck_card(&mut layout, CardSet::Base, Color::Yellow, 4);
    // A's sharing draw afterwards.
    ages(&mut layout, 5..=5);
    let (game, _) = dogma(layout, Expansions::ECHOES, trust, |decision| {
        match (decision.player, &decision.options[0]) {
            // B draws a [3], A a [4].
            (player, Alternative::Value(_)) => usize::from(player == A),
            _ => 0,
        }
    });
    let position = game.position();
    assert_eq!(position.top(B, Color::Purple), Some(three));
    assert_eq!(position.top(A, Color::Yellow), Some(four));
    // A melded a [4]: its top green card, Charitable Trust, was returned.
    assert_eq!(position.top(A, Color::Green), None);
    assert!(
        position
            .locate(trust)
            .is_some_and(|zone| matches!(zone, Zone::Deck(_)))
    );
}

/// A value-1 City showing `icon` on A's board, and a value-1 card in A's
/// hand: A's Dogma of a card featuring `icon` is offered an endorsement,
/// paid with that card (the only one cheap enough).
fn endorsable(layout: &mut Layout, color: Color, icon: Icon) {
    let mut city = printed(
        color,
        [
            Symbol::Icon(icon),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ],
    );
    city.set = CardSet::Cities;
    layout.add(city, Zone::Board(A, color));
    in_hand(layout, A, Color::Yellow, 1);
}

/// A's endorsed Dogma action on `card`, with Echoes and Cities.
fn endorsed_dogma(layout: Layout, card: CardId, pick: impl FnMut(&Decision) -> usize) -> Game {
    let expansions = Expansions::ECHOES.union(Expansions::CITIES);
    let (game, _) = dogma(layout, expansions, card, pick);
    assert!(game.position().endorsement_used());
    game
}

#[test]
fn an_endorsed_jet_echo_that_melds_nothing_keeps_the_earlier_meld() {
    // The echo runs twice: the first melds A's purple card, the second finds
    // an empty hand. The demand still reads purple, the last color melded.
    let mut layout = Layout::default();
    let jet = layout.add(card("E087"), Zone::Board(A, Color::Red));
    endorsable(&mut layout, Color::Green, Icon::Efficiency);
    let melded = in_hand(&mut layout, A, Color::Purple, 4);
    let returned = top(&mut layout, B, Color::Purple, 2);
    let achievements: Vec<_> = [2, 4, 5]
        .iter()
        .map(|&value| {
            let mut card = printed(Color::Green, [Symbol::Blank; 4]);
            card.value = value;
            layout.add(card, Zone::AvailableAchievements)
        })
        .collect();
    for _ in 0..2 {
        deck_card(&mut layout, CardSet::Base, Color::Blue, 10);
    }
    let game = endorsed_dogma(layout, jet, |_| 0);
    let position = game.position();
    assert_eq!(position.top(A, Color::Purple), Some(melded));
    assert_eq!(position.top(B, Color::Purple), None);
    assert!(
        position
            .locate(returned)
            .is_some_and(|zone| matches!(zone, Zone::Deck(_)))
    );
    // The [2] (returned) and [4] (melded) achievements are junked.
    assert_eq!(
        position.cards(Zone::AvailableAchievements),
        [achievements[2]]
    );
    assert!(position.cards(Zone::Junk).contains(&achievements[0]));
    assert!(position.cards(Zone::Junk).contains(&achievements[1]));
}

#[test]
fn an_endorsed_morphine_reads_both_demand_returns() {
    // B returns its [5] on the demand's first run and nothing on the second;
    // A's dogma, run twice, draws two [6]s, not [1]s.
    let mut layout = Layout::default();
    let morphine = layout.add(card("E059"), Zone::Board(A, Color::Yellow));
    endorsable(&mut layout, Color::Blue, Icon::Concept);
    let five = in_hand(&mut layout, B, Color::Red, 5);
    let four = in_hand(&mut layout, B, Color::Red, 4);
    let ones = [
        deck_card(&mut layout, CardSet::Base, Color::Green, 1),
        deck_card(&mut layout, CardSet::Base, Color::Green, 1),
    ];
    for _ in 0..4 {
        deck_card(&mut layout, CardSet::Base, Color::Green, 6);
    }
    let game = endorsed_dogma(layout, morphine, |_| 0);
    let position = game.position();
    let values = |player| {
        let mut values: Vec<_> = position
            .cards(Zone::Hand(player))
            .iter()
            .map(|&card| position.card(card).value)
            .collect();
        values.sort();
        values
    };
    assert_eq!(values(A), [6, 6]);
    assert_eq!(values(B), [4, 6, 6]);
    assert!(position.cards(Zone::Hand(B)).contains(&four));
    assert!(!position.cards(Zone::Hand(B)).contains(&five));
    assert!(
        ones.iter()
            .all(|&one| position.locate(one) == Some(Zone::Deck(Deck::base(1))))
    );
}

#[test]
fn an_endorsed_charitable_trust_melds_only_the_last_echo_draw() {
    // The echo runs twice, drawing a [3] then a [4]. The first dogma run
    // melds the [4]; the second has no last-drawn card in hand, and the [3]
    // stays there.
    let mut layout = Layout::default();
    let trust = layout.add(card("E030"), Zone::Board(A, Color::Green));
    endorsable(&mut layout, Color::Red, Icon::Prosperity);
    let three = deck_card(&mut layout, CardSet::Base, Color::Purple, 3);
    let four = deck_card(&mut layout, CardSet::Base, Color::Blue, 4);
    let mut values = 0;
    let mut menus = Vec::new();
    let game = endorsed_dogma(layout, trust, |decision| {
        if matches!(decision.options[0], Alternative::Value(_)) {
            values += 1;
            values - 1
        } else {
            menus.push(offered(decision));
            0
        }
    });
    let position = game.position();
    assert_eq!(values, 2);
    assert_eq!(position.top(A, Color::Blue), Some(four));
    assert_eq!(position.cards(Zone::Hand(A)), [three]);
    assert!(menus.iter().all(|menu| !menu.contains(&three)));
    // Melding the [4] returned the top green card, Charitable Trust.
    assert_eq!(position.top(A, Color::Green), None);
}

/// Draw a boxed `age` (or draw and score it as one block), then win if that
/// fully happened.
fn exact_draw_program(age: u8, block: bool) -> Vec<Instruction> {
    let mut program = if block {
        vec![build::once(2), build::draw(age), build::score(build::LAST)]
    } else {
        vec![build::draw(age)]
    };
    program.extend([
        build::if_(build::DID, 1),
        Instruction::Win(WinRule::Executor),
    ]);
    program
}

#[test]
fn a_draw_above_its_age_does_not_fully_happen() {
    // The draw still happens when the preceding tuck fails.
    for block in [false, true] {
        for (present, wins) in [(true, true), (false, false)] {
            let mut layout = Layout::default();
            let card = slot_program(&mut layout, exact_draw_program(2, block));
            if present {
                deck_card(&mut layout, CardSet::Base, Color::Green, 2);
            }
            let three = deck_card(&mut layout, CardSet::Base, Color::Red, 3);
            let (game, won) = run_program(layout, card);
            assert_eq!(won, wins, "block {block} present {present}");
            if !present {
                let zone = if block { Zone::Score(A) } else { Zone::Hand(A) };
                assert_eq!(game.position().cards(zone), [three]);
            }
        }
    }
}

#[test]
fn a_draw_escalated_by_karma_does_not_fully_happen() {
    // John von Neumann junks the [8] deck before a draw of an [8], which
    // then takes a [9].
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    for neumann in [false, true] {
        let mut layout = Layout::default();
        if neumann {
            layout.figure(Figure::JohnVonNeumann, A);
        }
        let card = slot_program(
            &mut layout,
            vec![
                build::draw(8),
                build::if_(build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        deck_card(&mut layout, CardSet::Base, Color::Green, 8);
        let nine = deck_card(&mut layout, CardSet::Base, Color::Red, 9);
        let mut game = Game::new(layout.position(both), both).unwrap();
        game.self_execute(card, Context::new(A)).unwrap();
        let won = settle(&mut game, |_| 0);
        assert_eq!(won, !neumann);
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&nine),
            neumann
        );
    }
}

#[test]
fn canning_still_tucks_a_card_drawn_above_its_age() {
    // "You may draw and tuck a [6]": an empty [6] deck gives a [7], which is
    // still tucked; the optional draw was taken.
    let mut layout = Layout::default();
    let canning = layout.add(base_card("B058").unwrap(), Zone::Board(A, Color::Yellow));
    let seven = deck_card(&mut layout, CardSet::Base, Color::Red, 7);
    let mut game = Game::new(layout.position(Expansions::BASE), Expansions::BASE).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: canning,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, |_| 0);
    // Tucked as the only red card, then scored as a top card without
    // [Industry]: it never stayed in hand.
    assert!(game.position().cards(Zone::Score(A)).contains(&seven));
}
