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

fn draw(game: &mut Game, value: u8, set: Option<CardSet>) {
    game.start(Operation::Draw { value, set }, Context::new(A))
        .unwrap();
}

fn icons(f: &mut Fixture, color: Color, player: Player, symbols: [Symbol; 4]) -> CardId {
    let card = f.tech(color, 1, Zone::Board(player, color), vec![]);
    f.cards[usize::from(card.0)].symbols = four_slots(symbols);
    card
}

#[test]
fn tran_and_maxwell_use_independent_basic_counts_not_borrowed_hands_or_added_authority() {
    let mut f = Fixture::default();
    f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    icons(
        &mut f,
        Color::Green,
        A,
        [
            Symbol::Icon(Icon::Authority),
            Symbol::Icon(Icon::Concept),
            Symbol::Blank,
            Symbol::Blank,
        ],
    );
    for player in [A, A, A, B, B] {
        f.tech(Color::Yellow, 1, Zone::Hand(player), vec![]);
    }
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.targets(LogicalZone::Hand(A)).count(), 5);
    assert_eq!(position.basic_icon_count(A, Icon::Authority), 3);
    for icon in position.standard_icons() {
        let basic = position.basic_icon_count(A, icon);
        let extra = if basic == 0 {
            0
        } else {
            3 + usize::from(icon != Icon::Authority)
        };
        assert_eq!(position.icon_count(A, icon), basic + extra, "{icon:?}");
        assert_eq!(
            position
                .clone()
                .with_expansions(Expansions::BASE)
                .icon_count(A, icon),
            basic
        );
    }
    assert_eq!(position.icon_count(A, Icon::Health), 0);
}

#[test]
fn tran_does_not_make_another_eachs_virtual_type_present() {
    let mut f = Fixture::default();
    f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    let position = f.position().with_expansions(Expansions::FIGURES);
    assert_eq!(position.basic_icon_count(A, Icon::Efficiency), 0);
    assert_eq!(position.icon_count(A, Icon::Efficiency), 2);
    assert_eq!(position.icon_count(A, Icon::Concept), 5);
    assert_eq!(position.icon_count(A, Icon::Authority), 2);
    assert_eq!(position.icon_count(A, Icon::Avatar), 3);
}

#[test]
fn tran_avatar_is_standard_and_food_waits_for_age_zero_introduction() {
    let mut f = Fixture::default();
    f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    icons(
        &mut f,
        Color::Green,
        A,
        [
            Symbol::Icon(Icon::Food),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ],
    );
    f.tech(Color::Yellow, 0, Zone::AgeZeroReserve, vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 2);
    assert_eq!(game.position().icon_count(A, Icon::Food), 1);
    game.start(Operation::IntroduceAgeZero, Context::new(A))
        .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert_eq!(game.position().icon_count(A, Icon::Food), 2);
}

#[test]
fn tran_effective_icons_change_actual_dogma_sharing() {
    let mut f = Fixture::default();
    f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    let program = icons(
        &mut f,
        Color::Blue,
        A,
        [
            Symbol::Icon(Icon::Concept),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ],
    );
    f.cards[usize::from(program.0)].featured = Some(Icon::Concept);
    f.cards[usize::from(program.0)].effects[0].instructions = vec![build::draw(1)];
    icons(
        &mut f,
        Color::Green,
        B,
        [
            Symbol::Icon(Icon::Concept),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ],
    );
    let reward = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Dogma {
            card: program,
            icon: Icon::Concept,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).destination, Zone::Hand(A));
    assert_eq!(moved(&events[0]).card, reward);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
}

#[test]
fn tran_resolved_three_keeps_both_public_red_choices_and_original_draw_on_clones() {
    for choose_own in [false, true] {
        let mut f = Fixture::default();
        let tran = f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
        // Shen Kuo would replace scoring a Base card outside karma.
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        let opponent = f.add(base_card("B001").unwrap(), Zone::Board(B, Color::Red));
        let reward = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.controller = B;
        context.demander = B;
        context.source_owner = B;
        game.start(
            Operation::Draw {
                value: 1,
                set: None,
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, A);
        assert_eq!(menu.options.len(), 2);
        for card in [tran, opponent] {
            assert!(menu.options.iter().any(|option| matches!(option,
                Alternative::Move { verb: Verb::Score, target: TargetDescriptor::Known { card: candidate, .. } } if *candidate == card)));
        }
        let chosen = if choose_own { tran } else { opponent };
        let choice = menu.options.iter().position(|option| matches!(option,
            Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == chosen)).unwrap();
        let mut fork = game.clone();
        game.choose(choice).unwrap();
        fork.choose(choice).unwrap();
        assert_eq!(settle(&mut game, &mut events), escalated(reward, 3));
        let mut replay = Vec::new();
        assert_eq!(settle(&mut fork, &mut replay), escalated(reward, 3));
        assert_eq!(game, fork);
        assert_eq!(events, replay);
        assert_eq!(events.len(), 2);
        let score = moved(&events[0]);
        assert_eq!(
            (
                score.card,
                score.verb,
                score.destination,
                score.executor,
                score.inside_karma
            ),
            (chosen, Some(Verb::Score), Zone::Score(A), A, true)
        );
        assert_eq!(moved(&events[1]).card, reward);
        assert!(!moved(&events[1]).inside_karma);
        assert_eq!(
            game.position().has_figure(A, Figure::TranHuangDao),
            !choose_own
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn tran_filters_printed_top_red_values_and_does_not_intercept_a_resolved_four() {
    for resolved in [3, 4] {
        let mut f = Fixture::default();
        let tran = f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
        let buried = f.add(base_card("B001").unwrap(), Zone::Board(B, Color::Red));
        f.tech(Color::Red, 4, Zone::Board(B, Color::Red), vec![]);
        let reward = f.tech(
            Color::Green,
            resolved,
            Zone::Deck(Deck::base(resolved)),
            vec![],
        );
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, 3, None);
        let mut events = Vec::new();
        // A requested [3] resolved at [4] is short of its age.
        let expected = if resolved == 3 {
            performed(reward, resolved)
        } else {
            escalated(reward, resolved)
        };
        assert_eq!(settle(&mut game, &mut events), expected);
        assert_eq!(events.len(), if resolved == 3 { 2 } else { 1 });
        assert_eq!(
            game.position().cards(Zone::Score(A)),
            if resolved == 3 { vec![tran] } else { vec![] }
        );
        assert_eq!(game.position().cards(Zone::Board(B, Color::Red))[0], buried);
    }
}

#[test]
fn tran_figures_draw_preserves_set_after_scoring_itself() {
    let mut f = Fixture::default();
    let tran = f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    let base = f.tech(Color::Blue, 3, Zone::Deck(Deck::base(3)), vec![]);
    let figure = f.figure(Figure::ShenKuo, Zone::Deck(Deck::figures(3)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, 2, Some(CardSet::Figures));
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), escalated(figure, 3));
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [tran, figure]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), &[base]);
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 0);
}

#[test]
fn curie_counts_distinct_physical_values_not_michelangelo_score_aliases() {
    let mut f = Fixture::default();
    let curie = f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
    for value in [6, 7, 7, 9, 11] {
        f.tech(Color::Green, value, Zone::Score(A), vec![]);
    }
    for (player, value) in [(A, 8), (A, 10), (B, 8)] {
        f.tech(Color::Green, value, Zone::Hand(player), vec![]);
    }
    let claim = f.tech(Color::Red, 7, Zone::AvailableAchievements, vec![]);
    let cover = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().targets(LogicalZone::Score(A)).count(), 8);
    assert_eq!(game.position().effective_achievement_count(A), 3);
    assert_eq!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .effective_achievement_count(A),
        0
    );
    assert_eq!(game.position().achievement_count(A), 0);
    assert!(game.position().cards(Zone::Achievements(A)).is_empty());
    assert_eq!(game.position().cards(Zone::AvailableAchievements), &[claim]);
    assert!(game.position().eligible_for_achievement(A, claim));
    game.start(meld(cover, Zone::Hand(A), A, Color::Blue), Context::new(A))
        .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), performed(cover, 1));
    assert_eq!(game.position().effective_achievement_count(A), 0);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        &[curie, cover]
    );
    game.start(
        Operation::Move {
            verb: Verb::Junk,
            target: target(cover, Zone::Board(A, Color::Blue)),
            destination: Zone::Junk,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut Vec::new());
    assert_eq!(game.position().effective_achievement_count(A), 3);
    assert!(game.position().eligible_for_achievement(A, claim));
}

#[test]
fn curie_independent_virtual_achievements_reach_real_victory_without_claiming_tokens() {
    let mut f = Fixture::default();
    f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    f.figure(Figure::NelsonMandela, Zone::Board(A, Color::Red));
    for value in 7..=11 {
        f.tech(Color::Green, value, Zone::Score(A), vec![]);
    }
    let owned = f.tech(Color::Yellow, 1, Zone::Achievements(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().basic_icon_count(A, Icon::Avatar), 2);
    assert_eq!(game.position().effective_achievement_count(A), 7);
    assert_eq!(game.position().achievement_count(A), 1);
    game.start(Operation::CheckAchievementVictory, Context::new(A))
        .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), Status::Winner(A));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[owned]);
    assert_eq!(game.position().cards(Zone::Score(A)).len(), 5);
}

#[test]
fn curie_uses_resolved_value_and_live_logical_hand_including_meijis_alias() {
    for (held, borrowed, second_action, fires) in [
        (3, false, false, false),
        (2, false, false, true),
        (3, true, false, false),
        (3, true, true, true),
    ] {
        let mut f = Fixture::default();
        f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
        f.figure(Figure::EmperorMeiji, Zone::Board(A, Color::Purple));
        f.tech(
            Color::Green,
            held,
            Zone::Hand(if borrowed { B } else { A }),
            vec![],
        );
        let reward = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
        let child = f.tech(Color::Green, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut position = f.position().with_expansions(Expansions::FIGURES);
        if second_action {
            position.begin_action(ActionKind::Regular).unwrap();
            position.begin_action(ActionKind::Regular).unwrap();
        }
        let mut game = Game::new(position, Expansions::FIGURES).unwrap();
        draw(&mut game, 2, None);
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), escalated(reward, 3));
        assert_eq!(events.len(), if fires { 2 } else { 1 });
        if fires {
            assert_eq!(moved(&events[0]).card, child);
            assert!(moved(&events[0]).inside_karma);
        }
        assert!(!moved(events.last().unwrap()).inside_karma);
    }
}

#[test]
fn curie_child_consumes_last_nine_then_original_reroutes_without_rediscovery_on_clone() {
    let mut f = Fixture::default();
    f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    f.tech(Color::Green, 7, Zone::Score(A), vec![]);
    let child = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let original = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, 8, None);
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(moved(&events[0]).card, child);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().effective_achievement_count(A), 1);
    let mut fork = game.clone();
    assert_eq!(settle(&mut game, &mut events), escalated(original, 10));
    let mut suffix = Vec::new();
    assert_eq!(settle(&mut fork, &mut suffix), escalated(original, 10));
    assert_eq!(events[1..], suffix);
    assert_eq!(game, fork);
    assert_eq!(events.len(), 2);
    assert!(!moved(&events[1]).inside_karma);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[child, original]);
}

#[test]
fn curie_child_can_advance_to_ten_and_original_figures_request_is_reresolved() {
    for figures_available in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
        let child = f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]);
        let stranded = f.figure(Figure::SergeyBrin, Zone::Deck(Deck::figures(10)));
        let base = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
        let figure =
            figures_available.then(|| f.figure(Figure::BearGrylls, Zone::Deck(Deck::figures(11))));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, 8, Some(CardSet::Figures));
        let expected = figure.unwrap_or(base);
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), escalated(expected, 11));
        assert_eq!(
            events
                .iter()
                .map(|event| moved(event).card)
                .collect::<Vec<_>>(),
            [child, expected]
        );
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(10))),
            &[stranded]
        );
        assert!(moved(&events[0]).inside_karma);
        assert_eq!(
            moved(&events[1]).source,
            Zone::Deck(if figures_available {
                Deck::figures(11)
            } else {
                Deck::base(11)
            })
        );
    }
}

#[test]
fn curie_original_draw_not_child_draw_is_melded_by_native_experimentation() {
    let mut f = Fixture::default();
    f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    let program = f.experimentation(Zone::Hand(A));
    let original = f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]);
    let child = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(program, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        settle(&mut game, &mut events),
        Status::Complete(_)
    ));
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [child, original, original]
    );
    assert_eq!(game.position().top(A, Color::Green), Some(original));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[program, child]);
    assert_eq!(moved(&events[2]).verb, Some(Verb::Meld));
    assert!(!moved(&events[2]).inside_karma);
}

#[test]
fn curie_child_or_original_exhaustion_stops_pending_program_and_never_substitutes_child() {
    for child_available in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
        let program = f.experimentation(Zone::Hand(A));
        f.cards[usize::from(program.0)].effects.push(Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![
                build::verb(Verb::Score, build::HAND).to(Destination::Score(Who::Executor)),
            ],
        });
        let retained = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
        let low =
            (!child_available).then(|| f.tech(Color::Green, 5, Zone::Deck(Deck::base(5)), vec![]));
        let child =
            child_available.then(|| f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.self_execute(program, Context::new(A)).unwrap();
        let mut events = Vec::new();
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Draw(DrawReason::Exhaustion)
        );
        assert_eq!(events.len(), usize::from(child_available));
        assert!(game.position().cards(Zone::Hand(A)).contains(&retained));
        assert!(game.position().cards(Zone::Score(A)).is_empty());
        if let Some(card) = child {
            assert!(game.position().cards(Zone::Hand(A)).contains(&card));
            assert!(moved(&events[0]).inside_karma);
        }
        if let Some(card) = low {
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(5))), &[card]);
        }
    }
}

#[test]
fn opponent_and_covered_sources_do_not_intercept_draws() {
    for figure in [Figure::TranHuangDao, Figure::MarieCurie] {
        for covered in [false, true] {
            let mut f = Fixture::default();
            let color = figure_card(figure.code()).unwrap().color;
            let owner = if covered { A } else { B };
            f.figure(figure, Zone::Board(owner, color));
            if covered {
                f.tech(color, 1, Zone::Board(owner, color), vec![]);
            }
            let original = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
            let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            draw(&mut game, 3, None);
            let mut events = Vec::new();
            assert_eq!(settle(&mut game, &mut events), performed(original, 3));
            assert_eq!(events.len(), 1);
            assert!(!moved(&events[0]).inside_karma);
            assert_eq!(game.position().cards(Zone::Deck(Deck::base(9))), &[nine]);
        }
    }
}

#[test]
fn figures_classify_draw_hazards_without_disabling_unrelated_local_reductions() {
    for figure in [Figure::TranHuangDao, Figure::MarieCurie] {
        let dependencies = figure.shortcut_dependencies();
        // Oars' lowest-first policy draws inside its loop, so a draw hazard
        // blocks it. Batch reductions depend only on moves and memberships.
        assert!(dependencies.contains(&ShortcutDependency::Draw));
        assert!(!dependencies.iter().any(|dependency| matches!(
            dependency,
            ShortcutDependency::Move(_)
                | ShortcutDependency::HandMembership
                | ShortcutDependency::ScoreMembership
                | ShortcutDependency::JunkSpecial
        )));
        assert!(!dependencies.contains(&ShortcutDependency::Splay));
    }
}

#[test]
fn curie_distinct_values_update_when_duplicates_or_source_leave() {
    let mut f = Fixture::default();
    let curie = f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    let first = f.tech(Color::Green, 7, Zone::Score(A), vec![]);
    let second = f.tech(Color::Yellow, 7, Zone::Score(A), vec![]);
    f.tech(Color::Purple, 8, Zone::Score(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    for (card, source, expected) in [
        (first, Zone::Score(A), 2),
        (second, Zone::Score(A), 1),
        (curie, Zone::Board(A, Color::Blue), 0),
    ] {
        game.start(
            Operation::Move {
                verb: Verb::Junk,
                target: target(card, source),
                destination: Zone::Junk,
            },
            Context::new(A),
        )
        .unwrap();
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        assert_eq!(game.position().effective_achievement_count(A), expected);
        assert_eq!(game.position().achievement_count(A), 0);
    }
}

#[test]
fn native_oars_keeps_genuine_transfer_choices_for_both_draw_hazards() {
    for figure in [Figure::TranHuangDao, Figure::MarieCurie] {
        let mut f = Fixture::default();
        let oars = f.oars(Zone::Board(A, Color::Red));
        let color = figure_card(figure.code()).unwrap().color;
        f.figure(figure, Zone::Board(B, color));
        let high = f.tech(Color::Green, 7, Zone::Hand(B), vec![]);
        let low = f.tech(Color::Yellow, 2, Zone::Hand(B), vec![]);
        for card in [high, low] {
            f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Prosperity);
        }
        for _ in 0..2 {
            f.tech(Color::Purple, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        f.tech(Color::Green, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(B);
        context.demander = A;
        context.source_owner = A;
        game.start(
            Operation::ExecuteEffect {
                card: oars,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        assert_eq!(menu.options.len(), 2);
        assert!(events.is_empty());
        choose_card(&mut game, &mut events, high);
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(game.position().cards(Zone::Score(A)), &[high, low]);
        assert_eq!(moved(&events[0]).card, high);
        assert_eq!(moved(&events[0]).executor, B);
    }
}

#[test]
fn native_tools_keeps_its_reduced_payment_and_ordinary_figure_children() {
    for figure in [Figure::TranHuangDao, Figure::MarieCurie] {
        let mut f = Fixture::default();
        let tools = f.tools_first(Zone::Board(B, Color::Blue));
        let color = figure_card(figure.code()).unwrap().color;
        let source = f.figure(figure, Zone::Board(A, color));
        let hand: Vec<_> = [7, 1, 2]
            .into_iter()
            .map(|value| f.tech(Color::Green, value, Zone::Hand(A), vec![]))
            .collect();
        let reward = f.tech(Color::Yellow, 3, Zone::Deck(Deck::base(3)), vec![]);
        let nine = f.tech(Color::Green, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let mut context = Context::new(A);
        context.source_owner = B;
        game.start(
            Operation::ExecuteEffect {
                card: tools,
                effect: 0,
                boxed: BoxedValues::default(),
            },
            context,
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.options.len(), 2);
        assert!(menu.options.contains(&Alternative::Decline));
        assert!(events.is_empty());
        let commit = menu
            .options
            .iter()
            .position(|option| *option != Alternative::Decline)
            .unwrap();
        game.choose(commit).unwrap();
        assert!(matches!(
            settle(&mut game, &mut events),
            Status::Complete(_)
        ));
        assert_eq!(
            events
                .iter()
                .map(|event| moved(event).card)
                .collect::<Vec<_>>(),
            [
                hand[1],
                hand[2],
                hand[0],
                if figure == Figure::TranHuangDao {
                    source
                } else {
                    nine
                },
                reward,
                reward
            ]
        );
        assert!(moved(&events[3]).inside_karma);
        assert!(!moved(&events[4]).inside_karma);
        assert_eq!(game.position().top(A, Color::Yellow), Some(reward));
    }
}

#[test]
fn tran_pairs_follow_live_visible_authority_and_covering_it_with_gilgamesh_cuts_over() {
    let mut f = Fixture::default();
    icons(&mut f, Color::Red, A, [Symbol::Icon(Icon::Authority); 4]);
    f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    let gilgamesh = f.figure(Figure::Gilgamesh, Zone::Hand(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 2);
    for (direction, authority, avatar) in [
        (Splay::Right, 4, 3),
        (Splay::Up, 5, 3),
        (Splay::Aslant, 6, 4),
    ] {
        game.start(
            Operation::Splay {
                player: A,
                color: Color::Red,
                direction,
            },
            Context::new(A),
        )
        .unwrap();
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        assert_eq!(
            game.position().basic_icon_count(A, Icon::Authority),
            authority
        );
        assert_eq!(game.position().icon_count(A, Icon::Authority), authority);
        assert_eq!(game.position().icon_count(A, Icon::Avatar), avatar);
    }
    game.start(
        meld(gilgamesh, Zone::Hand(A), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    assert_eq!(settle(&mut game, &mut Vec::new()), performed(gilgamesh, 1));
    assert!(!game.position().has_figure(A, Figure::TranHuangDao));
    assert!(game.position().has_figure(A, Figure::Gilgamesh));
    assert_eq!(game.position().icon_count(A, Icon::Authority), 14);
    assert_eq!(game.position().icon_count(A, Icon::Avatar), 2);
}

#[test]
fn choosing_curie_over_trans_competing_first_preserves_only_the_selected_reaction() {
    let mut f = Fixture::default();
    let tran = f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    let curie = f.figure(Figure::MarieCurie, Zone::Board(A, Color::Blue));
    let original = f.tech(Color::Green, 3, Zone::Deck(Deck::base(3)), vec![]);
    let child = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, 3, None);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    let choice = menu
        .options
        .iter()
        .position(|option| {
            *option
                == Alternative::Karma {
                    source: curie,
                    figure: Figure::MarieCurie,
                }
        })
        .unwrap();
    game.choose(choice).unwrap();
    assert_eq!(settle(&mut game, &mut events), performed(original, 3));
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [child, original]
    );
    assert_eq!(game.position().top(A, Color::Red), Some(tran));
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}
