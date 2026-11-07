//! Cities representation and queries: the printed catalog,
//! six-position visibility, Search icons, bonuses, flag and fountain
//! achievements and the event-earned special tokens. Cities games are not
//! constructible yet, so these build explicit positions and splay their
//! piles directly.

use crate::*;

const A: Player = Player::One;
const B: Player = Player::Two;

#[derive(Default)]
struct Layout {
    cards: Vec<Card>,
    placements: Vec<(CardId, Zone)>,
    splays: Vec<(Player, Color, Splay)>,
}

impl Layout {
    fn add(&mut self, card: Card, zone: Zone) -> CardId {
        let id = CardId(self.cards.len() as u16);
        self.cards.push(card);
        self.placements.push((id, zone));
        id
    }

    /// A City on top of (or, added first, under) a player's pile.
    fn city(&mut self, code: &str, player: Player) -> CardId {
        let card = city_card(code).unwrap();
        let zone = Zone::Board(player, card.color);
        self.add(card, zone)
    }

    /// A blank Base card in a player's pile.
    fn blank(&mut self, player: Player, color: Color) -> CardId {
        self.add(
            Card {
                code: "synthetic-base-program",
                set: CardSet::Base,
                color,
                value: 1,
                figure: None,
                echo: None,
                symbols: [Symbol::Blank; SLOTS],
                featured: None,
                effects: Vec::new(),
            },
            Zone::Board(player, color),
        )
    }

    fn splay(&mut self, player: Player, color: Color, splay: Splay) {
        self.splays.push((player, color, splay));
    }

    fn position(self, expansions: Expansions) -> Position {
        let mut position = Position::new(self.cards, &self.placements, A).unwrap();
        for (player, color, splay) in self.splays {
            assert!(position.cards(Zone::Board(player, color)).len() > 1);
            position.players[player.index()].splays[color as usize] = splay;
        }
        position.with_expansions(expansions)
    }
}

fn cities(layout: Layout) -> Position {
    layout.position(Expansions::CITIES)
}

#[test]
fn catalog_matches_the_audited_census() {
    assert_eq!(CITY_METADATA.len(), 115);
    let mut ages = [0; 12];
    let mut kinds = std::collections::BTreeMap::new();
    let mut bonuses = [0; 4];
    let mut bonus_values = std::collections::BTreeSet::new();
    let mut without_city_icons = 0;
    for metadata in &CITY_METADATA {
        ages[usize::from(metadata.value)] += 1;
        // Every City image is at top-right, and only there.
        assert_eq!(metadata.symbols[5], Symbol::Image, "{}", metadata.code);
        assert!(!metadata.symbols[..5].contains(&Symbol::Image));
        let mut count = 0;
        let mut special = false;
        for symbol in metadata.symbols {
            let kind = match symbol {
                Symbol::City(icon) => format!("{icon:?}"),
                Symbol::Search(_) => "Search".to_owned(),
                Symbol::Bonus(value) => {
                    count += 1;
                    bonus_values.insert(value);
                    continue;
                }
                _ => continue,
            };
            special = true;
            *kinds.entry(kind).or_insert(0) += 1;
        }
        bonuses[count] += 1;
        without_city_icons += usize::from(!special);
    }
    assert_eq!(ages, [0, 15, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10]);
    let expected = [
        ("Flag", 11),
        ("Fountain", 11),
        ("Junk", 10),
        ("Left", 10),
        ("Plus", 35),
        ("Right", 10),
        ("Search", 19),
        ("Unsplay", 10),
        ("Up", 10),
        ("Uplift", 9),
    ];
    assert_eq!(
        kinds
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect::<Vec<_>>(),
        expected
    );
    // 68 without bonuses, 32 with one, 15 with three: 77 occurrences, 2-12.
    assert_eq!(bonuses, [68, 32, 0, 15]);
    assert_eq!(bonus_values, (2..=12).collect());
    assert_eq!(without_city_icons, 15);
}

#[test]
fn cities_have_reserved_identities_and_backs_and_no_dogma() {
    let mut seen = std::collections::BTreeSet::new();
    for metadata in &CITY_METADATA {
        let card = city_card(metadata.code).unwrap();
        let id = card.stable_id().unwrap().0;
        assert!((230..345).contains(&id) && seen.insert(id), "{}", card.code);
        assert_eq!(card.origin_deck(), Deck::cities(card.value));
        assert_eq!(card.printed().len(), SLOTS);
        assert!(card.featured.is_none() && card.effects.is_empty());
    }
    assert_eq!(seen.len(), 115);
    assert_eq!(city_card("C116"), Err(Error::InvalidSetup("unknown City")));
    // Milan, Lisbon and Bangalore keep their printed icon order.
    let symbols = |code| city_card(code).unwrap().symbols;
    assert_eq!(symbols("C036")[2], Symbol::City(CityIcon::Uplift));
    assert_eq!(symbols("C036")[4], Symbol::City(CityIcon::Plus));
    assert_eq!(symbols("C039")[2], Symbol::City(CityIcon::Plus));
    assert_eq!(symbols("C039")[4], Symbol::City(CityIcon::Plus));
    assert_eq!(symbols("C103")[2], Symbol::City(CityIcon::Up));
    assert_eq!(symbols("C103")[4], Symbol::City(CityIcon::Unsplay));
    assert_eq!(symbols("C035")[4], Symbol::Search(Icon::Health));

    // A top City is never a Dogma source.
    let mut layout = Layout::default();
    let mecca = layout.city("C035", A);
    let position = cities(layout);
    assert_eq!(position.top(A, Color::Purple), Some(mecca));
    assert_eq!(position.dogma_source_owner(A, mecca), None);
}

#[test]
fn covered_slots_follow_the_six_position_geometry() {
    // Rows: None, Left, Right, Up, Aslant. Columns: top-left, bottom-left,
    // bottom-center, bottom-right, top-center, top-right.
    let shown = [
        [false, false, false, false, false, false],
        [false, false, false, true, false, true],
        [true, true, false, false, false, false],
        [false, true, true, true, false, false],
        [true, true, true, true, false, false],
    ];
    for (splay, row) in [
        Splay::None,
        Splay::Left,
        Splay::Right,
        Splay::Up,
        Splay::Aslant,
    ]
    .into_iter()
    .zip(shown)
    {
        for (slot, visible) in row.into_iter().enumerate() {
            assert_eq!(splay.reveals(slot), visible, "{splay:?} {slot}");
        }
    }
    // Four-position cards neither have nor count the City positions.
    let base = base_card("B001").unwrap();
    assert_eq!(base.printed().len(), 4);
    assert_eq!(base.symbols[4..], [Symbol::Blank; 2]);
}

#[test]
fn search_icons_count_as_their_printed_standard_icon() {
    // Mecca: Health top-left and bottom-right, Search(Health) top-center.
    let mecca = city_card("C035").unwrap();
    assert_eq!(mecca.printed_icon_count(Icon::Health), 3);
    assert_eq!(mecca.printed_icon_count(Icon::Prosperity), 1);

    let mut layout = Layout::default();
    layout.city("C035", A);
    let position = cities(layout);
    assert_eq!(position.icon_count(A, Icon::Health), 3);
    assert!(position.color_has_icon(A, Color::Purple, Icon::Health));

    // Covered, the top-center Search icon never shows.
    for (splay, health) in [(Splay::Up, 1), (Splay::Aslant, 2), (Splay::Left, 1)] {
        let mut layout = Layout::default();
        layout.city("C035", A);
        layout.blank(A, Color::Purple);
        layout.splay(A, Color::Purple, splay);
        assert_eq!(
            cities(layout).icon_count(A, Icon::Health),
            health,
            "{splay:?}"
        );
    }
}

#[test]
fn bonuses_score_the_highest_plus_one_per_other_visible_bonus() {
    // Melbourne, Toronto and Johannesburg each print three 7s.
    let mut layout = Layout::default();
    layout.city("C068", A);
    let position = cities(layout);
    assert_eq!((position.bonus_score(A), position.score(A)), (9, 9));

    let mut layout = Layout::default();
    for code in ["C068", "C071", "C072"] {
        layout.city(code, A);
    }
    assert_eq!(cities(layout).score(A), 15);

    // Riyadh: three 12s are worth 14, not its age or a score card.
    let mut layout = Layout::default();
    let riyadh = layout.city("C111", A);
    let position = cities(layout);
    assert_eq!(position.card(riyadh).value, 11);
    assert_eq!(position.score(A), 14);
    assert!(position.cards(Zone::Score(A)).is_empty());

    // Covered under Right: only the top-left and bottom-left 7s show.
    let mut layout = Layout::default();
    layout.city("C068", A);
    layout.blank(A, Color::Yellow);
    layout.splay(A, Color::Yellow, Splay::Right);
    assert_eq!(cities(layout).score(A), 8);
}

#[test]
fn flags_hold_while_no_opponent_shows_more_cards_of_that_color() {
    // Algiers (red) prints a Flag at top-center. Cases: B's red pile size,
    // whether B splays it, and whether A splays A's own red pile.
    for (theirs, they_splay, mine_splayed, flags) in [
        (0, false, false, 1),
        (1, false, false, 1),
        (3, false, false, 1),
        (2, true, false, 0),
        (2, true, true, 1),
        (3, true, true, 0),
    ] {
        let mut layout = Layout::default();
        if mine_splayed {
            layout.blank(A, Color::Red);
        }
        layout.city("C046", A);
        for _ in 0..theirs {
            layout.blank(B, Color::Red);
        }
        if they_splay {
            layout.splay(B, Color::Red, Splay::Left);
        }
        if mine_splayed {
            layout.splay(A, Color::Red, Splay::Left);
        }
        let position = cities(layout);
        let case = (theirs, they_splay, mine_splayed);
        assert_eq!(position.city_achievements(A), flags, "{case:?}");
        assert_eq!(position.achievement_count(A), flags, "{case:?}");
        assert_eq!(position.effective_achievement_count(A), flags);
        assert_eq!(position.claimed_achievement_count(A), 0);
        assert_eq!(position.city_achievements(B), 0);
    }

    // A covered Flag never shows, under any splay.
    for splay in [Splay::Left, Splay::Right, Splay::Up, Splay::Aslant] {
        let mut layout = Layout::default();
        layout.city("C046", A);
        layout.blank(A, Color::Red);
        layout.splay(A, Color::Red, splay);
        assert_eq!(cities(layout).city_achievements(A), 0, "{splay:?}");
    }
}

#[test]
fn every_visible_fountain_counts() {
    // Guangzhou (red) prints Fountains at bottom-center and top-center.
    let mut layout = Layout::default();
    layout.city("C106", A);
    assert_eq!(cities(layout).achievement_count(A), 2);

    for (splay, fountains) in [
        (Splay::Up, 1),
        (Splay::Aslant, 1),
        (Splay::Left, 0),
        (Splay::Right, 0),
    ] {
        let mut layout = Layout::default();
        layout.city("C106", A);
        layout.blank(A, Color::Red);
        layout.splay(A, Color::Red, splay);
        assert_eq!(cities(layout).city_achievements(A), fountains, "{splay:?}");
    }

    // Two Fountain cities: Guangzhou on top, Prague covered under Up.
    let mut layout = Layout::default();
    layout.city("C106", A);
    layout.city("C112", A);
    layout.blank(A, Color::Blue);
    layout.splay(A, Color::Blue, Splay::Up);
    let position = cities(layout);
    assert_eq!(position.achievement_count(A), 3);

    // Without the Cities rules, no symbol is an achievement.
    let mut layout = Layout::default();
    layout.city("C106", A);
    assert_eq!(layout.position(Expansions::BASE).achievement_count(A), 0);
}

#[test]
fn polhem_multiplies_by_the_rules_count_including_fountains() {
    let both = Expansions::FIGURES.union(Expansions::CITIES);
    let scores = [Expansions::FIGURES, both].map(|expansions| {
        let mut layout = Layout::default();
        let polhem = figure_card(Figure::ChristopherPolhem.code()).unwrap();
        assert_ne!(polhem.color, Color::Red);
        let zone = Zone::Board(A, polhem.color);
        layout.add(polhem, zone);
        layout.city("C106", A);
        let position = layout.position(expansions);
        let industry = position.basic_icon_count(A, Icon::Industry) as i64;
        (position.score(A), industry, position.achievement_count(A))
    });
    let [(without, industry, 0), (with, _, 2)] = scores else {
        panic!("{scores:?}");
    };
    assert!(industry > 0);
    assert_eq!(with - without, 2 * industry);
}

#[test]
fn cities_tokens_join_the_roster_without_standing_requirements() {
    let mut layout = Layout::default();
    layout.city("C035", A);
    let position = cities(layout);
    let roster: Vec<_> = position.specials().collect();
    assert_eq!(roster.len(), 10);
    assert_eq!(roster[5..], Special::CITIES);
    for (special, index) in Special::CITIES.into_iter().zip(10..) {
        assert_eq!(special.index(), index);
        assert_eq!(special.requirement(), None);
        assert_eq!(
            position.special_location(special),
            SpecialLocation::Available
        );
        // Earned only by events, never by the end-of-action check.
        assert!(!position.eligible_for_special(A, special));
    }
    let base = Position::new(Vec::new(), &[], A).unwrap();
    assert_eq!(base.specials().collect::<Vec<_>>(), Special::ALL);
}

#[test]
fn cities_decks_hold_their_own_backs_and_need_the_cities_rules() {
    let card = city_card("C035").unwrap();
    let deck = Deck::cities(3);
    let position = Position::new(vec![card.clone()], &[(CardId(0), Zone::Deck(deck))], A).unwrap();
    assert_eq!(position.cards(Zone::Deck(deck)), [CardId(0)]);
    assert_eq!(position.locate(CardId(0)), Some(Zone::Deck(deck)));
    assert!(position.check_conservation());
    assert!(position.cards(Zone::Deck(Deck::base(3))).is_empty());
    assert_eq!(
        Position::new(vec![card], &[(CardId(0), Zone::Deck(Deck::base(3)))], A),
        Err(Error::InvalidSetup(
            "wrong board color, origin deck or Age 0 reserve"
        ))
    );

    assert!(Game::new(position.clone(), Expansions::CITIES).is_ok());
    assert_eq!(
        Game::new(position, Expansions::FIGURES).err(),
        Some(Error::InvalidSetup(
            "a disabled expansion's cards, Age 0 or Decrees in the position"
        ))
    );
}

#[test]
fn shortcut_guards_include_city_consequences() {
    use crate::ShortcutDependency::{Move, Splay as SplayDependency};
    // Hedy watches claims: with Cities a meld or junk can claim. Hawking
    // watches draws: with Cities a splay can draw.
    for (figure, dependency) in [
        (Figure::HedyLamarr, Move(Verb::Meld)),
        (Figure::HedyLamarr, Move(Verb::Junk)),
        (Figure::StephenHawking, SplayDependency),
    ] {
        let clear = [
            Expansions::FIGURES,
            Expansions::FIGURES.union(Expansions::CITIES),
        ]
        .map(|expansions| {
            let mut layout = Layout::default();
            let card = figure_card(figure.code()).unwrap();
            let zone = Zone::Board(A, card.color);
            layout.add(card, zone);
            layout
                .position(expansions)
                .dependencies_clear(&[dependency], Context::new(A), false)
        });
        assert_eq!(clear, [true, false], "{figure:?} {dependency:?}");
    }
}
