//! B045 Reformation's tuck: up to one card per splayed color, stopping at
//! will. Placements of different colors commute, so the reduced menu keeps a
//! monotone color floor after each tuck; every subset, stop and within-color
//! order remains a real choice.
use super::common::*;
use super::*;

fn fixture(colors: &[Color], quota: usize) -> (Fixture, CardId, Vec<CardId>) {
    let mut f = Fixture::default();
    for color in Color::ALL {
        for _ in 0..2 {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
        }
    }
    let source = f.add(base_card("B045").unwrap(), Zone::Board(A, Color::Purple));
    let hand = colors
        .iter()
        .enumerate()
        .map(|(i, &color)| f.tech(color, (i + 1) as u8, Zone::Hand(A), vec![]))
        .collect();
    assert!(quota <= Color::ALL.len());
    (f, source, hand)
}

fn position(f: Fixture, quota: usize) -> Position {
    let mut position = f.position();
    for color in Color::ALL.into_iter().take(quota) {
        position.players[A.index()].splays[color as usize] = Splay::Right;
    }
    position
}

fn start(game: &mut Game, source: CardId, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 1,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn tucks(menu: &Decision) -> usize {
    menu.options
        .iter()
        .filter(|option| matches!(option, Alternative::Move { .. }))
        .count()
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                verb: Verb::Tuck,
                target: TargetDescriptor::Known { card, .. },
            } => Some(((game.position.card(*card).color as u8, *card), index)),
            _ => None,
        })
        .min_by_key(|(key, _)| *key)
        .map_or(0, |(_, index)| index)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            status => return status,
        }
    }
    panic!("Reformation did not finish");
}

fn outcomes(mut game: Game, depth: usize) -> Vec<(Game, Status)> {
    assert!(depth < 10);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut fork = game.clone();
                fork.choose(choice).unwrap();
                outcomes(fork, depth + 1)
            })
            .collect(),
        status => {
            game.shortcuts_disabled = false;
            assert!(game.position.check_conservation());
            vec![(game, status)]
        }
    }
}

/// Every reduced outcome is reachable unreduced and vice versa, comparing
/// exact pile order, hands, results and RNG. Returns both path counts.
fn all_paths<const F: bool>(position: Position, source: CardId) -> (usize, usize) {
    let mut reduced = Game::seeded(position, Expansions::with_figures(F), 42).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, Context::new(A));
    start(&mut manual, source, Context::new(A));
    let reduced = outcomes(reduced, 0);
    let manual = outcomes(manual, 0);
    assert!(manual.iter().all(|result| reduced.contains(result)));
    assert!(reduced.iter().all(|result| manual.contains(result)));
    (manual.len(), reduced.len())
}

#[test]
fn every_outcome_survives_cross_color_grouping_and_within_color_orders_are_not_collapsed() {
    use Color::*;
    // Unreduced paths are ordered prefixes; reduced ones are color-sorted.
    for (colors, quota, paths) in [
        (vec![Red, Blue, Green], 3, (16, 8)),
        (vec![Red, Red, Blue], 3, (16, 10)),
        (vec![Red, Red, Red], 3, (16, 16)),
        (vec![Blue, Green], 3, (5, 4)),
        (vec![], 3, (1, 1)),
        (vec![Red, Blue, Green], 1, (4, 4)),
        (vec![Red, Blue, Green], 0, (1, 1)),
    ] {
        let (f, source, _) = fixture(&colors, quota);
        let position = position(f, quota);
        assert_eq!(all_paths::<false>(position.clone(), source), paths);
        assert_eq!(all_paths::<true>(position, source), paths);
    }
}

#[test]
fn base_and_figure_cards_share_a_color_group_but_never_lose_their_internal_orders() {
    let (mut f, source, _) = fixture(&[Color::Blue, Color::Red], 3);
    f.figure(Figure::Archimedes, Zone::Hand(A));
    // The Figure stays buried, so its effect-values modifier never appears.
    assert_eq!(all_paths::<true>(position(f, 3), source), (16, 10));
}

#[test]
fn every_tuck_may_stop_and_the_count_is_fixed_before_the_first() {
    let (f, source, hand) = fixture(&[Color::Red, Color::Blue, Color::Blue], 2);
    let mut game = Game::new(position(f, 2), Expansions::BASE).unwrap();
    start(&mut game, source, Context::new(A));
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(tucks(&menu), 3);
    assert_eq!(menu.options.last(), Some(&Alternative::Decline));
    choose_card(&mut game, &mut events, hand[2]);
    // Red is below the blue floor; the other blue card stays a real choice.
    let menu = decision(&mut game, &mut events);
    assert_eq!(tucks(&menu), 1);
    assert_eq!(menu.options.last(), Some(&Alternative::Decline));
    choose_card(&mut game, &mut events, hand[1]);
    // Two splayed colors: the quota is spent, even though a card remains.
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(game.position.cards(Zone::Hand(A)), [hand[0]]);
    assert_eq!(
        &game.position.cards(Zone::Board(A, Color::Blue))[..2],
        &[hand[1], hand[2]]
    );
}

#[test]
fn meiji_live_membership_blocks_the_floor_and_expires_normally() {
    for opponent in [false, true] {
        for expired in [false, true] {
            let (mut f, source, hand) = fixture(&[Color::Red, Color::Blue, Color::Green], 5);
            f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
            if opponent {
                f.tech(Color::Yellow, 1, Zone::Hand(B), vec![]);
            }
            let mut game = Game::new(position(f, 5), Expansions::FIGURES).unwrap();
            if expired {
                game.begin_action(ActionKind::Regular).unwrap();
                game.begin_action(ActionKind::Regular).unwrap();
            }
            start(&mut game, source, Context::new(A));
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(tucks(&menu), 3 + usize::from(opponent && !expired));
            choose_card(&mut game, &mut events, hand[2]);
            // Live Meiji keeps lower colors; once expired, nothing is above
            // green and the lone stop resolves without a decision.
            if expired {
                assert!(matches!(
                    settle(&mut game, &mut events),
                    Status::Complete(_)
                ));
            } else {
                let menu = decision(&mut game, &mut events);
                assert_eq!(tucks(&menu), 2 + usize::from(opponent));
                assert_eq!(menu.options.last(), Some(&Alternative::Decline));
            }
        }
    }
}

#[test]
fn unrelated_live_figures_do_not_blanket_disable_the_color_floor() {
    for (figure, color, owner) in [
        (Figure::ShenKuo, Color::Green, A),
        (Figure::Nightingale, Color::Yellow, A),
        (Figure::EmperorMeiji, Color::Purple, B),
    ] {
        let (mut f, source, hand) = fixture(&[Color::Red, Color::Blue, Color::Purple], 3);
        f.figure(figure, Zone::Board(owner, color));
        let mut game = Game::new(position(f, 3), Expansions::FIGURES).unwrap();
        start(&mut game, source, Context::new(A));
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, hand[1]);
        let menu = decision(&mut game, &mut events);
        assert_eq!(tucks(&menu), 1, "{figure:?}");
    }
}

#[test]
fn a_figure_landing_on_an_empty_color_blocks_the_floor() {
    for figure in [false, true] {
        let (mut f, source, hand) = fixture(&[Color::Red, Color::Green], 5);
        if figure {
            f.figure(Figure::Archimedes, Zone::Hand(A));
        } else {
            f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        }
        let mut position = position(f, 5);
        let removed = std::mem::take(position.zone_mut(Zone::Board(A, Color::Blue)));
        position.zone_mut(Zone::Junk).extend(removed);
        position.players[A.index()].splays[Color::Blue as usize] = Splay::None;
        let mut game = Game::new(position, Expansions::FIGURES).unwrap();
        start(&mut game, source, Context::new(A));
        let mut events = Vec::new();
        choose_card(&mut game, &mut events, hand[1]);
        // Lower colors stay offered only while a tucked Figure could become a top.
        match settle(&mut game, &mut events) {
            Status::Decision(menu) => assert!(figure && tucks(&menu) == 2),
            status => assert!(!figure && matches!(status, Status::Complete(_))),
        }
    }
}

#[test]
fn foreign_controller_menus_do_not_depend_on_hidden_colors() {
    let make = |swap| {
        let (mut f, source, hand) = fixture(&[Color::Red, Color::Blue, Color::Green], 3);
        for &card in &hand {
            f.cards[usize::from(card.0)].value = 1;
        }
        if swap {
            f.cards[usize::from(hand[0].0)].color = Color::Blue;
            f.cards[usize::from(hand[1].0)].color = Color::Red;
        }
        let mut game = Game::new(position(f, 3), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        start(&mut game, source, context);
        game
    };
    let mut first = make(false);
    let mut second = make(true);
    let menu = decision(&mut first, &mut Vec::new());
    assert_eq!(menu, decision(&mut second, &mut Vec::new()));
    assert_eq!(menu.options.len(), 4);
    assert!(menu.options[..3].iter().all(|o| matches!(
        o,
        Alternative::Move {
            target: TargetDescriptor::Hidden { .. },
            ..
        }
    )));
}

#[test]
fn each_resumption_rechecks_newly_exposed_membership_including_under_suppression() {
    for suppressed in [false, true] {
        let (mut f, source, hand) = fixture(&[Color::Red, Color::Blue, Color::Green], 3);
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        let cover = f.tech(Color::Purple, 8, Zone::Board(A, Color::Purple), vec![]);
        let mut game = Game::new(position(f, 3), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.set_suppress_karma(suppressed);
        start(&mut game, source, context);
        let mut events = Vec::new();
        assert_eq!(tucks(&decision(&mut game, &mut events)), 3);
        choose_card(&mut game, &mut events, hand[2]);
        advance_to_events(&mut game, &mut events, 1);
        // Isolated guard witness, not a claimed gameplay Would-Tuck hook.
        assert_eq!(
            game.position.zone_mut(Zone::Board(A, Color::Purple)).pop(),
            Some(cover)
        );
        game.position.zone_mut(Zone::Junk).push(cover);
        game.position.refresh_boards();
        // The exposed live Meiji now blocks the floor below green.
        let menu = decision(&mut game, &mut events);
        assert_eq!(tucks(&menu), 2);
        assert!(menu.options.contains(&Alternative::Decline));
        assert!(game.position.check_conservation());
    }
}

#[test]
fn local_proof_rejects_wrong_verbs_destinations_and_aliases() {
    let (f, _, hand) = fixture(&[Color::Red, Color::Blue, Color::Green], 3);
    let game = Game::new(position(f, 3), Expansions::BASE).unwrap();
    let context = Context::new(A);
    let original = game
        .select_bound(build::HAND, context, None, SelectionParameter::None)
        .unwrap();
    for variant in 0..5 {
        let mut batch = MoveAll::new(
            Verb::Tuck,
            build::HAND,
            Destination::Board(Who::Executor),
            context,
        );
        batch.limit = Some(3);
        batch.stop = Stop::AnyTime;
        batch.shortcut = Some(Shortcut::ByColor);
        // A green tuck already happened in this ordered suffix, from a menu
        // that offered every hand card.
        batch.visited.push(Captured::new(hand[2], Evidence::PUBLIC));
        batch.ordered_since = Some(0);
        batch.seen = original.iter().map(|t| t.card).collect();
        let mut targets: Vec<_> = original
            .iter()
            .copied()
            .filter(|t| t.card != hand[2])
            .collect();
        let reduce = match variant {
            0 => true,
            1 => {
                batch.verb = Verb::Score;
                false
            }
            2 => {
                batch.to = Destination::Score(Who::Executor);
                false
            }
            3 => {
                batch.context.controller = B;
                false
            }
            _ => {
                targets[0].physical = Zone::Hand(B);
                false
            }
        };
        let expected = if reduce { Vec::new() } else { targets.clone() };
        game.reduce_batch_moves(&mut batch, &mut targets);
        assert_eq!(targets, expected, "variant {variant}");
    }
}

#[test]
fn canonical_events_rng_and_clones_match_at_every_tuck_boundary() {
    let (f, source, _) = fixture(&[Color::Red, Color::Blue, Color::Blue], 3);
    let mut game = Game::seeded(position(f, 3), Expansions::FIGURES, 42).unwrap();
    let mut manual = game.clone();
    manual.shortcuts_disabled = true;
    start(&mut game, source, Context::new(A));
    start(&mut manual, source, Context::new(A));
    let mut expected = game.clone();
    let mut all = Vec::new();
    let status = finish(&mut expected, &mut all);
    let mut manual_events = Vec::new();
    assert_eq!(finish(&mut manual, &mut manual_events), status);
    assert_eq!(manual_events, all);
    manual.shortcuts_disabled = false;
    assert_eq!(manual, expected);
    assert!(all.iter().all(
        |event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Tuck) && !m.inside_karma)
    ));
    let mut prefix = Vec::new();
    for _ in 0..200 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), status);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => game.choose(canonical(&game, &menu)).unwrap(),
            result => {
                assert_eq!(result, status);
                assert_eq!(game, expected);
                return;
            }
        }
    }
    panic!("Reformation continuation did not finish");
}
