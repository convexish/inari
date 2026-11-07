use inno_engine::*;
mod common;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn invoke(game: &mut Game, card: CardId, effect: usize, context: Context) {
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: effect as u16,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}
/// Choose a value; a menu of values present resolves a lone value itself.
fn value(game: &mut Game, events: &mut Vec<Event>, expected: Player, value: u8) {
    let menu = decision(game, events);
    if !menu
        .options
        .iter()
        .any(|option| matches!(option, Alternative::Value(_)))
    {
        return;
    }
    assert_eq!(menu.player, expected);
    game.choose(
        menu.options
            .iter()
            .position(|option| *option == Alternative::Value(value))
            .unwrap(),
    )
    .unwrap();
}
fn finish(game: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..200 {
        match settle(game, events) {
            Status::Decision(menu) => {
                assert!(
                    !menu
                        .options
                        .iter()
                        .any(|option| matches!(option, Alternative::Value(_)))
                );
                let choice = menu
                    .options
                    .iter()
                    .position(|option| *option != Alternative::Decline)
                    .unwrap();
                game.choose(choice).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                assert!(game.position().check_conservation());
                return;
            }
        }
    }
    panic!("chosen-value effect did not complete");
}
fn demand() -> Context {
    let mut context = Context::new(B);
    context.demander = A;
    context
}

fn statistics<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B048", Zone::Board(A, Color::Yellow));
    let first = base(&mut f, "B001", Zone::Score(B));
    let second = base(&mut f, "B008", Zone::Score(B));
    let other = base(&mut f, "B016", Zone::Score(B));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0, demand());
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert_eq!(menu.player, A);
    // Backs show values: [1] and [2] are in the score pile; every other value
    // (never [0] without Age 0) is the same empty choice, offered once as [3].
    assert_eq!(menu.options, [1, 2, 3].map(Alternative::Value).to_vec());
    value(&mut game, &mut events, A, 1);
    // The demander chose a value; the executor's hand is unordered, so both
    // same-value transfers need no further choice.
    let mut clone = game.clone();
    let mut cloned_events = Vec::new();
    finish(&mut game, &mut events);
    finish(&mut clone, &mut cloned_events);
    assert_eq!(game, clone);
    assert_eq!(events, cloned_events);
    assert_eq!(game.position().cards(Zone::Score(B)), [other]);
    let mut hand = game.position().cards(Zone::Hand(B)).to_vec();
    hand.sort_unstable();
    assert_eq!(hand, [first, second]);
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert!(events.iter().all(|event| matches!(event, Event::Moved(m) if m.executor == B && m.verb == Some(Verb::Transfer))));
}
#[test]
fn statistics_demander_chooses_and_same_value_ties_need_no_order() {
    statistics::<false>();
    statistics::<true>();
}

#[test]
fn statistics_yellow_splay_is_independent_of_an_empty_demand() {
    let mut f = Fixture::default();
    base(&mut f, "B004", Zone::Board(A, Color::Yellow));
    let source = base(&mut f, "B048", Zone::Board(A, Color::Yellow));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, demand());
    // An empty score pile makes every value the same choice: no menu.
    finish(&mut game, &mut Vec::new());
    invoke(&mut game, source, 1, Context::new(A));
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().splay(A, Color::Yellow), Splay::Right);
}

fn encyclopedia<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B063", Zone::Junk);
    let first = base(&mut f, "B001", Zone::Score(A));
    let second = base(&mut f, "B002", Zone::Score(A));
    let green = base(&mut f, "B008", Zone::Score(A));
    let other = base(&mut f, "B016", Zone::Score(A));
    let achievement = base(&mut f, "B048", Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    let mut events = Vec::new();
    value(&mut game, &mut events, A, 1);
    let mut declined = game.clone();
    let menu = decision(&mut declined, &mut Vec::new());
    declined
        .choose(
            menu.options
                .iter()
                .position(|option| *option == Alternative::Decline)
                .unwrap(),
        )
        .unwrap();
    finish(&mut declined, &mut Vec::new());
    assert_eq!(
        declined.position().cards(Zone::Score(A)),
        [first, second, green, other]
    );
    choose_card(&mut game, &mut events, second);
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Score(A)), [other]);
    assert_eq!(
        game.position().cards(Zone::Board(A, Color::Red)),
        [second, first]
    );
    assert_eq!(game.position().top(A, Color::Green), Some(green));
    // The separately printed achievement choice is available after declining melds.
    invoke(&mut declined, source, 1, Context::new(A));
    finish(&mut declined, &mut Vec::new());
    assert!(declined.position().cards(Zone::Junk).contains(&achievement));
}
#[test]
fn encyclopedia_commits_to_every_match_and_preserves_within_color_orders() {
    encyclopedia::<false>();
    encyclopedia::<true>();
}

#[test]
fn encyclopedia_requeries_chosen_value_after_logical_score_membership_changes() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B063", Zone::Junk);
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let cover = base(&mut f, "B004", Zone::Hand(A));
    let stays = base(&mut f, "B001", Zone::Hand(A));
    let physical = base(&mut f, "B008", Zone::Score(A));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    let mut events = Vec::new();
    value(&mut game, &mut events, A, 1);
    choose_card(&mut game, &mut events, cover);
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [stays]);
    assert_eq!(game.position().top(A, Color::Green), Some(physical));
}

#[test]
fn sanitation_boxes_are_adjusted_once_then_junk_only_the_exact_physical_deck() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B069", Zone::Junk);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let seven = base(&mut f, "B068", Zone::Deck(Deck::base(7)));
    let eight = base(&mut f, "B081", Zone::Deck(Deck::base(8)));
    let nine = base(&mut f, "B091", Zone::Deck(Deck::base(9)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 1, Context::new(A));
    let mut events = Vec::new();
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.contains(&Alternative::Value(8)));
    assert!(menu.options.contains(&Alternative::Value(9)));
    assert_eq!(menu.options.len(), 2);
    value(&mut game, &mut events, A, 8);
    finish(&mut game, &mut events);
    assert!(game.position().cards(Zone::Junk).contains(&eight));
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(7))), [seven]);
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(9))), [nine]);
    // The now-empty eight is still selectable: no age skipping or exhaustion.
    invoke(&mut game, source, 1, Context::new(A));
    value(&mut game, &mut events, A, 8);
    let before = events.len();
    finish(&mut game, &mut events);
    assert_eq!(events.len(), before);
}

#[test]
fn mass_media_original_payment_failure_and_decline_never_choose_a_value() {
    for pay in [false, true] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B081", Zone::Junk);
        f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
        let payment = f.figure(Figure::ShenKuo, Zone::Hand(A));
        let scored = base(&mut f, "B001", Zone::Score(B));
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut game, source, 0, Context::new(A));
        let mut events = Vec::new();
        if pay {
            choose_card(&mut game, &mut events, payment);
        } else {
            let menu = decision(&mut game, &mut events);
            game.choose(
                menu.options
                    .iter()
                    .position(|o| *o == Alternative::Decline)
                    .unwrap(),
            )
            .unwrap();
        }
        finish(&mut game, &mut events);
        assert_eq!(game.position().cards(Zone::Score(B)), [scored]);
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&payment),
            !pay
        );
        if pay {
            assert!(events.iter().any(|e| matches!(e, Event::Moved(m) if m.card == payment && m.verb == Some(Verb::Tuck) && m.inside_karma)));
        }
    }
    let mut f = Fixture::default();
    let source = base(&mut f, "B081", Zone::Junk);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    finish(&mut game, &mut Vec::new());
}

fn mass_media<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B081", Zone::Junk);
    let payment = base(&mut f, "B016", Zone::Hand(A));
    let own = base(&mut f, "B001", Zone::Score(A));
    let foreign = base(&mut f, "B002", Zone::Score(B));
    let foreign_two = base(&mut f, "B008", Zone::Score(B));
    let other = base(&mut f, "B042", Zone::Score(B));
    let mut game = Game::seeded(f.position(), Expansions::with_figures(F), 73).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    let mut events = Vec::new();
    choose_card(&mut game, &mut events, payment);
    value(&mut game, &mut events, A, 1);
    let menu = decision(&mut game, &mut events);
    assert!(menu.options.contains(&Alternative::BlindMove {
        verb: Verb::Return,
        source: Zone::Score(B),
        back: Deck {
            set: CardSet::Base,
            age: 1
        }
    }));
    assert!(!menu.options.iter().any(|o| matches!(
        o,
        Alternative::Move {
            target: TargetDescriptor::Hidden { .. },
            ..
        }
    )));
    let mut clone = game.clone();
    let mut cloned_events = events.clone();
    finish(&mut game, &mut events);
    finish(&mut clone, &mut cloned_events);
    assert_eq!(game, clone);
    assert_eq!(events, cloned_events);
    assert!(game.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Score(B)), [other]);
    for card in [own, foreign, foreign_two] {
        assert!(
            game.position()
                .cards(Zone::Deck(Deck::base(1)))
                .contains(&card)
        );
    }
    assert_eq!(game.position().cards(Zone::Deck(Deck::base(2))), [payment]);
}
#[test]
fn mass_media_returns_all_matching_piles_with_blind_foreign_selection() {
    mass_media::<false>();
    mass_media::<true>();
}

fn services<const F: bool>() {
    // [1] is in the score pile; [2] stands for every absent value (backs show
    // values, so all of them are one option).
    for selected in [1, 2] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B094", Zone::Board(A, Color::Purple));
        let gift = base(&mut f, "B001", Zone::Board(A, Color::Red));
        let scored = base(&mut f, "B008", Zone::Score(B));
        let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut game, source, 0, demand());
        let mut events = Vec::new();
        value(&mut game, &mut events, A, selected);
        finish(&mut game, &mut events);
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&scored),
            selected == 1
        );
        assert_eq!(
            game.position().cards(Zone::Hand(B)).contains(&gift),
            selected == 1
        );
        assert_eq!(game.position().top(A, Color::Purple), Some(source));
    }
}
#[test]
fn services_absent_value_is_legal_but_only_original_fulfillment_pays_top_without_health() {
    services::<false>();
    services::<true>();
    let mut f = Fixture::default();
    let source = base(&mut f, "B094", Zone::Board(A, Color::Purple));
    let gift = base(&mut f, "B001", Zone::Board(A, Color::Red));
    f.figure(Figure::Nightingale, Zone::Board(B, Color::Yellow));
    let protected = base(&mut f, "B008", Zone::Score(B));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0, demand());
    value(&mut game, &mut Vec::new(), A, 1);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().cards(Zone::Score(B)), [protected]);
    assert_eq!(game.position().top(A, Color::Red), Some(gift));
}

fn near_field<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B108", Zone::Junk);
    let selected = base(&mut f, "B042", Zone::Score(B));
    let tied = base(&mut f, "B045", Zone::Score(B));
    let lower = base(&mut f, "B001", Zone::Score(A));
    let drawn = base(&mut f, "B047", Zone::Deck(Deck::base(5)));
    let mut game = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut game, source, 0, demand());
    let mut events = Vec::new();
    value(&mut game, &mut events, A, 4);
    finish(&mut game, &mut events);
    invoke(&mut game, source, 1, Context::new(A));
    let menu = decision(&mut game, &mut events);
    for card in [selected, tied] {
        assert!(menu.options.iter().any(|o| matches!(o, Alternative::Reveal { target: TargetDescriptor::Known { card: id, .. } } if *id == card)));
    }
    assert!(!menu.options.iter().any(|o| matches!(o, Alternative::Reveal { target: TargetDescriptor::Known { card, .. } } if *card == lower)));
    game.choose(menu.options.iter().position(|o| matches!(o, Alternative::Reveal { target: TargetDescriptor::Known { card, .. } } if *card == selected)).unwrap()).unwrap();
    finish(&mut game, &mut events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Revealed { card, .. } if *card == selected))
    );
    assert_eq!(game.position().top(A, Color::Red), Some(drawn));
    assert!(game.position().cards(Zone::Score(A)).contains(&selected));
    assert!(game.position().cards(Zone::Score(B)).is_empty());
}
#[test]
fn near_field_transfers_then_reveals_and_self_executes_any_highest_tie() {
    near_field::<false>();
    near_field::<true>();
}

#[test]
fn ordinary_value_and_achievement_choices_do_not_inherit_archimedes_boxes() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B063", Zone::Junk);
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let scored = base(&mut f, "B001", Zone::Score(A));
    let achievement = base(&mut f, "B048", Zone::AvailableAchievements);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    value(&mut game, &mut Vec::new(), A, 1);
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().top(A, Color::Red), Some(scored));
    invoke(&mut game, source, 1, Context::new(A));
    finish(&mut game, &mut Vec::new());
    assert!(game.position().cards(Zone::Junk).contains(&achievement));
}

#[test]
fn mass_media_purple_splay_survives_an_absent_payment() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B081", Zone::Junk);
    base(&mut f, "B013", Zone::Board(A, Color::Purple));
    base(&mut f, "B015", Zone::Board(A, Color::Purple));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    invoke(&mut game, source, 0, Context::new(A));
    finish(&mut game, &mut Vec::new());
    invoke(&mut game, source, 1, Context::new(A));
    finish(&mut game, &mut Vec::new());
    assert_eq!(game.position().splay(A, Color::Purple), Splay::Up);
}

#[test]
fn near_field_reveals_a_highest_logical_score_member_without_moving_it() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B108", Zone::Junk);
    f.figure(Figure::Michelangelo, Zone::Board(A, Color::Yellow));
    let selected = base(&mut f, "B042", Zone::Hand(A));
    let lower = base(&mut f, "B001", Zone::Score(A));
    let drawn = base(&mut f, "B047", Zone::Deck(Deck::base(5)));
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut game, source, 1, Context::new(A));
    let mut events = Vec::new();
    finish(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), [selected]);
    assert_eq!(game.position().cards(Zone::Score(A)), [lower]);
    assert_eq!(game.position().top(A, Color::Red), Some(drawn));
    assert!(events.iter().any(|event| matches!(event,
        Event::Revealed { card, source: Zone::Hand(player), .. } if *card == selected && *player == A)));
}
