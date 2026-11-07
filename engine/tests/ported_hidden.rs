use std::collections::BTreeSet;

use inno_engine::*;

mod common;
use common::*;

fn tracked(position: Position, expansions: Expansions) -> Game {
    Game::new(position, expansions)
        .unwrap()
        .with_tracking()
        .unwrap()
}

fn truth(game: &Game) {
    assert!(game.public_tracker().unwrap().check_truth(game.position()));
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
    }
}

/// Run one operation to completion with no decisions.
fn run(game: &mut Game, operation: Operation, executor: Player) {
    game.start(operation, Context::new(executor)).unwrap();
    assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
    truth(game);
}

fn beliefs(game: &Game, viewer: Player, zone: Zone) -> Vec<BeliefSlot> {
    game.tracker(viewer).unwrap().belief(zone)
}

fn revealed(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

/// The index of `card`'s first reveal and of its first movement.
fn reveal_then_move(events: &[Event], card: CardId) -> (Option<usize>, Option<usize>) {
    let reveal = events
        .iter()
        .position(|event| matches!(event, Event::Revealed { card: c, .. } if *c == card));
    let movement = events
        .iter()
        .position(|event| matches!(event, Event::Moved(m) if m.card == card));
    (reveal, movement)
}

fn known_in(game: &Game, viewer: Player, zone: Zone, card: CardId) -> bool {
    beliefs(game, viewer, zone)
        .iter()
        .any(|slot| slot.known() == Some(card))
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

/// Settle, choosing the first preferred card offered, else option 0.
fn play(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) -> Status {
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

/// A sampled world continues its own random stream, not the real game's: a shuffle of a
/// deck whose order both players know varies across samples.
#[test]
fn sampled_worlds_shuffle_with_their_own_chance() {
    let mut f = Fixture::default();
    let deck = Zone::Deck(Deck::base(1));
    let board = Zone::Board(A, Color::Blue);
    let top = f.tech(Color::Red, 1, deck, vec![]);
    let returned = f.tech(Color::Blue, 1, board, vec![]);
    let mut game = Game::seeded(f.position(), Expansions::BASE, 5)
        .unwrap()
        .with_tracking()
        .unwrap();
    // A public return beneath the only other card of that back: both
    // players know the deck's order.
    run(
        &mut game,
        Operation::Move {
            verb: Verb::Return,
            target: target(returned, board),
            destination: deck,
        },
        A,
    );
    assert_eq!(game.position().cards(deck), [returned, top]);
    for viewer in Player::ALL {
        let known: Vec<_> = beliefs(&game, viewer, deck)
            .iter()
            .map(BeliefSlot::known)
            .collect();
        assert_eq!(known, [Some(returned), Some(top)]);
    }
    let shuffle = || Operation::Shuffle {
        deck: Deck::base(1),
    };
    let mut orders = BTreeSet::new();
    for seed in 0..16 {
        let mut world = game.determinize(A, seed).unwrap();
        assert_eq!(world.position().cards(deck), [returned, top]);
        run(&mut world, shuffle(), A);
        orders.insert(world.position().cards(deck).to_vec());
    }
    assert_eq!(
        orders.len(),
        2,
        "a world replaying the real game's stream would foresee its shuffle"
    );
}

/// Two states the viewer cannot tell apart sample the same worlds from one seed, in
/// every mode: a sample depends only on what its viewer knows.
#[test]
fn indistinguishable_states_sample_identical_worlds_in_every_mode() {
    std::thread::scope(|scope| {
        for bits in 0..Expansions::COMBINATIONS as u8 {
            scope.spawn(move || paired_samples(Expansions::from_bits(bits).unwrap()));
        }
    });
}

fn paired_samples(expansions: Expansions) {
    let mut compared = 0;
    for seed in 0..3 {
        let mut game = Game::new_game(seed, expansions)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut rng = Rng::new(seed ^ 0x9A1E_D0C5);
        for step in 0..300u64 {
            let status = game.advance(1 << 16, &mut Vec::new()).unwrap();
            let (viewer, options) = match &status {
                Status::Decision(menu) => (menu.player, menu.options.len()),
                Status::Complete(_) => (game.position().turn_owner(), game.legal_actions().len()),
                _ => break,
            };
            if step % 6 == 0 {
                // Another arrangement of everything this viewer cannot see,
                // reached through the viewer's own beliefs.
                let alt = game.determinize(viewer, rng.next_u64()).unwrap();
                let context = format!("{expansions:?} seed={seed} step={step}");
                assert_eq!(
                    alt.observe(viewer).unwrap(),
                    game.observe(viewer).unwrap(),
                    "{context}"
                );
                for sample in 0..2 {
                    let sample_seed = seed * 10_000 + step * 2 + sample;
                    let real = game.determinize(viewer, sample_seed).unwrap();
                    let other = alt.determinize(viewer, sample_seed).unwrap();
                    assert_eq!(real.position(), other.position(), "{context}");
                    assert_eq!(
                        real.observe(viewer).unwrap(),
                        other.observe(viewer).unwrap(),
                        "{context}"
                    );
                }
                compared += 1;
            }
            let index = (rng.next_u64() % options as u64) as usize;
            match status {
                Status::Decision(_) => game.choose(index).unwrap(),
                _ => {
                    let action = game.legal_actions()[index];
                    game.take_action(action, ActionKind::Regular).unwrap();
                }
            }
        }
    }
    assert!(compared > 20, "{expansions:?}: {compared} compared");
}

/// Foreseen Bell returns every hand card to its deck's bottom unrevealed: each hand's
/// owner knows the returned identity, the other player only its back unless it already
/// knew the card.
#[test]
fn foreseen_bell_returns_stay_private_to_each_hands_owner() {
    for shown in [false, true] {
        let mut f = Fixture::default();
        let bell = f.add(echoes_card("E013").unwrap(), Zone::Forecast(A));
        let melded = f.tech(Color::Red, 2, Zone::Hand(A), vec![]);
        let mine = f.tech(Color::Green, 3, Zone::Hand(A), vec![]);
        let theirs = f.tech(Color::Yellow, 4, Zone::Hand(B), vec![]);
        // Another card of each returned back that neither player sees.
        f.tech(Color::Blue, 3, Zone::AvailableAchievements, vec![]);
        f.tech(Color::Blue, 4, Zone::AvailableAchievements, vec![]);
        // Bell's own draws, which it foreshadows.
        f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
        f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
        let mut game = tracked(f.position(), Expansions::ECHOES);
        if shown {
            for (card, owner) in [(mine, A), (theirs, B)] {
                run(
                    &mut game,
                    Operation::Reveal {
                        target: target(card, Zone::Hand(owner)),
                    },
                    owner,
                );
            }
        }
        game.take_action(Action::Meld { card: melded }, ActionKind::Regular)
            .unwrap();
        let mut events = Vec::new();
        // Bell, the only eligible Forecast card, is promoted. Decline its
        // echo ("You may score a card from your hand").
        let echo = decision(&mut game, &mut events);
        assert_eq!(game.position().top(A, Color::Purple), Some(bell));
        let decline = echo
            .options
            .iter()
            .position(|option| *option == Alternative::Decline)
            .unwrap();
        game.choose(decline).unwrap();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        truth(&game);
        let mine_deck = Zone::Deck(Deck::base(3));
        let theirs_deck = Zone::Deck(Deck::base(4));
        assert!(game.position().cards(Zone::Hand(A)).is_empty());
        assert!(game.position().cards(Zone::Hand(B)).is_empty());
        assert_eq!(game.position().cards(mine_deck), [mine]);
        assert_eq!(game.position().cards(theirs_deck), [theirs]);
        assert!(
            revealed(&events).is_empty(),
            "Bell's returns reveal nothing"
        );
        for (card, deck, owner) in [(mine, mine_deck, A), (theirs, theirs_deck, B)] {
            assert_eq!(beliefs(&game, owner, deck)[0].known(), Some(card));
            let other = &beliefs(&game, owner.opponent(), deck)[0];
            if shown {
                assert_eq!(other.known(), Some(card), "a shown card stays known");
            } else {
                assert_eq!(other.known(), None, "shown={shown} owner={owner:?}");
                assert!(other.candidates.contains(&card));
            }
        }
    }
}

/// Secretum Secretorum's draw value counts the returned colors, so every returned card
/// is revealed and stays known at its deck's bottom.
#[test]
fn secretum_secretorum_reveals_every_returned_card() {
    let mut f = Fixture::default();
    let card = f.add(unseen_card("U033").unwrap(), Zone::Board(A, Color::Blue));
    let mut returned: Vec<_> = [Color::Red, Color::Red, Color::Green]
        .into_iter()
        .map(|color| f.tech(color, 1, Zone::Hand(A), vec![]))
        .collect();
    returned.extend(
        [Color::Blue, Color::Blue]
            .into_iter()
            .map(|color| f.tech(color, 2, Zone::Score(A), vec![])),
    );
    for color in [Color::Yellow, Color::Purple] {
        f.tech(color, 3, Zone::Deck(Deck::base(3)), vec![]);
    }
    let mut game = tracked(f.position(), Expansions::UNSEEN);
    effect(&mut game, card, 0, A, A);
    let mut events = Vec::new();
    assert!(matches!(
        play(&mut game, &mut events, &[]),
        Status::Complete(_)
    ));
    truth(&game);
    for &card in &returned {
        let (reveal, movement) = reveal_then_move(&events, card);
        assert!(reveal.unwrap() < movement.unwrap(), "{card:?} shown first");
        let deck = Zone::Deck(game.position().card(card).origin_deck());
        assert!(game.position().cards(deck).contains(&card));
        assert!(known_in(&game, B, deck, card), "{card:?} known at its deck");
    }
}

/// Quackery's draw value sums the two returned cards' Health and Concept icons, so both
/// are revealed; the card left in hand stays private.
#[test]
fn quackery_reveals_both_returned_cards_and_not_the_kept_one() {
    let mut f = Fixture::default();
    let card = f.add(unseen_card("U042").unwrap(), Zone::Board(A, Color::Blue));
    let first = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let second = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    let kept = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    for (card, icons) in [
        (first, [Icon::Health, Icon::Health]),
        (second, [Icon::Concept, Icon::Authority]),
    ] {
        for (slot, icon) in icons.into_iter().enumerate() {
            f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
        }
    }
    // Another card of the kept card's back that B cannot see.
    f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
    let three = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let mut game = tracked(f.position(), Expansions::UNSEEN);
    effect(&mut game, card, 1, A, A);
    let mut events = Vec::new();
    assert!(matches!(
        play(&mut game, &mut events, &[first, second]),
        Status::Complete(_)
    ));
    truth(&game);
    // Two Health and one Concept: a [3].
    let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
    hand.sort();
    assert_eq!(hand, [kept, three]);
    let deck = Zone::Deck(Deck::base(1));
    for card in [first, second] {
        let (reveal, movement) = reveal_then_move(&events, card);
        assert!(reveal.unwrap() < movement.unwrap(), "{card:?} shown first");
        assert!(known_in(&game, B, deck, card));
    }
    assert!(!revealed(&events).contains(&kept));
    assert!(!known_in(&game, B, Zone::Hand(A), kept));
}

/// Achieving a card from a private pile: its owner keeps the identity, and the other
/// player, even as the achiever, learns only its back.
#[test]
fn achieving_from_a_private_pile_keeps_only_the_owners_knowledge() {
    for owner in Player::ALL {
        for achiever in Player::ALL {
            for source in [Zone::Hand(owner), Zone::Score(owner), Zone::Forecast(owner)] {
                let mut f = Fixture::default();
                let achieved = f.tech(Color::Red, 2, source, vec![]);
                let remaining = f.tech(Color::Blue, 2, source, vec![]);
                // A third card of that back that neither player sees.
                f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
                let mut game = tracked(f.position(), Expansions::ECHOES);
                let case = format!("owner={owner:?} achiever={achiever:?} {source:?}");
                let mut events = Vec::new();
                game.start(
                    Operation::Claim {
                        target: target(achieved, source),
                        eligibility: ClaimEligibility::Ignore,
                    },
                    Context::new(achiever),
                )
                .unwrap();
                assert!(
                    matches!(settle(&mut game, &mut events), Status::Complete(_)),
                    "{case}"
                );
                truth(&game);
                let achievements = Zone::Achievements(achiever);
                assert_eq!(game.position().cards(achievements), [achieved], "{case}");
                assert!(revealed(&events).is_empty(), "{case}");
                assert!(known_in(&game, owner, achievements, achieved), "{case}");
                assert!(known_in(&game, owner, source, remaining), "{case}");
                for zone in [source, achievements] {
                    for slot in beliefs(&game, owner.opponent(), zone) {
                        assert_eq!(slot.known(), None, "{case} {zone:?}");
                        assert!(
                            slot.candidates.contains(&achieved)
                                && slot.candidates.contains(&remaining),
                            "{case} {zone:?}"
                        );
                    }
                }
            }
        }
    }
}

/// Each self-execution keyword earns a Chain draw of an [11].
fn elevens(f: &mut Fixture, count: usize) {
    for i in 0..count {
        f.tech(Color::ALL[i % 5], 11, Zone::Deck(Deck::base(11)), vec![]);
    }
}

/// Nested inspection grants (Hitchhiking) hold together: an inner execution controlled
/// the other way grants its own inspection without ending the outer one, and each ends
/// with its own execution.
#[test]
fn nested_inspection_grants_each_last_for_their_own_execution() {
    let mut f = Fixture::default();
    elevens(&mut f, 3);
    // A's blue top: its executor melds a card from hand.
    f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![build::meld(build::HAND)],
    );
    // A's green top: B self-executes the blue top with A controlling and
    // inspecting, then A (still controlled by B) melds from hand.
    f.tech(
        Color::Green,
        1,
        Zone::Board(A, Color::Green),
        vec![
            build::bind(Selector::TopOf(Who::Executor, Color::Blue)),
            build::execute(ExecutionMode::SelfExecute, Who::Opponent)
                .controlled_by(Who::Executor)
                .inspecting(),
            build::meld(build::HAND),
        ],
    );
    // A's purple top: A self-executes the green top, controlled and
    // inspected by B.
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::bind(Selector::TopOf(Who::Executor, Color::Green)),
            build::execute(ExecutionMode::SelfExecute, Who::Executor)
                .controlled_by(Who::Opponent)
                .inspecting(),
        ],
    );
    let ours = [
        f.tech(Color::Red, 2, Zone::Hand(A), vec![]),
        f.tech(Color::Yellow, 3, Zone::Hand(A), vec![]),
    ];
    let theirs = [
        f.tech(Color::Red, 2, Zone::Hand(B), vec![]),
        f.tech(Color::Yellow, 3, Zone::Hand(B), vec![]),
    ];
    let mut game = tracked(f.position(), Expansions::BASE);
    game.self_execute(caller, Context::new(A)).unwrap();
    let sees = |game: &Game, card, owner: Player, viewer| {
        matches!(
            game.position()
                .descriptor(target(card, Zone::Hand(owner)), viewer),
            TargetDescriptor::Known { .. }
        )
    };
    let mut events = Vec::new();
    // Inside both executions: A decides B's meld and sees B's hand; B still
    // sees A's hand through the outer grant.
    let inner = decision(&mut game, &mut events);
    assert_eq!(inner.player, A);
    assert!(
        theirs
            .iter()
            .all(|&card| pick_known(&inner, card).is_some())
    );
    assert!(theirs.iter().all(|&card| sees(&game, card, B, A)));
    assert!(ours.iter().all(|&card| sees(&game, card, A, B)));
    game.choose(pick_known(&inner, theirs[0]).unwrap()).unwrap();
    // Back in the outer execution only: B decides A's meld and still sees
    // A's hand; A's inspection of B's hand has ended.
    let outer = decision(&mut game, &mut events);
    assert_eq!(outer.player, B);
    assert!(ours.iter().all(|&card| pick_known(&outer, card).is_some()));
    assert!(ours.iter().all(|&card| sees(&game, card, A, B)));
    assert!(!sees(&game, theirs[1], B, A));
    game.choose(pick_known(&outer, ours[0]).unwrap()).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert!(!sees(&game, ours[1], A, B));
    // Permission ends with each execution; what was seen stays known.
    assert!(known_in(&game, A, Zone::Hand(B), theirs[1]));
    assert!(known_in(&game, B, Zone::Hand(A), ours[1]));
    truth(&game);
}

/// Cabal's sweep matches values, which public backs prove: it reveals no card, and the
/// cards left in the victim's hand stay private.
#[test]
fn cabal_sweeps_by_public_values_without_a_proof_reveal() {
    let mut f = Fixture::default();
    let card = f.add(unseen_card("U046").unwrap(), Zone::Board(A, Color::Red));
    f.tech(Color::Blue, 3, Zone::Safe(A), vec![]);
    let three = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
    let four = f.tech(Color::Yellow, 4, Zone::Hand(B), vec![]);
    // Another [4] that A cannot see.
    f.tech(Color::Red, 4, Zone::AvailableAchievements, vec![]);
    let five = f.tech(Color::Purple, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = tracked(f.position(), Expansions::UNSEEN);
    effect(&mut game, card, 0, B, A);
    let mut events = Vec::new();
    assert!(matches!(
        play(&mut game, &mut events, &[]),
        Status::Complete(_)
    ));
    truth(&game);
    assert_eq!(game.position().cards(Zone::Score(A)), [three]);
    let mut hand = game.position().cards(Zone::Hand(B)).to_vec();
    hand.sort();
    assert_eq!(hand, [four, five]);
    assert!(revealed(&events).is_empty(), "no proof reveal");
    assert!(!known_in(&game, A, Zone::Hand(B), four));
}
