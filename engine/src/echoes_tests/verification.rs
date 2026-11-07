//! Face-dependent mandatory instructions supply success and completeness proof.
use super::cards::{card, deck_card, in_hand};
use super::*;

fn ready(
    mut layout: Layout,
    code: &str,
    effect: u16,
    specials: &[(Special, SpecialLocation)],
) -> Game {
    let source = layout.add(card(code), Zone::Junk);
    let expansions = Expansions::ECHOES
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES);
    let mut position = layout.position(expansions);
    for special in position.specials().collect::<Vec<_>>() {
        position.specials[special.index()] = SpecialLocation::Junk;
    }
    for &(special, location) in specials {
        position.specials[special.index()] = location;
    }
    let mut game = Game::seeded(position, expansions, 17)
        .unwrap()
        .with_tracking()
        .unwrap();
    let mut context = Context::new(A);
    context.demander = B;
    game.start(
        Operation::ExecuteEffect {
            card: source,
            effect,
            boxed: BoxedValues::ZERO,
        },
        context,
    )
    .unwrap();
    game
}

fn finish(game: &mut Game, events: &mut Vec<Event>, mut pick: impl FnMut(&Decision) -> usize) {
    for _ in 0..200 {
        let cloned = game.clone();
        let mut clone = cloned;
        let mut other_events = Vec::new();
        let status = game.advance(1, events).unwrap();
        let other = clone.advance(1, &mut other_events).unwrap();
        assert_eq!(status, other);
        assert_eq!(*game, clone);
        match status {
            Status::WorkLimit => continue,
            Status::Decision(menu) => game.choose(pick(&menu)).unwrap(),
            Status::Complete(_) => {
                assert!(game.position().check_conservation());
                return;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("verification effect did not complete")
}

fn revealed(events: &[Event], card: CardId) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::Revealed { card: found, .. } if *found == card))
}

fn tech(layout: &mut Layout, color: Color, age: u8, zone: Zone, icon: Option<Icon>) -> CardId {
    let mut c = printed(color, [Symbol::Blank; 4]);
    c.value = age;
    if let Some(icon) = icon {
        c.symbols[0] = Symbol::Icon(icon);
        c.featured = Some(icon);
    }
    layout.add(c, zone)
}

#[test]
fn bandage_verifies_the_returned_score_card_and_junks_one_mixed_quota_only_after_both_returns() {
    for (score_succeeds, board_succeeds) in [(true, true), (false, true), (true, false)] {
        let mut f = Layout::default();
        let selected = f.add(
            base_card(if score_succeeds { "B076" } else { "B001" }).unwrap(),
            Zone::Score(A),
        );
        let same_back = score_succeeds.then(|| f.add(base_card("B077").unwrap(), Zone::Score(A)));
        let board = tech(
            &mut f,
            Color::Blue,
            7,
            Zone::Board(A, Color::Blue),
            board_succeeds.then_some(Icon::Efficiency),
        );
        tech(&mut f, Color::Green, 1, Zone::Achievements(A), None);
        let supply = tech(&mut f, Color::Purple, 2, Zone::AvailableAchievements, None);
        let mut game = ready(
            f,
            "E076",
            0,
            &[
                (Special::Monument, SpecialLocation::Owned(A)),
                (Special::Empire, SpecialLocation::Available),
            ],
        );
        let mut events = vec![];
        finish(&mut game, &mut events, |menu| {
            menu.options.iter().position(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == selected)).unwrap_or(0)
        });
        assert!(
            revealed(&events, selected),
            "success witness or failure proof"
        );
        assert_eq!(
            game.position().locate(board),
            Some(if board_succeeds {
                Zone::Deck(Deck::base(7))
            } else {
                Zone::Board(A, Color::Blue)
            })
        );
        if score_succeeds {
            assert_eq!(
                game.position().locate(selected),
                Some(Zone::Deck(Deck::base(8)))
            );
            assert_eq!(
                game.position().locate(same_back.unwrap()),
                Some(Zone::Score(A))
            );
            assert_eq!(
                game.tracker(B).unwrap().belief(Zone::Deck(Deck::base(8)))[0].known(),
                Some(selected),
                "the named reveal must stay tied to this departure among equal backs"
            );
            assert_eq!(
                game.position().locate(supply),
                Some(if board_succeeds {
                    Zone::Junk
                } else {
                    Zone::AvailableAchievements
                })
            );
            assert_eq!(
                game.position().special_location(Special::Empire),
                if board_succeeds {
                    SpecialLocation::Junk
                } else {
                    SpecialLocation::Available
                }
            );
        } else {
            assert_eq!(game.position().locate(selected), Some(Zone::Score(A)));
            assert_eq!(
                game.position().locate(supply),
                Some(Zone::AvailableAchievements)
            );
            assert_eq!(
                game.position().special_location(Special::Empire),
                SpecialLocation::Available
            );
        }
    }
}

#[test]
fn bandage_echo_melds_a_health_witness_or_reveals_the_impossible_hand() {
    for health in [false, true] {
        let mut f = Layout::default();
        let selected = tech(
            &mut f,
            Color::Yellow,
            1,
            Zone::Hand(A),
            health.then_some(Icon::Health),
        );
        let other = in_hand(&mut f, A, Color::Red, 1);
        let mut game = ready(f, "E076", Card::ECHO, &[]);
        let mut events = vec![];
        finish(&mut game, &mut events, |_| {
            panic!("at most one matching card")
        });
        if health {
            assert_eq!(
                game.position().locate(selected),
                Some(Zone::Board(A, Color::Yellow))
            );
            assert!(
                !revealed(&events, other),
                "a successful meld needs no whole-hand disclosure"
            );
        } else {
            assert_eq!(game.position().locate(selected), Some(Zone::Hand(A)));
            assert!(revealed(&events, selected) && revealed(&events, other));
        }
    }
}

#[test]
fn machine_gun_transfers_one_bonus_top_per_color_junks_four_mixed_achievements_and_draws() {
    let mut f = Layout::default();
    let bottom = tech(&mut f, Color::Blue, 1, Zone::Board(A, Color::Blue), None);
    f.cards[usize::from(bottom.0)].symbols[0] = Symbol::Bonus(1);
    let top = tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue), None);
    f.cards[usize::from(top.0)].symbols[0] = Symbol::Bonus(2);
    let no_bonus = tech(&mut f, Color::Red, 3, Zone::Board(A, Color::Red), None);
    let supply: Vec<_> = (0..3)
        .map(|_| tech(&mut f, Color::Green, 1, Zone::AvailableAchievements, None))
        .collect();
    let drawn = deck_card(&mut f, CardSet::Base, Color::Green, 7);
    let mut game = ready(
        f,
        "E066",
        0,
        &[
            (Special::Empire, SpecialLocation::Available),
            (Special::World, SpecialLocation::Available),
        ],
    );
    let mut events = vec![];
    let mut short_quota_choice = false;
    finish(&mut game, &mut events, |menu| {
        short_quota_choice |= menu
            .options
            .iter()
            .any(|o| matches!(o, Alternative::JunkSpecial(_)))
            && menu
                .options
                .iter()
                .any(|o| matches!(o, Alternative::BlindMove { .. }));
        0
    });
    assert!(
        short_quota_choice,
        "five available achievements with quota four must retain subset choices"
    );
    assert_eq!(game.position().cards(Zone::Score(B)), [top]);
    assert_eq!(
        game.position().locate(bottom),
        Some(Zone::Board(A, Color::Blue))
    );
    assert_eq!(
        game.position().locate(no_bonus),
        Some(Zone::Board(A, Color::Red))
    );
    let junked_cards = supply
        .iter()
        .filter(|&&c| game.position().locate(c) == Some(Zone::Junk))
        .count();
    let junked_tokens = [Special::Empire, Special::World]
        .into_iter()
        .filter(|&s| game.position().special_location(s) == SpecialLocation::Junk)
        .count();
    assert_eq!(junked_cards + junked_tokens, 4);
    assert_eq!(game.position().locate(drawn), Some(Zone::Hand(A)));
    let mut f = Layout::default();
    tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), None);
    let untouched = deck_card(&mut f, CardSet::Base, Color::Green, 7);
    let mut game = ready(
        f,
        "E066",
        0,
        &[(Special::Empire, SpecialLocation::Available)],
    );
    finish(&mut game, &mut vec![], |_| 0);
    assert_eq!(
        game.position().locate(untouched),
        Some(Zone::Deck(Deck::base(7)))
    );
    assert_eq!(
        game.position().special_location(Special::Empire),
        SpecialLocation::Available
    );
}

#[test]
fn machine_gun_echo_requires_five_tops_and_ordinary_dogma_returns_only_initial_nonred_tops() {
    for count in [4, 5] {
        let mut f = Layout::default();
        for &color in &Color::ALL[..count] {
            tech(&mut f, color, 1, Zone::Board(A, color), None);
        }
        let drawn = deck_card(&mut f, CardSet::Base, Color::Purple, 7);
        let mut game = ready(f, "E066", Card::ECHO, &[]);
        finish(&mut game, &mut vec![], |_| 0);
        assert_eq!(
            game.position().locate(drawn),
            Some(if count == 5 {
                Zone::Score(A)
            } else {
                Zone::Deck(Deck::base(7))
            })
        );
    }
    let mut f = Layout::default();
    let red = tech(&mut f, Color::Red, 1, Zone::Board(A, Color::Red), None);
    let bottom = tech(&mut f, Color::Blue, 2, Zone::Board(A, Color::Blue), None);
    let top = tech(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue), None);
    let mut game = ready(f, "E066", 1, &[]);
    finish(&mut game, &mut vec![], |_| 0);
    assert_eq!(game.position().locate(top), Some(Zone::Deck(Deck::base(3))));
    assert_eq!(
        game.position().locate(bottom),
        Some(Zone::Board(A, Color::Blue))
    );
    assert_eq!(
        game.position().locate(red),
        Some(Zone::Board(A, Color::Red))
    );
}

#[test]
fn parachute_reveals_the_hand_then_transfers_every_non_efficiency_card() {
    let mut f = Layout::default();
    let moving: Vec<_> = [Color::Red, Color::Blue]
        .into_iter()
        .map(|c| tech(&mut f, c, 1, Zone::Hand(A), None))
        .collect();
    let retained = tech(
        &mut f,
        Color::Green,
        1,
        Zone::Hand(A),
        Some(Icon::Efficiency),
    );
    let mut game = ready(f, "E077", 0, &[]);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    assert_eq!(game.position().cards(Zone::Hand(A)), [retained]);
    let mut transferred = game.position().cards(Zone::Hand(B)).to_vec();
    transferred.sort();
    assert_eq!(transferred, moving);
    // The whole hand shows before anything moves, and only once.
    let first_move = events
        .iter()
        .position(|event| matches!(event, Event::Moved(_)))
        .unwrap();
    let shown: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(shown, [moving[0], moving[1], retained]);
    assert!(
        events[..first_move]
            .iter()
            .filter(|event| matches!(event, Event::Revealed { .. }))
            .count()
            == 3
    );
    assert!(moving.iter().all(|&c| {
        game.tracker(B)
            .unwrap()
            .belief(Zone::Hand(B))
            .iter()
            .any(|s| s.known() == Some(c))
    }));
}

#[test]
fn digital_pet_draws_then_reveals_and_returns_the_whole_matching_board_and_score_set() {
    let mut f = Layout::default();
    let red: Vec<_> = [
        Zone::Score(A),
        Zone::Board(A, Color::Red),
        Zone::Board(A, Color::Red),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, z)| tech(&mut f, Color::Red, 2 + i as u8, z, None))
    .collect();
    let other = tech(&mut f, Color::Blue, 2, Zone::Score(A), None);
    let board = tech(&mut f, Color::Green, 5, Zone::Board(A, Color::Green), None);
    let drawn = deck_card(&mut f, CardSet::Base, Color::Red, 11);
    let mut game = ready(f, "E108", 0, &[]);
    let mut events = vec![];
    finish(&mut game, &mut events, |_| 0);
    assert!(matches!(events[0], Event::Moved(ref m) if m.card == drawn && m.verb.is_none()));
    assert!(matches!(events[1], Event::Revealed { card, .. } if card == drawn));
    for (i, &card) in red.iter().enumerate() {
        assert_eq!(
            game.position().locate(card),
            Some(Zone::Deck(Deck::base(2 + i as u8)))
        );
    }
    assert_eq!(game.position().locate(other), Some(Zone::Score(A)));
    assert_eq!(
        game.position().locate(board),
        Some(Zone::Board(A, Color::Green))
    );
    assert!(revealed(&events, other) && revealed(&events, red[0]));
    assert_eq!(
        game.tracker(B).unwrap().belief(Zone::Deck(Deck::base(2)))[0].known(),
        Some(red[0])
    );
}

#[test]
fn algocracy_uses_featured_icons_reveals_nonmatches_and_preserves_bonus_noop() {
    for leader in [A, B] {
        let mut f = Layout::default();
        tech(
            &mut f,
            Color::Blue,
            1,
            Zone::Board(leader, Color::Blue),
            Some(Icon::Health),
        );
        let matching: Vec<_> = [
            Zone::Hand(leader.opponent()),
            Zone::Score(A),
            Zone::Score(B),
        ]
        .into_iter()
        .map(|zone| tech(&mut f, Color::Green, 2, zone, Some(Icon::Health)))
        .collect();
        let nonmatching = tech(
            &mut f,
            Color::Red,
            2,
            Zone::Score(A),
            Some(Icon::Efficiency),
        );
        // Printed Health is insufficient when the featured type differs.
        f.cards[usize::from(nonmatching.0)].symbols[1] = Symbol::Icon(Icon::Health);
        let city = f.add(city_card("C035").unwrap(), Zone::Score(A));
        let figure = f.add(
            figure_card(Figure::NapoleonBonaparte.code()).unwrap(),
            Zone::Score(B),
        );
        let leaders_hand = tech(
            &mut f,
            Color::Purple,
            2,
            Zone::Hand(leader),
            Some(Icon::Health),
        );
        let mut game = ready(f, "E110", 0, &[]);
        let mut noop = game.clone();
        let mut no_events = vec![];
        finish(&mut noop, &mut no_events, |menu| {
            assert_eq!(
                menu.options,
                [Alternative::Icon(Icon::Health), Alternative::BonusIcon],
                "public tied-leader and non-featured City types alias the same no-op"
            );
            menu.options
                .iter()
                .position(|o| *o == Alternative::BonusIcon)
                .unwrap()
        });
        assert!(no_events.is_empty());
        assert_eq!(noop.position().cards(Zone::Hand(leader)), [leaders_hand]);
        let mut events = vec![];
        finish(&mut game, &mut events, |menu| {
            menu.options
                .iter()
                .position(|o| *o == Alternative::Icon(Icon::Health))
                .unwrap_or(0)
        });
        assert_eq!(game.position().locate(nonmatching), Some(Zone::Score(A)));
        assert!(revealed(&events, nonmatching));
        assert!(!revealed(&events, leaders_hand));
        assert!(revealed(&events, city) && revealed(&events, figure));
        assert_eq!(game.position().locate(city), Some(Zone::Score(A)));
        assert_eq!(game.position().locate(figure), Some(Zone::Score(B)));
        for card in matching {
            assert_eq!(game.position().locate(card), Some(Zone::Hand(leader)));
            assert!(revealed(&events, card));
        }
    }
    assert!(Test::FeaturedChosenIcon.reads_front());
}
