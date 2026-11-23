use super::*;
use common::*;

fn start_effect(game: &mut Game, source: CardId, effect: u16, player: Player) {
    let mut context = Context::new(player);
    context.demander = player.opponent();
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

fn oracle_outcomes(mut game: Game, depth: usize) -> Vec<(Position, Rng, Status)> {
    assert!(depth < 12, "oracle depth exceeded");
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                oracle_outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            assert!(game.position.check_conservation());
            let mut position = game.position;
            position.sort_unordered_piles();
            vec![(position, game.rng, status)]
        }
    }
}

fn assert_same_outcomes(reduced: Game, mut manual: Game) {
    manual.shortcuts_disabled = true;
    let reduced_paths = oracle_outcomes(reduced, 0);
    let manual_paths = oracle_outcomes(manual, 0);
    assert!(
        reduced_paths.len() < manual_paths.len(),
        "expected reduction: reduced={} manual={}",
        reduced_paths.len(),
        manual_paths.len()
    );
    assert!(
        manual_paths.iter().all(|p| reduced_paths.contains(p)),
        "reduced missed a real manual outcome"
    );
    assert!(
        reduced_paths.iter().all(|p| manual_paths.contains(p)),
        "reduced produced a non-manual outcome"
    );
}

#[test]
fn sanitation_exchange_canonicalizes_subset_order_even_with_top_figures_on_board() {
    let mut f = Fixture::default();
    let source = f.card("B069", Zone::Junk);
    // Live Figures on both boards (including Yi Sun-sin watching On::Exchange)
    // do not run during selection, and selected cohorts are sorted back to
    // logical-pile order before exchange commitment or hook discovery.
    f.figure(Figure::Gilgamesh, Zone::Board(A, Color::Red));
    f.figure(Figure::YiSunSin, Zone::Board(B, Color::Red));
    for code in ["B032", "B033", "B034"] {
        f.card(code, Zone::Hand(A));
    }
    f.card("B001", Zone::Hand(B));
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 19).unwrap();
    let mut manual = reduced.clone();
    start_effect(&mut reduced, source, 0, A);
    start_effect(&mut manual, source, 0, A);

    // Three tied highest cards for count=2: reduced offers only the first two
    // as first pick (3 subsets total), while unrestricted offers all 3 (6 paths).
    let mut reduced_probe = reduced.clone();
    let menu = decision(&mut reduced_probe, &mut Vec::new());
    assert_eq!(menu.options.len(), 2);

    assert_same_outcomes(reduced, manual);
}

fn board_card(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let color = Card::from_code(code).unwrap().color;
    f.card(code, Zone::Board(player, color))
}

fn board_figure(f: &mut Fixture, figure: Figure, player: Player) -> CardId {
    let color = Card::from_code(figure.code()).unwrap().color;
    f.figure(figure, Zone::Board(player, color))
}

fn first_code(metadata: &[CardMetadata], color: Color, value: u8) -> &'static str {
    metadata
        .iter()
        .find(|card| card.color == color && card.value == value)
        .unwrap()
        .code
}

#[test]
fn hypersonics_dedups_cross_deck_matching_pair_and_keeps_same_deck_and_return_figures() {
    let base_red_code = first_code(&BASE_METADATA, Color::Red, 2);
    let base_green_code = first_code(&BASE_METADATA, Color::Green, 2);
    let echoes_green_code = first_code(&ECHOES_METADATA, Color::Green, 2);

    // 1. Cross-deck matching pair (Base 2 Red + Echoes 2 Green) commutes into
    //    a single automatic option with no decision menu.
    let mut f = Fixture::default();
    let source = f.card("B110", Zone::Junk);
    board_card(&mut f, base_red_code, A);
    board_card(&mut f, echoes_green_code, A);
    let mut reduced = Game::seeded(f.position(), Expansions::ECHOES, 11).unwrap();
    let mut manual = reduced.clone();
    start_effect(&mut reduced, source, 0, A);
    start_effect(&mut manual, source, 0, A);
    assert!(matches!(
        settle(&mut reduced.clone(), &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    ));
    let mut manual_probe = manual.clone();
    manual_probe.shortcuts_disabled = true;
    assert_eq!(
        decision(&mut manual_probe, &mut Vec::new()).options.len(),
        2
    );
    assert_same_outcomes(reduced, manual);

    // 2. Same-deck matching pair (two Base 2 cards) keeps both return orders
    //    because deck bottom order differs.
    let mut f = Fixture::default();
    let source = f.card("B110", Zone::Junk);
    board_card(&mut f, base_red_code, A);
    board_card(&mut f, base_green_code, A);
    let mut same_deck = Game::seeded(f.position(), Expansions::BASE, 11).unwrap();
    start_effect(&mut same_deck, source, 0, A);
    assert_eq!(decision(&mut same_deck, &mut Vec::new()).options.len(), 2);

    // 3. Covered James Clerk Maxwell (watches On::Return) under one card of a
    //    cross-deck pair blocks deduplication because exposing him first changes
    //    whether the second return triggers his karma.
    let mut f = Fixture::default();
    let source = f.card("B110", Zone::Junk);
    let maxwell_color = Card::from_code(Figure::JamesClerkMaxwell.code())
        .unwrap()
        .color;
    let other_color = if maxwell_color == Color::Green {
        Color::Red
    } else {
        Color::Green
    };
    board_figure(&mut f, Figure::JamesClerkMaxwell, A);
    board_card(&mut f, first_code(&BASE_METADATA, maxwell_color, 2), A);
    board_card(&mut f, first_code(&ECHOES_METADATA, other_color, 2), A);
    let mut blocked = Game::seeded(f.position(), Expansions::from_bits(5).unwrap(), 11).unwrap();
    start_effect(&mut blocked, source, 0, A);
    assert_eq!(decision(&mut blocked, &mut Vec::new()).options.len(), 2);
}

#[test]
fn fusion_skips_value_choice_when_neither_step_matches_any_top_card() {
    // Score Fusion (11) with no top 10 or top 9 on board: both value options
    // score nothing and terminate, so reduced skips the menu completely.
    let mut f = Fixture::default();
    let source = board_card(&mut f, "B107", A);
    board_card(&mut f, first_code(&BASE_METADATA, Color::Blue, 1), A);
    let mut reduced = Game::seeded(f.position(), Expansions::BASE, 5).unwrap();
    let mut manual = reduced.clone();
    start_effect(&mut reduced, source, 0, A);
    start_effect(&mut manual, source, 0, A);
    assert!(matches!(
        settle(&mut reduced.clone(), &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    ));
    let mut manual_probe = manual.clone();
    manual_probe.shortcuts_disabled = true;
    assert_eq!(
        decision(&mut manual_probe, &mut Vec::new()).options,
        vec![Alternative::Value(10), Alternative::Value(9)]
    );
    assert_same_outcomes(reduced, manual);

    // When one value matches (top 9) and the other does not (no top 10),
    // both options remain because choosing 10 stops scoring while 9 continues.
    let mut f = Fixture::default();
    let source = board_card(&mut f, "B107", A);
    board_card(&mut f, first_code(&BASE_METADATA, Color::Blue, 9), A);
    let mut keep_choice = Game::seeded(f.position(), Expansions::BASE, 5).unwrap();
    start_effect(&mut keep_choice, source, 0, A);
    assert_eq!(
        decision(&mut keep_choice, &mut Vec::new()).options,
        vec![Alternative::Value(10), Alternative::Value(9)]
    );
}

#[test]
fn sanitation_with_emperor_meiji_does_not_panic_after_selecting_borrowed_hand_card() {
    // When Emperor Meiji is active on A's first action, A's logical hand
    // includes B's hand. Picking B's tied highest card first leaves A's cards
    // in `targets` while `selected[side]` holds a card physically in `Hand(B)`.
    // `reduce_exchange_order` must check already-selected cards' physical pile
    // as well as remaining candidates.
    let mut f = Fixture::default();
    let source = f.card("B069", Zone::Junk);
    board_figure(&mut f, Figure::EmperorMeiji, A);
    let own_a = f.card("B032", Zone::Hand(A));
    let own_b = f.card("B033", Zone::Hand(A));
    let borrowed = f.card("B034", Zone::Hand(B));
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 29).unwrap();
    let mut manual = reduced.clone();
    start_effect(&mut reduced, source, 0, A);
    start_effect(&mut manual, source, 0, A);

    // Pick B's borrowed card first on the reduced game: must not panic and must
    // offer both of A's remaining tied cards.
    let mut probe = reduced.clone();
    let first_menu = decision(&mut probe, &mut Vec::new());
    let borrowed_idx = first_menu
        .options
        .iter()
        .position(|option| {
            matches!(
                option,
                Alternative::Select {
                    target: TargetDescriptor::Known { card, .. }
                } if *card == borrowed
            )
        })
        .unwrap();
    probe.choose(borrowed_idx).unwrap();
    let second_menu = decision(&mut probe, &mut Vec::new());
    assert_eq!(second_menu.options.len(), 2);
    assert!(second_menu.options.iter().any(|option| {
        matches!(
            option,
            Alternative::Select {
                target: TargetDescriptor::Known { card, .. }
            } if *card == own_a
        )
    }));
    assert!(second_menu.options.iter().any(|option| {
        matches!(
            option,
            Alternative::Select {
                target: TargetDescriptor::Known { card, .. }
            } if *card == own_b
        )
    }));

    // Full oracle check: identical reachable outcomes with and without shortcuts.
    manual.shortcuts_disabled = true;
    let reduced_paths = oracle_outcomes(reduced, 0);
    let manual_paths = oracle_outcomes(manual, 0);
    assert!(manual_paths.iter().all(|p| reduced_paths.contains(p)));
    assert!(reduced_paths.iter().all(|p| manual_paths.contains(p)));
}
