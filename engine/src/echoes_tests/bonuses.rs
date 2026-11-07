//! Bonus queries (port step 3): a board's visible bonuses as a multiset,
//! a card's printed bonus and bonus points, proven on the bonus hard cases
//! (Wristwatch, Pressure Cooker, Toilet, Barcode, Crossword, Linguistics)
//! and on Dice, Liquid Fire, Palampore and Sudoku.
use super::cards::{card, deck_card, dogma, in_hand, offered, run_effect, settle};
use super::*;

/// A blank card of value 1 with a bonus in its bottom-right slot, which a
/// left, up or aslant splay shows when it is covered.
fn bonus_card(color: Color, bonus: u8) -> Card {
    printed(
        color,
        [
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Bonus(bonus),
        ],
    )
}

fn bonus_top(layout: &mut Layout, player: Player, color: Color, bonus: u8) -> CardId {
    layout.add(bonus_card(color, bonus), Zone::Board(player, color))
}

fn achievement(layout: &mut Layout, value: u8) -> CardId {
    let mut card = printed(Color::Green, [Symbol::Blank; 4]);
    card.value = value;
    layout.add(card, Zone::AvailableAchievements)
}

/// Put `code` in A's Forecast, to be promoted by a Meld action.
fn foreseen(layout: &mut Layout, code: &str) -> CardId {
    layout.add(card(code), Zone::Forecast(A))
}

/// A's Meld action of a red [11] promotes the card in A's Forecast: its free
/// Dogma action is foreseen.
fn promoted(mut layout: Layout, pick: impl FnMut(&Decision) -> usize) -> Game {
    let action = layout.add(dogma_card(Color::Red, 11, vec![Vec::new()]), Zone::Hand(A));
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.take_action(Action::Meld { card: action }, ActionKind::Regular)
        .unwrap();
    settle(&mut game, pick);
    game
}

const ECHOES_CITIES: Expansions = Expansions::ECHOES.union(Expansions::CITIES);

fn values(position: &Position, zone: Zone) -> Vec<u8> {
    let mut values: Vec<_> = position
        .cards(zone)
        .iter()
        .map(|&card| position.card(card).value)
        .collect();
    values.sort();
    values
}

/// The option offering `value`.
fn value_option(decision: &Decision, value: u8) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|option| *option == Alternative::Value(value))
}

/// A red pile splayed `splay` with bonuses 1 and 2 covered and 3 on top, a
/// City showing two 4s, and a green 6: six bonuses when the splay shows
/// covered bottom-right slots.
fn six_bonuses(splay: Splay, city: bool) -> Layout {
    let mut layout = Layout::default();
    for bonus in 1..=3 {
        bonus_top(&mut layout, A, Color::Red, bonus);
    }
    layout.splay(A, Color::Red, splay);
    let mut blue = bonus_card(Color::Blue, 4);
    if city {
        blue.set = CardSet::Cities;
        blue.symbols[5] = Symbol::Bonus(4);
    }
    layout.add(blue, Zone::Board(A, Color::Blue));
    bonus_top(&mut layout, A, Color::Green, 6);
    layout
}

#[test]
fn palampore_counts_visible_bonuses_and_a_citys_repeats() {
    for (splay, wealth) in [(Splay::Left, true), (Splay::Right, false)] {
        let mut layout = six_bonuses(splay, true);
        let palampore = layout.add(card("E050"), Zone::Hand(A));
        let game = run_effect(layout, ECHOES_CITIES, palampore, 2, |_| 0);
        let expected = if wealth {
            SpecialLocation::Owned(A)
        } else {
            SpecialLocation::Available
        };
        assert_eq!(
            game.position().special_location(Special::Wealth),
            expected,
            "{splay:?}"
        );
    }
}

#[test]
fn palampore_draws_only_a_repeated_bonus_value() {
    // The City repeats 4; a plain card shows it once.
    for city in [true, false] {
        let mut layout = six_bonuses(Splay::Left, city);
        let palampore = layout.add(card("E050"), Zone::Hand(A));
        let four = deck_card(&mut layout, CardSet::Base, Color::Yellow, 4);
        let mut menus = Vec::new();
        let game = run_effect(layout, ECHOES_CITIES, palampore, 0, |decision| {
            menus.push(decision.options.clone());
            0
        });
        assert_eq!(
            game.position().cards(Zone::Score(A)).contains(&four),
            city,
            "City {city}"
        );
        assert!(menus.is_empty(), "one value or none is no decision");
    }
}

#[test]
fn dice_melds_a_card_of_its_revealed_cards_bonus() {
    for bonus in [Some(3), None] {
        let mut layout = Layout::default();
        let dice = layout.add(card("E010"), Zone::Hand(A));
        // The age is resolved by the Base decks first.
        deck_card(&mut layout, CardSet::Base, Color::Green, 1);
        let revealed = deck_card(&mut layout, CardSet::Echoes, Color::Green, 1);
        if let Some(bonus) = bonus {
            layout.cards[usize::from(revealed.0)].symbols[3] = Symbol::Bonus(bonus);
        }
        let three = deck_card(&mut layout, CardSet::Base, Color::Red, 3);
        let game = run_effect(layout, Expansions::ECHOES, dice, 0, |_| 0);
        let position = game.position();
        assert!(position.cards(Zone::Hand(A)).contains(&revealed));
        assert_eq!(position.top(A, Color::Red) == Some(three), bonus.is_some());
    }
}

#[test]
fn foreseen_dice_gives_its_draw_to_an_opponent_with_more_bonus_points() {
    // A shows Dice's bonus 1 (one point); B shows a 5, or nothing.
    for b_bonus in [true, false] {
        let mut layout = Layout::default();
        if b_bonus {
            bonus_top(&mut layout, B, Color::Green, 5);
        }
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 1);
        deck_card(&mut layout, CardSet::Echoes, Color::Yellow, 1);
        let four = deck_card(&mut layout, CardSet::Base, Color::Yellow, 4);
        foreseen(&mut layout, "E010");
        let game = promoted(layout, |_| 0);
        let owner = if b_bonus { B } else { A };
        assert!(game.position().cards(Zone::Hand(owner)).contains(&four));
    }
}

#[test]
fn linguistics_chooses_among_bonuses_on_either_board() {
    let mut layout = Layout::default();
    let linguistics = layout.add(card("E023"), Zone::Hand(A));
    bonus_top(&mut layout, A, Color::Red, 2);
    bonus_top(&mut layout, B, Color::Red, 5);
    bonus_top(&mut layout, B, Color::Blue, 2);
    let five = deck_card(&mut layout, CardSet::Base, Color::Yellow, 5);
    let mut menus = Vec::new();
    let game = run_effect(layout, Expansions::ECHOES, linguistics, 0, |decision| {
        menus.push(decision.options.clone());
        value_option(decision, 5).unwrap()
    });
    assert_eq!(menus, [vec![Alternative::Value(2), Alternative::Value(5)]]);
    assert!(game.position().cards(Zone::Hand(A)).contains(&five));
}

#[test]
fn foreseen_linguistics_junks_only_after_a_draw_of_the_chosen_value() {
    // An empty [5] deck skips to a 6: it drew, but not the chosen value.
    for present in [true, false] {
        let mut layout = Layout::default();
        bonus_top(&mut layout, B, Color::Red, 5);
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 3);
        if present {
            deck_card(&mut layout, CardSet::Base, Color::Yellow, 5);
        }
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 6);
        let fives = [achievement(&mut layout, 5), achievement(&mut layout, 5)];
        let six = achievement(&mut layout, 6);
        foreseen(&mut layout, "E023");
        let game = promoted(layout, |_| 0);
        let position = game.position();
        let zone = if present {
            Zone::Junk
        } else {
            Zone::AvailableAchievements
        };
        assert!(fives.iter().all(|card| position.cards(zone).contains(card)));
        assert!(position.cards(Zone::AvailableAchievements).contains(&six));
    }
}

#[test]
fn liquid_fire_draws_at_the_highest_bonus_and_judges_the_drawn_card() {
    // A refused red draw stays in the victim's hand and goes with it.
    for (red, full) in [(true, false), (false, false), (true, true), (false, true)] {
        let mut layout = Layout::default();
        let fire = layout.add(card("E026"), Zone::Board(A, Color::Red));
        bonus_top(&mut layout, B, Color::Blue, 2);
        bonus_top(&mut layout, B, Color::Green, 4);
        let hand = [
            in_hand(&mut layout, B, Color::Purple, 3),
            in_hand(&mut layout, B, Color::Purple, 6),
        ];
        let color = if red { Color::Red } else { Color::Yellow };
        let drawn = deck_card(&mut layout, CardSet::Base, color, 4);
        if full {
            layout.forecast(A, 5);
        }
        let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
        game.take_action(
            Action::Dogma {
                card: fire,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut events = Vec::new();
        while let Status::Decision(_) = game.advance(1000, &mut events).unwrap() {
            game.choose(0).unwrap();
        }
        // Left in B's hand, it is revealed: its color decides the rest.
        let revealed = events
            .iter()
            .any(|event| matches!(event, Event::Revealed { card, .. } if *card == drawn));
        assert_eq!(revealed, full);
        let position = game.position();
        let scored = position.cards(Zone::Score(A));
        assert_eq!(
            position.cards(Zone::Forecast(A)).contains(&drawn),
            !full,
            "red {red} full {full}"
        );
        assert_eq!(hand.iter().all(|card| scored.contains(card)), red);
        assert_eq!(scored.contains(&drawn), full && red);
        assert_eq!(position.cards(Zone::Hand(B)).contains(&drawn), full && !red);
    }
}

#[test]
fn toilet_returns_one_card_per_different_bonus_value_of_the_demander() {
    let mut layout = Layout::default();
    let toilet = layout.add(card("E045"), Zone::Board(A, Color::Purple));
    bonus_top(&mut layout, A, Color::Red, 2);
    bonus_top(&mut layout, A, Color::Blue, 2);
    bonus_top(&mut layout, A, Color::Green, 3);
    let score: Vec<_> = [2, 2, 3, 4]
        .iter()
        .map(|&value| {
            let mut card = printed(Color::Yellow, [Symbol::Blank; 4]);
            card.value = value;
            layout.add(card, Zone::Score(B))
        })
        .collect();
    // The echo draws and tucks a [4].
    deck_card(&mut layout, CardSet::Base, Color::Yellow, 4);
    let mut menus = Vec::new();
    let (game, _) = dogma(layout, Expansions::ECHOES, toilet, |decision| {
        if decision.player == B {
            menus.push(offered(decision));
        }
        0
    });
    assert_eq!(values(game.position(), Zone::Score(B)), [2, 4]);
    // B's one real choice is which [2] goes; the [3] follows, with no
    // question about the order of values.
    assert_eq!(menus, [vec![score[0], score[1]]]);
}

#[test]
fn barcode_returns_one_card_per_bonus_occurrence() {
    let mut layout = Layout::default();
    let barcode = layout.add(card("E101"), Zone::Board(A, Color::Green));
    bonus_top(&mut layout, B, Color::Red, 2);
    bonus_top(&mut layout, B, Color::Blue, 2);
    bonus_top(&mut layout, B, Color::Yellow, 5);
    for value in [2, 2, 2, 5, 7] {
        let mut card = printed(Color::Purple, [Symbol::Blank; 4]);
        card.value = value;
        layout.add(card, Zone::Score(B));
    }
    let (game, _) = dogma(layout, Expansions::ECHOES, barcode, |_| 0);
    assert_eq!(values(game.position(), Zone::Score(B)), [2, 7]);
}

/// A's tops showing 2, 4, 4 (even) and 3 (odd), and one Base card each of
/// [2], [4] and [5]: the second [4] draw skips to the 5.
fn crossword_board() -> (Layout, [CardId; 3]) {
    let mut layout = Layout::default();
    bonus_top(&mut layout, A, Color::Blue, 2);
    bonus_top(&mut layout, A, Color::Green, 4);
    bonus_top(&mut layout, A, Color::Yellow, 4);
    bonus_top(&mut layout, A, Color::Purple, 3);
    let drawn = [2, 4, 5].map(|age| deck_card(&mut layout, CardSet::Base, Color::Blue, age));
    (layout, drawn)
}

#[test]
fn crossword_draws_once_per_even_bonus_and_returns_once_per_odd() {
    let (mut layout, drawn) = crossword_board();
    let crossword = layout.add(card("E084"), Zone::Hand(A));
    let game = run_effect(layout, Expansions::ECHOES, crossword, 0, |_| 0);
    let hand = game.position().cards(Zone::Hand(A));
    assert!(drawn.iter().all(|card| hand.contains(card)));

    let (mut layout, _) = crossword_board();
    let crossword = layout.add(card("E084"), Zone::Hand(A));
    let low = in_hand(&mut layout, A, Color::Red, 1);
    let high = in_hand(&mut layout, A, Color::Red, 6);
    let game = run_effect(layout, Expansions::ECHOES, crossword, 1, |_| 0);
    assert_eq!(game.position().cards(Zone::Hand(A)), [crossword, high]);
    assert!(
        game.position()
            .locate(low)
            .is_some_and(|zone| matches!(zone, Zone::Deck(_)))
    );
}

#[test]
fn foreseen_crossword_moves_every_drawn_card_to_the_achievements() {
    // Crossword's own bonus 8 is even too; the age-skipped 5 is kept.
    let (mut layout, drawn) = crossword_board();
    let eight = deck_card(&mut layout, CardSet::Base, Color::Blue, 8);
    foreseen(&mut layout, "E084");
    let game = promoted(layout, |_| 0);
    let supply = game.position().cards(Zone::AvailableAchievements);
    assert!(
        drawn
            .iter()
            .chain(&[eight])
            .all(|card| supply.contains(card))
    );
}

#[test]
fn wristwatch_tests_each_bonus_live() {
    // A shows 1 and 2. The [1] it melds covers the 2 and shows a 3, so it
    // draws and melds a 3, not a 2.
    let mut layout = Layout::default();
    let watch = layout.add(card("E089"), Zone::Hand(A));
    bonus_top(&mut layout, A, Color::Red, 1);
    bonus_top(&mut layout, A, Color::Blue, 2);
    let one = deck_card(&mut layout, CardSet::Base, Color::Blue, 1);
    layout.cards[usize::from(one.0)].symbols[3] = Symbol::Bonus(3);
    let two = deck_card(&mut layout, CardSet::Base, Color::Green, 2);
    let three = deck_card(&mut layout, CardSet::Base, Color::Green, 3);
    let game = run_effect(layout, Expansions::ECHOES, watch, 1, |_| 0);
    let position = game.position();
    assert_eq!(position.top(A, Color::Blue), Some(one));
    assert_eq!(position.top(A, Color::Green), Some(three));
    assert_eq!(position.locate(two), Some(Zone::Deck(Deck::base(2))));
}

#[test]
fn foreseen_wristwatch_returns_all_but_each_bottom_card() {
    // The echo tucks Wristwatch itself; then each pile keeps its bottom card,
    // and Wristwatch's bonus 10 draws and melds a [10].
    let mut layout = Layout::default();
    let green: Vec<_> = (0..3)
        .map(|_| {
            layout.add(
                printed(Color::Green, [Symbol::Blank; 4]),
                Zone::Board(A, Color::Green),
            )
        })
        .collect();
    let ten = deck_card(&mut layout, CardSet::Base, Color::Blue, 10);
    let watch = foreseen(&mut layout, "E089");
    let game = promoted(layout, |decision| {
        offered(decision)
            .iter()
            .position(|&card| card == watch)
            .unwrap_or(0)
    });
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Green)), [green[0]]);
    assert_eq!(position.top(A, Color::Blue), Some(ten));
}

#[test]
fn pressure_cooker_draws_and_junks_for_each_top_with_a_bonus() {
    let mut layout = Layout::default();
    let cooker = layout.add(card("E048"), Zone::Hand(A));
    bonus_top(&mut layout, A, Color::Red, 2);
    bonus_top(&mut layout, A, Color::Blue, 5);
    layout.add(
        printed(Color::Green, [Symbol::Blank; 4]),
        Zone::Board(A, Color::Green),
    );
    let returned = in_hand(&mut layout, A, Color::Purple, 3);
    let drawn = [
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 2),
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 5),
    ];
    let junked = [achievement(&mut layout, 2), achievement(&mut layout, 5)];
    let seven = achievement(&mut layout, 7);
    let mut orders = 0;
    let game = run_effect(layout, Expansions::ECHOES, cooker, 1, |decision| {
        orders += usize::from(offered(decision).len() == 2);
        0
    });
    let position = game.position();
    assert_eq!(position.locate(returned), Some(Zone::Deck(Deck::base(3))));
    // Run from A's hand, Pressure Cooker returned with it.
    assert_eq!(position.cards(Zone::Hand(A)), drawn);
    assert!(
        position
            .locate(cooker)
            .is_some_and(|zone| matches!(zone, Zone::Deck(_)))
    );
    assert!(
        junked
            .iter()
            .all(|card| position.cards(Zone::Junk).contains(card))
    );
    assert_eq!(position.cards(Zone::AvailableAchievements), [seven]);
    assert_eq!(orders, 1);
}

/// Sudoku in A's hand over bonuses 1-4 on an up-splayed red pile, 5-6 on
/// blue, and 7 and 8 on top. Its [2] draw (a card that draws a [1] when
/// executed) adds a 9 if `bonus`.
fn sudoku(bonus: bool) -> (Layout, CardId) {
    let mut layout = Layout::default();
    let sudoku = layout.add(card("E105"), Zone::Hand(A));
    for value in 1..=4 {
        bonus_top(&mut layout, A, Color::Red, value);
    }
    layout.splay(A, Color::Red, Splay::Up);
    for value in 5..=6 {
        bonus_top(&mut layout, A, Color::Blue, value);
    }
    layout.splay(A, Color::Blue, Splay::Up);
    bonus_top(&mut layout, A, Color::Green, 7);
    bonus_top(&mut layout, A, Color::Yellow, 8);
    let mut melded = dogma_card(Color::Purple, 2, vec![vec![draw_box(1)]]);
    if bonus {
        melded.symbols[3] = Symbol::Bonus(9);
    }
    layout.add(melded, Zone::Deck(Deck::base(2)));
    (layout, sudoku)
}

/// Run Sudoku's dogma, choosing the [2]; whether A won.
fn run_sudoku(layout: Layout, expansions: Expansions, sudoku: CardId) -> (Game, bool) {
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: sudoku,
            effect: 0,
            boxed: BoxedValues::ZERO,
        },
        Context::new(A),
    )
    .unwrap();
    let won = settle(&mut game, |decision| value_option(decision, 2).unwrap_or(0));
    (game, won)
}

#[test]
fn sudoku_wins_with_nine_different_bonus_values_or_self_executes() {
    for bonus in [true, false] {
        let (mut layout, sudoku) = sudoku(bonus);
        let one = deck_card(&mut layout, CardSet::Base, Color::Yellow, 1);
        let (game, won) = run_sudoku(layout, Expansions::ECHOES, sudoku);
        assert_eq!(won, bonus);
        assert_eq!(game.position().cards(Zone::Hand(A)).contains(&one), !bonus);
    }
}

#[test]
fn a_replaced_sudoku_meld_still_self_executes_the_drawn_card() {
    // Von Neumann melds and self-executes it instead; Sudoku then executes it again.
    let (mut layout, sudoku) = sudoku(false);
    layout.figure(Figure::JohnVonNeumann, A);
    let ones = [
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 1),
        deck_card(&mut layout, CardSet::Base, Color::Yellow, 1),
    ];
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let (game, won) = run_sudoku(layout, both, sudoku);
    assert!(!won);
    let hand = game.position().cards(Zone::Hand(A));
    assert!(ones.iter().all(|one| hand.contains(one)));
}
