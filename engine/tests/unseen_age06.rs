//! Unseen Age 6: one expected-outcome scenario per card.

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
fn red_herring_offers_three_red_splays_and_judges_the_tucked_color_against_red() {
    for (blue, expected) in [(Splay::Right, Splay::Up), (Splay::Left, Splay::None)] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U056", Zone::Board(A, Color::Red));
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
        f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        let six = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
        let position = f.position().with_splay(A, Color::Blue, blue).unwrap();
        let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 3, "{:?}", menu.options);
        game.choose(1).unwrap();
        complete(run(&mut game, &mut events, &[]));
        assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
        effect(&mut game, card, 1, A, A);
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert_eq!(position.cards(Zone::Board(A, Color::Blue))[0], six);
        assert_eq!(position.splay(A, Color::Blue), expected);
    }
}

/// The current direction stays a legal no-op.
#[test]
fn red_herring_offers_the_current_direction_as_a_no_op() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U056", Zone::Board(A, Color::Red));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let position = f.position().with_splay(A, Color::Red, Splay::Left).unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3, "{:?}", menu.options);
    let left = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Splay {
                    direction: Splay::Left,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(left).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
}

#[test]
fn sniping_unsplays_melds_the_bottom_and_takes_the_new_bottom() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U057", Zone::Board(A, Color::Red));
    let one = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let three = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
    let five = f.tech(Color::Blue, 5, Zone::Board(B, Color::Blue), vec![]);
    let position = f
        .position()
        .with_splay(B, Color::Blue, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(B, Color::Blue), Splay::None);
    assert_eq!(position.cards(Zone::Board(B, Color::Blue)), &[five, one]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[three]);
}

#[test]
fn attic_safeguards_or_scores_returns_from_score_and_draws_a_scored_value() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U058", Zone::Board(A, Color::Yellow));
    let hand = f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let scored = f.tech(Color::Green, 3, Zone::Score(A), vec![]);
    let kept = f.tech(Color::Red, 4, Zone::Score(A), vec![]);
    let four = f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 3, "{:?}", branch.options);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[hand]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[scored]));
    assert_eq!(game.position().cards(Zone::Score(A)), &[kept]);
    effect(&mut game, card, 2, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)), &[kept, four]);
}

#[test]
fn sabotage_returns_the_demanders_pick_and_tucks_its_color_from_board_and_score() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U059", Zone::Board(A, Color::Yellow));
    let red_hand = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let red_bottom = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let red_top = f.tech(Color::Red, 4, Zone::Board(B, Color::Red), vec![]);
    let red_scored = f.tech(Color::Red, 3, Zone::Score(B), vec![]);
    let green_scored = f.tech(Color::Green, 3, Zone::Score(B), vec![]);
    let six = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[red_hand]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(B)), &[six]);
    assert!(
        position
            .cards(Zone::Deck(Deck::base(2)))
            .contains(&red_hand)
    );
    assert_eq!(
        position.cards(Zone::Board(B, Color::Red)),
        &[red_scored, red_top, red_bottom]
    );
    assert_eq!(position.cards(Zone::Score(B)), &[green_scored]);
}

#[test]
fn hiking_tucks_under_an_industry_top_and_melds_a_healthy_seven() {
    for industry in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U060", Zone::Board(A, Color::Green));
        let blue_top = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
        if industry {
            icons(&mut f, blue_top, &[Icon::Industry]);
        }
        let six = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
        let seven = f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
        icons(&mut f, seven, &[Icon::Health]);
        let eight = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if industry {
            assert_eq!(
                position.cards(Zone::Board(A, Color::Blue)),
                &[six, blue_top]
            );
            assert_eq!(position.top(A, Color::Red), Some(seven));
            assert_eq!(position.cards(Zone::Hand(A)), &[eight]);
        } else {
            assert_eq!(position.cards(Zone::Hand(A)), &[six]);
            assert_eq!(position.top(A, Color::Red), None);
        }
    }
}

#[test]
fn swiss_bank_account_safeguards_by_score_count_and_draws_per_secret() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U061", Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]);
    let two = f.tech(Color::Green, 2, Zone::AvailableAchievements, vec![]);
    let hand_two = f.tech(Color::Purple, 2, Zone::Hand(A), vec![]);
    let hand_three = f.tech(Color::Purple, 3, Zone::Hand(A), vec![]);
    let six = f.tech(Color::Yellow, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[two]);
    assert!(position.cards(Zone::Score(A)).contains(&hand_two));
    assert_eq!(position.cards(Zone::Hand(A)), &[hand_three]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[hand_three, six]);
}

#[test]
fn placebo_repeats_returns_down_one_color_and_rewards_a_single_boxed_seven() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U062", Zone::Board(A, Color::Blue));
    let red_bottom = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let red_middle = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    let red_top = f.tech(Color::Red, 7, Zone::Board(A, Color::Red), vec![]);
    let green = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    let sevens: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]))
        .collect();
    let eight = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    // Return the red 7, then the red 3, then stop before the red 1.
    loop {
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => {
                let index = [red_top, red_middle]
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .or_else(|| {
                        menu.options
                            .iter()
                            .position(|o| *o == Alternative::Branch(1))
                    })
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                complete(status);
                break;
            }
        }
    }
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red_bottom]);
    assert_eq!(position.top(A, Color::Green), Some(green));
    let hand = position.cards(Zone::Hand(A));
    assert_eq!(hand.len(), 3, "{hand:?}");
    assert!(hand.contains(&eight));
    assert_eq!(hand.iter().filter(|c| sevens.contains(c)).count(), 2);
}

#[test]
fn reconnaissance_returns_two_of_three_sevens_and_may_splay_the_survivor() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U063", Zone::Board(A, Color::Blue));
    let secret_hand = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    let green = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
    let blue = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let red = f.tech(Color::Red, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::Revealed { card, .. } if *card == secret_hand
        )),
        "{events:?}"
    );
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[red, blue]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(A)), &[green]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(7))).len(), 2);
    assert_eq!(position.splay(A, Color::Green), Splay::Right);
}

#[test]
fn illuminati_splays_the_revealed_color_and_safeguards_its_top_and_one_higher() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U064", Zone::Board(A, Color::Purple));
    f.tech(Color::Blue, 2, Zone::Hand(A), vec![]);
    let bottom = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let middle = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let top = f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    let four = f.tech(Color::Red, 4, Zone::AvailableAchievements, vec![]);
    f.tech(Color::Red, 3, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(A, Color::Blue), Splay::Right);
    assert_eq!(
        position.cards(Zone::Board(A, Color::Blue)),
        &[bottom, middle]
    );
    assert_eq!(position.cards(Zone::Safe(A)), &[top, four]);
}

#[test]
fn triad_needs_three_hand_cards_then_returns_splays_tucks_and_scores() {
    for cards in [3, 2] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U065", Zone::Board(A, Color::Purple));
        let red = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let blue = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let green = if cards == 3 {
            Some(f.tech(Color::Green, 1, Zone::Hand(A), vec![]))
        } else {
            None
        };
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        let red_top = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(
            &mut game,
            &mut events,
            &[red, blue, green.unwrap_or(red)],
        ));
        let position = game.position();
        if let Some(green) = green {
            assert!(position.cards(Zone::Deck(Deck::base(1))).contains(&red));
            assert_eq!(position.splay(A, Color::Red), Splay::Right);
            assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[blue]);
            assert_eq!(position.cards(Zone::Score(A)), &[green]);
            assert_eq!(position.top(A, Color::Red), Some(red_top));
        } else {
            assert_eq!(position.cards(Zone::Hand(A)), &[red, blue]);
            assert_eq!(position.splay(A, Color::Red), Splay::None);
        }
    }
}

#[test]
fn sabotage_reveals_the_whole_score_pile_including_von_brauns_junk() {
    // Wernher von Braun makes the junk part of B's score pile. The whole score pile is
    // revealed as the proof, junk included.
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U059", Zone::Board(A, Color::Yellow));
    let von_braun = figure_card(Figure::WernherVonBraun.code()).unwrap().color;
    f.figure(Figure::WernherVonBraun, Zone::Board(B, von_braun));
    let red_hand = f.tech(Color::Red, 2, Zone::Hand(B), vec![]);
    let red_top = f.tech(Color::Red, 4, Zone::Board(B, Color::Red), vec![]);
    let red_junk = f.tech(Color::Red, 3, Zone::Junk, vec![]);
    let green_junk = f.tech(Color::Green, 5, Zone::Junk, vec![]);
    let six = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN.union(Expansions::FIGURES)).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[red_hand]));
    let revealed: Vec<CardId> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert!(revealed.contains(&red_junk) && revealed.contains(&green_junk));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(B)), &[six]);
    assert_eq!(
        position.cards(Zone::Board(B, Color::Red)),
        &[red_junk, red_top]
    );
    assert_eq!(position.cards(Zone::Junk), &[green_junk]);
}
