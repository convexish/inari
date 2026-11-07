//! Echoes representation and queries (port step 1): the printed catalog and
//! its echo positions, the Forecast zone and its limit, and the five Echoes
//! special achievements. These representation scenarios use explicit
//! positions and splay their piles directly, independently of full setup.

use crate::*;

mod bonuses;
mod cards;
mod gaps;
mod groups;

const A: Player = Player::One;
const B: Player = Player::Two;

/// A metadata-only Echoes card for isolated representation scenarios.
fn echoes(code: &str) -> Card {
    let metadata = echoes_metadata(code).unwrap();
    Card {
        code: metadata.code,
        set: CardSet::Echoes,
        color: metadata.color,
        value: metadata.value,
        figure: None,
        echo: None,
        symbols: four_slots(metadata.symbols),
        featured: metadata.featured,
        effects: Vec::new(),
    }
}

/// A Base-set card with these printed symbols and no effects.
fn printed(color: Color, symbols: [Symbol; 4]) -> Card {
    Card {
        code: "synthetic-base-program",
        set: CardSet::Base,
        color,
        value: 1,
        figure: None,
        echo: None,
        symbols: four_slots(symbols),
        featured: None,
        effects: Vec::new(),
    }
}

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

    /// `count` cards with these symbols in a player's pile.
    fn pile(&mut self, player: Player, color: Color, count: usize, symbols: [Symbol; 4]) {
        for _ in 0..count {
            self.add(printed(color, symbols), Zone::Board(player, color));
        }
    }

    fn forecast(&mut self, player: Player, count: usize) {
        for _ in 0..count {
            self.add(
                printed(Color::Red, [Symbol::Blank; 4]),
                Zone::Forecast(player),
            );
        }
    }

    fn figure(&mut self, figure: Figure, player: Player) -> CardId {
        let card = figure_card(figure.code()).unwrap();
        let zone = Zone::Board(player, card.color);
        self.add(card, zone)
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

fn eligible(layout: Layout, expansions: Expansions, special: Special) -> bool {
    layout.position(expansions).eligible_for_special(A, special)
}

#[test]
fn catalog_matches_the_audited_census() {
    assert_eq!(ECHOES_METADATA.len(), 115);
    let mut ages = [0; 12];
    let mut colors = [0; 5];
    let mut echo_count = 0;
    let mut bonuses = Vec::new();
    for metadata in &ECHOES_METADATA {
        assert_eq!(metadata.set, CardSet::Echoes);
        assert!(metadata.featured.is_some(), "{}", metadata.code);
        ages[usize::from(metadata.value)] += 1;
        colors[metadata.color as usize] += 1;
        // One image per card; at most one printed echo.
        let count = |wanted: Symbol| metadata.symbols.iter().filter(|&&s| s == wanted).count();
        assert_eq!(count(Symbol::Image), 1, "{}", metadata.code);
        assert!(count(Symbol::Echo) <= 1, "{}", metadata.code);
        echo_count += count(Symbol::Echo);
        bonuses.extend(metadata.symbols.iter().filter_map(|&s| match s {
            Symbol::Bonus(value) => Some(value),
            _ => None,
        }));
        let card = echoes(metadata.code);
        assert_eq!(
            card.origin_deck(),
            Deck {
                set: CardSet::Echoes,
                age: metadata.value
            }
        );
        let id = card.stable_id().unwrap();
        assert!((115..230).contains(&id.0), "{}", metadata.code);
    }
    assert_eq!(ages, [0, 15, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10]);
    assert_eq!(colors, [23; 5]);
    // 63 echo clauses; 69 bonuses spanning 1-12 (Esports and Meritocracy show 12).
    assert_eq!(echo_count, 63);
    assert_eq!(bonuses.len(), 69);
    assert_eq!(bonuses.iter().min(), Some(&1));
    assert_eq!(bonuses.iter().filter(|&&value| value == 12).count(), 2);
    assert!(ECHOES_METADATA.windows(2).all(|w| w[0].code < w[1].code));
}

#[test]
fn printed_layouts_keep_spatial_order() {
    let bangle = echoes_metadata("E001").unwrap();
    assert_eq!(
        bangle.symbols,
        [
            Symbol::Image,
            Symbol::Icon(Icon::Authority),
            Symbol::Echo,
            Symbol::Bonus(1),
        ]
    );
    assert_eq!(bangle.featured, Some(Icon::Authority));
    // Ruler's source entry omits bonuses: it has none.
    let ruler = echoes_metadata("E012").unwrap();
    assert!(!ruler.symbols.iter().any(|s| matches!(s, Symbol::Bonus(_))));
    assert!(ruler.symbols.contains(&Symbol::Echo));
}

#[test]
fn forecast_is_private_storage_counted_by_conservation() {
    let mut layout = Layout::default();
    let held = layout.add(echoes("E001"), Zone::Forecast(A));
    layout.add(echoes("E012"), Zone::Hand(B));
    let position = layout.position(Expansions::ECHOES);
    assert_eq!(position.cards(Zone::Forecast(A)), [held]);
    assert!(position.cards(Zone::Forecast(B)).is_empty());
    assert!(position.can_view(Zone::Forecast(A), A));
    assert!(!position.can_view(Zone::Forecast(A), B));
    assert_eq!(position.locate(held), Some(Zone::Forecast(A)));
    assert!(position.check_conservation());

    // Allocated-but-empty storage equals storage never allocated.
    let mut layout = Layout::default();
    let card = layout.add(echoes("E001"), Zone::Hand(A));
    let fresh = layout.position(Expansions::ECHOES);
    let mut moved = fresh.clone();
    moved.zone_mut(Zone::Hand(A)).retain(|&c| c != card);
    moved.zone_mut(Zone::Forecast(B)).push(card);
    assert_ne!(moved, fresh);
    moved.zone_mut(Zone::Forecast(B)).clear();
    moved.zone_mut(Zone::Hand(A)).push(card);
    assert_eq!(moved, fresh);
}

#[test]
fn forecast_limit_follows_the_best_splay() {
    let limit = |splays: &[(Color, Splay)]| {
        let mut layout = Layout::default();
        for color in Color::ALL {
            layout.pile(A, color, 2, [Symbol::Blank; 4]);
        }
        for &(color, splay) in splays {
            layout.splay(A, color, splay);
        }
        layout.position(Expansions::ECHOES).forecast_limit(A)
    };
    assert_eq!(limit(&[]), 5);
    assert_eq!(limit(&[(Color::Red, Splay::Left)]), 4);
    assert_eq!(limit(&[(Color::Red, Splay::Right)]), 3);
    assert_eq!(limit(&[(Color::Red, Splay::Up)]), 2);
    assert_eq!(limit(&[(Color::Red, Splay::Aslant)]), 1);
    assert_eq!(
        limit(&[(Color::Red, Splay::Left), (Color::Blue, Splay::Up)]),
        2
    );
}

#[test]
fn echoes_specials_join_the_roster_in_reserved_slots() {
    let roster = |expansions| {
        Layout::default()
            .position(expansions)
            .specials()
            .collect::<Vec<_>>()
    };
    assert_eq!(roster(Expansions::BASE), Special::ALL);
    assert_eq!(
        roster(Expansions::ECHOES),
        [Special::ALL, Special::ECHOES].concat()
    );
    assert_eq!(
        roster(Expansions::ECHOES.union(Expansions::CITIES)),
        [Special::ALL, Special::ECHOES, Special::CITIES].concat()
    );
    let slots: Vec<_> = Special::ECHOES.iter().map(|s| s.index()).collect();
    assert_eq!(slots, [5, 6, 7, 8, 9]);
}

#[test]
fn destiny_counts_forecast_cards() {
    let destiny = |cards| {
        let mut layout = Layout::default();
        layout.forecast(A, cards);
        eligible(layout, Expansions::ECHOES, Special::Destiny)
    };
    assert!(!destiny(4));
    assert!(destiny(5));
}

#[test]
fn wealth_counts_every_visible_bonus() {
    let wealth = |splay| {
        let mut layout = Layout::default();
        // Bonuses at bottom-right: a right splay covers them, up shows them.
        layout.pile(
            A,
            Color::Red,
            4,
            [
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Bonus(3),
            ],
        );
        layout.pile(
            A,
            Color::Blue,
            4,
            [
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Bonus(3),
            ],
        );
        layout.splay(A, Color::Red, splay);
        layout.splay(A, Color::Blue, splay);
        eligible(layout, Expansions::ECHOES, Special::Wealth)
    };
    assert!(!wealth(Splay::Right));
    assert!(wealth(Splay::Up));
}

#[test]
fn heritage_and_history_count_one_color() {
    let in_one_color = |symbol, count, special| {
        let mut layout = Layout::default();
        // Bottom-left shows under Right, Up and Aslant.
        layout.pile(
            A,
            Color::Green,
            count,
            [Symbol::Blank, symbol, Symbol::Blank, Symbol::Blank],
        );
        layout.splay(A, Color::Green, Splay::Up);
        eligible(layout, Expansions::ECHOES, special)
    };
    assert!(!in_one_color(Symbol::Image, 7, Special::Heritage));
    assert!(in_one_color(Symbol::Image, 8, Special::Heritage));
    assert!(!in_one_color(Symbol::Echo, 3, Special::History));
    assert!(in_one_color(Symbol::Echo, 4, Special::History));

    // Spread over two colors, eight images don't qualify.
    let mut layout = Layout::default();
    for color in [Color::Green, Color::Purple] {
        layout.pile(
            A,
            color,
            4,
            [Symbol::Blank, Symbol::Image, Symbol::Blank, Symbol::Blank],
        );
        layout.splay(A, color, Splay::Up);
    }
    assert!(!eligible(layout, Expansions::ECHOES, Special::Heritage));
}

#[test]
fn supremacy_needs_one_icon_type_three_times_in_four_colors() {
    let supremacy = |colors: &[Color], symbols: [Symbol; 4]| {
        let mut layout = Layout::default();
        for &color in colors {
            layout.pile(A, color, 1, symbols);
        }
        eligible(layout, Expansions::ECHOES, Special::Supremacy)
    };
    let four = [Color::Red, Color::Blue, Color::Green, Color::Yellow];
    let concepts = [
        Symbol::Icon(Icon::Concept),
        Symbol::Icon(Icon::Concept),
        Symbol::Icon(Icon::Concept),
        Symbol::Image,
    ];
    assert!(supremacy(&four, concepts));
    assert!(!supremacy(&four[..3], concepts));
    // Bonuses are one type whatever their values.
    let bonuses = [
        Symbol::Bonus(1),
        Symbol::Bonus(4),
        Symbol::Bonus(9),
        Symbol::Image,
    ];
    assert!(supremacy(&four, bonuses));
    // Each City icon kind is its own type.
    let flags = [
        Symbol::City(CityIcon::Flag),
        Symbol::City(CityIcon::Flag),
        Symbol::City(CityIcon::Flag),
        Symbol::Image,
    ];
    assert!(supremacy(&four, flags));
    // Mixed types never combine.
    let mixed = [
        Symbol::Icon(Icon::Concept),
        Symbol::Bonus(2),
        Symbol::City(CityIcon::Flag),
        Symbol::Image,
    ];
    assert!(!supremacy(&four, mixed));
}

#[test]
fn hedy_lowers_every_echoes_requirement() {
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let destiny = |hedy| {
        let mut layout = Layout::default();
        layout.forecast(A, 4);
        if hedy {
            layout.figure(Figure::HedyLamarr, A);
        }
        eligible(layout, both, Special::Destiny)
    };
    assert!(!destiny(false));
    assert!(destiny(true));

    // Two of a type in three colors, with Hedy on a fourth.
    let supremacy = |hedy| {
        let mut layout = Layout::default();
        for color in [Color::Red, Color::Blue, Color::Yellow] {
            layout.pile(
                A,
                color,
                1,
                [
                    Symbol::Icon(Icon::Health),
                    Symbol::Icon(Icon::Health),
                    Symbol::Image,
                    Symbol::Blank,
                ],
            );
        }
        if hedy {
            layout.figure(Figure::HedyLamarr, A);
        }
        eligible(layout, both, Special::Supremacy)
    };
    assert!(!supremacy(false));
    assert!(supremacy(true));
}

#[test]
fn figure_additions_never_count_toward_supremacy() {
    // Two of an icon in each of four colors. Each additions raise the board's totals
    // but are not symbols on the board, so no color reaches three.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let cases = [
        (Figure::Gilgamesh, Icon::Authority),
        (Figure::CatherineTheGreat, Icon::Concept),
        (Figure::AdamSmith, Icon::Prosperity),
        (Figure::TranHuangDao, Icon::Health),
    ];
    for (figure, icon) in cases {
        // Authority elsewhere feeds Tran's pairs without adding a third.
        let other = if icon == Icon::Authority {
            Symbol::Blank
        } else {
            Symbol::Icon(Icon::Authority)
        };
        let mut layout = Layout::default();
        for color in [Color::Blue, Color::Green, Color::Yellow, Color::Purple] {
            layout.pile(
                A,
                color,
                1,
                [Symbol::Icon(icon), Symbol::Icon(icon), other, Symbol::Image],
            );
        }
        layout.figure(figure, A);
        let position = layout.position(both);
        assert!(position.icon_count(A, icon) > 8, "{figure:?}");
        assert!(
            !position.eligible_for_special(A, Special::Supremacy),
            "{figure:?}"
        );
    }
}

#[test]
fn a_forecast_needs_echoes() {
    let mut layout = Layout::default();
    layout.forecast(B, 1);
    assert_eq!(
        Game::new(layout.position(Expansions::BASE), Expansions::BASE).err(),
        Some(Error::InvalidSetup("a Forecast without Echoes"))
    );
}

/// A blank card of this set and age in its own deck.
fn in_deck(layout: &mut Layout, set: CardSet, age: u8) -> CardId {
    let mut card = printed(Color::Green, [Symbol::Blank; 4]);
    card.set = set;
    card.value = age;
    layout.add(card, Zone::Deck(Deck { set, age }))
}

/// A blank top card of this value.
fn top(layout: &mut Layout, player: Player, color: Color, value: u8) -> CardId {
    let mut card = printed(color, [Symbol::Blank; 4]);
    card.value = value;
    layout.add(card, Zone::Board(player, color))
}

/// Run to completion, taking the first option at each decision.
fn finish(game: &mut Game) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => break,
            status => panic!("unexpected {status:?}"),
        }
    }
    assert!(game.position().check_conservation());
    events
}

/// The card A takes from this draw request.
fn drawn(layout: Layout, expansions: Expansions, value: u8, set: Option<CardSet>) -> CardId {
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    game.start(Operation::Draw { value, set }, Context::new(A))
        .unwrap();
    finish(&mut game);
    *game.position().cards(Zone::Hand(A)).last().unwrap()
}

#[test]
fn ordinary_draws_come_from_echoes_with_a_single_highest_top() {
    let route = |tops: &[(Color, u8)]| {
        let mut layout = Layout::default();
        for &(color, value) in tops {
            top(&mut layout, A, color, value);
        }
        let base = in_deck(&mut layout, CardSet::Base, 3);
        let echoes = in_deck(&mut layout, CardSet::Echoes, 3);
        let card = drawn(layout, Expansions::ECHOES, 3, None);
        (card == echoes, card == base)
    };
    assert_eq!(route(&[(Color::Red, 3), (Color::Blue, 1)]), (true, false));
    assert_eq!(route(&[(Color::Red, 3), (Color::Blue, 3)]), (false, true));
    assert_eq!(route(&[]), (false, true));
}

#[test]
fn echoes_draws_skip_empty_base_ages_and_fall_back_to_base() {
    // The Echoes 3 deck is empty: Base 3, not a later Echoes deck.
    let mut layout = Layout::default();
    top(&mut layout, A, Color::Red, 3);
    let base = in_deck(&mut layout, CardSet::Base, 3);
    in_deck(&mut layout, CardSet::Echoes, 4);
    in_deck(&mut layout, CardSet::Base, 4);
    assert_eq!(drawn(layout, Expansions::ECHOES, 3, None), base);

    // The Base 3 deck is empty: age 3 is skipped though Echoes 3 has cards.
    let mut layout = Layout::default();
    top(&mut layout, A, Color::Red, 3);
    in_deck(&mut layout, CardSet::Echoes, 3);
    in_deck(&mut layout, CardSet::Base, 4);
    let echoes = in_deck(&mut layout, CardSet::Echoes, 4);
    assert_eq!(drawn(layout, Expansions::ECHOES, 3, None), echoes);
}

#[test]
fn explicit_set_draws_are_never_redirected() {
    // A named Echoes draw with tied tops.
    let mut layout = Layout::default();
    top(&mut layout, A, Color::Red, 3);
    top(&mut layout, A, Color::Blue, 3);
    in_deck(&mut layout, CardSet::Base, 3);
    let echoes = in_deck(&mut layout, CardSet::Echoes, 3);
    assert_eq!(
        drawn(layout, Expansions::ECHOES, 3, Some(CardSet::Echoes)),
        echoes
    );

    // A named Figures draw with a single highest top.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    top(&mut layout, A, Color::Red, 3);
    in_deck(&mut layout, CardSet::Base, 3);
    in_deck(&mut layout, CardSet::Echoes, 3);
    let figure = layout.add(
        figure_card(Figure::TranHuangDao.code()).unwrap(),
        Zone::Deck(Deck::figures(3)),
    );
    assert_eq!(drawn(layout, both, 3, Some(CardSet::Figures)), figure);
}

#[test]
fn a_first_karma_that_changes_the_tops_changes_the_set() {
    // Tied tops (Tran Huang Dao and a blue 3) make an ordinary draw Base.
    // Tran first scores himself as a top red card below 4, leaving one
    // highest top, so the same draw comes from Echoes.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    let tran = layout.figure(Figure::TranHuangDao, A);
    top(&mut layout, A, Color::Blue, 3);
    in_deck(&mut layout, CardSet::Base, 3);
    let echoes = in_deck(&mut layout, CardSet::Echoes, 3);
    let mut game = Game::new(layout.position(both), both).unwrap();
    game.start(
        Operation::Draw {
            value: 3,
            set: None,
        },
        Context::new(A),
    )
    .unwrap();
    finish(&mut game);
    assert_eq!(game.position().cards(Zone::Score(A)), [tran]);
    assert_eq!(game.position().cards(Zone::Hand(A)), [echoes]);
}

#[test]
fn a_draw_action_follows_the_rule_and_its_end_claims_wealth() {
    let mut layout = Layout::default();
    // Eight visible bonuses, and a single highest top (the red 2).
    for (color, value) in [(Color::Red, 2), (Color::Blue, 1)] {
        for _ in 0..4 {
            let mut card = printed(
                color,
                [
                    Symbol::Blank,
                    Symbol::Blank,
                    Symbol::Blank,
                    Symbol::Bonus(1),
                ],
            );
            card.value = value;
            layout.add(card, Zone::Board(A, color));
        }
        layout.splay(A, color, Splay::Up);
    }
    in_deck(&mut layout, CardSet::Base, 2);
    let echoes = in_deck(&mut layout, CardSet::Echoes, 2);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    finish(&mut game);
    assert_eq!(game.position().cards(Zone::Hand(A)), [echoes]);
    assert_eq!(
        game.position().special_location(Special::Wealth),
        SpecialLocation::Owned(A)
    );
}

/// A top card whose single effect is this program.
fn program(layout: &mut Layout, instructions: Vec<Instruction>) -> CardId {
    let mut card = printed(Color::Purple, [Symbol::Blank; 4]);
    card.effects = vec![Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }];
    layout.add(card, Zone::Board(A, Color::Purple))
}

/// Self-execute A's program card; whether A won.
fn run_program(layout: Layout, card: CardId) -> (Game, bool) {
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let won = loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => break false,
            Status::Winner(winner) => break winner == A,
            status => panic!("unexpected {status:?}"),
        }
    };
    assert!(game.position().check_conservation());
    (game, won)
}

/// Draw a [1] and foreshadow it; win if the foreshadow happened.
fn draw_and_foreshadow() -> Vec<Instruction> {
    vec![
        program::build::draw(1),
        program::build::foreshadow(program::build::LAST),
        program::build::if_(program::build::DID, 1),
        Instruction::Win(WinRule::Executor),
    ]
}

#[test]
fn draw_and_foreshadow_needs_room() {
    for held in [0, 4, 5] {
        let mut layout = Layout::default();
        let card = program(&mut layout, draw_and_foreshadow());
        layout.forecast(A, held);
        let drawn = in_deck(&mut layout, CardSet::Base, 1);
        let (game, won) = run_program(layout, card);
        let position = game.position();
        // A full Forecast refuses the card; the draw still happened.
        assert_eq!(won, held < 5, "{held} held");
        let home = if held < 5 {
            Zone::Forecast(A)
        } else {
            Zone::Hand(A)
        };
        assert_eq!(position.locate(drawn), Some(home), "{held} held");
    }
}

#[test]
fn a_lower_limit_keeps_cards_but_refuses_new_ones() {
    let mut layout = Layout::default();
    let card = program(&mut layout, draw_and_foreshadow());
    layout.pile(A, Color::Red, 2, [Symbol::Blank; 4]);
    layout.splay(A, Color::Red, Splay::Up);
    layout.forecast(A, 3);
    let drawn = in_deck(&mut layout, CardSet::Base, 1);
    let (game, won) = run_program(layout, card);
    assert!(!won);
    assert_eq!(game.position().cards(Zone::Forecast(A)).len(), 3);
    assert_eq!(game.position().cards(Zone::Hand(A)), [drawn]);
}

#[test]
fn transfers_into_a_forecast_obey_its_owners_limit() {
    // Liquid Fire's shape: a transfer into the opponent's Forecast.
    for held in [4, 5] {
        let mut layout = Layout::default();
        let card = program(
            &mut layout,
            vec![
                program::build::transfer(
                    program::build::HAND,
                    program::Destination::Forecast(Who::Opponent),
                ),
                program::build::if_(program::build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        let moved = layout.add(printed(Color::Red, [Symbol::Blank; 4]), Zone::Hand(A));
        layout.forecast(B, held);
        let (game, won) = run_program(layout, card);
        assert_eq!(won, held == 4);
        let home = if held == 4 {
            Zone::Forecast(B)
        } else {
            Zone::Hand(A)
        };
        assert_eq!(game.position().locate(moved), Some(home));
    }
}

#[test]
fn foreshadowing_any_number_ends_when_the_forecast_fills() {
    let mut layout = Layout::default();
    let card = program(
        &mut layout,
        vec![program::build::foreshadow(program::build::HAND).up_to(None)],
    );
    for _ in 0..3 {
        layout.add(printed(Color::Red, [Symbol::Blank; 4]), Zone::Hand(A));
    }
    layout.forecast(A, 4);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.self_execute(card, Context::new(A)).unwrap();
    let mut events = Vec::new();
    let mut decisions = 0;
    loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(_) => {
                decisions += 1;
                game.choose(0).unwrap();
            }
            Status::Complete(_) => break,
            status => panic!("unexpected {status:?}"),
        }
    }
    // One choice fills the Forecast; no pointless menu follows.
    assert_eq!(decisions, 1);
    assert_eq!(game.position().cards(Zone::Forecast(A)).len(), 5);
    assert_eq!(game.position().cards(Zone::Hand(A)).len(), 2);
}

#[test]
fn a_mandatory_foreshadow_of_all_fails_if_any_card_is_refused() {
    for held in [3, 4] {
        let mut layout = Layout::default();
        let card = program(
            &mut layout,
            vec![
                program::build::foreshadow(program::build::HAND).all(),
                program::build::if_(program::build::DID, 1),
                Instruction::Win(WinRule::Executor),
            ],
        );
        for _ in 0..2 {
            layout.add(printed(Color::Red, [Symbol::Blank; 4]), Zone::Hand(A));
        }
        layout.forecast(A, held);
        let (game, won) = run_program(layout, card);
        // Room for both: all foreshadowed. Room for one: the other stays.
        assert_eq!(won, held == 3);
        assert_eq!(game.position().cards(Zone::Hand(A)).len(), held - 3);
    }
}

/// A card with an echo at bottom-left (shown by Right, Up and Aslant splays,
/// not Left) and these dogma effects, featuring Concept.
fn echo_card(color: Color, echo: Vec<Instruction>, dogmas: Vec<Vec<Instruction>>) -> Card {
    let mut card = printed(
        color,
        [
            Symbol::Icon(Icon::Concept),
            Symbol::Echo,
            Symbol::Icon(Icon::Concept),
            Symbol::Image,
        ],
    );
    card.featured = Some(Icon::Concept);
    card.echo = Some(Box::new(Effect {
        kind: EffectKind::NonDemand,
        instructions: echo,
    }));
    card.effects = dogmas
        .into_iter()
        .map(|instructions| Effect {
            kind: EffectKind::NonDemand,
            instructions,
        })
        .collect();
    card
}

fn draw_box(age: u8) -> Instruction {
    program::build::draw(age)
}

/// One Base card per age, so a draw's age names the effect that drew it.
fn ages(layout: &mut Layout, ages: std::ops::RangeInclusive<u8>) -> Vec<CardId> {
    ages.map(|age| in_deck(layout, CardSet::Base, age))
        .collect()
}

/// A purple pile: bottom (echo draws a 1), middle (echo draws a 2) and top
/// (echo draws a 3, dogma draws a 4).
fn echo_pile(layout: &mut Layout, bottom_echo: Vec<Instruction>) -> [CardId; 3] {
    let bottom = layout.add(
        echo_card(Color::Purple, bottom_echo, vec![vec![draw_box(6)]]),
        Zone::Board(A, Color::Purple),
    );
    let middle = layout.add(
        echo_card(Color::Purple, vec![draw_box(2)], Vec::new()),
        Zone::Board(A, Color::Purple),
    );
    let top = layout.add(
        echo_card(Color::Purple, vec![draw_box(3)], vec![vec![draw_box(4)]]),
        Zone::Board(A, Color::Purple),
    );
    [bottom, middle, top]
}

/// Take a Dogma action on `card` and return the ages A drew, in order.
fn dogma_draws(layout: Layout, expansions: Expansions, card: CardId) -> (Game, Vec<u8>) {
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    finish(&mut game);
    let drawn = game
        .position()
        .cards(Zone::Hand(A))
        .iter()
        .map(|&card| game.position().card(card).value)
        .collect();
    (game, drawn)
}

#[test]
fn a_dogma_runs_visible_echoes_bottom_to_top_then_its_dogmas() {
    for (splay, expected) in [
        (Splay::Right, vec![1, 2, 3, 4]),
        (Splay::Up, vec![1, 2, 3, 4]),
        // A left splay covers bottom-left echoes; unsplayed shows only the top.
        (Splay::Left, vec![3, 4]),
        (Splay::None, vec![3, 4]),
    ] {
        let mut layout = Layout::default();
        let [_, _, top] = echo_pile(&mut layout, vec![draw_box(1)]);
        if splay != Splay::None {
            layout.splay(A, Color::Purple, splay);
        }
        ages(&mut layout, 1..=6);
        let (_, drawn) = dogma_draws(layout, Expansions::ECHOES, top);
        assert_eq!(drawn, expected, "{splay:?}");
    }
}

#[test]
fn the_schedule_is_frozen_when_the_dogma_locks_in() {
    // The bottom echo melds a purple card whose own echo would draw a 5 on
    // top of the pile. The covered echoes and dogma still run; the newly
    // exposed echo waits for a later invocation.
    let mut layout = Layout::default();
    let [_, _, top] = echo_pile(
        &mut layout,
        vec![program::build::meld(program::build::HAND), draw_box(1)],
    );
    layout.splay(A, Color::Purple, Splay::Right);
    let newcomer = layout.add(
        echo_card(Color::Purple, vec![draw_box(5)], Vec::new()),
        Zone::Hand(A),
    );
    ages(&mut layout, 1..=6);
    let (game, drawn) = dogma_draws(layout, Expansions::ECHOES, top);
    assert_eq!(drawn, [1, 2, 3, 4]);
    assert_eq!(game.position().top(A, Color::Purple), Some(newcomer));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(5))).len(), 1);
}

#[test]
fn echoes_are_shared_non_demand_effects() {
    // B shows more Concept, so B shares: each echo and dogma runs for B first.
    let mut layout = Layout::default();
    let card = layout.add(
        echo_card(Color::Purple, vec![draw_box(1)], vec![vec![draw_box(2)]]),
        Zone::Board(A, Color::Purple),
    );
    layout.pile(
        B,
        Color::Blue,
        1,
        [
            Symbol::Icon(Icon::Concept),
            Symbol::Icon(Icon::Concept),
            Symbol::Icon(Icon::Concept),
            Symbol::Image,
        ],
    );
    for _ in 0..3 {
        ages(&mut layout, 1..=2);
    }
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let events = finish(&mut game);
    let draws: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if matches!(m.destination, Zone::Hand(_)) => {
                Some((m.executor, game.position().card(m.card).value))
            }
            _ => None,
        })
        .take(4)
        .collect();
    assert_eq!(draws, [(B, 1), (A, 1), (B, 2), (A, 2)]);
}

#[test]
fn self_execution_runs_its_piles_echoes_only_from_a_top_card() {
    let both = |card_index: usize| {
        let mut layout = Layout::default();
        let pile = echo_pile(&mut layout, vec![draw_box(1)]);
        layout.splay(A, Color::Purple, Splay::Right);
        ages(&mut layout, 1..=6);
        let card = pile[card_index];
        let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
        game.self_execute(card, Context::new(A)).unwrap();
        finish(&mut game);
        game.position()
            .cards(Zone::Hand(A))
            .iter()
            .map(|&card| game.position().card(card).value)
            .collect::<Vec<_>>()
    };
    // The top card: its pile's visible echoes, then its dogma.
    assert_eq!(both(2), [1, 2, 3, 4]);
    // A buried card: only its own printed echo, then its own dogma.
    assert_eq!(both(0), [1, 6]);
}

#[test]
fn executing_a_top_card_without_dogma_effects_runs_no_echoes() {
    // A City (or Figure, or Yamato) on top of a splayed pile: executing it runs nothing
    // at all, not even the echoes visible below it.
    let mut layout = Layout::default();
    echo_pile(&mut layout, vec![draw_box(1)]);
    let city = layout.add(
        printed(Color::Purple, [Symbol::Image; 4]),
        Zone::Board(A, Color::Purple),
    );
    layout.splay(A, Color::Purple, Splay::Right);
    ages(&mut layout, 1..=6);
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.self_execute(city, Context::new(A)).unwrap();
    finish(&mut game);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn archimedes_raises_echo_values_and_scionti_ignores_echoes() {
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    // Archimedes: "if you would execute an effect" includes the echo.
    let mut layout = Layout::default();
    let card = layout.add(
        echo_card(Color::Purple, vec![draw_box(1)], vec![Vec::new()]),
        Zone::Board(A, Color::Purple),
    );
    layout.figure(Figure::Archimedes, A);
    ages(&mut layout, 1..=2);
    let (_, drawn) = dogma_draws(layout, both, card);
    assert_eq!(drawn, [2]);

    // Scionti: "if you would execute a dogma effect" skips the echo, so he
    // draws and scores one 9, for the dogma.
    let mut layout = Layout::default();
    let card = layout.add(
        echo_card(Color::Purple, vec![draw_box(1)], vec![vec![draw_box(2)]]),
        Zone::Board(A, Color::Purple),
    );
    layout.figure(Figure::GiuseppeScionti, A);
    ages(&mut layout, 1..=2);
    for _ in 0..2 {
        in_deck(&mut layout, CardSet::Base, 9);
    }
    let (game, drawn) = dogma_draws(layout, both, card);
    assert_eq!(drawn, [1, 2]);
    assert_eq!(game.position().cards(Zone::Score(A)).len(), 1);
}

#[test]
fn an_echo_in_a_copied_execution_earns_chain() {
    // The middle card's echo self-executes a card whose one dogma effect
    // does nothing. Inside a self-execution of the top card, that echo is
    // the copied card's non-demand invocation, so the keyword first draws and
    // achieves an 11.
    let chained = |copied: bool| {
        let mut layout = Layout::default();
        layout.add(
            echo_card(Color::Green, Vec::new(), vec![Vec::new()]),
            Zone::Hand(B),
        );
        layout.add(
            echo_card(Color::Purple, Vec::new(), Vec::new()),
            Zone::Board(A, Color::Purple),
        );
        layout.add(
            echo_card(
                Color::Purple,
                vec![
                    Instruction::Bind(Binding::Card(Selector::Hand(Who::Opponent))),
                    crate::build::execute(ExecutionMode::SelfExecute, Who::Executor),
                ],
                Vec::new(),
            ),
            Zone::Board(A, Color::Purple),
        );
        // Every Dogma source needs a dogma effect; this one does nothing.
        let top = layout.add(
            echo_card(Color::Purple, Vec::new(), vec![Vec::new()]),
            Zone::Board(A, Color::Purple),
        );
        layout.splay(A, Color::Purple, Splay::Right);
        in_deck(&mut layout, CardSet::Base, 11);
        let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
        if copied {
            game.self_execute(top, Context::new(A)).unwrap();
        } else {
            game.take_action(
                Action::Dogma {
                    card: top,
                    auspice: false,
                },
                ActionKind::Regular,
            )
            .unwrap();
        }
        finish(&mut game);
        game.position().cards(Zone::Achievements(A)).len()
    };
    assert_eq!(chained(true), 1);
    assert_eq!(chained(false), 0);
}

/// A card of this color and value with these dogma effects and no echo.
fn dogma_card(color: Color, value: u8, dogmas: Vec<Vec<Instruction>>) -> Card {
    let mut card = echo_card(color, Vec::new(), dogmas);
    card.echo = None;
    card.symbols[1] = Symbol::Icon(Icon::Concept);
    card.value = value;
    card
}

/// A's regular `action`, taking the first option at each decision; whether
/// A won.
fn act(game: &mut Game, action: Action) -> bool {
    game.take_action(action, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    let won = loop {
        match game.advance(1000, &mut events).unwrap() {
            Status::Decision(_) => game.choose(0).unwrap(),
            Status::Complete(_) => break false,
            Status::Winner(winner) => break winner == A,
            status => panic!("unexpected {status:?}"),
        }
    };
    assert!(game.position().check_conservation());
    won
}

/// A's Meld action of `card`, taking the first option at each decision.
fn meld_action(layout: Layout, expansions: Expansions, card: CardId) -> (Game, bool) {
    let mut game = Game::new(layout.position(expansions), expansions).unwrap();
    let won = act(&mut game, Action::Meld { card });
    (game, won)
}

#[test]
fn a_meld_action_promotes_after_its_checks_and_dogmas_the_card() {
    // Five Forecast cards earn Destiny at the Meld action's end; only then does
    // promotion take one.
    let mut layout = Layout::default();
    let melded = layout.add(
        dogma_card(Color::Purple, 5, vec![Vec::new()]),
        Zone::Hand(A),
    );
    let promoted = layout.add(
        dogma_card(Color::Green, 2, vec![vec![draw_box(4)]]),
        Zone::Forecast(A),
    );
    layout.forecast(A, 4);
    let reward = in_deck(&mut layout, CardSet::Base, 4);
    let (game, _) = meld_action(layout, Expansions::ECHOES, melded);
    let position = game.position();
    assert_eq!(
        position.special_location(Special::Destiny),
        SpecialLocation::Owned(A)
    );
    assert_eq!(position.top(A, Color::Green), Some(promoted));
    assert_eq!(position.cards(Zone::Forecast(A)).len(), 4);
    // The promoted card's free Dogma ran; it used no regular action.
    assert_eq!(position.cards(Zone::Hand(A)), [reward]);
    assert_eq!(position.regular_actions_started, 1);
}

#[test]
fn promotion_takes_only_a_card_no_higher_than_the_melded_one() {
    let mut layout = Layout::default();
    let melded = layout.add(
        dogma_card(Color::Purple, 3, vec![Vec::new()]),
        Zone::Hand(A),
    );
    let higher = layout.add(
        dogma_card(Color::Green, 4, vec![Vec::new()]),
        Zone::Forecast(A),
    );
    let (game, _) = meld_action(layout, Expansions::ECHOES, melded);
    assert_eq!(game.position().cards(Zone::Forecast(A)), [higher]);
}

#[test]
fn promotion_is_one_mandatory_choice_and_never_chains() {
    let mut layout = Layout::default();
    let melded = layout.add(
        dogma_card(Color::Purple, 5, vec![Vec::new()]),
        Zone::Hand(A),
    );
    for color in [Color::Green, Color::Red] {
        layout.add(dogma_card(color, 1, vec![Vec::new()]), Zone::Forecast(A));
    }
    let mut game = Game::new(layout.position(Expansions::ECHOES), Expansions::ECHOES).unwrap();
    game.take_action(Action::Meld { card: melded }, ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let Status::Decision(menu) = game.advance(1000, &mut events).unwrap() else {
        panic!("expected the promotion choice");
    };
    assert_eq!(menu.options.len(), 2);
    assert!(!menu.options.contains(&Alternative::Decline));
    game.choose(0).unwrap();
    finish(&mut game);
    // The promotion meld doesn't promote again.
    assert_eq!(game.position().cards(Zone::Forecast(A)).len(), 1);
}

#[test]
fn an_effect_meld_does_not_promote() {
    let mut layout = Layout::default();
    let card = program(
        &mut layout,
        vec![program::build::meld(program::build::HAND)],
    );
    layout.add(dogma_card(Color::Red, 5, vec![Vec::new()]), Zone::Hand(A));
    let waiting = layout.add(
        dogma_card(Color::Green, 1, vec![Vec::new()]),
        Zone::Forecast(A),
    );
    let (game, _) = run_program(layout, card);
    assert_eq!(game.position().cards(Zone::Forecast(A)), [waiting]);
}

#[test]
fn a_promoted_figure_takes_no_dogma_action() {
    // J.P. Morgan splays the color of any card being dogma'd up. A promoted ordinary
    // red card is dogma'd; a promoted Figure is melded but never dogma'd.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let red_splay = |figure: bool| {
        let mut layout = Layout::default();
        layout.figure(Figure::JPMorgan, A);
        layout.pile(A, Color::Red, 1, [Symbol::Blank; 4]);
        let melded = layout.add(
            dogma_card(Color::Purple, 5, vec![Vec::new()]),
            Zone::Hand(A),
        );
        let promoted = if figure {
            figure_card(Figure::TranHuangDao.code()).unwrap()
        } else {
            dogma_card(Color::Red, 3, vec![Vec::new()])
        };
        let promoted = layout.add(promoted, Zone::Forecast(A));
        let (game, _) = meld_action(layout, both, melded);
        assert_eq!(game.position().top(A, Color::Red), Some(promoted));
        game.position().splay(A, Color::Red)
    };
    assert_eq!(red_splay(false), Splay::Up);
    assert_eq!(red_splay(true), Splay::None);
}

#[test]
fn a_promoted_battleship_yamato_takes_no_dogma_action() {
    // Yamato has no Dogma effects and no featured icon: promotion melds it (value 8 in
    // the Forecast, 11 on the board) and takes no free Dogma action.
    let both = Expansions::ECHOES.union(Expansions::ARTIFACTS);
    let mut layout = Layout::default();
    let melded = layout.add(
        dogma_card(Color::Purple, 9, vec![Vec::new()]),
        Zone::Hand(A),
    );
    let yamato = layout.add(artifacts_card("A077").unwrap(), Zone::Forecast(A));
    let (game, _) = meld_action(layout, both, melded);
    let position = game.position();
    assert_eq!(position.top(A, Color::Red), Some(yamato));
    assert_eq!(position.value(yamato), 11);
    assert!(position.cards(Zone::Forecast(A)).is_empty());
    assert!(position.check_conservation());
}

/// Draw a [1] (showing the effect ran), then win if its card was foreseen.
fn foreseen_win() -> Vec<Instruction> {
    vec![
        draw_box(1),
        program::build::if_(&Cond::Foreseen, 1),
        Instruction::Win(WinRule::Executor),
    ]
}

#[test]
fn foreseen_holds_only_in_the_promoted_cards_own_dogma() {
    // Promoted: foreseen.
    let mut layout = Layout::default();
    let melded = layout.add(
        dogma_card(Color::Purple, 5, vec![Vec::new()]),
        Zone::Hand(A),
    );
    layout.add(
        dogma_card(Color::Green, 2, vec![foreseen_win()]),
        Zone::Forecast(A),
    );
    in_deck(&mut layout, CardSet::Base, 1);
    assert!(meld_action(layout, Expansions::ECHOES, melded).1);

    // The same card dogma'd as an ordinary action: not foreseen (`finish`
    // panics on a win).
    let mut layout = Layout::default();
    let card = layout.add(
        dogma_card(Color::Green, 2, vec![foreseen_win()]),
        Zone::Board(A, Color::Green),
    );
    in_deck(&mut layout, CardSet::Base, 1);
    let (_, drawn) = dogma_draws(layout, Expansions::ECHOES, card);
    assert_eq!(drawn, [1]);

    // A buried echo's "foreseen" names its own card, never the promoted one.
    let mut layout = Layout::default();
    let melded = layout.add(
        dogma_card(Color::Purple, 5, vec![Vec::new()]),
        Zone::Hand(A),
    );
    layout.add(
        echo_card(Color::Green, foreseen_win(), vec![Vec::new()]),
        Zone::Board(A, Color::Green),
    );
    layout.add(
        dogma_card(Color::Green, 2, vec![Vec::new()]),
        Zone::Board(A, Color::Green),
    );
    layout.splay(A, Color::Green, Splay::Right);
    layout.add(
        dogma_card(Color::Green, 2, vec![Vec::new()]),
        Zone::Forecast(A),
    );
    in_deck(&mut layout, CardSet::Base, 1);
    let (game, won) = meld_action(layout, Expansions::ECHOES, melded);
    assert!(!won);
    assert_eq!(
        game.position().cards(Zone::Hand(A)).len(),
        1,
        "the echo ran"
    );
}

#[test]
fn a_replaced_free_dogma_is_not_foreseen() {
    // Kepler replaces any Dogma action he sees, the promoted free one too.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let mut layout = Layout::default();
    layout.figure(Figure::JohannesKepler, A);
    let melded = layout.add(
        dogma_card(Color::Purple, 5, vec![Vec::new()]),
        Zone::Hand(A),
    );
    layout.add(
        dogma_card(Color::Green, 2, vec![foreseen_win()]),
        Zone::Forecast(A),
    );
    ages(&mut layout, 1..=3);
    let (game, won) = meld_action(layout, both, melded);
    assert!(!won);
    // Kepler's replacement ran the dogma: two top values above 3 (Kepler's
    // 4, the melded 5) raise its boxed 1 to a 3.
    let hand = game.position().cards(Zone::Hand(A));
    assert_eq!(hand.len(), 1);
    assert_eq!(game.position().card(hand[0]).value, 3);
}

/// The colors other than `figure`'s, so cards melded there never cover it.
fn other_colors(figure: Figure) -> Vec<Color> {
    let own = figure_card(figure.code()).unwrap().color;
    Color::ALL
        .into_iter()
        .filter(|&color| color != own)
        .collect()
}

/// Whether `figure`'s karma on dogma'ing "as your first/second action", or
/// on a draw "during your first action", fires for A's Dogma of a card that
/// draws a [1]: as A's regular action number `ordinal`, or as the free Dogma
/// promoted by A's Meld action number `ordinal`.
fn action_karma_fires(figure: Figure, ordinal: u8, promoted: bool) -> bool {
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let colors = other_colors(figure);
    let mut layout = Layout::default();
    layout.figure(figure, A);
    // A covered card for Plato to splay, and a [1] in hand for Ptahhotep to
    // score and Shennong to count.
    layout.pile(A, colors[0], 1, [Symbol::Blank; 4]);
    layout.add(dogma_card(colors[2], 1, vec![Vec::new()]), Zone::Hand(A));
    let dogmad = dogma_card(colors[0], 2, vec![vec![draw_box(1)]]);
    let action = if promoted {
        layout.add(dogmad, Zone::Forecast(A));
        let melded = layout.add(dogma_card(colors[1], 5, vec![Vec::new()]), Zone::Hand(A));
        Action::Meld { card: melded }
    } else {
        let card = layout.add(dogmad, Zone::Board(A, colors[0]));
        Action::Dogma {
            card,
            auspice: false,
        }
    };
    for _ in 0..3 {
        ages(&mut layout, 1..=3);
    }
    let mut position = layout.position(both);
    position.regular_actions_started = ordinal - 1;
    let mut game = Game::new(position, both).unwrap();
    act(&mut game, action);
    let position = game.position();
    !position.cards(Zone::Score(A)).is_empty() || position.splay(A, colors[0]) == Splay::Left
}

#[test]
fn the_promoted_dogma_is_neither_the_first_nor_the_second_action() {
    // Each karma fires for the same card dogma'd as that regular action.
    for (figure, ordinal) in [
        (Figure::Plato, 1),
        (Figure::Shennong, 1),
        (Figure::FuXi, 1),
        (Figure::Ptahhotep, 2),
    ] {
        assert!(action_karma_fires(figure, ordinal, false), "{figure:?}");
        assert!(!action_karma_fires(figure, ordinal, true), "{figure:?}");
    }
}

#[test]
fn a_promoted_dogma_uses_no_regular_action() {
    // After a first-action Meld and its promoted Dogma, A's next Dogma is
    // still the second action: Ptahhotep replaces it.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let colors = other_colors(Figure::Ptahhotep);
    let mut layout = Layout::default();
    layout.figure(Figure::Ptahhotep, A);
    let melded = layout.add(dogma_card(colors[1], 5, vec![Vec::new()]), Zone::Hand(A));
    layout.add(
        dogma_card(colors[0], 2, vec![vec![draw_box(1)]]),
        Zone::Forecast(A),
    );
    ages(&mut layout, 1..=1);
    let mut game = Game::new(layout.position(both), both).unwrap();
    act(&mut game, Action::Meld { card: melded });
    let position = game.position();
    assert_eq!(
        position.cards(Zone::Hand(A)).len(),
        1,
        "the free Dogma drew"
    );
    assert_eq!(position.regular_actions_started, 1);
    assert_eq!(position.turn_owner, A);
    act(
        &mut game,
        Action::Dogma {
            card: melded,
            auspice: false,
        },
    );
    assert_eq!(game.position().cards(Zone::Score(A)).len(), 1);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
}

#[test]
fn promotion_needs_the_melded_card_on_top_after_the_checks() {
    // Zhang Heng's karma on A's Destiny claim (step 7) draws and tucks a [3], then
    // scores every card above it. A [3] of the melded card's color scores the melded
    // card, so step 9 promotes nothing.
    let both = Expansions::ECHOES.union(Expansions::FIGURES);
    let colors = other_colors(Figure::ZhangHeng);
    let forecast_after = |tucked: Color| {
        let mut layout = Layout::default();
        layout.figure(Figure::ZhangHeng, A);
        let melded = layout.add(dogma_card(colors[0], 5, vec![Vec::new()]), Zone::Hand(A));
        layout.add(
            dogma_card(colors[1], 2, vec![Vec::new()]),
            Zone::Forecast(A),
        );
        layout.forecast(A, 4);
        let mut three = printed(tucked, [Symbol::Blank; 4]);
        three.value = 3;
        layout.add(
            three,
            Zone::Deck(Deck {
                set: CardSet::Base,
                age: 3,
            }),
        );
        let (game, _) = meld_action(layout, both, melded);
        let position = game.position();
        assert_eq!(
            position.special_location(Special::Destiny),
            SpecialLocation::Owned(A)
        );
        assert_eq!(
            position.cards(Zone::Score(A)).contains(&melded),
            tucked == colors[0]
        );
        position.cards(Zone::Forecast(A)).len()
    };
    assert_eq!(forecast_after(colors[0]), 5);
    assert_eq!(forecast_after(colors[2]), 4);
}

mod verification;
