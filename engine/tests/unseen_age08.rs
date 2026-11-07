//! Unseen Age 8: one expected-outcome scenario per card.

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

fn run_declining(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = preferred
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .or_else(|| menu.options.iter().position(|o| *o == Alternative::Decline))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("did not terminate");
}

#[test]
fn concealment_tucks_every_secret_and_safeguards_the_bottom_purple() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U076", Zone::Board(A, Color::Red));
    let red = f.tech(Color::Red, 2, Zone::Safe(B), vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Safe(B), vec![]);
    let red_top = f.tech(Color::Red, 5, Zone::Board(B, Color::Red), vec![]);
    let bottom = f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    let top = f.tech(Color::Purple, 4, Zone::Board(A, Color::Purple), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert!(position.cards(Zone::Safe(B)).is_empty());
    assert_eq!(position.cards(Zone::Board(B, Color::Red)), &[red, red_top]);
    assert_eq!(position.cards(Zone::Board(B, Color::Blue)), &[blue]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[bottom]);
    assert_eq!(position.cards(Zone::Board(A, Color::Purple)), &[top]);
}

#[test]
fn enigma_machine_offers_three_bulk_moves_and_resplays_a_left_color_up() {
    for branch in 0..3 {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U077", Zone::Board(A, Color::Red));
        let achievement = f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]);
        let secret = f.tech(Color::Blue, 2, Zone::Safe(A), vec![]);
        let hand = f.tech(Color::Yellow, 3, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 3);
        game.choose(branch).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        match branch {
            0 => assert_eq!(position.cards(Zone::Safe(A)), &[secret, achievement]),
            1 => {
                assert!(position.cards(Zone::Safe(A)).is_empty());
                assert_eq!(position.cards(Zone::Hand(A)), &[hand, secret]);
            }
            _ => {
                assert!(position.cards(Zone::Hand(A)).is_empty());
                assert_eq!(
                    position.cards(Zone::AvailableAchievements),
                    &[achievement, hand]
                );
            }
        }
    }

    // With room for one of two achievements, the player orders the batch:
    // the chosen back enters and the other stays available.
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U077", Zone::Board(A, Color::Red));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let one = f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]);
    let three = f.tech(Color::Green, 3, Zone::AvailableAchievements, vec![]);
    let secret = f.tech(Color::Blue, 2, Zone::Safe(A), vec![]);
    let position = f.position().with_splay(A, Color::Red, Splay::Up).unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    assert_eq!(game.position().safe_limit(A), 2);
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    let order = decision(&mut game, &mut events);
    let pick = order
        .options
        .iter()
        .position(|option| {
            matches!(option,
            Alternative::Move { target: TargetDescriptor::Hidden { back, .. }, .. }
            | Alternative::BlindMove { back, .. } if back.age == 3)
        })
        .unwrap_or_else(|| panic!("{:?}", order.options));
    game.choose(pick).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[secret, three]);
    assert_eq!(position.cards(Zone::AvailableAchievements), &[one]);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U077", Zone::Board(A, Color::Red));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    let position = f
        .position()
        .with_splay(A, Color::Red, Splay::Right)
        .unwrap()
        .with_splay(A, Color::Blue, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    // Only blue is splayed left: no question.
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(A, Color::Blue), Splay::Up);
    assert_eq!(position.splay(A, Color::Red), Splay::Right);
    assert_eq!(position.splay(A, Color::Green), Splay::None);
}

#[test]
fn jackalope_takes_the_highest_plain_top_and_unsplays_the_most_visible_color() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U078", Zone::Board(A, Color::Yellow));
    let efficient = f.tech(Color::Red, 9, Zone::Board(B, Color::Red), vec![]);
    icons(&mut f, efficient, &[Icon::Efficiency]);
    f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let highest = f.tech(Color::Blue, 6, Zone::Board(B, Color::Blue), vec![]);
    f.tech(Color::Green, 3, Zone::Board(B, Color::Green), vec![]);
    let position = f
        .position()
        .with_splay(B, Color::Blue, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(A, Color::Blue), Some(highest));
    assert_eq!(position.splay(B, Color::Blue), Splay::None);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U078", Zone::Board(A, Color::Yellow));
    for _ in 0..3 {
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    }
    for _ in 0..4 {
        f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    }
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    // Red shows three cards, blue only its top, green two.
    let position = f
        .position()
        .with_splay(A, Color::Red, Splay::Right)
        .unwrap()
        .with_splay(A, Color::Green, Splay::Up)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(A, Color::Red), Splay::None);
    assert_eq!(position.splay(A, Color::Green), Splay::Up);
}

#[test]
fn shangri_la_scores_an_industry_tuck_or_melds_and_repeats_on_an_eight() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U079", Zone::Board(A, Color::Yellow));
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    let ten = f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]);
    let nine = f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
    // Deck top first: a plain 8 is tucked, the next 8 melds and repeats;
    // the third 8 has Industry and is scored, which repeats again: the 9
    // is tucked and the 10 melded, and a 10 ends the loop.
    let industry = f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]);
    icons(&mut f, industry, &[Icon::Industry]);
    let melded = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
    let tucked = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Green))[0], tucked);
    assert_eq!(position.top(A, Color::Purple), Some(melded));
    assert_eq!(position.cards(Zone::Score(A)), &[industry]);
    assert!(position.cards(Zone::Board(A, Color::Blue)).is_empty());
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[nine]);
    assert_eq!(position.top(A, Color::Yellow), Some(ten));
    assert!(position.cards(Zone::Hand(A)).is_empty());
}

#[test]
fn handbag_offers_bottoms_to_hand_score_tucks_or_scoring_a_value() {
    for branch in [Some(0), Some(1), Some(2), None] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U080", Zone::Board(A, Color::Green));
        let red_bottom = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        let red_top = f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
        // Handbag itself is the bottom green card.
        let green_top = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        let scored = f.tech(Color::Blue, 3, Zone::Score(A), vec![]);
        let threes = [
            f.tech(Color::Yellow, 3, Zone::Hand(A), vec![]),
            f.tech(Color::Purple, 3, Zone::Hand(A), vec![]),
        ];
        let five = f.tech(Color::Purple, 5, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 4, "{:?}", menu.options);
        game.choose(branch.unwrap_or(3)).unwrap();
        if branch == Some(2) {
            let values = decision(&mut game, &mut events);
            assert_eq!(
                values.options,
                vec![Alternative::Value(3), Alternative::Value(5)]
            );
            game.choose(0).unwrap();
        }
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        match branch {
            Some(0) => {
                assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red_top]);
                assert_eq!(position.cards(Zone::Board(A, Color::Green)), &[green_top]);
                let hand = position.cards(Zone::Hand(A));
                assert!(hand.contains(&red_bottom) && hand.contains(&card));
                assert_eq!(hand.len(), 5);
            }
            Some(1) => {
                assert!(position.cards(Zone::Score(A)).is_empty());
                assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[scored]);
            }
            Some(2) => {
                let mut score = position.cards(Zone::Score(A)).to_vec();
                score.sort();
                assert_eq!(score, vec![scored, threes[0], threes[1]]);
                assert_eq!(position.cards(Zone::Hand(A)), &[five]);
            }
            _ => {
                assert_eq!(position.cards(Zone::Hand(A)).len(), 3);
                assert_eq!(position.cards(Zone::Score(A)), &[scored]);
            }
        }
    }
}

#[test]
fn hitchhiking_lets_the_donor_decide_inside_the_recipients_self_execution() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U081", Zone::Board(A, Color::Green));
    // The donated card scores a card from the executor's hand: the donor
    // picks which one, seeing A's hand.
    let gift = f.tech(
        Color::Red,
        3,
        Zone::Hand(B),
        vec![build::score(build::HAND)],
    );
    let other = f.tech(Color::Blue, 1, Zone::Hand(B), vec![]);
    let low = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let high = f.tech(Color::Yellow, 5, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let donate = decision(&mut game, &mut events);
    assert_eq!(donate.player, B);
    game.choose(pick_known(&donate, gift).unwrap()).unwrap();
    let pick = decision(&mut game, &mut events);
    assert_eq!(pick.player, B);
    assert!(pick_known(&pick, low).is_some() && pick_known(&pick, high).is_some());
    game.choose(pick_known(&pick, low).unwrap()).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(A, Color::Red), Some(gift));
    assert_eq!(position.cards(Zone::Hand(B)), &[other]);
    assert_eq!(position.cards(Zone::Score(A)), &[low]);
    assert_eq!(position.cards(Zone::Hand(A)), &[high]);
}

#[test]
fn blacklight_unsplays_a_color_or_splays_one_up_and_draws() {
    // Either branch takes any nonempty color its splay may not change: the first
    // unsplays any, the second splays up any unsplayed one, a lone card included. Piles
    // the splay would leave as they are (unsplayed green, the lone blue) are one no-op
    // option, the first of them.
    for (branch, color, options) in [
        (0, Color::Red, 2),
        (0, Color::Blue, 2),
        (1, Color::Green, 2),
        (1, Color::Blue, 2),
    ] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U082", Zone::Board(A, Color::Blue));
        f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
        let position = f.position().with_splay(A, Color::Red, Splay::Left).unwrap();
        let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        decision(&mut game, &mut events);
        game.choose(branch).unwrap();
        let direction = if branch == 0 { Splay::None } else { Splay::Up };
        let d = decision(&mut game, &mut events);
        assert_eq!(d.options.len(), options);
        let option = d
            .options
            .iter()
            .position(|o| {
                *o == Alternative::Splay {
                    player: A,
                    color,
                    direction,
                }
            })
            .unwrap();
        game.choose(option).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        let red = if (branch, color) == (0, Color::Red) {
            Splay::None
        } else {
            Splay::Left
        };
        assert_eq!(position.splay(A, Color::Red), red);
        let green = if (branch, color) == (1, Color::Green) {
            Splay::Up
        } else {
            Splay::None
        };
        assert_eq!(position.splay(A, Color::Green), green);
        let drawn: &[CardId] = if branch == 0 { &[] } else { &[nine] };
        assert_eq!(position.cards(Zone::Hand(A)), drawn);
    }
}

#[test]
fn blacklight_draws_a_nine_with_no_unsplayed_color() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U082", Zone::Board(A, Color::Blue));
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let position = f
        .position()
        .with_splay(A, Color::Blue, Splay::Left)
        .unwrap();
    let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Left);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[nine]);
}

#[test]
fn scouting_returns_at_least_one_nine_and_rewards_a_matching_ten_deck_top() {
    for (return_both, matches) in [(true, true), (true, false), (false, true)] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U083", Zone::Board(A, Color::Blue));
        let ten = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
        let top = f.tech(
            if matches { Color::Red } else { Color::Purple },
            10,
            Zone::Deck(Deck::base(10)),
            vec![],
        );
        let blue = f.tech(Color::Blue, 9, Zone::Deck(Deck::base(9)), vec![]);
        let red = f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let status = if return_both {
            run(&mut game, &mut events, &[red, blue])
        } else {
            run_declining(&mut game, &mut events, &[red])
        };
        complete(status);
        let position = game.position();
        let hand = position.cards(Zone::Hand(A));
        if !return_both {
            assert_eq!(hand, &[blue]);
            assert_eq!(position.cards(Zone::Deck(Deck::base(10))), &[ten, top]);
        } else if matches {
            assert_eq!(hand, &[top]);
        } else {
            assert!(hand.is_empty());
            assert!(events.iter().any(|event| matches!(
                event,
                Event::Revealed { card, .. } if *card == top
            )));
        }
    }
}

#[test]
fn joy_buzzer_swaps_the_hand_for_the_demanders_lowest_and_scores_a_value_then_purple() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U084", Zone::Board(A, Color::Purple));
    let victim = f.tech(Color::Red, 7, Zone::Hand(B), vec![]);
    let lows = [
        f.tech(Color::Blue, 2, Zone::Hand(A), vec![]),
        f.tech(Color::Green, 2, Zone::Hand(A), vec![]),
    ];
    let high = f.tech(Color::Yellow, 5, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    let mut theirs = position.cards(Zone::Hand(B)).to_vec();
    theirs.sort();
    assert_eq!(theirs, lows);
    assert_eq!(position.cards(Zone::Hand(A)), &[high, victim]);

    effect(&mut game, card, 1, A, A);
    let values = decision(&mut game, &mut events);
    assert_eq!(
        values.options,
        vec![
            Alternative::Value(5),
            Alternative::Value(7),
            Alternative::Decline
        ]
    );
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(A)), &[victim, card]);
    assert_eq!(position.cards(Zone::Hand(A)), &[high]);
}

#[test]
fn opus_dei_reveals_the_highest_score_card_splays_its_color_up_and_draws_per_up_splay() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U085", Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 2, Zone::Score(A), vec![]);
    let highest = f.tech(Color::Blue, 6, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    let eight = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(A, Color::Blue), Splay::Up);
    assert_eq!(position.cards(Zone::Safe(A)), &[highest]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[eight]);
}
