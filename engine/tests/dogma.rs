use inno_engine::*;

mod common;
use common::*;

fn draw(value: u8) -> Instruction {
    build::draw(value)
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("Dogma did not terminate");
}

fn dogma(game: &mut Game, card: CardId, auspice: bool) {
    game.take_action(Action::Dogma { card, auspice }, ActionKind::Regular)
        .unwrap();
}

fn icons(f: &mut Fixture, card: CardId, icon: Icon, count: usize) {
    let symbols = &mut f.cards[usize::from(card.0)].symbols;
    *symbols = [Symbol::Blank; SLOTS];
    symbols[..count].fill(Symbol::Icon(icon));
}

fn writing_fixture() -> (Fixture, CardId, [CardId; 3]) {
    let mut f = Fixture::default();
    let writing = f.writing(Zone::Board(A, Color::Blue));
    let b_top = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    icons(&mut f, b_top, Icon::Concept, 2);
    let reward = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let a_draw = f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    let b_draw = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    (f, writing, [b_draw, a_draw, reward])
}

#[test]
fn writing_shares_in_order_and_earns_one_free_draw_before_turn_advancement() {
    let (f, writing, draws) = writing_fixture();
    let position = f.position();
    let mut base = Game::new(position.clone(), Expansions::BASE).unwrap();
    let mut figures = Game::new(position, Expansions::FIGURES).unwrap();
    base.begin_opening_turn(A).unwrap();
    figures.begin_opening_turn(A).unwrap();
    dogma(&mut base, writing, false);
    dogma(&mut figures, writing, false);
    let mut events = Vec::new();
    let mut figure_events = Vec::new();
    assert_eq!(
        finish(&mut base, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        finish(&mut figures, &mut figure_events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events, figure_events);
    assert_eq!(base.position(), figures.position());
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        draws
    );
    assert_eq!(
        events.iter().map(|e| moved(e).executor).collect::<Vec<_>>(),
        [B, A, A]
    );
    assert_eq!(base.position().turn_owner(), B);
    assert_eq!(base.position().regular_actions_started(), 0);
}

#[test]
fn demand_executor_and_demander_are_distinct_and_demands_do_not_earn_sharing() {
    let mut f = Fixture::default();
    let source = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    icons(&mut f, source, Icon::Authority, 1);
    f.cards[usize::from(source.0)].effects = vec![
        Effect {
            kind: EffectKind::Demand,
            instructions: vec![
                draw(1),
                build::verb(Verb::Transfer, build::LAST).to(Destination::Hand(Who::Demander)),
            ],
        },
        Effect {
            kind: EffectKind::NonDemand,
            instructions: vec![draw(1)],
        },
    ];
    let own_draw = f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    let demanded = f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    dogma(&mut game, source, false);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 3);
    assert_eq!(moved(&events[0]).executor, B);
    assert_eq!(moved(&events[1]).executor, B);
    assert_eq!(moved(&events[1]).source, Zone::Hand(B));
    assert_eq!(moved(&events[1]).destination, Zone::Hand(A));
    assert_eq!(moved(&events[2]).executor, A);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[demanded, own_draw]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
}

#[test]
fn shared_effects_lock_eligibility_and_programs_but_not_current_board_state() {
    let mut f = Fixture::default();
    let source = f.tech(
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        vec![
            build::verb(Verb::Score, Selector::TopOf(Who::Executor, Color::Red))
                .to(Destination::Score(Who::Executor)),
        ],
    );
    f.cards[usize::from(source.0)].effects.push(Effect {
        kind: EffectKind::NonDemand,
        instructions: vec![draw(2)],
    });
    let b_top = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
    let a_top = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![]);
    icons(&mut f, a_top, Icon::Authority, 1);
    icons(&mut f, b_top, Icon::Authority, 1); // B loses its tie in effect one.
    let reward = f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let a_draw = f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let b_draw = f.tech(Color::Purple, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    dogma(&mut game, source, false);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(
        events.iter().map(|e| moved(e).card).collect::<Vec<_>>(),
        [b_top, source, b_draw, a_draw, reward]
    );
    assert_eq!(game.position().top(A, Color::Red), None);
    assert_eq!(game.position().highest_top_value(A), 1);
    assert_eq!(game.position().basic_icon_count(A, Icon::Authority), 1);
    assert_eq!(game.position().basic_icon_count(B, Icon::Authority), 0);
    assert_eq!(game.position().cards(Zone::Score(A)), &[source]);
}

#[test]
fn reveals_and_declined_optional_effects_do_not_earn_a_free_draw() {
    for reveal in [true, false] {
        let mut f = Fixture::default();
        let instruction = if reveal {
            Instruction::Reveal {
                from: build::HAND,
                optional: false,
                all: false,
                unless_proven: None,
                keep: None,
                by: None,
            }
        } else {
            build::verb(Verb::Return, build::HAND)
                .to(Destination::Deck)
                .optional()
        };
        let source = f.tech(
            Color::Blue,
            1,
            Zone::Board(A, Color::Blue),
            vec![instruction],
        );
        f.tech(Color::Red, 1, Zone::Hand(B), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        dogma(&mut game, source, false);
        let mut events = Vec::new();
        if !reveal {
            let menu = decision(&mut game, &mut events);
            assert_eq!(menu.player, B);
            let decline = menu
                .options
                .iter()
                .position(|o| *o == Alternative::Decline)
                .unwrap();
            game.choose(decline).unwrap();
        }
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), usize::from(reveal)); // Decks empty: any reward would exhaust.
        if reveal {
            assert!(matches!(events[0], Event::Revealed { executor: B, .. }));
        }
    }
}

#[test]
fn replacement_children_and_action_end_fading_do_not_count_as_shared_activity() {
    let mut f = Fixture::default();
    let source = f.agriculture(Zone::Board(A, Color::Yellow));
    let b_top = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    icons(&mut f, b_top, Icon::Health, 4);
    f.figure(Figure::Homer, Zone::Board(B, Color::Purple));
    let tucked = f.figure(Figure::AlexanderTheGreat, Zone::Hand(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, false);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, tucked);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Tuck));
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(moved(&events[1]).verb, Some(Verb::Score));
    assert!(!moved(&events[1]).inside_karma);
}

#[test]
fn auspice_uses_avatar_counts_instead_of_the_printed_featured_icon() {
    for auspice in [false, true] {
        let mut f = Fixture::default();
        let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![draw(1)]);
        f.cards[usize::from(source.0)].featured = Some(Icon::Prosperity);
        icons(&mut f, source, Icon::Prosperity, 1);
        f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
        let b_top = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
        icons(&mut f, b_top, Icon::Prosperity, 4);
        for _ in 0..2 {
            f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        f.tech(Color::Purple, 3, Zone::Deck(Deck::base(3)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        dogma(&mut game, source, auspice);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), if auspice { 1 } else { 3 });
        assert_eq!(
            game.position().cards(Zone::Hand(B)).len(),
            usize::from(!auspice)
        );
    }
}

#[test]
fn invalid_dogma_and_auspice_requests_do_not_consume_an_action() {
    let mut f = Fixture::default();
    let covered = f.writing(Zone::Board(A, Color::Blue));
    let top = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    let foreign = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    let figure = f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    let before = game.clone();
    for (card, auspice) in [
        (covered, false),
        (foreign, false),
        (figure, false),
        (CardId(u16::MAX), false),
        (top, true),
    ] {
        assert_eq!(
            game.take_action(Action::Dogma { card, auspice }, ActionKind::Regular),
            Err(Error::InvalidAction)
        );
        assert_eq!(game, before);
    }
}

fn homer_fixture(same_color: bool) -> (Fixture, CardId, CardId, CardId) {
    let mut f = Fixture::default();
    let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![draw(1)]);
    let homer = f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]); // Sharing reward if Homer leaves.
    let tucked = f.tech(
        if same_color {
            Color::Blue
        } else {
            Color::Purple
        },
        2,
        Zone::Deck(Deck::base(2)),
        vec![],
    );
    for _ in 0..2 {
        f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    (f, source, homer, tucked)
}

#[test]
fn homer_first_finishes_before_the_sharing_snapshot_and_retains_auspice_permission() {
    for auspice in [false, true] {
        let (f, source, homer, tucked) = homer_fixture(false);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        dogma(&mut game, source, auspice); // Homer grants Authority Auspice before leaving.
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(events.len(), 6);
        assert_eq!(moved(&events[0]).card, tucked);
        assert_eq!(moved(&events[1]).verb, Some(Verb::Tuck));
        assert_eq!(moved(&events[2]).card, homer);
        assert_eq!(moved(&events[2]).verb, Some(Verb::Return));
        assert!(events[..3].iter().all(|e| moved(e).inside_karma));
        assert!(events[3..].iter().all(|e| !moved(e).inside_karma));
        assert_eq!(moved(&events[3]).executor, B); // Now tied at zero: snapshot is after First.
        assert_eq!(game.position().top(A, Color::Purple), Some(tucked));
    }
}

#[test]
fn homer_can_return_the_activated_card_but_does_not_trigger_on_self_execution() {
    let (f, source, homer, tucked) = homer_fixture(true);
    let position = f.position();
    let mut game = Game::new(position.clone(), Expansions::FIGURES).unwrap();
    let mut effect = Game::new(position, Expansions::FIGURES).unwrap();
    dogma(&mut game, source, false);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4); // Draw, tuck, return source, original body draw.
    assert_eq!(game.position().top(A, Color::Purple), Some(homer));
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        &[tucked]
    );
    assert_eq!(moved(&events[2]).card, source);
    assert_eq!(moved(&events[2]).verb, Some(Verb::Return));
    assert_eq!(moved(&events[3]).executor, A);
    effect.self_execute(source, Context::new(A)).unwrap();
    let mut effect_events = Vec::new();
    assert_eq!(
        finish(&mut effect, &mut effect_events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(effect_events.len(), 1);
    assert!(!moved(&effect_events[0]).inside_karma);
}

#[test]
fn homer_leaves_the_drawn_card_when_it_is_itself_the_new_top() {
    let (mut f, source, _, tucked) = homer_fixture(false);
    f.cards[usize::from(tucked.0)].color = Color::Green;
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, false);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 3);
    assert_eq!(game.position().top(A, Color::Green), Some(tucked));
    assert!(events.iter().all(|e| moved(e).verb != Some(Verb::Return)));
}

#[test]
fn alexander_first_scores_the_actual_draw_and_himself_only_if_age_skipped() {
    for age in [2, 3] {
        let mut f = Fixture::default();
        let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![draw(1)]);
        let alexander = f.figure(Figure::AlexanderTheGreat, Zone::Board(A, Color::Red));
        let scored = f.tech(Color::Green, age, Zone::Deck(Deck::base(age)), vec![]);
        for _ in 0..3 {
            f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice: false,
            },
            ActionKind::Free,
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(moved(&events[0]).card, scored);
        assert_eq!(moved(&events[1]).card, scored);
        assert_eq!(moved(&events[1]).destination, Zone::Score(A));
        if age == 3 {
            assert_eq!(events.len(), 6);
            assert_eq!(moved(&events[2]).card, alexander);
            assert!(moved(&events[2]).inside_karma);
            assert_eq!(moved(&events[3]).executor, B);
        } else {
            assert_eq!(events.len(), 3);
            assert_eq!(game.position().top(A, Color::Red), Some(alexander));
        }
        assert_eq!(game.position().regular_actions_started(), 0);
        assert_eq!(game.position().turn_owner(), A);
    }
}

#[test]
fn current_player_selects_only_one_competing_dogma_first_reaction() {
    for selected in [Figure::Homer, Figure::AlexanderTheGreat] {
        let (mut f, source, _, drawn) = homer_fixture(true);
        f.figure(Figure::AlexanderTheGreat, Zone::Board(A, Color::Red));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(
            Operation::Dogma {
                card: source,
                icon: Icon::Authority,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, A);
        assert_eq!(menu.options.len(), 2);
        let choice = menu
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Karma { figure, .. } if *figure == selected))
            .unwrap();
        game.choose(choice).unwrap();
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Dogma { shared: false })
        );
        assert_eq!(events.len(), if selected == Figure::Homer { 4 } else { 3 });
        assert_eq!(moved(&events[0]).card, drawn);
        assert_eq!(
            moved(&events[1]).verb,
            Some(if selected == Figure::Homer {
                Verb::Tuck
            } else {
                Verb::Score
            })
        );
    }
}

#[test]
fn terminal_first_reaction_aborts_the_body_and_preserves_turn_ownership() {
    let mut f = Fixture::default();
    let source = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![draw(1)]);
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let body_draw = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Green, 8, Zone::Score(B), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    dogma(&mut game, source, false);
    let mut events = Vec::new();
    assert_eq!(finish(&mut game, &mut events), Status::Winner(B));
    assert!(events.is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        &[body_draw]
    );
    assert_eq!(game.position().turn_owner(), A);
}

fn fading_fixture() -> (Fixture, CardId, CardId) {
    let (mut f, writing, _) = writing_fixture();
    let michelangelo = f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    f.tech(Color::Red, 3, Zone::Deck(Deck::base(3)), vec![]);
    f.tech(Color::Purple, 4, Zone::Deck(Deck::base(4)), vec![]);
    (f, writing, michelangelo)
}

#[test]
fn sharing_free_draw_occurs_after_fading_and_uses_the_new_highest_top_value() {
    let (f, writing, michelangelo) = fading_fixture();
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_opening_turn(A).unwrap();
    dogma(&mut game, writing, false);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, michelangelo);
    assert_eq!(events.len(), 2);
    assert_eq!(game.position().turn_owner(), A);
    assert_eq!(game.position().regular_actions_started(), 1);
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 4);
    assert_eq!(moved(&events[2]).card, michelangelo);
    assert_eq!(moved(&events[3]).source, Zone::Deck(Deck::base(3)));
    assert_eq!(game.position().turn_owner(), B);
}

#[test]
fn figures_sharing_reward_draws_a_figure_and_achievement_victory_preempts_it() {
    for terminal in [false, true] {
        let (mut f, writing, _) = writing_fixture();
        let figure = f.figure(Figure::Homer, Zone::Deck(Deck::figures(1)));
        if terminal {
            for _ in 0..7 {
                f.tech(Color::Red, 2, Zone::Achievements(A), vec![]);
            }
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        dogma(&mut game, writing, false);
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events),
            if terminal {
                Status::Winner(A)
            } else {
                Status::Complete(ReturnValue::Unit)
            }
        );
        assert_eq!(events.len(), if terminal { 2 } else { 3 });
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&figure),
            !terminal
        );
        assert_eq!(game.position().regular_actions_started(), 1);
    }
}

#[test]
fn empty_board_draws_age_zero_when_introduced_instead_of_forcing_age_one() {
    let mut f = Fixture::default();
    let zero = f.tech(Color::Green, 0, Zone::Deck(Deck::base(0)), vec![]);
    f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[zero]);
}

#[test]
fn cloning_at_every_dogma_boundary_retains_program_sharing_and_free_action_progress() {
    let (f, writing, _) = fading_fixture();
    let mut game = Game::seeded(f.position(), Expansions::FIGURES, 42).unwrap();
    game.begin_opening_turn(A).unwrap();
    dogma(&mut game, writing, false);
    let mut expected = game.clone();
    let mut expected_events = Vec::new();
    let result = finish(&mut expected, &mut expected_events);
    let mut events = Vec::new();
    for _ in 0..300 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix), result);
        assert_eq!(fork, expected);
        assert_eq!(
            events.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            expected_events
        );
        match game.advance(1, &mut events).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, result);
                assert_eq!(game, expected);
                return;
            }
        }
        assert!(game.position().check_conservation());
    }
    panic!("Dogma did not terminate");
}

#[test]
fn auspice_is_offered_only_when_the_icon_can_change_play() {
    // Maxwell prints one Avatar and one Efficiency; the source adds a second
    // Efficiency. With an empty opposing board neither icon shares.
    let offers = |featured: Icon, rival: usize, sun_tzu: bool| {
        let mut f = Fixture::default();
        f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
        let source = f.tech(Color::Green, 1, Zone::Board(A, Color::Green), vec![draw(1)]);
        icons(&mut f, source, Icon::Efficiency, 1);
        f.cards[usize::from(source.0)].featured = Some(featured);
        if rival > 0 {
            let card = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
            icons(&mut f, card, Icon::Efficiency, rival);
        }
        if sun_tzu {
            // Sun Tzu's share draw melds by the featured icon.
            f.figure(Figure::SunTzu, Zone::Board(A, Color::Red));
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        let auspice = Action::Dogma {
            card: source,
            auspice: true,
        };
        let offered = game.legal_actions().contains(&auspice);
        let taken = game.take_action(auspice, ActionKind::Regular);
        assert_eq!(taken.is_ok(), offered);
        offered
    };
    assert!(!offers(Icon::Efficiency, 0, false));
    // The opponent shares Efficiency but not Avatar.
    assert!(offers(Icon::Efficiency, 2, false));
    assert!(offers(Icon::Efficiency, 0, true));
    // Auspice would replace Avatar with Avatar.
    assert!(!offers(Icon::Avatar, 0, true));
}

#[test]
fn auspice_is_not_offered_for_a_dogma_karma_that_cannot_fire_or_use_the_icon() {
    // A's only top Figure decides whether Auspice is offered; B's optional
    // Figure watches scoring. B never shares: A prints more of the featured
    // icon and of Avatar.
    let offers = |figure: Figure, watcher: Option<Figure>, second_action: bool| {
        let mut f = Fixture::default();
        let color = figure_card(figure.code()).unwrap().color;
        f.figure(figure, Zone::Board(A, color));
        if let Some(watcher) = watcher {
            let color = figure_card(watcher.code()).unwrap().color;
            f.figure(watcher, Zone::Board(B, color));
        }
        let free = Color::ALL.into_iter().find(|&free| free != color).unwrap();
        // Auspice needs a top Figure printing the featured icon.
        let featured = figure_card(figure.code())
            .unwrap()
            .symbols
            .iter()
            .find_map(|symbol| match symbol {
                Symbol::Icon(icon) if *icon != Icon::Avatar => Some(*icon),
                _ => None,
            })
            .unwrap();
        let source = f.tech(free, 1, Zone::Board(A, free), vec![draw(1)]);
        let symbols = &mut f.cards[usize::from(source.0)].symbols;
        *symbols = [Symbol::Blank; SLOTS];
        symbols[0] = Symbol::Icon(featured);
        symbols[1] = Symbol::Icon(Icon::Avatar);
        f.cards[usize::from(source.0)].featured = Some(featured);
        // A first Draw action climbs from the Figure's age to this deck.
        f.tech(Color::Yellow, 10, Zone::Deck(Deck::base(10)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if second_action {
            game.take_action(Action::Draw, ActionKind::Regular).unwrap();
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(game.position().regular_actions_started(), 1);
        }
        let auspice = Action::Dogma {
            card: source,
            auspice: true,
        };
        let offered = game.legal_actions().contains(&auspice);
        let taken = game.take_action(auspice, ActionKind::Regular);
        assert_eq!(taken.is_ok(), offered);
        offered
    };
    // Amina Sukhera scores a Figure from the opponent's hand before a
    // second-action Dogma: no icon is read and no board changes.
    assert!(!offers(Figure::AminaSukhera, None, false));
    assert!(!offers(Figure::AminaSukhera, None, true));
    // Unless a top Figure watches scoring (Cai Lun) and could change a board.
    assert!(offers(Figure::AminaSukhera, Some(Figure::CaiLun), true));
    assert!(!offers(Figure::AminaSukhera, Some(Figure::CaiLun), false));
    // Plato splays before a first-action Dogma only.
    assert!(offers(Figure::Plato, None, false));
    assert!(!offers(Figure::Plato, None, true));
}
