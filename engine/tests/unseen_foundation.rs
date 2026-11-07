//! Unseen metadata import: the printed catalog is complete and reserved
//! identities hold, while unauthored programs and games fail explicitly.
use std::collections::BTreeMap;

use inno_engine::*;

mod common;
use common::*;

#[test]
fn printed_catalog_matches_the_publisher_decks_and_reserved_identities() {
    assert_eq!(UNSEEN_METADATA.len(), 115);
    assert!(UNSEEN_METADATA.windows(2).all(|w| w[0].code < w[1].code));
    let mut per_age = BTreeMap::new();
    let mut per_color = BTreeMap::new();
    for metadata in &UNSEEN_METADATA {
        assert_eq!(metadata.set, CardSet::Unseen);
        *per_age.entry(metadata.value).or_insert(0) += 1;
        *per_color.entry(metadata.color as u8).or_insert(0) += 1;
        let image = metadata
            .symbols
            .iter()
            .filter(|&&s| s == Symbol::Image)
            .count();
        assert_eq!(image, 1, "{} has one image slot", metadata.code);
        assert!(
            metadata.symbols.iter().all(|&s| s != Symbol::Blank),
            "{} has no blank position",
            metadata.code
        );
        assert!(
            metadata.featured.is_some(),
            "{} features an icon",
            metadata.code
        );
        let id = StableId::from_code(metadata.code).unwrap();
        assert!((345..460).contains(&id.0), "{}", metadata.code);
    }
    assert_eq!(per_age[&1], 15);
    assert!((2..=11).all(|age| per_age[&age] == 10));
    assert!(per_color.values().all(|&count| count == 23));
    assert_eq!(title("U001"), Some("ASSASSINATION"));
    let assassination = unseen_metadata("U001").unwrap();
    assert_eq!(assassination.featured, Some(Icon::Prosperity));
    assert_eq!(
        assassination.symbols,
        [
            Symbol::Icon(Icon::Prosperity),
            Symbol::Image,
            Symbol::Icon(Icon::Prosperity),
            Symbol::Icon(Icon::Authority),
        ]
    );
}

#[test]
fn unauthored_programs_and_rules_fail_explicitly() {
    assert_eq!(
        unseen_card("U999").unwrap_err(),
        Error::InvalidSetup("unknown Unseen card")
    );
    // The language hard cases are authored; every other card is not yet.
    let authored: Vec<String> = (1..=115).map(|i| format!("U{i:03}")).collect();
    for metadata in &UNSEEN_METADATA {
        if authored.iter().any(|code| code == metadata.code) {
            assert!(unseen_card(metadata.code).is_ok(), "{}", metadata.code);
            continue;
        }
        assert_eq!(
            unseen_card(metadata.code).unwrap_err(),
            Error::Unsupported("Unseen program not authored"),
            "{}",
            metadata.code
        );
    }
    // No rules profile includes Unseen cards yet.
    assert!(!Expansions::BASE.includes(CardSet::Unseen));
    assert!(
        !Expansions::ARTIFACTS
            .union(Expansions::ECHOES)
            .union(Expansions::FIGURES)
            .union(Expansions::CITIES)
            .includes(CardSet::Unseen)
    );
}

/// Safe storage: ordered, hidden from its owner too, capacity by best splay,
/// conserved and clone-equal; a Safe outside Unseen rules is rejected.
#[test]
fn safes_are_ordered_hidden_capacity_limited_zones() {
    let mut f = Fixture::default();
    let first = f.tech(Color::Red, 3, Zone::Safe(A), vec![]);
    let second = f.tech(Color::Blue, 1, Zone::Safe(A), vec![]);
    let theirs = f.tech(Color::Green, 2, Zone::Safe(B), vec![]);
    for color in Color::ALL {
        f.tech(color, 2, Zone::Board(A, color), vec![]);
    }
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Yellow, 1, Zone::Hand(A), vec![]);
    let position = f.position();
    assert_eq!(
        Game::new(position.clone(), Expansions::BASE).unwrap_err(),
        Error::InvalidSetup("a Safe without Unseen")
    );
    let game = Game::new(position, Expansions::UNSEEN).unwrap();
    let position = game.position();
    assert_eq!(position.cards(Zone::Safe(A)), &[first, second]);
    assert_eq!(position.cards(Zone::Safe(B)), &[theirs]);
    assert!(position.check_conservation());
    for viewer in Player::ALL {
        for (card, zone) in [
            (first, Zone::Safe(A)),
            (second, Zone::Safe(A)),
            (theirs, Zone::Safe(B)),
        ] {
            assert!(
                matches!(
                    position.descriptor(target(card, zone), viewer),
                    TargetDescriptor::Hidden { .. }
                ),
                "{viewer:?} must not see {card:?} in {zone:?}"
            );
        }
    }
    assert_eq!(position.safe_limit(A), 5);
    let splayed = position
        .clone()
        .with_splay(A, Color::Red, Splay::Up)
        .unwrap();
    assert_eq!(splayed.safe_limit(A), 2);
    assert_eq!(splayed.forecast_limit(A), 2);
    assert_eq!(game.clone().position(), position);
}

#[test]
fn unseen_games_start_with_shuffled_unseen_decks_empty_safes_and_five_specials() {
    let game = Game::new_game(1, Expansions::UNSEEN).unwrap();
    let position = game.position();
    assert_eq!(Expansions::UNSEEN.count(), 1);
    assert!(Expansions::UNSEEN.includes(CardSet::Unseen));
    assert!(!Expansions::UNSEEN.includes(CardSet::Artifacts));
    assert_eq!(Expansions::UNSEEN.index(), 16);
    for age in 1..=11 {
        let deck = position.cards(Zone::Deck(Deck {
            set: CardSet::Unseen,
            age,
        }));
        assert!(!deck.is_empty(), "age {age}");
        assert!(
            deck.iter()
                .all(|&c| position.card(c).set == CardSet::Unseen)
        );
    }
    for player in Player::ALL {
        assert!(position.cards(Zone::Safe(player)).is_empty());
        assert_eq!(position.cards(Zone::Hand(player)).len(), 2);
    }
    for special in Special::UNSEEN {
        assert_eq!(
            position.special_location(special),
            SpecialLocation::Available
        );
    }
    assert_eq!(position.cards(Zone::AvailableAchievements).len(), 10);
}
