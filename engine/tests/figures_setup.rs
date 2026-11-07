//! The complete Base + Figures deal, seeded full games, and the draw that
//! ends a mandatory loop.
use inno_engine::*;

mod common;
use common::*;

#[test]
fn figures_deal_keeps_the_base_deal_and_adds_figures_decrees_and_dormant_age_zero() {
    for seed in [0, 7, 42] {
        let figures = Game::new_figures(seed).unwrap();
        let base = Game::new_base(seed).unwrap();
        let (with, without) = (figures.position(), base.position());
        for player in Player::ALL {
            assert_eq!(
                with.cards(Zone::Hand(player)),
                without.cards(Zone::Hand(player))
            );
        }
        for age in 0..=11 {
            assert_eq!(
                with.cards(Zone::Deck(Deck::base(age))),
                without.cards(Zone::Deck(Deck::base(age)))
            );
        }
        assert_eq!(
            with.cards(Zone::AvailableAchievements),
            without.cards(Zone::AvailableAchievements)
        );
        for age in 1..=11 {
            let deck = with.cards(Zone::Deck(Deck::figures(age)));
            let printed = FIGURE_METADATA.iter().filter(|m| m.value == age).count();
            assert_eq!(deck.len(), printed);
            assert!(deck.iter().all(|&card| {
                let card = with.card(card);
                card.figure.is_some() && card.value == age
            }));
        }
        assert_eq!(with.cards(Zone::AgeZeroReserve).len(), 15);
        assert!(!with.age_zero_introduced());
        for decree in Decree::ALL {
            assert_eq!(with.decree_location(decree), DecreeLocation::Supply);
        }
        assert!(with.check_conservation());
    }
    // The Figures decks are shuffled.
    let order = |seed| {
        Game::new_figures(seed)
            .unwrap()
            .position()
            .cards(Zone::Deck(Deck::figures(1)))
            .to_vec()
    };
    assert_ne!(order(0), order(1));
}

#[test]
fn seeded_random_figures_games_finish_and_conserve_every_card() {
    for seed in 0..40 {
        let mut game = Game::new_figures(seed).unwrap();
        let mut policy = Rng::new(seed);
        let mut events = Vec::new();
        let mut steps = 0;
        let status = loop {
            steps += 1;
            assert!(steps < 20_000, "seed {seed} did not finish");
            match game.advance(1 << 16, &mut events).unwrap() {
                Status::Decision(menu) => {
                    let index = (policy.next_u64() % menu.options.len() as u64) as usize;
                    game.choose(index).unwrap();
                }
                Status::Complete(_) => {
                    let actions = game.legal_actions();
                    let action = actions[(policy.next_u64() % actions.len() as u64) as usize];
                    game.take_action(action, ActionKind::Regular).unwrap();
                }
                status => break status,
            }
            assert!(game.position().check_conservation(), "seed {seed}");
        };
        assert!(matches!(status, Status::Winner(_) | Status::Draw(_)));
    }
}

#[test]
fn a_mandatory_loop_that_repeats_the_whole_game_state_is_a_draw() {
    // Astronomy repeats on green draws; Emma returns the melder's previous
    // green top, so two green cards cycle between the [6] and [11] decks.
    let mut f = Fixture::default();
    f.figure(Figure::EmmaWatson, Zone::Board(A, Color::Purple));
    let astronomy = f.add(base_card("B054").unwrap(), Zone::Board(B, Color::Purple));
    f.tech(Color::Green, 6, Zone::Board(B, Color::Green), vec![]);
    f.tech(Color::Green, 11, Zone::Deck(Deck::base(11)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::ExecuteEffect {
            card: astronomy,
            effect: 0,
            boxed: BoxedValues::default(),
        },
        Context::new(B),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        game.advance(10_000, &mut events).unwrap(),
        Status::Draw(DrawReason::Loop)
    );
    assert!(game.position().check_conservation());
}
