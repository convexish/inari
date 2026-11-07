use inno_engine::*;

mod common;
use common::*;

fn native(f: &mut Fixture, figure: Figure, owner: Player) -> CardId {
    let card = figure_card(figure.code()).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(owner, color))
}

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}

fn draw(value: u8) -> Instruction {
    build::draw(value)
}

// Explicit synthetic original: its unmatched Authority prevents accidental
// sharing and its draw is a sentinel for whether the Dogma was replaced.
fn original(f: &mut Fixture, owner: Player) -> CardId {
    let card = f.tech(Color::Red, 3, Zone::Board(owner, Color::Red), vec![draw(1)]);
    f.cards[usize::from(card.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    card
}

fn dogma(game: &mut Game, card: CardId, kind: ActionKind) {
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        kind,
    )
    .unwrap();
}

fn pick(menu: &Decision, preferred: Option<Figure>) -> usize {
    menu.options
        .iter()
        .position(|option| {
            matches!(option,
                Alternative::Karma { figure, .. } if Some(*figure) == preferred
            )
        })
        .unwrap_or(0)
}

fn finish(game: &mut Game, events: &mut Vec<Event>, preferred: Option<Figure>) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => game.choose(pick(&menu, preferred)).unwrap(),
            status => return status,
        }
    }
    panic!("copied Figure action did not terminate");
}

// Clone every work boundary, including menus, nested copies, and action end.
fn clone_boundaries(mut game: Game, preferred: Option<Figure>) -> (Game, Vec<Event>, Status) {
    let mut expected = game.clone();
    let mut all = Vec::new();
    let status = finish(&mut expected, &mut all, preferred);
    let mut prefix = Vec::new();
    for _ in 0..1000 {
        let mut fork = game.clone();
        let mut suffix = Vec::new();
        assert_eq!(finish(&mut fork, &mut suffix, preferred), status);
        assert_eq!(fork, expected);
        assert_eq!(
            prefix.iter().chain(&suffix).cloned().collect::<Vec<_>>(),
            all
        );
        assert!(game.position().check_conservation());
        match game.advance(1, &mut prefix).unwrap() {
            Status::WorkLimit => {}
            Status::Decision(menu) => game.choose(pick(&menu, preferred)).unwrap(),
            end => {
                assert_eq!(end, status);
                return (expected, all, status);
            }
        }
    }
    panic!("clone walk did not terminate");
}

#[test]
fn ptahhotep_scores_original_then_super_executes_native_demand_without_sharing_or_nobel() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Ptahhotep, A);
    native(&mut f, Figure::AlfredNobel, B);
    let source = original(&mut f, A);
    let gunpowder = base(&mut f, "B037", Zone::Hand(A));
    let payment = base(&mut f, "B024", Zone::Board(B, Color::Purple));
    let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let untouched = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let seven = base(&mut f, "B075", Zone::Deck(Deck::base(7)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        events[1],
        Event::Revealed {
            card: gunpowder,
            source: Zone::Score(A),
            executor: A,
            inside_karma: true
        }
    );
    assert_eq!(
        events[..1]
            .iter()
            .chain(&events[2..])
            .map(|event| {
                let m = moved(event);
                (m.card, m.executor, m.verb, m.inside_karma)
            })
            .collect::<Vec<_>>(),
        [
            (gunpowder, A, Some(Verb::Score), true),
            (payment, B, Some(Verb::Transfer), true),
            (reward, A, None, true),
            (reward, A, Some(Verb::Score), true),
        ]
    );
    assert_eq!(
        game.position().cards(Zone::Score(A)),
        &[gunpowder, payment, reward]
    );
    assert!(game.position().cards(Zone::Score(B)).is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        &[untouched]
    );
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), &[seven]);
    assert_eq!(game.position().turn_owner(), B);
}

#[test]
fn regular_ordinal_owner_and_free_scope_do_not_borrow_replacements() {
    for figure in [Figure::Ptahhotep, Figure::Plato, Figure::Avicenna] {
        let trigger = if figure == Figure::Ptahhotep { 2 } else { 1 };
        for (owner, kind, previous) in [
            (A, ActionKind::Regular, if trigger == 2 { 0 } else { 1 }),
            (A, ActionKind::Free, 0),
            (A, ActionKind::Free, 1),
            (A, ActionKind::Free, 2),
            (B, ActionKind::Regular, trigger - 1),
        ] {
            let mut f = Fixture::default();
            native(&mut f, figure, A);
            let source = original(&mut f, owner);
            let untouched = base(&mut f, "B012", Zone::Hand(A));
            let achievement = base(&mut f, "B021", Zone::AvailableAchievements);
            let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
            let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
            game.begin_turn(owner).unwrap();
            for _ in 0..previous {
                game.begin_action(ActionKind::Regular).unwrap();
            }
            dogma(&mut game, source, kind);
            let mut events = Vec::new();
            assert_eq!(
                finish(&mut game, &mut events, None),
                Status::Complete(ReturnValue::Unit)
            );
            assert_eq!(events.len(), 1);
            assert_eq!(moved(&events[0]).card, reward);
            assert_eq!(moved(&events[0]).executor, owner);
            assert!(!moved(&events[0]).inside_karma);
            assert!(game.position().cards(Zone::Hand(A)).contains(&untouched));
            assert_eq!(
                game.position().cards(Zone::AvailableAchievements),
                &[achievement]
            );
        }
    }
}

#[test]
fn idle_operation_is_not_a_regular_action_even_when_clock_matches() {
    for (figure, ordinal) in [
        (Figure::Ptahhotep, 2),
        (Figure::Plato, 1),
        (Figure::Avicenna, 1),
    ] {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        let source = original(&mut f, A);
        let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        for _ in 0..ordinal {
            game.begin_action(ActionKind::Regular).unwrap();
        }
        game.start(
            Operation::Dogma {
                card: source,
                icon: Icon::Authority,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        assert!(matches!(
            finish(&mut game, &mut events, None),
            Status::Complete(_)
        ));
        assert_eq!(events.len(), 1);
        assert_eq!(moved(&events[0]).card, reward);
        assert!(!moved(&events[0]).inside_karma);
    }
}

#[test]
fn empty_ptahhotep_and_avicenna_still_replace_the_original_without_optional_decline() {
    for figure in [Figure::Ptahhotep, Figure::Avicenna] {
        let mut f = Fixture::default();
        native(&mut f, figure, A);
        let source = original(&mut f, A);
        let untouched = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if figure == Figure::Ptahhotep {
            game.begin_action(ActionKind::Regular).unwrap();
        }
        dogma(&mut game, source, ActionKind::Regular);
        let (game, events, status) = clone_boundaries(game, None);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert!(events.is_empty());
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(1))),
            &[untouched]
        );
    }
}

#[test]
fn ptahhotep_mandatory_logical_hand_menu_excludes_opponents_cards() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Ptahhotep, A);
    let source = original(&mut f, A);
    let writing = base(&mut f, "B012", Zone::Hand(A));
    let other = base(&mut f, "B031", Zone::Hand(A));
    let borrowed = base(&mut f, "B008", Zone::Hand(B));
    let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(
        !game
            .position()
            .targets(LogicalZone::Hand(A))
            .any(|t| t.card == borrowed)
    );
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(menu.options.len(), 2);
    for card in [writing, other] {
        assert!(menu.options.contains(&Alternative::Move {
            verb: Verb::Score,
            target: TargetDescriptor::Known {
                card,
                source: Zone::Hand(A)
            },
        }));
    }
    choose_card(&mut game, &mut events, writing);
    let (game, suffix, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(moved(&suffix[0]).card, writing);
    assert!(game.position().cards(Zone::Hand(A)).contains(&reward));
    assert_eq!(game.position().cards(Zone::Hand(B)), &[borrowed]);
}

#[test]
fn plato_splays_selected_color_or_junks_only_hidden_age_two_then_runs_original() {
    for (stack_size, already_left) in [(2, false), (1, false), (2, true)] {
        let mut f = Fixture::default();
        native(&mut f, Figure::Plato, A);
        if stack_size == 2 {
            f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
        }
        let source = original(&mut f, A);
        let first = base(&mut f, "B021", Zone::AvailableAchievements);
        let second = base(&mut f, "B024", Zone::AvailableAchievements);
        let other = base(&mut f, "B004", Zone::AvailableAchievements);
        let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        if already_left {
            game.start(
                Operation::Splay {
                    player: A,
                    color: Color::Red,
                    direction: Splay::Left,
                },
                Context::new(A),
            )
            .unwrap();
            settle(&mut game, &mut Vec::new());
        }
        dogma(&mut game, source, ActionKind::Regular);
        // The two tied [2] backs are one native sample, never a menu.
        assert!(!matches!(
            settle(&mut game.clone(), &mut Vec::new()),
            Status::Decision(_)
        ));
        let (game, events, status) = clone_boundaries(game, None);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
        let junk = game.position().cards(Zone::Junk);
        let supply = game.position().cards(Zone::AvailableAchievements);
        if stack_size == 1 || already_left {
            assert_eq!(junk.len(), 1);
            assert!([first, second].contains(&junk[0]));
            let kept = if junk[0] == first { second } else { first };
            assert!(supply.contains(&kept));
        } else {
            assert!(junk.is_empty());
            assert!(supply.contains(&first) && supply.contains(&second));
        }
        assert!(
            game.position()
                .cards(Zone::AvailableAchievements)
                .contains(&other)
        );
        let movement = events
            .iter()
            .find_map(|e| match e {
                Event::Moved(m) if m.card == reward => Some(m),
                _ => None,
            })
            .unwrap();
        assert!(!movement.inside_karma);
        if stack_size == 2 {
            assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
        }
    }
}

#[test]
fn plato_can_junk_the_selected_live_board_alias_without_aborting_original_dogma() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Plato, A);
    native(&mut f, Figure::CaiLun, A);
    let source = f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![draw(1)]);
    f.cards[usize::from(source.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    // Keep ordinary sharing out of this source-removal proof after the selected
    // red top leaves play; participant icons are correctly queried afterward.
    let support = f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.cards[usize::from(support.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.contains(&Alternative::Move {
        verb: Verb::Junk,
        target: TargetDescriptor::Known {
            card: source,
            source: Zone::Board(A, Color::Red)
        }
    }));
    choose_card(&mut game, &mut events, source);
    let (game, suffix, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(game.position().cards(Zone::Junk).contains(&source));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
    assert!(
        suffix
            .iter()
            .any(|e| matches!(e, Event::Moved(m) if m.card == reward && !m.inside_karma))
    );
}

#[test]
fn avicenna_samples_one_lowest_hidden_tie_and_executes_that_identity() {
    let mut sampled = [false; 3];
    for seed in 0..16 {
        let mut f = Fixture::default();
        native(&mut f, Figure::Avicenna, A);
        let source = original(&mut f, A);
        // Three tied [2] backs; each draws a distinct sentinel value.
        let ties = [4, 5, 6].map(|value| {
            f.tech(
                Color::Green,
                2,
                Zone::AvailableAchievements,
                vec![draw(value)],
            )
        });
        let higher = base(&mut f, "B037", Zone::AvailableAchievements);
        let untouched = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let rewards = [4, 5, 6]
            .map(|value| f.tech(Color::Yellow, value, Zone::Deck(Deck::base(value)), vec![]));
        let mut game = Game::seeded(f.position(), Expansions::FIGURES, seed).unwrap();
        dogma(&mut game, source, ActionKind::Regular);
        // A cannot tell the tied backs apart: no menu, one native sample.
        assert!(!matches!(
            settle(&mut game.clone(), &mut Vec::new()),
            Status::Decision(_)
        ));
        let (game, suffix, status) = clone_boundaries(game, None);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let junk = game.position().cards(Zone::Junk);
        assert_eq!(junk.len(), 1);
        let index = ties.iter().position(|&tie| tie == junk[0]).unwrap();
        sampled[index] = true;
        // The junked identity is the one super-executed.
        assert_eq!(game.position().cards(Zone::Hand(A)), &[rewards[index]]);
        let mut supply: Vec<_> = ties.into_iter().filter(|&tie| tie != junk[0]).collect();
        supply.push(higher);
        assert_eq!(game.position().cards(Zone::AvailableAchievements), supply);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::base(1))),
            &[untouched]
        );
        assert!(
            suffix.iter().any(
                |e| matches!(e, Event::Moved(m) if m.card == rewards[index] && m.inside_karma)
            )
        );
    }
    assert_eq!(sampled, [true; 3]);
}

#[test]
fn avicenna_junks_and_super_executes_hidden_native_writing_without_a_share() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Avicenna, A);
    let source = original(&mut f, A);
    let writing = base(&mut f, "B012", Zone::AvailableAchievements);
    let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    // Opponent's abundant Concepts do not turn a super-execution into a share.
    native(&mut f, Figure::JamesClerkMaxwell, B);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[1],
        Event::Revealed {
            card: writing,
            source: Zone::Junk,
            executor: A,
            inside_karma: true
        }
    );
    let events = [events[0].clone(), events[2].clone()];
    assert_eq!(moved(&events[0]).card, writing);
    assert_eq!(moved(&events[0]).verb, Some(Verb::Junk));
    assert_eq!(moved(&events[1]).card, reward);
    assert!(events.iter().all(|event| moved(event).inside_karma));
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
}

#[test]
fn wells_uses_actual_featured_icon_not_visible_icons_and_not_auspice() {
    for (featured, auspice, kind, owner) in [
        (Icon::Concept, false, ActionKind::Regular, A),
        (Icon::Concept, false, ActionKind::Free, A),
        (Icon::Concept, true, ActionKind::Free, A),
        (Icon::Authority, false, ActionKind::Free, A),
        (Icon::Concept, false, ActionKind::Regular, B),
    ] {
        let mut f = Fixture::default();
        native(&mut f, Figure::HGWells, A);
        if auspice {
            // Wells has no printed Concept; Antonie's natural icons allow Auspice.
            native(&mut f, Figure::AntonieVanLeeuwenhoek, A);
        }
        let source = original(&mut f, owner);
        f.cards[usize::from(source.0)].featured = Some(featured);
        // Deliberately mismatched spatial icons on the explicit synthetic Base.
        f.cards[usize::from(source.0)].symbols = four_slots(
            [Symbol::Icon(if featured == Icon::Concept {
                Icon::Authority
            } else {
                Icon::Concept
            }); 4],
        );
        if owner == B {
            f.cards[usize::from(source.0)].symbols = four_slots([Symbol::Icon(Icon::Concept); 4]);
        }
        let copied = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![draw(2)]);
        let original_reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let copy_reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
        // On the Authority non-trigger, A has zero featured icons and B shares.
        let spare = base(&mut f, "B008", Zone::Deck(Deck::base(1)));
        let figure_reward = f.figure(Figure::JackieChan, Zone::Deck(Deck::figures(10)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.begin_turn(owner).unwrap();
        game.take_action(
            Action::Dogma {
                card: source,
                auspice,
            },
            kind,
        )
        .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            finish(&mut game, &mut events, None),
            Status::Complete(ReturnValue::Unit)
        );
        let replaced = featured == Icon::Concept && !auspice && owner == A;
        assert_eq!(
            game.position().cards(Zone::Junk).contains(&copied),
            replaced
        );
        if replaced {
            assert_eq!(game.position().cards(Zone::Hand(A)), &[copy_reward]);
            assert_eq!(
                events[2],
                Event::Revealed {
                    card: copied,
                    source: Zone::Junk,
                    executor: A,
                    inside_karma: true
                }
            );
            assert!(
                events[..2]
                    .iter()
                    .chain(&events[3..])
                    .all(|event| moved(event).inside_karma)
            );
            assert_eq!(
                game.position().cards(Zone::Deck(Deck::base(1))),
                &[original_reward, spare]
            );
            assert_eq!(
                game.position().cards(Zone::Deck(Deck::figures(10))),
                &[figure_reward]
            );
        } else {
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Event::Moved(m) if m.card == copied))
            );
            assert!(events.iter().any(
                |event| matches!(event, Event::Moved(m) if m.card == spare && !m.inside_karma)
            ));
        }
    }
}

#[test]
fn wells_native_robotics_internet_keeps_chain_and_continues_after_covering_wells() {
    let mut f = Fixture::default();
    let wells = native(&mut f, Figure::HGWells, A);
    let writing = base(&mut f, "B012", Zone::Board(A, Color::Blue));
    let internet = base(&mut f, "B105", Zone::Deck(Deck::base(10)));
    let robotics = base(&mut f, "B097", Zone::Deck(Deck::base(10)));
    // Clearly synthetic age-11 supply: Chain plus Internet's three routed draws.
    let second = f.tech(Color::Red, 11, Zone::Deck(Deck::base(11)), vec![]);
    let first = f.tech(Color::Blue, 11, Zone::Deck(Deck::base(11)), vec![]);
    let scored = f.tech(Color::Yellow, 11, Zone::Deck(Deck::base(11)), vec![]);
    let award = f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, writing, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        events[2],
        Event::Revealed {
            card: robotics,
            source: Zone::Junk,
            executor: A,
            inside_karma: true
        }
    );
    let events: Vec<_> = events[..2].iter().chain(&events[3..]).cloned().collect();
    assert_eq!(
        events
            .iter()
            .map(|e| (moved(e).verb, moved(e).card))
            .collect::<Vec<_>>(),
        [
            (None, robotics),
            (Some(Verb::Junk), robotics),
            (None, internet),
            (Some(Verb::Meld), internet),
            (None, award),
            (Some(Verb::Achieve), award),
            (None, scored),
            (Some(Verb::Score), scored),
            (None, first),
            (Some(Verb::Meld), first),
            (None, second),
            (Some(Verb::Meld), second),
        ]
    );
    assert!(events.iter().all(|e| moved(e).inside_karma));
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[award]);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Purple)),
        &[wells, internet]
    );
    assert_eq!(game.position().cards(Zone::Junk), &[robotics]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[scored]);
}

#[test]
fn exhausted_wells_child_abandons_copy_original_and_action_tail() {
    let mut f = Fixture::default();
    native(&mut f, Figure::HGWells, A);
    native(&mut f, Figure::Gilgamesh, A);
    let writing = base(&mut f, "B012", Zone::Board(A, Color::Blue));
    let untouched = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, writing, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert!(matches!(status, Status::Winner(_) | Status::Draw(_)));
    assert!(events.is_empty());
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[untouched]
    );
    assert_eq!(game.position().top_figures(A).count(), 2);
}

#[test]
fn instead_retains_unsuppressed_action_end_fading_and_first_competition_is_single_choice() {
    for preferred in [Figure::Avicenna, Figure::Plato] {
        let mut f = Fixture::default();
        let avicenna = native(&mut f, Figure::Avicenna, A);
        let plato = native(&mut f, Figure::Plato, A);
        let source = original(&mut f, A);
        let writing = base(&mut f, "B012", Zone::AvailableAchievements);
        let achievement = base(&mut f, "B021", Zone::AvailableAchievements);
        let original_reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let copy_reward = base(&mut f, "B024", Zone::Deck(Deck::base(2)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        dogma(&mut game, source, ActionKind::Regular);
        let mut prefix = Vec::new();
        let menu = decision(&mut game, &mut prefix);
        assert_eq!(menu.player, A);
        assert_eq!(menu.options.len(), 2);
        assert!(
            menu.options
                .iter()
                .all(|o| matches!(o, Alternative::Karma { .. }))
        );
        let (game, events, status) = clone_boundaries(game, Some(preferred));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let chosen = if preferred == Figure::Avicenna {
            writing
        } else {
            achievement
        };
        assert_eq!(game.position().cards(Zone::Junk), &[chosen]);
        let reward = if preferred == Figure::Avicenna {
            copy_reward
        } else {
            original_reward
        };
        assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
        let fade = events
            .iter()
            .filter_map(|e| match e {
                Event::Moved(m) if m.card == avicenna || m.card == plato => Some(m),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fade.len(), 1);
        assert_eq!(fade[0].verb, Some(Verb::Score));
        assert!(!fade[0].inside_karma);
        assert_eq!(game.position().top_figures(A).count(), 1);
    }
}

#[test]
fn native_any_two_decrees_require_live_owner_and_complete_payment_and_effect() {
    for (figure, decree) in [
        (Figure::Plato, Decree::Rivalry),
        (Figure::Avicenna, Decree::Expansion),
    ] {
        for covered in [false, true] {
            for count in 1..=2 {
                let mut f = Fixture::default();
                let native = native(&mut f, figure, A);
                if covered {
                    let color = f.cards[usize::from(native.0)].color;
                    f.tech(color, 1, Zone::Board(A, color), vec![]);
                }
                f.figure(Figure::Shennong, Zone::Hand(A));
                if count == 2 {
                    f.figure(Figure::Tigernmas, Zone::Hand(A));
                }
                let first = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
                let second = f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
                let rival_score = base(&mut f, "B004", Zone::Score(B));
                let rival_figure = f.figure(Figure::Tigernmas, Zone::Board(B, Color::Red));
                let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
                if covered || count == 1 {
                    let before = game.clone();
                    assert_eq!(
                        game.take_action(Action::Decree, ActionKind::Free),
                        Err(Error::InvalidAction)
                    );
                    assert_eq!(game, before);
                } else {
                    game.take_action(Action::Decree, ActionKind::Free).unwrap();
                    let (game, _, status) = clone_boundaries(game, None);
                    assert_eq!(status, Status::Complete(ReturnValue::Unit));
                    assert!(game.position().cards(Zone::Hand(A)).is_empty());
                    assert_eq!(
                        game.position().decree_location(decree),
                        DecreeLocation::Owned(A)
                    );
                    if decree == Decree::Expansion {
                        assert_eq!(game.position().splay(A, Color::Red), Splay::Up);
                        assert_eq!(
                            game.position().cards(Zone::Board(A, Color::Red)),
                            &[first, second]
                        );
                    } else {
                        assert!(game.position().cards(Zone::Score(B)).is_empty());
                        assert_eq!(
                            game.position().cards(Zone::Deck(Deck::base(1))),
                            &[rival_score]
                        );
                        assert!(
                            game.position()
                                .cards(Zone::Deck(Deck::figures(1)))
                                .contains(&rival_figure)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn ptahhotep_foreign_dogma_keeps_the_scored_cards_owner_and_demand_controller() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Ptahhotep, A);
    native(&mut f, Figure::SergeyBrin, A);
    let foreign = base(&mut f, "B012", Zone::Board(B, Color::Blue));
    let gunpowder = base(&mut f, "B037", Zone::Hand(A));
    base(&mut f, "B005", Zone::Hand(A));
    let payment = base(&mut f, "B024", Zone::Board(B, Color::Purple));
    let unselected = base(&mut f, "B021", Zone::Board(B, Color::Green));
    let reward = base(&mut f, "B022", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, foreign, ActionKind::Regular);
    let mut events = Vec::new();
    let hooks = decision(&mut game, &mut events);
    game.choose(pick(&hooks, Some(Figure::Ptahhotep))).unwrap();
    let score = decision(&mut game, &mut events);
    assert_eq!(score.player, A);
    choose_card(&mut game, &mut events, gunpowder);
    let demand = decision(&mut game, &mut events);
    assert_eq!(demand.player, B);
    assert_eq!(demand.options.len(), 2);
    assert!(demand.options.contains(&Alternative::Move {
        verb: Verb::Transfer,
        target: TargetDescriptor::Known {
            card: payment,
            source: Zone::Board(B, Color::Purple)
        },
    }));
    assert!(demand.options.contains(&Alternative::Move {
        verb: Verb::Transfer,
        target: TargetDescriptor::Known {
            card: unselected,
            source: Zone::Board(B, Color::Green)
        },
    }));
    choose_card(&mut game, &mut events, payment);
    let (game, suffix, status) = clone_boundaries(game, Some(Figure::Ptahhotep));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(game.position().cards(Zone::Score(A)).contains(&gunpowder));
    assert!(game.position().cards(Zone::Score(A)).contains(&payment));
    assert!(game.position().cards(Zone::Score(A)).contains(&reward));
    assert!(game.position().cards(Zone::Hand(B)).is_empty());
    assert_eq!(moved(&suffix[0]).executor, B);
    assert_eq!(moved(&suffix[0]).destination, Zone::Score(A));
    assert_eq!(game.position().top(B, Color::Blue), Some(foreign));
}

#[test]
fn avicenna_copies_ignore_archimedes_but_keep_dukes_unsuppressed_action_end() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Avicenna, A);
    native(&mut f, Figure::Archimedes, A);
    native(&mut f, Figure::DukeEllington, A);
    let source = original(&mut f, A);
    let writing = base(&mut f, "B012", Zone::AvailableAchievements);
    let reward = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let untouched = base(&mut f, "B031", Zone::Deck(Deck::base(3)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().cards(Zone::Junk), &[writing]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(3))),
        &[untouched]
    );
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[1],
        Event::Revealed {
            card: writing,
            source: Zone::Junk,
            executor: A,
            inside_karma: true
        }
    );
    assert_eq!(moved(&events[0]).card, writing);
    assert_eq!(moved(&events[2]).card, reward);
    // An action end still suppressed by the replacement would score Figures.
    assert_eq!(game.position().top_figures(A).count(), 3);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
}

#[test]
fn wells_competes_with_kepler_without_inheriting_its_box_or_action_end_scope() {
    let mut f = Fixture::default();
    native(&mut f, Figure::HGWells, A);
    let kepler = native(&mut f, Figure::JohannesKepler, A);
    let source = original(&mut f, A);
    f.cards[usize::from(source.0)].featured = Some(Icon::Concept);
    let copied = f.tech(Color::Green, 10, Zone::Deck(Deck::base(10)), vec![draw(1)]);
    let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, Some(Figure::HGWells));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().cards(Zone::Junk), &[copied]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
    assert_eq!(game.position().cards(Zone::Score(A)), &[kepler]);
    assert_eq!(events.len(), 5);
    assert_eq!(
        events[2],
        Event::Revealed {
            card: copied,
            source: Zone::Junk,
            executor: A,
            inside_karma: true
        }
    );
    assert!(
        events[..2]
            .iter()
            .chain(&events[3..4])
            .all(|e| moved(e).inside_karma)
    );
    assert!(!moved(&events[4]).inside_karma);
}

#[test]
fn platos_first_splay_suppresses_shen_kuo_instead_of_treating_a_child_as_success() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Plato, A);
    native(&mut f, Figure::ShenKuo, A);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    let source = original(&mut f, A);
    let achievement = base(&mut f, "B021", Zone::AvailableAchievements);
    let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, _, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        &[achievement]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
}

#[test]
fn native_same_color_covering_disables_reactions_rather_than_inventing_alias_combinations() {
    for (covered, covering, ordinal) in [
        (Figure::Ptahhotep, Figure::EmperorMeiji, 2),
        (Figure::Avicenna, Figure::CaiLun, 1),
        (Figure::Avicenna, Figure::JohnLoudonMcAdam, 1),
    ] {
        let mut f = Fixture::default();
        let buried = native(&mut f, covered, A);
        let top = native(&mut f, covering, A);
        assert_eq!(
            f.cards[usize::from(buried.0)].color,
            f.cards[usize::from(top.0)].color
        );
        let source = original(&mut f, A);
        let writing = base(&mut f, "B012", Zone::Hand(B));
        let achievement = base(&mut f, "B021", Zone::AvailableAchievements);
        let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        for _ in 1..ordinal {
            game.begin_action(ActionKind::Regular).unwrap();
        }
        dogma(&mut game, source, ActionKind::Regular);
        let (game, events, status) = clone_boundaries(game, None);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
        assert_eq!(game.position().cards(Zone::Hand(B)), &[writing]);
        assert_eq!(
            game.position().cards(Zone::AvailableAchievements),
            &[achievement]
        );
        assert_eq!(events.len(), 1);
        assert!(!moved(&events[0]).inside_karma);
    }
}

#[test]
fn declining_a_copied_optional_effect_does_not_restore_ptahhoteps_original_dogma() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Ptahhotep, A);
    let source = original(&mut f, A);
    let agriculture = base(&mut f, "B004", Zone::Hand(A));
    let writing = base(&mut f, "B012", Zone::Hand(A));
    let untouched = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, agriculture);
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    let decline = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Decline)
        .unwrap();
    game.choose(decline).unwrap();
    let (game, suffix, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().cards(Zone::Score(A)), &[agriculture]);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[writing]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(2))),
        &[untouched]
    );
    assert!(suffix.is_empty());
    assert_eq!(events.len(), 2);
    assert_eq!(moved(&events[0]).card, agriculture);
    assert_eq!(
        events[1],
        Event::Revealed {
            card: agriculture,
            source: Zone::Score(A),
            executor: A,
            inside_karma: true
        }
    );
}

#[test]
fn platos_failed_splay_without_an_age_two_still_runs_the_original() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Plato, A);
    let source = original(&mut f, A);
    let achievement = base(&mut f, "B012", Zone::AvailableAchievements);
    let reward = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        game.position().cards(Zone::AvailableAchievements),
        &[achievement]
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[reward]);
    assert_eq!(events.len(), 1);
    assert!(!moved(&events[0]).inside_karma);
}

#[test]
fn terminal_copied_draw_abandons_remaining_effects_and_unsuppressed_fading() {
    let mut f = Fixture::default();
    native(&mut f, Figure::Ptahhotep, A);
    native(&mut f, Figure::Gilgamesh, A);
    // Red is occupied by Gilgamesh; use a distinct explicit synthetic source.
    let source = f.tech(Color::Green, 3, Zone::Board(A, Color::Green), vec![draw(1)]);
    f.cards[usize::from(source.0)].symbols = four_slots([Symbol::Icon(Icon::Authority); 4]);
    let writing = base(&mut f, "B012", Zone::Hand(A));
    let untouched = base(&mut f, "B004", Zone::Deck(Deck::base(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source, ActionKind::Regular);
    let (game, events, status) = clone_boundaries(game, None);
    assert!(matches!(status, Status::Winner(_) | Status::Draw(_)));
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[1],
        Event::Revealed {
            card: writing,
            source: Zone::Score(A),
            executor: A,
            inside_karma: true
        }
    );
    assert_eq!(moved(&events[0]).card, writing);
    assert!(moved(&events[0]).inside_karma);
    assert_eq!(game.position().top_figures(A).count(), 2);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(1))),
        &[untouched]
    );
}
