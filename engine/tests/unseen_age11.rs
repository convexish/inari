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
fn cloaking_moves_a_claimed_standard_achievement_into_the_demanders_safe() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U106", Zone::Board(A, Color::Red));
    let claimed = f.tech(Color::Blue, 3, Zone::Achievements(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[claimed]);
    assert!(position.cards(Zone::Achievements(B)).is_empty());
}

#[test]
fn dark_web_unsplays_any_board_and_bulk_safeguards_or_achieves() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U107", Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    f.tech(Color::Blue, 2, Zone::Board(B, Color::Blue), vec![]);
    let position = f
        .position()
        .with_splay(B, Color::Blue, Splay::Left)
        .unwrap();
    let mut events = Vec::new();
    // Any nonempty color on either board, splayed or not: unsplaying an unsplayed pile
    // changes nothing.
    for (player, color, left) in [(A, Color::Red, Splay::Left), (B, Color::Blue, Splay::None)] {
        let mut game = Game::new(position.clone(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let d = decision(&mut game, &mut events);
        // A's red and B's blue: an empty color is no target.
        assert_eq!(d.options.len(), 2);
        let option = d
            .options
            .iter()
            .position(|o| {
                *o == Alternative::Splay {
                    player,
                    color,
                    direction: Splay::None,
                }
            })
            .unwrap();
        game.choose(option).unwrap();
        complete(run(&mut game, &mut events, &[]));
        assert_eq!(game.position().splay(B, Color::Blue), left);
    }

    for branch in 0..2 {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U107", Zone::Board(A, Color::Red));
        let supply = [
            f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]),
            f.tech(Color::Green, 2, Zone::AvailableAchievements, vec![]),
        ];
        let secrets = [
            f.tech(Color::Blue, 8, Zone::Safe(A), vec![]),
            f.tech(Color::Blue, 9, Zone::Safe(A), vec![]),
        ];
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 1, A, A);
        decision(&mut game, &mut events);
        game.choose(branch).unwrap();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if branch == 0 {
            assert_eq!(position.cards(Zone::Safe(A)).len(), 4);
            assert!(position.cards(Zone::AvailableAchievements).is_empty());
        } else {
            let mut achieved = position.cards(Zone::Achievements(A)).to_vec();
            achieved.sort();
            assert_eq!(achieved, secrets);
            assert_eq!(position.cards(Zone::AvailableAchievements), &supply);
        }
    }
}

#[test]
fn escape_room_scores_a_revealed_eleven_and_a_matching_hand_card_or_eliminates() {
    for matching in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U108", Zone::Board(A, Color::Yellow));
        let hand = f.tech(
            if matching { Color::Blue } else { Color::Red },
            2,
            Zone::Hand(B),
            vec![],
        );
        let eleven = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        if matching {
            complete(status);
            assert_eq!(game.position().cards(Zone::Score(B)), &[eleven, hand]);
        } else {
            assert_eq!(status, Status::Winner(A));
            assert!(events.iter().any(|e| matches!(
                e,
                Event::Revealed { card, .. } if *card == hand
            )));
        }
    }

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U108", Zone::Board(A, Color::Yellow));
    let mut efficient = Vec::new();
    for color in [Color::Red, Color::Blue, Color::Green, Color::Purple] {
        let top = f.tech(color, 1, Zone::Board(A, color), vec![]);
        icons(&mut f, top, &[Icon::Efficiency]);
        efficient.push(top);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    let mut score = position.cards(Zone::Score(A)).to_vec();
    score.sort();
    assert_eq!(score, efficient);
    assert_eq!(position.top(A, Color::Yellow), Some(card));
}

#[test]
fn fashion_mask_tucks_one_marked_top_per_color_then_scores_and_splays_aslant() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U109", Zone::Board(A, Color::Yellow));
    let red_under = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let red = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    icons(&mut f, red, &[Icon::Prosperity]);
    let blue = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    icons(&mut f, blue, &[Icon::Industry]);
    let green = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[red]));
    let position = game.position();
    // Red tucked under its own pile, then safeguarded (first option).
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red_under]);
    assert_eq!(position.cards(Zone::Safe(A)), &[red]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[blue]);
    assert_eq!(position.top(A, Color::Green), Some(green));

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U109", Zone::Board(A, Color::Yellow));
    let yellows: Vec<_> = (0..6)
        .map(|_| f.tech(Color::Yellow, 2, Zone::Board(A, Color::Yellow), vec![]))
        .collect();
    f.tech(Color::Purple, 2, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Purple, 3, Zone::Board(A, Color::Purple), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    // Seven yellow cards: the bottom two score; purple has nothing below
    // its top five.
    let mut score = position.cards(Zone::Score(A)).to_vec();
    score.sort();
    assert_eq!(score, vec![card, yellows[0]]);
    assert_eq!(position.splay(A, Color::Yellow), Splay::Aslant);
    assert_eq!(position.splay(A, Color::Purple), Splay::Aslant);
}

#[test]
fn green_hydrogen_scores_covered_greens_and_tucks_an_eleven_each() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U110", Zone::Board(A, Color::Green));
    let covered = [
        f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]),
        f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]),
    ];
    let top = f.tech(Color::Green, 5, Zone::Board(A, Color::Green), vec![]);
    // Green Hydrogen itself is covered too: three cards score.
    let elevens = [
        f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]),
        f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]),
        f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    let mut score = position.cards(Zone::Score(A)).to_vec();
    score.sort();
    assert_eq!(score, vec![card, covered[0], covered[1]]);
    assert_eq!(position.cards(Zone::Board(A, Color::Green)), &[top]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[elevens[1]]);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[elevens[0]]);
    assert_eq!(position.cards(Zone::Board(A, Color::Purple)), &[elevens[2]]);
}

/// Each [11] is tucked before the next draw: when the deck runs out the game
/// ends with the earlier draws already tucked, none left in hand.
#[test]
fn green_hydrogen_tucks_each_eleven_before_an_exhausted_draw_ends_the_game() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U110", Zone::Board(A, Color::Green));
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    f.tech(Color::Green, 5, Zone::Board(A, Color::Green), vec![]);
    let red = f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let blue = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let status = run(&mut game, &mut events, &[]);
    assert!(!matches!(status, Status::Complete(_)), "{status:?}");
    let position = game.position();
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[blue]);
}

#[test]
fn mystery_box_claims_without_eligibility_reveals_only_that_card_and_executes_it() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U111", Zone::Board(A, Color::Green));
    let prize = f.tech(
        Color::Red,
        9,
        Zone::AvailableAchievements,
        vec![build::draw(1)],
    );
    let other = f.tech(Color::Blue, 1, Zone::AvailableAchievements, vec![]);
    let one = f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    // Achievements are chosen by back: the 9 is the second value.
    assert_eq!(menu.options.len(), 2, "{:?}", menu.options);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Achievements(A)), &[prize]);
    assert_eq!(position.cards(Zone::AvailableAchievements), &[other]);
    assert_eq!(position.cards(Zone::Hand(A)), &[one]);
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, vec![prize]);
}

#[test]
fn astrobiology_returns_a_bottom_splays_aslant_and_scores_the_healthless_rest() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U112", Zone::Board(A, Color::Blue));
    let bottom = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let healthy = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    icons(&mut f, healthy, &[Icon::Health]);
    let plain = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    let top = f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[bottom]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[bottom]);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[healthy]);
    let mut score = position.cards(Zone::Score(A)).to_vec();
    score.sort();
    assert_eq!(score, vec![plain, top]);
}

#[test]
fn holography_scores_below_five_splays_aslant_and_swaps_low_scores_for_lower_achievements() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U114", Zone::Board(A, Color::Purple));
    let reds: Vec<_> = (0..6)
        .map(|_| f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]))
        .collect();
    let low_claimed = f.tech(Color::Blue, 1, Zone::Achievements(A), vec![]);
    let high_claimed = f.tech(Color::Blue, 7, Zone::Achievements(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let colors = decision(&mut game, &mut events);
    assert_eq!(colors.options.len(), 3);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.splay(A, Color::Red), Splay::Aslant);
    assert_eq!(position.cards(Zone::Board(A, Color::Red)), &reds[1..]);
    // The scored red 4 swaps with the claimed 1; the 7 stays claimed.
    assert_eq!(position.cards(Zone::Score(A)), &[low_claimed]);
    let mut claimed = position.cards(Zone::Achievements(A)).to_vec();
    claimed.sort();
    assert_eq!(claimed, vec![reds[0], high_claimed]);
}

#[test]
fn metaverse_scores_each_splayed_top_and_eliminates_under_three() {
    for splayed in [3, 2] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U115", Zone::Board(A, Color::Purple));
        let mut tops = Vec::new();
        let mut position = {
            for color in [Color::Red, Color::Blue, Color::Green]
                .into_iter()
                .take(splayed)
            {
                f.tech(color, 1, Zone::Board(A, color), vec![]);
                tops.push(f.tech(color, 2, Zone::Board(A, color), vec![]));
            }
            f.position()
        };
        for color in [Color::Red, Color::Blue, Color::Green]
            .into_iter()
            .take(splayed)
        {
            position = position.with_splay(A, color, Splay::Up).unwrap();
        }
        let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        if splayed == 3 {
            complete(status);
            let mut score = game.position().cards(Zone::Score(A)).to_vec();
            score.sort();
            assert_eq!(score, tops);
        } else {
            // Elimination junks the loser's cards, scored tops included.
            assert_eq!(status, Status::Winner(B));
            let junk = game.position().cards(Zone::Junk);
            assert!(tops.iter().all(|top| junk.contains(top)), "{junk:?}");
        }
    }
}

#[test]
fn quantum_computers_eliminates_on_a_lost_flip_and_pays_secrets_for_lost_flips() {
    let mut outcomes = [false, false];
    for seed in 0..32 {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U113", Zone::Board(A, Color::Blue));
        let mut game = Game::seeded(f.position(), Expansions::UNSEEN, seed).unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        let flips: Vec<bool> = events
            .iter()
            .filter_map(|e| match e {
                Event::Flipped { executor, won } => {
                    assert_eq!(*executor, B);
                    Some(*won)
                }
                _ => None,
            })
            .collect();
        assert_eq!(flips.len(), 1);
        if flips[0] {
            complete(status);
        } else {
            assert_eq!(status, Status::Winner(A));
        }
        outcomes[usize::from(flips[0])] = true;
    }
    assert_eq!(outcomes, [true, true], "both coin results occur");

    let mut outcomes = [false, false];
    for seed in 0..32 {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U113", Zone::Board(A, Color::Blue));
        let secrets = [
            f.tech(Color::Red, 1, Zone::Safe(A), vec![]),
            f.tech(Color::Green, 2, Zone::Safe(A), vec![]),
        ];
        let mut game = Game::seeded(f.position(), Expansions::UNSEEN, seed).unwrap();
        effect(&mut game, card, 1, A, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        let flips: Vec<bool> = events
            .iter()
            .filter_map(|e| match e {
                Event::Flipped { won, .. } => Some(*won),
                _ => None,
            })
            .collect();
        let lost = flips.iter().filter(|&&won| !won).count();
        // Every flip but the last is lost and paid with a secret; the last
        // is either won or lost with no secret left.
        assert!(
            flips[..flips.len() - 1].iter().all(|&won| !won),
            "{flips:?}"
        );
        if *flips.last().unwrap() {
            complete(status);
            assert_eq!(game.position().cards(Zone::Safe(A)).len(), 2 - lost);
            outcomes[0] = true;
        } else {
            assert_eq!(lost, 3, "{flips:?}");
            assert_eq!(status, Status::Winner(B));
            for secret in secrets {
                assert!(
                    game.position()
                        .cards(Zone::Deck(Deck::base(game.position().value(secret))))
                        .contains(&secret)
                        || game.position().cards(Zone::Junk).contains(&secret)
                );
            }
            outcomes[1] = true;
        }
    }
    assert_eq!(outcomes, [true, true], "both endings occur");
}
