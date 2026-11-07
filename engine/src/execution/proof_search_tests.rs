//! Public proofs make search possible without disclosing unrelated fronts.
use super::*;
use crate::program::build::*;
use shortcuts::common::{A, B, Fixture, decision};

const PACKET: Slot = Slot::effect(0);
const BLUE: Selector = filtered(
    &Selector::Slot(PACKET),
    &[Filter::Has(Test::Color(Color::Blue))],
);
const HAS_BLUE: Cond = Cond::Compare(Expr::Count(&BLUE), Cmp::Greater, Expr::Const(0));
const STOP: Instruction = choose(Who::Opponent, Choice::Value(ValueChoices::Boxed(&[1, 2])));

fn search(game: &Game, player: Player) {
    let before = game.clone();
    for seed in 0..8 {
        let world = game.determinize(player, seed).unwrap();
        assert_eq!(
            world.observe(player).unwrap(),
            game.observe(player).unwrap()
        );
        assert!(world.position.check_conservation());
        assert!(world.tracker(player).unwrap().check_truth(world.position()));
    }
    assert_eq!(
        game.search(SearchConfig {
            iterations: 4,
            rollout_actions: 0,
            seed: 19,
            ..SearchConfig::default()
        })
        .unwrap()
        .iterations,
        4
    );
    assert_eq!(*game, before);
}

#[test]
fn a_public_witness_proves_existence_without_proving_the_exact_count() {
    for exact in [false, true] {
        let mut f = Fixture::default();
        let witness = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let private = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let alternative = f.tech(Color::Red, 1, Zone::Deck(Deck::base(1)), vec![]);
        let source = f.tech(
            Color::Yellow,
            2,
            Zone::Junk,
            vec![
                return_(HAND)
                    .all()
                    .verify_when(&Cond::Compare(
                        Expr::Count(&Selector::Slot(PACKET)),
                        Cmp::Equal,
                        Expr::Const(0),
                    ))
                    .keep(PACKET),
                reveal_all(Selector::Slot(PACKET)).unless_proven(&HAS_BLUE),
                if_(
                    if exact {
                        &Cond::Compare(Expr::Count(&BLUE), Cmp::Equal, Expr::Const(2))
                    } else {
                        &HAS_BLUE
                    },
                    0,
                ),
                STOP,
            ],
        );
        let mut game = Game::new(f.position(), Expansions::BASE)
            .unwrap()
            .with_tracking()
            .unwrap();
        let mut events = Vec::new();
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: 0,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        loop {
            let menu = decision(&mut game, &mut events);
            if menu.player == B {
                break;
            }
            game.choose(0).unwrap();
        }
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed { card, .. } => Some(*card),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [witness]
        );
        if exact {
            assert!(matches!(
                game.search(SearchConfig {
                    iterations: 4,
                    ..SearchConfig::default()
                }),
                Err(Error::Unsupported(
                    "a retained scalar depends on private information"
                ))
            ));
        } else {
            search(&game, B);
            let mut identities = std::collections::BTreeSet::new();
            for seed in 0..32 {
                let world = game.determinize(B, seed).unwrap();
                for frame in &world.frames {
                    if let Frame::Locals(locals) = frame
                        && let Some(slots::Held::Cards(records)) = &locals.slots[0]
                    {
                        identities.extend(
                            records
                                .as_slice()
                                .iter()
                                .filter_map(|held| (held.inner != witness).then_some(held.inner)),
                        );
                    }
                }
            }
            assert_eq!(
                identities,
                [private, alternative].into_iter().collect(),
                "the other returned card must remain hidden"
            );
        }
    }
}

#[test]
fn sunglasses_proves_matching_selection_or_absence_before_foreign_search() {
    for matching in [false, true] {
        let mut f = Fixture::default();
        let mut printed = echoes_card("E035").unwrap();
        printed.echo.as_mut().unwrap().instructions.push(STOP);
        let source = f.add(printed, Zone::Junk);
        for _ in 0..2 {
            f.tech(Color::Red, 2, Zone::Board(A, Color::Red), vec![]);
        }
        let selected = f.tech(
            if matching { Color::Red } else { Color::Green },
            1,
            Zone::Hand(A),
            vec![],
        );
        let hidden = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        f.tech(Color::Green, 1, Zone::Deck(Deck::base(1)), vec![]);
        let mut position = f.position();
        position.players[A.index()].splays[Color::Red as usize] = Splay::Right;
        let mut game = Game::new(position, Expansions::ECHOES)
            .unwrap()
            .with_tracking()
            .unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: source,
                effect: Card::ECHO,
                boxed: BoxedValues::ZERO,
            },
            Context::new(A),
        )
        .unwrap();
        let mut events = Vec::new();
        loop {
            let menu = decision(&mut game, &mut events);
            if menu.player == B {
                break;
            }
            game.choose(0).unwrap();
        }
        let reveals: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(
            reveals,
            if matching {
                vec![selected]
            } else {
                vec![selected, hidden]
            }
        );
        assert_eq!(
            game.position.locate(selected),
            Some(if matching {
                Zone::Score(A)
            } else {
                Zone::Hand(A)
            })
        );
        search(&game, B);
    }
}
