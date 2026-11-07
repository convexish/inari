use inno_engine::*;

mod common;
use common::*;

#[test]
fn an_empty_base_deck_makes_the_age_empty_even_with_figures_remaining() {
    for set in [None, Some(CardSet::Figures)] {
        let mut f = Fixture::default();
        let skipped = f.figure(Figure::EmperorMeiji, Zone::Deck(Deck::figures(7)));
        let figure = f.figure(Figure::Kalashnikov, Zone::Deck(Deck::figures(9)));
        let base = f.tech(Color::Blue, 9, Zone::Deck(Deck::base(9)), vec![]);
        let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
        game.start(Operation::Draw { value: 7, set }, Context::new(A))
            .unwrap();
        let mut events = Vec::new();
        let expected = if set.is_none() { base } else { figure };
        assert_eq!(
            settle(&mut game, &mut events),
            Status::Complete(ReturnValue::Operation(OperationOutcome {
                disposition: Disposition::Performed,
                card: Some(expected),
                value: Some(9),
                short: true,
            }))
        );
        assert_eq!(game.position().cards(Zone::Hand(A)), &[expected]);
        assert_eq!(
            game.position().cards(Zone::Deck(Deck::figures(7))),
            &[skipped]
        );
        assert_eq!(events.len(), 1);
        assert_eq!(
            moved(&events[0]).source,
            Zone::Deck(Deck {
                set: set.unwrap_or(CardSet::Base),
                age: 9
            })
        );
        assert!(game.position().check_conservation());
    }
}

#[test]
fn an_empty_figures_deck_falls_back_to_base_instead_of_a_later_figures_deck() {
    let mut f = Fixture::default();
    let figure = f.figure(Figure::Kalashnikov, Zone::Deck(Deck::figures(9)));
    let base = f.tech(Color::Blue, 8, Zone::Deck(Deck::base(8)), vec![]);
    let later_base = f.tech(Color::Green, 9, Zone::Deck(Deck::base(9)), vec![]);
    let mut game = Game::new(f.position(), Expansions::FIGURES).unwrap();
    game.start(
        Operation::Draw {
            value: 7,
            set: Some(CardSet::Figures),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Operation(OperationOutcome {
            disposition: Disposition::Performed,
            card: Some(base),
            value: Some(8),
            short: true,
        }))
    );
    assert_eq!(game.position().cards(Zone::Hand(A)), &[base]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::figures(9))),
        &[figure]
    );
    assert_eq!(
        game.position().cards(Zone::Deck(Deck::base(9))),
        &[later_base]
    );
    assert_eq!(events.len(), 1);
    assert_eq!(moved(&events[0]).source, Zone::Deck(Deck::base(8)));
    assert!(game.position().check_conservation());
}

/// Echoes redirects only ordinary draws: an explicit Base request draws
/// Base, and an explicit set whose deck is empty falls back to Base once.
#[test]
fn only_ordinary_draws_are_redirected_to_echoes() {
    for (set, from_echoes) in [(None, true), (Some(CardSet::Base), false)] {
        let mut f = Fixture::default();
        f.tech(Color::Blue, 5, Zone::Board(A, Color::Blue), vec![]);
        let base = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
        let echo = f.add(
            echoes_card("E016").unwrap(),
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 2,
            }),
        );
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        game.start(Operation::Draw { value: 2, set }, Context::new(A))
            .unwrap();
        let mut events = Vec::new();
        settle(&mut game, &mut events);
        let expected = if from_echoes { echo } else { base };
        assert_eq!(game.position().cards(Zone::Hand(A)), &[expected], "{set:?}");
    }

    let mut f = Fixture::default();
    let base = f.tech(Color::Red, 2, Zone::Deck(Deck::base(2)), vec![]);
    let later = f.add(
        echoes_card("E026").unwrap(),
        Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 3,
        }),
    );
    let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
    game.start(
        Operation::Draw {
            value: 2,
            set: Some(CardSet::Echoes),
        },
        Context::new(A),
    )
    .unwrap();
    let mut events = Vec::new();
    settle(&mut game, &mut events);
    assert_eq!(game.position().cards(Zone::Hand(A)), &[base]);
    assert_eq!(
        game.position().cards(Zone::Deck(Deck {
            set: CardSet::Echoes,
            age: 3
        })),
        &[later]
    );
}
