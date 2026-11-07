use super::common::*;
use super::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn invoke(game: &mut Game, card: CardId, effect: u16) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}
fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = menu
                    .options
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, option)| match option {
                        Alternative::Move {
                            target: TargetDescriptor::Known { card, .. },
                            ..
                        } => {
                            let deck = game.position.card(*card).origin_deck();
                            (0, deck.age, deck.set as u8, card.0)
                        }
                        Alternative::Decline => (2, 0, 0, 0),
                        _ => (1, 0, 0, 0),
                    })
                    .unwrap()
                    .0;
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("board-depth invocation did not terminate")
}

#[test]
fn depth_boundaries_and_ordinary_buried_admission_remain_distinct() {
    let mut f = Fixture::default();
    let cards =
        ["B001", "B002", "B003", "B016"].map(|code| base(&mut f, code, Zone::Board(A, Color::Red)));
    let game = Game::new(f.position(), Expansions::BASE).unwrap();
    let select = |region| {
        game.select_board(
            Who::Executor,
            ColorSet::Only(&[Color::Red]),
            region,
            Context::new(A),
            Regs {
                bound: None,
                chosen: SelectionParameter::None,
                ..Regs::EMPTY
            },
        )
        .unwrap()
    };
    assert_eq!(
        select(BoardRegion::Bottom)
            .iter()
            .map(|t| t.card)
            .collect::<Vec<_>>(),
        [cards[0]]
    );
    assert_eq!(
        select(BoardRegion::BelowTop(2))
            .iter()
            .map(|t| t.card)
            .collect::<Vec<_>>(),
        cards[..2]
    );
    assert!(select(BoardRegion::BelowTop(4)).is_empty());
    assert!(select(BoardRegion::BelowTop(5)).is_empty());
    assert!(
        game.select_board(
            Who::Executor,
            ColorSet::Only(&[Color::Blue]),
            BoardRegion::Bottom,
            Context::new(A),
            Regs {
                bound: None,
                chosen: SelectionParameter::None,
                ..Regs::EMPTY
            }
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        !game
            .valid_source(target(cards[0], Zone::Board(A, Color::Red)))
            .unwrap()
    );
    assert!(game.valid_source(select(BoardRegion::Bottom)[0]).unwrap());
    // The chosen register holds a value, colors, an icon or a card.
    // Card-valued choices now include a per-observer provenance mask.
    assert!(std::mem::size_of::<SelectionParameter>() <= 6);
    assert!(std::mem::size_of::<Frame>() <= 56);
}

#[test]
fn retained_depth_follows_identity_on_another_board_not_original_color_alone() {
    let mut f = Fixture::default();
    let below = base(&mut f, "B001", Zone::Board(B, Color::Red));
    let retained = base(&mut f, "B002", Zone::Board(B, Color::Red));
    base(&mut f, "B003", Zone::Board(B, Color::Red));
    base(&mut f, "B016", Zone::Board(A, Color::Red));
    let game = Game::new(f.position(), Expansions::BASE).unwrap();
    let selected = game
        .select_board(
            Who::Either,
            ColorSet::Of(Reg::Bound),
            BoardRegion::BeneathBound,
            Context::new(A),
            Regs {
                bound: Some(game.held(retained)),
                chosen: SelectionParameter::Value(11),
                ..Regs::EMPTY
            },
        )
        .unwrap();
    assert_eq!(selected.iter().map(|t| t.card).collect::<Vec<_>>(), [below]);
    assert!(
        game.select_board(
            Who::Either,
            ColorSet::Of(Reg::Bound),
            BoardRegion::BeneathBound,
            Context::new(A),
            Regs {
                bound: Some(game.held(below)),
                chosen: SelectionParameter::None,
                ..Regs::EMPTY
            }
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn coal_chooses_one_color_and_scores_sequential_new_tops() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B046", Zone::Junk);
    let cards = ["B001", "B002", "B003"].map(|code| base(&mut f, code, Zone::Board(A, Color::Red)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 2);
    let menu = decision(&mut game, &mut Vec::new());
    assert!(
        menu.options
            .contains(&Alternative::Colors(1 << Color::Blue as u8))
    );
    let mut declined = game.clone();
    declined
        .choose(
            menu.options
                .iter()
                .position(|o| *o == Alternative::Decline)
                .unwrap(),
        )
        .unwrap();
    finish(&mut declined, &mut Vec::new());
    assert_eq!(declined.position.cards(Zone::Board(A, Color::Red)), cards);
    game.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
            .unwrap(),
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Score(A)), [cards[2], cards[1]]);
    assert_eq!(game.position.cards(Zone::Board(A, Color::Red)), [cards[0]]);
}

#[test]
fn steam_engine_scores_the_new_physical_bottom_after_two_tucks() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B049", Zone::Board(A, Color::Yellow));
    let first = base(&mut f, "B038", Zone::Deck(Deck::base(4)));
    let second = base(&mut f, "B039", Zone::Deck(Deck::base(4)));
    let untouched = base(&mut f, "B056", Zone::Deck(Deck::base(6)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Score(A)), [first]);
    assert_eq!(
        game.position.cards(Zone::Board(A, Color::Yellow)),
        [second, source]
    );
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(6))), [untouched]);
}

#[test]
fn measurement_reveals_returns_then_draws_live_stack_size_without_splay_success() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B051", Zone::Junk);
    let returned = base(&mut f, "B016", Zone::Hand(A));
    base(&mut f, "B001", Zone::Board(A, Color::Red));
    let drawn = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(game.position.cards(Zone::Hand(A)), [drawn]);
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(2))), [returned]);
    assert_eq!(game.position.splay(A, Color::Red), Splay::None);
    let reveal = events
        .iter()
        .position(|event| matches!(event, Event::Revealed { card, .. } if *card == returned))
        .unwrap();
    let returned_at = events.iter().position(|event| matches!(event, Event::Moved(movement) if movement.card == returned && movement.verb == Some(Verb::Return))).unwrap();
    assert!(reveal < returned_at);
}

#[test]
fn combustion_counts_demanders_colors_and_returns_buried_bottom() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B066", Zone::Junk);
    let bottom = base(&mut f, "B001", Zone::Board(A, Color::Red));
    let top = base(&mut f, "B002", Zone::Board(A, Color::Red));
    base(&mut f, "B003", Zone::Board(B, Color::Red));
    base(&mut f, "B008", Zone::Board(B, Color::Green));
    let score = ["B004", "B010"].map(|code| base(&mut f, code, Zone::Score(A)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    finish(&mut game, &mut Vec::new());
    assert!(game.position.cards(Zone::Score(A)).is_empty());
    assert!(
        score
            .iter()
            .all(|id| game.position.cards(Zone::Score(B)).contains(id))
    );
    invoke(&mut game, source, 1);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Board(A, Color::Red)), [top]);
    assert_eq!(game.position.cards(Zone::Deck(Deck::base(1))), [bottom]);
}

#[test]
fn genetics_scores_buried_cards_beneath_original_draw_not_drawn_identity() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B093", Zone::Junk);
    let below =
        ["B010", "B032", "B042"].map(|code| base(&mut f, code, Zone::Board(A, Color::Blue)));
    let drawn = base(&mut f, "B113", Zone::Deck(Deck::base(11)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Board(A, Color::Blue)), [drawn]);
    assert!(
        below
            .iter()
            .all(|id| game.position.cards(Zone::Score(A)).contains(id))
    );
}

#[test]
fn astrogeology_transfers_exactly_below_four_and_preserves_top_suffix() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B106", Zone::Junk);
    let stack = ["B001", "B002", "B003", "B016", "B037", "B046"]
        .map(|code| base(&mut f, code, Zone::Board(A, Color::Red)));
    let drawn = base(&mut f, "B107", Zone::Deck(Deck::base(11)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position.cards(Zone::Board(A, Color::Red)), &stack[2..]);
    assert_eq!(game.position.splay(A, Color::Red), Splay::Aslant);
    assert!(
        stack[..2]
            .iter()
            .all(|id| game.position.cards(Zone::Hand(A)).contains(id))
    );
    assert!(game.position.cards(Zone::Hand(A)).contains(&drawn));
}

fn solar_fixture() -> (Position, CardId) {
    let mut f = Fixture::default();
    let source = base(&mut f, "B113", Zone::Junk);
    for code in ["B010", "B011", "B032", "B042", "B043", "B093"] {
        base(&mut f, code, Zone::Board(A, Color::Blue));
    }
    base(&mut f, "B112", Zone::Deck(Deck::base(11)));
    (f.position(), source)
}
fn outcomes(mut game: Game, depth: usize) -> Vec<Position> {
    assert!(depth < 8);
    match settle(&mut game, &mut Vec::new()) {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|index| {
                let mut child = game.clone();
                child.choose(index).unwrap();
                outcomes(child, depth + 1)
            })
            .collect(),
        Status::Complete(_) => vec![game.position],
        status => panic!("unexpected terminal {status:?}"),
    }
}
#[test]
fn solar_return_origin_oracle_preserves_same_origin_orders_and_exact_canonical_replay() {
    let (position, source) = solar_fixture();
    let mut reduced = Game::seeded(position, Expansions::BASE, 83).unwrap();
    let mut unrestricted = reduced.clone();
    unrestricted.shortcuts_disabled = true;
    invoke(&mut reduced, source, 0);
    invoke(&mut unrestricted, source, 0);
    let a = outcomes(reduced.clone(), 0);
    let b = outcomes(unrestricted.clone(), 0);
    assert!(a.iter().all(|position| b.contains(position)));
    assert!(b.iter().all(|position| a.contains(position)));
    assert!(
        a.iter()
            .all(|position| position.cards(Zone::Board(A, Color::Blue)).len() == 4)
    );
    let mut reduced_events = Vec::new();
    let mut unrestricted_events = Vec::new();
    assert_eq!(
        finish(&mut reduced, &mut reduced_events),
        finish(&mut unrestricted, &mut unrestricted_events)
    );
    assert_eq!(reduced.position, unrestricted.position);
    assert_eq!(reduced_events, unrestricted_events);
    assert_eq!(reduced.rng, unrestricted.rng);
}

#[test]
fn covered_figure_disables_depth_origin_proof_and_clears_prior_prefix() {
    let mut f = Fixture::default();
    let retained = base(&mut f, "B113", Zone::Hand(A));
    // Maxwell guards returns. Covered in the very stack being returned, he
    // could be exposed mid-phase; elsewhere he could not.
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    base(&mut f, "B010", Zone::Board(A, Color::Blue));
    base(&mut f, "B032", Zone::Board(A, Color::Blue));
    let game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let from = Selector::Board(
        Who::Executor,
        ColorSet::Of(Reg::Bound),
        BoardRegion::BelowTop(0),
    );
    let mut batch = MoveAll::new(Verb::Return, from, Destination::Deck, Context::new(A));
    batch.regs.bound = Some(game.held(retained));
    batch.ordered_since = Some(0);
    let expected = game
        .select_bound(
            from,
            batch.context,
            batch.regs.bound.map(|c| c.inner),
            batch.regs.chosen,
        )
        .unwrap();
    let mut actual = expected.clone();
    {
        batch.shortcut = Some(Shortcut::ByOrigin {
            descending: false,
            quota: true,
        });
        game.reduce_batch_moves(&mut batch, &mut actual)
    };
    assert_eq!(actual, expected);
    assert_eq!(batch.ordered_since, None);
}
