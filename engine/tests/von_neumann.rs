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

fn replaced() -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Replaced,
        card: None,
        value: None,
        short: false,
    }))
}

fn draw(game: &mut Game, player: Player, value: u8, set: Option<CardSet>) {
    game.start(Operation::Draw { value, set }, Context::new(player))
        .unwrap();
}

#[test]
fn skipped_ages_trigger_at_the_resolved_eight_then_reresolve_without_a_second_hook() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let bottom = f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]);
    let top = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
    let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, A, 5, None);
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    let mut fork = game.clone();
    let mut resumed = Vec::new();
    assert_eq!(settle(&mut fork, &mut resumed), escalated(nine, 9));
    assert_eq!(settle(&mut game, &mut events), escalated(nine, 9));
    assert_eq!(resumed, events[1..]);
    assert_eq!(fork, game);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [top, bottom, nine]
    );
    for event in &events[..2] {
        let movement = moved(event);
        assert_eq!(movement.verb, Some(Verb::Junk));
        assert_eq!(movement.source, Zone::Deck(Deck::base(8)));
        assert!(movement.inside_karma);
    }
    assert!(!moved(&events[2]).inside_karma);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[nine]);
    assert!(game.position().check_conservation());
}

#[test]
fn requesting_eight_with_an_empty_base_eight_does_not_intercept_the_resolved_nine() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    f.figure(Figure::GalileoGalilei, Zone::Board(A, Color::Green));
    let stranded = f.figure(Figure::DukeEllington, Zone::Deck(Deck::figures(8)));
    let nine = f.tech(Color::Blue, 9, Zone::Deck(Deck::base(9)), vec![]);
    // A nonempty 5 keeps Galileo's deck choice real; the test chooses the 4.
    f.tech(Color::Red, 5, Zone::Deck(Deck::base(5)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    draw(&mut game, A, 8, None);
    let mut events = Vec::new();
    // Only Galileo is applicable: its deck menu appears directly, rather than
    // a spurious competition with von Neumann for a requested but absent eight.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.options.len(), 2);
    assert!(
        menu.options
            .iter()
            .all(|option| matches!(option, Alternative::Value(_)))
    );
    game.choose(0).unwrap();
    assert_eq!(settle(&mut game, &mut events), escalated(nine, 9));
    assert_eq!(events.len(), 1);
    assert!(!moved(&events[0]).inside_karma);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(8))),
        &[stranded]
    );
}

#[test]
fn figures_request_preserves_set_and_fallback_but_never_junks_figures_eight() {
    for figures_at_nine in [false, true] {
        let mut f = Fixture::default();
        f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
        let eight = f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]);
        let stranded = f.figure(Figure::DukeEllington, Zone::Deck(Deck::figures(8)));
        let base = f.tech(Color::Green, 9, Zone::Deck(Deck::base(9)), vec![]);
        let figure =
            figures_at_nine.then(|| f.figure(Figure::Kalashnikov, Zone::Deck(Deck::figures(9))));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, A, 7, Some(CardSet::Figures));
        let mut events = Vec::new();
        let expected = figure.unwrap_or(base);
        assert_eq!(settle(&mut game, &mut events), escalated(expected, 9));
        assert_eq!(
            events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
            [eight, expected]
        );
        assert_eq!(game.position().cards(Zone::Junk), &[eight]);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(8))),
            &[stranded]
        );
        assert_eq!(
            moved(&events[1]).source,
            Zone::Deck(if figures_at_nine {
                Deck::figures(9)
            } else {
                Deck::base(9)
            })
        );
    }
}

#[test]
fn exhaustion_after_first_cancels_experimentations_meld_and_all_later_effects() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let program = f.experimentation(Zone::Board(A, Color::Blue));
    f.cards[usize::from(program.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![
            build::verb(Verb::Score, build::HAND).to(Destination::Score(Who::Executor)),
        ],
    });
    let retained = f.tech(Color::Yellow, 2, Zone::Hand(A), vec![]);
    let eight = f.tech(Color::Green, 8, Zone::Deck(Deck::base(8)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(program, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Draw(DrawReason::Exhaustion)
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, eight);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Junk));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[retained]);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn opponent_and_buried_sources_do_not_replace_draws_or_melds() {
    for buried in [false, true] {
        let mut f = Fixture::default();
        let owner = if buried { A } else { B };
        f.figure(Figure::JohnVonNeumann, Zone::Board(owner, Color::Red));
        if buried {
            f.tech(Color::Red, 9, Zone::Board(A, Color::Red), vec![]);
        }
        let eight = f.tech(
            Color::Blue,
            8,
            Zone::Deck(Deck::base(8)),
            vec![build::draw(9)],
        );
        let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        draw(&mut game, A, 8, None);
        let mut events = Vec::new();
        assert_eq!(settle(&mut game, &mut events), performed(eight, 8));
        game.start(meld(eight, Zone::Hand(A), A, Color::Blue), Context::new(A))
            .unwrap();
        assert_eq!(settle(&mut game, &mut events), performed(eight, 8));
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| !moved(event).inside_karma));
        assert_eq!(game.position().cards(Zone::Deck(Deck::base(9))), &[nine]);
        assert!(game.position().cards(Zone::Junk).is_empty());
    }
}

#[test]
fn replacement_meld_keeps_exact_score_source_and_returns_no_child_card_or_value() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let program = f.tech(Color::Blue, 5, Zone::Score(A), vec![build::draw(1)]);
    let decoy = f.tech(Color::Blue, 5, Zone::Hand(A), vec![]);
    let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(program, Zone::Score(A), A, Color::Blue),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), replaced());
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [program, drawn]
    );
    assert_eq!(moved(&events[0]).source, Zone::Score(A));
    assert_eq!(moved(&events[0]).verb, Some(Verb::Meld));
    assert!(
        events
            .iter()
            .all(|e| moved(e).inside_karma && moved(e).executor == A)
    );
    assert_eq!(game.position().top(A, Color::Blue), Some(program));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[decoy, drawn]);
}

#[test]
fn replaced_meld_cannot_pay_the_callers_if_you_do_reward() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let caller = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![
            build::verb(Verb::Meld, build::HAND).to(Destination::Board(Who::Executor)),
            build::if_(build::DID, 1),
            build::draw_age(Expr::Add(&build::LAST_VALUE, 1)),
        ],
    );
    let child = f.tech(Color::Blue, 5, Zone::Hand(A), vec![build::draw(1)]);
    let drawn = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let reward = f.tech(Color::Green, 6, Zone::Deck(Deck::base(6)), vec![]);
    let chain = f.tech(Color::Purple, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.self_execute(caller, Context::new(A)).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [child, chain, chain, drawn]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(6))), &[reward]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[chain]);
}

#[test]
fn experimentation_regular_dogma_melds_then_self_executes_only_nondemands_without_sharing() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let experimentation = f.experimentation(Zone::Board(A, Color::Blue));
    let child = f.tech(
        Color::Green,
        5,
        Zone::Deck(Deck::base(5)),
        vec![build::draw(1)],
    );
    f.cards[usize::from(child.0)].effects.insert(
        0,
        Effect {
            kind: EffectKind::Demand,
            instructions: vec![
                build::verb(Verb::Transfer, build::HAND).to(Destination::Hand(Who::Demander)),
            ],
        },
    );
    let peer = f.tech(Color::Yellow, 1, Zone::Board(B, Color::Yellow), vec![]);
    f.cards[usize::from(peer.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    let protected = f.tech(Color::Purple, 2, Zone::Hand(B), vec![]);
    let drawn = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(
        Action::Dogma {
            card: experimentation,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [child, child, drawn]
    );
    assert_eq!(
        events
            .iter()
            .map(|e| moved(e).inside_karma)
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    assert_eq!(moved(&events[1]).verb, Some(Verb::Meld));
    assert!(events.iter().all(|e| moved(e).executor == A));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[drawn]);
    assert_eq!(game.position().cards(Zone::Hand(B)), &[protected]);
    assert_eq!(game.position().top(A, Color::Green), Some(child));
    assert_eq!(game.position().regular_actions_started(), 1);
    assert!(game.position().check_conservation());
}

#[test]
fn replacement_child_meld_does_not_start_alexanders_when_after_covering_von_neumann() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Hand(A));
    let protected = f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        meld(alexander, Zone::Hand(A), A, Color::Red),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), replaced());
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).card, alexander);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().top(A, Color::Red), Some(alexander));
    assert_eq!(game.position().top(B, Color::Yellow), Some(protected));
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn suppression_survives_source_removal_multiple_effects_and_archimedes() {
    let mut f = Fixture::default();
    let source = f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let child = f.tech(
        Color::Green,
        5,
        Zone::Hand(A),
        vec![
            build::draw(8),
            build::verb(Verb::Score, Selector::TopOf(Who::Executor, Color::Red))
                .to(Destination::Score(Who::Executor)),
        ],
    );
    f.cards[usize::from(child.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![build::draw(8)],
    });
    let bottom = f.tech(Color::Yellow, 8, Zone::Deck(Deck::base(8)), vec![]);
    let top = f.tech(Color::Purple, 8, Zone::Deck(Deck::base(8)), vec![]);
    let nine = f.tech(Color::Yellow, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(child, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 2);
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [child, top]
    );
    assert_eq!(settle(&mut game, &mut events), replaced());
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [child, top, source, bottom]
    );
    assert!(
        events
            .iter()
            .all(|e| moved(e).inside_karma && moved(e).executor == A)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[top, bottom]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[source]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(9))), &[nine]);
    assert!(game.position().cards(Zone::Junk).is_empty());
}

#[test]
fn clones_resume_after_child_meld_and_private_choice_without_leaking_or_replaying() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    let child = f.agriculture(Zone::Hand(A));
    let payment = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let reward = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.start(
        meld(child, Zone::Hand(A), A, Color::Yellow),
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    advance_to_events(&mut game, &mut events, 1);
    assert_eq!(moved(&events[0]).card, child);
    assert_eq!(game.position().top(A, Color::Yellow), Some(child));
    let after_meld = game.clone();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    assert!(
        matches!(menu.options[0], Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if card == payment)
    );
    assert_eq!(menu.options[1], Alternative::Decline);
    assert!(matches!(
        game.position()
            .descriptor(target(payment, Zone::Hand(A)), B),
        TargetDescriptor::Hidden { .. }
    ));
    let at_choice = game.clone();
    assert_eq!(game.choose(menu.options.len()), Err(Error::InvalidChoice));
    assert_eq!(game, at_choice);
    game.choose(0).unwrap();
    let mut suffix = Vec::new();
    assert_eq!(settle(&mut game, &mut suffix), replaced());
    assert_eq!(
        suffix.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [payment, reward, reward]
    );
    assert_eq!(game.position().cards(Zone::Score(A)), &[reward]);
    for mut fork in [after_meld, at_choice] {
        let mut fork_events = Vec::new();
        choose_card(&mut fork, &mut fork_events, payment);
        assert_eq!(settle(&mut fork, &mut fork_events), replaced());
        assert_eq!(fork_events, suffix);
        assert_eq!(fork, game);
    }
}

#[test]
fn self_execution_keeps_michelangelos_score_alias_live_inside_karma() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnVonNeumann, Zone::Board(A, Color::Red));
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let child = f.tech(
        Color::Blue,
        5,
        Zone::Hand(A),
        vec![build::verb(Verb::Return, build::SCORE).to(Destination::Deck)],
    );
    let payment = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(meld(child, Zone::Hand(A), A, Color::Blue), Context::new(A))
        .unwrap();
    let mut events = Vec::new();
    assert_eq!(settle(&mut game, &mut events), replaced());
    assert_eq!(
        events
            .iter()
            .map(|event| moved(event).card)
            .collect::<Vec<_>>(),
        [child, payment]
    );
    assert_eq!(moved(&events[1]).verb, Some(Verb::Return));
    assert_eq!(moved(&events[1]).source, Zone::Hand(A));
    assert!(events.iter().all(|event| moved(event).inside_karma));
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), &[payment]);
    assert!(game.position().check_conservation());
}
