//! Expected outcomes for Figures declared as karma programs: one scenario
//! per printed clause, driven through ordinary operations and actions.
use inno_engine::*;

mod common;
use common::*;

/// A blank synthetic card with these printed icons in its first slots.
fn card(f: &mut Fixture, color: Color, value: u8, zone: Zone, icons: &[Icon]) -> CardId {
    let id = f.tech(color, value, zone, vec![]);
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(id.0)].symbols[slot] = Symbol::Icon(icon);
    }
    id
}

/// A synthetic top card whose single effect is this program.
fn program(
    f: &mut Fixture,
    color: Color,
    value: u8,
    player: Player,
    body: Vec<Instruction>,
) -> CardId {
    f.tech(color, value, Zone::Board(player, color), body)
}

fn deck(f: &mut Fixture, age: u8, count: usize) -> Vec<CardId> {
    (0..count)
        .map(|i| card(f, Color::ALL[i % 5], age, Zone::Deck(Deck::base(age)), &[]))
        .collect()
}

/// The Figure on top of its own color on that player's board.
fn put(f: &mut Fixture, figure: Figure, player: Player) -> CardId {
    let color = figure_card(figure.code()).unwrap().color;
    f.figure(figure, Zone::Board(player, color))
}

fn new(f: Fixture) -> Game {
    Game::new(f.position(), Expansions::FIGURES).unwrap()
}

/// Resolve every decision with `pick`, defaulting to the first option.
fn run(
    game: &mut Game,
    mut pick: impl FnMut(&Game, &Decision) -> Option<usize>,
) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    for _ in 0..500 {
        match settle(game, &mut events) {
            Status::Decision(menu) => {
                let index = pick(game, &menu).unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => {
                assert!(game.position().check_conservation());
                return (status, events);
            }
        }
    }
    panic!("scenario did not finish");
}

fn first(game: &mut Game) -> (Status, Vec<Event>) {
    run(game, |_, _| None)
}

fn start(game: &mut Game, operation: Operation, player: Player) {
    game.start(operation, Context::new(player)).unwrap();
}

fn draw_op(value: u8) -> Operation {
    Operation::Draw { value, set: None }
}

fn score_op(card: CardId, source: Zone, player: Player) -> Operation {
    Operation::Move {
        verb: Verb::Score,
        target: target(card, source),
        destination: Zone::Score(player),
    }
}

fn meld_op(game: &Game, card: CardId, player: Player) -> Operation {
    meld(
        card,
        Zone::Hand(player),
        player,
        game.position().card(card).color,
    )
}

fn dogma(game: &mut Game, card: CardId) {
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
}

fn hand(game: &Game, player: Player) -> Vec<CardId> {
    game.position().cards(Zone::Hand(player)).to_vec()
}

fn score(game: &Game, player: Player) -> Vec<CardId> {
    game.position().cards(Zone::Score(player)).to_vec()
}

/// The option moving this known card.
fn moving(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(option, Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. } if *c == card)
    })
}

fn demand(f: &mut Fixture, color: Color, player: Player, body: Vec<Instruction>) -> CardId {
    let id = program(f, color, 1, player, body);
    let card = &mut f.cards[usize::from(id.0)];
    card.effects[0].kind = EffectKind::Demand;
    card.symbols[0] = Symbol::Icon(Icon::Authority);
    id
}

#[test]
fn hammurabi_reflects_a_demand_onto_its_demander_first() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Hammurabi, A);
    let source = demand(&mut f, Color::Blue, B, vec![build::draw(1)]);
    deck(&mut f, 1, 4);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    let (_, events) = first(&mut game);
    let draws: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb.is_none() => Some((m.executor, m.inside_karma)),
            _ => None,
        })
        .collect();
    assert_eq!(draws, [(B, true), (A, false)]);
    assert_eq!(hand(&game, A).len(), 1);
    assert_eq!(hand(&game, B).len(), 1);
}

#[test]
fn sunshu_ao_self_executes_a_melded_non_figure_and_rewards_a_sixth_tuck() {
    let mut f = Fixture::default();
    put(&mut f, Figure::SunshuAo, A);
    let melded = f.tech(Color::Green, 3, Zone::Hand(A), vec![build::draw(1)]);
    deck(&mut f, 1, 3);
    let mut game = new(f);
    game.take_action(Action::Meld { card: melded }, ActionKind::Regular)
        .unwrap();
    first(&mut game);
    assert!(hand(&game, A).contains(&melded));
    assert_eq!(hand(&game, A).len(), 2);

    let mut f = Fixture::default();
    put(&mut f, Figure::SunshuAo, A);
    let tucked: Vec<_> = (0..6)
        .map(|_| card(&mut f, Color::Blue, 2, Zone::Hand(A), &[]))
        .collect();
    deck(&mut f, 1, 2);
    let mut game = new(f);
    for (i, &id) in tucked.iter().enumerate() {
        start(
            &mut game,
            Operation::Move {
                verb: Verb::Tuck,
                target: target(id, Zone::Hand(A)),
                destination: Zone::Board(A, Color::Blue),
            },
            A,
        );
        first(&mut game);
        let achieved = game.position().cards(Zone::Achievements(A)).len();
        assert_eq!(achieved, usize::from(i == 5));
    }
}

#[test]
fn hatshepsut_returns_the_hand_and_draws_that_many_of_the_draw_value() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Hatshepsut, A);
    let one = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
    let four = card(&mut f, Color::Blue, 4, Zone::Hand(A), &[]);
    deck(&mut f, 3, 4);
    let mut game = new(f);
    start(&mut game, draw_op(3), A);
    first(&mut game);
    let hand = hand(&game, A);
    assert_eq!(hand.len(), 3);
    assert!(hand.iter().all(|&c| game.position().card(c).value == 3));
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(1)))
            .contains(&one)
    );
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(4)))
            .contains(&four)
    );
}

#[test]
fn imhotep_melds_a_two_splays_it_and_returns_both_tops_of_that_color() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Imhotep, A);
    let source = program(&mut f, Color::Purple, 1, A, vec![]);
    let old = card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
    let theirs = card(&mut f, Color::Red, 3, Zone::Board(B, Color::Red), &[]);
    let two = card(&mut f, Color::Red, 2, Zone::Deck(Deck::base(2)), &[]);
    let mut game = new(f);
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), [old]);
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(2)))
            .contains(&two)
    );
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(3)))
            .contains(&theirs)
    );
}

#[test]
fn ximen_bao_junks_the_opponents_hand_before_issuing_a_decree() {
    let mut f = Fixture::default();
    put(&mut f, Figure::XimenBao, A);
    f.figure(Figure::Gilgamesh, Zone::Hand(A));
    f.figure(Figure::Tigernmas, Zone::Hand(A));
    let theirs: Vec<_> = (0..2)
        .map(|_| card(&mut f, Color::Green, 1, Zone::Hand(B), &[]))
        .collect();
    let mut game = new(f);
    game.take_action(Action::Decree, ActionKind::Regular)
        .unwrap();
    first(&mut game);
    assert!(hand(&game, B).is_empty());
    assert!(
        theirs
            .iter()
            .all(|c| game.position().cards(Zone::Junk).contains(c))
    );
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Owned(A)
    );
}

#[test]
fn augustus_melds_an_authority_draw_or_has_the_player_super_execute() {
    for concept in [true, false] {
        let mut f = Fixture::default();
        put(&mut f, Figure::AugustusCaesar, A);
        let source = program(&mut f, Color::Yellow, 2, B, vec![build::draw(1)]);
        f.cards[usize::from(source.0)].symbols[0] = Symbol::Icon(Icon::Authority);
        let icons: &[Icon] = if concept { &[Icon::Concept] } else { &[] };
        let drawn = card(&mut f, Color::Blue, 2, Zone::Deck(Deck::base(2)), icons);
        deck(&mut f, 1, 2);
        let mut game = new(f);
        game.begin_turn(B).unwrap();
        dogma(&mut game, source);
        first(&mut game);
        if concept {
            assert_eq!(game.position().top(A, Color::Blue), Some(drawn));
            assert!(hand(&game, B).is_empty());
        } else {
            assert_eq!(hand(&game, A), [drawn]);
            assert_eq!(hand(&game, B).len(), 1);
        }
    }
}

#[test]
fn zhang_heng_tucks_a_three_and_scores_every_card_above_it() {
    let mut f = Fixture::default();
    put(&mut f, Figure::ZhangHeng, A);
    let low = card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
    let high = card(&mut f, Color::Red, 2, Zone::Board(A, Color::Red), &[]);
    let three = card(&mut f, Color::Red, 3, Zone::Deck(Deck::base(3)), &[]);
    let claimed = card(&mut f, Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    start(
        &mut game,
        Operation::Claim {
            target: target(claimed, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Ignore,
        },
        A,
    );
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), [three]);
    let mut scored = score(&game, A);
    scored.sort();
    assert_eq!(scored, [low, high]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), [claimed]);
}

#[test]
fn confucius_scores_the_authority_dogma_card_and_junks_a_chosen_deck() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Confucius, A);
    let source = program(&mut f, Color::Red, 3, A, vec![build::draw(1)]);
    let twos = deck(&mut f, 2, 2);
    let mut game = new(f);
    dogma(&mut game, source);
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(2))
    });
    assert_eq!(score(&game, A), [source]);
    assert!(
        twos.iter()
            .all(|c| game.position().cards(Zone::Junk).contains(c))
    );
    assert!(hand(&game, A).is_empty());
}

#[test]
fn confucius_skips_its_deck_choice_when_both_base_decks_are_empty() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Confucius, A);
    let source = program(&mut f, Color::Red, 3, A, vec![build::draw(1)]);
    deck(&mut f, 4, 1);
    let mut game = new(f);
    dogma(&mut game, source);
    run(&mut game, |_, menu| {
        assert!(
            !menu
                .options
                .iter()
                .any(|o| matches!(o, Alternative::Value(_)))
        );
        None
    });
    assert_eq!(score(&game, A), [source]);
    assert!(game.position().cards(Zone::Junk).is_empty());
}

#[test]
fn saladin_scores_each_opposing_authority_top_instead_of_a_demand() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Saladin, A);
    let source = demand(&mut f, Color::Green, A, vec![build::draw(1)]);
    let red = card(
        &mut f,
        Color::Yellow,
        2,
        Zone::Board(B, Color::Yellow),
        &[Icon::Authority],
    );
    card(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue), &[]);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(score(&game, A), [red]);
}

#[test]
fn rhazes_tucks_a_same_value_card_from_hand_or_the_opposing_score_pile() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Rhazes, A);
    let mine = card(&mut f, Color::Blue, 2, Zone::Hand(A), &[]);
    let theirs = card(&mut f, Color::Green, 2, Zone::Score(B), &[]);
    card(&mut f, Color::Red, 3, Zone::Hand(A), &[]);
    deck(&mut f, 2, 1);
    let mut game = new(f);
    start(&mut game, draw_op(2), A);
    let (_, events) = first(&mut game);
    let tucks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb == Some(Verb::Tuck) => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(tucks.len(), 1);
    assert!([mine, theirs].contains(&tucks[0]));
}

#[test]
fn alhazen_tucks_an_authority_top_and_draws_by_a_colors_icons() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Alhazen, A);
    let theirs = card(
        &mut f,
        Color::Red,
        3,
        Zone::Board(B, Color::Red),
        &[Icon::Authority, Icon::Concept],
    );
    deck(&mut f, 2, 2);
    deck(&mut f, 3, 2);
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)), [theirs]);
    let hand = hand(&game, A);
    assert_eq!(hand.len(), 1);
    assert_eq!(game.position().card(hand[0]).value, 2);
}

#[test]
fn amina_counts_authority_tops_as_achievements_and_scores_an_opposing_hand_figure() {
    let mut f = Fixture::default();
    put(&mut f, Figure::AminaSukhera, A);
    let source = program(&mut f, Color::Blue, 1, A, vec![]);
    let authority = card(
        &mut f,
        Color::Green,
        2,
        Zone::Board(A, Color::Green),
        &[Icon::Authority],
    );
    let figure = f.figure(Figure::Gilgamesh, Zone::Hand(B));
    let mut game = new(f);
    assert!(
        game.position()
            .targets(LogicalZone::AvailableAchievements(A))
            .any(|t| t.card == authority)
    );
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(score(&game, A), [figure]);
}

#[test]
fn leonardo_melds_another_card_and_rewards_a_purple_one() {
    let mut f = Fixture::default();
    put(&mut f, Figure::LeonardoDaVinci, A);
    let melded = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
    let purple = card(&mut f, Color::Purple, 1, Zone::Hand(A), &[]);
    let fours = deck(&mut f, 4, 3);
    let theirs = put(&mut f, Figure::Gilgamesh, B);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    run(&mut game, |_, menu| moving(menu, theirs));
    assert_eq!(game.position().top(A, Color::Purple), Some(purple));
    assert_eq!(game.position().top(A, Color::Red), Some(melded));
    let mut hand = hand(&game, A);
    hand.sort();
    assert_eq!(hand, fours);
    assert_eq!(score(&game, A), [theirs]);
}

#[test]
fn taki_ad_din_replaces_the_draw_action_with_a_revealed_five() {
    let mut f = Fixture::default();
    put(&mut f, Figure::TakiAdDin, A);
    for value in [1, 2] {
        card(
            &mut f,
            Color::Green,
            value,
            Zone::Board(A, Color::Green),
            &[],
        );
    }
    let five = card(
        &mut f,
        Color::Green,
        5,
        Zone::Deck(Deck::base(5)),
        &[Icon::Concept],
    );
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    first(&mut game);
    assert_eq!(game.position().top(A, Color::Green), Some(five));
    assert_eq!(game.position().splay(A, Color::Green), Splay::Right);
    assert!(hand(&game, A).is_empty());
}

#[test]
fn shivaji_claims_another_eligible_achievement_and_supplies_purple_cards() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Shivaji, A);
    card(&mut f, Color::Yellow, 5, Zone::Score(A), &[]);
    let theirs = card(&mut f, Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mine = card(&mut f, Color::Blue, 1, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    start(
        &mut game,
        Operation::Claim {
            target: target(theirs, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Ignore,
        },
        B,
    );
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Achievements(A)), [mine]);
    assert_eq!(game.position().cards(Zone::Achievements(B)), [theirs]);

    let mut f = Fixture::default();
    put(&mut f, Figure::Shivaji, A);
    let source = program(&mut f, Color::Yellow, 1, A, vec![]);
    let purples: Vec<_> = (0..2)
        .map(|_| card(&mut f, Color::Purple, 2, Zone::Board(A, Color::Purple), &[]))
        .collect();
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert!(purples.iter().all(|c| {
        game.position()
            .cards(Zone::AvailableAchievements)
            .contains(c)
    }));
}

#[test]
fn peter_the_great_scores_the_bottom_green_and_may_achieve_the_next() {
    let mut f = Fixture::default();
    put(&mut f, Figure::PeterTheGreat, A);
    let bottom = card(
        &mut f,
        Color::Green,
        1,
        Zone::Board(A, Color::Green),
        &[Icon::Prosperity],
    );
    let next = card(&mut f, Color::Green, 1, Zone::Board(A, Color::Green), &[]);
    card(&mut f, Color::Green, 2, Zone::Board(A, Color::Green), &[]);
    card(&mut f, Color::Yellow, 5, Zone::Score(A), &[]);
    let returned = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    let mut game = new(f);
    start(
        &mut game,
        Operation::Move {
            verb: Verb::Return,
            target: target(returned, Zone::Hand(A)),
            destination: Zone::Deck(Deck::base(1)),
        },
        A,
    );
    first(&mut game);
    assert!(score(&game, A).contains(&bottom));
    assert_eq!(game.position().cards(Zone::Achievements(A)), [next]);
}

#[test]
fn champlain_rewards_a_fifth_and_a_first_card_into_the_hand() {
    let mut f = Fixture::default();
    put(&mut f, Figure::SamuelDeChamplain, A);
    for _ in 0..4 {
        card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    }
    let low = card(&mut f, Color::Green, 2, Zone::AvailableAchievements, &[]);
    card(&mut f, Color::Green, 4, Zone::AvailableAchievements, &[]);
    deck(&mut f, 3, 1);
    let mut game = new(f);
    start(&mut game, draw_op(3), A);
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Achievements(A)), [low]);

    let mut f = Fixture::default();
    put(&mut f, Figure::SamuelDeChamplain, A);
    deck(&mut f, 1, 1);
    let six = deck(&mut f, 6, 1);
    let mut game = new(f);
    start(&mut game, draw_op(1), A);
    first(&mut game);
    assert!(hand(&game, A).contains(&six[0]));
    assert_eq!(hand(&game, A).len(), 2);
}

#[test]
fn bartholomew_roberts_replaces_the_draw_action_before_its_draw_and_claims_by_drawn_value() {
    let mut f = Fixture::default();
    put(&mut f, Figure::BartholomewRoberts, A);
    let prosperity = card(
        &mut f,
        Color::Red,
        2,
        Zone::Board(B, Color::Red),
        &[Icon::Prosperity],
    );
    let achievement = card(&mut f, Color::Blue, 5, Zone::AvailableAchievements, &[]);
    deck(&mut f, 5, 2);
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut offered = 0;
    run(&mut game, |_, menu| {
        offered = offered.max(
            menu.options
                .iter()
                .filter(|o| matches!(o, Alternative::Karma { .. }))
                .count(),
        );
        moving(menu, prosperity).or_else(|| {
            menu.options
                .iter()
                .position(|o| matches!(o, Alternative::Karma { .. }))
        })
    });
    // Taking the Draw action and drawing are separate events: the Instead on the action
    // resolves alone, so the draw clause never triggers.
    assert!(offered <= 1);
    assert_eq!(score(&game, A), [prosperity]);
    assert!(hand(&game, A).is_empty());

    let mut f = Fixture::default();
    put(&mut f, Figure::BartholomewRoberts, A);
    let achievement2 = card(&mut f, Color::Blue, 5, Zone::AvailableAchievements, &[]);
    deck(&mut f, 5, 1);
    let mut game = new(f);
    start(&mut game, draw_op(5), A);
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Achievements(A)), [achievement2]);
    let _ = achievement;
}

/// A Draw action with Newton (A) and John Ericsson (B) on top, Newton's
/// drawn 1 matching Ericsson's color. Returns whether Ericsson tucked a 7
/// and the size of A's hand afterwards.
fn newton_then_ericsson(newton_to_opponent: bool) -> (bool, usize) {
    let mut f = Fixture::default();
    put(&mut f, Figure::IsaacNewton, A);
    let ericsson = put(&mut f, Figure::JohnEricsson, B);
    let color = figure_card(Figure::JohnEricsson.code()).unwrap().color;
    let one = card(&mut f, color, 1, Zone::Deck(Deck::base(1)), &[]);
    for age in 2..=11 {
        deck(&mut f, age, 2);
    }
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let branch = Alternative::Branch(usize::from(newton_to_opponent));
    run(&mut game, |_, menu| {
        menu.options.iter().position(|o| *o == branch)
    });
    if newton_to_opponent {
        assert_eq!(game.position().top(B, color), Some(one));
    } else {
        assert_eq!(game.position().top(B, color), Some(ericsson));
    }
    let tucked = Color::ALL.into_iter().any(|color| {
        game.position()
            .cards(Zone::Board(B, color))
            .iter()
            .any(|&c| c != ericsson && c != one)
    });
    (tucked, hand(&game, A).len())
}

#[test]
fn a_draw_action_and_its_draw_are_separate_karma_events() {
    // Newton's First on the Draw action resolves, then the draw inside it discovers
    // Ericsson's draw karma as a new event.
    assert_eq!(newton_then_ericsson(false), (true, 1));
    // Karma are evaluated in the moment: once Newton's 1 covers Ericsson,
    // his karma no longer triggers.
    assert_eq!(newton_then_ericsson(true), (false, 1));
}

#[test]
fn draw_karma_do_not_trigger_on_a_draw_that_ends_the_game() {
    let mut f = Fixture::default();
    put(&mut f, Figure::JohnEricsson, B);
    let seven = card(&mut f, Color::Blue, 7, Zone::Deck(Deck::base(7)), &[]);
    let mut game = new(f);
    start(&mut game, draw_op(8), A);
    let (status, _) = first(&mut game);
    assert_eq!(status, Status::Draw(DrawReason::Exhaustion));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), [seven]);
}

#[test]
fn newton_transfers_a_drawn_one_and_splays_matching_value_colors_right() {
    let mut f = Fixture::default();
    put(&mut f, Figure::IsaacNewton, A);
    let one = card(&mut f, Color::Yellow, 1, Zone::Deck(Deck::base(1)), &[]);
    deck(&mut f, 5, 1);
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Branch(1))
    });
    assert_eq!(game.position().top(B, Color::Yellow), Some(one));
    assert_eq!(hand(&game, A).len(), 1);

    let mut f = Fixture::default();
    put(&mut f, Figure::IsaacNewton, A);
    let source = program(&mut f, Color::Red, 2, A, vec![]);
    card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
    f.placements.rotate_right(1);
    for value in [1, 2] {
        card(
            &mut f,
            Color::Green,
            value,
            Zone::Board(A, Color::Green),
            &[],
        );
    }
    for value in [3, 1] {
        card(
            &mut f,
            Color::Yellow,
            value,
            Zone::Board(A, Color::Yellow),
            &[],
        );
    }
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(game.position().splay(A, Color::Green), Splay::Right);
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::None);
}

#[test]
fn cristofori_claims_when_melding_a_fifth_visible_card() {
    let mut f = Fixture::default();
    put(&mut f, Figure::BartolomeoCristofori, A);
    for value in 1..=4 {
        card(&mut f, Color::Blue, value, Zone::Board(A, Color::Blue), &[]);
    }
    let melded = card(&mut f, Color::Blue, 5, Zone::Hand(A), &[]);
    let achievement = card(&mut f, Color::Red, 9, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    for (splayed, expected) in [(false, 0), (true, 1)] {
        let mut game = game.clone();
        if splayed {
            start(
                &mut game,
                Operation::Splay {
                    player: A,
                    color: Color::Blue,
                    direction: Splay::Right,
                },
                A,
            );
            first(&mut game);
        }
        let op = meld_op(&game, melded, A);
        start(&mut game, op, A);
        first(&mut game);
        assert_eq!(game.position().cards(Zone::Achievements(A)).len(), expected);
    }
    game.begin_turn(A).unwrap();
    let _ = achievement;
}

#[test]
fn vermeer_claims_one_higher_and_scores_a_whole_color_on_meld() {
    let mut f = Fixture::default();
    put(&mut f, Figure::JohannesVermeer, A);
    let claimed = card(&mut f, Color::Red, 1, Zone::AvailableAchievements, &[]);
    let higher = card(&mut f, Color::Red, 2, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    start(
        &mut game,
        Operation::Claim {
            target: target(claimed, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Ignore,
        },
        A,
    );
    first(&mut game);
    assert_eq!(
        game.position().cards(Zone::Achievements(A)),
        [higher, claimed]
    );

    let mut f = Fixture::default();
    put(&mut f, Figure::JohannesVermeer, A);
    let reds: Vec<_> = (1..=2)
        .map(|v| card(&mut f, Color::Red, v, Zone::Board(A, Color::Red), &[]))
        .collect();
    let melded = card(&mut f, Color::Green, 1, Zone::Hand(A), &[]);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << Color::Red as u8))
    });
    let mut scored = score(&game, A);
    scored.sort();
    assert_eq!(scored, reds);
}

#[test]
fn napoleon_melds_instead_tucks_red_or_blue_and_scores_a_five_or_six() {
    let mut f = Fixture::default();
    put(&mut f, Figure::NapoleonBonaparte, A);
    let under = card(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue), &[]);
    let scored = card(&mut f, Color::Blue, 2, Zone::Hand(A), &[]);
    let five = card(&mut f, Color::Yellow, 5, Zone::Board(B, Color::Yellow), &[]);
    let mut game = new(f);
    start(&mut game, score_op(scored, Zone::Hand(A), A), A);
    run(&mut game, |_, menu| moving(menu, five));
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Blue)),
        [scored, under]
    );
    assert_eq!(score(&game, A), [five]);
}

/// Run Translation's "meld all the cards in your score pile", always taking
/// the batch and scoring `score_pick` when Napoleon offers it.
fn translation_with_napoleon(f: &mut Fixture, score_pick: CardId) -> (Game, Vec<Event>) {
    put(f, Figure::NapoleonBonaparte, A);
    let translation = f.add(base_card("B033").unwrap(), Zone::Board(A, Color::Blue));
    let mut game = Game::new(std::mem::take(f).position(), Expansions::FIGURES).unwrap();
    game.self_execute(translation, Context::new(A)).unwrap();
    let (status, events) = run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| {
                matches!(o, Alternative::Move { verb: Verb::Score,
                target: TargetDescriptor::Known { card, .. } } if *card == score_pick)
            })
            .or_else(|| menu.options.iter().position(|o| *o != Alternative::Decline))
    });
    assert!(matches!(status, Status::Complete(_)));
    (game, events)
}

fn melds(events: &[Event], card: CardId) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Moved(m) if m.card == card && m.verb == Some(Verb::Meld)))
        .count()
}

#[test]
fn translation_melds_a_score_card_once_even_after_napoleon_scores_it_back() {
    let mut f = Fixture::default();
    let five = card(&mut f, Color::Yellow, 5, Zone::Score(A), &[]);
    let (game, events) = translation_with_napoleon(&mut f, five);
    // Napoleon melds it instead, then scores it as a top 5: it is not melded again.
    assert_eq!(melds(&events, five), 1);
    assert_eq!(score(&game, A), [five]);
}

#[test]
fn translation_melds_a_card_napoleon_scores_into_the_pile_once() {
    let mut f = Fixture::default();
    let one = card(&mut f, Color::Yellow, 1, Zone::Score(A), &[]);
    let five = card(&mut f, Color::Green, 5, Zone::Board(B, Color::Green), &[]);
    let (game, events) = translation_with_napoleon(&mut f, five);
    // The arriving card is melded once, then scored back by its own Napoleon meld.
    assert_eq!(melds(&events, one), 1);
    assert_eq!(melds(&events, five), 1);
    assert_eq!(game.position().top(A, Color::Yellow), Some(one));
    assert_eq!(score(&game, A), [five]);
}

#[test]
fn tecumseh_returns_industry_tops_of_a_chosen_value_and_junks_that_achievement() {
    let mut f = Fixture::default();
    put(&mut f, Figure::Tecumseh, A);
    let mine = card(
        &mut f,
        Color::Green,
        2,
        Zone::Board(A, Color::Green),
        &[Icon::Industry],
    );
    let theirs = card(
        &mut f,
        Color::Blue,
        2,
        Zone::Board(B, Color::Blue),
        &[Icon::Industry],
    );
    let kept = card(&mut f, Color::Yellow, 2, Zone::Board(B, Color::Yellow), &[]);
    let achievement = card(&mut f, Color::Purple, 2, Zone::AvailableAchievements, &[]);
    let melded = card(&mut f, Color::Purple, 1, Zone::Hand(A), &[]);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(2))
    });
    let decks = game.position().cards(Zone::Deck(Deck::base(2)));
    assert!(decks.contains(&mine) && decks.contains(&theirs));
    assert_eq!(game.position().top(B, Color::Yellow), Some(kept));
    assert_eq!(game.position().cards(Zone::Junk), [achievement]);
}

#[test]
fn jenner_answers_demands_with_a_return_and_junks_opponents_score_values() {
    let mut f = Fixture::default();
    put(&mut f, Figure::EdwardJenner, A);
    let source = demand(&mut f, Color::Blue, B, vec![build::draw(1)]);
    let returned = card(&mut f, Color::Red, 2, Zone::Hand(A), &[]);
    deck(&mut f, 1, 2);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    first(&mut game);
    assert!(hand(&game, A).is_empty());
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(2)))
            .contains(&returned)
    );

    let mut f = Fixture::default();
    put(&mut f, Figure::EdwardJenner, A);
    let scored = card(&mut f, Color::Red, 3, Zone::Hand(B), &[]);
    let achievement = card(&mut f, Color::Blue, 3, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    start(&mut game, score_op(scored, Zone::Hand(B), B), B);
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Junk), [achievement]);
}

#[test]
fn adam_smith_triples_prosperity_and_junks_or_splays_when_unshared() {
    let mut f = Fixture::default();
    put(&mut f, Figure::AdamSmith, A);
    let mut game = new(f);
    assert_eq!(game.position().icon_count(A, Icon::Prosperity), 6);

    for sixes in [true, false] {
        let mut f = Fixture::default();
        put(&mut f, Figure::AdamSmith, A);
        let source = program(&mut f, Color::Red, 1, A, vec![]);
        f.cards[usize::from(source.0)].symbols[0] = Symbol::Icon(Icon::Authority);
        for value in [1, 2] {
            card(&mut f, Color::Blue, value, Zone::Board(A, Color::Blue), &[]);
        }
        let six = if sixes { deck(&mut f, 6, 2) } else { vec![] };
        game = new(f);
        dogma(&mut game, source);
        // The splay may name any nonempty color (a no-op choice is
        // allowed): blue.
        run(&mut game, |_, menu| {
            menu.options
                .iter()
                .position(|o| {
                    matches!(
                        o,
                        Alternative::Splay {
                            color: Color::Blue,
                            ..
                        }
                    )
                })
                .or(Some(0))
        });
        assert!(
            six.iter()
                .all(|c| game.position().cards(Zone::Junk).contains(c))
        );
        assert_eq!(
            game.position().splay(A, Color::Blue),
            if sixes { Splay::None } else { Splay::Right }
        );
    }
}

#[test]
fn harrison_returns_a_card_and_draws_its_value_from_a_chosen_set() {
    let mut f = Fixture::default();
    put(&mut f, Figure::JohnHarrison, A);
    let returned = card(&mut f, Color::Red, 3, Zone::Hand(A), &[]);
    let figure = f.figure(Figure::Saladin, Zone::Deck(Deck::figures(3)));
    deck(&mut f, 3, 1);
    deck(&mut f, 6, 1);
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    run(&mut game, |_, menu| {
        menu.options.iter().position(|o| {
            *o == Alternative::Draw {
                value: 3,
                set: CardSet::Figures,
            }
        })
    });
    assert!(hand(&game, A).contains(&figure));
    assert!(!hand(&game, A).contains(&returned));
    assert_eq!(hand(&game, A).len(), 2);
}

#[test]
fn gauss_melds_every_other_card_of_a_chosen_value_and_draws_a_seven() {
    let mut f = Fixture::default();
    put(&mut f, Figure::CarlFriedrichGauss, A);
    let melded = card(&mut f, Color::Red, 2, Zone::Hand(A), &[]);
    let same = card(&mut f, Color::Green, 2, Zone::Hand(A), &[]);
    let scored = card(&mut f, Color::Yellow, 2, Zone::Score(A), &[]);
    let other = card(&mut f, Color::Purple, 3, Zone::Hand(A), &[]);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(2))
    });
    assert_eq!(game.position().top(A, Color::Green), Some(same));
    assert_eq!(game.position().top(A, Color::Yellow), Some(scored));
    assert_eq!(game.position().top(A, Color::Red), Some(melded));
    assert_eq!(hand(&game, A), [other]);

    let mut f = Fixture::default();
    put(&mut f, Figure::CarlFriedrichGauss, A);
    let seven = deck(&mut f, 7, 1);
    deck(&mut f, 6, 1);
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    first(&mut game);
    assert!(hand(&game, A).contains(&seven[0]));
    assert_eq!(hand(&game, A).len(), 2);
}

#[test]
fn franklin_takes_an_opposing_top_figure_of_the_melded_color() {
    let mut f = Fixture::default();
    put(&mut f, Figure::BenjaminFranklin, A);
    let theirs = put(&mut f, Figure::Gilgamesh, B);
    let color = figure_card(Figure::Gilgamesh.code()).unwrap().color;
    let melded = card(&mut f, color, 1, Zone::Hand(A), &[]);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    first(&mut game);
    assert_eq!(hand(&game, A), [theirs]);
}

#[test]
fn beethoven_junks_the_score_pile_and_rewards_only_fives_twice() {
    let mut f = Fixture::default();
    put(&mut f, Figure::LudwigVanBeethoven, A);
    let old = card(&mut f, Color::Red, 9, Zone::Score(A), &[]);
    let scored = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    deck(&mut f, 5, 8);
    let mut game = new(f);
    start(&mut game, score_op(scored, Zone::Hand(A), A), A);
    first(&mut game);
    assert!(game.position().cards(Zone::Junk).contains(&old));
    let pile = score(&game, A);
    assert_eq!(pile.len(), 9);
    assert!(pile.contains(&scored));
}

#[test]
fn robert_e_lee_counts_seven_health_and_moves_a_health_top() {
    let mut f = Fixture::default();
    put(&mut f, Figure::RobertELee, A);
    card(
        &mut f,
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        &[Icon::Health, Icon::Health, Icon::Health, Icon::Health],
    );
    card(
        &mut f,
        Color::Green,
        1,
        Zone::Board(A, Color::Green),
        &[Icon::Health],
    );
    let game = new(f);
    // Lee's own two Health make seven.
    assert_eq!(game.position().effective_achievement_count(A), 1);

    let mut f = Fixture::default();
    put(&mut f, Figure::RobertELee, A);
    let source = demand(&mut f, Color::Blue, B, vec![]);
    let health = card(
        &mut f,
        Color::Green,
        2,
        Zone::Board(B, Color::Green),
        &[Icon::Health],
    );
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| {
                *o == Alternative::Branch(0)
                    || matches!(o, Alternative::Select { target: TargetDescriptor::Known { card, .. } } if *card == health)
            })
    });
    assert_eq!(game.position().top(A, Color::Green), Some(health));
}

#[test]
fn rowland_hill_returns_three_and_achieves_the_rest_of_the_hand() {
    let mut f = Fixture::default();
    put(&mut f, Figure::RowlandHill, A);
    let cards: Vec<_> = (1..=5)
        .map(|v| card(&mut f, Color::Blue, v, Zone::Hand(A), &[]))
        .collect();
    let claimed = card(&mut f, Color::Red, 1, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    start(
        &mut game,
        Operation::Claim {
            target: target(claimed, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Ignore,
        },
        A,
    );
    first(&mut game);
    assert!(hand(&game, A).is_empty());
    assert_eq!(game.position().cards(Zone::Achievements(A)).len(), 3);
    let _ = cards;
}

#[test]
fn stephenson_splays_right_colors_up_and_supplies_bottom_yellows() {
    let mut f = Fixture::default();
    put(&mut f, Figure::GeorgeStephenson, A);
    card(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue), &[]);
    let source = program(&mut f, Color::Blue, 1, A, vec![]);
    let mut game = new(f);
    start(
        &mut game,
        Operation::Splay {
            player: A,
            color: Color::Blue,
            direction: Splay::Right,
        },
        A,
    );
    first(&mut game);
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Up);

    let mut f = Fixture::default();
    put(&mut f, Figure::GeorgeStephenson, A);
    let mine = card(&mut f, Color::Yellow, 1, Zone::Board(A, Color::Yellow), &[]);
    card(&mut f, Color::Yellow, 2, Zone::Board(A, Color::Yellow), &[]);
    let theirs = card(&mut f, Color::Yellow, 3, Zone::Board(B, Color::Yellow), &[]);
    let claimed = card(&mut f, Color::Red, 1, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    start(
        &mut game,
        Operation::Claim {
            target: target(claimed, Zone::AvailableAchievements),
            eligibility: ClaimEligibility::Ignore,
        },
        A,
    );
    first(&mut game);
    let supply = game.position().cards(Zone::AvailableAchievements);
    assert!(supply.contains(&mine) && supply.contains(&theirs));
}

#[test]
fn queen_victoria_scores_or_supplies_a_figure_from_any_score_pile() {
    for (branch, supplied) in [(0, false), (1, true)] {
        let mut f = Fixture::default();
        put(&mut f, Figure::QueenVictoria, A);
        let figure = f.figure(Figure::Gilgamesh, Zone::Score(B));
        let scored = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
        let mut game = new(f);
        start(&mut game, score_op(scored, Zone::Hand(A), A), A);
        run(&mut game, |_, menu| {
            menu.options
                .iter()
                .position(|o| *o == Alternative::Branch(branch))
        });
        assert_eq!(
            game.position()
                .cards(Zone::AvailableAchievements)
                .contains(&figure),
            supplied
        );
        assert_eq!(score(&game, A).contains(&figure), !supplied);
    }
}

#[test]
fn rommel_scores_both_tops_of_the_color_and_a_score_pile_card_instead() {
    let mut f = Fixture::default();
    put(&mut f, Figure::ErwinRommel, A);
    let mine = card(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue), &[]);
    let theirs = card(&mut f, Color::Blue, 2, Zone::Board(B, Color::Blue), &[]);
    let pile = card(&mut f, Color::Green, 3, Zone::Score(B), &[]);
    let scored = card(&mut f, Color::Blue, 4, Zone::Hand(A), &[]);
    let mut game = new(f);
    start(&mut game, score_op(scored, Zone::Hand(A), A), A);
    run(&mut game, |_, menu| {
        menu.options.iter().position(|o| {
            matches!(
                o,
                Alternative::Move {
                    target: TargetDescriptor::Hidden {
                        source: Zone::Score(B),
                        ..
                    },
                    ..
                }
            )
        })
    });
    assert_eq!(hand(&game, A), [scored]);
    let mut pile_a = score(&game, A);
    pile_a.sort();
    assert_eq!(pile_a, [mine, theirs, pile]);
}

#[test]
fn caresse_crosby_splays_left_draws_twos_and_wins_with_a_fifth_left() {
    let mut f = Fixture::default();
    put(&mut f, Figure::CaresseCrosby, A);
    let source = program(&mut f, Color::Red, 1, B, vec![]);
    for color in [Color::Red, Color::Blue, Color::Green, Color::Purple] {
        for value in [1, 2] {
            card(&mut f, color, value, Zone::Board(A, color), &[]);
        }
    }
    deck(&mut f, 2, 2);
    let mut game = new(f);
    for color in [Color::Blue, Color::Green, Color::Purple] {
        start(
            &mut game,
            Operation::Splay {
                player: A,
                color,
                direction: Splay::Left,
            },
            A,
        );
        first(&mut game);
    }
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    let (status, _) = first(&mut game);
    // Caresse's own yellow pile is a single card: four colors left, no win.
    assert_eq!(game.position().splay(A, Color::Red), Splay::Left);
    assert!(matches!(status, Status::Complete(_)));
    assert_eq!(hand(&game, A).len(), 2);
}

#[test]
fn tesla_scores_a_plain_opposing_top_and_draws_if_it_is_highest() {
    let mut f = Fixture::default();
    put(&mut f, Figure::NikolaTesla, A);
    let theirs = card(&mut f, Color::Green, 5, Zone::Board(B, Color::Green), &[]);
    card(
        &mut f,
        Color::Blue,
        6,
        Zone::Board(B, Color::Blue),
        &[Icon::Concept],
    );
    card(&mut f, Color::Red, 4, Zone::Score(A), &[]);
    let melded = card(&mut f, Color::Purple, 1, Zone::Hand(A), &[]);
    let nine = deck(&mut f, 9, 1);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    first(&mut game);
    assert!(score(&game, A).contains(&theirs));
    assert_eq!(hand(&game, A), nine);
}

#[test]
fn jp_morgan_splays_the_dogma_color_up_on_its_owners_board() {
    let mut f = Fixture::default();
    put(&mut f, Figure::JPMorgan, A);
    let source = program(&mut f, Color::Blue, 1, B, vec![]);
    for value in [1, 2] {
        card(&mut f, Color::Blue, value, Zone::Board(A, Color::Blue), &[]);
    }
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(game.position().splay(A, Color::Blue), Splay::Up);
}

#[test]
fn noether_squares_efficiency_and_junks_from_the_scorers_pile() {
    let mut f = Fixture::default();
    put(&mut f, Figure::EmmyNoether, A);
    let game = new(f);
    // Noether's own two Efficiency: 2 * 2 points.
    assert_eq!(game.position().score(A), 4);

    let mut f = Fixture::default();
    put(&mut f, Figure::EmmyNoether, A);
    let old = card(&mut f, Color::Red, 3, Zone::Score(B), &[]);
    let scored = card(&mut f, Color::Blue, 1, Zone::Hand(B), &[]);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    start(&mut game, score_op(scored, Zone::Hand(B), B), B);
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Junk), [old]);
    assert_eq!(score(&game, B), [scored]);
}

#[test]
fn einstein_melds_every_concept_or_efficiency_card_from_both_hands() {
    let mut f = Fixture::default();
    put(&mut f, Figure::AlbertEinstein, A);
    let mine = card(&mut f, Color::Red, 1, Zone::Hand(A), &[Icon::Concept]);
    let theirs = card(&mut f, Color::Green, 1, Zone::Hand(B), &[Icon::Efficiency]);
    let kept = card(&mut f, Color::Yellow, 1, Zone::Hand(B), &[]);
    deck(&mut f, 8, 1);
    let mut game = new(f);
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    first(&mut game);
    assert_eq!(game.position().top(A, Color::Red), Some(mine));
    assert_eq!(game.position().top(A, Color::Green), Some(theirs));
    assert_eq!(hand(&game, B), [kept]);
}

/// Each hand card the events revealed, in order.
fn revealed_from(events: &[Event], player: Player) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, source, .. } if *source == Zone::Hand(player) => Some(*card),
            _ => None,
        })
        .collect()
}

/// The known cards a meld menu offers.
fn offered_melds(menu: &Decision) -> Vec<CardId> {
    menu.options
        .iter()
        .map(|option| match option {
            Alternative::Move {
                verb: Verb::Meld,
                target: TargetDescriptor::Known { card, .. },
            } => *card,
            other => panic!("expected a known meld, got {other:?}"),
        })
        .collect()
}

#[test]
fn einstein_reveals_both_hands_then_orders_same_color_melds_among_known_cards() {
    let mut f = Fixture::default();
    put(&mut f, Figure::AlbertEinstein, A);
    let mine = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    let low = card(&mut f, Color::Red, 1, Zone::Hand(B), &[Icon::Concept]);
    let kept = card(&mut f, Color::Yellow, 1, Zone::Hand(B), &[]);
    let high = card(&mut f, Color::Red, 2, Zone::Hand(B), &[Icon::Efficiency]);
    deck(&mut f, 8, 1);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    // Both red cards land on one pile, so their order is a real choice,
    // made after both hands were shown.
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    assert_eq!(offered_melds(&menu), [low, high]);
    assert_eq!(revealed_from(&events, A), [mine]);
    assert_eq!(revealed_from(&events, B), [low, kept, high]);
    assert!(events.iter().all(|event| !matches!(event, Event::Moved(_))));
    choose_card(&mut game, &mut events, high);
    let (_, rest) = first(&mut game);
    events.extend(rest);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [high, low]
    );
    assert_eq!(hand(&game, B), [kept]);
    // Nothing is revealed twice.
    assert_eq!(revealed_from(&events, B), [low, kept, high]);
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
    }
}

#[test]
fn ruth_handler_reveals_both_hands_then_orders_the_melds_among_known_cards() {
    let mut f = Fixture::default();
    put(&mut f, Figure::RuthHandler, A);
    let melded = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
    let mine = card(&mut f, Color::Red, 2, Zone::Hand(A), &[]);
    let theirs = card(&mut f, Color::Red, 3, Zone::Hand(B), &[]);
    let kept = card(&mut f, Color::Blue, 3, Zone::Hand(B), &[]);
    deck(&mut f, 9, 3);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(offered_melds(&menu), [mine, theirs]);
    // Every card shows before any moves: both hands, then the melded card
    // when its color is read.
    assert_eq!(revealed_from(&events, A), [mine, melded]);
    assert!(events.iter().all(|event| !matches!(event, Event::Moved(_))));
    assert_eq!(revealed_from(&events, B), [theirs, kept]);
    choose_card(&mut game, &mut events, theirs);
    let (_, rest) = first(&mut game);
    events.extend(rest);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [theirs, mine, melded]
    );
    assert_eq!(game.position().cards(Zone::Achievements(A)).len(), 2);
    assert_eq!(revealed_from(&events, B), [theirs, kept]);
    for viewer in Player::ALL {
        assert!(game.tracker(viewer).unwrap().check_truth(game.position()));
    }
}

#[test]
fn ruth_handler_melds_matching_hand_cards_and_achieves_a_nine_for_each() {
    let mut f = Fixture::default();
    put(&mut f, Figure::RuthHandler, A);
    let melded = card(&mut f, Color::Red, 1, Zone::Hand(A), &[]);
    card(&mut f, Color::Red, 2, Zone::Hand(A), &[]);
    card(&mut f, Color::Red, 3, Zone::Hand(B), &[]);
    card(&mut f, Color::Blue, 3, Zone::Hand(B), &[]);
    deck(&mut f, 9, 3);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    first(&mut game);
    assert_eq!(game.position().cards(Zone::Board(A, Color::Red)).len(), 3);
    assert_eq!(game.position().top(A, Color::Red), Some(melded));
    assert_eq!(game.position().cards(Zone::Achievements(A)).len(), 2);
}

#[test]
fn che_guevara_scores_a_nine_and_a_yellow_one_junks_scores_and_supply() {
    let mut f = Fixture::default();
    put(&mut f, Figure::CheGuevara, A);
    let theirs = card(&mut f, Color::Red, 3, Zone::Score(B), &[]);
    let supply = card(&mut f, Color::Red, 4, Zone::AvailableAchievements, &[]);
    let nine = card(&mut f, Color::Yellow, 9, Zone::Deck(Deck::base(9)), &[]);
    let melded = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    first(&mut game);
    let junk = game.position().cards(Zone::Junk);
    assert!(junk.contains(&theirs) && junk.contains(&supply) && junk.contains(&nine));
    assert!(score(&game, A).is_empty());
}

/// Whether a menu offers to move or select a card that is in the junk.
fn offers_junk(menu: &Decision) -> bool {
    let in_junk = |target: &TargetDescriptor| match target {
        TargetDescriptor::Known { source, .. } | TargetDescriptor::Hidden { source, .. } => {
            *source == Zone::Junk
        }
    };
    menu.options.iter().any(|option| match option {
        Alternative::Move { target, .. } | Alternative::Select { target } => in_junk(target),
        Alternative::BlindMove { source, .. } | Alternative::BlindSelect { source, .. } => {
            *source == Zone::Junk
        }
        _ => false,
    })
}

#[test]
fn che_guevara_with_von_braun_leaves_the_junk_where_it_is() {
    // Wernher counts the junk as his owner's score pile, but a card already in the junk
    // is not junked again.
    let mut f = Fixture::default();
    put(&mut f, Figure::CheGuevara, A);
    put(&mut f, Figure::WernherVonBraun, B);
    let old_junk = [
        card(&mut f, Color::Red, 2, Zone::Junk, &[]),
        card(&mut f, Color::Green, 5, Zone::Junk, &[]),
        card(&mut f, Color::Purple, 6, Zone::Junk, &[]),
    ];
    let theirs = card(&mut f, Color::Red, 3, Zone::Score(B), &[]);
    let nine = card(&mut f, Color::Yellow, 9, Zone::Deck(Deck::base(9)), &[]);
    let melded = card(&mut f, Color::Blue, 1, Zone::Hand(A), &[]);
    let mut game = new(f);
    let op = meld_op(&game, melded, A);
    start(&mut game, op, A);
    // Junking score piles in any order commutes, Wernher's alias included.
    let (_, events) = run(&mut game, |_, menu| {
        panic!("no choice is needed: {menu:?}");
    });
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Moved(m) if m.source == Zone::Junk)),
        "no card moves out of the junk"
    );
    let junk = game.position().cards(Zone::Junk);
    assert!(old_junk.iter().all(|c| junk.contains(c)));
    assert!(junk.contains(&theirs) && junk.contains(&nine));
}

#[test]
fn junking_from_a_von_braun_score_pile_offers_only_score_cards() {
    for scored in [true, false] {
        let mut f = Fixture::default();
        put(&mut f, Figure::WernherVonBraun, A);
        let junked = [
            card(&mut f, Color::Red, 2, Zone::Junk, &[]),
            card(&mut f, Color::Green, 5, Zone::Junk, &[]),
        ];
        let pile = scored.then(|| card(&mut f, Color::Yellow, 4, Zone::Score(A), &[]));
        let source = program(
            &mut f,
            Color::Red,
            1,
            A,
            vec![build::junk(Selector::Score(Who::Executor))],
        );
        let mut game = new(f);
        dogma(&mut game, source);
        let (_, events) = run(&mut game, |_, menu| {
            assert!(!offers_junk(menu), "a junk card is offered: {menu:?}");
            None
        });
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::Moved(m) if m.source == Zone::Junk))
        );
        let junk = game.position().cards(Zone::Junk);
        assert!(junked.iter().all(|c| junk.contains(c)));
        assert_eq!(pile.is_some_and(|c| junk.contains(&c)), scored);
        assert!(score(&game, A).is_empty());
    }
}

#[test]
fn verstappen_samples_worlds_while_escapism_self_executes_its_revealed_card() {
    // Escapism reveals and junks a hand card, then self-executes it. The
    // junked card is public, so Max Verstappen's reaction to its effect
    // still lets either player sample a world (it once refused: the
    // karma's operand looked hidden once it lay in the junk).
    let mut f = Fixture::default();
    put(&mut f, Figure::MaxVerstappen, A);
    let escapism = base_card("B114").unwrap();
    let color = escapism.color;
    let escapism = f.add(escapism, Zone::Board(A, color));
    let revealed = f.tech(Color::Red, 2, Zone::Hand(A), vec![build::draw(1)]);
    // Same-back cards already in the junk keep the tracker from pinning it.
    for c in [Color::Blue, Color::Green, Color::Yellow] {
        f.tech(c, 2, Zone::Junk, vec![]);
    }
    deck(&mut f, 1, 3);
    deck(&mut f, 2, 4);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.begin_turn(A).unwrap();
    dogma(&mut game, escapism);
    let mut reacted = false;
    let mut events = Vec::new();
    for _ in 0..50 {
        let status = settle(&mut game, &mut events);
        let Status::Decision(menu) = status else {
            break;
        };
        let source = menu
            .source_card(menu.player)
            .map(|c| game.position().card(c).code);
        reacted |= source == Some("F106") && game.position().cards(Zone::Junk).contains(&revealed);
        for seed in 0..4 {
            let world = game
                .determinize(menu.player, seed)
                .unwrap_or_else(|error| panic!("{source:?}: {error:?}"));
            assert_eq!(
                world.observe(menu.player).unwrap(),
                game.observe(menu.player).unwrap()
            );
        }
        let decline = menu.options.iter().position(|o| *o == Alternative::Decline);
        game.choose(decline.unwrap_or(0)).unwrap();
    }
    assert!(reacted, "Verstappen reacted to the junked card's effect");
}

#[test]
fn candles_draw_reactions_can_be_searched_by_the_demander() {
    // Candles' victim transfers a Concept card (or proves it has none) and
    // then draws a [1]; Marie Curie and Rod Serling both react, and the
    // demander orders them. The victim's hand once left a private branch
    // in Candles that kept the demander from sampling.
    let mut f = Fixture::default();
    put(&mut f, Figure::MarieCurie, A);
    put(&mut f, Figure::RodSerling, B);
    let candles = echoes_card("E002").unwrap();
    let color = candles.color;
    let candles = f.add(candles, Zone::Board(B, color));
    card(&mut f, Color::Purple, 5, Zone::Hand(A), &[Icon::Concept]);
    card(&mut f, Color::Red, 6, Zone::Hand(A), &[Icon::Food]);
    // Same-back cards elsewhere keep the victim's hand ambiguous.
    for (color, icon) in [(Color::Blue, Icon::Food), (Color::Green, Icon::Concept)] {
        card(&mut f, color, 5, Zone::Deck(Deck::base(5)), &[icon]);
        card(&mut f, color, 6, Zone::Deck(Deck::base(6)), &[icon]);
    }
    deck(&mut f, 1, 3);
    deck(&mut f, 9, 3);
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::ECHOES))
        .unwrap()
        .with_tracking()
        .unwrap();
    game.begin_turn(B).unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    context.source_owner = B;
    game.start(
        Operation::ExecuteEffect {
            card: candles,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
    let mut ordered = false;
    let mut events = Vec::new();
    for _ in 0..30 {
        let Status::Decision(menu) = settle(&mut game, &mut events) else {
            break;
        };
        ordered |= menu.player == B
            && menu
                .options
                .iter()
                .any(|o| matches!(o, Alternative::Karma { .. }));
        for seed in 0..4 {
            let world = game
                .determinize(menu.player, seed)
                .unwrap_or_else(|error| panic!("{:?}: {error:?}", menu.player));
            assert_eq!(
                world.observe(menu.player).unwrap(),
                game.observe(menu.player).unwrap()
            );
        }
        game.choose(0).unwrap();
    }
    assert!(ordered, "the demander ordered the draw reactions");
}

#[test]
fn nightingale_guards_only_score_piles_so_fermi_sweeps_the_junk_freely() {
    for von_braun in [false, true] {
        let mut f = Fixture::default();
        put(&mut f, Figure::Nightingale, B);
        if von_braun {
            // The junk is then B's score pile, which Nightingale guards.
            put(&mut f, Figure::WernherVonBraun, B);
        }
        let fermi = unseen_card("U093").unwrap();
        let color = fermi.color;
        let fermi = f.add(fermi, Zone::Board(A, color));
        let junk = [
            card(&mut f, Color::Red, 2, Zone::Junk, &[]),
            card(&mut f, Color::Blue, 5, Zone::Junk, &[]),
            card(&mut f, Color::Green, 7, Zone::Junk, &[]),
        ];
        let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::UNSEEN))
            .unwrap()
            .with_tracking()
            .unwrap();
        game.begin_turn(A).unwrap();
        game.start(
            Operation::ExecuteEffect {
                card: fermi,
                effect: 1,
                boxed: BoxedValues::default(),
            },
            Context::new(A),
        )
        .unwrap();
        let mut decisions = 0;
        run(&mut game, |_, _| {
            decisions += 1;
            None
        });
        let hand = hand(&game, A);
        if von_braun {
            assert!(
                decisions > 0,
                "Nightingale may react, so the order is asked"
            );
            assert!(junk.iter().all(|card| !hand.contains(card)));
        } else {
            assert_eq!(decisions, 0, "Nightingale cannot react to junk transfers");
            assert!(junk.iter().all(|card| hand.contains(card)));
        }
    }
}

#[test]
fn ohno_achieves_a_matching_featured_card_or_draws_an_eleven() {
    for achieve in [true, false] {
        let mut f = Fixture::default();
        put(&mut f, Figure::TaiichiOhno, A);
        let source = program(&mut f, Color::Red, 1, A, vec![]);
        let matching = f.tech(Color::Blue, 3, Zone::Hand(A), vec![]);
        let eleven = deck(&mut f, 11, 1);
        let mut game = new(f);
        dogma(&mut game, source);
        run(&mut game, |_, menu| {
            if achieve {
                None
            } else {
                menu.options.iter().position(|o| *o == Alternative::Decline)
            }
        });
        assert_eq!(
            game.position()
                .cards(Zone::Achievements(A))
                .contains(&matching),
            achieve
        );
        assert_eq!(hand(&game, A).contains(&eleven[0]), !achieve);
    }
}

#[test]
fn von_braun_counts_junk_as_score_and_junks_a_deck_before_figure_draws() {
    let mut f = Fixture::default();
    put(&mut f, Figure::WernherVonBraun, A);
    card(&mut f, Color::Red, 7, Zone::Junk, &[]);
    let game = new(f);
    assert_eq!(game.position().score(A), 7);

    let mut f = Fixture::default();
    put(&mut f, Figure::WernherVonBraun, A);
    let figure = f.figure(Figure::Saladin, Zone::Deck(Deck::figures(3)));
    let tens = deck(&mut f, 10, 2);
    deck(&mut f, 3, 1);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    start(
        &mut game,
        Operation::Draw {
            value: 3,
            set: Some(CardSet::Figures),
        },
        B,
    );
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(10))
    });
    assert!(
        tens.iter()
            .all(|c| game.position().cards(Zone::Junk).contains(c))
    );
    assert_eq!(hand(&game, B), [figure]);
}

#[test]
fn von_braun_skips_its_deck_choice_when_both_base_decks_are_empty() {
    let mut f = Fixture::default();
    put(&mut f, Figure::WernherVonBraun, A);
    let figure = f.figure(Figure::Saladin, Zone::Deck(Deck::figures(3)));
    let three = deck(&mut f, 3, 1);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    start(
        &mut game,
        Operation::Draw {
            value: 3,
            set: Some(CardSet::Figures),
        },
        B,
    );
    run(&mut game, |_, menu| {
        assert!(
            !menu
                .options
                .iter()
                .any(|o| matches!(o, Alternative::Value(_)))
        );
        None
    });
    assert!(game.position().cards(Zone::Junk).is_empty());
    assert_eq!(hand(&game, B), [figure]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(3))), three);
}

#[test]
fn rod_serling_makes_a_low_value_mover_lose() {
    let mut f = Fixture::default();
    put(&mut f, Figure::RodSerling, A);
    let low = card(&mut f, Color::Blue, 3, Zone::Hand(B), &[]);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    start(&mut game, score_op(low, Zone::Hand(B), B), B);
    let (status, _) = first(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert!(hand(&game, B).is_empty() && game.position().cards(Zone::Junk).contains(&low));
}

#[test]
fn thatcher_scores_a_prosperity_or_industry_top_before_dogma() {
    let mut f = Fixture::default();
    put(&mut f, Figure::MargaretThatcher, A);
    let source = program(&mut f, Color::Blue, 1, A, vec![]);
    let theirs = card(
        &mut f,
        Color::Green,
        4,
        Zone::Board(B, Color::Green),
        &[Icon::Industry],
    );
    let mut game = new(f);
    dogma(&mut game, source);
    run(&mut game, |_, menu| moving(menu, theirs));
    assert_eq!(score(&game, A), [theirs]);
}

#[test]
fn hawking_scores_bottom_cards_per_hex_and_tucks_a_same_value_draw() {
    let mut f = Fixture::default();
    put(&mut f, Figure::StephenHawking, A);
    let source = program(&mut f, Color::Red, 3, A, vec![]);
    f.cards[usize::from(source.0)].symbols =
        four_slots([Symbol::Image, Symbol::Image, Symbol::Blank, Symbol::Blank]);
    f.placements.rotate_right(1);
    let bottoms: Vec<_> = (1..=2)
        .map(|v| card(&mut f, Color::Red, v, Zone::Board(A, Color::Red), &[]))
        .collect();
    f.placements.rotate_right(2);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    let mut scored = score(&game, A);
    scored.sort();
    assert_eq!(scored, bottoms);

    let mut f = Fixture::default();
    put(&mut f, Figure::StephenHawking, A);
    let fours = deck(&mut f, 4, 2);
    let mut game = new(f);
    start(&mut game, draw_op(4), A);
    first(&mut game);
    assert_eq!(hand(&game, A), [fours[0]]);
    assert_eq!(
        game.position()
            .cards(Zone::Board(A, game.position().card(fours[1]).color))[0],
        fours[1]
    );
}

#[test]
fn yuna_kim_wins_on_an_authority_score_and_turns_melds_into_tucks() {
    let mut f = Fixture::default();
    put(&mut f, Figure::YunaKim, A);
    card(
        &mut f,
        Color::Red,
        1,
        Zone::Board(A, Color::Red),
        &[Icon::Authority],
    );
    let scored = card(&mut f, Color::Red, 2, Zone::Hand(A), &[]);
    let mut game = new(f);
    start(&mut game, score_op(scored, Zone::Hand(A), A), A);
    assert_eq!(first(&mut game).0, Status::Winner(A));

    let mut f = Fixture::default();
    put(&mut f, Figure::YunaKim, A);
    for value in 1..=4 {
        card(
            &mut f,
            Color::Green,
            value,
            Zone::Board(B, Color::Green),
            &[],
        );
    }
    let melded = card(&mut f, Color::Green, 5, Zone::Hand(B), &[]);
    let mut game = new(f).with_narration();
    game.begin_turn(B).unwrap();
    let op = meld_op(&game, melded, B);
    start(&mut game, op, B);
    let (_, events) = first(&mut game);
    let pile = game.position().cards(Zone::Board(B, Color::Green));
    assert_eq!(pile.len(), 2);
    assert_eq!(pile[0], melded);
    // A narrated game announces the karma once, before its effects.
    let yuna = game
        .position()
        .top(A, figure_card(Figure::YunaKim.code()).unwrap().color);
    let triggers: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::KarmaTriggered { .. }))
        .collect();
    assert_eq!(triggers.len(), 1);
    let (
        at,
        &Event::KarmaTriggered {
            owner,
            card,
            clause,
            reaction,
            actor,
            would,
        },
    ) = triggers[0]
    else {
        unreachable!();
    };
    assert_eq!((owner, Some(card), clause), (A, yuna, 1));
    // B would meld the card, but instead the karma runs.
    assert_eq!((reaction, actor), (Reaction::Instead, B));
    assert!(matches!(
        would,
        Some(Operation::Move { verb: Verb::Meld, target, .. }) if target.card == melded
    ));
    assert!(!events[..at].iter().any(|e| matches!(e, Event::Moved(_))));
}

#[test]
fn sheikh_tamim_returns_a_featured_icon_card_to_issue_its_colors_decree() {
    let mut f = Fixture::default();
    put(&mut f, Figure::SheikhTamim, A);
    let source = program(&mut f, Color::Blue, 1, A, vec![]);
    let returned = card(&mut f, Color::Yellow, 2, Zone::Hand(A), &[Icon::Authority]);
    for value in [1, 2] {
        card(
            &mut f,
            Color::Purple,
            value,
            Zone::Board(A, Color::Purple),
            &[],
        );
    }
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(2)))
            .contains(&returned)
    );
    assert_eq!(
        game.position().decree_location(Decree::Expansion),
        DecreeLocation::Owned(A)
    );
    assert_eq!(game.position().splay(A, Color::Purple), Splay::Up);
}

#[test]
fn greta_junks_another_industry_top_and_wins_when_none_remain() {
    let mut f = Fixture::default();
    put(&mut f, Figure::GretaThunberg, A);
    let source = program(&mut f, Color::Red, 1, B, vec![]);
    let industry = card(
        &mut f,
        Color::Green,
        2,
        Zone::Board(B, Color::Green),
        &[Icon::Industry],
    );
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    let (status, _) = first(&mut game);
    assert!(game.position().cards(Zone::Junk).contains(&industry));
    assert_eq!(status, Status::Winner(A));
}

#[test]
fn kirongozi_makes_the_opponent_draw_reveal_return_and_super_execute() {
    let mut f = Fixture::default();
    put(&mut f, Figure::ThereseKirongozi, A);
    let source = program(&mut f, Color::Blue, 1, B, vec![]);
    let executed = program(&mut f, Color::Green, 1, B, vec![build::draw(4)]);
    let three = card(&mut f, Color::Green, 3, Zone::Deck(Deck::base(3)), &[]);
    let four = deck(&mut f, 4, 1);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    run(&mut game, |_, menu| {
        menu.options
            .iter()
            .position(|o| *o == Alternative::Value(3))
    });
    assert!(
        game.position()
            .cards(Zone::Deck(Deck::base(3)))
            .contains(&three)
    );
    assert_eq!(hand(&game, B), four);
    let _ = executed;
}

#[test]
fn scionti_draws_and_scores_a_nine_before_each_dogma_effect() {
    let mut f = Fixture::default();
    put(&mut f, Figure::GiuseppeScionti, A);
    let source = program(&mut f, Color::Red, 1, A, vec![]);
    let nines = deck(&mut f, 9, 1);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(score(&game, A), nines);
}

#[test]
fn scionti_rewards_the_demander_not_the_victim_for_a_demand_effect() {
    for owner in [A, B] {
        for copied in [false, true] {
            let mut f = Fixture::default();
            put(&mut f, Figure::GiuseppeScionti, owner);
            let source = demand(&mut f, Color::Red, A, vec![build::draw(1)]);
            let drawn = deck(&mut f, 1, 1);
            let nines = deck(&mut f, 9, 1);
            let mut game = new(f);
            if copied {
                game.super_execute(source, Context::new(A)).unwrap();
            } else {
                dogma(&mut game, source);
            }
            let (status, events) = first(&mut game);
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(hand(&game, B), drawn);
            assert!(hand(&game, A).is_empty());
            assert!(score(&game, B).is_empty());
            if owner == A {
                assert_eq!(score(&game, A), nines);
                assert!(moved(&events[0]).inside_karma);
                assert_eq!(moved(&events[0]).executor, A);
            } else {
                assert!(score(&game, A).is_empty());
                assert_eq!(events.len(), 1);
            }
            assert_eq!(moved(events.last().unwrap()).executor, B);
            assert!(!moved(events.last().unwrap()).inside_karma);
        }
    }
}

#[test]
fn novoselov_super_executes_every_top_in_color_order_as_the_second_action() {
    let mut f = Fixture::default();
    put(&mut f, Figure::KonstantinNovoselov, A);
    let source = program(&mut f, Color::Red, 1, A, vec![build::draw(1)]);
    program(&mut f, Color::Green, 1, A, vec![build::draw(2)]);
    let ones = deck(&mut f, 1, 1);
    let twos = deck(&mut f, 2, 1);
    let mut game = new(f);
    game.begin_action(ActionKind::Regular).unwrap();
    dogma(&mut game, source);
    let (_, events) = first(&mut game);
    let drawn: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Moved(m) if m.verb.is_none() => Some(m.card),
            _ => None,
        })
        .collect();
    assert_eq!(drawn, [ones[0], twos[0]]);
}

/// A top card whose single effect is this program, showing one [Authority]
/// so its Dogma player outnumbers a blank opponent.
fn authority(f: &mut Fixture, color: Color, player: Player, body: Vec<Instruction>) -> CardId {
    let id = program(f, color, 1, player, body);
    f.cards[usize::from(id.0)].symbols[0] = Symbol::Icon(Icon::Authority);
    id
}

fn deck_cards(game: &Game, deck: Deck) -> Vec<CardId> {
    game.position().cards(Zone::Deck(deck)).to_vec()
}

#[test]
fn sargon_makes_an_ineligible_owner_share_and_junks_a_one_achievement() {
    for sargon in [false, true] {
        let mut f = Fixture::default();
        if sargon {
            put(&mut f, Figure::Sargon, A);
        }
        let source = authority(&mut f, Color::Blue, B, vec![build::draw(1)]);
        let one = card(&mut f, Color::Red, 1, Zone::AvailableAchievements, &[]);
        deck(&mut f, 1, 4);
        let mut game = new(f);
        game.begin_turn(B).unwrap();
        dogma(&mut game, source);
        first(&mut game);
        assert_eq!(hand(&game, A).len(), usize::from(sargon));
        // Sharing earned B the free draw.
        assert_eq!(hand(&game, B).len(), 1 + usize::from(sargon));
        assert_eq!(game.position().cards(Zone::Junk).contains(&one), sargon);
    }
}

#[test]
fn sargon_owner_shares_but_stays_vulnerable_to_demands_and_compels() {
    for sargon in [false, true] {
        let mut f = Fixture::default();
        if sargon {
            put(&mut f, Figure::Sargon, A);
        }
        let source = demand(&mut f, Color::Blue, B, vec![build::draw(1)]);
        for kind in [EffectKind::Compel, EffectKind::NonDemand] {
            f.cards[usize::from(source.0)].effects.push(Effect {
                kind,
                instructions: vec![build::draw(1)],
            });
        }
        card(&mut f, Color::Red, 1, Zone::AvailableAchievements, &[]);
        deck(&mut f, 1, 6);
        let mut game = new(f);
        game.begin_turn(B).unwrap();
        dogma(&mut game, source);
        first(&mut game);
        // Fewer icons: the demand either way. Eligible through Sargon: the compel and
        // the shared effect too.
        assert_eq!(hand(&game, A).len(), if sargon { 3 } else { 1 });
        // B's own draw, plus the share draw Sargon's sharing earned.
        assert_eq!(hand(&game, B).len(), 1 + usize::from(sargon));
    }
}

#[test]
fn sun_tzu_melds_featured_icon_cards_before_the_share_draw() {
    let mut f = Fixture::default();
    put(&mut f, Figure::SunTzu, A);
    let source = program(&mut f, Color::Blue, 1, A, vec![build::draw(1)]);
    // B shows more [Authority] than Sun Tzu, so B shares.
    card(
        &mut f,
        Color::Green,
        1,
        Zone::Board(B, Color::Green),
        &[Icon::Authority; 4],
    );
    let featured = card(&mut f, Color::Yellow, 1, Zone::Hand(A), &[Icon::Authority]);
    let plain = card(&mut f, Color::Purple, 1, Zone::Hand(A), &[]);
    deck(&mut f, 1, 4);
    deck(&mut f, 2, 2);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Yellow)),
        [featured]
    );
    let hand = hand(&game, A);
    assert!(hand.contains(&plain));
    // The plain card, A's own draw and the share draw of A's top value 2.
    assert_eq!(hand.len(), 3);
}

#[test]
fn murasaki_may_achieve_a_score_card_eligible_for_both_without_its_points() {
    let mut f = Fixture::default();
    put(&mut f, Figure::MurasakiShikibu, A);
    let one = card(&mut f, Color::Red, 1, Zone::Score(A), &[]);
    let six = card(&mut f, Color::Blue, 6, Zone::Score(A), &[]);
    let claimed = card(&mut f, Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    game.take_action(Action::Achieve { card: claimed }, ActionKind::Regular)
        .unwrap();
    let mut offered = Vec::new();
    run(&mut game, |_, menu| {
        offered.push(menu.options.clone());
        moving(menu, one)
    });
    // The six fails its top-value test; only the one (and declining) remain.
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].len(), 2);
    let achievements = game.position().cards(Zone::Achievements(A));
    assert_eq!(achievements, [one, claimed]);
    assert_eq!(score(&game, A), [six]);
}

#[test]
fn al_kindi_draws_two_before_its_own_share_draw_and_scores_before_the_opponents() {
    let mut f = Fixture::default();
    put(&mut f, Figure::AlKindi, A);
    let source = program(&mut f, Color::Blue, 1, A, vec![build::draw(1)]);
    deck(&mut f, 1, 4);
    let threes = deck(&mut f, 3, 3);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    let drawn = hand(&game, A);
    assert_eq!(drawn.len(), 4);
    assert!(threes.iter().all(|card| drawn.contains(card)));

    let mut f = Fixture::default();
    put(&mut f, Figure::AlKindi, B);
    let source = program(&mut f, Color::Blue, 1, A, vec![build::draw(1)]);
    deck(&mut f, 1, 4);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    // B shared one draw, then scored it before A's share draw.
    assert_eq!(score(&game, B).len(), 1);
    assert!(hand(&game, B).is_empty());
    assert_eq!(hand(&game, A).len(), 2);
}

#[test]
fn huygens_replaces_the_first_dogma_and_returns_drawn_cards_with_more_efficiency() {
    for efficient in [false, true] {
        let mut f = Fixture::default();
        put(&mut f, Figure::ChristiaanHuygens, A);
        let source = program(&mut f, Color::Red, 1, A, vec![build::draw(1)]);
        let sixes = deck(&mut f, 6, 3);
        let icons: &[Icon] = if efficient { &[Icon::Efficiency] } else { &[] };
        let eights: Vec<_> = (0..2)
            .map(|_| card(&mut f, Color::Green, 8, Zone::Deck(Deck::base(8)), icons))
            .collect();
        deck(&mut f, 1, 2);
        let mut game = new(f);
        dogma(&mut game, source);
        let mut revealed = 0;
        let (_, events) = run(&mut game, |_, menu| {
            menu.options.iter().position(|option| {
                *option
                    == Alternative::Draw {
                        value: 8,
                        set: CardSet::Base,
                    }
            })
        });
        for event in &events {
            if matches!(event, Event::Revealed { card, .. } if eights.contains(card)) {
                revealed += 1;
            }
        }
        assert_eq!(revealed, 2);
        assert!(deck_cards(&game, Deck::base(6)).is_empty());
        assert!(
            sixes
                .iter()
                .all(|c| game.position().cards(Zone::Junk).contains(c))
        );
        // The replaced Dogma drew no [1].
        let hand = hand(&game, A);
        assert_eq!(hand.len(), if efficient { 0 } else { 2 });
        assert_eq!(
            deck_cards(&game, Deck::base(8)).len(),
            if efficient { 2 } else { 0 }
        );
    }
}

#[test]
fn grace_hopper_reveals_a_ten_when_the_opponent_earns_no_share_draw() {
    for (color, wins) in [(Color::Blue, true), (Color::Red, false)] {
        let mut f = Fixture::default();
        put(&mut f, Figure::GraceHopper, A);
        let source = program(&mut f, Color::Green, 1, B, vec![]);
        let ten = card(&mut f, color, 10, Zone::Deck(Deck::base(10)), &[]);
        let mut game = new(f);
        game.begin_turn(B).unwrap();
        dogma(&mut game, source);
        let (status, _) = first(&mut game);
        assert_eq!(status == Status::Winner(A), wins);
        if !wins {
            assert_eq!(hand(&game, A), [ten]);
        }
    }
}

#[test]
fn yunus_returns_the_bottom_green_card_for_sole_majority_in_the_action() {
    let mut f = Fixture::default();
    let bottom = card(&mut f, Color::Green, 2, Zone::Board(A, Color::Green), &[]);
    put(&mut f, Figure::MuhammadYunus, A);
    let source = demand(&mut f, Color::Blue, B, vec![build::draw(1)]);
    deck(&mut f, 1, 4);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    first(&mut game);
    // Without the majority A would have been demanded to draw.
    assert!(hand(&game, A).is_empty());
    assert!(deck_cards(&game, Deck::base(2)).contains(&bottom));
}

#[test]
fn susan_blackmore_self_executes_the_activated_card_around_share_draws() {
    // An opponent's Dogma without a share draw.
    let mut f = Fixture::default();
    put(&mut f, Figure::SusanBlackmore, A);
    let source = authority(&mut f, Color::Green, B, vec![build::draw(1)]);
    deck(&mut f, 1, 4);
    let mut game = new(f);
    game.begin_turn(B).unwrap();
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(hand(&game, A).len(), 1);
    assert_eq!(hand(&game, B).len(), 1);

    // The owner's own share draw.
    let mut f = Fixture::default();
    put(&mut f, Figure::SusanBlackmore, A);
    let source = program(&mut f, Color::Green, 1, A, vec![build::draw(1)]);
    deck(&mut f, 1, 3);
    deck(&mut f, 10, 1);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    assert_eq!(hand(&game, B).len(), 1);
    // Own draw, the self-execution's draw and the share draw at value 10.
    assert_eq!(hand(&game, A).len(), 3);
}

#[test]
fn scorsese_tucks_a_melded_figure_and_claims_any_achievement() {
    let mut f = Fixture::default();
    put(&mut f, Figure::MartinScorsese, A);
    let top = card(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), &[]);
    let figure = f.figure(Figure::Gilgamesh, Zone::Hand(A));
    card(&mut f, Color::Green, 9, Zone::AvailableAchievements, &[]);
    let mut game = new(f);
    let color = game.position().card(figure).color;
    assert_eq!(color, Color::Red);
    game.take_action(Action::Meld { card: figure }, ActionKind::Regular)
        .unwrap();
    let mut offered = Vec::new();
    run(&mut game, |_, menu| {
        offered.push(menu.options.clone());
        menu.options
            .iter()
            .position(|option| *option == Alternative::ClaimSpecial(Special::Wonder))
    });
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [figure, top]
    );
    // Ineligible standard and special achievements are all offered.
    assert_eq!(offered[0].len(), 1 + Special::ALL.len());
    assert_eq!(
        game.position().special_location(Special::Wonder),
        SpecialLocation::Owned(A)
    );
}

#[test]
fn max_verstappen_repeats_an_effect_per_returned_card_then_draws_an_eleven() {
    let mut f = Fixture::default();
    put(&mut f, Figure::MaxVerstappen, A);
    let source = authority(&mut f, Color::Blue, A, vec![build::draw(1)]);
    let returned: Vec<_> = (0..2)
        .map(|_| card(&mut f, Color::Green, 2, Zone::Hand(A), &[]))
        .collect();
    deck(&mut f, 1, 3);
    let eleven = card(&mut f, Color::Red, 11, Zone::Deck(Deck::base(11)), &[]);
    let mut game = new(f);
    dogma(&mut game, source);
    first(&mut game);
    let hand = hand(&game, A);
    assert_eq!(hand.len(), 4);
    assert!(hand.contains(&eleven));
    assert!(
        hand.iter()
            .filter(|&&c| c != eleven)
            .all(|&c| game.position().card(c).value == 1)
    );
    assert!(
        returned
            .iter()
            .all(|c| deck_cards(&game, Deck::base(2)).contains(c))
    );
}

#[test]
fn max_verstappen_repeats_the_owners_demand_against_the_same_victim() {
    for owner in [A, B] {
        for copied in [false, true] {
            let mut f = Fixture::default();
            put(&mut f, Figure::MaxVerstappen, owner);
            let source = demand(
                &mut f,
                Color::Blue,
                A,
                vec![
                    build::draw(1),
                    build::verb(Verb::Transfer, build::LAST).to(Destination::Hand(Who::Demander)),
                ],
            );
            let payment = card(&mut f, Color::Green, 2, Zone::Hand(owner), &[]);
            let ones = deck(&mut f, 1, 2);
            let eleven = deck(&mut f, 11, 1)[0];
            let mut game = new(f);
            if copied {
                game.super_execute(source, Context::new(A)).unwrap();
            } else {
                dogma(&mut game, source);
            }
            let (status, events) = run(&mut game, |_, menu| {
                assert_eq!(menu.player, A, "only the demander may pay to repeat");
                moving(menu, payment)
            });
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            if owner == A {
                assert_eq!(hand(&game, A), [ones[1], eleven, ones[0]]);
                assert!(hand(&game, B).is_empty());
                assert_eq!(deck_cards(&game, Deck::base(2)), [payment]);
                let transfers: Vec<_> = events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Moved(m) if m.verb == Some(Verb::Transfer) => {
                            assert_eq!(m.executor, B);
                            assert_eq!(m.destination, Zone::Hand(A));
                            Some(m.inside_karma)
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(transfers, [true, false]);
            } else {
                assert_eq!(hand(&game, A), [ones[1]]);
                assert_eq!(hand(&game, B), [payment]);
                assert_eq!(events.len(), 2);
                assert!(events.iter().all(|e| !moved(e).inside_karma));
            }
        }
    }
}

#[test]
fn boyan_slat_achieves_a_lone_returned_score_card_before_drawing() {
    for theirs in [false, true] {
        let mut f = Fixture::default();
        put(&mut f, Figure::BoyanSlat, A);
        let mine = card(&mut f, Color::Red, 4, Zone::Score(A), &[]);
        if theirs {
            card(&mut f, Color::Blue, 4, Zone::Score(B), &[]);
        }
        card(&mut f, Color::Blue, 2, Zone::Score(B), &[]);
        let drawn = card(&mut f, Color::Green, 1, Zone::Deck(Deck::base(1)), &[]);
        let mut game = new(f);
        start(&mut game, draw_op(1), A);
        run(&mut game, |_, menu| {
            menu.options
                .iter()
                .position(|option| *option == Alternative::Value(4))
        });
        assert_eq!(hand(&game, A), [drawn]);
        let achieved = game.position().cards(Zone::Achievements(A)).to_vec();
        if theirs {
            assert!(achieved.is_empty());
            assert_eq!(deck_cards(&game, Deck::base(4)).len(), 2);
        } else {
            assert_eq!(achieved, [mine]);
        }
        assert_eq!(score(&game, B).len(), 1);
    }
}

#[test]
fn lily_hevesh_wins_on_five_revealed_colors_or_draws_one_and_splays_aslant() {
    for colors in [
        Color::ALL,
        [
            Color::Red,
            Color::Red,
            Color::Blue,
            Color::Green,
            Color::Yellow,
        ],
    ] {
        let mut f = Fixture::default();
        let bottom = card(&mut f, Color::Purple, 2, Zone::Board(A, Color::Purple), &[]);
        let lily = put(&mut f, Figure::LilyHevesh, A);
        let tops: Vec<_> = colors
            .iter()
            .enumerate()
            .map(|(i, &color)| {
                let age = i as u8 + 1;
                card(&mut f, color, age, Zone::Deck(Deck::base(age)), &[])
            })
            .collect();
        let mut game = new(f);
        game.take_action(Action::Draw, ActionKind::Regular).unwrap();
        let (status, events) = first(&mut game);
        let revealed = events
            .iter()
            .filter(|event| matches!(event, Event::Revealed { .. }))
            .count();
        assert_eq!(revealed, 5);
        if colors == Color::ALL {
            assert_eq!(status, Status::Winner(A));
        } else {
            assert_eq!(hand(&game, A), [tops[0]]);
            assert_eq!(
                game.position().cards(Zone::Board(A, Color::Purple)),
                [bottom, lily]
            );
            assert_eq!(game.position().splay(A, Color::Purple), Splay::Aslant);
        }
    }
}
