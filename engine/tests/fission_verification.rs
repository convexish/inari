//! A hidden drawn color must be verifiable on both branches of Fission.
use inno_engine::*;
mod common;
use common::*;

#[test]
fn fission_reveals_only_its_original_draw_before_testing_red() {
    for tracked in [false, true] {
        for color in [Color::Red, Color::Blue] {
            let mut f = Fixture::default();
            let source = f.add(base_card("B087").unwrap(), Zone::Board(A, Color::Red));
            f.tech(Color::Purple, 10, Zone::Deck(Deck::base(10)), vec![]);
            let unrelated = f.tech(Color::Green, 10, Zone::Hand(B), vec![]);
            let drawn = f.tech(color, 10, Zone::Deck(Deck::base(10)), vec![]);
            let mut game = Game::seeded(f.position(), Expansions::BASE, 0).unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            let mut context = Context::new(B);
            context.demander = A;
            context.source_owner = A;
            game.start(
                Operation::ExecuteEffect {
                    card: source,
                    effect: 0,
                    boxed: BoxedValues::default(),
                },
                context,
            )
            .unwrap();
            let mut events = Vec::new();
            assert!(matches!(
                settle(&mut game, &mut events),
                Status::Complete(_)
            ));
            let revealed = events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed {
                        card,
                        source: Zone::Hand(B),
                        ..
                    } => Some(*card),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(revealed, [drawn]);
            assert!(!revealed.contains(&unrelated));
            if color == Color::Red {
                let reveal = events
                    .iter()
                    .position(|event| matches!(event,Event::Revealed{card,..} if *card==drawn))
                    .unwrap();
                let junk=events.iter().position(|event|matches!(event,Event::Moved(movement) if movement.destination==Zone::Junk)).unwrap();
                assert!(reveal < junk);
                assert!(game.position().cards(Zone::Hand(B)).is_empty());
            } else {
                assert!(game.position().cards(Zone::Hand(B)).contains(&drawn));
                if tracked {
                    let belief = game.tracker(A).unwrap().belief(Zone::Hand(B));
                    assert!(belief.iter().any(|slot| slot.known() == Some(drawn)));
                    assert!(!belief.iter().any(|slot| slot.known() == Some(unrelated)));
                }
            }
        }
    }
}
