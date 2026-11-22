//! Unseen batch reductions: Safe departures to unordered piles (Enigma
//! Machine, Camouflage), Enigma Machine's Supply transfer, Handbag and Fashion
//! Mask tucks, and Escape Room's per-color top scoring.

use super::common::*;
use super::*;

fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng)> {
    assert!(depth < 14);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        Status::Complete(_) | Status::Draw(_) | Status::Winner(_) => {
            assert!(game.position.check_conservation());
            vec![(normalized(game.position), game.rng)]
        }
        Status::WorkLimit => panic!("unexpected oracle work limit"),
    }
}

fn compare(reduced: Game) -> (usize, usize) {
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert!(
        manual.iter().all(|o| reduced.contains(o)),
        "missing outcome"
    );
    assert!(
        reduced.iter().all(|o| manual.contains(o)),
        "invented outcome"
    );
    (reduced.len(), manual.len())
}

fn start_effect(game: &mut Game, source: CardId, effect: u16, actor: Player, demander: Player) {
    let mut context = Context::new(actor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

#[test]
fn handbag_and_fashion_mask_tucks_commute_across_colors_and_keep_same_color_order() {
    // Handbag (U080) branch 1: tuck all cards from score pile.
    // Two Red + one Blue score cards: 3! = 6 manual orders collapse to 2 (Red's internal order).
    let mut f = Fixture::default();
    let source = f.card("U080", Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Score(A), vec![]);
    f.tech(Color::Red, 2, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 3, Zone::Score(A), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::UNSEEN, 11).unwrap();
    start_effect(&mut game, source, 0, A, A);
    let menu = decision(&mut game, &mut Vec::new());
    let tuck_branch = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Branch(1))
        .unwrap();
    game.choose(tuck_branch).unwrap();
    assert_eq!(compare(game), (2, 6));

    // Fashion Mask (U109) effect 0: tuck a top with Prosperity or Industry of each color.
    let mut f = Fixture::default();
    let source = f.card("U109", Zone::Junk);
    for (color, icon) in [
        (Color::Red, Icon::Industry),
        (Color::Blue, Icon::Prosperity),
        (Color::Green, Icon::Industry),
    ] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
        let top = f.tech(color, 2, Zone::Board(A, color), vec![]);
        f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(icon);
    }
    let mut game = Game::seeded(f.position(), Expansions::UNSEEN, 13).unwrap();
    start_effect(&mut game, source, 0, A, A);
    // Optional safeguard of one tucked card multiplies each path by 4 (3 cards + decline).
    assert_eq!(compare(game), (4, 24));
}

#[test]
fn escape_room_scores_non_yellow_efficiency_tops_in_one_order_even_over_deep_stacks() {
    let mut f = Fixture::default();
    let source = f.card("U108", Zone::Board(A, Color::Yellow));
    for color in [Color::Red, Color::Blue, Color::Green] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
        let top = f.tech(color, 9, Zone::Board(A, color), vec![]);
        f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(Icon::Efficiency);
    }
    let mut game = Game::seeded(f.position(), Expansions::UNSEEN, 17).unwrap();
    start_effect(&mut game, source, 1, A, A);
    assert_eq!(compare(game), (1, 6));

    // Covering a score-watching Figure blocks the reduction.
    let mut f = Fixture::default();
    let source = f.card("U108", Zone::Board(A, Color::Yellow));
    f.figure(Figure::NapoleonBonaparte, Zone::Board(A, Color::Red));
    for color in [Color::Red, Color::Blue, Color::Green] {
        let top = f.tech(color, 9, Zone::Board(A, color), vec![]);
        f.cards[usize::from(top.0)].symbols[0] = Symbol::Icon(Icon::Efficiency);
    }
    let mut game = Game::seeded(
        f.position(),
        Expansions::UNSEEN.union(Expansions::FIGURES),
        17,
    )
    .unwrap();
    start_effect(&mut game, source, 1, A, A);
    let (reduced, manual) = compare(game);
    assert_eq!(reduced, manual);
    assert!(manual >= 6);
}

#[test]
fn enigma_machine_transfers_safe_to_hand_and_hand_to_supply_in_one_order() {
    for branch in [1, 2] {
        let mut f = Fixture::default();
        let source = f.card("U077", Zone::Board(A, Color::Red));
        let zone = if branch == 1 {
            Zone::Safe(A)
        } else {
            Zone::Hand(A)
        };
        // Include duplicate backs in Safe to verify same-back departures also collapse.
        f.tech(Color::Blue, 4, zone, vec![]);
        f.tech(Color::Green, 4, zone, vec![]);
        f.tech(Color::Yellow, 6, zone, vec![]);
        let mut game = Game::seeded(f.position(), Expansions::UNSEEN, 19).unwrap();
        start_effect(&mut game, source, 0, A, A);
        let menu = decision(&mut game, &mut Vec::new());
        let pick = menu
            .options
            .iter()
            .position(|o| *o == Alternative::Branch(branch))
            .unwrap();
        game.choose(pick).unwrap();
        assert_eq!(compare(game), (1, 6), "branch {branch}");
    }
}

#[test]
fn camouflage_scores_selected_secrets_in_one_order_preserving_safe_order() {
    let mut f = Fixture::default();
    let source = f.card("U066", Zone::Board(A, Color::Red));
    let s1 = f.tech(Color::Blue, 5, Zone::Safe(A), vec![]);
    let keep = f.tech(Color::Green, 3, Zone::Safe(A), vec![]);
    let s2 = f.tech(Color::Yellow, 5, Zone::Safe(A), vec![]);
    let keep2 = f.tech(Color::Purple, 4, Zone::Safe(A), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::UNSEEN, 23).unwrap();
    start_effect(&mut game, source, 0, A, A);
    let menu = decision(&mut game, &mut Vec::new());
    let score_branch = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Branch(1))
        .unwrap();
    game.choose(score_branch).unwrap();
    // Hidden Safe slots are picked in either order (2 group paths); scoring them collapses 2 -> 1.
    assert_eq!(compare(game.clone()), (2, 4));
    let mut events = Vec::new();
    let pick_first = decision(&mut game, &mut events);
    assert_eq!(pick_first.options.len(), 2);
    game.choose(0).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position.cards(Zone::Safe(A)), &[keep, keep2]);
    assert!(game.position.cards(Zone::Score(A)).contains(&s1));
    assert!(game.position.cards(Zone::Score(A)).contains(&s2));
}

#[test]
fn partial_safe_departures_to_unordered_piles_preserve_subsets_and_reduce_distinct_backs() {
    // 1. Partial Safe-to-Score quota batch with ByOrigin canonicalizes distinct
    //    public-back subsets (3 choose 2 = 3 instead of 6) while preserving
    //    distinct same-back secret slot choices.
    let mut f = Fixture::default();
    let score_two = f.tech(
        Color::Blue,
        5,
        Zone::Board(A, Color::Blue),
        vec![
            build::verb(Verb::Score, Selector::Safe(Who::Executor))
                .to(Destination::Score(Who::Executor))
                .exactly(Expr::Const(2))
                .shortcut(Shortcut::ByOrigin {
                    descending: false,
                    quota: true,
                }),
        ],
    );
    f.tech(Color::Red, 1, Zone::Safe(A), vec![]);
    f.tech(Color::Green, 3, Zone::Safe(A), vec![]);
    f.tech(Color::Yellow, 5, Zone::Safe(A), vec![]);
    let mut distinct = Game::seeded(f.position(), Expansions::UNSEEN, 31).unwrap();
    start_effect(&mut distinct, score_two, 0, A, A);
    assert_eq!(compare(distinct), (3, 6));

    // With duplicate backs in Safe, both slots stay reachable without reading hidden fronts.
    let mut f = Fixture::default();
    let score_two = f.tech(
        Color::Blue,
        5,
        Zone::Board(A, Color::Blue),
        vec![
            build::verb(Verb::Score, Selector::Safe(Who::Executor))
                .to(Destination::Score(Who::Executor))
                .exactly(Expr::Const(2))
                .shortcut(Shortcut::ByOrigin {
                    descending: false,
                    quota: true,
                }),
        ],
    );
    f.tech(Color::Red, 2, Zone::Safe(A), vec![]);
    f.tech(Color::Green, 2, Zone::Safe(A), vec![]);
    f.tech(Color::Yellow, 5, Zone::Safe(A), vec![]);
    let mut tied = Game::seeded(f.position(), Expansions::UNSEEN, 31).unwrap();
    start_effect(&mut tied, score_two, 0, A, A);
    assert_eq!(compare(tied), (4, 6));

    // 2. Safe-to-Junk with AnyOrder: mandatory `all()` collapses 6 -> 1, while
    //    partial `exactly(2)` preserves all 6 distinct-order paths (since AnyOrder
    //    itself requires `all_must_go`).
    let mut f = Fixture::default();
    let junk_all = f.tech(
        Color::Red,
        5,
        Zone::Board(A, Color::Red),
        vec![
            build::verb(Verb::Junk, Selector::Safe(Who::Executor))
                .to(Destination::Junk)
                .all()
                .shortcut(Shortcut::AnyOrder),
        ],
    );
    let junk_two = f.tech(
        Color::Green,
        5,
        Zone::Board(A, Color::Green),
        vec![
            build::verb(Verb::Junk, Selector::Safe(Who::Executor))
                .to(Destination::Junk)
                .exactly(Expr::Const(2))
                .shortcut(Shortcut::AnyOrder),
        ],
    );
    f.tech(Color::Red, 1, Zone::Safe(A), vec![]);
    f.tech(Color::Blue, 3, Zone::Safe(A), vec![]);
    f.tech(Color::Yellow, 5, Zone::Safe(A), vec![]);
    let pos = f.position();
    let mut all_game = Game::seeded(pos.clone(), Expansions::UNSEEN, 37).unwrap();
    start_effect(&mut all_game, junk_all, 0, A, A);
    assert_eq!(compare(all_game), (1, 6));

    let mut partial_game = Game::seeded(pos, Expansions::UNSEEN, 37).unwrap();
    start_effect(&mut partial_game, junk_two, 0, A, A);
    assert_eq!(compare(partial_game), (6, 6));
}
