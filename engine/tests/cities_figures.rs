//! Cities with Figures: expansion-wide deck choices and the Dogma icon
//! retained through endorsement preparation and sharing rewards.

use inno_engine::*;

mod common;
use common::*;

#[test]
fn lily_hevesh_can_reveal_a_city_deck() {
    let mut f = Fixture::default();
    let lily = figure_card(Figure::LilyHevesh.code()).unwrap();
    let color = lily.color;
    f.add(lily, Zone::Board(A, color));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    f.add(city_card("C004").unwrap(), Zone::Deck(Deck::cities(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::CITIES)).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    for _ in 0..10 {
        let menu = decision(&mut game, &mut events);
        if menu
            .options
            .iter()
            .any(|o| matches!(o, Alternative::Deck(_)))
        {
            assert!(
                menu.options.contains(&Alternative::Deck(Deck::cities(1))),
                "City deck missing from Lily's menu: {:?}",
                menu.options
            );
            return;
        }
        game.choose(0).unwrap();
    }
    panic!("no deck selection");
}

#[test]
fn lily_draws_a_revealed_city_then_splays_without_another_city_draw() {
    let mut f = Fixture::default();
    f.tech(Color::Purple, 1, Zone::Board(A, Color::Purple), vec![]);
    f.figure(Figure::LilyHevesh, Zone::Board(A, Color::Purple));
    f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let city = f.add(city_card("C004").unwrap(), Zone::Deck(Deck::cities(1)));
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::CITIES))
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    let menu = loop {
        let menu = decision(&mut game, &mut events);
        if events
            .iter()
            .filter(|event| matches!(event, Event::Revealed { .. }))
            .count()
            == 3
        {
            break menu;
        }
        game.choose(0).unwrap();
    };
    let choice = menu
        .options
        .iter()
        .position(|o| *o == Alternative::Deck(Deck::cities(1)))
        .unwrap();
    let mut clone = game.clone();
    let mut sample = game.determinize(A, 7).unwrap();
    assert_eq!(decision(&mut sample, &mut Vec::new()), menu);
    assert_eq!(sample.observe(A).unwrap(), game.observe(A).unwrap());
    for game in [&mut game, &mut clone, &mut sample] {
        game.choose(choice).unwrap();
        let mut suffix = Vec::new();
        assert!(matches!(settle(game, &mut suffix), Status::Complete(_)));
        assert_eq!(game.position().cards(Zone::Hand(A)), [city]);
        assert!(
            game.position()
                .cards(Zone::Deck(Deck::cities(1)))
                .is_empty()
        );
        assert_eq!(game.position().splay(A, Color::Purple), Splay::Aslant);
        assert_eq!(
            suffix
                .iter()
                .filter(|event| matches!(event, Event::Moved(_)))
                .count(),
            1
        );
        assert!(game.position().check_conservation());
    }
    assert_eq!(game, clone);
}

#[test]
fn lily_can_win_with_a_city_as_the_fifth_revealed_color() {
    let mut f = Fixture::default();
    f.figure(Figure::LilyHevesh, Zone::Board(A, Color::Purple));
    for (i, color) in [Color::Red, Color::Yellow, Color::Green, Color::Blue]
        .into_iter()
        .enumerate()
    {
        let age = i as u8 + 1;
        f.tech(color, age, Zone::Deck(Deck::base(age)), vec![]);
    }
    let city = f.add(city_card("C024").unwrap(), Zone::Deck(Deck::cities(2)));
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::CITIES))
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    loop {
        match settle(&mut game, &mut events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => {
                assert_eq!(status, Status::Winner(A));
                break;
            }
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Revealed { .. }))
            .count(),
        5
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Revealed { card, .. } if *card == city))
    );
    assert!(game.position().cards(Zone::Hand(A)).is_empty());
    assert!(game.position().check_conservation());
}

#[test]
fn john_harrison_offers_every_enabled_set() {
    for expansions in [
        Expansions::FIGURES.union(Expansions::CITIES),
        Expansions::FIGURES
            .union(Expansions::ECHOES)
            .union(Expansions::UNSEEN)
            .union(Expansions::ARTIFACTS),
    ] {
        let mut f = Fixture::default();
        let harrison = figure_card(Figure::JohnHarrison.code()).unwrap();
        let color = harrison.color;
        f.add(harrison, Zone::Board(A, color));
        f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
        f.figure(Figure::Shennong, Zone::Deck(Deck::figures(1)));
        if expansions.cities() {
            f.add(city_card("C004").unwrap(), Zone::Deck(Deck::cities(1)));
        }
        let mut game = Game::new(f.position(), expansions).unwrap();
        game.take_action(Action::Draw, ActionKind::Regular).unwrap();
        let mut events = Vec::new();
        let menu = loop {
            let menu = decision(&mut game, &mut events);
            if menu
                .options
                .iter()
                .any(|o| matches!(o, Alternative::Draw { .. }))
            {
                break menu;
            }
            game.choose(0).unwrap();
        };
        let expected: Vec<_> = CardSet::ALL
            .into_iter()
            .filter(|&set| expansions.includes(set))
            .map(|set| Alternative::Draw { value: 1, set })
            .collect();
        assert_eq!(menu.options, expected);
    }
}

#[test]
fn invalid_dogma_requests_leave_every_supported_mode_unchanged() {
    for expansions in [
        Expansions::BASE,
        Expansions::FIGURES,
        Expansions::CITIES,
        Expansions::FIGURES.union(Expansions::CITIES),
    ] {
        let mut f = Fixture::default();
        let hand_card = f.add(base_card("B009").unwrap(), Zone::Hand(A));
        let mut game = Game::new(f.position(), expansions)
            .unwrap()
            .with_tracking()
            .unwrap();
        let before = game.clone();
        for card in [hand_card, CardId(1), CardId(u16::MAX)] {
            for auspice in [false, true] {
                assert_eq!(
                    game.take_action(Action::Dogma { card, auspice }, ActionKind::Regular),
                    Err(Error::InvalidAction)
                );
                assert_eq!(game, before);
            }
        }
    }
}

#[test]
fn harrison_draws_the_chosen_set_and_preserves_the_regular_draw() {
    for cities in [false, true] {
        for set in [CardSet::Base, CardSet::Figures, CardSet::Cities] {
            if set == CardSet::Cities && !cities {
                continue;
            }
            harrison_draw(cities, set, false, false);
        }
    }
    // The Cities option remains available when its deck is empty, including
    // an introduced Age 0: explicit-set draws fall back to Base.
    harrison_draw(true, CardSet::Cities, true, false);
    harrison_draw(true, CardSet::Cities, true, true);
}

#[test]
fn harrison_without_a_card_to_return_only_takes_the_regular_draw() {
    let mut f = Fixture::default();
    f.figure(Figure::JohnHarrison, Zone::Board(A, Color::Green));
    let regular = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::CITIES)).unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert!(matches!(
        settle(&mut game, &mut Vec::new()),
        Status::Complete(_)
    ));
    assert_eq!(game.position().cards(Zone::Hand(A)), [regular]);
    assert!(game.position().check_conservation());
}

fn harrison_draw(cities: bool, set: CardSet, empty_city_deck: bool, age_zero: bool) {
    let mut f = Fixture::default();
    f.figure(Figure::JohnHarrison, Zone::Board(A, Color::Green));
    let age = u8::from(!age_zero);
    let returned = f.tech(Color::Blue, age, Zone::Hand(A), vec![]);
    let base = f.tech(Color::Blue, age, Zone::Deck(Deck::base(age)), vec![]);
    let figure = f.figure(Figure::Shennong, Zone::Deck(Deck::figures(1)));
    let city = if cities && !empty_city_deck {
        Some(f.add(city_card("C004").unwrap(), Zone::Deck(Deck::cities(1))))
    } else {
        None
    };
    let regular = f.tech(Color::Blue, 6, Zone::Deck(Deck::base(6)), vec![]);
    let expansions = if cities {
        Expansions::FIGURES.union(Expansions::CITIES)
    } else {
        Expansions::FIGURES
    };
    let mut game = Game::new(f.position(), expansions)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    let mut events = Vec::new();
    let menu = loop {
        let menu = decision(&mut game, &mut events);
        if menu
            .options
            .iter()
            .any(|o| matches!(o, Alternative::Draw { .. }))
        {
            break menu;
        }
        game.choose(0).unwrap();
    };
    let sets: &[CardSet] = if cities {
        &[CardSet::Base, CardSet::Cities, CardSet::Figures]
    } else {
        &[CardSet::Base, CardSet::Figures]
    };
    let expected_options: Vec<_> = sets
        .iter()
        .map(|&set| Alternative::Draw { value: age, set })
        .collect();
    assert_eq!(menu.options, expected_options);
    let choice = sets.iter().position(|&s| s == set).unwrap();
    let drawn = match set {
        CardSet::Base => base,
        CardSet::Figures => figure,
        CardSet::Cities => city.unwrap_or(base),
        _ => unreachable!(),
    };
    let mut clone = game.clone();
    let mut sample = game.determinize(A, 9).unwrap();
    assert_eq!(decision(&mut sample, &mut Vec::new()), menu);
    assert_eq!(sample.observe(A).unwrap(), game.observe(A).unwrap());
    for game in [&mut game, &mut clone, &mut sample] {
        game.choose(choice).unwrap();
        let mut suffix = Vec::new();
        assert!(matches!(settle(game, &mut suffix), Status::Complete(_)));
        assert_eq!(game.position().cards(Zone::Hand(A)), [drawn, regular]);
        assert!(
            game.position()
                .cards(Zone::Deck(Deck::base(age)))
                .contains(&returned)
        );
        assert_eq!(
            suffix
                .iter()
                .filter(|event| matches!(event, Event::Moved(_)))
                .count(),
            2
        );
        assert!(game.position().check_conservation());
    }
    assert_eq!(game, clone);
}

#[test]
fn cities_auspice_is_retained_for_sun_tzus_sharing_reward() {
    sun_tzu_uses_auspice(true, true);
}

#[test]
fn cities_auspice_is_retained_after_declining_endorsement() {
    sun_tzu_uses_auspice(false, true);
}

#[test]
fn retaining_the_printed_icon_still_limits_sun_tzu_to_that_icon() {
    sun_tzu_uses_auspice(false, false);
    sun_tzu_uses_auspice(true, false);
}

fn sun_tzu_uses_auspice(endorse: bool, auspice: bool) {
    let mut f = Fixture::default();
    f.figure(Figure::SunTzu, Zone::Board(A, Color::Red));
    f.figure(Figure::Homer, Zone::Board(B, Color::Purple));
    // The printed-icon control must also share (the Avatar case ties).
    let authority_icons = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    f.cards[usize::from(authority_icons.0)].symbols =
        four_slots([Symbol::Icon(Icon::Authority); 4]);
    let authority_icons = f.tech(Color::Green, 1, Zone::Board(B, Color::Green), vec![]);
    f.cards[usize::from(authority_icons.0)].symbols =
        four_slots([Symbol::Icon(Icon::Authority); 4]);
    // Damascus qualifies an Authority Dogma for endorsement; Sun Tzu
    // qualifies it for Auspice. Both players have one Avatar, so B shares.
    f.add(city_card("C004").unwrap(), Zone::Board(A, Color::Yellow));
    let source = f.add(base_card("B009").unwrap(), Zone::Board(A, Color::Green));
    let payment = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    let avatar = f.figure(Figure::Shennong, Zone::Hand(A));
    let authority = f.add(base_card("B006").unwrap(), Zone::Hand(A));
    for _ in 0..8 {
        f.tech(Color::Blue, 1, Zone::Deck(Deck::base(1)), vec![]);
    }
    f.tech(Color::Blue, 2, Zone::Deck(Deck::base(2)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES.union(Expansions::CITIES))
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(
        Action::Dogma {
            card: source,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    let mut offered = false;
    for _ in 0..30 {
        let Status::Decision(menu) = settle(&mut game, &mut events) else {
            break;
        };
        let index = if menu.options == [Alternative::Branch(0), Alternative::Branch(1)] {
            usize::from(!endorse)
        } else if menu.options.contains(&Alternative::Icon(Icon::Avatar)) {
            menu.options
                .iter()
                .position(|o| {
                    *o == Alternative::Icon(if auspice {
                        Icon::Avatar
                    } else {
                        Icon::Authority
                    })
                })
                .unwrap()
        } else if menu.options.iter().any(|o| {
            matches!(
                o,
                Alternative::Move {
                    verb: Verb::Junk,
                    ..
                }
            )
        }) {
            menu.options.iter().position(|o| matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == payment)).unwrap()
        } else if menu.options.iter().any(|o| {
            matches!(
                o,
                Alternative::Move {
                    verb: Verb::Meld,
                    ..
                }
            )
        }) {
            offered = true;
            let can_meld = |wanted| {
                menu.options.iter().any(|o| matches!(o, Alternative::Move { verb: Verb::Meld, target: TargetDescriptor::Known { card, .. } } if *card == wanted))
            };
            assert_eq!(
                can_meld(avatar),
                auspice,
                "endorse={endorse} auspice={auspice}: {:?}",
                menu.options
            );
            assert_eq!(can_meld(authority), !auspice);
            let mut clone = game.clone();
            let mut sample = game.determinize(A, 5).unwrap();
            assert_eq!(decision(&mut sample, &mut Vec::new()), menu);
            assert_eq!(sample.observe(A).unwrap(), game.observe(A).unwrap());
            let decline = menu
                .options
                .iter()
                .position(|o| *o == Alternative::Decline)
                .unwrap();
            for game in [&mut game, &mut clone, &mut sample] {
                game.choose(decline).unwrap();
                assert!(matches!(settle(game, &mut Vec::new()), Status::Complete(_)));
                assert!(game.position().check_conservation());
            }
            assert_eq!(game, clone);
            break;
        } else {
            0
        };
        game.choose(index).unwrap();
    }
    assert!(offered, "Sun Tzu must offer an Avatar meld");
}
