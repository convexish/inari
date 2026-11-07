//! Unseen Age 9: one expected-outcome scenario per card (Iron Curtain lives
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
fn mccarthyism_melds_an_eight_and_eliminates_a_socialism_top() {
    for socialism in [false, true] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U087", Zone::Board(A, Color::Red));
        let eight = if socialism {
            f.add(base_card("B085").unwrap(), Zone::Deck(Deck::base(8)))
        } else {
            f.tech(Color::Red, 8, Zone::Deck(Deck::base(8)), vec![])
        };
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        if socialism {
            assert_eq!(status, Status::Winner(A));
        } else {
            complete(status);
            assert_eq!(game.position().top(B, Color::Red), Some(eight));
        }
    }

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U087", Zone::Board(A, Color::Red));
    let purple = f.tech(Color::Purple, 3, Zone::Board(A, Color::Purple), vec![]);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Score(A)), &[purple]);
    effect(&mut game, card, 2, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Up);
}

#[test]
fn magic_8_ball_grants_the_wish_on_a_concept_tuck_and_repeats_on_red_or_purple() {
    for wish in 0..3 {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U088", Zone::Board(A, Color::Yellow));
        let achievements = [
            f.tech(Color::Green, 1, Zone::AvailableAchievements, vec![]),
            f.tech(Color::Green, 2, Zone::AvailableAchievements, vec![]),
        ];
        let tens = [
            f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]),
            f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]),
        ];
        // Deck top first: a red Concept 8 grants the wish and repeats; a
        // plain blue 8 ends it. The second wish draws two green 8s first.
        let mut blue = None;
        if wish == 1 {
            blue = Some(f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]));
        }
        let scored = [
            f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]),
            f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]),
        ];
        let blue =
            blue.unwrap_or_else(|| f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]));
        let red = f.tech(Color::Red, 8, Zone::Deck(Deck::base(8)), vec![]);
        icons(&mut f, red, &[Icon::Concept]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 3);
        game.choose(wish).unwrap();
        // The repeat asks for the wish again (after any ordering question
        // of the wish itself); keep the same one.
        loop {
            let menu = decision(&mut game, &mut events);
            if menu.options == [0, 1, 2].map(Alternative::Branch) {
                game.choose(wish).unwrap();
                break;
            }
            game.choose(0).unwrap();
        }
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert_eq!(position.cards(Zone::Board(A, Color::Red)), &[red]);
        assert_eq!(position.cards(Zone::Board(A, Color::Blue)), &[blue]);
        match wish {
            0 => {
                let mut hand = position.cards(Zone::Hand(A)).to_vec();
                hand.sort();
                assert_eq!(hand, tens);
            }
            1 => {
                let mut score = position.cards(Zone::Score(A)).to_vec();
                score.sort();
                assert_eq!(score, scored);
            }
            _ => assert_eq!(position.cards(Zone::Safe(A)), &achievements),
        }
    }
}

#[test]
fn surveillance_wins_for_the_demander_when_both_hands_show_the_same_colors() {
    for (victim, demander, wins, expected_reveals) in [
        (
            vec![Color::Red, Color::Blue],
            vec![Color::Blue, Color::Red, Color::Red],
            true,
            3,
        ),
        (vec![Color::Red, Color::Blue], vec![Color::Red], false, 1),
        (vec![Color::Red], vec![Color::Red, Color::Blue], false, 1),
        (vec![], vec![Color::Red], false, 0),
    ] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U089", Zone::Board(A, Color::Yellow));
        for color in victim {
            f.tech(color, 3, Zone::Hand(B), vec![]);
        }
        for color in demander {
            f.tech(color, 4, Zone::Hand(A), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, B, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        let revealed = events
            .iter()
            .filter(|e| matches!(e, Event::Revealed { card, .. } if game.position().cards(Zone::Hand(A)).contains(card)))
            .count();
        if wins {
            assert_eq!(status, Status::Winner(A));
            assert_eq!(revealed, 3, "the whole demander hand proves the win");
        } else {
            complete(status);
            // One witness card, or the whole hand when the victim holds the
            // extra color; nothing when the victim's hand is empty.
            assert_eq!(revealed, expected_reveals, "{expected_reveals}");
        }
    }

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U089", Zone::Board(A, Color::Yellow));
    let ten = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[ten]);
}

#[test]
fn area_51_reveals_a_secret_in_place_and_super_executes_it_only_on_the_executors_turn() {
    for executor in [A, B] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U090", Zone::Board(executor, Color::Green));
        let secret = f.tech(Color::Red, 2, Zone::Safe(executor), vec![build::draw(3)]);
        let three = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 2, executor, executor);
        let mut events = Vec::new();
        complete(run(&mut game, &mut events, &[]));
        let position = game.position();
        assert_eq!(position.cards(Zone::Safe(executor)), &[secret]);
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Revealed { card, .. } if *card == secret
        )));
        if executor == A {
            assert_eq!(position.cards(Zone::Hand(A)), &[three]);
        } else {
            assert!(position.cards(Zone::Hand(B)).is_empty());
        }
    }

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U090", Zone::Board(A, Color::Green));
    f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    let one = f.tech(Color::Red, 1, Zone::AvailableAchievements, vec![]);
    let eleven = f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Green), Splay::Up);
    effect(&mut game, card, 1, A, A);
    decision(&mut game, &mut events);
    game.choose(0).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[eleven]);
    effect(&mut game, card, 1, A, A);
    decision(&mut game, &mut events);
    game.choose(1).unwrap();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Safe(A)), &[one]);
}

#[test]
fn teleprompter_runs_the_revealed_tops_non_demand_effects_in_turn_while_they_act() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U091", Zone::Board(A, Color::Green));
    let two = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    // Deck 1 holds one card: draw a [2], then (demand, skipped) then score
    // a hand card, then reveal only, which does not count as activity.
    let top = f.tech(
        Color::Red,
        1,
        Zone::Deck(Deck::base(1)),
        vec![build::draw(2)],
    );
    let effects = &mut f.cards[usize::from(top.0)].effects;
    effects.push(Effect {
        kind: EffectKind::Demand,
        instructions: vec![build::draw(5)],
    });
    effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![build::score(build::HAND)],
    });
    effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![build::reveal_all(build::SCORE)],
    });
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert!(position.cards(Zone::Hand(A)).is_empty());
    assert_eq!(position.cards(Zone::Score(A)), &[two]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), &[top]);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Revealed { card, .. } if *card == top))
            .count(),
        3
    );
}

#[test]
fn consulting_melds_two_tens_and_self_executes_the_opponents_pick_of_a_top() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U092", Zone::Board(A, Color::Blue));
    let red = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![build::draw(1)],
    );
    let one = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let green = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let purple = f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    let pick = decision(&mut game, &mut events);
    assert_eq!(pick.player, B);
    assert_eq!(pick.options.len(), 4, "{:?}", pick.options);
    game.choose(pick_known(&pick, red).unwrap()).unwrap();
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(A, Color::Purple), Some(purple));
    assert_eq!(position.top(A, Color::Green), Some(green));
    assert_eq!(position.cards(Zone::Hand(A)), &[one]);
}

#[test]
fn fermi_paradox_reveals_both_deck_tops_returns_one_and_wins_on_an_empty_board() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U093", Zone::Board(A, Color::Blue));
    let nine_under = f.tech(Color::Red, 9, Zone::Deck(Deck::base(9)), vec![]);
    let nine = f.tech(Color::Green, 9, Zone::Deck(Deck::base(9)), vec![]);
    let ten = f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[nine]));
    let position = game.position();
    for revealed in [nine, ten] {
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Revealed { card, .. } if *card == revealed))
        );
    }
    assert_eq!(
        position.cards(Zone::Deck(Deck::base(9))),
        &[nine, nine_under]
    );
    assert_eq!(position.cards(Zone::Deck(Deck::base(10))), &[ten]);

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U093", Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    assert_eq!(run(&mut game, &mut events, &[]), Status::Winner(A));

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U093", Zone::Board(A, Color::Blue));
    let junked = f.tech(Color::Red, 3, Zone::Junk, vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[junked]);
}

#[test]
fn clown_car_keeps_melding_from_the_demanders_score_pile_until_prosperity() {
    let mut f = Fixture::default();
    let card = unseen(&mut f, "U094", Zone::Board(A, Color::Purple));
    let plain = f.tech(Color::Red, 2, Zone::Score(A), vec![]);
    let prosperous = f.tech(Color::Blue, 4, Zone::Score(A), vec![]);
    icons(&mut f, prosperous, &[Icon::Prosperity]);
    let spare = f.tech(Color::Green, 6, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    // Blind picks by back: the 2 first, then the 4, which ends the loop.
    complete(run(&mut game, &mut events, &[]));
    let position = game.position();
    assert_eq!(position.top(B, Color::Red), Some(plain));
    assert_eq!(position.top(B, Color::Blue), Some(prosperous));
    assert_eq!(position.cards(Zone::Score(A)), &[spare]);
}

#[test]
fn urban_legend_draws_per_industry_color_and_wins_on_five() {
    for colors in [2, 5] {
        let mut f = Fixture::default();
        let card = unseen(&mut f, "U095", Zone::Board(A, Color::Purple));
        icons(&mut f, card, &[Icon::Industry]);
        for color in Color::ALL
            .into_iter()
            .filter(|&c| c != Color::Purple)
            .take(colors - 1)
        {
            let top = f.tech(color, 1, Zone::Board(A, color), vec![]);
            icons(&mut f, top, &[Icon::Industry]);
        }
        let nines: Vec<_> = (0..5)
            .map(|_| f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]))
            .collect();
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, card, 0, A, A);
        let mut events = Vec::new();
        let status = run(&mut game, &mut events, &[]);
        if colors == 5 {
            assert_eq!(status, Status::Winner(A));
        } else {
            complete(status);
            assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
        }
        let _ = nines;
    }

    let mut f = Fixture::default();
    let card = unseen(&mut f, "U095", Zone::Board(A, Color::Purple));
    f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    f.tech(Color::Yellow, 2, Zone::Board(A, Color::Yellow), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    complete(run(&mut game, &mut events, &[]));
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::Up);
}
