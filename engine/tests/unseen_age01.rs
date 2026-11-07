//! Unseen Age 1: one expected-outcome scenario per card.

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

/// Choose preferred known cards where offered, else the first option.
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
fn assassination_takes_an_authority_draw_with_its_top_and_claims_confidence() {
    for authority in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U001", Zone::Board(A, Color::Red));
        let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        if authority {
            icons(&mut f, drawn, &[Icon::Authority]);
        }
        let top = f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if authority {
            assert_eq!(position.cards(Zone::Score(A)), &[drawn, top]);
            assert!(position.cards(Zone::Hand(B)).is_empty());
        } else {
            assert_eq!(position.cards(Zone::Hand(B)), &[drawn]);
            assert_eq!(position.top(B, Color::Blue), Some(top));
        }
        // No green top anywhere: Confidence is claimed.
        effect(&mut game, card, 1, A, A);
        complete(run(&mut game, &mut events, &[]));
        assert_eq!(
            game.position().special_location(Special::Confidence),
            SpecialLocation::Owned(A)
        );
    }
}

#[test]
fn maze_scores_by_the_demanders_colors_or_exchanges_hand_and_score() {
    for matching in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U002", Zone::Board(A, Color::Red));
        let mine = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let scored = f.tech(Color::Yellow, 5, Zone::Score(A), vec![]);
        let theirs = f.tech(
            if matching { Color::Red } else { Color::Blue },
            3,
            Zone::Hand(B),
            vec![],
        );
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert_eq!(position.cards(Zone::Hand(A)), &[mine]);
        if matching {
            assert_eq!(position.cards(Zone::Score(B)), &[theirs]);
            assert_eq!(position.cards(Zone::Score(A)), &[scored]);
        } else {
            assert_eq!(position.cards(Zone::Hand(B)), &[scored]);
            assert_eq!(position.cards(Zone::Score(A)), &[theirs]);
        }
    }
}

#[test]
fn maze_reveals_the_remaining_hand_only_once_for_multiple_missing_matches() {
    for tracked in [false, true] {
        for matches in 0..=3 {
            let mut f = Fixture::default();
            let maze = unseen(&mut f, "U002", Zone::Board(A, Color::Red));
            let demanding: Vec<_> = (0..3)
                .map(|_| f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]))
                .collect();
            let matching: Vec<_> = (0..matches)
                .map(|_| f.tech(Color::Yellow, 2, Zone::Hand(B), vec![]))
                .collect();
            let red = f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
            let blue = f.tech(Color::Blue, 4, Zone::Hand(B), vec![]);
            let scored = f.tech(Color::Green, 5, Zone::Score(A), vec![]);
            let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            effect(&mut game, maze, 0, B, A);
            let mut events = Vec::new();
            complete(run(&mut game, &mut events, &matching));
            let revealed: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed { card, .. } => Some(*card),
                    _ => None,
                })
                .collect();
            let mut expected = demanding.clone();
            expected.extend(&matching);
            if matches < 3 {
                expected.extend([red, blue]);
            }
            assert_eq!(revealed, expected, "tracked={tracked}, matches={matches}");
            let position = game.position();
            assert_eq!(position.cards(Zone::Hand(A)), demanding);
            assert_eq!(position.cards(Zone::Score(B)), matching);
            if matches < 3 {
                assert_eq!(position.cards(Zone::Hand(B)), [scored]);
                assert_eq!(position.cards(Zone::Score(A)), [red, blue]);
            } else {
                assert_eq!(position.cards(Zone::Hand(B)), [red, blue]);
                assert_eq!(position.cards(Zone::Score(A)), [scored]);
            }
        }
    }
}

#[test]
fn pilgrimage_returns_rising_values_and_safeguards_each() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U003", Zone::Board(A, Color::Red));
    let one = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let two = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let four = f.tech(Color::Yellow, 4, Zone::Hand(A), vec![]);
    let s1 = f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
    let s2 = f.tech(Color::Purple, 2, Zone::AvailableAchievements, vec![]);
    let s3 = f.tech(Color::Purple, 3, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[one, two]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[s1, s2]);
    assert_eq!(position.cards(Zone::Hand(A)), &[four]);
    assert_eq!(position.cards(Zone::AvailableAchievements), &[s3]);
}

#[test]
fn handshake_hands_over_everything_then_two_colors_back() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U004", Zone::Board(A, Color::Yellow));
    let red1 = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let blue2 = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let red3 = f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
    let green4 = f.tech(Color::Green, 4, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    let colors = decision(&mut game, &mut events);
    assert_eq!(colors.player, B);
    // The victim's own cards show before the choice; the demander's are
    // already known to both.
    let shown: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(shown, [red3, green4]);
    // Pairs among red, blue and green only.
    assert_eq!(colors.options.len(), 3);
    let red_blue = (1 << Color::Red as u8) | (1 << Color::Blue as u8);
    let index = colors
        .options
        .iter()
        .position(|option| *option == Alternative::Colors(red_blue))
        .unwrap();
    game.choose(index).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    let mut a: Vec<_> = position.cards(Zone::Hand(A)).to_vec();
    a.sort();
    assert_eq!(a, vec![red1, blue2, red3]);
    assert_eq!(position.cards(Zone::Hand(B)), &[green4]);
}

#[test]
fn handshake_takes_same_back_cards_without_an_order_choice() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U004", Zone::Board(A, Color::Yellow));
    let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let green = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    // B can't tell A's two age-1 cards apart, and they land in an unordered
    // hand, so the color choice is the first question.
    let colors = decision(&mut game, &mut events);
    assert_eq!(colors.player, B);
    assert!(
        colors
            .options
            .iter()
            .all(|option| matches!(option, Alternative::Colors(_)))
    );
    let mut hand: Vec<_> = game.position().cards(Zone::Hand(B)).to_vec();
    hand.sort();
    assert_eq!(hand, vec![red, blue, green]);
}

#[test]
fn tomb_safeguards_by_achievement_count_and_returns_yellow_and_blue_for_the_lowest() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U005", Zone::Board(A, Color::Yellow));
    f.tech(Color::Red, 1, Zone::Achievements(A), vec![]);
    let s1 = f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
    let s2 = f.tech(Color::Purple, 2, Zone::AvailableAchievements, vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    let green = f.tech(Color::Green, 3, Zone::Board(A, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[s2]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[s1]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), &[s1]);
    assert!(position.cards(Zone::Board(A, Color::Yellow)).is_empty());
    assert!(position.cards(Zone::Board(A, Color::Blue)).is_empty());
    assert_eq!(position.top(A, Color::Green), Some(green));
    assert_eq!(position.cards(Zone::Deck(Deck::base(3))), &[blue]);
    // Tomb returns to its own set's deck.
    assert_eq!(
        position.cards(Zone::Deck(Deck {
            set: CardSet::Unseen,
            age: 1
        })),
        &[card]
    );
}

#[test]
fn woodworking_scores_the_melded_card_only_when_it_is_a_bottom_card() {
    for existing in [false, true] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U006", Zone::Board(A, Color::Yellow));
        let two = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
        if existing {
            f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if existing {
            assert_eq!(position.top(A, Color::Green), Some(two));
            assert!(position.cards(Zone::Score(A)).is_empty());
        } else {
            assert_eq!(position.cards(Zone::Score(A)), &[two]);
        }
    }
}

#[test]
fn dance_gives_an_authority_top_and_melds_their_lowest_other_top() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U007", Zone::Board(A, Color::Green));
    let given = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    icons(&mut f, given, &[Icon::Authority]);
    let blue = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
    let yellow = f.tech(Color::Yellow, 1, Zone::Board(B, Color::Yellow), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(B, Color::Red), Some(given));
    assert_eq!(position.top(A, Color::Yellow), Some(yellow));
    assert_eq!(position.top(B, Color::Blue), Some(blue));
}

#[test]
fn rumor_returns_a_score_card_for_one_higher_and_passes_a_hand_card() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U008", Zone::Board(A, Color::Green));
    let three = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    let four = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let hand = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), &[three]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[hand, four]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[hand]));
    assert_eq!(game.position().cards(Zone::Hand(B)), &[hand]);
}

#[test]
fn silk_may_score_one_hand_card_per_board_color() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U009", Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let red_a = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let red_b = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
    let green = f.tech(Color::Green, 4, Zone::Hand(A), vec![]);
    let blue = f.tech(Color::Blue, 5, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    let first = decision(&mut game, &mut events);
    assert!(first.options.contains(&Alternative::Decline));
    // Blue is not on the board: never offered.
    assert!(pick_known(&first, blue).is_none());
    // Once started, one card of each remaining board color follows; green
    // is the only candidate left, so it scores without a question.
    game.choose(pick_known(&first, red_b).unwrap()).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)), &[red_b, green]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[red_a, blue]);
}

#[test]
fn silk_reveals_the_remaining_hand_when_a_board_color_goes_unscored() {
    for yellow_on_board in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U009", Zone::Board(A, Color::Green));
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        if yellow_on_board {
            f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
        }
        let red = f.tech(Color::Red, 3, Zone::Hand(A), vec![]);
        let green = f.tech(Color::Green, 4, Zone::Hand(A), vec![]);
        let blue = f.tech(Color::Blue, 5, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN)
            .unwrap()
            .with_tracking()
            .unwrap();
        effect(&mut game, card, 1, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[red, green]));
        assert_eq!(game.position().cards(Zone::Score(A)), &[red, green]);
        let revealed: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        // Each scored card proves its color; an unscored board color
        // (yellow) is proven absent by the rest of the hand.
        assert!(revealed.contains(&red) && revealed.contains(&green));
        assert_eq!(revealed.contains(&blue), yellow_on_board);
    }
}

#[test]
fn espionage_takes_revealed_cards_of_colors_the_demander_lacks_and_proves_it() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U010", Zone::Board(A, Color::Blue));
    let red = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let blue = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let mine = f.tech(Color::Blue, 5, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    // B reveals red: A's hand has no red. The absence proof is skipped
    // because B can already deduce A's only hand card from the catalog,
    // so red moves and the effect repeats; blue then finds A's blue, which
    // A shows as the witness.
    complete(run(&mut game, &mut events, &[red, blue]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), &[mine, red]);
    assert_eq!(position.cards(Zone::Hand(B)), &[blue]);
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, vec![red, blue, mine]);
    for viewer in Player::ALL {
        game.determinize(viewer, 1).unwrap();
    }
}

#[test]
fn palmistry_rewards_only_a_full_two_card_return() {
    for cards in [1, 2] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U011", Zone::Board(A, Color::Blue));
        for i in 0..cards {
            f.tech(Color::ALL[i], 1, Zone::Hand(A), vec![]);
        }
        let two = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 1, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert!(position.cards(Zone::Hand(A)).is_empty());
        if cards == 2 {
            assert_eq!(position.cards(Zone::Score(A)), &[two]);
        } else {
            assert_eq!(position.cards(Zone::Deck(Deck::base(2))), &[two]);
        }
    }
}

#[test]
fn proverb_branches_on_the_returned_color() {
    for yellow in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U012", Zone::Board(A, Color::Blue));
        let extra = [
            f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]),
            f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]),
        ];
        let top = f.tech(
            if yellow { Color::Yellow } else { Color::Blue },
            1,
            Zone::Deck(Deck::base(1)),
            vec![],
        );
        let hand = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
        let s2 = f.tech(Color::Purple, 2, Zone::AvailableAchievements, vec![]);
        let s3 = f.tech(Color::Purple, 3, Zone::AvailableAchievements, vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&top));
        if yellow {
            assert_eq!(position.cards(Zone::Safe(A)), &[s2]);
            assert!(position.cards(Zone::Hand(A)).is_empty());
            assert!(position.cards(Zone::Deck(Deck::base(2))).contains(&hand));
        } else {
            let mut got = position.cards(Zone::Hand(A)).to_vec();
            got.sort();
            let mut want = vec![hand, extra[0], extra[1]];
            want.sort();
            assert_eq!(got, want);
            assert_eq!(position.cards(Zone::AvailableAchievements), &[s2, s3]);
        }
    }
}

#[test]
fn myth_tucks_a_same_color_pair_splays_and_safeguards_by_the_bottom() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U013", Zone::Board(A, Color::Purple));
    let red1 = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let red2 = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Hand(A), vec![]);
    let two = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let one = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[red1, red2]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), &[blue]);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)).len(), 2);
    assert_eq!(position.splay(A, Color::Red), Splay::Left);
    // The bottom red card's value decides the draw.
    let bottom = position.cards(Zone::Board(A, Color::Red))[0];
    let expected = if bottom == red1 { one } else { two };
    assert_eq!(position.cards(Zone::Safe(A)), &[expected]);
}

#[test]
fn polytheism_melds_cards_with_no_icon_already_melded_this_action() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U014", Zone::Board(A, Color::Purple));
    let first = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    icons(&mut f, first, &[Icon::Authority]);
    let shares = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    icons(&mut f, shares, &[Icon::Authority, Icon::Concept]);
    let other = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    icons(&mut f, other, &[Icon::Health]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[first]));
    let position = game.position();
    assert_eq!(position.top(A, Color::Red), Some(first));
    assert_eq!(position.top(A, Color::Green), Some(other));
    assert_eq!(position.cards(Zone::Hand(A)), &[shares]);
}

/// City icons count as icons: Lisbon's two Plus icons and two Industry make two of two.
/// A executes Symbology on B's board so only Lisbon's icons count; without Cities'
/// icons there would be no draw.
#[test]
fn symbology_counts_each_city_icon_kind_as_an_icon_type() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U015", Zone::Board(B, Color::Purple));
    f.add(city_card("C039").unwrap(), Zone::Board(A, Color::Yellow));
    let two = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN.union(Expansions::CITIES)).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[two]);
}

/// Bonuses count as icons, all values as one type: two Industry and two bonuses of
/// different values make two of two.
#[test]
fn symbology_counts_visible_bonuses_as_one_icon_type() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U015", Zone::Board(B, Color::Purple));
    let top = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    icons(&mut f, top, &[Icon::Industry, Icon::Industry]);
    f.cards[usize::from(top.0)].symbols[2] = Symbol::Bonus(3);
    f.cards[usize::from(top.0)].symbols[3] = Symbol::Bonus(5);
    let two = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN.union(Expansions::ECHOES)).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[two]);
}

#[test]
fn symbology_draws_by_the_largest_square_of_icon_types() {
    // Symbology itself shows two Concept and one Authority; the piles use
    // other types so each case is exact.
    for (types, each, drawn) in [
        (4, 4, Some(4)),
        (3, 3, Some(3)),
        (3, 2, Some(2)),
        (1, 1, None),
    ] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U015", Zone::Board(A, Color::Purple));
        let kinds = [
            Icon::Health,
            Icon::Prosperity,
            Icon::Industry,
            Icon::Efficiency,
        ];
        for (i, &icon) in kinds.iter().take(types).enumerate() {
            let color = [Color::Red, Color::Blue, Color::Green, Color::Yellow][i];
            let top = f.tech(color, 1, Zone::Board(A, color), vec![]);
            icons(&mut f, top, &vec![icon; each]);
        }
        let decks =
            [2, 3, 4].map(|age| f.tech(Color::Red, age, Zone::Deck(Deck::base(age)), vec![]));
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let hand = game.position().cards(Zone::Hand(A));
        match drawn {
            Some(age) => assert_eq!(hand, &[decks[age - 2]], "{types} x {each}"),
            None => assert!(hand.is_empty(), "{types} x {each}"),
        }
    }
}
