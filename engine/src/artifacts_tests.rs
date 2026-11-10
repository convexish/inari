use crate::*;

const A: Player = Player::One;
const B: Player = Player::Two;

fn blank(color: Color, value: u8) -> Card {
    Card {
        code: "synthetic-artifact",
        set: CardSet::Artifacts,
        color,
        value,
        figure: None,
        echo: None,
        symbols: [Symbol::Blank; SLOTS],
        featured: None,
        effects: Vec::new(),
    }
}

#[test]
fn a_museum_award_counts_at_once_and_frees_its_museum_only_after_the_return() {
    let cards = vec![
        blank(Color::Red, 1),
        blank(Color::Blue, 2),
        blank(Color::Green, 3),
    ];
    let placements = [
        (CardId(0), Zone::Museum(A)),
        (CardId(1), Zone::Museum(A)),
        (CardId(2), Zone::Hand(B)),
    ];
    let mut position = Position::new(cards, &placements, A).unwrap();
    assert_eq!(position.available_museums(), 3);
    assert_eq!(
        position.claim_museum(B).unwrap_err(),
        Error::Unsupported("no occupied Museum to claim")
    );
    position.zone_mut(Zone::Hand(B)).clear();
    position.zone_mut(Zone::Museum(B)).push(CardId(2));
    position.rebuild_locations();
    assert_eq!(position.locate(CardId(2)), Some(Zone::Museum(B)));
    assert_eq!(position.available_museums(), 2);
    for (zone, viewer) in [(Zone::Museum(A), B), (Zone::Display(B), A)] {
        assert!(position.can_view(zone, viewer));
    }
    position.claim_museum(A).unwrap();
    assert_eq!(position.claimed_museums(A), 1);
    assert_eq!(position.claimed_achievement_count(A), 1);
    assert_eq!(position.achievement_count(B), 0);
    // The awarded Museum still holds its Artifact: no second award, and the
    // supply is unchanged until the returns.
    assert!(position.claim_museum(A).is_err());
    assert_eq!(position.available_museums(), 2);
    assert!(position.check_conservation());
    for zone in [Zone::Museum(A), Zone::Museum(B)] {
        let cards = std::mem::take(position.zone_mut(zone));
        for card in cards {
            let deck = position.card(card).origin_deck();
            position.zone_mut(Zone::Deck(deck)).push(card);
        }
    }
    position.rebuild_locations();
    position.settle_museum_claim();
    assert!(position.check_conservation());
    assert_eq!(position.claimed_museums(A), 1);
    assert_eq!(position.available_museums(), 4);
    assert_eq!(
        position.claim_museum(A),
        Err(Error::Unsupported("no occupied Museum to claim"))
    );
    // Conservation rejects more pieces than exist.
    position.artifacts.as_mut().unwrap().claimed[B.index()] = 5;
    assert!(!position.check_conservation());
}

#[test]
fn unallocated_artifacts_storage_equals_empty_storage() {
    let cards = vec![blank(Color::Red, 1)];
    let placements = [(CardId(0), Zone::Hand(A))];
    let plain = Position::new(cards.clone(), &placements, A).unwrap();
    let mut touched = plain.clone();
    assert!(touched.zone_mut(Zone::Display(B)).is_empty());
    assert!(touched.artifacts.is_some());
    assert_eq!(plain, touched);
    assert!(touched.artifacts.as_ref().unwrap().is_empty());
    assert!(Game::new(touched, Expansions::BASE).is_err());
}

/// Once every Museum is claimed, a showcased Artifact rotates to hand and
/// the check that follows finds nothing to award or return.
#[test]
fn with_every_museum_claimed_the_showcase_rotates_to_hand() {
    let cards = vec![blank(Color::Green, 2)];
    let placements = [(CardId(0), Zone::Display(A))];
    let mut position = Position::new(cards, &placements, A).unwrap();
    position.artifacts.get_or_insert_default().claimed = [3, 2];
    assert_eq!(position.available_museums(), 0);
    assert!(position.check_conservation());
    let mut game = Game::new(position, Expansions::ARTIFACTS).unwrap();
    game.begin_turn(A).unwrap();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(1000, &mut events).unwrap(),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(events.len(), 1);
    let Event::Moved(movement) = &events[0] else {
        panic!("rotation");
    };
    assert_eq!(
        (movement.verb, movement.source, movement.destination),
        (Some(Verb::Rotate), Zone::Display(A), Zone::Hand(A))
    );
    assert_eq!(game.position().claimed_museums(A), 3);
    assert_eq!(game.position().claimed_museums(B), 2);
    assert_eq!(game.position().achievement_count(A), 3);
    assert!(game.position().check_conservation());
}

/// A tied Museum Check awards nothing but still returns every stored Artifact.
#[test]
fn a_tied_museum_check_returns_without_an_award() {
    let cards = vec![
        blank(Color::Green, 2),
        blank(Color::Red, 2),
        blank(Color::Yellow, 2),
        blank(Color::Purple, 5),
    ];
    let placements = [
        (CardId(0), Zone::Display(A)),
        (CardId(1), Zone::Museum(A)),
        (CardId(2), Zone::Museum(B)),
        (CardId(3), Zone::Museum(B)),
    ];
    let mut position = Position::new(cards, &placements, A).unwrap();
    position.artifacts.get_or_insert_default().claimed = [1, 0];
    assert_eq!(position.available_museums(), 1);
    let mut game = Game::new(position, Expansions::ARTIFACTS).unwrap();
    game.begin_turn(A).unwrap();
    let mut events = Vec::new();
    let mut decisions = 0;
    loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(decision) => {
                decisions += 1;
                assert_eq!(decision.player, A);
                // Oldest first: B's 2, then A's 2, then the showcased 2.
                let wanted = [CardId(2), CardId(1), CardId(0)];
                let index = wanted
                    .into_iter()
                    .find_map(|card| {
                        decision.options.iter().position(|o| {
                            matches!(o, Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. } if *c == card)
                        })
                    })
                    .unwrap();
                game.choose(index).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                break;
            }
        }
        assert!(game.position().check_conservation());
    }
    assert_eq!(decisions, 2);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::MuseumClaimed { .. }))
    );
    let deck = |age| {
        Zone::Deck(Deck {
            set: CardSet::Artifacts,
            age,
        })
    };
    assert_eq!(
        game.position().cards(deck(2)),
        &[CardId(0), CardId(1), CardId(2)]
    );
    assert_eq!(game.position().cards(deck(5)), &[CardId(3)]);
    assert_eq!(game.position().claimed_museums(A), 1);
    assert_eq!(game.position().available_museums(), 4);
}
