//! Age-seven printed clauses, native reaction timing, and hidden-card evidence.
use super::*;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);
fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, vec![])
}
fn printed(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = echoes_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn figure(f: &mut Fixture, which: Figure, owner: Player) -> CardId {
    let card = figure_card(which.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn deck(f: &mut Fixture, set: CardSet, age: u8, color: Color) -> CardId {
    let id = tech(f, color, age, Zone::Deck(Deck { set, age }));
    f.cards[usize::from(id.0)].set = set;
    id
}
fn start(game: &mut Game, card: CardId, effect: u16, executor: Player, increase: u8) {
    let mut context = Context::new(executor);
    context.demander = executor.opponent();
    let mut boxed = BoxedValues::ZERO;
    boxed.increase(increase).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed,
        },
        context,
    )
    .unwrap();
}
fn finish(
    game: &mut Game,
    events: &mut Vec<Event>,
    mut pick: impl FnMut(&Decision) -> usize,
) -> Status {
    for _ in 0..100 {
        let mut cloned = game.clone();
        let mut next = Vec::new();
        let mut cloned_events = Vec::new();
        let status = game.advance(10000, &mut next).unwrap();
        assert_eq!(status, cloned.advance(10000, &mut cloned_events).unwrap());
        assert_eq!(*game, cloned);
        assert_eq!(next, cloned_events);
        events.extend(next);
        if let Status::Decision(menu) = status {
            game.choose(pick(&menu)).unwrap();
        } else {
            assert!(game.position.check_conservation());
            for viewer in Player::ALL {
                if let Some(tracker) = game.tracker(viewer) {
                    assert!(tracker.check_truth(&game.position));
                }
            }
            if let Some(tracker) = game.public_tracker() {
                assert!(tracker.check_truth(&game.position));
            }
            return status;
        }
    }
    panic!("age-seven scenario did not terminate");
}
fn decline(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|o| *o == Alternative::Decline)
        .unwrap_or(0)
}
fn named(menu: &Decision, card: CardId) -> usize {
    menu.options
        .iter()
        .position(|option| match option {
            Alternative::Move {
                target: TargetDescriptor::Known { card: id, .. },
                ..
            }
            | Alternative::Select {
                target: TargetDescriptor::Known { card: id, .. },
            } => *id == card,
            _ => false,
        })
        .unwrap_or_else(|| panic!("missing named {card:?}: {menu:?}"))
}
fn chosen_value(menu: &Decision, value: u8) -> usize {
    menu.options
        .iter()
        .position(|o| *o == Alternative::Value(value))
        .unwrap()
}
fn moved_cards(events: &[Event], verb: Verb) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb == Some(verb) && !m.inside_karma => Some(m.card),
            _ => None,
        })
        .collect()
}
fn echoes_on(f: &mut Fixture, color: Color, owner: Player, n: usize, slot: usize) -> Vec<CardId> {
    (0..n)
        .map(|_| {
            let id = tech(f, color, 1, Zone::Board(owner, color));
            f.cards[usize::from(id.0)].symbols[slot] = Symbol::Echo;
            f.cards[usize::from(id.0)].echo = Some(Box::new(Effect {
                kind: EffectKind::NonDemand,
                instructions: vec![],
            }));
            id
        })
        .collect()
}
fn promote_effect(f: &mut Fixture, code: &str, effect: usize, echo: bool) -> (CardId, CardId) {
    let mut source = echoes_card(code).unwrap();
    source.effects = vec![source.effects[effect].clone()];
    if !echo {
        source.echo = None;
    }
    let id = f.add(source, Zone::Forecast(A));
    let driver = tech(f, Color::Yellow, 7, Zone::Hand(A));
    (id, driver)
}

#[test]
fn visible_symbol_queries_use_each_physical_slot_and_reject_hidden_front_filters() {
    for (splay, slots) in [
        (Splay::None, &[][..]),
        (Splay::Left, &[3][..]),
        (Splay::Right, &[0, 1][..]),
        (Splay::Up, &[1, 2, 3][..]),
        (Splay::Aslant, &[0, 1, 2, 3][..]),
    ] {
        for slot in 0..4 {
            let mut f = Fixture::default();
            let covered = tech(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
            f.cards[usize::from(covered.0)].symbols[slot] = Symbol::Echo;
            tech(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue));
            let hidden = tech(&mut f, Color::Red, 1, Zone::Junk);
            f.cards[usize::from(hidden.0)].symbols[0] = Symbol::Echo;
            let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
            game.position.players[B.index()].splays[Color::Blue as usize] = splay;
            let query = Expr::Visible(&Visibility {
                who: Who::Opponent,
                colors: ColorSet::Only(&[Color::Blue]),
                seen: Seen::Echoes,
            });
            assert_eq!(
                game.eval(query, Context::new(A), Regs::EMPTY).unwrap(),
                Some(i64::from(slots.contains(&slot)))
            );
            let selected = game
                .select(
                    Selector::Filtered(
                        &Selector::Board(
                            Who::Either,
                            ColorSet::Only(&Color::ALL),
                            BoardRegion::BelowTop(0),
                        ),
                        &[Filter::Has(Test::Shows(Shown::Echo))],
                    ),
                    Context::new(A),
                    Regs::EMPTY,
                    Excluded::default(),
                )
                .unwrap();
            assert_eq!(
                selected.iter().map(|t| t.card).collect::<Vec<_>>(),
                if slots.contains(&slot) {
                    vec![covered]
                } else {
                    vec![]
                }
            );
            assert!(matches!(
                game.select(
                    Selector::Filtered(&Selector::Junk, &[Filter::Has(Test::Shows(Shown::Echo))]),
                    Context::new(A),
                    Regs::EMPTY,
                    Excluded::default()
                ),
                Err(Error::Unsupported("private filtered card selection"))
            ));
        }
    }
    for symbol in [Symbol::Image, Symbol::Icon(Icon::Authority)] {
        for splay in [Splay::None, Splay::Left, Splay::Right, Splay::Up] {
            let mut f = Fixture::default();
            let lower = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
            f.cards[usize::from(lower.0)].symbols[0] = symbol;
            tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
            let mut game = Game::new(f.position(), BOTH).unwrap();
            game.position.players[A.index()].splays[Color::Red as usize] = splay;
            let from = if symbol == Symbol::Image {
                Selector::Filtered(
                    &Selector::Board(
                        Who::Executor,
                        ColorSet::Only(&[Color::Red]),
                        BoardRegion::BelowTop(0),
                    ),
                    &[Filter::Has(Test::Shows(Shown::Image))],
                )
            } else {
                Selector::Filtered(
                    &Selector::Board(
                        Who::Executor,
                        ColorSet::Only(&[Color::Red]),
                        BoardRegion::BelowTop(0),
                    ),
                    &[Filter::Has(Test::Shows(Shown::Icon(Icon::Authority)))],
                )
            };
            assert_eq!(
                game.select(from, Context::new(A), Regs::EMPTY, Excluded::default())
                    .unwrap()
                    .len(),
                usize::from(splay == Splay::Right)
            );
        }
    }
    for shown in [Shown::Icon(Icon::Authority), Shown::Image, Shown::Echo] {
        assert!(Test::Shows(shown).reads_front());
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}

#[test]
fn rubber_echo_interleaves_two_boxed_tucks_and_sunshu_first_before_the_sixth_tuck() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E067", A);
        tech(&mut f, Color::Green, 7, Zone::Board(A, Color::Green));
        figure(&mut f, Figure::SunshuAo, A);
        let first = deck(&mut f, CardSet::Base, 8 + increase, Color::Blue);
        let second = deck(&mut f, CardSet::Echoes, 8 + increase, Color::Purple);
        deck(&mut f, CardSet::Base, 8 + increase, Color::Red);
        // Top draw is first; it creates the unique highest blue pile, redirecting
        // the next ordinary draw to Echoes only after the tuck finishes.
        let reward = deck(&mut f, CardSet::Base, 1, Color::Red);
        let mut position = f.position();
        let pile = position.zone_mut(Zone::Deck(Deck::base(8 + increase)));
        pile.retain(|id| *id != first);
        pile.push(first);
        position.turn_moves[A.index()][0] = 5;
        let mut game = Game::new(position, BOTH).unwrap().with_tracking().unwrap();
        start(&mut game, source, Card::ECHO, A, increase);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(moved_cards(&events, Verb::Tuck), [first, second]);
        assert_eq!(game.position.locate(reward), Some(Zone::Achievements(A)));
        let tuck = events.iter().position(|event| matches!(event,Event::Moved(m) if m.card==first && m.verb==Some(Verb::Tuck))).unwrap();
        let reward_at = events
            .iter()
            .position(|event| matches!(event,Event::Moved(m) if m.card==reward && m.inside_karma))
            .unwrap();
        let second_draw = events.iter().position(|event| matches!(event,Event::Moved(m) if m.card==second && matches!(m.source,Zone::Deck(_)))).unwrap();
        assert!(reward_at < tuck && tuck < second_draw);
    }
}

#[test]
fn rubber_scores_only_bonus_free_tops_and_red_splay_is_optional() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red));
        let source = printed(&mut f, "E067", A);
        let scored = tech(&mut f, Color::Blue, 4, Zone::Board(A, Color::Blue));
        let bonus = tech(&mut f, Color::Yellow, 3, Zone::Board(A, Color::Yellow));
        f.cards[usize::from(bonus.0)].symbols[0] = Symbol::Bonus(3);
        let napoleon = figure(&mut f, Figure::NapoleonBonaparte, A);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        // Isolate the printed-bonus filter from Napoleon's Instead scoring.
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context {
                scope: Context::SUPPRESS,
                ..Context::new(A)
            },
        )
        .unwrap();
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.cards(Zone::Score(A)), [scored]);
        for id in [source, bonus, napoleon] {
            assert!(matches!(game.position.locate(id), Some(Zone::Board(A, _))));
        }
        start(&mut game, source, 1, A, 0);
        finish(
            &mut game,
            &mut events,
            |m| if accept { 0 } else { decline(m) },
        );
        assert_eq!(
            game.position.splay(A, Color::Red),
            if accept { Splay::Up } else { Splay::None }
        );
    }
}

#[test]
fn rubber_native_foreseen_foreshadows_a_top_and_respects_a_full_forecast() {
    for foreseen in [false, true] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let target = tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
            let (source, driver) = if foreseen {
                promote_effect(&mut f, "E067", 2, false)
            } else {
                (printed(&mut f, "E067", A), CardId(0))
            };
            if full {
                for _ in 0..4 {
                    tech(&mut f, Color::Purple, 1, Zone::Forecast(A));
                }
            }
            for _ in 0..2 {
                tech(&mut f, Color::Green, 1, Zone::Board(A, Color::Green));
            }
            let mut game = Game::new(f.position(), Expansions::ECHOES)
                .unwrap()
                .with_tracking()
                .unwrap();
            if full {
                game.position.players[A.index()].splays[Color::Green as usize] = Splay::Left;
            }
            if foreseen {
                game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                    .unwrap();
            } else {
                start(&mut game, source, 2, A, 0);
            }
            let mut events = Vec::new();
            finish(&mut game, &mut events, |m| {
                if m.options.iter().any(|o|matches!(o,Alternative::Move{verb:Verb::Meld,target:TargetDescriptor::Known{card,source:Zone::Forecast(A)}} if *card==source)){named(m,source)}
            else if m.options.iter().any(|o|matches!(o,Alternative::Move{verb:Verb::Foreshadow,target:TargetDescriptor::Known{card,..}} if *card==target)){named(m,target)}else{decline(m)}
            });
            assert_eq!(
                game.position.locate(target),
                Some(if foreseen && !full {
                    Zone::Forecast(A)
                } else {
                    Zone::Board(A, Color::Blue)
                })
            );
            assert_eq!(
                moved_cards(&events, Verb::Foreshadow),
                if foreseen && !full {
                    vec![target]
                } else {
                    vec![]
                }
            );
        }
    }
}

#[test]
fn elevator_echo_keeps_top_bottom_choices_and_deduplicates_a_singleton() {
    for n in [0, 1, 3] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E068", A);
        let pile: Vec<_> = (0..n)
            .map(|i| tech(&mut f, Color::Green, i + 1, Zone::Board(A, Color::Green)))
            .collect();
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        start(&mut game, source, Card::ECHO, A, 0);
        let mut events = Vec::new();
        if n == 3 {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options.len(), 2);
            game.choose(named(&menu, pile[0])).unwrap();
        }
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.cards(Zone::Score(A)),
            if n == 0 { vec![] } else { vec![pile[0]] }
        );
        if n == 3 {
            assert_eq!(
                game.position.cards(Zone::Board(A, Color::Green)),
                &pile[1..]
            );
        }
    }
}

#[test]
fn elevator_uses_saved_value_for_either_opposing_source_and_the_final_forecast_draw() {
    for hands in [false, true] {
        for full in [false, true] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "E068", A);
            let original = tech(&mut f, Color::Red, 3, Zone::Score(A));
            tech(&mut f, Color::Purple, 5, Zone::Score(A));
            let hand = [
                tech(&mut f, Color::Green, 3, Zone::Hand(B)),
                tech(&mut f, Color::Blue, 3, Zone::Hand(B)),
            ];
            let score = [
                tech(&mut f, Color::Red, 3, Zone::Score(B)),
                tech(&mut f, Color::Purple, 3, Zone::Score(B)),
            ];
            let other = tech(&mut f, Color::Yellow, 2, Zone::Hand(B));
            let draw = deck(&mut f, CardSet::Base, 3, Color::Blue);
            deck(&mut f, CardSet::Base, 1, Color::Red);
            if full {
                for _ in 0..5 {
                    tech(&mut f, Color::Red, 1, Zone::Forecast(A));
                }
            }
            let mut game = Game::seeded(f.position(), Expansions::ECHOES, 19)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, source, 0, A, 0);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options, [Alternative::Value(3), Alternative::Value(5)]);
            game.choose(chosen_value(&menu, 3)).unwrap();
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.options.len(), 2);
            game.choose(usize::from(!hands)).unwrap();
            finish(&mut game, &mut events, |_| 0);
            for id in if hands { hand } else { score } {
                assert_eq!(game.position.locate(id), Some(Zone::Score(A)));
            }
            for id in if hands { score } else { hand } {
                assert_eq!(
                    game.position.locate(id),
                    Some(if hands { Zone::Score(B) } else { Zone::Hand(B) })
                );
            }
            assert_eq!(game.position.locate(original), Some(Zone::Score(A)));
            assert_eq!(game.position.locate(other), Some(Zone::Hand(B)));
            assert_eq!(
                game.position.locate(draw),
                Some(if full {
                    Zone::Hand(A)
                } else {
                    Zone::Forecast(A)
                })
            );
            assert_eq!(moved_cards(&events, Verb::Score).len(), 2);
            assert!(moved_cards(&events, Verb::Transfer).is_empty());
        }
    }
}

#[test]
fn fertilizer_return_gates_a_live_both_score_transfer_and_counts_the_later_hand() {
    for accept in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E069", A);
        let returned = tech(&mut f, Color::Red, 2, Zone::Hand(A));
        let scores = [
            tech(&mut f, Color::Red, 2, Zone::Score(A)),
            tech(&mut f, Color::Blue, 2, Zone::Score(B)),
        ];
        let unrelated = tech(&mut f, Color::Green, 3, Zone::Score(B));
        let draw = deck(
            &mut f,
            CardSet::Base,
            if accept { 2 } else { 1 },
            Color::Purple,
        );
        let mut game = Game::new(f.position(), Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, A, 0);
        let mut events = Vec::new();
        finish(
            &mut game,
            &mut events,
            |m| if accept { 0 } else { decline(m) },
        );
        for (index, id) in scores.into_iter().enumerate() {
            assert_eq!(
                game.position.locate(id),
                Some(if accept {
                    Zone::Hand(A)
                } else {
                    Zone::Score(if index == 0 { A } else { B })
                })
            );
        }
        assert_eq!(
            game.position.locate(returned),
            Some(if accept {
                Zone::Deck(Deck::base(2))
            } else {
                Zone::Hand(A)
            })
        );
        assert_eq!(game.position.locate(unrelated), Some(Zone::Score(B)));
        start(&mut game, source, 1, A, 0);
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(game.position.locate(draw), Some(Zone::Forecast(A)));
    }
}

#[test]
fn fertilizer_replaced_return_does_not_transfer_and_empty_hand_draw_resolves_zero() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "E069", A);
    figure(&mut f, Figure::NapoleonBonaparte, A);
    let returned = tech(&mut f, Color::Red, 2, Zone::Hand(A));
    let scored = tech(&mut f, Color::Blue, 2, Zone::Score(B));
    let mut game = Game::new(f.position(), BOTH).unwrap();
    start(&mut game, source, 0, A, 0);
    let mut events = Vec::new();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(
        game.position.locate(returned),
        Some(Zone::Board(A, Color::Red))
    );
    assert_eq!(game.position.locate(scored), Some(Zone::Score(B)));
    assert!(moved_cards(&events, Verb::Transfer).is_empty());
    for zero in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E069", A);
        let draw = deck(&mut f, CardSet::Base, if zero { 0 } else { 1 }, Color::Blue);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        start(&mut game, source, 1, A, 0);
        finish(&mut game, &mut Vec::new(), |_| 0);
        assert_eq!(game.position.locate(draw), Some(Zone::Forecast(A)));
    }
}

#[test]
fn jeans_echo_keeps_only_its_two_draws_and_excludes_curies_first_draw() {
    for full in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E070", A);
        figure(&mut f, Figure::MarieCurie, A);
        let unrelated = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
        let other = deck(&mut f, CardSet::Base, 8, Color::Red);
        let chosen = deck(&mut f, CardSet::Base, 8, Color::Purple);
        let extra = deck(&mut f, CardSet::Base, 9, Color::Green);
        if full {
            for _ in 0..5 {
                tech(&mut f, Color::Yellow, 1, Zone::Forecast(A));
            }
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, Card::ECHO, A, 0);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 2);
        assert!(menu.options.iter().all(|option|matches!(option,Alternative::Select{target:TargetDescriptor::Known{card,..}} if [chosen,other].contains(card))));
        assert!(
            events
                .iter()
                .any(|event| matches!(event,Event::Moved(m) if m.card==extra&&m.inside_karma))
        );
        game.choose(named(&menu, chosen)).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(chosen),
            Some(Zone::Deck(Deck::base(8)))
        );
        assert_eq!(
            game.position.locate(other),
            Some(if full {
                Zone::Hand(A)
            } else {
                Zone::Forecast(A)
            })
        );
        for id in [unrelated, extra] {
            assert_eq!(game.position.locate(id), Some(Zone::Hand(A)));
        }
        assert_eq!(moved_cards(&events, Verb::Return), [chosen]);
    }
}

#[test]
fn jeans_draw_reveals_a_named_packet_and_preserves_its_chosen_member_in_samples() {
    for tracked in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E070", A);
        figure(&mut f, Figure::MarieCurie, A);
        let unrelated = tech(&mut f, Color::Yellow, 1, Zone::Hand(A));
        let other = deck(&mut f, CardSet::Base, 8, Color::Red);
        let chosen = deck(&mut f, CardSet::Base, 8, Color::Purple);
        let extra = deck(&mut f, CardSet::Base, 9, Color::Green);
        let mut game = Game::new(f.position(), BOTH).unwrap();
        if tracked {
            game = game.with_tracking().unwrap();
        }
        start(&mut game, source, 0, A, 1);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 2);
        for id in [chosen, other] {
            named(&menu, id);
        }
        assert_eq!(
            events
                .iter()
                .filter_map(|event| if let Event::Revealed { card, .. } = event {
                    Some(*card)
                } else {
                    None
                })
                .collect::<Vec<_>>(),
            [chosen, other]
        );
        if tracked {
            for seed in 0..16 {
                for mut world in [
                    game.determinize(A, seed).unwrap(),
                    game.sample_world(A, seed).unwrap().world,
                ] {
                    let sampled = decision(&mut world, &mut Vec::new());
                    assert_eq!(sampled.options, menu.options);
                    world.choose(named(&sampled, chosen)).unwrap();
                    finish(&mut world, &mut Vec::new(), |_| 0);
                    assert_eq!(
                        world.position.locate(chosen),
                        Some(Zone::Board(A, Color::Purple))
                    );
                    assert_eq!(
                        world.position.locate(other),
                        Some(Zone::Deck(Deck::base(8)))
                    );
                }
            }
        }
        game.choose(named(&menu, chosen)).unwrap();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(chosen),
            Some(Zone::Board(A, Color::Purple))
        );
        assert_eq!(game.position.locate(other), Some(Zone::Deck(Deck::base(8))));
        for id in [unrelated, extra] {
            assert_eq!(game.position.locate(id), Some(Zone::Hand(A)));
        }
        assert_eq!(moved_cards(&events, Verb::Meld), [chosen]);
        assert_eq!(moved_cards(&events, Verb::Return), [other]);
    }
}

#[test]
fn jeans_without_blue_requests_zero_and_a_present_blue_value_is_frozen_before_draws() {
    for age in [0, 3] {
        for zero in [false, true] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "E070", A);
            if age > 0 {
                tech(&mut f, Color::Blue, age, Zone::Board(A, Color::Blue));
            }
            let drawn_age = if age > 0 { age } else { u8::from(!zero) };
            let one = deck(&mut f, CardSet::Base, drawn_age, Color::Red);
            let two = deck(&mut f, CardSet::Base, drawn_age, Color::Yellow);
            let mut game = Game::new(f.position(), BOTH).unwrap();
            start(&mut game, source, 0, A, 1);
            let mut events = Vec::new();
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(
                events
                    .iter()
                    .filter(
                        |event| matches!(event,Event::Revealed{card,..} if [one,two].contains(card))
                    )
                    .count(),
                2
            );
            assert_eq!(moved_cards(&events, Verb::Meld).len(), 1);
            assert_eq!(moved_cards(&events, Verb::Return).len(), 1);
            assert_eq!(game.position.cards(Zone::Hand(A)).len(), 0);
        }
    }
}

#[test]
fn jeans_junks_only_the_chosen_boxed_base_deck() {
    for age in [7, 8] {
        for increase in [0, 1] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "E070", A);
            let selected = deck(&mut f, CardSet::Base, age + increase, Color::Red);
            let other = deck(
                &mut f,
                CardSet::Base,
                if age == 7 { 8 } else { 7 } + increase,
                Color::Yellow,
            );
            let echo = deck(&mut f, CardSet::Echoes, age + increase, Color::Blue);
            let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
            start(&mut game, source, 1, A, increase);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            assert_eq!(
                menu.options,
                [
                    Alternative::Value(7 + increase),
                    Alternative::Value(8 + increase)
                ]
            );
            game.choose(chosen_value(&menu, age + increase)).unwrap();
            finish(&mut game, &mut events, |_| 0);
            assert_eq!(game.position.locate(selected), Some(Zone::Junk));
            assert!(matches!(game.position.locate(other), Some(Zone::Deck(_))));
            assert!(matches!(game.position.locate(echo), Some(Zone::Deck(_))));
        }
    }
}

#[test]
fn jeans_skips_its_deck_choice_when_both_boxed_base_decks_are_empty() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E070", A);
        // A Base deck the boxes don't name, and an Echoes deck they do.
        deck(
            &mut f,
            CardSet::Base,
            if increase == 0 { 9 } else { 7 },
            Color::Red,
        );
        deck(&mut f, CardSet::Echoes, 7 + increase, Color::Blue);
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        let before = game.position.clone();
        start(&mut game, source, 1, A, increase);
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert!(events.is_empty());
        assert_eq!(game.position, before);
    }
}

#[test]
fn jeans_foreseen_blind_junk_pick_uses_public_backs_and_stays_hidden_through_nobel() {
    for foreseen in [false, true] {
        for tracked in [false, true] {
            let mut f = Fixture::default();
            let (source, driver) = if foreseen {
                promote_effect(&mut f, "E070", 2, false)
            } else {
                (printed(&mut f, "E070", A), CardId(0))
            };
            figure(&mut f, Figure::AlfredNobel, B);
            let pair = [
                tech(&mut f, Color::Red, 6, Zone::Junk),
                tech(&mut f, Color::Blue, 6, Zone::Junk),
            ];
            let other = tech(&mut f, Color::Purple, 8, Zone::Junk);
            let seven = deck(&mut f, CardSet::Base, 7, Color::Green);
            let mut game = Game::seeded(f.position(), BOTH, 93).unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            if foreseen {
                game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                    .unwrap();
            } else {
                start(&mut game, source, 2, A, 0);
            }
            let mut events = Vec::new();
            if foreseen {
                loop {
                    let menu = decision(&mut game, &mut events);
                    if menu
                        .options
                        .iter()
                        .any(|option| matches!(option, Alternative::BlindSelect { .. }))
                    {
                        assert_eq!(
                            menu.options,
                            [6, 8].map(|age| Alternative::BlindSelect {
                                source: Zone::Junk,
                                back: Deck::base(age)
                            })
                        );
                        let mut expected = game.rng.clone();
                        let selected = pair[(expected.next_u64() % 2) as usize];
                        game.choose(0).unwrap();
                        // Advance through the real First one operation at a time; the
                        // transfer must remain anonymous until its hand admission.
                        loop {
                            let before = game.position.locate(selected);
                            let status = game.advance(1, &mut events).unwrap();
                            if before == Some(Zone::Junk)
                                && game.position.locate(selected) == Some(Zone::Junk)
                                && tracked
                            {
                                assert!(!game.tracker(A).unwrap().knows(
                                    &game.position,
                                    selected,
                                    Zone::Junk
                                ));
                            }
                            if matches!(status, Status::Complete(_)) {
                                break;
                            }
                            if let Status::Decision(menu) = status {
                                game.choose(decline(&menu)).unwrap();
                            }
                        }
                        assert_eq!(game.rng, expected);
                        assert_eq!(game.position.locate(selected), Some(Zone::Hand(A)));
                        assert_eq!(
                            game.position.locate(pair[usize::from(selected == pair[0])]),
                            Some(Zone::Junk)
                        );
                        break;
                    } else {
                        game.choose(named(&menu, source)).unwrap();
                    }
                }
                assert_eq!(game.position.locate(seven), Some(Zone::Junk));
                assert!(
                    events.iter().any(
                        |event| matches!(event,Event::Moved(m) if m.card==seven&&m.inside_karma)
                    )
                );
            } else {
                finish(&mut game, &mut events, |_| 0);
                assert!(moved_cards(&events, Verb::Transfer).is_empty());
            }
            assert_eq!(game.position.locate(other), Some(Zone::Junk));
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Event::Revealed { .. }))
            );
            assert!(game.position.check_conservation());
        }
    }
}

#[test]
fn telegraph_copies_matching_opponent_splay_including_unsplayed_and_allows_decline() {
    for direction in [
        Splay::None,
        Splay::Left,
        Splay::Right,
        Splay::Up,
        Splay::Aslant,
    ] {
        for accept in [false, true] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "E071", A);
            for owner in Player::ALL {
                for age in [1, 2] {
                    tech(&mut f, Color::Blue, age, Zone::Board(owner, Color::Blue));
                }
            }
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            game.position.players[A.index()].splays[Color::Blue as usize] = Splay::Right;
            game.position.players[B.index()].splays[Color::Blue as usize] = direction;
            start(&mut game, source, 0, A, 0);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            // Copying the direction blue already has changes nothing, so
            // that option is merged with the other no-op colours: any
            // splay option then stands for it.
            let selected = menu
                .options
                .iter()
                .position(|o| matches!(o, Alternative::Splay { player: A, color: Color::Blue, direction: d } if *d == direction))
                .or_else(|| {
                    assert_eq!(direction, Splay::Right);
                    menu.options
                        .iter()
                        .position(|o| matches!(o, Alternative::Splay { .. }))
                })
                .unwrap();
            game.choose(if accept { selected } else { decline(&menu) })
                .unwrap();
            finish(&mut game, &mut events, decline);
            assert_eq!(
                game.position.splay(A, Color::Blue),
                if accept { direction } else { Splay::Right }
            );
            assert_eq!(game.position.splay(B, Color::Blue), direction);
        }
    }
}

#[test]
fn telegraph_freezes_copied_direction_before_machiavelli_and_blue_up_is_optional() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "E071", A);
    for owner in Player::ALL {
        for age in [1, 2] {
            tech(&mut f, Color::Blue, age, Zone::Board(owner, Color::Blue));
        }
    }
    figure(&mut f, Figure::NiccoloMachiavelli, A);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.position.players[B.index()].splays[Color::Blue as usize] = Splay::Right;
    start(&mut game, source, 0, A, 0);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    let selected = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Splay {
                    player: A,
                    color: Color::Blue,
                    direction: Splay::Right
                }
            )
        })
        .unwrap();
    game.choose(selected).unwrap();
    finish(&mut game, &mut events, decline);
    assert_eq!(game.position.splay(B, Color::Blue), Splay::None);
    assert_eq!(game.position.splay(A, Color::Blue), Splay::Right);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Splayed {
            player: B,
            to: Splay::None,
            inside_karma: true,
            ..
        }
    )));
    for accept in [false, true] {
        let mut fork = game.clone();
        start(&mut fork, source, 1, A, 0);
        let mut events = Vec::new();
        finish(&mut fork, &mut events, |menu| {
            if accept { 0 } else { decline(menu) }
        });
        assert_eq!(
            fork.position.splay(A, Color::Blue),
            if accept { Splay::Up } else { Splay::Right }
        );
    }
}

#[test]
fn telegraph_foreseen_splays_only_its_initially_splayed_colors_up() {
    for foreseen in [false, true] {
        let mut f = Fixture::default();
        let (source, driver) = if foreseen {
            promote_effect(&mut f, "E071", 2, false)
        } else {
            (printed(&mut f, "E071", A), CardId(0))
        };
        for color in [Color::Red, Color::Blue, Color::Green] {
            for age in [1, 2] {
                tech(&mut f, color, age, Zone::Board(A, color));
            }
        }
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.position.players[A.index()].splays[Color::Red as usize] = Splay::Left;
        game.position.players[A.index()].splays[Color::Blue as usize] = Splay::Right;
        if foreseen {
            game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                .unwrap();
        } else {
            start(&mut game, source, 2, A, 0);
        }
        let mut events = Vec::new();
        finish(&mut game, &mut events, |menu| {
            menu.options.iter().position(|o|matches!(o,Alternative::Select{target:TargetDescriptor::Known{card,..}} | Alternative::Move{target:TargetDescriptor::Known{card,..},..} if *card==source)).unwrap_or(0)
        });
        assert_eq!(
            game.position.splay(A, Color::Red),
            if foreseen { Splay::Up } else { Splay::Left }
        );
        assert_eq!(
            game.position.splay(A, Color::Blue),
            if foreseen { Splay::Up } else { Splay::Right }
        );
        assert_eq!(game.position.splay(A, Color::Green), Splay::None);
    }
}

#[test]
fn photography_echo_melds_forecast_without_promotion_and_demand_keeps_highest_ties() {
    let mut f = Fixture::default();
    let source = printed(&mut f, "E072", A);
    let forecast = f.add(echoes_card("E001").unwrap(), Zone::Forecast(A));
    let untouched = deck(&mut f, CardSet::Base, 1, Color::Yellow);
    let red = tech(&mut f, Color::Green, 9, Zone::Board(A, Color::Green));
    let purple = tech(&mut f, Color::Purple, 9, Zone::Board(A, Color::Purple));
    let low = tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut events = Vec::new();
    start(&mut game, source, Card::ECHO, A, 0);
    finish(&mut game, &mut events, |_| 0);
    assert!(matches!(
        game.position.locate(forecast),
        Some(Zone::Board(A, _))
    ));
    assert_eq!(
        game.position.locate(untouched),
        Some(Zone::Deck(Deck::base(1)))
    );
    assert!(game.position.cards(Zone::Forecast(A)).is_empty());
    start(&mut game, source, 0, A, 0);
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(menu.options.iter().any(
        |o| matches!(o,Alternative::Move{target:TargetDescriptor::Known{card,..},..} if *card==red)
    ));
    game.choose(named(&menu, purple)).unwrap();
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position.locate(purple), Some(Zone::Hand(A)));
    assert_eq!(
        game.position.locate(red),
        Some(Zone::Board(A, Color::Green))
    );
    assert_eq!(game.position.locate(low), Some(Zone::Board(A, Color::Red)));
}

#[test]
fn photography_counts_visible_echoes_per_color_and_requires_a_successful_foreseen_claim() {
    for foreseen in [false, true] {
        for n in [2, 3] {
            for available in [false, true] {
                let mut f = Fixture::default();
                let (source, driver) = if foreseen {
                    promote_effect(&mut f, "E072", 1, false)
                } else {
                    (printed(&mut f, "E072", A), CardId(0))
                };
                echoes_on(&mut f, Color::Red, A, n, 3); // left exposes every echo, including the top.
                echoes_on(&mut f, Color::Blue, A, 1, 3); // totals across colors cannot meet the condition.
                let mut game = Game::new(f.position(), BOTH)
                    .unwrap()
                    .with_tracking()
                    .unwrap();
                game.position.players[A.index()].splays[Color::Red as usize] = Splay::Left;
                game.position.specials[Special::History.index()] = if available {
                    SpecialLocation::Available
                } else {
                    SpecialLocation::Owned(B)
                };
                if foreseen {
                    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
                        .unwrap();
                } else {
                    start(&mut game, source, 1, A, 0);
                }
                let mut events = Vec::new();
                let status = finish(&mut game, &mut events, |menu| {
                    menu.options.iter().position(|o|matches!(o,Alternative::Select{target:TargetDescriptor::Known{card,..}} | Alternative::Move{target:TargetDescriptor::Known{card,..},..} if *card==source)).unwrap_or_else(||decline(menu))
                });
                let claimed = n == 3 && available;
                assert_eq!(
                    game.position.specials[Special::History.index()],
                    if claimed {
                        SpecialLocation::Owned(A)
                    } else if available {
                        SpecialLocation::Available
                    } else {
                        SpecialLocation::Owned(B)
                    }
                );
                assert_eq!(
                    matches!(status, Status::Winner(A)),
                    claimed && foreseen,
                    "foreseen={foreseen}, n={n}, available={available}: {status:?}"
                );
            }
        }
    }
    // A face-down echo on a covered card cannot contribute, even when three
    // cards of that color each have an echo printed on their fronts.
    let mut f = Fixture::default();
    let source = printed(&mut f, "E072", A);
    echoes_on(&mut f, Color::Red, A, 3, 0);
    let mut game = Game::new(f.position(), BOTH).unwrap();
    game.position.players[A.index()].splays[Color::Red as usize] = Splay::Left;
    start(&mut game, source, 1, A, 0);
    finish(&mut game, &mut Vec::new(), |_| 0);
    assert_eq!(
        game.position.specials[Special::History.index()],
        SpecialLocation::Available
    );
    // Promotion raises the blue pile from two visible echoes to three.
    // Hedy's First then hides the lower echoes; the already-earned printed
    // claim still succeeds, and that successful foreseen claim wins.
    let mut f = Fixture::default();
    echoes_on(&mut f, Color::Blue, A, 2, 0);
    let (source, driver) = promote_effect(&mut f, "E072", 1, false);
    figure(&mut f, Figure::HedyLamarr, A);
    let mut game = Game::new(f.position(), BOTH)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.position.players[A.index()].splays[Color::Blue as usize] = Splay::Right;
    game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let status = finish(&mut game, &mut events, |menu| {
        menu.options
            .iter()
            .position(|option| {
                matches!(
                    option,
                    Alternative::Splay {
                        player: A,
                        color: Color::Blue,
                        direction: Splay::Up
                    }
                )
            })
            .unwrap_or(0)
    });
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.position.top(A, Color::Blue), Some(source));
    assert_eq!(game.position.splay(A, Color::Blue), Splay::Up);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Splayed {
            player: A,
            color: Color::Blue,
            inside_karma: true,
            ..
        }
    )));
}

#[test]
fn typewriter_reveals_returned_faces_counts_distinct_colors_and_uses_live_hand_maxima() {
    for curie in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E073", A);
        if curie {
            figure(&mut f, Figure::MarieCurie, A);
        }
        let returned = [
            tech(&mut f, Color::Red, 1, Zone::Hand(A)),
            tech(&mut f, Color::Red, 2, Zone::Hand(A)),
            tech(&mut f, Color::Blue, 3, Zone::Hand(A)),
        ];
        let six = deck(&mut f, CardSet::Base, 6, Color::Green);
        let seven = deck(&mut f, CardSet::Base, 7, Color::Green);
        let eight = deck(&mut f, CardSet::Base, 8, Color::Green);
        let nine = deck(&mut f, CardSet::Base, 9, Color::Green);
        let ten = deck(&mut f, CardSet::Base, 10, Color::Green);
        let eleven = deck(&mut f, CardSet::Base, 11, Color::Green);
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, A, 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(moved_cards(&events, Verb::Return).len(), 3);
        let revealed = events
            .iter()
            .filter_map(|e| {
                if let Event::Revealed { card, .. } = e {
                    Some(*card)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(returned.iter().all(|card| revealed.contains(card)));
        let hand = game.position.cards(Zone::Hand(A));
        if curie {
            assert_eq!(hand.len(), 4);
            assert!(
                hand.contains(&six)
                    && hand.contains(&nine)
                    && hand.contains(&ten)
                    && hand.contains(&eleven)
            );
            assert!(!hand.contains(&seven) && !hand.contains(&eight));
        } else {
            assert_eq!(hand.len(), 3);
            assert!(hand.contains(&six) && hand.contains(&seven) && hand.contains(&eight));
        }
    }
}

#[test]
fn typewriter_empty_hand_and_replaced_returns_do_not_invent_returned_colors() {
    for napoleon in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E073", A);
        if napoleon {
            figure(&mut f, Figure::NapoleonBonaparte, A);
            tech(&mut f, Color::Red, 2, Zone::Hand(A));
        }
        let six = deck(&mut f, CardSet::Base, 6, Color::Yellow);
        let seven = deck(&mut f, CardSet::Base, 7, Color::Blue);
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        start(&mut game, source, 0, A, 0);
        let mut events = Vec::new();
        finish(&mut game, &mut events, decline);
        assert!(moved_cards(&events, Verb::Return).is_empty());
        assert_eq!(game.position.cards(Zone::Hand(A)), [six]);
        assert_eq!(game.position.locate(seven), Some(Zone::Deck(Deck::base(7))));
    }
}

#[test]
fn ice_cream_echo_excludes_purple_and_bonus_tops_and_demand_uses_boxed_age() {
    for increase in [0, 1] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E074", A);
        let purple = tech(&mut f, Color::Purple, 3, Zone::Board(A, Color::Purple));
        let bonus = tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
        f.cards[usize::from(bonus.0)].symbols[0] = Symbol::Bonus(3);
        let selected = tech(&mut f, Color::Red, 2, Zone::Board(A, Color::Red));
        let drawn = deck(&mut f, CardSet::Base, 1 + increase, Color::Yellow);
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut events = Vec::new();
        start(&mut game, source, Card::ECHO, A, increase);
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(moved_cards(&events, Verb::Score), [selected]);
        assert_eq!(
            game.position.locate(purple),
            Some(Zone::Board(A, Color::Purple))
        );
        assert_eq!(
            game.position.locate(bonus),
            Some(Zone::Board(A, Color::Blue))
        );
        start(&mut game, source, 0, A, increase);
        finish(&mut game, &mut events, |_| 0);
        assert_eq!(
            game.position.locate(drawn),
            Some(Zone::Board(A, Color::Yellow))
        );
    }
}

#[test]
fn ice_cream_chosen_optional_sweep_claims_only_the_highest_eligible_junk() {
    for accept in [false, true] {
        for eligible in [false, true] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "E074", A);
            tech(&mut f, Color::Red, 10, Zone::Board(A, Color::Red));
            if eligible {
                for _ in 0..4 {
                    tech(&mut f, Color::Yellow, 10, Zone::Score(A));
                }
            }
            let six = deck(&mut f, CardSet::Base, 6, Color::Green);
            let highest = tech(&mut f, Color::Blue, 8, Zone::Junk);
            let echo = deck(&mut f, CardSet::Echoes, 6, Color::Purple);
            let mut game = Game::new(f.position(), BOTH)
                .unwrap()
                .with_tracking()
                .unwrap();
            start(&mut game, source, 1, A, 0);
            let mut events = Vec::new();
            let menu = decision(&mut game, &mut events);
            // The empty [7], [8] and [9] decks are one option.
            assert_eq!(menu.options, [6, 7].map(Alternative::Value));
            game.choose(0).unwrap();
            finish(&mut game, &mut events, |menu| {
                if accept { 0 } else { decline(menu) }
            });
            assert_eq!(
                game.position.locate(six),
                Some(if accept {
                    Zone::Junk
                } else {
                    Zone::Deck(Deck::base(6))
                })
            );
            assert_eq!(
                game.position.locate(highest),
                Some(if accept && eligible {
                    Zone::Achievements(A)
                } else {
                    Zone::Junk
                })
            );
            assert_eq!(
                game.position.locate(echo),
                Some(Zone::Deck(Deck {
                    set: CardSet::Echoes,
                    age: 6
                }))
            );
        }
    }
}

fn instrument(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    if let Ok(card) = echoes_card(code) {
        let color = card.color;
        return f.add(card, Zone::Board(owner, color));
    }
    let meta = crate::catalog::echoes_metadata(code).unwrap();
    let id = tech(f, meta.color, meta.value, Zone::Board(owner, meta.color));
    let card = &mut f.cards[usize::from(id.0)];
    card.code = meta.code;
    card.set = CardSet::Echoes;
    card.symbols[..4].copy_from_slice(&meta.symbols);
    id
}

#[test]
fn saxophone_counts_only_visible_instrument_images_on_both_boards_and_wins_at_four() {
    for n in 0usize..=4 {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E075", A);
        // Put Saxophone in junk when it should not count its own image.
        if n == 0 {
            f.placements
                .iter_mut()
                .find(|(id, _)| *id == source)
                .unwrap()
                .1 = Zone::Junk;
        }
        for (i, code) in ["E013", "E014", "E054"]
            .into_iter()
            .take(n.saturating_sub(1))
            .enumerate()
        {
            instrument(&mut f, code, if i % 2 == 0 { B } else { A });
        }
        let extra = tech(&mut f, Color::Green, 2, Zone::Board(B, Color::Green));
        f.cards[usize::from(extra.0)].symbols[0] = Symbol::Image;
        let bonus_draw = if n == 2 {
            figure(&mut f, Figure::MarieCurie, A);
            Some(deck(&mut f, CardSet::Base, 9, Color::Yellow))
        } else {
            None
        };
        let drawn = (0..4)
            .map(|_| deck(&mut f, CardSet::Base, 7, Color::Red))
            .collect::<Vec<_>>();
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        for owner in Player::ALL {
            game.position.players[owner.index()].splays[Color::Purple as usize] = Splay::Aslant;
        }
        start(&mut game, source, 1, A, 0);
        let mut events = Vec::new();
        let status = finish(&mut game, &mut events, |_| 0);
        assert_eq!(matches!(status, Status::Winner(A)), n == 4);
        assert_eq!(
            game.position.cards(Zone::Hand(A)).len(),
            if n == 4 {
                0
            } else {
                n + usize::from(bonus_draw.is_some())
            }
        );
        if let Some(extra) = bonus_draw {
            assert_eq!(game.position.locate(extra), Some(Zone::Hand(A)));
            assert!(events.iter().any(
                |event| matches!(event, Event::Moved(m) if m.card == extra && m.inside_karma)
            ));
        }
        assert_eq!(
            drawn
                .iter()
                .filter(|id| game.position.locate(**id) == Some(Zone::Hand(A)))
                .count(),
            if n == 4 { 0 } else { n }
        );
    }
}

#[test]
fn saxophone_checks_the_image_hex_itself_and_purple_up_is_optional() {
    for splay in [Splay::Left, Splay::Right, Splay::Up] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E075", A);
        let bell = instrument(&mut f, "E013", B);
        let color = f.cards[usize::from(bell.0)].color;
        tech(&mut f, color, 4, Zone::Board(B, color));
        for _ in 0..3 {
            deck(&mut f, CardSet::Base, 7, Color::Blue);
        }
        for age in [1, 2] {
            tech(&mut f, Color::Purple, age, Zone::Board(A, Color::Purple));
        }
        let source_place = f.placements.remove(
            f.placements
                .iter()
                .position(|(id, _)| *id == source)
                .unwrap(),
        );
        f.placements.push(source_place);
        let mut game = Game::new(f.position(), BOTH)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.position.players[B.index()].splays[color as usize] = splay;
        let bell_shows =
            game.position
                .card(bell)
                .symbols
                .iter()
                .enumerate()
                .any(|(slot, symbol)| {
                    *symbol == Symbol::Image
                        && match splay {
                            Splay::Left => slot == 3,
                            Splay::Right => slot < 2,
                            Splay::Up => (1..=3).contains(&slot),
                            _ => false,
                        }
                });
        start(&mut game, source, 1, A, 0);
        finish(&mut game, &mut Vec::new(), |_| 0);
        assert_eq!(
            game.position.cards(Zone::Hand(A)).len(),
            1 + usize::from(bell_shows)
        );
        for accept in [false, true] {
            let mut fork = game.clone();
            start(&mut fork, source, 0, A, 0);
            finish(&mut fork, &mut Vec::new(), |menu| {
                if accept { 0 } else { decline(menu) }
            });
            assert_eq!(
                fork.position.splay(A, Color::Purple),
                if accept { Splay::Up } else { Splay::None }
            );
        }
    }
}
