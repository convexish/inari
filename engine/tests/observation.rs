use inno_engine::observation::{
    self, FAMILY, GENERATION, GROUPS, Group, HEADER, OPTION_FIELDS, SCALARS, TOKEN_FIELDS, field,
    kind, scalar,
};
use inno_engine::*;

mod common;
use common::*;

fn options(status: &Status, game: &Game) -> usize {
    match status {
        Status::Decision(menu) => menu.options.len(),
        Status::Complete(_) => game.legal_actions().len(),
        _ => 0,
    }
}

const MODES: [Expansions; 32] = [
    Expansions::BASE,
    Expansions::FIGURES,
    Expansions::CITIES,
    Expansions::FIGURES.union(Expansions::CITIES),
    Expansions::ECHOES,
    Expansions::ECHOES.union(Expansions::FIGURES),
    Expansions::ECHOES.union(Expansions::CITIES),
    Expansions::ECHOES
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
    Expansions::ARTIFACTS,
    Expansions::ARTIFACTS.union(Expansions::FIGURES),
    Expansions::ARTIFACTS.union(Expansions::CITIES),
    Expansions::ARTIFACTS
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
    Expansions::ARTIFACTS.union(Expansions::ECHOES),
    Expansions::ARTIFACTS
        .union(Expansions::ECHOES)
        .union(Expansions::FIGURES),
    Expansions::ARTIFACTS
        .union(Expansions::ECHOES)
        .union(Expansions::CITIES),
    Expansions::ARTIFACTS
        .union(Expansions::ECHOES)
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
    Expansions::UNSEEN,
    Expansions::UNSEEN.union(Expansions::FIGURES),
    Expansions::UNSEEN.union(Expansions::CITIES),
    Expansions::UNSEEN
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
    Expansions::UNSEEN.union(Expansions::ECHOES),
    Expansions::UNSEEN
        .union(Expansions::ECHOES)
        .union(Expansions::FIGURES),
    Expansions::UNSEEN
        .union(Expansions::ECHOES)
        .union(Expansions::CITIES),
    Expansions::UNSEEN
        .union(Expansions::ECHOES)
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
    Expansions::UNSEEN.union(Expansions::ARTIFACTS),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::FIGURES),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::CITIES),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::ECHOES),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::ECHOES)
        .union(Expansions::FIGURES),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::ECHOES)
        .union(Expansions::CITIES),
    Expansions::UNSEEN
        .union(Expansions::ARTIFACTS)
        .union(Expansions::ECHOES)
        .union(Expansions::FIGURES)
        .union(Expansions::CITIES),
];

/// Seeded random games; `check` runs at every settled root with its actor.
fn roots(expansions: Expansions, games: u64, mut check: impl FnMut(&Game, &Status, Player, u64)) {
    for seed in 0..games {
        let game = Game::new_game(seed, expansions);
        let mut game = game.unwrap().with_tracking().unwrap();
        let mut rng = Rng::new(seed ^ 0x0B5E);
        for step in 0..400 {
            let status = game.advance(1 << 16, &mut Vec::new()).unwrap();
            let actor = match &status {
                Status::Decision(menu) => menu.player,
                Status::Complete(_) => game.position().turn_owner(),
                _ => break,
            };
            check(&game, &status, actor, seed * 1000 + step);
            let index = (rng.next_u64() % options(&status, &game) as u64) as usize;
            match status {
                Status::Decision(_) => game.choose(index).unwrap(),
                _ => {
                    let action = game.legal_actions()[index];
                    game.take_action(action, ActionKind::Regular).unwrap();
                }
            }
        }
    }
}

#[test]
fn observations_depend_only_on_the_viewers_information() {
    // A determinized world has the same public state and the sampling
    // viewer's beliefs: any real hidden fact in the encoding would differ.
    // Each mode runs on its own thread.
    std::thread::scope(|scope| {
        for expansions in MODES {
            scope.spawn(move || privacy_samples(expansions));
        }
    });
}

fn privacy_samples(expansions: Expansions) {
    let mut compared = 0;
    roots(expansions, 3, |game, status, actor, seed| {
        let real = game.observe(actor).unwrap();
        for sample in 0..1 {
            let world = match game.determinize(actor, seed ^ sample) {
                Ok(world) => world,
                Err(error) => panic!(
                    "{expansions:?} seed={seed} actor={actor:?} status={status:?}: {error:?}"
                ),
            };
            assert_eq!(
                world.observe(actor).unwrap(),
                real,
                "{expansions:?} seed={seed} sample={sample}"
            );
            compared += 1;
        }
        // Between actions either player may sample and encode.
        if matches!(status, Status::Complete(_)) {
            let other = actor.opponent();
            let world = game.determinize(other, seed).unwrap();
            assert_eq!(world.observe(other).unwrap(), game.observe(other).unwrap());
        }
    });
    assert!(compared > 250, "{expansions:?}: {compared} sampled");
    eprintln!("{expansions:?}: {compared} sampled, no refusals");
}

/// A shared Priest King reveals the sharer's scored card, so the owner can
/// still sample worlds (and search) while choosing their own card.
#[test]
fn a_shared_priest_king_leaves_the_owner_able_to_sample() {
    let mut f = Fixture::default();
    let priest_card = artifacts_card("A007").unwrap();
    let featured = priest_card.featured.unwrap();
    let priest = f.add(priest_card.clone(), Zone::Board(A, priest_card.color));
    let b_top = f.tech(Color::Green, 2, Zone::Board(B, Color::Green), vec![]);
    f.cards[usize::from(b_top.0)].symbols[..3].fill(Symbol::Icon(featured));
    let b_hand = f.tech(Color::Green, 3, Zone::Hand(B), vec![]);
    f.tech(Color::Red, 3, Zone::Hand(B), vec![]);
    f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Board(A, Color::Blue), vec![]);
    f.tech(Color::Yellow, 1, Zone::Deck(Deck::base(1)), vec![]);
    let mut game = Game::new(f.position(), Expansions::ARTIFACTS)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(
        Action::Dogma {
            card: priest,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    let mut sampled = 0;
    while let Status::Decision(decision) = settle(&mut game, &mut events) {
        let world = game.determinize(decision.player, 1).unwrap();
        assert_eq!(
            world.observe(decision.player).unwrap(),
            game.observe(decision.player).unwrap()
        );
        sampled += 1;
        game.choose(0).unwrap();
    }
    assert!(sampled >= 2, "both players decide");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Revealed { card, .. } if *card == b_hand))
    );
    assert!(game.position().check_conservation());
}

#[test]
fn rows_hold_the_header_scalars_tokens_and_one_option_per_legal_step() {
    for expansions in MODES {
        roots(expansions, 2, |game, status, actor, _| {
            let observation = game.observe(actor).unwrap();
            assert_eq!(observation.options.len(), options(status, game));
            assert_eq!(observation.scalars.len(), SCALARS);
            let row = observation.row();
            assert_eq!(
                row.len(),
                HEADER
                    + SCALARS
                    + TOKEN_FIELDS * observation.tokens.len()
                    + OPTION_FIELDS * observation.options.len()
            );
            assert_eq!(row[0], (FAMILY + GENERATION) as f32);
            assert_eq!(row[3] as usize, observation.tokens.len());
            assert_eq!(row[4] as usize, observation.options.len());
            assert_eq!(row[5] as usize, actor.index());
            assert_eq!(row[7], (6 + expansions.count()) as f32);
            assert!(row.iter().all(|x| x.is_finite()));
            // The other player is not choosing: no options for them.
            assert!(game.observe(actor.opponent()).unwrap().options.is_empty());
            for option in &observation.options {
                let k = option[field::KIND] as u8;
                match status {
                    Status::Complete(_) => assert!((kind::DRAW..=kind::DECREE_ACTION).contains(&k)),
                    _ => assert!(k >= kind::VALUE),
                }
                // A named Figure is a card identity, not a token.
                if k == kind::NAME_FIGURE {
                    assert!(option[field::CARD] > 0.0, "{option:?}");
                    assert_eq!(option[field::TOKEN], 0.0);
                    assert!(option[field::NUMBER] >= 1.0);
                }
                // Auspice lives in the slot field; tokens split specials
                // (1–20) from Decrees (21–25); an exchange names two zones.
                if k == kind::SHOWCASE || k == kind::DOGMA {
                    assert!(option[field::SLOT] <= 1.0 && option[field::ICON] == 0.0);
                    assert!(option[field::CARD] > 0.0);
                }
                if k == kind::DECREE {
                    assert!((21.0..=25.0).contains(&option[field::TOKEN]), "{option:?}");
                }
                if matches!(
                    k,
                    kind::JUNK_SPECIAL | kind::RESTORE_SPECIAL | kind::CLAIM_SPECIAL
                ) {
                    assert!((1.0..=20.0).contains(&option[field::TOKEN]), "{option:?}");
                }
                if k == kind::EXCHANGE {
                    assert!(option[field::ZONE] > 0.0 && option[field::SECOND_ZONE] > 0.0);
                    assert_eq!(option[field::SECOND], 0.0);
                } else {
                    assert_eq!(option[field::SECOND_ZONE], 0.0);
                }
                if k == kind::MATCHING_PAIR {
                    assert!(option[field::SECOND] > 0.0 || option[field::CARD] == 0.0);
                }
            }
            for token in &observation.tokens {
                assert!((token.group as usize) < GROUPS);
                assert!(token.weight > 0.0);
            }
        });
    }
}

#[test]
fn reserved_expansion_layout_stays_empty_in_base_and_figures_games() {
    for figures in [false, true] {
        let game = if figures {
            Game::new_figures(9)
        } else {
            Game::new_base(9)
        };
        let mut game = game.unwrap().with_tracking().unwrap();
        settle(&mut game, &mut Vec::new());
        let observation = game.observe(Player::One).unwrap();
        let s = &observation.scalars;
        assert_eq!(s[scalar::SETS + CardSet::Base.index()], 1.0);
        assert_eq!(
            s[scalar::SETS + CardSet::Figures.index()],
            f32::from(u8::from(figures))
        );
        for set in [
            CardSet::Echoes,
            CardSet::Cities,
            CardSet::Unseen,
            CardSet::Artifacts,
        ] {
            assert_eq!(s[scalar::SETS + set.index()], 0.0);
            for age in 0..12 {
                assert_eq!(s[scalar::DECKS + Deck { set, age }.index()], 0.0);
            }
        }
        // Only Base's five special slots are in play.
        for slot in 5..Special::RESERVED {
            assert!(
                s[scalar::SPECIALS + 4 * slot..][..4]
                    .iter()
                    .all(|&x| x == 0.0)
            );
        }
        if !figures {
            assert!(s[scalar::DECREES..scalar::CITIES].iter().all(|&x| x == 0.0));
        }
        assert!(s[scalar::CITIES..scalar::PLAYERS].iter().all(|&x| x == 0.0));
        for player in 0..2 {
            let reserved = scalar::PLAYERS + player * scalar::PLAYER + scalar::RESERVED;
            assert!(s[reserved..reserved + 16].iter().all(|&x| x == 0.0));
        }
        assert!(!observation.tokens.iter().any(|t| matches!(
            t.group,
            Group::MyForecast
                | Group::TheirForecast
                | Group::MySafe
                | Group::TheirSafe
                | Group::MyDisplay
                | Group::TheirDisplay
                | Group::MyMuseums
                | Group::TheirMuseums
        )));
        // The opening deal: each opponent hand slot spreads over unknowns.
        let hand: f32 = observation
            .tokens
            .iter()
            .filter(|t| t.group == Group::TheirHand)
            .map(|t| t.weight)
            .sum();
        assert!((hand - 2.0).abs() < 1e-4, "{hand}");
    }
}

#[test]
fn a_face_down_achievement_option_names_only_its_back() {
    let mut f = Fixture::default();
    let achievement = f.agriculture(Zone::AvailableAchievements);
    f.tech(Color::Red, 1, Zone::Board(A, Color::Red), vec![]);
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    f.tech(Color::Blue, 1, Zone::Score(A), vec![]);
    let game = Game::new(f.position(), Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    assert!(
        game.legal_actions()
            .contains(&Action::Achieve { card: achievement })
    );
    let observation = game.observe(A).unwrap();
    let row = observation
        .options
        .iter()
        .find(|o| o[field::KIND] == f32::from(kind::ACHIEVE))
        .unwrap();
    assert_eq!(row[field::CARD], 0.0, "the front is hidden");
    assert_eq!(row[field::BACK], (Deck::base(1).index() + 1) as f32);
    assert_eq!(
        row[field::ZONE],
        f32::from(observation::zone_code(Zone::AvailableAchievements, A))
    );
}

#[test]
fn the_documented_layout_sizes_hold() {
    // doc/design/OBSERVATION.md states these offsets; change both together.
    assert_eq!(
        (
            scalar::DECKS,
            scalar::SPECIALS,
            scalar::DECREES,
            scalar::CITIES,
            scalar::PLAYERS
        ),
        (13, 85, 165, 185, 187)
    );
    assert_eq!(scalar::PLAYER, 79);
    assert_eq!(SCALARS, 345);
    assert_eq!(
        (HEADER, TOKEN_FIELDS, OPTION_FIELDS, GROUPS),
        (8, 5, 13, 22)
    );
    assert_eq!(FAMILY + GENERATION, 101);
    assert_eq!(observation::REVISION, 7, "revision 7 is frozen");
    assert_eq!((field::SECOND, field::SECOND_ZONE), (11, 12));
    assert_eq!(field::DECREE_TOKENS, 21);
    let mut game = Game::new_base(1).unwrap().with_tracking().unwrap();
    settle(&mut game, &mut Vec::new());
    assert_eq!(game.observe(Player::One).unwrap().row()[1], 7.0);
}

/// Token codes of the Options group are an OR of eight class bits, and every
/// option field is an integer `f32` represents exactly.
#[test]
fn option_codes_are_class_masks_and_option_fields_are_exact() {
    assert_eq!(observation::option_class(kind::MELD), 1);
    assert_eq!(observation::option_class(kind::ACHIEVE), 2);
    assert_eq!(observation::option_class(kind::DOGMA), 4);
    for k in [kind::SELECT, kind::REVEAL, kind::BLIND_SELECT] {
        assert_eq!(observation::option_class(k), 8);
    }
    for k in [kind::MOVE, kind::BLIND_MOVE] {
        assert_eq!(observation::option_class(k), 16);
    }
    assert_eq!(observation::option_class(kind::MATCHING_PAIR), 32);
    for k in [kind::SELF_EXECUTE, kind::SHOWCASE, kind::KARMA] {
        assert_eq!(observation::option_class(k), 64);
    }
    assert_eq!(observation::option_class(kind::NAME_FIGURE), 128);
    for k in [
        kind::DRAW,
        kind::DECREE_ACTION,
        kind::VALUE,
        kind::PASS,
        0,
        255,
    ] {
        assert_eq!(observation::option_class(k), 0);
    }
    for expansions in MODES {
        roots(expansions, 2, |game, _, actor, _| {
            let observation = game.observe(actor).unwrap();
            for token in &observation.tokens {
                if token.group == Group::Options {
                    assert!(token.code < 256, "{token:?}");
                    assert!(token.code != 0, "a named card has a class: {token:?}");
                }
                assert!(token.code < 1 << 24, "{token:?}");
            }
            for option in &observation.options {
                for &x in option.iter() {
                    assert!(
                        x.fract() == 0.0 && (0.0..16_777_216.0).contains(&x),
                        "{option:?}"
                    );
                }
            }
        });
    }
}

/// Alex Trebek's named draw: every name option names the Figure's card
/// identity and age, and pools into the Options group under the name class.
#[test]
fn name_figure_options_name_the_figures_card() {
    let mut f = Fixture::default();
    f.figure(Figure::AlexTrebek, Zone::Board(A, Color::Yellow));
    f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
    f.figure(Figure::Nightingale, Zone::Deck(Deck::figures(7)));
    f.tech(Color::Blue, 7, Zone::Deck(Deck::base(7)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.start(
        Operation::Draw {
            set: Some(CardSet::Figures),
            value: 7,
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    let decks = decision(&mut game, &mut events);
    let index = decks
        .options
        .iter()
        .position(|o| *o == Alternative::Deck(Deck::figures(7)))
        .unwrap();
    game.choose(index).unwrap();
    let names = decision(&mut game, &mut events);
    let observation = game.observe(A).unwrap();
    assert_eq!(observation.options.len(), names.options.len());
    let mut named = 0;
    for (option, row) in names.options.iter().zip(&observation.options) {
        match option {
            Alternative::NameFigure(name) => {
                let id = StableId::from_code(name.code).unwrap();
                assert_eq!(row[field::KIND], f32::from(kind::NAME_FIGURE));
                assert_eq!(row[field::CARD], f32::from(id.0 + 1));
                assert_eq!(row[field::NUMBER], 7.0);
                assert_eq!(row[field::TOKEN], 0.0);
                let token = observation
                    .tokens
                    .iter()
                    .find(|t| t.group == Group::Options && t.card == id.0 + 1)
                    .unwrap();
                assert_eq!(token.code, observation::option_class(kind::NAME_FIGURE));
                named += 1;
            }
            Alternative::Pass => assert_eq!(row[field::KIND], f32::from(kind::PASS)),
            other => panic!("{other:?}"),
        }
    }
    assert!(named >= 10, "{named}");
}

/// Unseen: Safe slots fill the reserved groups with one code bit per slot,
/// the capacity scalars are live, and secret options name their Safe.
#[test]
fn unseen_games_fill_their_reserved_layout() {
    // Three printed secrets, two of one back: the only age-3 card is known
    // in slot 0; the two age-1 candidates share slots 1 and 2, weight 1 each.
    let mut f = Fixture::default();
    let old = f.add(base_card("B031").unwrap(), Zone::Safe(A));
    let blue = f.add(base_card("B011").unwrap(), Zone::Safe(A));
    let green = f.add(base_card("B012").unwrap(), Zone::Safe(A));
    let code = f.tech(
        Color::Purple,
        1,
        Zone::Board(A, Color::Purple),
        vec![build::return_(Selector::Safe(Who::Executor))],
    );
    let mut game = Game::new(f.position(), Expansions::UNSEEN)
        .unwrap()
        .with_tracking()
        .unwrap();
    assert_eq!(game.position().card(old).origin_deck(), Deck::base(3));
    for card in [blue, green] {
        assert_eq!(game.position().card(card).origin_deck(), Deck::base(1));
    }
    game.self_execute(code, Context::new(A)).unwrap();
    let menu = decision(&mut game, &mut Vec::new());
    assert_eq!(menu.player, A);
    for viewer in Player::ALL {
        let observation = game.observe(viewer).unwrap();
        let s = &observation.scalars;
        assert_eq!(s[scalar::SETS + CardSet::Unseen.index()], 1.0);
        let a = scalar::PLAYERS + usize::from(viewer != A) * scalar::PLAYER;
        assert_eq!(s[a + scalar::SAFE_LIMIT], 0.5, "one unsplayed pile");
        assert_eq!(s[a + scalar::SAFE_ROOM], 0.2);
        let group = if viewer == A {
            Group::MySafe
        } else {
            Group::TheirSafe
        };
        let find = |card| {
            observation
                .tokens
                .iter()
                .find(|t| t.group == group && t.card == babylon_stable(&game, card))
                .unwrap_or_else(|| panic!("{viewer:?} {card:?}"))
        };
        assert_eq!((find(old).code, find(old).weight), (0b001, 1.0));
        for card in [blue, green] {
            assert_eq!(find(card).code, 0b110);
            assert!((find(card).weight - 1.0).abs() < 1e-6);
        }
        let total: f32 = observation
            .tokens
            .iter()
            .filter(|t| t.group == group)
            .map(|t| t.weight)
            .sum();
        assert!((total - 3.0).abs() < 1e-5, "{total}");
        let other = if viewer == A {
            Group::TheirSafe
        } else {
            Group::MySafe
        };
        assert!(!observation.tokens.iter().any(|t| t.group == other));
        if viewer == A {
            let slots: Vec<_> = observation
                .options
                .iter()
                .map(|o| {
                    (
                        o[field::KIND] as u8,
                        o[field::ZONE] as u8,
                        o[field::SLOT] as u8,
                    )
                })
                .collect();
            assert_eq!(
                slots,
                [
                    (kind::MOVE, observation::zone_code(Zone::Safe(A), A), 1),
                    (kind::MOVE, observation::zone_code(Zone::Safe(A), A), 2),
                    (kind::MOVE, observation::zone_code(Zone::Safe(A), A), 3),
                ]
            );
            assert_eq!(observation::zone_code(Zone::Safe(A), A), 19);
            assert_eq!(observation::zone_code(Zone::Safe(B), A), 20);
            assert!(observation.options.iter().all(|o| o[field::CARD] == 0.0));
        } else {
            assert!(observation.options.is_empty());
        }
    }

    // Random Unseen play reaches secrets on both sides, with slot bits and
    // own-Safe options (every printed Safe selector is the executor's own,
    // so zone 20 names only Bear Grylls' junk sweep); without Unseen the Safe groups
    // stay empty.
    let mut seen = (false, false, false);
    for expansions in MODES {
        if !expansions.unseen() {
            continue;
        }
        roots(expansions, 2, |game, _, actor, _| {
            let observation = game.observe(actor).unwrap();
            for token in &observation.tokens {
                match token.group {
                    Group::MySafe => {
                        assert!(token.code != 0 && token.code < 512, "{token:?}");
                        seen.0 = true;
                    }
                    Group::TheirSafe => {
                        assert!(token.code != 0 && token.code < 512, "{token:?}");
                        seen.1 = true;
                    }
                    _ => {}
                }
            }
            let s = &observation.scalars;
            for player in 0..2 {
                let base = scalar::PLAYERS + player * scalar::PLAYER;
                assert!(s[base + scalar::SAFE_LIMIT] > 0.0);
                assert!(s[base + scalar::SAFE_ROOM] <= s[base + scalar::SAFE_LIMIT]);
            }
            for option in &observation.options {
                match option[field::ZONE] as u8 {
                    19 => seen.2 = true,
                    // Only Bear Grylls' junk-everything sweep can name it.
                    20 if option[field::VERB] as u8 == Verb::Junk as u8 + 1 => {}
                    20 => panic!("no card selects from the other Safe: {option:?}"),
                    _ => {}
                }
            }
        });
    }
    assert_eq!(seen, (true, true, true), "(my Safe, their Safe, zone 19)");
    for expansions in MODES {
        if expansions.unseen() {
            continue;
        }
        roots(expansions, 1, |game, _, actor, _| {
            let observation = game.observe(actor).unwrap();
            assert!(
                !observation
                    .tokens
                    .iter()
                    .any(|t| matches!(t.group, Group::MySafe | Group::TheirSafe))
            );
            for player in 0..2 {
                let base = scalar::PLAYERS + player * scalar::PLAYER;
                assert_eq!(s_at(&observation, base + scalar::SAFE_LIMIT), 0.0);
                assert_eq!(s_at(&observation, base + scalar::SAFE_ROOM), 0.0);
                assert_eq!(s_at(&observation, base + scalar::FIRST_DRAWN), 0.0);
            }
            assert!(
                !observation
                    .options
                    .iter()
                    .any(|o| matches!(o[field::ZONE] as u8, 19 | 20))
            );
        });
    }
}

fn s_at(observation: &observation::Observation, at: usize) -> f32 {
    observation.scalars[at]
}
#[test]
fn cities_games_fill_their_reserved_layout() {
    let mut game = Game::new_game(9, Expansions::CITIES)
        .unwrap()
        .with_tracking()
        .unwrap();
    settle(&mut game, &mut Vec::new());
    let observation = game.observe(Player::One).unwrap();
    let s = &observation.scalars;
    assert_eq!(s[scalar::SETS + CardSet::Cities.index()], 1.0);
    assert_eq!(
        s[scalar::DECKS + Deck::cities(1).index()],
        15.0 / 15.0,
        "fifteen age-1 Cities"
    );
    // Cities' five tokens, available.
    for special in Special::CITIES {
        assert_eq!(s[scalar::SPECIALS + 4 * special.index()], 1.0);
    }
    assert_eq!(s[scalar::CITIES], 0.0);

    // An endorsed Dogma: Writing with Babylon and two value-1 cards to pay.
    let mut f = Fixture::default();
    let writing = f.add(base_card("B012").unwrap(), Zone::Board(A, Color::Blue));
    let babylon = f.add(city_card("C007").unwrap(), Zone::Board(A, Color::Green));
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    f.tech(Color::Red, 1, Zone::Hand(A), vec![]);
    for _ in 0..2 {
        f.tech(Color::Green, 2, Zone::Deck(Deck::base(2)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::CITIES)
        .unwrap()
        .with_tracking()
        .unwrap();
    game.take_action(
        Action::Dogma {
            card: writing,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    decision(&mut game, &mut Vec::new());
    let observation = game.observe(A).unwrap();
    assert_eq!(
        observation.scalars[scalar::CITIES..scalar::PLAYERS],
        [0.0, 0.0]
    );
    let kinds: Vec<_> = observation.options.iter().map(|o| o[field::KIND]).collect();
    assert_eq!(kinds, [f32::from(kind::BRANCH); 2]);
    // A top City's board token shows all six positions.
    let city = observation
        .tokens
        .iter()
        .find(|t| t.group == Group::MyBoard && t.card == babylon_stable(&game, babylon))
        .unwrap();
    assert_eq!(city.code & 0b11_1111, 0b11_1111);
    game.choose(0).unwrap();
    // The payment choice: the allowance is used and this action is endorsed.
    decision(&mut game, &mut Vec::new());
    let observation = game.observe(A).unwrap();
    assert_eq!(
        observation.scalars[scalar::CITIES..scalar::PLAYERS],
        [1.0, 1.0]
    );
    game.choose(0).unwrap();
    settle(&mut game, &mut Vec::new());
    let observation = game.observe(A).unwrap();
    assert_eq!(
        observation.scalars[scalar::CITIES..scalar::PLAYERS],
        [1.0, 0.0]
    );
}

#[test]
fn artifacts_games_fill_their_reserved_layout() {
    let mut game = Game::new_game(9, Expansions::ARTIFACTS)
        .unwrap()
        .with_tracking()
        .unwrap();
    settle(&mut game, &mut Vec::new());
    let observation = game.observe(Player::One).unwrap();
    let s = &observation.scalars;
    assert_eq!(s[scalar::SETS + CardSet::Artifacts.index()], 1.0);
    let age_one = Deck {
        set: CardSet::Artifacts,
        age: 1,
    };
    assert!(
        s[scalar::DECKS + age_one.index()] > 0.0,
        "ten age-1 Artifacts"
    );

    // A displayed and a stored Artifact are public to both viewers, each
    // under the viewer-relative display and Museum groups.
    let mut f = Fixture::default();
    let shown = f.add(artifacts_card("A001").unwrap(), Zone::Display(A));
    let stored = f.add(artifacts_card("A016").unwrap(), Zone::Museum(B));
    let game = Game::new(f.position(), Expansions::ARTIFACTS)
        .unwrap()
        .with_tracking()
        .unwrap();
    for viewer in Player::ALL {
        let observation = game.observe(viewer).unwrap();
        let (display, museum) = if viewer == A {
            (Group::MyDisplay, Group::TheirMuseums)
        } else {
            (Group::TheirDisplay, Group::MyMuseums)
        };
        let find = |group, card| {
            observation
                .tokens
                .iter()
                .find(|t| t.group == group && t.card == babylon_stable(&game, card))
                .unwrap_or_else(|| panic!("{viewer:?} {group:?} {card:?}"))
        };
        assert!((find(display, shown).weight - 1.0).abs() < 1e-6);
        assert!((find(museum, stored).weight - 1.0).abs() < 1e-6);
        assert!(!observation.tokens.iter().any(|t| matches!(
            t.group,
            Group::MyForecast | Group::TheirForecast | Group::MySafe | Group::TheirSafe
        )));
    }
}

fn babylon_stable(game: &Game, card: CardId) -> u16 {
    game.position().card(card).stable_id().unwrap().0 + 1
}
