use inno_engine::*;

mod common;
use common::*;

#[test]
fn universe_and_monument_count_qualifying_top_cards_only() {
    for special in [Special::Universe, Special::Monument] {
        for count in 0..=5 {
            let mut f = Fixture::default();
            for (i, color) in Color::ALL.into_iter().enumerate() {
                let buried = f.tech(color, 11, Zone::Board(A, color), vec![]);
                f.cards[usize::from(buried.0)].effects[0].kind = EffectKind::Demand;
                let top = f.tech(
                    color,
                    if i < count { 8 } else { 7 },
                    Zone::Board(A, color),
                    vec![],
                );
                f.cards[usize::from(top.0)].effects[0].kind = EffectKind::from_demand(i < count);
            }
            let position = f.position();
            assert_eq!(
                position
                    .clone()
                    .with_expansions(Expansions::BASE)
                    .eligible_for_special(A, special),
                count >= if special == Special::Universe { 5 } else { 4 }
            );
        }
    }
}

#[test]
fn world_and_empire_require_the_actual_visible_standard_icons() {
    for complete in [false, true] {
        let mut f = Fixture::default();
        for (i, color) in [Color::Red, Color::Blue, Color::Green]
            .into_iter()
            .enumerate()
        {
            let card = f.tech(color, 1, Zone::Board(A, color), vec![]);
            f.cards[usize::from(card.0)].symbols = four_slots([Symbol::Icon(Icon::Efficiency); 4]);
            if !complete && i == 0 {
                f.cards[usize::from(card.0)].symbols[0] = Symbol::Icon(Icon::Avatar);
            }
        }
        assert_eq!(
            f.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_special(A, Special::World),
            complete
        );

        let mut f = Fixture::default();
        let icons = [
            Icon::Authority,
            Icon::Concept,
            Icon::Health,
            Icon::Prosperity,
            Icon::Industry,
            Icon::Efficiency,
        ];
        let mut symbols: Vec<_> = icons
            .into_iter()
            .flat_map(|icon| [Symbol::Icon(icon); 3])
            .collect();
        if !complete {
            symbols[17] = Symbol::Icon(Icon::Avatar);
        }
        symbols.resize(20, Symbol::Blank);
        for (color, slots) in Color::ALL.into_iter().zip(symbols.as_chunks::<4>().0) {
            let buried = f.tech(color, 1, Zone::Board(A, color), vec![]);
            f.cards[usize::from(buried.0)].symbols =
                four_slots([Symbol::Icon(Icon::Efficiency); 4]);
            let top = f.tech(color, 1, Zone::Board(A, color), vec![]);
            f.cards[usize::from(top.0)].symbols[..4].copy_from_slice(slots);
        }
        assert_eq!(
            f.position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_special(A, Special::Empire),
            complete
        );
    }
}

#[test]
fn wonder_requires_all_five_colors_right_up_or_aslant() {
    let mut f = Fixture::default();
    for color in Color::ALL {
        for age in [1, 2] {
            f.tech(color, age, Zone::Board(A, color), vec![]);
        }
    }
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    let mut events = Vec::new();
    for (i, color) in Color::ALL.into_iter().enumerate() {
        let direction = [
            Splay::Right,
            Splay::Up,
            Splay::Aslant,
            Splay::Right,
            Splay::Left,
        ][i];
        game.start(
            Operation::Splay {
                player: A,
                color,
                direction,
            },
            Context::new(A),
        )
        .unwrap();
        settle(&mut game, &mut events);
        assert!(
            !game
                .position()
                .clone()
                .with_expansions(Expansions::BASE)
                .eligible_for_special(A, Special::Wonder)
        );
    }
    game.start(
        Operation::Splay {
            player: A,
            color: Color::Purple,
            direction: Splay::Right,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert!(
        game.position()
            .clone()
            .with_expansions(Expansions::BASE)
            .eligible_for_special(A, Special::Wonder)
    );
}

#[test]
fn special_claims_and_junking_preserve_singular_ownership_and_achievement_counts() {
    let mut game = Game::new(Fixture::default().position(), Expansions::BASE).unwrap();
    let mut events = Vec::new();
    let checked = Operation::ClaimSpecial {
        special: Special::World,
        check_eligibility: true,
    };
    game.start(checked, Context::new(A)).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Inapplicable,
            card: None,
            value: None,
            short: false,
        }))
    );
    let effect = Operation::ClaimSpecial {
        special: Special::World,
        check_eligibility: false,
    };
    game.start(effect, Context::new(A)).unwrap();
    settle(&mut game, &mut events);
    assert_eq!(game.position().achievement_count(A), 1);
    assert_eq!(
        game.position().special_location(Special::World),
        SpecialLocation::Owned(A)
    );
    for request in [
        effect,
        Operation::JunkAvailableSpecial {
            special: Special::World,
        },
    ] {
        game.start(request, Context::new(B)).unwrap();
        settle(&mut game, &mut events);
    }
    assert_eq!(game.position().achievement_count(B), 0);
    assert_eq!(events.len(), 1);
    game.start(
        Operation::JunkAvailableSpecial {
            special: Special::Monument,
        },
        Context::new(B),
    )
    .unwrap();
    settle(&mut game, &mut events);
    game.start(
        Operation::ClaimSpecial {
            special: Special::Monument,
            check_eligibility: false,
        },
        Context::new(A),
    )
    .unwrap();
    settle(&mut game, &mut events);
    assert_eq!(events.len(), 2);
    assert_eq!(
        game.position().special_location(Special::Monument),
        SpecialLocation::Junk
    );
    // A special counts in exhaustion's achievement tiebreak as well.
    game.start(
        Operation::Draw {
            value: 12,
            set: None,
        },
        Context::new(B),
    )
    .unwrap();
    assert_eq!(settle(&mut game, &mut events), Status::Winner(A));
}
