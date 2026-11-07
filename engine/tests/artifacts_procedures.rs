//! Artifacts: the dig or seizure after a Meld action, Museum
//! Meld actions, the showcase at turn start, rotation, the Museum Check and
//! the victory check that follows it. Explicit positions with synthetic
//! cards; expected outcomes follow doc/rules/ARTIFACTS_RULES.md and the FAQ.

use inno_engine::*;

mod common;
use common::*;

fn artifacts() -> Expansions {
    Expansions::ARTIFACTS
}

/// A synthetic Artifact: its image at the third printed position.
fn artifact(f: &mut Fixture, color: Color, value: u8, zone: Zone, effects: Vec<Effect>) -> CardId {
    f.add(
        Card {
            code: "synthetic-artifact",
            set: CardSet::Artifacts,
            color,
            value,
            figure: None,
            echo: None,
            symbols: [
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Image,
                Symbol::Blank,
                Symbol::Blank,
                Symbol::Blank,
            ],
            featured: Some(Icon::Concept),
            effects,
        },
        zone,
    )
}

fn deck(value: u8) -> Zone {
    Zone::Deck(Deck {
        set: CardSet::Artifacts,
        age: value,
    })
}

fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, vec![])
}

fn non_demand(instructions: Vec<Instruction>) -> Effect {
    Effect {
        kind: EffectKind::NonDemand,
        instructions,
    }
}

/// Settle, answering each decision with `decide`; a lone option is taken.
fn play(
    game: &mut Game,
    mut decide: impl FnMut(&Decision) -> Option<usize>,
) -> (Status, Vec<Event>) {
    let mut events = Vec::new();
    loop {
        let status = settle(game, &mut events);
        assert!(game.position().check_conservation());
        match status {
            Status::Decision(decision) => {
                let option = decide(&decision)
                    .or((decision.options.len() == 1).then_some(0))
                    .unwrap_or_else(|| panic!("unexpected decision {decision:?}"));
                game.choose(option).unwrap();
            }
            status => return (status, events),
        }
    }
}

fn no_decisions(game: &mut Game) -> (Status, Vec<Event>) {
    play(game, |_| None)
}

fn moves(events: &[Event]) -> Vec<(Option<Verb>, CardId, Zone, Zone)> {
    events
        .iter()
        .filter_map(|event| match *event {
            Event::Moved(Movement {
                verb,
                card,
                source,
                destination,
                ..
            }) => Some((verb, card, source, destination)),
            _ => None,
        })
        .collect()
}

fn meld_action(game: &mut Game, card: CardId) {
    game.take_action(Action::Meld { card }, ActionKind::Regular)
        .unwrap();
}

fn option_index(decision: &Decision, pick: impl Fn(&Alternative) -> bool) -> Option<usize> {
    decision.options.iter().position(pick)
}

/// A Meld action over a higher or equal top digs the Artifact of the
/// covered value onto the display; a higher card with a different image
/// does not.
#[test]
fn a_meld_action_digs_when_the_value_is_not_higher_or_the_image_matches() {
    for (melded_value, same_image, expect_dig) in [
        (2, false, true),
        (3, false, true),
        (4, false, false),
        (4, true, true),
    ] {
        let mut f = Fixture::default();
        let covered = blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
        let melded = blank(&mut f, Color::Red, melded_value, Zone::Hand(A));
        if same_image {
            for card in [covered, melded] {
                f.cards[usize::from(card.0)].symbols[1] = Symbol::Image;
            }
        }
        blank(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
        let dug = artifact(&mut f, Color::Green, 3, deck(3), vec![]);
        let mut game = Game::new(f.position(), artifacts()).unwrap();
        meld_action(&mut game, melded);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let mut expected = vec![(
            Some(Verb::Meld),
            melded,
            Zone::Hand(A),
            Zone::Board(A, Color::Red),
        )];
        if expect_dig {
            expected.push((Some(Verb::Dig), dug, deck(3), Zone::Display(A)));
        }
        assert_eq!(moves(&events), expected, "{melded_value} {same_image}");
        let display: Vec<CardId> = if expect_dig { vec![dug] } else { vec![] };
        assert_eq!(game.position().cards(Zone::Display(A)), display.as_slice());
        assert_eq!(game.position().value(dug), 3);
    }
}

/// The dig skips values whose Base deck is empty, takes the top of the
/// resolved Artifact deck, and is ignored when that deck is empty, the
/// value passes 11 or the display is occupied. It never falls back to Base.
#[test]
fn a_dig_skips_empty_base_ages_and_is_ignored_without_a_card_or_room() {
    // Base 3 empty, Base 4 present: dig at 4.
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    blank(&mut f, Color::Blue, 4, Zone::Deck(Deck::base(4)));
    let three = artifact(&mut f, Color::Green, 3, deck(3), vec![]);
    let four_under = artifact(&mut f, Color::Green, 4, deck(4), vec![]);
    let four_top = artifact(&mut f, Color::Yellow, 4, deck(4), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    meld_action(&mut game, melded);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(
        moves(&events)[1],
        (Some(Verb::Dig), four_top, deck(4), Zone::Display(A))
    );
    assert_eq!(game.position().cards(deck(4)), &[four_under]);
    assert_eq!(game.position().cards(deck(3)), &[three]);

    // No Artifact of the resolved value: nothing happens, no Base fallback.
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    blank(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
    artifact(&mut f, Color::Green, 4, deck(4), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    meld_action(&mut game, melded);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(moves(&events).len(), 1);
    assert!(game.position().cards(Zone::Display(A)).is_empty());

    // Every Base deck from the covered value up is empty: no dig, no end.
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 11, Zone::Board(A, Color::Red));
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    artifact(&mut f, Color::Green, 11, deck(11), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    meld_action(&mut game, melded);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(moves(&events).len(), 1);

    // An occupied display ignores the dig.
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    blank(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
    artifact(&mut f, Color::Green, 3, deck(3), vec![]);
    let shown = artifact(&mut f, Color::Purple, 2, Zone::Display(A), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    meld_action(&mut game, melded);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(moves(&events).len(), 1);
    assert_eq!(game.position().cards(Zone::Display(A)), &[shown]);
}

/// Effect melds never dig; a Meld action onto an empty color has nothing
/// covered; a game without Artifacts has no dig stage.
#[test]
fn only_meld_actions_over_a_covered_card_dig() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let melder = f.tech(
        Color::Blue,
        1,
        Zone::Board(A, Color::Blue),
        vec![build::meld(build::HAND)],
    );
    let _melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    blank(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
    artifact(&mut f, Color::Green, 3, deck(3), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    game.take_action(
        Action::Dogma {
            card: melder,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
    let (_, events) = no_decisions(&mut game);
    assert_eq!(moves(&events).len(), 1);
    assert!(game.position().cards(Zone::Display(A)).is_empty());

    let mut f = Fixture::default();
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    blank(&mut f, Color::Blue, 1, Zone::Deck(Deck::base(1)));
    artifact(&mut f, Color::Green, 1, deck(1), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    meld_action(&mut game, melded);
    let (_, events) = no_decisions(&mut game);
    assert_eq!(moves(&events).len(), 1);

    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    blank(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    meld_action(&mut game, melded);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(moves(&events).len(), 1);
}

/// Seizing an opposing Museum Artifact of the original covered value is
/// offered before the dig, with its Museum; it cancels the dig. Declining
/// digs as usual.
#[test]
fn seizing_an_opposing_museum_of_the_covered_value_replaces_the_dig() {
    for seize in [true, false] {
        let mut f = Fixture::default();
        blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
        let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
        blank(&mut f, Color::Blue, 3, Zone::Deck(Deck::base(3)));
        let dug = artifact(&mut f, Color::Green, 3, deck(3), vec![]);
        let stored = artifact(&mut f, Color::Yellow, 3, Zone::Museum(B), vec![]);
        let other = artifact(&mut f, Color::Purple, 4, Zone::Museum(B), vec![]);
        let mut game = Game::new(f.position(), artifacts()).unwrap();
        assert_eq!(game.position().available_museums(), 3);
        meld_action(&mut game, melded);
        let mut offers = 0;
        let (status, events) = play(&mut game, |decision| {
            offers += 1;
            assert_eq!(decision.player, A);
            assert_eq!(decision.options.len(), 2);
            assert_eq!(
                decision.options[0],
                Alternative::Move {
                    verb: Verb::Seize,
                    target: TargetDescriptor::Known {
                        card: stored,
                        source: Zone::Museum(B),
                    },
                }
            );
            assert_eq!(decision.options[1], Alternative::Decline);
            Some(if seize { 0 } else { 1 })
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert_eq!(offers, 1);
        let second = if seize {
            (Some(Verb::Seize), stored, Zone::Museum(B), Zone::Museum(A))
        } else {
            (Some(Verb::Dig), dug, deck(3), Zone::Display(A))
        };
        assert_eq!(moves(&events)[1], second);
        assert_eq!(moves(&events).len(), 2);
        if seize {
            assert_eq!(game.position().cards(Zone::Museum(A)), &[stored]);
            assert_eq!(game.position().cards(Zone::Museum(B)), &[other]);
            assert_eq!(game.position().available_museums(), 3);
            assert!(game.position().cards(Zone::Display(A)).is_empty());
        }
    }
}

/// A Meld action may take an Artifact from one of the player's Museums,
/// freeing the Museum; it is a genuine Meld action, so it can dig.
#[test]
fn a_museum_meld_action_frees_the_museum_and_can_dig() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 5, Zone::Board(A, Color::Red));
    let stored = artifact(&mut f, Color::Red, 4, Zone::Museum(A), vec![]);
    blank(&mut f, Color::Blue, 5, Zone::Deck(Deck::base(5)));
    let dug = artifact(&mut f, Color::Green, 5, deck(5), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    assert!(
        game.legal_actions()
            .contains(&Action::Meld { card: stored })
    );
    assert_eq!(game.position().available_museums(), 4);
    meld_action(&mut game, stored);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (
                Some(Verb::Meld),
                stored,
                Zone::Museum(A),
                Zone::Board(A, Color::Red)
            ),
            (Some(Verb::Dig), dug, deck(5), Zone::Display(A)),
        ]
    );
    assert_eq!(game.position().available_museums(), 5);
    assert_eq!(game.position().top(A, Color::Red), Some(stored));
    assert_eq!(game.position().value(stored), 4);
}

/// At the start of its owner's turn a displayed Artifact may be dogma'd as
/// a free action (its icons count for sharing), then rotates into a Museum.
/// Both regular actions remain.
#[test]
fn a_showcase_offers_a_free_dogma_then_rotates_into_a_museum() {
    for take in [true, false] {
        let mut f = Fixture::default();
        let shown = artifact(
            &mut f,
            Color::Green,
            2,
            Zone::Display(A),
            vec![non_demand(vec![build::draw(1)])],
        );
        f.cards[usize::from(shown.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        let b_top = blank(&mut f, Color::Blue, 1, Zone::Board(B, Color::Blue));
        f.cards[usize::from(b_top.0)].symbols[0] = Symbol::Icon(Icon::Concept);
        let first = blank(&mut f, Color::Red, 1, Zone::Deck(Deck::base(1)));
        let second = blank(&mut f, Color::Red, 1, Zone::Deck(Deck::base(1)));
        let third = blank(&mut f, Color::Red, 1, Zone::Deck(Deck::base(1)));
        let mut game = Game::new(f.position(), artifacts()).unwrap();
        game.begin_turn(A).unwrap();
        let (status, events) = play(&mut game, |decision| {
            assert_eq!(decision.player, A);
            assert_eq!(
                decision.options,
                [
                    Alternative::Showcase {
                        card: shown,
                        auspice: false
                    },
                    Alternative::Decline
                ]
            );
            Some(usize::from(!take))
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let rotation = (Some(Verb::Rotate), shown, Zone::Display(A), Zone::Museum(A));
        if take {
            // B shares (one Concept each, the display's counting), A executes,
            // then A draws for sharing.
            assert_eq!(
                moves(&events),
                [
                    (None, third, Zone::Deck(Deck::base(1)), Zone::Hand(B)),
                    (None, second, Zone::Deck(Deck::base(1)), Zone::Hand(A)),
                    (None, first, Zone::Deck(Deck::base(1)), Zone::Hand(A)),
                    rotation,
                ]
            );
        } else {
            assert_eq!(moves(&events), [rotation]);
        }
        assert_eq!(game.position().cards(Zone::Museum(A)), &[shown]);
        assert_eq!(game.position().available_museums(), 4);
        assert_eq!(game.position().turn_owner(), A);
        assert_eq!(game.position().regular_actions_started(), 0);
        assert!(game.legal_actions().contains(&Action::Draw));
    }
}

/// An effectless Artifact (Yamato) offers no Dogma and simply rotates; an
/// Artifact that leaves the display during its Dogma is not rotated.
#[test]
fn showcases_without_a_dogma_or_without_a_card_left_only_rotate_what_remains() {
    let mut f = Fixture::default();
    let yamato = f.add(artifacts_card("A077").unwrap(), Zone::Display(B));
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    game.begin_turn(B).unwrap();
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Rotate),
            yamato,
            Zone::Display(B),
            Zone::Museum(B)
        )]
    );
    assert_eq!(game.position().value(yamato), 11);

    let mut f = Fixture::default();
    let shown = artifact(
        &mut f,
        Color::Green,
        2,
        Zone::Display(A),
        vec![non_demand(vec![
            build::verb(Verb::Transfer, Selector::Reg(Reg::Source))
                .to(Destination::Board(Who::Executor)),
        ])],
    );
    // One Concept on the display: B cannot share.
    f.cards[usize::from(shown.0)].symbols[0] = Symbol::Icon(Icon::Concept);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    game.begin_turn(A).unwrap();
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Transfer),
            shown,
            Zone::Display(A),
            Zone::Board(A, Color::Green)
        )]
    );
    assert!(game.position().cards(Zone::Museum(A)).is_empty());
    assert_eq!(game.position().top(A, Color::Green), Some(shown));
}

/// Filling the last Museum awards one to the unique largest holder before
/// the current player returns every stored Artifact; the award then counts
/// as an achievement and the Museums go back to the supply.
#[test]
fn filling_the_last_museum_awards_the_largest_holder_then_returns_everything() {
    let mut f = Fixture::default();
    let shown = artifact(&mut f, Color::Green, 2, Zone::Display(A), vec![]);
    let a1 = artifact(&mut f, Color::Red, 1, Zone::Museum(A), vec![]);
    let a3 = artifact(&mut f, Color::Blue, 3, Zone::Museum(A), vec![]);
    let b4 = artifact(&mut f, Color::Yellow, 4, Zone::Museum(B), vec![]);
    let b5 = artifact(&mut f, Color::Purple, 5, Zone::Museum(B), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    assert_eq!(game.position().available_museums(), 1);
    game.begin_turn(A).unwrap();
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let claimed = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::MuseumClaimed {
                    player: A,
                    executor: A,
                    ..
                }
            )
        })
        .expect("A claims a Museum");
    assert_eq!(claimed, 1, "award before the returns");
    let returns: Vec<_> = moves(&events)[1..]
        .iter()
        .map(|&(verb, card, source, _)| (verb, card, source))
        .collect();
    assert_eq!(returns.len(), 5);
    assert!(
        returns
            .iter()
            .all(|&(verb, _, _)| verb == Some(Verb::Return))
    );
    for (card, owner) in [(a1, A), (a3, A), (shown, A), (b4, B), (b5, B)] {
        assert!(returns.contains(&(Some(Verb::Return), card, Zone::Museum(owner))));
        assert_eq!(
            game.position().cards(deck(game.position().value(card))),
            &[card]
        );
    }
    assert!(game.position().cards(Zone::Museum(A)).is_empty());
    assert!(game.position().cards(Zone::Museum(B)).is_empty());
    assert_eq!(game.position().claimed_museums(A), 1);
    assert_eq!(game.position().achievement_count(A), 1);
    assert_eq!(game.position().available_museums(), 4);
    assert_eq!(game.position().regular_actions_started(), 0);
}

/// The current player orders every return, including an opponent's cards,
/// and only same-deck returns are real choices; each return goes under its
/// deck. The award goes to the opponent when they hold more.
#[test]
fn the_current_player_orders_same_deck_returns_even_when_the_opponent_is_awarded() {
    let mut f = Fixture::default();
    let shown = artifact(&mut f, Color::Green, 2, Zone::Display(A), vec![]);
    let a_two = artifact(&mut f, Color::Red, 2, Zone::Museum(A), vec![]);
    let b_two = artifact(&mut f, Color::Yellow, 2, Zone::Museum(B), vec![]);
    let b_five = artifact(&mut f, Color::Purple, 5, Zone::Museum(B), vec![]);
    let b_seven = artifact(&mut f, Color::Blue, 7, Zone::Museum(B), vec![]);
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    assert_eq!(game.position().available_museums(), 1);
    game.begin_turn(A).unwrap();
    let mut decisions = 0;
    let (status, events) = play(&mut game, |decision| {
        assert_eq!(decision.player, A);
        decisions += 1;
        assert!(decision.options.iter().all(|o| matches!(
            o,
            Alternative::Move {
                verb: Verb::Return,
                ..
            }
        )));
        // B's 2 first, then A's 2, then the showcased 2.
        [b_two, a_two, shown].into_iter().find_map(|wanted| {
            option_index(decision, |o| {
                matches!(o, Alternative::Move { target: TargetDescriptor::Known { card, .. }, .. } if *card == wanted)
            })
        })
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // After B's and A's 2s, the showcased 2 is alone in its deck.
    assert_eq!(decisions, 2);
    let claims: Vec<Player> = events
        .iter()
        .filter_map(|e| match e {
            Event::MuseumClaimed { player, .. } => Some(*player),
            _ => None,
        })
        .collect();
    assert_eq!(claims, [B]);
    assert_eq!(game.position().claimed_museums(B), 1);
    assert_eq!(game.position().claimed_museums(A), 0);
    assert_eq!(game.position().cards(deck(2)), &[shown, a_two, b_two]);
    assert_eq!(game.position().cards(deck(5)), &[b_five]);
    assert_eq!(game.position().cards(deck(7)), &[b_seven]);
    assert!(game.position().cards(Zone::Museum(A)).is_empty());
    assert!(game.position().cards(Zone::Museum(B)).is_empty());
    assert_eq!(game.position().available_museums(), 4);
}

/// The Museum award can complete an achievement victory, recognized by the
/// check that follows the Museum Check, before any regular action.
#[test]
fn a_museum_award_can_win_before_the_turns_first_action() {
    let mut f = Fixture::default();
    let shown = artifact(&mut f, Color::Green, 6, Zone::Display(A), vec![]);
    for value in 1..=4 {
        artifact(&mut f, Color::Red, value, Zone::Museum(A), vec![]);
    }
    for value in 1..=6 {
        blank(&mut f, Color::Blue, value, Zone::Achievements(A));
    }
    let mut game = Game::new(f.position(), artifacts()).unwrap();
    assert_eq!(game.position().available_museums(), 1);
    game.begin_turn(A).unwrap();
    let (status, _) = no_decisions(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(game.position().claimed_museums(A), 1);
    let _ = shown;
}

/// Kepler's "instead" re-issues the showcase Dogma under its own context;
/// the displayed Artifact is still an admitted source.
#[test]
fn a_showcase_dogma_survives_a_figure_reissuing_the_dogma_action() {
    let mut f = Fixture::default();
    let shown = artifact(
        &mut f,
        Color::Green,
        2,
        Zone::Display(A),
        vec![non_demand(vec![build::draw(1)])],
    );
    let kepler = figure_card(Figure::JohannesKepler.code()).unwrap();
    let color = kepler.color;
    f.add(kepler, Zone::Board(A, color));
    let one = blank(&mut f, Color::Red, 1, Zone::Deck(Deck::base(1)));
    let two = blank(&mut f, Color::Red, 2, Zone::Deck(Deck::base(2)));
    let mut game = Game::new(f.position(), artifacts().union(Expansions::FIGURES)).unwrap();
    game.begin_turn(A).unwrap();
    let (status, events) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Showcase { .. }))
            .or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    // Kepler raises the boxed 1 by the number of distinct top values above
    // 3 on A's board, so the draw takes a 1 or a 2 depending on his value.
    assert!(
        moved
            .iter()
            .any(|m| m.0.is_none() && (m.1 == one || m.1 == two)),
        "{moved:?}"
    );
    assert_eq!(
        moved.last(),
        Some(&(Some(Verb::Rotate), shown, Zone::Display(A), Zone::Museum(A)))
    );
}

/// Returns to distinct decks commute: the Museum Check offers one deck at a
/// time, lowest first, so a lone [2], two [3]s and two [5]s ask two two-way
/// questions rather than a five-card order. A Figure that reacts to returns
/// keeps every order.
#[test]
fn museum_check_returns_one_deck_at_a_time_unless_a_figure_watches_returns() {
    for maxwell in [false, true] {
        let mut f = Fixture::default();
        let shown = artifact(&mut f, Color::Green, 2, Zone::Display(A), vec![]);
        let threes = [Color::Red, Color::Blue]
            .map(|color| artifact(&mut f, color, 3, Zone::Museum(A), vec![]));
        let fives = [Color::Yellow, Color::Purple]
            .map(|color| artifact(&mut f, color, 5, Zone::Museum(B), vec![]));
        let mut expansions = artifacts();
        if maxwell {
            // Maxwell draws an [8] before each of these returns.
            let card = figure_card(Figure::JamesClerkMaxwell.code()).unwrap();
            let color = card.color;
            f.add(card, Zone::Board(B, color));
            for _ in 0..6 {
                blank(&mut f, Color::Yellow, 8, Zone::Deck(Deck::base(8)));
            }
            expansions = expansions.union(Expansions::FIGURES);
        }
        let mut game = Game::new(f.position(), expansions).unwrap();
        assert_eq!(game.position().available_museums(), 1);
        game.begin_turn(A).unwrap();
        let mut sizes = Vec::new();
        let (status, _) = play(&mut game, |decision| {
            assert!(decision.options.iter().all(|o| matches!(
                o,
                Alternative::Move {
                    verb: Verb::Return,
                    ..
                }
            )));
            sizes.push(decision.options.len());
            Some(0)
        });
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        if maxwell {
            assert_eq!(sizes, [5, 4, 3, 2]);
        } else {
            // The [2] is alone in its deck and goes first unasked; then one
            // question per deck.
            assert_eq!(sizes, [2, 2]);
        }
        for (card, value) in [
            (shown, 2),
            (threes[0], 3),
            (threes[1], 3),
            (fives[0], 5),
            (fives[1], 5),
        ] {
            assert!(game.position().cards(deck(value)).contains(&card));
        }
        assert!(game.position().cards(Zone::Museum(A)).is_empty());
        assert!(game.position().cards(Zone::Museum(B)).is_empty());
    }
}
