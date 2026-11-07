//! Oars' impossible mandatory transfer proves the remaining hand publicly.
use inno_engine::*;

const A: Player = Player::One;
const B: Player = Player::Two;

fn fixture(expansions: Expansions, matching: bool) -> Game {
    let cards = ["B003", "B001", "B006", "B007", "B002", "B004", "B009"]
        .map(|code| base_card(code).unwrap());
    let placements = [
        (CardId(0), Zone::Board(A, Color::Red)),
        (CardId(1), Zone::Hand(B)),
        (CardId(2), Zone::Hand(B)),
        (
            CardId(3),
            if matching {
                Zone::Hand(B)
            } else {
                Zone::Deck(Deck::base(1))
            },
        ),
        (CardId(4), Zone::Deck(Deck::base(1))),
        (CardId(5), Zone::Deck(Deck::base(1))),
        (CardId(6), Zone::Deck(Deck::base(1))),
    ];
    Game::seeded(
        Position::new(cards.to_vec(), &placements, A).unwrap(),
        expansions,
        17,
    )
    .unwrap()
    .with_tracking()
    .unwrap()
}

#[test]
fn oars_reveals_the_whole_remaining_hand_before_its_failed_transfer_returns() {
    for mode in 0..8 {
        let mut expansions = Expansions::BASE;
        for (bit, expansion) in [
            (1, Expansions::ECHOES),
            (2, Expansions::FIGURES),
            (4, Expansions::CITIES),
        ] {
            if mode & bit != 0 {
                expansions = expansions.union(expansion);
            }
        }
        for matching in [false, true] {
            let mut game = fixture(expansions, matching);
            assert!(
                game.tracker(A)
                    .unwrap()
                    .belief(Zone::Hand(B))
                    .iter()
                    .all(|slot| slot.known().is_none())
            );
            game.take_action(
                Action::Dogma {
                    card: CardId(0),
                    auspice: false,
                },
                ActionKind::Regular,
            )
            .unwrap();
            let mut events = Vec::new();
            let mut completed = false;
            for _ in 0..1000 {
                let mut fork = game.clone();
                let mut fork_events = Vec::new();
                let status = game.advance(1, &mut events).unwrap();
                assert_eq!(fork.advance(1, &mut fork_events).unwrap(), status);
                assert_eq!(fork, game);
                assert_eq!(&events[events.len() - fork_events.len()..], fork_events);
                assert!(game.position().check_conservation());
                for viewer in Player::ALL {
                    assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
                }
                if matches!(status, Status::WorkLimit) {
                    continue;
                }
                assert!(matches!(status, Status::Complete(_)), "{status:?}");
                completed = true;
                break;
            }
            assert!(completed, "Oars must finish within the bounded work budget");
            let remaining = game.position().cards(Zone::Hand(B));
            assert_eq!(remaining.len(), if matching { 3 } else { 2 });
            let revealed: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Revealed {
                        card,
                        source: Zone::Hand(B),
                        executor: B,
                        ..
                    } => Some(*card),
                    _ => None,
                })
                .collect();
            assert_eq!(revealed, remaining);
            assert_eq!(
                game.position().cards(Zone::Score(A)),
                if matching { &[CardId(3)][..] } else { &[] }
            );
            assert_eq!(
                game.position().cards(Zone::Hand(A)).len(),
                usize::from(!matching)
            );
            let mut known: Vec<_> = game
                .tracker(A)
                .unwrap()
                .belief(Zone::Hand(B))
                .iter()
                .map(|slot| slot.known().unwrap())
                .collect();
            known.sort_unstable();
            let mut expected = remaining.to_vec();
            expected.sort_unstable();
            assert_eq!(known, expected);
            assert!(
                game.public_tracker()
                    .unwrap()
                    .belief(Zone::Hand(B))
                    .iter()
                    .all(|slot| slot.known().is_some())
            );
            if !matching {
                let first_draw = events
                    .iter()
                    .position(|event| matches!(event, Event::Moved(m) if m.verb.is_none()))
                    .unwrap();
                assert_eq!(
                    first_draw, 2,
                    "both hand cards reveal before the owner's fallback draw"
                );
            }
        }
    }
}
