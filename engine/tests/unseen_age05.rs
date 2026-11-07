//! Unseen Age 5: one expected-outcome scenario per card (Chartreuse lives
//! in `unseen_hard_cases.rs`).

use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
}

fn icons(f: &mut Fixture, card: CardId, icons: &[Icon]) {
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
    }
}

fn effect(game: &mut Game, card: CardId, index: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
            | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
            if *c == card
        )
    })
}

fn run(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = preferred
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("did not terminate");
}

fn complete(status: Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

#[test]
fn cabal_takes_hand_cards_matching_secret_values_and_safeguards_a_top_value() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U046", Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 3, Zone::Safe(A), vec![]);
    let three = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
    let four = f.tech(Color::Yellow, 4, Zone::Hand(B), vec![]);
    let five = f.tech(Color::Purple, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(A)), &[three]);
    assert_eq!(position.cards(Zone::Hand(B)), &[four, five]);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U046", Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let two = f.tech(Color::Green, 2, Zone::AvailableAchievements, vec![]);
    f.tech(Color::Green, 3, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[two]));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[two]);
}

#[test]
fn witch_trial_repeats_only_while_all_three_returns_happen() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U047", Zone::Board(A, Color::Red));
    let top = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
    let hand = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let scored = f.tech(Color::Green, 3, Zone::Score(B), vec![]);
    let second = f.tech(Color::Yellow, 5, Zone::Deck(Deck::base(5)), vec![]);
    let first = f.tech(Color::Blue, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    // First pass: blue 5 drawn; the blue top, the blue hand card and the
    // score card return, so it repeats; the yellow 5 finds no yellow top,
    // so the second pass fails and the effect ends.
    assert!(position.cards(Zone::Deck(Deck::base(2))).contains(&top));
    assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&hand));
    assert!(position.cards(Zone::Deck(Deck::base(3))).contains(&scored));
    let mut remaining = position.cards(Zone::Hand(B)).to_vec();
    remaining.sort();
    assert_eq!(remaining, vec![second, first]);
}

#[test]
fn gallery_draws_by_scored_twos_and_ones() {
    for (scored_two, scored_one) in [(true, true), (false, false)] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U049", Zone::Board(A, Color::Yellow));
        if scored_two {
            f.tech(Color::Red, 2, Zone::Score(A), vec![]);
        }
        if scored_one {
            f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
        }
        let five = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
        let six = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
        let seven = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        effect(&mut game, card, 1, A, A);
        complete(run(&mut game, &mut events, &[]));
        let hand = game.position().cards(Zone::Hand(A)).to_vec();
        if scored_two {
            assert_eq!(hand, vec![six, seven]);
        } else {
            assert_eq!(hand, vec![five]);
        }
    }
}

#[test]
fn buried_treasure_moves_an_odd_value_from_both_score_piles_and_rewards_four() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U050", Zone::Board(A, Color::Green));
    let mine = [
        f.tech(Color::Red, 3, Zone::Score(A), vec![]),
        f.tech(Color::Blue, 3, Zone::Score(A), vec![]),
    ];
    let theirs = [
        f.tech(Color::Green, 3, Zone::Score(B), vec![]),
        f.tech(Color::Yellow, 3, Zone::Score(B), vec![]),
    ];
    let keep = f.tech(Color::Purple, 4, Zone::Score(B), vec![]);
    let three = f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let values = decision(&mut game, &mut events);
    assert_eq!(
        values.options,
        // [3] is scored; every other odd value is one empty option.
        [1, 3].map(Alternative::Value).to_vec()
    );
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(B)), &[keep]);
    // Four transfers: the drawn 3 is safeguarded and three of the four
    // standard achievements now in the supply are scored back.
    assert_eq!(position.cards(Zone::Safe(A)), &[three]);
    let transferred = [mine.as_slice(), theirs.as_slice()].concat();
    let scored = position.cards(Zone::Score(A));
    assert_eq!(scored.len(), 3);
    assert!(scored.iter().all(|c| transferred.contains(c)));
    assert_eq!(position.cards(Zone::AvailableAchievements).len(), 1);
}

#[test]
fn secret_history_moves_a_secret_to_the_demanders_safe_and_claims_mystery() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U051", Zone::Board(A, Color::Green));
    let secret = f.tech(Color::Blue, 3, Zone::Safe(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[secret]);
    assert!(game.position().cards(Zone::Safe(B)).is_empty());

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U051", Zone::Board(A, Color::Green));
    f.tech(Color::Red, 9, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Purple, 2, Zone::Board(A, Color::Purple), vec![]);
    let position = f
        .position()
        .with_splay(A, Color::Red, Splay::Right)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    // Purple is the only pile the splay changes; red, already right, stays a no-op
    // choice.
    let d = decision(&mut game, &mut events);
    assert_eq!(d.options.len(), 2);
    let purple = d
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Splay {
                    color: Color::Purple,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(purple).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Purple), Splay::Right);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(
        game.position().special_location(Special::Mystery),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn popular_science_melds_by_a_top_green_anywhere_and_above_its_top_yellow() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U052", Zone::Board(A, Color::Blue));
    f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    f.tech(Color::Yellow, 3, Zone::Board(A, Color::Yellow), vec![]);
    let two = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let four = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().top(A, Color::Red), Some(two));
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().top(A, Color::Purple), Some(four));
}

#[test]
fn probability_rewards_two_or_four_icon_types_on_the_revealed_sixes() {
    for (types, extra_draws) in [(2, 2), (4, 1), (3, 0)] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U053", Zone::Board(A, Color::Blue));
        let kinds = [
            Icon::Authority,
            Icon::Concept,
            Icon::Health,
            Icon::Prosperity,
        ];
        let rewards: Vec<_> = (0..3)
            .map(|_| f.tech(Color::Yellow, 6, Zone::Deck(Deck::base(6)), vec![]))
            .collect();
        let seven = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
        let second = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
        icons(&mut f, second, &kinds[..types.min(2)]);
        let first = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
        icons(&mut f, first, &kinds[..types]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 1, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        for card in [first, second] {
            assert!(position.cards(Zone::Deck(Deck::base(6))).contains(&card));
        }
        let hand = position.cards(Zone::Hand(A)).len();
        let scored = position.cards(Zone::Score(A)).len();
        match types {
            2 => assert_eq!((hand, scored), (1, 2), "two types"),
            4 => {
                assert_eq!((hand, scored), (2, 0), "four types");
                assert!(position.cards(Zone::Hand(A)).contains(&seven));
            }
            _ => assert_eq!((hand, scored), (1, 0), "three types"),
        }
        let _ = (rewards, extra_draws);
    }
}

#[test]
fn pantheism_scores_one_tucked_color_and_splays_the_other_right() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U054", Zone::Board(A, Color::Purple));
    let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let red_top = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let four = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    // Tuck red, draw and tuck the blue 4, then choose red to score.
    complete(run(&mut game, &mut events, &[red]));
    let position = game.position();
    let mut scored = position.cards(Zone::Score(A)).to_vec();
    scored.sort();
    assert_eq!(scored, vec![red, red_top]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue))[0], four);
    assert_eq!(position.splay(A, Color::Blue), Splay::Right);
}

#[test]
fn pen_name_offers_a_left_splay_with_execution_or_a_meld_with_a_right_splay() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U055", Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(
        Color::Red,
        2,
        Zone::Board(A, Color::Red),
        vec![build::draw(3)],
    );
    let three = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let hand = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    for i in 0..2 {
        f.tech(Color::ALL[i], 11, Zone::Deck(Deck::base(11)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 2);
    game.choose(0).unwrap();
    // Red and blue are unsplayed and non-purple; choose red.
    let colors = decision(&mut game, &mut events);
    let red = colors
        .options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << Color::Red as u8))
        .unwrap();
    game.choose(red).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(A, Color::Red), Splay::Left);
    assert!(position.cards(Zone::Hand(A)).contains(&three));

    let mut game = Game::new(
        {
            let mut f = Fixture::default();
            let card = unseen(&mut f, "U055", Zone::Board(A, Color::Purple));
            let _ = card;
            f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
            f.tech(Color::Blue, 3, Zone::Hand(A), vec![]);
            f.position()
        },
        Expansions::UNSEEN,
    )
    .unwrap();
    let card = CardId(0);
    effect(&mut game, card, 0, A, A);
    let branch = decision(&mut game, &mut events);
    game.choose(1).unwrap();
    let _ = (branch, hand);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Blue)).len(), 2);
}

/// Buried Treasure: every card of the chosen value goes to the unordered
/// supply with no order menu, while which three standard achievements to
/// score stays the executor's choice.
#[test]
fn buried_treasure_transfers_without_a_menu_and_chooses_three_to_score() {
    let mut scored = Vec::new();
    for last in [false, true] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U050", Zone::Board(A, Color::Green));
        let mut fives = vec![f.tech(Color::Red, 5, Zone::Score(A), vec![])];
        for color in [Color::Red, Color::Green, Color::Blue] {
            fives.push(f.tech(color, 5, Zone::Score(B), vec![]));
        }
        f.cards[usize::from(fives[2].0)].set = CardSet::Unseen;
        let three = f.tech(Color::Green, 3, Zone::Score(B), vec![]);
        let standard: Vec<_> = (1..=4)
            .map(|value| f.tech(Color::Purple, value, Zone::AvailableAchievements, vec![]))
            .collect();
        let drawn = f.tech(Color::Purple, 5, Zone::Deck(Deck::base(5)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let Status::Decision(menu) = settle(&mut game, &mut events) else {
            panic!("expected the odd value choice");
        };
        let five = menu
            .options
            .iter()
            .position(|option| *option == Alternative::Value(5))
            .unwrap();
        game.choose(five).unwrap();
        let mut menus = Vec::new();
        let status = loop {
            match settle(&mut game, &mut events) {
                Status::Decision(menu) => {
                    let index = if last { menu.options.len() - 1 } else { 0 };
                    menus.push(menu);
                    game.choose(index).unwrap();
                }
                status => break status,
            }
        };
        complete(status);
        // Only the scoring asks, the first pick among several. Taking a
        // Base 5 first leaves one order per set: the second Base 5 is forced.
        assert_eq!(menus.len(), if last { 2 } else { 3 }, "{menus:#?}");
        assert!(menus[0].options.len() > 1);
        let position = game.position();
        assert_eq!(position.cards(Zone::Score(B)), [three]);
        assert_eq!(position.cards(Zone::Safe(A)), [drawn]);
        let supply = position.cards(Zone::AvailableAchievements);
        assert_eq!(supply.len(), standard.len() + fives.len() - 3);
        let mut mine = position.cards(Zone::Score(A)).to_vec();
        mine.sort_unstable();
        assert_eq!(mine.len(), 3);
        scored.push(mine);
    }
    assert_ne!(scored[0], scored[1], "the scored three are a real choice");
}
