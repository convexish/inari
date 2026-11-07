//! Every-path proofs for the age-three card batches not covered by mechanics.
use super::*;
use age03_mechanics_tests::oracle;
use shortcuts::common::*;

const BOTH: Expansions = Expansions::ECHOES.union(Expansions::FIGURES);

fn printed(f: &mut Fixture, code: &str, owner: Player) -> CardId {
    let card = echoes_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}
fn tech(f: &mut Fixture, color: Color, age: u8, zone: Zone) -> CardId {
    f.tech(color, age, zone, Vec::new())
}
fn icon(f: &mut Fixture, card: CardId, icon: Icon) {
    f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(icon);
}
fn figure(f: &mut Fixture, figure: Figure, owner: Player) {
    let card = figure_card(figure.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color));
}
fn effect(game: &mut Game, source: CardId, index: u16, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: index,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
}
fn demand() -> Context {
    let mut context = Context::new(B);
    context.demander = A;
    context.source_owner = A;
    context
}

#[test]
fn katana_quota_oracle_requires_closed_tops_and_keeps_nobels_actual_reaction() {
    for buried in [false, true] {
        for reacting in [false, true] {
            for another_top in [false, true] {
                let mut f = Fixture::default();
                let source = printed(&mut f, "E027", A);
                let lower = buried.then(|| {
                    let card = tech(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
                    icon(&mut f, card, Icon::Authority);
                    card
                });
                let red = tech(&mut f, Color::Red, 2, Zone::Board(B, Color::Red));
                icon(&mut f, red, Icon::Authority);
                if another_top {
                    let blue = tech(&mut f, Color::Blue, 3, Zone::Board(B, Color::Blue));
                    icon(&mut f, blue, Icon::Authority);
                }
                let seven = tech(&mut f, Color::Yellow, 7, Zone::Deck(Deck::base(7)));
                if reacting {
                    figure(&mut f, Figure::AlfredNobel, A);
                }
                let mut game = Game::seeded(f.position(), BOTH, 71)
                    .unwrap()
                    .with_tracking()
                    .unwrap();
                effect(&mut game, source, 0, demand());
                let (reduced, manual) = oracle(game);
                if !another_top {
                    assert_eq!(reduced.len(), 1);
                    assert_eq!(manual.len(), 1);
                } else if !buried && !reacting {
                    assert_eq!(reduced.len(), 1);
                    assert_eq!(manual.len(), 2);
                } else {
                    assert_eq!(reduced.len(), manual.len());
                }
                for end in reduced.iter().chain(&manual) {
                    assert_eq!(end.status, Status::Complete(ReturnValue::Unit));
                    assert_eq!(
                        end.game.position.cards(Zone::Score(A)).len(),
                        if buried || another_top { 2 } else { 1 }
                    );
                    assert_eq!(
                        end.game.position.locate(seven),
                        Some(if reacting {
                            Zone::Junk
                        } else {
                            Zone::Deck(Deck::base(7))
                        })
                    );
                    if reacting {
                        assert!(end.events.iter().any(|event| matches!(event, Event::Moved(m) if m.card == seven && m.inside_karma)));
                    }
                }
                if let Some(lower) = lower {
                    assert!(
                        reduced.iter().any(|end| end
                            .game
                            .position
                            .cards(Zone::Score(A))
                            .contains(&lower))
                    );
                    if another_top {
                        assert!(
                            reduced.iter().any(|end| !end
                                .game
                                .position
                                .cards(Zone::Score(A))
                                .contains(&lower))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn sandpaper_optional_return_oracle_preserves_subsets_and_maxwells_child_draws() {
    for same_deck in [false, true] {
        for reacting in [false, true] {
            let mut f = Fixture::default();
            let source = printed(&mut f, "E029", A);
            let first = tech(&mut f, Color::Red, 1, Zone::Hand(A));
            let second = tech(
                &mut f,
                Color::Blue,
                if same_deck { 1 } else { 2 },
                Zone::Hand(A),
            );
            for _ in 0..2 {
                tech(&mut f, Color::Green, 3, Zone::Deck(Deck::base(3)));
            }
            let eights: Vec<_> = (0..2)
                .map(|_| tech(&mut f, Color::Purple, 8, Zone::Deck(Deck::base(8))))
                .collect();
            if reacting {
                figure(&mut f, Figure::JamesClerkMaxwell, B);
            }
            let mut game = Game::seeded(f.position(), BOTH, 47)
                .unwrap()
                .with_tracking()
                .unwrap();
            effect(&mut game, source, 0, Context::new(A));
            let (reduced, manual) = oracle(game);
            assert_eq!(manual.len(), 5);
            assert_eq!(reduced.len(), if same_deck || reacting { 5 } else { 4 });
            for end in reduced.iter().chain(&manual) {
                let returns = end.events.iter().filter(|event| matches!(event, Event::Moved(m) if [first,second].contains(&m.card) && m.verb == Some(Verb::Return) && !m.inside_karma)).count();
                let rewards = end
                    .game
                    .position
                    .cards(Zone::Hand(A))
                    .iter()
                    .filter(|&&card| end.game.position.card(card).value == 3)
                    .count();
                assert_eq!(returns, rewards);
                if reacting {
                    assert_eq!(end.events.iter().filter(|event| matches!(event, Event::Moved(m) if eights.contains(&m.card) && m.inside_karma)).count(), returns);
                }
            }
            if reacting {
                assert!(reduced.iter().any(|end| end.events.iter().any(|event| matches!(event, Event::Moved(m) if eights.contains(&m.card) && m.inside_karma))));
            }
        }
    }
}

#[test]
fn homing_pigeons_return_oracle_keeps_quota_subsets_same_deck_order_and_maxwell() {
    for count in [2, 3] {
        for same_deck in [false, true] {
            for reacting in [false, true] {
                let mut f = Fixture::default();
                let source = printed(&mut f, "E031", A);
                for index in 0..count {
                    let age = if same_deck { 1 } else { index as u8 + 1 };
                    tech(&mut f, Color::Red, age, Zone::Score(B));
                    tech(&mut f, Color::Purple, age, Zone::Hand(A));
                }
                let eights: Vec<_> = (0..2)
                    .map(|_| tech(&mut f, Color::Blue, 8, Zone::Deck(Deck::base(8))))
                    .collect();
                if reacting {
                    figure(&mut f, Figure::JamesClerkMaxwell, A);
                }
                let mut game = Game::seeded(f.position(), BOTH, 31)
                    .unwrap()
                    .with_tracking()
                    .unwrap();
                effect(&mut game, source, 0, demand());
                let (reduced, manual) = oracle(game);
                assert_eq!(manual.len(), if count == 2 { 2 } else { 6 });
                assert_eq!(
                    reduced.len(),
                    if count == 2 && !same_deck && !reacting {
                        1
                    } else {
                        manual.len()
                    }
                );
                for end in reduced.iter().chain(&manual) {
                    assert_eq!(end.game.position.cards(Zone::Score(B)).len(), count - 2);
                    assert_eq!(end.events.iter().filter(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Return) && !m.inside_karma)).count(), 2);
                    if reacting {
                        for &card in &eights {
                            assert_eq!(end.game.position.locate(card), Some(Zone::Hand(A)));
                        }
                        assert_eq!(end.events.iter().filter(|event| matches!(event, Event::Moved(m) if eights.contains(&m.card) && m.inside_karma)).count(), 2);
                    }
                }
            }
        }
    }
}

#[test]
fn homing_pigeons_foreseen_block_oracle_preserves_optional_splay_and_shens_replacement() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let mut source = echoes_card("E031").unwrap();
        // Isolate the actual non-demand effect while native promotion supplies
        // foreseen. The sharer keeps Shen active on their own green pile; the
        // source's owner's green promotion would otherwise cover him.
        source.effects = vec![source.effects[1].clone()];
        let source = f.add(source, Zone::Forecast(A));
        for color in [Color::Red, Color::Green, Color::Blue] {
            for _ in 0..2 {
                tech(&mut f, color, 1, Zone::Board(B, color));
            }
        }
        if reacting {
            figure(&mut f, Figure::ShenKuo, B);
        }
        let blue = *f
            .placements
            .iter()
            .filter(|(_, zone)| *zone == Zone::Board(B, Color::Blue))
            .map(|(card, _)| card)
            .next_back()
            .unwrap();
        f.cards[usize::from(blue.0)].symbols = [Symbol::Icon(Icon::Health); SLOTS];
        let driver = tech(&mut f, Color::Red, 3, Zone::Hand(A));
        for _ in 0..2 {
            tech(&mut f, Color::Purple, 3, Zone::Deck(Deck::base(3)));
        }
        let mut game = Game::seeded(f.position(), BOTH, 33)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(Action::Meld { card: driver }, ActionKind::Regular)
            .unwrap();
        // Enter the sharer's real promoted invocation. Keep its native
        // Dogma/ActionTail records so Foreseen reads ordinary engine history,
        // and skip only the subsequent independent owner execution.
        let mut entered = false;
        for _ in 0..1000 {
            if let Some(Frame::Program {
                source: Source::Effect { card, effect: 0 },
                context,
                pc: 0,
                ..
            }) = game.frames.last()
                && *card == source
                && context.executor == B
            {
                assert!(game.foreseen(*context));
                entered = true;
                break;
            }
            match game.advance(1, &mut Vec::new()).unwrap() {
                Status::Decision(menu) => {
                    let choice = menu
                        .options
                        .iter()
                        .position(|option| {
                            matches!(option,
                        Alternative::Select { target: TargetDescriptor::Known { card, .. } }
                            if *card == source)
                        })
                        .expect("native promotion selection");
                    game.choose(choice).unwrap();
                }
                Status::WorkLimit => {}
                other => panic!("missing foreseen shared invocation: {other:?}"),
            }
        }
        assert!(entered);
        let Some(Frame::Dogma(dogma)) = game
            .frames
            .iter_mut()
            .rev()
            .find(|frame| matches!(frame, Frame::Dogma(dogma) if dogma.card == source))
        else {
            panic!("native foreseen dogma")
        };
        assert_eq!(dogma.participant, 1);
        dogma.participant = 2;
        let (reduced, manual) = oracle(game);
        if reacting {
            assert_eq!(
                reduced.len(),
                manual.len(),
                "Shen keeps ordered reaction choices"
            );
        } else {
            assert!(reduced.len() < manual.len(), "the all-color block reduces");
        }
        for end in reduced.iter().chain(&manual) {
            for color in [Color::Red, Color::Green, Color::Blue] {
                assert_eq!(
                    end.game.position.splay(B, color),
                    if reacting { Splay::Right } else { Splay::Left }
                );
            }
            if reacting {
                assert!(end.events.iter().any(|event| matches!(
                    event,
                    Event::Splayed {
                        player: B,
                        to: Splay::Right,
                        inside_karma: true,
                        ..
                    }
                )));
            }
        }
        if reacting {
            assert!(
                reduced.iter().any(|end| end.decisions > 1),
                "Shen retains the sharer's real ordered reaction choices"
            );
        }
    }
}

#[test]
fn deodorant_deck_sweep_oracle_uses_archimedes_actual_box_adjustment() {
    for archimedes in [false, true] {
        let mut f = Fixture::default();
        let source = printed(&mut f, "E028", A);
        let industry = tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
        icon(&mut f, industry, Icon::Industry);
        if archimedes {
            figure(&mut f, Figure::Archimedes, A);
        }
        let mut fourth = Vec::new();
        let mut fifth = Vec::new();
        for _ in 0..3 {
            fourth.push(tech(&mut f, Color::Purple, 4, Zone::Deck(Deck::base(4))));
            fifth.push(tech(&mut f, Color::Green, 5, Zone::Deck(Deck::base(5))));
        }
        let mut game = Game::seeded(f.position(), BOTH, 28)
            .unwrap()
            .with_tracking()
            .unwrap();
        effect(&mut game, source, 1, Context::new(A));
        let (reduced, manual) = oracle(game);
        assert_eq!(reduced.len(), 1);
        assert_eq!(manual.len(), 6);
        for end in reduced.iter().chain(&manual) {
            for &card in &fourth {
                assert_eq!(
                    end.game.position.locate(card),
                    Some(if archimedes {
                        Zone::Deck(Deck::base(4))
                    } else {
                        Zone::Junk
                    })
                );
            }
            for &card in &fifth {
                assert_eq!(
                    end.game.position.locate(card),
                    Some(if archimedes {
                        Zone::Junk
                    } else {
                        Zone::Deck(Deck::base(5))
                    })
                );
            }
        }
        // No native Figure directly intercepts a Base age-4/5 deck Junk:
        // Rod's threshold is <4, Florence's source is her score pile.
        // Archimedes' real First changes which complete deck this phase sweeps.
    }
}
