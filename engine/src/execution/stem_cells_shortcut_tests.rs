use super::common::*;
use crate::*;

fn fixture(ages: &[u8]) -> (Fixture, CardId, Vec<CardId>) {
    let mut f = Fixture::default();
    let source = f.add(base_card("B099").unwrap(), Zone::Board(A, Color::Yellow));
    let hand = ages
        .iter()
        .map(|&age| f.tech(Color::Red, age, Zone::Hand(A), vec![]))
        .collect();
    (f, source, hand)
}

fn start(game: &mut Game, source: CardId, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn canonical(game: &Game, menu: &Decision) -> usize {
    menu.options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| match option {
            Alternative::Move {
                verb: Verb::Score,
                target: TargetDescriptor::Known { card, .. },
            } => {
                let deck = game.position().card(*card).origin_deck();
                Some(((deck.age, deck.set as u8, card.0), index))
            }
            _ => None,
        })
        .min_by_key(|&(key, _)| key)
        .unwrap()
        .1
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(canonical(game, &menu)).unwrap(),
            status => return status,
        }
    }
    panic!("score phase failed to finish");
}

fn leaves(mut game: Game, depth: usize) -> Vec<Game> {
    assert!(depth < 10);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut next = game.clone();
                next.choose(index).unwrap();
                leaves(next, depth + 1)
            })
            .collect(),
        Status::Complete(ReturnValue::Unit) => vec![game],
        other => panic!("unexpected leaf {other:?}"),
    }
}

// Hands, score piles and junk are unordered: only their contents matter.
fn normalized(mut position: Position) -> Position {
    position.sort_unordered_piles();
    position
}

fn assert_outcome_classes<const F: bool>() {
    for (ages, manual_count, reduced_count) in [
        (vec![], 1, 1),
        (vec![1], 2, 2),
        (vec![1, 2, 4], 7, 2),
        (vec![1, 1, 2], 7, 2),
        (vec![1, 1, 1], 7, 2),
        (vec![0, 0, 1], 7, 2),
    ] {
        if !F && ages.contains(&0) {
            continue;
        }
        let (f, source, _) = fixture(&ages);
        let mut reduced = Game::seeded(f.position(), Expansions::with_figures(F), 83).unwrap();
        let mut manual = reduced.clone();
        manual.shortcuts_disabled = true;
        start(&mut reduced, source, Context::new(A));
        start(&mut manual, source, Context::new(A));
        let reduced = leaves(reduced, 0);
        let manual = leaves(manual, 0);
        assert_eq!(reduced.len(), reduced_count);
        assert_eq!(manual.len(), manual_count);
        let classes: Vec<_> = reduced
            .iter()
            .map(|g| normalized(g.position().clone()))
            .collect();
        for game in manual {
            assert!(classes.contains(&normalized(game.position().clone())));
            assert_eq!(game.rng, reduced[0].rng);
        }
    }
}

#[test]
fn every_back_group_order_and_initial_decline_survive_the_reduction() {
    assert_outcome_classes::<false>();
    assert_outcome_classes::<true>();
}

#[test]
fn canonical_events_rng_and_every_work_boundary_clone_match_the_oracle() {
    let (mut f, source, _) = fixture(&[2, 1, 1, 4]);
    f.figure(Figure::Tigernmas, Zone::Hand(A));
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 83).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, Context::new(A));
    start(&mut manual, source, Context::new(A));
    let mut events = Vec::new();
    loop {
        let mut clone = reduced.clone();
        let mut a = Vec::new();
        let mut b = Vec::new();
        let status = reduced.advance(1, &mut a).unwrap();
        assert_eq!(clone.advance(1, &mut b).unwrap(), status);
        assert_eq!(a, b);
        assert_eq!(clone, reduced);
        events.extend(a);
        match status {
            Status::WorkLimit => {}
            Status::Decision(menu) => reduced.choose(canonical(&reduced, &menu)).unwrap(),
            Status::Complete(ReturnValue::Unit) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    let mut expected = Vec::new();
    assert_eq!(
        finish(&mut manual, &mut expected),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events, expected);
    manual.shortcuts_disabled = false;
    assert_eq!(manual, reduced);
}

#[test]
fn future_blind_score_returns_sample_the_same_backs_for_every_removed_order() {
    let (mut f, source, _) = fixture(&[1, 2, 1, 2, 3]);
    f.tech(Color::Blue, 2, Zone::Score(A), vec![]);
    let mut reduced = Game::seeded(f.position(), Expansions::FIGURES, 83).unwrap();
    let mut manual = reduced.clone();
    manual.shortcuts_disabled = true;
    start(&mut reduced, source, Context::new(A));
    start(&mut manual, source, Context::new(A));
    let reduced = leaves(reduced, 0);
    for mut original in leaves(manual, 0)
        .into_iter()
        .filter(|g| g.position().cards(Zone::Hand(A)).is_empty())
    {
        let class = normalized(original.position().clone());
        let mut representative = reduced
            .iter()
            .find(|g| normalized(g.position().clone()) == class)
            .unwrap()
            .clone();
        original.shortcuts_disabled = false;
        let suffix = |game: &mut Game| {
            game.start(
                Operation::IssueDecree {
                    decree: Decree::Rivalry,
                },
                Context::new(B),
            )
            .unwrap();
            let mut events = Vec::new();
            for _ in 0..100 {
                match settle(game, &mut events) {
                    Status::Decision(menu) => {
                        assert!(matches!(
                            menu.options[0],
                            Alternative::BlindMove {
                                source: Zone::Score(A),
                                ..
                            }
                        ));
                        game.choose(0).unwrap();
                    }
                    Status::Complete(ReturnValue::Unit) => return events,
                    other => panic!("unexpected Rivalry suffix {other:?}"),
                }
            }
            panic!("Rivalry suffix failed to finish");
        };
        // A score pile is unordered: a blind pick may take a different card
        // of the same back, never a different back or random stream.
        let catalog = std::sync::Arc::clone(&original.position.catalog);
        let backs = |events: Vec<Event>| -> Vec<(u8, Zone)> {
            events
                .iter()
                .filter_map(|event| match event {
                    Event::Moved(m) => Some((catalog[usize::from(m.card.0)].value, m.destination)),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            backs(suffix(&mut original)),
            backs(suffix(&mut representative))
        );
        assert_eq!(original.rng, representative.rng);
    }
}

#[test]
fn live_score_and_membership_hazards_block_but_other_each_and_later_draw_hooks_do_not() {
    for (figure, owner, color, blocked) in [
        (Figure::ShenKuo, A, Color::Green, true),
        (Figure::Nightingale, A, Color::Yellow, true),
        (Figure::EmperorMeiji, A, Color::Purple, true),
        (Figure::Michelangelo, A, Color::Yellow, false),
        (Figure::Michelangelo, B, Color::Yellow, false),
        (Figure::Tigernmas, A, Color::Red, false),
        (Figure::GalileoGalilei, A, Color::Green, false),
        (Figure::JackieChan, A, Color::Red, false),
    ] {
        let (mut f, source, _) = fixture(&[1, 2, 4]);
        f.figure(figure, Zone::Board(owner, color));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        start(&mut game, source, Context::new(A));
        assert_eq!(
            decision(&mut game, &mut Vec::new()).options.len(),
            if blocked { 4 } else { 2 },
            "{figure:?} {owner:?}"
        );
    }
}

#[test]
fn hidden_foreign_controllers_get_one_public_back_group_at_a_time() {
    let make = |reverse| {
        let (mut f, source, hand) = fixture(&[1, 1, 2]);
        if reverse {
            f.placements
                .swap(usize::from(hand[0].0), usize::from(hand[1].0));
        }
        let mut context = Context::new(A);
        context.controller = B;
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        start(&mut game, source, context);
        decision(&mut game, &mut Vec::new())
    };
    let menu = make(false);
    assert_eq!(menu, make(true));
    assert_eq!(menu.player, B);
    // The score pile is unordered, so the two age-1 backs go first as one
    // group even though B can't tell them apart.
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.contains(&Alternative::Decline));
    assert!(menu.options.iter().any(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Score,
            back,
            ..
        } if back.age == 1
    )));
}

#[test]
fn a_midphase_score_source_change_is_a_guard_witness_not_a_legal_gameplay_sequence() {
    let (mut f, source, hand) = fixture(&[1, 1, 2, 4]);
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    let cover = f.tech(Color::Green, 8, Zone::Board(A, Color::Green), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    start(&mut game, source, Context::new(A));
    let mut events = Vec::new();
    // Covered Shen Kuo cannot see these scores yet: one canonical card.
    assert_eq!(decision(&mut game, &mut events).options.len(), 2);
    choose_card(&mut game, &mut events, hand[0]);
    assert_eq!(
        game.position.zone_mut(Zone::Board(A, Color::Green)).pop(),
        Some(cover)
    );
    game.position.zone_mut(Zone::Junk).push(cover);
    game.position.refresh_boards();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 3);
    assert!(!menu.options.contains(&Alternative::Decline));
    assert!(game.position().check_conservation());
}

#[test]
fn later_galileo_draw_reaction_is_not_skipped_by_score_grouping() {
    let (mut f, source, hand) = fixture(&[1, 2, 4]);
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let junked = f.tech(Color::Blue, 4, Zone::Deck(Deck::base(4)), vec![]);
    let drawn = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(decision(&mut game, &mut events).options.len(), 2);
    choose_card(&mut game, &mut events, hand[0]);
    let decks = decision(&mut game, &mut events);
    assert!(
        decks
            .options
            .iter()
            .all(|o| matches!(o, Alternative::Value(_)))
    );
    let index = decks
        .options
        .iter()
        .position(|o| *o == Alternative::Value(4))
        .unwrap();
    game.choose(index).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Score(A)), hand);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(game.position().cards(Zone::Junk), &[junked]);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [hand[0], hand[1], hand[2], junked, drawn]
    );
    assert!(moved(&events[3]).inside_karma);
    assert!(!moved(&events[4]).inside_karma);
}
