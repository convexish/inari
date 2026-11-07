//! Query hard cases and their complete bounded shortcut oracles.
use super::common::*;
use super::*;

const E: Expansions = Expansions::ECHOES;
const EF: Expansions = E.union(Expansions::FIGURES);

fn printed(f: &mut Fixture, color: Color, value: u8, symbols: [Symbol; 4], zone: Zone) -> CardId {
    let id = f.tech(color, value, zone, vec![]);
    f.cards[usize::from(id.0)].symbols = four_slots(symbols);
    id
}

fn source(f: &mut Fixture, code: &str) -> CardId {
    f.add(echoes_card(code).unwrap(), Zone::Junk)
}

fn start(
    position: Position,
    expansions: Expansions,
    card: CardId,
    effect: u16,
    demand: bool,
) -> Game {
    let mut game = Game::seeded(position, expansions, 57)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(if demand { B } else { A });
    context.demander = A;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}

fn finish(game: &mut Game, mut pick: impl FnMut(&Decision) -> usize) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            status => {
                assert!(game.position.check_conservation());
                return (status, events);
            }
        }
    }
}

fn skip(menu: &Decision) -> usize {
    menu.options
        .iter()
        .position(|option| *option == Alternative::Branch(1))
        .unwrap()
}

#[test]
fn novel_common_types_include_bonus_city_search_and_figure_but_exclude_prosperity_and_text() {
    use Symbol::{Bonus, City, Echo, Icon as I, Image, Search};
    let cases = [
        (vec![], true),
        (vec![I(Icon::Prosperity), I(Icon::Prosperity)], false),
        (vec![I(Icon::Concept), I(Icon::Concept)], true),
        (vec![I(Icon::Avatar), I(Icon::Avatar)], true),
        (vec![I(Icon::Food), I(Icon::Food)], true),
        (vec![I(Icon::Concept), I(Icon::Health)], false),
        (vec![Bonus(1), Bonus(12)], true),
        (vec![City(CityIcon::Flag), City(CityIcon::Flag)], true),
        (vec![City(CityIcon::Flag), City(CityIcon::Fountain)], false),
        (vec![City(CityIcon::Flag), Bonus(1)], false),
        (vec![Search(Icon::Health), I(Icon::Health)], true),
        (vec![Search(Icon::Concept), Search(Icon::Health)], false),
        (vec![Echo, Echo], false),
        (vec![Image, Image], false),
        (vec![Symbol::Blank], false),
    ];
    for (symbols, expected) in cases {
        let mut f = Fixture::default();
        let novel = source(&mut f, "E034");
        // Purple never participates, nor do covered non-purple cards.
        printed(
            &mut f,
            Color::Purple,
            1,
            [Symbol::Blank; 4],
            Zone::Board(A, Color::Purple),
        );
        for (&symbol, color) in symbols.iter().zip([Color::Red, Color::Blue]) {
            printed(&mut f, color, 1, [Symbol::Blank; 4], Zone::Board(A, color));
            let id = printed(&mut f, color, 1, [symbol; 4], Zone::Board(A, color));
            f.cards[usize::from(id.0)].set = CardSet::Cities;
        }
        let mut game = start(f.position(), E.union(Expansions::CITIES), novel, 2, false);
        finish(&mut game, |_| panic!("claim needs no choice"));
        assert_eq!(
            game.position.special_location(Special::Supremacy) == SpecialLocation::Owned(A),
            expected,
            "{symbols:?}"
        );
    }
    let mut f = Fixture::default();
    let novel = source(&mut f, "E034");
    let napoleon = figure_card(Figure::NapoleonBonaparte.code()).unwrap();
    assert_eq!(napoleon.bonus(), Some(6));
    let other = Color::ALL
        .into_iter()
        .find(|&c| c != napoleon.color && c != Color::Purple)
        .unwrap();
    f.add(napoleon.clone(), Zone::Board(A, napoleon.color));
    printed(&mut f, other, 1, [Bonus(2); 4], Zone::Board(A, other));
    let mut game = start(f.position(), EF, novel, 2, false);
    finish(&mut game, |_| 0);
    assert_eq!(
        game.position.special_location(Special::Supremacy),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn novel_draw_splay_and_human_genome_echo_use_their_printed_clauses() {
    for (code, effect, age) in [("E034", 0, 3), ("E103", Card::ECHO, 11)] {
        let mut f = Fixture::default();
        let card = source(&mut f, code);
        let drawn = printed(
            &mut f,
            Color::Blue,
            age,
            [Symbol::Blank; 4],
            Zone::Deck(Deck::base(age)),
        );
        let mut game = start(f.position(), E, card, effect, false);
        finish(&mut game, |_| panic!("one draw"));
        assert_eq!(game.position.cards(Zone::Hand(A)), &[drawn]);
    }
    for take in [false, true] {
        let mut f = Fixture::default();
        let novel = source(&mut f, "E034");
        for _ in 0..2 {
            printed(
                &mut f,
                Color::Purple,
                1,
                [Symbol::Blank; 4],
                Zone::Board(A, Color::Purple),
            );
        }
        let mut game = start(f.position(), E, novel, 1, false);
        let mut menus = 0;
        finish(&mut game, |menu| {
            menus += 1;
            menu.options
                .iter()
                .position(|o| matches!(o, Alternative::Decline) != take)
                .unwrap()
        });
        assert_eq!(menus, 1);
        assert_eq!(
            game.position.splay(A, Color::Purple),
            if take { Splay::Left } else { Splay::None }
        );
    }
}

#[test]
fn human_genome_compares_multisets_after_bottom_transfer_and_allows_empty_equality() {
    for (hand, score, red, expected) in [
        (vec![], vec![], None, true),
        (vec![], vec![1], None, false),
        (vec![2, 1, 1], vec![1, 2, 1], None, true),
        (vec![1, 2, 2], vec![1, 1, 2], None, false),
        (vec![1, 2], vec![1, 1, 2], None, false),
        (vec![1], vec![1, 2], Some(2), true),
        (vec![1], vec![1], Some(2), false),
    ] {
        let mut f = Fixture::default();
        let genome = source(&mut f, "E103");
        for &age in &hand {
            printed(&mut f, Color::Green, age, [Symbol::Blank; 4], Zone::Hand(A));
        }
        for &age in &score {
            printed(&mut f, Color::Blue, age, [Symbol::Blank; 4], Zone::Score(A));
        }
        let bottom = red.map(|age| {
            printed(
                &mut f,
                Color::Red,
                age,
                [Symbol::Blank; 4],
                Zone::Board(A, Color::Red),
            )
        });
        let top = bottom.map(|_| {
            printed(
                &mut f,
                Color::Red,
                7,
                [Symbol::Blank; 4],
                Zone::Board(A, Color::Red),
            )
        });
        let mut game = start(f.position(), E, genome, 0, false);
        let (status, _) = finish(&mut game, skip);
        assert_eq!(
            status == Status::Winner(A),
            expected,
            "{hand:?} {score:?} {red:?}"
        );
        if let Some(bottom) = bottom {
            assert_eq!(game.position.locate(bottom), Some(Zone::Hand(A)));
        }
        if let Some(top) = top {
            assert_eq!(game.position.top(A, Color::Red), Some(top));
        }
    }
}

#[test]
fn human_genome_optional_draw_scores_actual_draw_then_compares_values() {
    let mut f = Fixture::default();
    let genome = source(&mut f, "E103");
    let drawn = printed(
        &mut f,
        Color::Blue,
        3,
        [Symbol::Blank; 4],
        Zone::Deck(Deck::base(3)),
    );
    printed(&mut f, Color::Green, 3, [Symbol::Blank; 4], Zone::Hand(A));
    let mut game = start(f.position(), E, genome, 0, false);
    let (status, _) = finish(&mut game, |menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(2))
            .unwrap_or(0)
    });
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.position.cards(Zone::Score(A)), &[drawn]);
}

#[test]
fn meritocracy_victim_chooses_only_demanders_most_icons_and_sweeps_live_visible_matches() {
    for chosen in [Icon::Health, Icon::Industry] {
        let mut f = Fixture::default();
        let merit = source(&mut f, "E115");
        printed(
            &mut f,
            Color::Purple,
            1,
            [
                Symbol::Icon(Icon::Health),
                Symbol::Icon(Icon::Health),
                Symbol::Icon(Icon::Industry),
                Symbol::Icon(Icon::Industry),
            ],
            Zone::Board(A, Color::Purple),
        );
        // The bottom matching card is initially hidden; after the top moves,
        // the one-card remainder loses its splay and becomes fully visible.
        let bottom = printed(
            &mut f,
            Color::Red,
            1,
            [
                Symbol::Icon(chosen),
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
            ],
            Zone::Board(B, Color::Red),
        );
        let top = printed(
            &mut f,
            Color::Red,
            2,
            [Symbol::Icon(chosen); 4],
            Zone::Board(B, Color::Red),
        );
        let survivor = printed(
            &mut f,
            Color::Blue,
            2,
            [Symbol::Icon(Icon::Concept); 4],
            Zone::Board(B, Color::Blue),
        );
        let mut p = f.position();
        p.players[B.index()].splays[Color::Red as usize] = Splay::Left;
        let mut game = start(p, E, merit, 0, true);
        let mut icon_choices = 0;
        finish(&mut game, |menu| {
            assert_eq!(menu.player, B);
            assert_eq!(
                menu.options,
                vec![
                    Alternative::Icon(Icon::Health),
                    Alternative::Icon(Icon::Industry)
                ]
            );
            icon_choices += 1;
            menu.options
                .iter()
                .position(|o| *o == Alternative::Icon(chosen))
                .unwrap()
        });
        assert_eq!(icon_choices, 1);
        assert_eq!(
            game.position.cards(Zone::Board(A, Color::Red)),
            &[top, bottom]
        );
        assert_eq!(
            game.position.locate(survivor),
            Some(Zone::Board(B, Color::Blue))
        );
        assert_eq!(game.position.splay(B, Color::Red), Splay::None);
    }
    assert!(Test::ShowsChosenIcon.reads_front());
}

#[derive(Debug, PartialEq, Eq)]
struct Leaf {
    position: Position,
    rng: Rng,
    status: Status,
    events: Vec<String>,
    private_deck_one: Vec<crate::tracking::BeliefSlot>,
}

fn leaves(mut game: Game, mut events: Vec<Event>, depth: usize) -> Vec<Leaf> {
    assert!(depth < 12, "bounded oracle fixture");
    let mut tiny = game.clone();
    let mut tiny_events = events.clone();
    let status = game.advance(1000, &mut events).unwrap();
    let tiny_status = loop {
        let status = tiny.advance(1, &mut tiny_events).unwrap();
        if status != Status::WorkLimit {
            break status;
        }
    };
    assert_eq!(status, tiny_status);
    assert_eq!(
        game, tiny,
        "every oracle path clones and resumes at tiny budgets"
    );
    assert_eq!(
        events, tiny_events,
        "canonical path preserves exact chronology"
    );
    match status {
        Status::Decision(menu) => (0..menu.options.len())
            .flat_map(|choice| {
                let mut child = game.clone();
                child.choose(choice).unwrap();
                leaves(child, events.clone(), depth + 1)
            })
            .collect(),
        status => {
            assert!(!matches!(status, Status::WorkLimit));
            assert!(game.position.check_conservation());
            for player in Player::ALL {
                assert!(game.tracker(player).unwrap().check_truth(&game.position));
            }
            let private_deck_one = game.tracker(B).unwrap().belief(Zone::Deck(Deck::base(1)));
            game.position.sort_unordered_piles();
            // Commuting operations may permute their events, but preserve each
            // operation, observer revelation, source/destination and reaction.
            let mut events: Vec<_> = events
                .into_iter()
                .map(|event| format!("{event:?}"))
                .collect();
            events.sort();
            vec![Leaf {
                position: game.position,
                rng: game.rng,
                status,
                events,
                private_deck_one,
            }]
        }
    }
}

fn oracle(game: Game, reduced_paths: usize, unrestricted_paths: usize) -> Vec<Leaf> {
    let mut unrestricted = game.clone();
    unrestricted.shortcuts_disabled = true;
    let reduced = leaves(game, Vec::new(), 0);
    let unrestricted = leaves(unrestricted, Vec::new(), 0);
    assert_eq!(reduced.len(), reduced_paths, "reduced path count");
    assert_eq!(
        unrestricted.len(),
        unrestricted_paths,
        "unrestricted path count"
    );
    if let Some(missing) = unrestricted.iter().find(|leaf| !reduced.contains(leaf)) {
        panic!(
            "lost outcome: position {} rng {} status {} events {}; missing events {:?}; reduced {:?}",
            reduced.iter().any(|leaf| leaf.position == missing.position),
            reduced.iter().any(|leaf| leaf.rng == missing.rng),
            reduced.iter().any(|leaf| leaf.status == missing.status),
            reduced.iter().any(|leaf| leaf.events == missing.events),
            missing.events,
            reduced.iter().map(|leaf| &leaf.events).collect::<Vec<_>>()
        );
    }
    assert!(
        reduced.iter().all(|leaf| unrestricted.contains(leaf)),
        "invented outcome"
    );
    reduced
}

#[test]
fn novel_forecast_return_oracle_keeps_same_deck_orders_and_runs_maxwell() {
    for reacting in [false, true] {
        let mut f = Fixture::default();
        let novel = source(&mut f, "E034");
        for age in [1, 1, 2] {
            printed(
                &mut f,
                Color::Blue,
                age,
                [Symbol::Blank; 4],
                Zone::Forecast(A),
            );
        }
        if reacting {
            let maxwell = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
            f.add(maxwell.clone(), Zone::Board(A, maxwell.color));
            for _ in 0..3 {
                printed(
                    &mut f,
                    Color::Green,
                    8,
                    [Symbol::Blank; 4],
                    Zone::Deck(Deck::base(8)),
                );
            }
        }
        let game = start(f.position(), EF, novel, Card::ECHO, false);
        let outcomes = oracle(game, if reacting { 6 } else { 2 }, 6);
        for leaf in outcomes {
            assert!(leaf.position.cards(Zone::Forecast(A)).is_empty());
            assert_eq!(leaf.private_deck_one.len(), 2);
            assert!(
                leaf.private_deck_one
                    .iter()
                    .all(|slot| slot.known().is_none()),
                "returning same-back Forecast cards never identifies their deck order"
            );
            assert!(
                !leaf
                    .events
                    .iter()
                    .any(|event| event.contains("Revealed") && event.contains("Forecast"))
            );
            assert_eq!(
                leaf.position.cards(Zone::Hand(A)).len(),
                if reacting { 3 } else { 0 }
            );
            assert_eq!(
                leaf.events
                    .iter()
                    .filter(|s| s.contains("inside_karma: true") && s.contains("Moved"))
                    .count(),
                if reacting { 3 } else { 0 }
            );
        }
    }
}

fn meritocracy_game(foreseen: bool, reacting: bool) -> (Game, Vec<CardId>, CardId) {
    let mut f = Fixture::default();
    let merit = f.add(
        echoes_card("E115").unwrap(),
        if foreseen {
            Zone::Forecast(A)
        } else {
            Zone::Board(A, Color::Purple)
        },
    );
    // Health is uniquely most shown; the demand's featured Concept still
    // makes B vulnerable. Keep the promotion meld free of City/Figure rules.
    let action = printed(
        &mut f,
        Color::Yellow,
        11,
        [Symbol::Icon(Icon::Health); 4],
        if foreseen {
            Zone::Hand(A)
        } else {
            Zone::Board(A, Color::Yellow)
        },
    );
    let mut moved = vec![];
    moved.push(printed(
        &mut f,
        Color::Red,
        1,
        [
            Symbol::Icon(Icon::Health),
            Symbol::Blank,
            Symbol::Blank,
            Symbol::Blank,
        ],
        Zone::Board(B, Color::Red),
    ));
    for _ in 0..2 {
        moved.push(printed(
            &mut f,
            Color::Red,
            1,
            [
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Icon(Icon::Health),
            ],
            Zone::Board(B, Color::Red),
        ));
    }
    moved.push(printed(
        &mut f,
        Color::Green,
        1,
        [Symbol::Icon(Icon::Health); 4],
        Zone::Board(B, Color::Green),
    ));
    let survivor = printed(
        &mut f,
        Color::Blue,
        1,
        [Symbol::Icon(Icon::Industry); 4],
        Zone::Board(B, Color::Blue),
    );
    let junked = printed(
        &mut f,
        Color::Green,
        7,
        [Symbol::Blank; 4],
        Zone::Deck(Deck::base(7)),
    );
    if reacting {
        let nobel = figure_card(Figure::AlfredNobel.code()).unwrap();
        f.add(nobel.clone(), Zone::Board(A, nobel.color));
    }
    let mut position = f.position();
    position.players[B.index()].splays[Color::Red as usize] = Splay::Left;
    let game = if foreseen {
        let mut game = Game::seeded(position, EF, 57)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.take_action(Action::Meld { card: action }, ActionKind::Regular)
            .unwrap();
        game
    } else {
        start(position, EF, merit, 0, true)
    };
    assert_eq!(
        game.position.locate(survivor),
        Some(Zone::Board(B, Color::Blue))
    );
    (game, moved, junked)
}

#[test]
fn meritocracy_all_paths_preserve_same_color_orders_and_foreseen_visibility_closure() {
    for foreseen in [false, true] {
        for reacting in [false, true] {
            let (game, moved, junked) = meritocracy_game(foreseen, reacting);
            let outcomes = oracle(
                game,
                if reacting {
                    8
                } else if foreseen {
                    1
                } else {
                    2
                },
                8,
            );
            for leaf in outcomes {
                assert!(leaf.position.cards(Zone::Board(B, Color::Red)).is_empty());
                assert_eq!(leaf.position.splay(B, Color::Red), Splay::None);
                for card in &moved {
                    let expected = if foreseen {
                        Zone::Achievements(A)
                    } else {
                        Zone::Board(A, leaf.position.card(*card).color)
                    };
                    assert_eq!(leaf.position.locate(*card), Some(expected));
                }
                assert_eq!(
                    leaf.position.locate(junked),
                    Some(if reacting {
                        Zone::Junk
                    } else {
                        Zone::Deck(Deck::base(7))
                    })
                );
                assert!(
                    leaf.events
                        .iter()
                        .any(|event| event.contains("inside_karma: true"))
                        == reacting
                );
            }
        }
    }
    assert!(std::mem::size_of::<Instruction>() <= 96);
    assert_eq!(std::mem::size_of::<Frame>(), 56);
}
