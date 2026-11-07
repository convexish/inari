//! Unseen Age 7: one expected-outcome scenario per card.

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
fn camouflage_junks_then_safeguards_a_pair_or_scores_two_secrets_and_draws_per_special() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U066", Zone::Board(A, Color::Red));
    let red = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    let blue = f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
    let claims = f.tech(
        Color::Purple,
        1,
        Zone::Hand(B),
        vec![
            build::claim(Special::Monument),
            build::claim(Special::Empire),
        ],
    );
    let sevens = [
        f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]),
        f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 2);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[red, blue]));
    let position = game.position();
    assert_eq!(position.top(A, Color::Red), Some(card));
    assert_eq!(position.top(A, Color::Blue), None);
    assert_eq!(position.cards(Zone::Safe(A)), &[red, blue]);
    assert!(position.cards(Zone::Junk).is_empty());

    effect(&mut game, claims, 0, A, A);
    complete(run(&mut game, &mut events, &[]));
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
    hand.sort();
    assert_eq!(hand, sevens);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U066", Zone::Board(A, Color::Red));
    let fours = [
        f.tech(Color::Red, 4, Zone::Safe(A), vec![]),
        f.tech(Color::Blue, 4, Zone::Safe(A), vec![]),
    ];
    let five = f.tech(Color::Green, 5, Zone::Safe(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let branch = decision(&mut game, &mut events);
    game.choose(1).unwrap();
    let _ = branch;
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[five]);
    let mut scored = position.cards(Zone::Score(A)).to_vec();
    scored.sort();
    assert_eq!(scored, fours);
}

#[test]
fn safe_deposit_box_junks_two_sevens_or_swaps_the_score_pile_with_the_junk() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U067", Zone::Board(A, Color::Red));
    let sevens = [
        f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]),
        f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]),
    ];
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let branch = decision(&mut game, &mut events);
    assert_eq!(branch.options.len(), 3, "{:?}", branch.options);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let mut junk = game.position().cards(Zone::Junk).to_vec();
    junk.sort();
    assert_eq!(junk, sevens);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U067", Zone::Board(A, Color::Red));
    let scored = f.tech(Color::Red, 3, Zone::Score(A), vec![]);
    let junked = f.tech(Color::Blue, 4, Zone::Junk, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(A)), &[junked]);
    assert_eq!(position.cards(Zone::Junk), &[scored]);
}

#[test]
fn mafia_takes_the_lowest_secret_tucks_from_any_score_pile_and_may_splay() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U068", Zone::Board(A, Color::Yellow));
    let high = f.tech(Color::Blue, 3, Zone::Safe(B), vec![]);
    let low = f.tech(Color::Red, 2, Zone::Safe(B), vec![]);
    let mine = f.tech(Color::Red, 2, Zone::Score(A), vec![]);
    let theirs = f.tech(Color::Blue, 5, Zone::Score(B), vec![]);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[low]);
    assert_eq!(position.cards(Zone::Safe(B)), &[high]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[theirs]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[theirs]);
    assert_eq!(position.cards(Zone::Score(A)), &[mine]);
    effect(&mut game, card, 2, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Right);
}

#[test]
fn subway_rewards_seven_visible_cards_or_junks_the_tucked_color() {
    for splayed in [true, false] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U069", Zone::Board(A, Color::Yellow));
        let blues: Vec<_> = (0..6)
            .map(|_| f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]))
            .collect();
        let seven = f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
        let eight = f.tech(Color::Red, 8, Zone::Deck(Deck::base(8)), vec![]);
        let nine = f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut position = f.position();
        if splayed {
            position = position.with_splay(A, Color::Blue, Splay::Up).unwrap();
        }
        let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        if splayed {
            assert_eq!(position.cards(Zone::Board(A, Color::Blue)).len(), 7);
            assert_eq!(position.cards(Zone::Hand(A)), &[nine]);
        } else {
            assert!(position.cards(Zone::Board(A, Color::Blue)).is_empty());
            let junk = position.cards(Zone::Junk);
            assert_eq!(junk.len(), 7);
            assert!(junk.contains(&seven) && blues.iter().all(|b| junk.contains(b)));
            assert_eq!(position.cards(Zone::Hand(A)), &[eight]);
        }
    }
}

#[test]
fn black_market_reveals_two_achievements_melds_a_plain_one_and_returns_the_rest() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U070", Zone::Board(A, Color::Green));
    let secret = f.tech(Color::Purple, 3, Zone::Hand(A), vec![]);
    let plain = f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]);
    let efficient = f.tech(Color::Red, 2, Zone::AvailableAchievements, vec![]);
    icons(&mut f, efficient, &[Icon::Efficiency]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[secret]);
    assert_eq!(position.top(A, Color::Green), Some(plain));
    assert_eq!(position.cards(Zone::Deck(Deck::base(2))), &[efficient]);
    assert!(position.cards(Zone::AvailableAchievements).is_empty());

    // Nothing to safeguard: the achievements stay untouched.
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U070", Zone::Board(A, Color::Green));
    let plain = f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::AvailableAchievements), &[plain]);
}

#[test]
fn plot_voucher_super_executes_on_the_executors_turn_and_self_executes_otherwise() {
    for executor in [A, B] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U071", Zone::Board(executor, Color::Green));
        // A demand and a non-demand effect: only a super-execution runs the
        // demand (neither mode shares).
        let melded = f.add(
            Card {
                code: "synthetic-demanding",
                set: CardSet::Base,
                color: Color::Red,
                value: 2,
                figure: None,
                echo: None,
                symbols: [Symbol::Blank; SLOTS],
                featured: Some(Icon::Authority),
                effects: vec![
                    Effect {
                        kind: EffectKind::Demand,
                        instructions: vec![build::draw(3)],
                    },
                    Effect {
                        kind: EffectKind::NonDemand,
                        instructions: vec![build::draw(2)],
                    },
                ],
            },
            Zone::Score(executor),
        );
        let one = f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]);
        f.tech(Color::Green, 3, Zone::AvailableAchievements, vec![]);
        f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
        f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
        // The fixture's turn belongs to A.
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, executor, executor);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert_eq!(position.top(executor, Color::Red), Some(melded));
        assert_eq!(position.cards(Zone::Safe(executor)), &[one]);
        let hands = (
            position.cards(Zone::Hand(A)).len(),
            position.cards(Zone::Hand(B)).len(),
        );
        if executor == A {
            // Super-execution: B suffers the demand, then A draws the 2.
            assert_eq!(hands, (1, 1));
        } else {
            // Self-execution on A's turn: B only draws the 2.
            assert_eq!(hands, (0, 1));
        }
    }
}

#[test]
fn counterintelligence_tucks_a_concept_top_then_takes_the_new_top_and_draws() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U072", Zone::Board(A, Color::Blue));
    let under = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let concept = f.tech(Color::Blue, 5, Zone::Board(B, Color::Blue), vec![]);
    icons(&mut f, concept, &[Icon::Concept]);
    f.tech(Color::Red, 4, Zone::Board(B, Color::Red), vec![]);
    let seven = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
    let eight = f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(B, Color::Blue)), &[concept]);
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[card, under]);
    assert_eq!(position.cards(Zone::Hand(B)), &[seven]);
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[eight]);
}

#[test]
fn private_eye_takes_the_demanders_pick_scores_a_secret_and_may_splay_blue() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U073", Zone::Board(A, Color::Blue));
    let keep = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    let taken = f.tech(Color::Green, 4, Zone::Hand(B), vec![]);
    let seven = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
    let secret = f.tech(Color::Purple, 2, Zone::Safe(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[taken]));
    let position = game.position();
    assert_eq!(position.cards(Zone::Hand(B)), &[keep, seven]);
    assert_eq!(position.top(A, Color::Green), Some(taken));
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)), &[secret]);
    effect(&mut game, card, 2, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
}

#[test]
fn fortune_cookie_rewards_exactly_seven_eight_or_nine_of_an_icon() {
    for count in [7, 8, 9] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U074", Zone::Board(A, Color::Purple));
        let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        icons(&mut f, red, &[Icon::Concept; 4]);
        f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        let green = f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        icons(&mut f, green, &[Icon::Concept; 4][..(count - 4).min(4)]);
        if count == 9 {
            let blue = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
            icons(&mut f, blue, &[Icon::Concept]);
        }
        let seven = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
        let eight = f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
        let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        match count {
            7 => {
                assert_eq!(position.cards(Zone::Score(A)), &[seven]);
                assert!(position.cards(Zone::Hand(A)).is_empty());
            }
            8 => {
                assert_eq!(position.splay(A, Color::Green), Splay::Right);
                assert_eq!(position.cards(Zone::Hand(A)), &[eight]);
            }
            _ => {
                assert_eq!(position.cards(Zone::Hand(A)), &[nine]);
                assert_eq!(position.splay(A, Color::Green), Splay::None);
            }
        }
    }
}

#[test]
fn fortune_cookie_counts_bonuses_as_one_icon_type_and_city_icons_as_their_own() {
    // Seven visible bonuses of mixed values: draw and score a [7].
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U074", Zone::Board(A, Color::Purple));
    let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    for (slot, value) in [3, 5, 5, 8].into_iter().enumerate() {
        f.cards[usize::from(red.0)].symbols[slot] = Symbol::Bonus(value);
    }
    let green = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    for slot in 0..3 {
        f.cards[usize::from(green.0)].symbols[slot] = Symbol::Bonus(2);
    }
    let seven = f.tech(Color::Yellow, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)), &[seven]);

    // Nine Plus icons, no other type at nine: draw a [9].
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U074", Zone::Board(A, Color::Purple));
    for color in [Color::Red, Color::Green, Color::Blue] {
        let city = f.tech(color, 1, Zone::Board(A, color), vec![]);
        for slot in 0..3 {
            f.cards[usize::from(city.0)].symbols[slot] = Symbol::City(CityIcon::Plus);
        }
    }
    let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[nine]);
}

#[test]
fn slot_machine_asks_no_splay_when_every_pile_would_stay_as_it_is() {
    for (purple_already_right, decisions) in [(true, 0), (false, 1)] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U075", Zone::Board(A, Color::Purple));
        f.tech(Color::Purple, 2, Zone::Board(A, Color::Purple), vec![]);
        f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        for value in 1..=5 {
            let color = if value == 1 { Color::Green } else { Color::Red };
            f.tech(color, value, Zone::Deck(Deck::base(value)), vec![]);
        }
        let mut position = f
            .position()
            .with_splay(A, Color::Green, Splay::Right)
            .unwrap();
        if purple_already_right {
            position = position.with_splay(A, Color::Purple, Splay::Right).unwrap();
        }
        let mut game = Game::new(position, Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let mut asked = 0;
        loop {
            match settle(&mut game, &mut events) {
                Status::Decision(menu) => {
                    asked += 1;
                    assert_eq!(menu.options.len(), 2, "{menu:?}");
                    game.choose(1).unwrap();
                }
                status => {
                    complete(status);
                    break;
                }
            }
        }
        assert_eq!(
            asked, decisions,
            "purple already right: {purple_already_right}"
        );
        assert_eq!(game.position().splay(A, Color::Green), Splay::Right);
        assert_eq!(game.position().splay(A, Color::Purple), Splay::Right);
    }
}

#[test]
fn slot_machine_splays_scores_and_wins_by_the_number_of_green_draws() {
    for greens in [0, 1, 3] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U075", Zone::Board(A, Color::Purple));
        f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
        f.tech(Color::Green, 2, Zone::Board(A, Color::Green), vec![]);
        let drawn: Vec<_> = (1..=5)
            .map(|value| {
                let color = if value <= greens {
                    Color::Green
                } else {
                    Color::Red
                };
                f.tech(color, value, Zone::Deck(Deck::base(value)), vec![])
            })
            .collect();
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        let position = game.position();
        assert_eq!(
            position.splay(A, Color::Green),
            if greens > 0 {
                Splay::Right
            } else {
                Splay::None
            }
        );
        if greens >= 3 {
            assert_eq!(status, Status::Winner(A));
            let mut scored = position.cards(Zone::Score(A)).to_vec();
            scored.sort();
            assert_eq!(scored, drawn);
        } else {
            complete(status);
            assert!(position.cards(Zone::Score(A)).is_empty());
            for (value, card) in (1..=5).zip(&drawn) {
                assert_eq!(position.cards(Zone::Deck(Deck::base(value))), &[*card]);
            }
        }
    }
}
