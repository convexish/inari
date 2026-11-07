use inno_engine::*;

mod common;
use common::*;

fn performed(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: false,
    }))
}

/// A draw that took a card above its requested age (an empty age was
/// skipped): performed, but short of the requested value.
fn escalated(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: true,
    }))
}

fn score(card: CardId, player: Player) -> Operation {
    Operation::Move {
        verb: Verb::Score,
        target: target(card, Zone::Hand(player)),
        destination: Zone::Score(player),
    }
}

fn aliases(position: &Position, player: Player) -> Vec<Target> {
    position
        .targets(LogicalZone::AvailableAchievements(player))
        .collect()
}

#[test]
fn cai_lun_aliases_only_live_own_unsplayed_tops_and_keeps_supply_private() {
    let mut f = Fixture::default();
    let cai = f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    let buried = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let top = f.tech(Color::Red, 3, Zone::Board(A, Color::Red), vec![]);
    let foreign = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
    let supply = f.tech(Color::Purple, 3, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let all = aliases(game.position(), A);
    assert_eq!(
        all.iter().map(|t| t.card).collect::<Vec<_>>(),
        [supply, top, cai]
    );
    assert!(!all.iter().any(|t| [buried, foreign].contains(&t.card)));
    assert_eq!(
        aliases(game.position(), B)
            .iter()
            .map(|t| t.card)
            .collect::<Vec<_>>(),
        [supply]
    );
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .targets(LogicalZone::AvailableAchievements(A))
            .count(),
        1
    );
    let retained = *all.iter().find(|t| t.card == top).unwrap();
    for viewer in [A, B] {
        assert!(
            matches!(game.position().descriptor(retained, viewer), TargetDescriptor::Known { card, source: Zone::Board(A, Color::Red) } if card == top)
        );
        assert!(matches!(
            game.position().descriptor(all[0], viewer),
            TargetDescriptor::Hidden {
                source: Zone::AvailableAchievements,
                ..
            }
        ));
    }
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Red,
            direction: Splay::Right,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    assert!(!aliases(game.position(), A).iter().any(|t| t.card == top));
    game.start(
        Operation::Move {
            verb: Verb::Junk,
            target: retained,
            destination: Zone::Junk,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            ..
        }))
    ));
    assert!(events.is_empty());
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        &[supply]
    );
    assert!(game.position().check_conservation());
}

#[test]
fn cai_lun_source_loss_requeries_aliases_and_independent_mcadam_permission() {
    let mut f = Fixture::default();
    // Both native Figures are yellow: exposing one must not retain the other's Each.
    f.figure(Figure::JohnLoudonMcAdam, Zone::Board(A, Color::Yellow));
    let cai = f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    let blank = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    let industry = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    f.cards[usize::from(industry.0)].symbols[0] = Symbol::Icon(Icon::Industry);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(
        aliases(game.position(), A)
            .iter()
            .filter(|t| t.card == industry)
            .count(),
        1
    );
    let old = aliases(game.position(), A)
        .into_iter()
        .find(|t| t.card == blank)
        .unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Return,
            target: target(cai, Zone::Board(A, Color::Yellow)),
            destination: Zone::Deck(Deck::figures(2)),
        },
        Context::new(A),
    )
    .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), performed(cai, 2));
    assert!(!aliases(game.position(), A).iter().any(|t| t.card == blank));
    assert_eq!(
        aliases(game.position(), A)
            .iter()
            .filter(|t| t.card == industry)
            .count(),
        1
    );
    game.start(
        Operation::Move {
            verb: Verb::Junk,
            target: old,
            destination: Zone::Junk,
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            ..
        }))
    ));
}

#[test]
fn cai_lun_all_color_first_runs_for_either_executor_and_keeps_original_score_result() {
    for executor in [A, B] {
        let mut f = Fixture::default();
        for color in Color::ALL {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
            if color != Color::Yellow {
                f.tech(color, 2, Zone::Board(A, color), vec![]);
            }
        }
        f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
        let card = f.tech(Color::Red, 3, Zone::Hand(executor), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(executor);
        context.controller = executor.opponent();
        context.demander = B;
        game.start(score(card, executor), context).unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), performed(card, 3));
        assert_eq!(events.len(), 6);
        for (event, color) in events[..5].iter().zip(Color::ALL) {
            assert_eq!(
                *event,
                Event::Splayed {
                    player: A,
                    color,
                    from: Splay::None,
                    to: Splay::Left,
                    executor: A,
                    inside_karma: true
                }
            );
            assert_eq!(game.position().splay(A, color), Splay::Left);
        }
        assert_eq!(moved(&events[5]).card, card);
        assert_eq!(moved(&events[5]).executor, executor);
        assert!(!moved(&events[5]).inside_karma);
        assert!(aliases(game.position(), A).is_empty());
        assert!(game.position().check_conservation());
    }
}

#[test]
fn cai_lun_first_suppresses_real_shen_and_machiavelli_splay_reactions() {
    let mut f = Fixture::default();
    for color in [Color::Green, Color::Purple, Color::Yellow] {
        f.tech(color, 1, Zone::Board(A, color), vec![]);
    }
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.figure(Figure::NiccoloMachiavelli, Zone::Board(A, Color::Purple));
    let cai = f.figure(Figure::CaiLun, Zone::Hand(A));
    for value in [1, 2] {
        f.tech(Color::Green, value, Zone::Board(B, Color::Green), vec![]);
    }
    let card = f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Splay {
            player: B,
            color: Color::Green,
            direction: Splay::Right,
        },
        Context::new(B),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    game.start(meld(cai, Zone::Hand(A), A, Color::Yellow), Context::new(A))
        .unwrap();
    settle(&mut game, &mut Vec::new());
    game.start(score(card, B), Context::new(B)).unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(card, 3));
    assert_eq!(events.len(), 4);
    assert_eq!(game.position().splay(A, Color::Green), Splay::Left);
    assert_eq!(game.position().splay(A, Color::Purple), Splay::Left);
    assert_eq!(game.position().splay(B, Color::Green), Splay::Right);
    assert!(events[..3].iter().all(|e| matches!(
        e,
        Event::Splayed {
            to: Splay::Left,
            inside_karma: true,
            ..
        }
    )));
}

#[test]
fn galileo_effect_junks_cai_lun_public_alias_without_claim_eligibility() {
    let mut f = Fixture::default();
    f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let alias = f.tech(Color::Blue, 3, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Purple, 3, Zone::AvailableAchievements, vec![]);
    let pending = f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(!game.position().eligible_for_achievement(A, alias));
    game.start(meld(pending, Zone::Hand(A), A, Color::Red), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.iter().any(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Junk,
            source: Zone::AvailableAchievements,
            ..
        }
    )));
    let index = menu.options.iter().position(|o| matches!(o, Alternative::Move { verb: Verb::Junk, target: TargetDescriptor::Known { card, source: Zone::Board(A, Color::Blue) } } if *card == alias)).unwrap();
    game.choose(index).unwrap();
    assert_eq!(settle(&mut game, &mut events), performed(pending, 1));
    assert_eq!(moved(&events[0]).source, Zone::Board(A, Color::Blue));
    assert_eq!(moved(&events[0]).card, alias);
    assert!(moved(&events[0]).inside_karma);
}

#[test]
fn leeuwenhoek_claim_points_are_exact_age_additive_and_do_not_change_score_or_top() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::Tigernmas, Zone::Board(A, Color::Red));
    f.figure(Figure::Daedalus, Zone::Board(A, Color::Blue));
    f.tech(Color::Green, 6, Zone::Hand(A), vec![]);
    f.figure(Figure::JohnLoudonMcAdam, Zone::Hand(A));
    f.tech(Color::Purple, 8, Zone::Score(A), vec![]);
    let six = f.tech(Color::Green, 6, Zone::AvailableAchievements, vec![]);
    let five = f.tech(Color::Green, 5, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().score(A), 10); // Eight real points plus Tigernmas' two.
    assert_eq!(game.position().highest_top_value(A), 5);
    assert_eq!(game.position().highest_claim_value(A), 8);
    assert!(game.position().eligible_for_achievement(A, six));
    assert!(!game.position().eligible_for_achievement(A, five));
    let claim = aliases(game.position(), A)
        .into_iter()
        .find(|t| t.card == six)
        .unwrap();
    game.start(
        Operation::Claim {
            target: claim,
            eligibility: ClaimEligibility::Available,
        },
        Context::new(A),
    )
    .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), performed(six, 6));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[six]);
}

#[test]
fn leeuwenhoek_basic_hand_ignores_meiji_aliases_and_duplicate_claim_cost_is_live() {
    for own in [2, 3, 4] {
        let mut f = Fixture::default();
        let lee = f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        for _ in 0..own {
            f.tech(Color::Red, 4, Zone::Hand(A), vec![]);
        }
        for _ in 0..4 {
            f.tech(Color::Blue, 4, Zone::Hand(B), vec![]);
        }
        f.tech(Color::Green, 4, Zone::Achievements(A), vec![]);
        let candidate = f.tech(Color::Red, 4, Zone::AvailableAchievements, vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(
            game.position().targets(LogicalZone::Hand(A)).count(),
            own + 4
        );
        assert_eq!(game.position().score(A), 0);
        assert_eq!(
            game.position().eligible_for_achievement(A, candidate),
            own == 4
        );
        game.start(
            Operation::Move {
                verb: Verb::Return,
                target: target(lee, Zone::Board(A, Color::Yellow)),
                destination: Zone::Deck(Deck::figures(5)),
            },
            Context::new(A),
        )
        .unwrap();
        settle(&mut game, &mut Vec::new());
        assert!(!game.position().eligible_for_achievement(A, candidate));
    }
}

#[test]
fn leeuwenhoek_points_alone_never_raise_highest_top_claim_eligibility() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    for _ in 0..4 {
        f.tech(Color::Red, 7, Zone::Hand(A), vec![]);
    }
    let seven = f.tech(Color::Red, 7, Zone::AvailableAchievements, vec![]);
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.highest_claim_value(A), 5);
    assert!(!position.eligible_for_achievement(A, seven));
    assert_eq!(position.score(A), 0);
}

#[test]
fn leeuwenhoek_draw_payment_is_optional_exact_live_hand_and_preserves_original_result() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let own = f.tech(Color::Red, 5, Zone::Hand(A), vec![]);
    let borrowed = f.tech(Color::Green, 5, Zone::Hand(B), vec![]);
    let wrong = f.tech(Color::Red, 6, Zone::Hand(A), vec![]);
    let bonus = f.tech(Color::Red, 6, Zone::Deck(Deck::base(6)), vec![]);
    let original = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 3);
    assert!(events.is_empty());
    for (index, option) in menu.options.iter().enumerate() {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        fork.choose(index).unwrap();
        assert_eq!(settle(&mut fork, &mut suffix), performed(original, 1));
        match option {
            Alternative::Decline => {
                assert_eq!(suffix.len(), 1);
                assert_eq!(fork.position().cards(Zone::Deck(Deck::base(6))), &[bonus]);
            }
            Alternative::Move {
                verb: Verb::Return,
                target: TargetDescriptor::Known { card, source },
            } => {
                assert!([own, borrowed].contains(card));
                assert_ne!(*card, wrong);
                assert_eq!(suffix.len(), 3);
                assert_eq!(moved(&suffix[0]).source, *source);
                assert_eq!(moved(&suffix[0]).card, *card);
                assert_eq!(moved(&suffix[1]).card, bonus);
                assert!(suffix[..2].iter().all(|e| moved(e).inside_karma));
            }
            other => panic!("unexpected payment {other:?}"),
        }
        assert_eq!(moved(suffix.last().unwrap()).card, original);
        assert!(!moved(suffix.last().unwrap()).inside_karma);
        assert!(fork.position().check_conservation());
    }
}

#[test]
fn leeuwenhoek_no_payment_or_inactive_source_leaves_only_the_original_draw() {
    for case in 0..3 {
        let mut f = Fixture::default();
        f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
        if case == 1 {
            f.tech(Color::Yellow, 6, Zone::Board(A, Color::Yellow), vec![]);
        }
        if case != 0 {
            f.tech(Color::Red, 5, Zone::Hand(A), vec![]);
        }
        let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        let six = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Draw {
                value: 1,
                set: None,
            },
            Context::new(if case >= 2 { B } else { A }),
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), performed(drawn, 1));
        assert_eq!(events.len(), 1);
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(6))), &[six]);
    }
}

#[test]
fn native_archery_junks_cai_lun_himself_and_immediately_removes_other_aliases() {
    let mut f = Fixture::default();
    let definition = base_card("B001").unwrap();
    let color = definition.color;
    let source = f.add(definition, Zone::Board(A, color));
    let cai = f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    let other = f.tech(Color::Blue, 2, Zone::Board(A, Color::Blue), vec![]);
    let supply = f.tech(Color::Purple, 1, Zone::AvailableAchievements, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect: 1,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.iter().any(|o| matches!(
        o,
        Alternative::BlindMove {
            verb: Verb::Junk,
            source: Zone::AvailableAchievements,
            ..
        }
    )));
    let index = menu.options.iter().position(|o| matches!(o, Alternative::Move { verb: Verb::Junk, target: TargetDescriptor::Known { card, source: Zone::Board(A, Color::Yellow) } } if *card == cai)).unwrap();
    game.choose(index).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(moved(&events[0]).card, cai);
    assert_eq!(moved(&events[0]).source, Zone::Board(A, Color::Yellow));
    assert!(!aliases(game.position(), A).iter().any(|t| t.card == other));
    assert_eq!(
        aliases(game.position(), A)
            .iter()
            .map(|t| t.card)
            .collect::<Vec<_>>(),
        [supply]
    );
    assert!(game.position().check_conservation());
}

#[test]
fn cai_lun_claim_rechecks_membership_after_hedy_splays_the_selected_color() {
    let mut f = Fixture::default();
    f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    f.figure(Figure::HedyLamarr, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let selected = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Blue, 10, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(game.position().eligible_for_achievement(A, selected));
    let alias = aliases(game.position(), A)
        .into_iter()
        .find(|t| t.card == selected)
        .unwrap();
    game.start(
        Operation::Claim {
            target: alias,
            eligibility: ClaimEligibility::Available,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    // Hedy may name any nonempty color, the lone Figures' too (a no-op
    // choice is allowed): splay red.
    let Status::Decision(d) = settle(&mut game, &mut events) else {
        panic!("Hedy's splay is a choice");
    };
    let red = d
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Splay {
                    color: Color::Red,
                    ..
                }
            )
        })
        .unwrap();
    game.choose(red).unwrap();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            ..
        }))
    ));
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        Event::Splayed {
            color: Color::Red,
            to: Splay::Up,
            inside_karma: true,
            ..
        }
    ));
    assert_eq!(game.position().top(A, Color::Red), Some(selected));
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
}

#[test]
fn leeuwenhoek_karma_boxes_ignore_archimedes_and_move_last_keeps_original_draw_identity() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let source = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![
            build::draw(1),
            build::verb(Verb::Score, build::LAST).to(Destination::Score(Who::Executor)),
        ],
    );
    let payment = f.tech(Color::Green, 5, Zone::Hand(A), vec![]);
    let wrong_payment = f.tech(Color::Green, 6, Zone::Hand(A), vec![]);
    let original = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let bonus = f.tech(Color::Purple, 6, Zone::Deck(Deck::base(6)), vec![]);
    let wrong_bonus = f.tech(Color::Purple, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(source, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    let index = menu.options.iter().position(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == payment)).unwrap();
    game.choose(index).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [payment, bonus, original, original]
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[original]);
    assert_eq!(
        game.position().cards(Zone::Hand(A)),
        &[wrong_payment, bonus]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(7))),
        &[wrong_bonus]
    );
    assert!(events[..2].iter().all(|e| moved(e).inside_karma));
    assert!(events[2..].iter().all(|e| !moved(e).inside_karma));
}

#[test]
fn cai_lun_can_claim_himself_without_duplicating_any_physical_card() {
    let mut f = Fixture::default();
    let cai = f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    let other = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Blue, 10, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let alias = aliases(game.position(), A)
        .into_iter()
        .find(|t| t.card == cai)
        .unwrap();
    assert!(game.position().eligible_for_achievement(A, cai));
    game.start(
        Operation::Claim {
            target: alias,
            eligibility: ClaimEligibility::Available,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(cai, 2));
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).source, Zone::Board(A, Color::Yellow));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[cai]);
    assert!(!aliases(game.position(), A).iter().any(|t| t.card == other));
    assert!(game.position().check_conservation());
}

#[test]
fn leeuwenhoek_return_does_not_reset_the_original_draw_age() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    let payment = f.tech(Color::Red, 5, Zone::Hand(A), vec![]);
    let bonus = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let seven = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 5,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    // The draw resolves to age 6 before the karma refills Base 5. It keeps that age and
    // escalates past the now-empty 6.
    choose_card(&mut game, &mut events, payment);
    assert_eq!(settle(&mut game, &mut events), escalated(seven, 7));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [payment, bonus, seven]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[bonus, seven]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(5))), &[payment]);
    assert!(!moved(&events[2]).inside_karma);
}

#[test]
fn leeuwenhoek_payment_requeries_meiji_timing_instead_of_borrowing_a_stale_hand() {
    let mut f = Fixture::default();
    f.figure(Figure::AntonieVanLeeuwenhoek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    let borrowed = f.tech(Color::Red, 5, Zone::Hand(B), vec![]);
    let original = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let bonus = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.targets(LogicalZone::Hand(A)).count(), 1);
    position.begin_action(ActionKind::Regular).unwrap();
    position.begin_action(ActionKind::Regular).unwrap();
    let mut game = Game::new(position, Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 1,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(original, 1));
    assert_eq!(events.len(), 1);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[borrowed]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(6))), &[bonus]);
}

#[test]
fn scoring_cai_lun_finishes_his_first_then_disables_both_clauses() {
    let mut f = Fixture::default();
    f.tech(Color::Yellow, 1, Zone::Board(A, Color::Yellow), vec![]);
    let cai = f.figure(Figure::CaiLun, Zone::Board(A, Color::Yellow));
    for value in [1, 2] {
        f.tech(Color::Blue, value, Zone::Board(A, Color::Blue), vec![]);
    }
    let next = f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(cai, Zone::Board(A, Color::Yellow)),
            destination: Zone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), performed(cai, 2));
    assert_eq!(events.len(), 3);
    assert!(events[..2].iter().all(|event| matches!(
        event,
        Event::Splayed {
            to: Splay::Left,
            inside_karma: true,
            ..
        }
    )));
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Left);
    assert!(!game.position().has_figure(A, Figure::CaiLun));
    assert!(aliases(game.position(), A).is_empty());
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Blue,
            direction: Splay::Right,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    game.start(score(next, B), Context::new(B)).unwrap();
    events.clear();
    assert_eq!(settle(&mut game, &mut events), performed(next, 3));
    assert_eq!(events.len(), 1);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Right);
}
