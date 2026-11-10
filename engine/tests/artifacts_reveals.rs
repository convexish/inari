//! Proof reveals: a chosen hidden card is revealed when its front sets a restriction or
//! decides a later operation, and an impossible mandatory instruction or an exhausted
//! sweep reveals what proves it.
use inno_engine::*;

mod common;
use common::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

/// A printed Artifact as that player's top of its own color.
fn top(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let color = Card::from_code(code).unwrap().color;
    f.card(code, Zone::Board(player, color))
}

fn any(game: &mut Game) -> (Status, Vec<Event>) {
    play(game, |_| Some(0))
}

#[test]
fn priest_king_reveals_the_scored_card_of_every_executor() {
    let mut f = Fixture::default();
    let priest = top(&mut f, "A007", A);
    f.plain(
        Color::Green,
        2,
        board(B, Color::Green),
        &sharing_icons("A007"),
    );
    let b_hand = f.plain(Color::Green, 3, HAND_B, &[]);
    let a_hand = f.plain(Color::Blue, 1, HAND_A, &[]);
    f.plain(Color::Blue, 1, board(A, Color::Blue), &[]);
    f.plain(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, priest);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert!(
        shown.contains(&b_hand) && shown.contains(&a_hand),
        "{shown:?}"
    );
}

#[test]
fn ark_of_the_covenant_reveals_the_returned_card() {
    let mut f = Fixture::default();
    let ark = top(&mut f, "A014", A);
    let returned = f.plain(Color::Red, 2, HAND_A, &[]);
    let b_red = f.plain(Color::Red, 1, board(B, Color::Red), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, ark);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [returned]);
    assert_eq!(game.position().cards(SCORE_A), &[b_red]);
}

#[test]
fn papyrus_of_ani_reveals_the_purple_return_or_the_hand_proving_none() {
    let mut f = Fixture::default();
    let papyrus = top(&mut f, "A002", A);
    let purple = f.plain(Color::Purple, 1, HAND_A, &[]);
    let other = f.plain(Color::Blue, 1, HAND_A, &[]);
    let three = f.plain(Color::Green, 3, base_deck(3), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, papyrus);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, purple).or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [purple, three]);
    assert!(!revealed(&events).contains(&other));

    let mut f = Fixture::default();
    let papyrus = top(&mut f, "A002", A);
    let blue = f.plain(Color::Blue, 1, HAND_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, papyrus);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [blue]);
    assert!(moves(&events).is_empty());
}

#[test]
fn terracotta_army_reveals_the_scored_card_or_the_hand_proving_none() {
    let mut f = Fixture::default();
    let army = top(&mut f, "A019", A);
    f.cards[usize::from(army.0)].effects.remove(0);
    let plain = f.plain(Color::Blue, 2, HAND_A, &[]);
    let deck = f.plain(Color::Green, 2, base_deck(2), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, army);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [plain]);
    assert_eq!(game.position().cards(Zone::Junk), &[deck]);

    let mut f = Fixture::default();
    let army = top(&mut f, "A019", A);
    f.cards[usize::from(army.0)].effects.remove(0);
    let authority = f.plain(Color::Blue, 2, HAND_A, &[Icon::Authority]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, army);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [authority]);
    assert_eq!(moves(&events).len(), 1, "Terracotta tucks itself");
}

#[test]
fn shroud_of_turin_reveals_both_hidden_returns_or_the_score_proving_none() {
    let mut f = Fixture::default();
    let shroud = top(&mut f, "A035", A);
    let hand = f.plain(Color::Red, 2, HAND_A, &[]);
    f.plain(Color::Red, 3, board(A, Color::Red), &[]);
    let scored = f.plain(Color::Red, 4, SCORE_A, &[]);
    let ach = f.plain(Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, shroud);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [hand, scored]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[ach]);

    let mut f = Fixture::default();
    let shroud = top(&mut f, "A035", A);
    let hand = f.plain(Color::Red, 2, HAND_A, &[]);
    let blue = f.plain(Color::Blue, 4, SCORE_A, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, shroud);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [hand, blue]);
    assert_eq!(game.position().cards(SCORE_A), &[blue]);
}

#[test]
fn shroud_of_turin_selects_from_von_brauns_junk_without_a_proof_reveal() {
    // Wernher von Braun makes the junk part of A's score pile, so A may look at it: the
    // color-restricted return is an ordinary private choice and no completeness proof
    // is shown.
    let mut f = Fixture::default();
    let shroud = top(&mut f, "A035", A);
    f.top_figure(Figure::WernherVonBraun, A);
    let occupied = [
        Card::from_code("A035").unwrap().color,
        Card::from_code(Figure::WernherVonBraun.code())
            .unwrap()
            .color,
    ];
    let color = Color::ALL
        .into_iter()
        .find(|c| !occupied.contains(c))
        .unwrap();
    let other = Color::ALL
        .into_iter()
        .find(|c| !occupied.contains(c) && *c != color)
        .unwrap();
    let hand = f.plain(color, 2, HAND_A, &[]);
    f.plain(color, 3, board(A, color), &[]);
    let matching_junk = f.plain(color, 4, Zone::Junk, &[]);
    let other_junk = f.plain(other, 5, Zone::Junk, &[]);
    let ach = f.plain(other, 1, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS.union(Expansions::FIGURES));
    dogma(&mut game, shroud);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    // The returned junk card is verified like any score return; the other
    // junk card stays unseen.
    assert_eq!(revealed(&events), [hand, matching_junk]);
    assert_eq!(game.position().cards(Zone::Junk), &[other_junk]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[ach]);
}

#[test]
fn delft_pocket_telescope_reveals_the_returned_card_and_both_draws_proving_no_shared_icon() {
    let mut f = Fixture::default();
    let delft = top(&mut f, "A043", A);
    let scored = f.plain(Color::Red, 3, SCORE_A, &[Icon::Authority]);
    let five = f.plain(Color::Green, 5, base_deck(5), &[Icon::Prosperity]);
    let six = f.plain(Color::Blue, 6, base_deck(6), &[Icon::Prosperity]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, delft);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert_eq!(shown[0], scored);
    assert!(shown.contains(&five) && shown.contains(&six), "{shown:?}");
    assert!(game.position().cards(HAND_A).is_empty());
}

#[test]
fn buttonwood_agreement_reveals_the_returned_score_cards_and_the_rest_of_the_pile() {
    let mut f = Fixture::default();
    let buttonwood = top(&mut f, "A061", A);
    let red_scored = f.plain(Color::Red, 5, SCORE_A, &[]);
    let blue_scored = f.plain(Color::Blue, 2, SCORE_A, &[]);
    let drawn = f.plain(Color::Red, 8, base_deck(8), &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, buttonwood);
    let (status, events) =
        play(&mut game, |decision| {
            decision.options.iter().position(|o| {
            matches!(o, Alternative::Colors(mask) if mask & (1 << Color::Red as u8) == 0)
        })
        .or(Some(0))
        });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert_eq!(shown[0], drawn);
    assert!(
        shown.contains(&red_scored) && shown.contains(&blue_scored),
        "{shown:?}"
    );
    assert_eq!(game.position().cards(SCORE_A), &[blue_scored]);
}

#[test]
fn singer_model_27_reveals_the_score_pile_after_tucking_its_color() {
    let mut f = Fixture::default();
    let singer = top(&mut f, "A069", A);
    let tucked = f.plain(Color::Red, 2, HAND_A, &[]);
    f.plain(Color::Red, 1, board(A, Color::Red), &[]);
    let red_scored = f.plain(Color::Red, 3, SCORE_A, &[]);
    let blue_scored = f.plain(Color::Blue, 4, SCORE_A, &[]);
    f.plain(Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, singer);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, tucked).or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(revealed(&events).contains(&blue_scored));
    assert_eq!(game.position().cards(SCORE_A), &[blue_scored]);
    assert_eq!(game.position().cards(board(A, Color::Red))[0], red_scored);
}

#[test]
fn an_impossible_colored_meld_reveals_the_hand() {
    // Molasses Reef Caravel: three non-green draws, no green to meld.
    let mut f = Fixture::default();
    let caravel = top(&mut f, "A040", A);
    f.cards[usize::from(caravel.0)].effects.remove(0);
    let drawn: Vec<_> = [Color::Blue, Color::Red, Color::Yellow]
        .into_iter()
        .map(|color| f.plain(color, 4, base_deck(4), &[]))
        .collect();
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, caravel);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert!(drawn.iter().all(|card| shown.contains(card)), "{shown:?}");

    // Hansen Writing Ball's compel: four non-blue draws, nothing to meld.
    let mut f = Fixture::default();
    let hansen = top(&mut f, "A071", A);
    f.cards[usize::from(hansen.0)].effects.remove(1);
    f.plain(
        Color::Green,
        1,
        board(B, Color::Green),
        &sharing_icons("A071"),
    );
    let drawn: Vec<_> = (0..4)
        .map(|_| f.plain(Color::Red, 7, base_deck(7), &[]))
        .collect();
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, hansen);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert!(drawn.iter().all(|card| shown.contains(card)), "{shown:?}");
    assert_eq!(game.position().cards(HAND_A).len(), 4);

    // Dragon's Lair: no hand card of the melded color, so the hand proves it.
    let mut f = Fixture::default();
    let lair = top(&mut f, "A105", A);
    f.plain(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A105"),
    );
    f.plain(Color::Green, 5, SCORE_B, &[]);
    let red = f.plain(Color::Red, 2, HAND_B, &[]);
    let mut game = f.game(Expansions::ARTIFACTS);
    dogma(&mut game, lair);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(revealed(&events), [red]);
}
