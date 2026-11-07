use inno_engine::*;

mod common;
use common::*;

// The plain catalog preserves Hedy's age and printed icons, but has no karma.
fn hedy_or_plain(f: &mut Fixture, enabled: bool, player: Player) -> CardId {
    let card = f.figure(Figure::HedyLamarr, Zone::Board(player, Color::Green));
    if !enabled {
        f.cards[usize::from(card.0)].figure = None;
        f.cards[usize::from(card.0)].set = CardSet::Base;
        f.cards[usize::from(card.0)].code = "synthetic-base-program";
    }
    card
}

fn icon_tops(f: &mut Fixture, player: Player, symbols: &[Icon]) {
    assert!(symbols.len() <= 16);
    for (color, chunk) in [Color::Red, Color::Blue, Color::Yellow, Color::Purple]
        .into_iter()
        .zip(symbols.chunks(4))
    {
        let card = f.tech(color, 1, Zone::Board(player, color), vec![]);
        for (slot, &icon) in chunk.iter().enumerate() {
            f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
        }
    }
}

fn splay(game: &mut Game, color: Color, direction: Splay) {
    game.start(
        Operation::Splay {
            player: A,
            color,
            direction,
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
}

#[test]
fn monument_lowers_four_top_demands_to_three_not_two() {
    for enabled in [false, true] {
        for count in 2..=4 {
            let mut f = Fixture::default();
            hedy_or_plain(&mut f, enabled, A);
            for (i, color) in [Color::Red, Color::Blue, Color::Yellow, Color::Purple]
                .into_iter()
                .enumerate()
            {
                let buried = f.tech(color, 1, Zone::Board(A, color), vec![]);
                f.cards[usize::from(buried.0)].effects[0].kind = EffectKind::Demand;
                let top = f.tech(color, 1, Zone::Board(A, color), vec![]);
                f.cards[usize::from(top.0)].effects[0].kind = EffectKind::from_demand(i < count);
            }
            let position = f.position().with_expansions(Expansions::FIGURES);
            assert_eq!(
                position.eligible_for_special(A, Special::Monument),
                count >= if enabled { 3 } else { 4 },
                "enabled={enabled}, demands={count}"
            );
            assert_eq!(
                position
                    .clone()
                    .with_expansions(Expansions::BASE)
                    .eligible_for_special(A, Special::Monument),
                count >= 4
            );
        }
    }
}

#[test]
fn universe_lowers_both_top_count_and_minimum_age_without_counting_buried_cards() {
    for enabled in [false, true] {
        for count in 3..=5 {
            for minimum in 6..=8 {
                let mut f = Fixture::default();
                hedy_or_plain(&mut f, enabled, A); // One qualifying age-nine top.
                for (i, color) in [Color::Red, Color::Blue, Color::Yellow, Color::Purple]
                    .into_iter()
                    .enumerate()
                {
                    f.tech(color, 11, Zone::Board(A, color), vec![]);
                    f.tech(
                        color,
                        if i < count - 1 { minimum } else { 1 },
                        Zone::Board(A, color),
                        vec![],
                    );
                }
                let position = f.position().with_expansions(Expansions::FIGURES);
                let basic = count == 5 && minimum >= 8;
                assert_eq!(
                    position.eligible_for_special(A, Special::Universe),
                    if enabled {
                        count >= 4 && minimum >= 7
                    } else {
                        basic
                    },
                    "enabled={enabled}, count={count}, minimum={minimum}"
                );
                assert_eq!(
                    position
                        .clone()
                        .with_expansions(Expansions::BASE)
                        .eligible_for_special(A, Special::Universe),
                    basic
                );
            }
        }
    }
}

#[test]
fn world_lowers_twelve_efficiency_to_eleven_not_ten() {
    for enabled in [false, true] {
        for total in 10..=12 {
            let mut f = Fixture::default();
            hedy_or_plain(&mut f, enabled, A);
            icon_tops(&mut f, A, &vec![Icon::Efficiency; total - 2]);
            let position = f.position().with_expansions(Expansions::FIGURES);
            assert_eq!(
                position.eligible_for_special(A, Special::World),
                total >= if enabled { 11 } else { 12 },
                "enabled={enabled}, efficiency={total}"
            );
            assert_eq!(
                position
                    .clone()
                    .with_expansions(Expansions::BASE)
                    .eligible_for_special(A, Special::World),
                total >= 12
            );
        }
    }
}

#[test]
fn empire_lowers_to_two_icons_of_any_five_of_its_six_printed_types() {
    let types = [
        Icon::Authority,
        Icon::Concept,
        Icon::Health,
        Icon::Prosperity,
        Icon::Industry,
        Icon::Efficiency,
    ];
    // Hedy supplies two Efficiency. One type is deficient while the others hold three:
    // printed Empire needs the sixth; lowered Empire does not.
    for enabled in [false, true] {
        for deficient in types {
            for count in 0..=3 {
                if deficient == Icon::Efficiency && count < 2 {
                    continue; // Cannot remove Hedy's printed Efficiency icons.
                }
                let mut f = Fixture::default();
                hedy_or_plain(&mut f, enabled, A);
                let mut symbols = Vec::new();
                for icon in types {
                    let wanted = if icon == deficient { count } else { 3 };
                    let supplied = if icon == Icon::Efficiency { 2 } else { 0 };
                    symbols.extend(std::iter::repeat_n(icon, wanted - supplied));
                }
                icon_tops(&mut f, A, &symbols);
                let position = f.position().with_expansions(Expansions::FIGURES);
                assert_eq!(
                    position.eligible_for_special(A, Special::Empire),
                    enabled || count >= 3,
                    "enabled={enabled}, deficient={deficient:?}, count={count}"
                );
                assert_eq!(
                    position
                        .clone()
                        .with_expansions(Expansions::BASE)
                        .eligible_for_special(A, Special::Empire),
                    count >= 3
                );
            }
        }
    }

    // Two of each of five printed types qualifies, whichever type is
    // omitted; an unlisted icon never stands in for a printed type, and two
    // deficient types are one too many.
    for omitted in types.into_iter().filter(|icon| *icon != Icon::Efficiency) {
        let mut f = Fixture::default();
        hedy_or_plain(&mut f, true, A);
        let mut symbols = Vec::new();
        for icon in types.into_iter().filter(|icon| *icon != Icon::Efficiency) {
            symbols.extend([if icon == omitted { Icon::Avatar } else { icon }; 2]);
        }
        icon_tops(&mut f, A, &symbols);
        assert!(
            f.position()
                .with_expansions(Expansions::FIGURES)
                .eligible_for_special(A, Special::Empire),
            "omitted {omitted:?}"
        );
    }
    let mut f = Fixture::default();
    hedy_or_plain(&mut f, true, A);
    // Three types plus Hedy's own Efficiency are only four.
    let symbols: Vec<_> = types[..3].iter().flat_map(|&icon| [icon; 2]).collect();
    icon_tops(&mut f, A, &symbols);
    assert!(
        !f.position()
            .with_expansions(Expansions::FIGURES)
            .eligible_for_special(A, Special::Empire)
    );
}

#[test]
fn wonder_lowers_five_colors_to_four_but_left_is_still_not_qualifying() {
    for enabled in [false, true] {
        let mut f = Fixture::default();
        for color in Color::ALL {
            f.tech(color, 1, Zone::Board(A, color), vec![]);
            if color == Color::Green {
                hedy_or_plain(&mut f, enabled, A);
            } else {
                f.tech(color, 2, Zone::Board(A, color), vec![]);
            }
        }
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        for (color, direction) in [
            (Color::Red, Splay::Right),
            (Color::Blue, Splay::Up),
            (Color::Green, Splay::Aslant),
            (Color::Yellow, Splay::Left),
            (Color::Purple, Splay::Left),
        ] {
            splay(&mut game, color, direction);
            assert!(!game.position().eligible_for_special(A, Special::Wonder));
        }
        splay(&mut game, Color::Yellow, Splay::Right);
        assert_eq!(
            game.position().eligible_for_special(A, Special::Wonder),
            enabled
        );
        assert!(
            !game
                .position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_special(A, Special::Wonder)
        );
        splay(&mut game, Color::Purple, Splay::Aslant);
        assert!(game.position().eligible_for_special(A, Special::Wonder));
        assert!(
            game.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_special(A, Special::Wonder)
        );
    }
}

#[test]
fn lowered_requirements_are_owner_only_and_disappear_on_burial_and_removal() {
    let mut f = Fixture::default();
    hedy_or_plain(&mut f, false, A);
    let hedy = hedy_or_plain(&mut f, true, A);
    hedy_or_plain(&mut f, false, B);
    for player in [A, B] {
        icon_tops(&mut f, player, &[Icon::Efficiency; 9]);
    }
    // Preserve the same eleven Efficiency when the karma is covered/removed.
    let cover = f.tech(Color::Green, 9, Zone::Hand(A), vec![]);
    f.cards[usize::from(cover.0)].symbols = four_slots([
        Symbol::Icon(Icon::Efficiency),
        Symbol::Icon(Icon::Avatar),
        Symbol::Icon(Icon::Efficiency),
        Symbol::Image,
    ]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    assert!(game.position().eligible_for_special(A, Special::World));
    assert!(!game.position().eligible_for_special(B, Special::World));
    game.start(meld(cover, Zone::Hand(A), A, Color::Green), Context::new(A))
        .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert!(!game.position().eligible_for_special(A, Special::World));
    game.start(
        Operation::Move {
            verb: Verb::Return,
            target: target(cover, Zone::Board(A, Color::Green)),
            destination: Zone::Deck(Deck::base(9)),
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert!(game.position().eligible_for_special(A, Special::World));
    game.start(
        Operation::Move {
            verb: Verb::Score,
            target: target(hedy, Zone::Board(A, Color::Green)),
            destination: Zone::Score(A),
        },
        Context::new(A),
    )
    .unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert_eq!(game.position().basic_icon_count(A, Icon::Efficiency), 11);
    assert!(!game.position().eligible_for_special(A, Special::World));
    assert!(!game.position().eligible_for_special(B, Special::World));
}

#[test]
fn hedy_and_maxwell_each_modifiers_apply_independently_and_recompute_live() {
    for hand_count in [7, 8] {
        let mut f = Fixture::default();
        let hedy = hedy_or_plain(&mut f, true, A);
        f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
        let hand: Vec<_> = (0..hand_count)
            .map(|_| f.tech(Color::Red, 1, Zone::Hand(A), vec![]))
            .collect();
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        assert_eq!(game.position().basic_icon_count(A, Icon::Efficiency), 3);
        assert_eq!(
            game.position().icon_count(A, Icon::Efficiency),
            3 + hand_count
        );
        assert_eq!(
            game.position().eligible_for_special(A, Special::World),
            hand_count == 8
        );
        assert!(
            !game
                .position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_special(A, Special::World)
        );
        game.start(
            Operation::Move {
                verb: Verb::Score,
                target: target(hand[0], Zone::Hand(A)),
                destination: Zone::Score(A),
            },
            Context::new(A),
        )
        .unwrap();
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        assert!(!game.position().eligible_for_special(A, Special::World));
        game.start(
            Operation::Move {
                verb: Verb::Score,
                target: target(hedy, Zone::Board(A, Color::Green)),
                destination: Zone::Score(A),
            },
            Context::new(A),
        )
        .unwrap();
        assert!(matches!(
            settle(&mut game, &mut Vec::new()),
            Status::Complete(_)
        ));
        assert_eq!(game.position().icon_count(A, Icon::Efficiency), hand_count);
        assert!(!game.position().eligible_for_special(A, Special::World));
    }
}
