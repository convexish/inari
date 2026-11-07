//! Martian Internet's printed subgame (`Game::with_subgames`); the draw
//! variant's test is in `age11`.

use super::*;
use crate::program::build::{HAND, score as score_from};

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

fn board(player: Player, color: Color) -> Zone {
    Zone::Board(player, color)
}

fn base_deck(age: u8) -> Zone {
    Zone::Deck(Deck::base(age))
}

fn sharing_icons(code: &str) -> [Icon; 3] {
    [artifacts_card(code).unwrap().featured.unwrap(); 3]
}

/// A Base card with this code (its alphabetical rank) and dogma.
fn programmed(
    l: &mut Layout,
    code: &'static str,
    color: Color,
    value: u8,
    zone: Zone,
    instructions: Vec<Instruction>,
) -> CardId {
    // Three featured icons on a card with a dogma: its owner alone executes.
    let mut symbols = [Symbol::Blank; SLOTS];
    if !instructions.is_empty() {
        symbols[..3].fill(Symbol::Icon(Icon::Authority));
    }
    l.add(
        Card {
            code,
            set: CardSet::Base,
            color,
            value,
            figure: None,
            echo: None,
            symbols,
            featured: Some(Icon::Authority),
            effects: vec![Effect {
                kind: EffectKind::from_demand(false),
                instructions,
            }],
        },
        zone,
    )
}

/// Martian Internet on A's board, with A's and B's other cards to set aside.
fn parents(l: &mut Layout) -> (CardId, [CardId; 3]) {
    let martian = l.artifact("A113", board(A, Color::Blue));
    let held = [
        l.tech(Color::Red, 5, HAND_A, &[]),
        l.tech(Color::Yellow, 3, SCORE_A, &[]),
        l.tech(Color::Green, 2, board(B, Color::Green), &[]),
    ];
    (martian, held)
}

fn printed(l: Layout) -> Game {
    l.game().with_tracking().unwrap().with_subgames(true)
}

fn act(game: &mut Game, action: Action) -> (Status, Vec<Event>) {
    game.take_action(action, ActionKind::Regular).unwrap();
    no_decisions(game)
}

fn started(events: &[Event]) -> Vec<(Player, usize)> {
    events
        .iter()
        .filter_map(|event| match *event {
            Event::SubgameStarted { executor, depth } => Some((executor, depth)),
            _ => None,
        })
        .collect()
}

fn ended(events: &[Event]) -> Vec<(Option<Player>, usize)> {
    events
        .iter()
        .filter_map(|event| match *event {
            Event::SubgameEnded { winner, depth } => Some((winner, depth)),
            _ => None,
        })
        .collect()
}

fn transferred(events: &[Event]) -> Vec<(CardId, Zone, Zone)> {
    events
        .iter()
        .filter_map(|event| match *event {
            Event::Transferred {
                card,
                source,
                destination,
                ..
            } => Some((card, source, destination)),
            _ => None,
        })
        .collect()
}

fn sorted(cards: &[CardId]) -> Vec<CardId> {
    let mut cards = cards.to_vec();
    cards.sort();
    cards
}

#[test]
fn without_two_high_cards_the_effect_does_nothing() {
    let mut l = Layout::default();
    let (martian, _) = parents(&mut l);
    l.tech(Color::Red, 8, base_deck(8), &[]);
    let mut game = printed(l);
    dogma(&mut game, martian);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.subgame_depth(), 0);
    assert!(started(&events).is_empty());
    assert!(game.position().cards(Zone::Junk).is_empty());
    assert_eq!(game.position().cards(base_deck(8)).len(), 1);
}

#[test]
fn with_one_high_card_it_is_junked_instead() {
    let mut l = Layout::default();
    let (martian, _) = parents(&mut l);
    let ten = l.tech(Color::Red, 10, base_deck(10), &[]);
    let mut game = printed(l);
    dogma(&mut game, martian);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.subgame_depth(), 0);
    assert!(started(&events).is_empty());
    assert_eq!(game.position().cards(Zone::Junk), &[ten]);
    assert!(moves(&events).contains(&(Some(Verb::Junk), ten, base_deck(10), Zone::Junk)));
}

#[test]
fn two_high_cards_start_a_subgame_on_the_shared_supply() {
    let mut l = Layout::default();
    let (martian, [hand, score, green]) = parents(&mut l);
    // Player One draws the top (last) card; the lower code moves first.
    let x = programmed(&mut l, "AAA", Color::Red, 9, base_deck(9), Vec::new());
    let y = programmed(&mut l, "ZZZ", Color::Purple, 9, base_deck(9), Vec::new());
    let eleven = l.tech(Color::Blue, 11, base_deck(11), &[]);
    let mut game = printed(l);
    dogma(&mut game, martian);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(started(&events), [(A, 1)]);
    assert_eq!(game.subgame_depth(), 1);
    let child = game.position();
    assert_eq!(child.cards(board(A, Color::Purple)), &[y]);
    assert_eq!(child.cards(board(B, Color::Red)), &[x]);
    assert!(child.cards(HAND_A).is_empty() && child.cards(SCORE_A).is_empty());
    assert!(child.cards(board(A, Color::Blue)).is_empty());
    assert!(child.cards(board(B, Color::Green)).is_empty());
    assert!(child.cards(base_deck(9)).is_empty());
    assert_eq!(child.cards(base_deck(11)), &[eleven]);
    assert_eq!(child.turn_owner(), B);
    assert_eq!(child.regular_action_limit, 1);
    assert!(child.check_conservation());
    assert!(game.legal_actions().contains(&Action::Draw));
    let parent = game.parent_position(1).unwrap();
    assert_eq!(parent.cards(board(A, Color::Blue)), &[martian]);
    assert_eq!(parent.cards(HAND_A), &[hand]);
    assert_eq!(parent.cards(SCORE_A), &[score]);
    assert_eq!(parent.cards(board(B, Color::Green)), &[green]);
    assert!(game.parent_position(2).is_none());
}

#[test]
fn a_drawn_subgame_junks_everything_and_resumes_the_parent() {
    let mut l = Layout::default();
    let (martian, [hand, score, green]) = parents(&mut l);
    let x = programmed(&mut l, "AAA", Color::Red, 9, base_deck(9), Vec::new());
    let y = programmed(&mut l, "ZZZ", Color::Purple, 9, base_deck(9), Vec::new());
    let mut game = printed(l);
    dogma(&mut game, martian);
    no_decisions(&mut game);
    // B's one action: a draw past the empty 9s, 10s and 11s ends the
    // subgame by exhaustion with no score on either side.
    let (status, events) = act(&mut game, Action::Draw);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(ended(&events), [(None, 1)]);
    assert_eq!(game.subgame_depth(), 0);
    assert_eq!(
        sorted(&transferred(&events).iter().map(|t| t.0).collect::<Vec<_>>()),
        sorted(&[x, y])
    );
    assert!(
        transferred(&events)
            .iter()
            .all(|&(_, _, destination)| destination == Zone::Junk)
    );
    let parent = game.position();
    assert_eq!(sorted(parent.cards(Zone::Junk)), sorted(&[x, y]));
    assert_eq!(parent.cards(board(A, Color::Blue)), &[martian]);
    assert_eq!(parent.cards(HAND_A), &[hand]);
    assert_eq!(parent.cards(SCORE_A), &[score]);
    assert_eq!(parent.cards(board(B, Color::Green)), &[green]);
    assert_eq!(parent.turn_owner(), A);
    assert!(parent.check_conservation());
    assert!(game.tracker(A).unwrap().check_truth(parent));
    assert!(game.tracker(B).unwrap().check_truth(parent));
}

#[test]
fn the_winner_transfers_hand_score_and_board_and_the_loser_is_junked() {
    let mut l = Layout::default();
    let (martian, [hand, score, green]) = parents(&mut l);
    let c = programmed(&mut l, "CCC", Color::Yellow, 9, base_deck(9), Vec::new());
    let d = programmed(&mut l, "DDD", Color::Blue, 9, base_deck(9), Vec::new());
    // B draws and melds X: scoring a card from hand and winning the subgame.
    let x = programmed(
        &mut l,
        "XXX",
        Color::Green,
        9,
        base_deck(9),
        vec![score_from(HAND), Instruction::Win(WinRule::Executor)],
    );
    let y = programmed(&mut l, "YYY", Color::Purple, 9, base_deck(9), Vec::new());
    let mut game = printed(l);
    dogma(&mut game, martian);
    no_decisions(&mut game);
    assert_eq!(game.position().turn_owner(), B);
    assert_eq!(
        act(&mut game, Action::Draw).0,
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(HAND_B), &[d]);
    assert_eq!(game.position().turn_owner(), A);
    act(&mut game, Action::Draw);
    act(&mut game, Action::Meld { card: c });
    assert_eq!(game.position().turn_owner(), B);
    let (status, events) = act(
        &mut game,
        Action::Dogma {
            card: x,
            auspice: false,
        },
    );
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(ended(&events), [(Some(B), 1)]);
    assert_eq!(game.subgame_depth(), 0);
    let parent = game.position();
    // B's green pile: the subgame's card on top of the parent's.
    assert_eq!(parent.cards(board(B, Color::Green)), &[green, x]);
    assert_eq!(parent.splay(B, Color::Green), Splay::None);
    assert_eq!(parent.cards(SCORE_B), &[d]);
    assert!(parent.cards(HAND_B).is_empty());
    assert_eq!(sorted(parent.cards(Zone::Junk)), sorted(&[c, y]));
    assert_eq!(parent.cards(board(A, Color::Blue)), &[martian]);
    assert_eq!(parent.cards(HAND_A), &[hand]);
    assert_eq!(parent.cards(SCORE_A), &[score]);
    assert!(parent.check_conservation());
    assert!(game.tracker(A).unwrap().check_truth(parent));
    assert!(game.tracker(B).unwrap().check_truth(parent));
    let transfers = transferred(&events);
    assert!(transfers.contains(&(x, board(B, Color::Green), board(B, Color::Green))));
    assert!(transfers.contains(&(d, SCORE_B, SCORE_B)));
    assert!(transfers.contains(&(c, board(A, Color::Yellow), Zone::Junk)));
    assert!(transfers.contains(&(y, board(A, Color::Purple), Zone::Junk)));
}

#[test]
fn a_shared_execution_runs_one_subgame_per_executor() {
    let mut l = Layout::default();
    let (martian, _) = parents(&mut l);
    l.tech(Color::Red, 4, board(B, Color::Red), &sharing_icons("A113"));
    let c = programmed(&mut l, "CCC", Color::Yellow, 9, base_deck(9), Vec::new());
    let d = programmed(&mut l, "DDD", Color::Blue, 9, base_deck(9), Vec::new());
    let y = programmed(&mut l, "YYY", Color::Purple, 9, base_deck(9), Vec::new());
    let x = programmed(
        &mut l,
        "XXX",
        Color::Red,
        9,
        base_deck(9),
        vec![Instruction::Win(WinRule::Executor)],
    );
    let mut game = printed(l);
    dogma(&mut game, martian);
    // B shares and executes first: the first subgame deals X to A and Y to
    // B; A moves first and wins it with X.
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(started(&events), [(B, 1)]);
    assert_eq!(game.position().turn_owner(), A);
    let (status, events) = act(
        &mut game,
        Action::Dogma {
            card: x,
            auspice: false,
        },
    );
    // A's own execution starts the second subgame on the two cards left:
    // D to A, C to B, B first.
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(ended(&events), [(Some(A), 1)]);
    assert_eq!(started(&events), [(A, 1)]);
    assert_eq!(game.subgame_depth(), 1);
    assert_eq!(game.position().cards(board(A, Color::Blue)), &[d]);
    assert_eq!(game.position().cards(board(B, Color::Yellow)), &[c]);
    assert_eq!(game.position().turn_owner(), B);
    // B's draw exhausts the decks: a drawn subgame, everything junked; then
    // A's sharing bonus draw exhausts the parent, which A wins on score.
    let (status, events) = act(&mut game, Action::Draw);
    assert_eq!(ended(&events), [(None, 1)]);
    assert_eq!(game.subgame_depth(), 0);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(
        game.victory().map(|v| v.reason),
        Some(WinReason::Exhaustion)
    );
    let parent = game.position();
    assert_eq!(parent.cards(board(A, Color::Red)), &[x]);
    assert_eq!(sorted(parent.cards(Zone::Junk)), sorted(&[y, c, d]));
    assert!(parent.check_conservation());
}

#[test]
fn search_inside_a_subgame_plays_to_win_it_and_never_rejoins() {
    let mut l = Layout::default();
    let (martian, _) = parents(&mut l);
    programmed(&mut l, "AAA", Color::Red, 9, base_deck(9), Vec::new());
    programmed(&mut l, "ZZZ", Color::Purple, 9, base_deck(9), Vec::new());
    for age in [10, 11] {
        l.tech(Color::Blue, age, base_deck(age), &[]);
        l.tech(Color::Green, age, base_deck(age), &[]);
    }
    let mut game = printed(l);
    dogma(&mut game, martian);
    no_decisions(&mut game);
    assert_eq!(game.subgame_depth(), 1);
    let before = game.clone();
    let result = game
        .search(SearchConfig {
            iterations: 24,
            rollout_actions: 2,
            ..SearchConfig::default()
        })
        .unwrap();
    assert_eq!(game, before);
    assert!(result.iterations > 0);
    let Step::Action(action) = result.step else {
        panic!("an action at the subgame's first turn");
    };
    assert!(game.legal_actions().contains(&action));
}

#[test]
fn the_draw_variant_is_unchanged_without_the_flag() {
    let mut l = Layout::default();
    let (martian, _) = parents(&mut l);
    programmed(&mut l, "AAA", Color::Red, 9, base_deck(9), Vec::new());
    programmed(&mut l, "ZZZ", Color::Purple, 9, base_deck(9), Vec::new());
    let mut game = l.game();
    assert!(!game.subgames());
    dogma(&mut game, martian);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Draw(DrawReason::Effect));
    assert!(started(&events).is_empty());
}

#[test]
fn a_subgame_hitting_the_work_cap_rejoins_its_parent_as_a_draw() {
    let mut l = Layout::default();
    let (martian, held) = parents(&mut l);
    let nines = [
        programmed(&mut l, "AAA", Color::Red, 9, base_deck(9), Vec::new()),
        programmed(&mut l, "ZZZ", Color::Purple, 9, base_deck(9), Vec::new()),
    ];
    l.tech(Color::Blue, 10, base_deck(10), &[]);
    let mut game = printed(l);
    dogma(&mut game, martian);
    no_decisions(&mut game);
    assert_eq!(game.subgame_depth(), 1);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    game.exhaust_automatic_work();
    let mut events = Vec::new();
    let status = game.advance(1000, &mut events).unwrap();
    // The cap drew the child, not the match: the parent plays on.
    assert!(!matches!(status, Status::Draw(_)), "{status:?}");
    assert_eq!(game.subgame_depth(), 0);
    assert_eq!(ended(&events), [(None, 1)]);
    for card in nines {
        assert_eq!(game.position().locate(card), Some(Zone::Junk));
    }
    for card in held {
        assert_ne!(game.position().locate(card), Some(Zone::Junk));
    }
    assert!(game.position().check_conservation());
}

#[test]
fn a_sampled_subgame_world_leaves_the_parents_hidden_cards_out() {
    let mut l = Layout::default();
    let (martian, _) = parents(&mut l);
    programmed(&mut l, "AAA", Color::Red, 9, base_deck(9), Vec::new());
    programmed(&mut l, "ZZZ", Color::Purple, 9, base_deck(9), Vec::new());
    // B's hidden 10 shares a back with two supply 10s. Whichever identity a
    // world leaves in B's hand is the one the child's supply does without.
    l.tech(Color::Green, 10, HAND_B, &[]);
    l.tech(Color::Blue, 10, base_deck(10), &[]);
    l.tech(Color::Yellow, 10, base_deck(10), &[]);
    let mut game = printed(l);
    dogma(&mut game, martian);
    no_decisions(&mut game);
    assert_eq!(game.subgame_depth(), 1);
    for seed in 0..12 {
        let sampled = game.sample_world(A, seed).unwrap();
        assert!(sampled.world.position().check_conservation(), "seed {seed}");
        assert_eq!(sampled.world.position().cards(base_deck(10)).len(), 2);
    }
}
