//! Unseen execution integration. Hitchhiking, Plot Voucher,
//! Propaganda, Consulting, Area 51 and Mystery Box keep the executor,
//! demander, controller, performer and source zone apart across nested
//! executions, and terminal results inside them end the game at once.

use inno_engine::*;

mod common;
use common::*;

fn unseen(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(unseen_card(code).unwrap(), zone)
}

fn effect(game: &mut Game, card: CardId, index: u16, executor: Player) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        Context::new(executor),
    )
    .unwrap();
}

fn known(option: &Alternative) -> Option<CardId> {
    match option {
        Alternative::Select {
            target: TargetDescriptor::Known { card, .. },
        }
        | Alternative::Move {
            target: TargetDescriptor::Known { card, .. },
            ..
        }
        | Alternative::Reveal {
            target: TargetDescriptor::Known { card, .. },
        } => Some(*card),
        _ => None,
    }
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| known(option) == Some(card))
}

/// A hidden card offered by its back: the first option showing `age`.
fn pick_back(menu: &Decision, age: u8) -> Option<usize> {
    menu.options.iter().position(|option| match option {
        Alternative::Select {
            target: TargetDescriptor::Hidden { back, .. },
        }
        | Alternative::Move {
            target: TargetDescriptor::Hidden { back, .. },
            ..
        }
        | Alternative::Reveal {
            target: TargetDescriptor::Hidden { back, .. },
        }
        | Alternative::BlindMove { back, .. }
        | Alternative::BlindSelect { back, .. } => back.age == age,
        _ => false,
    })
}

fn pick_color(menu: &Decision, color: Color) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << color as u8))
}

/// Run to the end, letting `decide` answer each menu and recording who
/// decided it.
fn play(
    game: &mut Game,
    events: &mut Vec<Event>,
    deciders: &mut Vec<Player>,
    mut decide: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                deciders.push(menu.player);
                let option = decide(&menu);
                game.choose(option).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return status;
            }
        }
    }
    panic!("did not terminate");
}

fn complete(status: Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

fn elevens(f: &mut Fixture, count: usize) -> Vec<CardId> {
    (0..count)
        .map(|i| f.tech(Color::ALL[i % 5], 11, Zone::Deck(Deck::base(11)), vec![]))
        .collect()
}

fn movements(events: &[Event]) -> Vec<(CardId, Player, Zone, Zone)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) => Some((m.card, m.executor, m.source, m.destination)),
            _ => None,
        })
        .collect()
}

fn all_known(menu: &Decision) -> bool {
    menu.options.iter().all(|option| known(option).is_some())
}

fn trackers_hold(game: &Game) {
    let position = game.position();
    for tracker in Player::ALL
        .into_iter()
        .filter_map(|p| game.tracker(p))
        .chain(game.public_tracker())
    {
        assert!(tracker.check_truth(position));
    }
    for viewer in Player::ALL {
        for seed in 0..4 {
            game.determinize(viewer, seed).unwrap();
        }
    }
}

/// Hitchhiking self-executes the donated Plot
/// Voucher, which super-executes Propaganda on the executor's turn (or
/// self-executes it on the opponent's). The donor decides everything,
/// including Propaganda's demander-assigned color, sees the executor's hand
/// and score, and performs only the donation and the demand's victim moves.
/// "You" stays the executor: their meld, safeguard, Chain and board.
#[test]
fn hitchhiking_plot_voucher_propaganda_keep_executor_demander_and_controller_apart() {
    for executor in [A, B] {
        let donor = executor.opponent();
        let mut f = Fixture::default();
        let hitchhiking = unseen(&mut f, "U081", Zone::Board(executor, Color::Green));
        let voucher = unseen(&mut f, "U071", Zone::Hand(donor));
        let kept = f.tech(Color::Blue, 3, Zone::Hand(donor), vec![]);
        let propaganda = unseen(&mut f, "U024", Zone::Score(executor));
        let other_score = f.tech(Color::Green, 1, Zone::Score(executor), vec![]);
        let one = f.tech(Color::Yellow, 1, Zone::AvailableAchievements, vec![]);
        f.tech(Color::Yellow, 3, Zone::AvailableAchievements, vec![]);
        let eleven = elevens(&mut f, 1)[0];
        // Propaganda's victim (the donor) holds one red card over a red pile.
        let under = f.tech(Color::Red, 1, Zone::Board(donor, Color::Red), vec![]);
        let red = f.tech(Color::Red, 2, Zone::Hand(donor), vec![]);
        // The executor's hand for Propaganda's own meld.
        let green = f.tech(Color::Green, 1, Zone::Hand(executor), vec![]);
        let yellow = f.tech(Color::Yellow, 1, Zone::Hand(executor), vec![]);
        let voucher_color = f.cards[usize::from(voucher.0)].color;
        let propaganda_color = f.cards[usize::from(propaganda.0)].color;
        // The fixture's turn belongs to A.
        let mut game = Game::new(f.position(), Expansions::UNSEEN)
            .unwrap()
            .with_tracking()
            .unwrap();
        effect(&mut game, hitchhiking, 0, executor);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        let mut saw_hand = false;
        let status = play(&mut game, &mut events, &mut deciders, |menu| {
            if let Some(index) = pick_known(menu, green) {
                // The donor sees the executor's whole hand while deciding.
                assert!(all_known(menu), "{:?}", menu.options);
                saw_hand = true;
                return index;
            }
            if let Some(index) = pick_known(menu, voucher).or(pick_known(menu, propaganda)) {
                return index;
            }
            pick_color(menu, Color::Red).unwrap_or_else(|| panic!("{:?}", menu.options))
        });
        complete(status);
        assert!(saw_hand, "executor {executor:?}");
        assert!(
            deciders.iter().all(|&player| player == donor),
            "{deciders:?}"
        );
        assert_eq!(deciders.len(), if executor == A { 4 } else { 3 });

        let position = game.position();
        // Plot Voucher is green too: the final meld covers it.
        assert!(
            position
                .cards(Zone::Board(executor, voucher_color))
                .contains(&voucher)
        );
        assert_eq!(position.top(executor, propaganda_color), Some(propaganda));
        assert_eq!(position.cards(Zone::Score(executor)), &[other_score]);
        assert_eq!(position.cards(Zone::Safe(executor)), &[one]);
        // Plot Voucher's keyword inside Hitchhiking's self-execution earns
        // the executor's Chain, never the controller's.
        assert_eq!(position.cards(Zone::Achievements(executor)), &[eleven]);
        assert!(position.cards(Zone::Achievements(donor)).is_empty());
        assert_eq!(position.top(executor, Color::Green), Some(green));
        assert_eq!(position.cards(Zone::Hand(executor)), &[yellow]);
        let moves = movements(&events);
        let of = |card: CardId| moves.iter().find(|m| m.0 == card).copied().unwrap();
        assert_eq!(
            of(voucher),
            (
                voucher,
                donor,
                Zone::Hand(donor),
                Zone::Board(executor, voucher_color)
            )
        );
        assert_eq!(of(propaganda).1, executor);
        assert_eq!(
            of(one),
            (
                one,
                executor,
                Zone::AvailableAchievements,
                Zone::Safe(executor)
            )
        );
        assert_eq!(of(eleven).1, executor);
        assert_eq!(of(green).1, executor);
        if executor == A {
            // Super-execution on the executor's turn ran the demand: the
            // victim melded the controller-chosen color and handed the card
            // beneath to the demander's board.
            assert_eq!(position.cards(Zone::Hand(donor)), &[kept]);
            assert_eq!(position.cards(Zone::Board(donor, Color::Red)), &[red]);
            assert_eq!(position.top(executor, Color::Red), Some(under));
            assert_eq!(
                of(red),
                (
                    red,
                    donor,
                    Zone::Hand(donor),
                    Zone::Board(donor, Color::Red)
                )
            );
            assert_eq!(
                of(under),
                (
                    under,
                    donor,
                    Zone::Board(donor, Color::Red),
                    Zone::Board(executor, Color::Red)
                )
            );
        } else {
            // Self-execution on the opponent's turn skips the demand.
            assert_eq!(position.cards(Zone::Hand(donor)), &[kept, red]);
            assert_eq!(position.cards(Zone::Board(donor, Color::Red)), &[under]);
            assert!(position.top(executor, Color::Red).is_none());
        }
        trackers_hold(&game);
    }
}

/// Consulting's opponent picks the top card but does not control its
/// execution; inside Hitchhiking the override persists through the pick,
/// and a winning top ends the game before anything else.
#[test]
fn consulting_pick_grants_no_control_but_an_outer_override_persists() {
    let setup = |f: &mut Fixture, program: Vec<Instruction>| {
        let red = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), program);
        let low = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
        let high = f.tech(Color::Yellow, 5, Zone::Hand(A), vec![]);
        f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
        f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
        (red, low, high)
    };

    // Plain: B picks, then A decides inside the self-executed card.
    let mut f = Fixture::default();
    let consulting = unseen(&mut f, "U092", Zone::Board(A, Color::Blue));
    let (red, low, high) = setup(&mut f, vec![build::meld(build::HAND)]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, consulting, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    let status = play(&mut game, &mut events, &mut deciders, |menu| {
        pick_known(menu, red)
            .or(pick_known(menu, low))
            .unwrap_or_else(|| panic!("{:?}", menu.options))
    });
    complete(status);
    assert_eq!(deciders, [B, A]);
    assert_eq!(game.position().top(A, Color::Yellow), Some(low));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[high]);

    // Under Hitchhiking: B donates Consulting, picks the top and still
    // decides inside it, seeing A's hand; A performs every move.
    let mut f = Fixture::default();
    let hitchhiking = unseen(&mut f, "U081", Zone::Board(A, Color::Green));
    let consulting = unseen(&mut f, "U092", Zone::Hand(B));
    let other = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
    let (red, low, high) = setup(&mut f, vec![build::meld(build::HAND)]);
    let eleven = elevens(&mut f, 1)[0];
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    effect(&mut game, hitchhiking, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    let status = play(&mut game, &mut events, &mut deciders, |menu| {
        if let Some(index) = pick_known(menu, low) {
            assert!(all_known(menu), "{:?}", menu.options);
            return index;
        }
        pick_known(menu, red)
            .or(pick_known(menu, consulting))
            .unwrap_or_else(|| panic!("{:?}", menu.options))
    });
    complete(status);
    assert_eq!(deciders, [B, B, B]);
    let position = game.position();
    assert_eq!(position.top(A, Color::Yellow), Some(low));
    assert_eq!(position.cards(Zone::Hand(A)), &[high]);
    assert_eq!(position.cards(Zone::Hand(B)), &[other]);
    assert_eq!(position.cards(Zone::Achievements(A)), &[eleven]);
    assert!(position.cards(Zone::Score(A)).is_empty());
    assert!(
        movements(&events)
            .iter()
            .filter(|m| m.0 != consulting)
            .all(|m| m.1 == A)
    );
    trackers_hold(&game);

    // A winning top ends the game at the win; nothing follows.
    let mut f = Fixture::default();
    let consulting = unseen(&mut f, "U092", Zone::Board(A, Color::Blue));
    let (red, low, high) = setup(
        &mut f,
        vec![
            Instruction::Win(WinRule::Executor),
            build::score(build::HAND),
        ],
    );
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, consulting, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    let status = play(&mut game, &mut events, &mut deciders, |menu| {
        pick_known(menu, red).unwrap_or_else(|| panic!("{:?}", menu.options))
    });
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[low, high]);
}

/// Area 51 donated through Hitchhiking: the controller declines the splay,
/// takes the safeguard, picks a secret by its back and decides inside the
/// secret, which super-executes from the Safe on the executor's turn. A
/// winning secret ends the game there; the secret never leaves the Safe.
#[test]
fn area_51_runs_the_chosen_secret_from_the_safe_under_an_outer_controller() {
    for terminal in [false, true] {
        let mut f = Fixture::default();
        let hitchhiking = unseen(&mut f, "U081", Zone::Board(A, Color::Green));
        let area = unseen(&mut f, "U090", Zone::Hand(B));
        let other = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let program = if terminal {
            vec![Instruction::Win(WinRule::Executor)]
        } else {
            vec![build::score(build::HAND)]
        };
        let two = f.tech(Color::Red, 2, Zone::Safe(A), program);
        let three = f.tech(Color::Blue, 3, Zone::Safe(A), vec![build::draw(1)]);
        let low = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
        let high = f.tech(Color::Yellow, 5, Zone::Hand(A), vec![]);
        let one = f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
        let eleven = elevens(&mut f, 1)[0];
        let mut game = Game::new(f.position(), Expansions::UNSEEN)
            .unwrap()
            .with_tracking()
            .unwrap();
        effect(&mut game, hitchhiking, 0, A);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        let status = play(&mut game, &mut events, &mut deciders, |menu| {
            if let Some(index) = pick_known(menu, area).or(pick_known(menu, low)) {
                return index;
            }
            if let Some(index) = pick_back(menu, 2) {
                // Secrets are offered by back, never by front.
                assert!(pick_known(menu, two).is_none() && pick_known(menu, three).is_none());
                return index;
            }
            if menu.options.contains(&Alternative::Decline) {
                // The splay offer: decline, keeping the Safe's capacity.
                return menu
                    .options
                    .iter()
                    .position(|o| *o == Alternative::Decline)
                    .unwrap();
            }
            // The branch: safeguard rather than draw.
            assert_eq!(
                menu.options,
                [Alternative::Branch(0), Alternative::Branch(1)]
            );
            1
        });
        assert!(deciders.iter().all(|&player| player == B), "{deciders:?}");
        let position = game.position();
        assert_eq!(position.cards(Zone::Hand(B)), &[other]);
        assert_eq!(position.cards(Zone::Safe(A)), &[two, three, one]);
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, source, .. } if *card == two && *source == Zone::Safe(A))));
        if terminal {
            assert_eq!(status, Status::Winner(A));
            assert!(position.cards(Zone::Score(A)).is_empty());
        } else {
            complete(status);
            assert_eq!(deciders.len(), 5);
            assert_eq!(position.splay(A, Color::Green), Splay::None);
            assert_eq!(position.cards(Zone::Score(A)), &[low]);
            assert_eq!(position.cards(Zone::Hand(A)), &[high]);
            assert_eq!(position.cards(Zone::Achievements(A)), &[eleven]);
            trackers_hold(&game);
        }
    }
}

/// Mystery Box donated through Hitchhiking: the controller claims by back
/// and decides inside the prize, which executes where it lies; a losing
/// prize eliminates the executor, not the controller.
#[test]
fn mystery_box_executes_the_prize_in_place_under_an_outer_controller() {
    for losing in [false, true] {
        let mut f = Fixture::default();
        let hitchhiking = unseen(&mut f, "U081", Zone::Board(A, Color::Green));
        let mystery = unseen(&mut f, "U111", Zone::Hand(B));
        let other = f.tech(Color::Blue, 2, Zone::Hand(B), vec![]);
        let program = if losing {
            vec![Instruction::Win(WinRule::Loses(Who::Executor))]
        } else {
            vec![build::score(build::HAND)]
        };
        let prize = f.tech(Color::Red, 9, Zone::AvailableAchievements, program);
        let plain = f.tech(Color::Blue, 1, Zone::AvailableAchievements, vec![]);
        let low = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
        let high = f.tech(Color::Yellow, 5, Zone::Hand(A), vec![]);
        let eleven = elevens(&mut f, 1)[0];
        let mut game = Game::new(f.position(), Expansions::UNSEEN)
            .unwrap()
            .with_tracking()
            .unwrap();
        effect(&mut game, hitchhiking, 0, A);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        let status = play(&mut game, &mut events, &mut deciders, |menu| {
            pick_known(menu, mystery)
                .or(pick_known(menu, low))
                .or(pick_back(menu, 9))
                .unwrap_or_else(|| panic!("{:?}", menu.options))
        });
        assert!(deciders.iter().all(|&player| player == B), "{deciders:?}");
        let revealed: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Revealed { card, source, .. } => Some((*card, *source)),
                _ => None,
            })
            .collect();
        assert_eq!(revealed, [(prize, Zone::Achievements(A))]);
        if losing {
            assert_eq!(status, Status::Winner(B));
        } else {
            complete(status);
            assert_eq!(deciders, [B, B, B]);
            let position = game.position();
            assert_eq!(position.cards(Zone::Achievements(A)), &[prize, eleven]);
            assert_eq!(position.cards(Zone::AvailableAchievements), &[plain]);
            assert_eq!(position.cards(Zone::Score(A)), &[low]);
            assert_eq!(position.cards(Zone::Hand(A)), &[high]);
            assert_eq!(position.cards(Zone::Hand(B)), &[other]);
            trackers_hold(&game);
        }
    }
}
