//! Unseen action histories. Polytheism's icon memory and Legend's
//! scored points are per executor and survive sharing, repeats and the
//! endorsed repeat; Brethren of Purity's last meld is the action's, across
//! executors; Padlock's demand memory is fresh per execution mode.

use inno_engine::*;

mod common;
use common::*;

fn unseen_on_board(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = unseen_card(code).unwrap();
    let zone = Zone::Board(player, card.color);
    f.add(card, zone)
}

fn icons(f: &mut Fixture, card: CardId, icons: &[Icon]) {
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
    }
}

/// A blank card of value 1 on `player`'s board carrying three of one icon,
/// enough to share any Unseen card featuring it.
fn sharer(f: &mut Fixture, player: Player, color: Color, icon: Icon) -> CardId {
    let card = f.tech(color, 1, Zone::Board(player, color), vec![]);
    icons(f, card, &[icon, icon, icon]);
    card
}

fn dogma(game: &mut Game, card: CardId) {
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
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
        } => Some(*card),
        _ => None,
    }
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| known(option) == Some(card))
}

fn pick_color(menu: &Decision, color: Color) -> Option<usize> {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Colors(1 << color as u8))
}

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

/// Polytheism (Concept): a sharer's melds do not restrict the owner's, and
/// the owner's own melds accumulate across the repeat.
#[test]
fn polytheism_memory_is_per_executor_under_sharing() {
    let mut f = Fixture::default();
    let polytheism = unseen_on_board(&mut f, "U014", A);
    sharer(&mut f, B, Color::Blue, Icon::Concept);
    let b_authority = f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
    icons(&mut f, b_authority, &[Icon::Authority]);
    let a_authority = f.tech(Color::Green, 1, Zone::Hand(A), vec![]);
    icons(&mut f, a_authority, &[Icon::Authority]);
    let a_health = f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    icons(&mut f, a_health, &[Icon::Health]);
    let a_both = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    icons(&mut f, a_both, &[Icon::Authority, Icon::Health]);
    // Two "draw and tuck a [1]" plus the share bonus draw.
    let ones: Vec<_> = (0..3)
        .map(|_| f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    dogma(&mut game, polytheism);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        if let Some(index) = pick_known(menu, a_authority) {
            // B's Authority meld did not remove A's Authority card.
            assert_eq!(menu.options.len(), 3, "{:?}", menu.options);
            return index;
        }
        panic!("{:?}", menu.options)
    }));
    // The repeat has one candidate (the card sharing no icon with A's
    // meld), which melds without a question.
    assert_eq!(deciders, [A]);
    let position = game.position();
    assert_eq!(position.top(B, Color::Red), Some(b_authority));
    assert_eq!(position.top(A, Color::Green), Some(a_authority));
    assert_eq!(position.top(A, Color::Yellow), Some(a_health));
    assert_eq!(position.cards(Zone::Hand(A)), &[a_both, ones[0]]);
    assert!(position.cards(Zone::Deck(Deck::base(1))).is_empty());
}

/// An endorsed Polytheism keeps its icon memory into the repeated
/// execution: the second run cannot meld a card sharing an icon with the
/// first run's meld.
#[test]
fn polytheism_memory_survives_the_endorsed_repeat() {
    let mut f = Fixture::default();
    let polytheism = unseen_on_board(&mut f, "U014", A);
    let babylon = city_card("C007").unwrap();
    let babylon_color = babylon.color;
    f.add(babylon, Zone::Board(A, babylon_color));
    let payment = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let x = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    icons(&mut f, x, &[Icon::Authority]);
    let y = f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
    icons(&mut f, y, &[Icon::Authority]);
    let ones: Vec<_> = (0..2)
        .map(|_| f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN.union(Expansions::CITIES)).unwrap();
    dogma(&mut game, polytheism);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        if menu.options == [Alternative::Branch(0), Alternative::Branch(1)] {
            return 0; // endorse
        }
        pick_known(menu, x)
            .or(pick_known(menu, payment))
            .unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Endorsed { .. }))
    );
    let position = game.position();
    assert_eq!(position.cards(Zone::Junk), &[payment]);
    assert_eq!(position.top(A, Color::Green), Some(x));
    assert_eq!(position.cards(Zone::Hand(A)), &[y]);
    // Both executions' "draw and tuck a [1]" happened, under Polytheism.
    assert!(position.cards(Zone::Deck(Deck::base(1))).is_empty());
    assert_eq!(
        position.cards(Zone::Board(A, Color::Purple)),
        &[ones[0], ones[1], polytheism]
    );
}

/// Brethren of Purity: the sharer's meld is the action's last meld, so the
/// owner may draw one higher than it rather than a [3].
#[test]
fn brethren_reads_the_actions_last_meld_across_executors() {
    for higher in [true, false] {
        brethren_after_a_sharer(higher);
    }
}

fn brethren_after_a_sharer(higher: bool) {
    let mut f = Fixture::default();
    let brethren = unseen_on_board(&mut f, "U032", A);
    sharer(&mut f, B, Color::Purple, Icon::Concept);
    let threes: Vec<_> = (0..2)
        .map(|_| f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]))
        .collect();
    let four = f.tech(Color::Yellow, 4, Zone::Deck(Deck::base(4)), vec![]);
    let fives: Vec<_> = (0..2)
        .map(|_| f.tech(Color::Red, 5, Zone::Deck(Deck::base(5)), vec![]))
        .collect();
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    dogma(&mut game, brethren);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        assert_eq!(
            (menu.option_label(0, A), menu.option_label(1, A)),
            (
                Some("Draw and meld a [3]"),
                Some("Draw and meld one higher than the last Brethren meld (a [1] if none)")
            )
        );
        usize::from(higher)
    }));
    // Only the owner chooses: with no [1] or [2] deck, the sharer's [3] and
    // [1] requests both draw a [3].
    assert_eq!(deciders, [A]);
    let position = game.position();
    assert_eq!(position.top(B, Color::Green), Some(threes[1]));
    if higher {
        assert_eq!(position.cards(Zone::Deck(Deck::base(3))), &threes[..1]);
        assert_eq!(position.top(A, Color::Yellow), Some(four));
        assert!(position.cards(Zone::Deck(Deck::base(4))).is_empty());
        // The share bonus draw of A's highest top value.
        assert_eq!(position.cards(Zone::Hand(A)), &fives[1..]);
        assert_eq!(position.cards(Zone::Deck(Deck::base(5))), &fives[..1]);
    } else {
        assert_eq!(position.top(A, Color::Green), Some(threes[0]));
        assert!(position.cards(Zone::Deck(Deck::base(3))).is_empty());
        // The share bonus asks for a [3] and skips to the [4].
        assert_eq!(position.cards(Zone::Hand(A)), &[four]);
    }
}

/// Legend counts only its own scores: a card its self-execution scores does
/// not bring the total to nine, and a sharer's Legend points are theirs.
#[test]
fn legend_counts_only_its_own_scores_per_executor() {
    // The executed red top scores a [10] from hand; Legend still repeats.
    let mut f = Fixture::default();
    let legend = unseen_on_board(&mut f, "U045", A);
    let six = f.tech(Color::Red, 6, Zone::Board(A, Color::Red), vec![]);
    let five = f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
    let four = f.tech(
        Color::Red,
        4,
        Zone::Board(A, Color::Red),
        vec![build::score(build::HAND)],
    );
    let ten = f.tech(Color::Blue, 10, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, legend, 0, A);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_color(menu, Color::Red).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    assert_eq!(deciders, [A]);
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(A)), &[ten, four, five]);
    assert_eq!(position.top(A, Color::Red), Some(six));

    // Shared: B scores nine of their own first; A's count starts at zero.
    let mut f = Fixture::default();
    let legend = unseen_on_board(&mut f, "U045", A);
    let six = f.tech(Color::Red, 6, Zone::Board(A, Color::Red), vec![]);
    let five = f.tech(Color::Red, 5, Zone::Board(A, Color::Red), vec![]);
    let four = f.tech(Color::Red, 4, Zone::Board(A, Color::Red), vec![]);
    sharer(&mut f, B, Color::Purple, Icon::Health);
    let five_b = f.tech(Color::Red, 5, Zone::Board(B, Color::Red), vec![]);
    let four_b = f.tech(Color::Red, 4, Zone::Board(B, Color::Red), vec![]);
    let bonus = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    dogma(&mut game, legend);
    let mut events = Vec::new();
    let mut deciders = Vec::new();
    complete(play(&mut game, &mut events, &mut deciders, |menu| {
        pick_color(menu, Color::Red).unwrap_or_else(|| panic!("{:?}", menu.options))
    }));
    assert_eq!(deciders, [B, A]);
    let position = game.position();
    assert_eq!(position.cards(Zone::Score(B)), &[four_b, five_b]);
    assert!(position.top(B, Color::Red).is_none());
    assert_eq!(position.cards(Zone::Score(A)), &[four, five]);
    assert_eq!(position.top(A, Color::Red), Some(six));
    assert_eq!(position.cards(Zone::Hand(A)), &[bonus]);
}

/// Padlock's demand memory belongs to each execution: a super-execution
/// whose demand transferred skips the scoring; a self-execution, which runs
/// no demand, offers it.
#[test]
fn padlock_demand_memory_is_fresh_per_execution() {
    for mode in [ExecutionMode::SuperExecute, ExecutionMode::SelfExecute] {
        let mut f = Fixture::default();
        let padlock = unseen_on_board(&mut f, "U016", A);
        let padlock_color = f.cards[usize::from(padlock.0)].color;
        let caller = f.tech(
            Color::Purple,
            1,
            Zone::Board(A, Color::Purple),
            vec![
                build::bind(Selector::TopOf(Who::Executor, padlock_color)),
                build::execute(mode, Who::Executor),
            ],
        );
        let card = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
        let secret = f.tech(Color::Blue, 4, Zone::Safe(B), vec![]);
        let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
        effect(&mut game, caller, 0, A);
        let mut events = Vec::new();
        let mut deciders = Vec::new();
        complete(play(&mut game, &mut events, &mut deciders, |menu| {
            assert!(menu.options.contains(&Alternative::Decline));
            pick_known(menu, card).unwrap_or_else(|| panic!("{:?}", menu.options))
        }));
        let position = game.position();
        let label = format!("{mode:?}");
        if mode == ExecutionMode::SuperExecute {
            assert!(deciders.is_empty(), "{label}");
            assert_eq!(
                position.cards(Zone::AvailableAchievements),
                &[secret],
                "{label}"
            );
            assert_eq!(position.cards(Zone::Hand(A)), &[card], "{label}");
            assert!(position.cards(Zone::Score(A)).is_empty(), "{label}");
        } else {
            assert_eq!(deciders, [A], "{label}");
            assert_eq!(position.cards(Zone::Safe(B)), &[secret], "{label}");
            assert_eq!(position.cards(Zone::Score(A)), &[card], "{label}");
        }
    }
}
