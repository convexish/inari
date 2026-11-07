//! Proof reveals: a chosen hidden card is revealed when its front sets a restriction or
//! decides a later operation, and an impossible mandatory instruction or an exhausted
//! sweep reveals what proves it.
use super::*;

const HAND_A: Zone = Zone::Hand(A);
const HAND_B: Zone = Zone::Hand(B);
const SCORE_A: Zone = Zone::Score(A);
const SCORE_B: Zone = Zone::Score(B);

fn base_deck(age: u8) -> Zone {
    Zone::Deck(Deck::base(age))
}

fn board(player: Player, color: Color) -> Zone {
    Zone::Board(player, color)
}

/// A printed Artifact as that player's top of its own color.
fn top(l: &mut Layout, code: &str, player: Player) -> CardId {
    let color = artifacts_card(code).unwrap().color;
    l.artifact(code, Zone::Board(player, color))
}

fn sharing_icons(code: &str) -> [Icon; 3] {
    [artifacts_card(code).unwrap().featured.unwrap(); 3]
}

fn revealed(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

fn any(game: &mut Game) -> (Status, Vec<Event>) {
    play(game, |_| Some(0))
}

#[test]
fn priest_king_reveals_the_scored_card_of_every_executor() {
    let mut l = Layout::default();
    let priest = top(&mut l, "A007", A);
    l.tech(
        Color::Green,
        2,
        board(B, Color::Green),
        &sharing_icons("A007"),
    );
    let b_hand = l.tech(Color::Green, 3, HAND_B, &[]);
    let a_hand = l.tech(Color::Blue, 1, HAND_A, &[]);
    l.tech(Color::Blue, 1, board(A, Color::Blue), &[]);
    l.tech(Color::Yellow, 1, base_deck(1), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let ark = top(&mut l, "A014", A);
    let returned = l.tech(Color::Red, 2, HAND_A, &[]);
    let b_red = l.tech(Color::Red, 1, board(B, Color::Red), &[]);
    let mut game = l.game();
    dogma(&mut game, ark);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [returned]);
    assert_eq!(game.position().cards(SCORE_A), &[b_red]);
}

#[test]
fn papyrus_of_ani_reveals_the_purple_return_or_the_hand_proving_none() {
    let mut l = Layout::default();
    let papyrus = top(&mut l, "A002", A);
    let purple = l.tech(Color::Purple, 1, HAND_A, &[]);
    let other = l.tech(Color::Blue, 1, HAND_A, &[]);
    let three = l.tech(Color::Green, 3, base_deck(3), &[]);
    let mut game = l.game();
    dogma(&mut game, papyrus);
    let (status, events) = play(&mut game, |decision| {
        option_moving(decision, purple).or(Some(0))
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [purple, three]);
    assert!(!revealed(&events).contains(&other));

    let mut l = Layout::default();
    let papyrus = top(&mut l, "A002", A);
    let blue = l.tech(Color::Blue, 1, HAND_A, &[]);
    let mut game = l.game();
    dogma(&mut game, papyrus);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [blue]);
    assert!(moves(&events).is_empty());
}

#[test]
fn terracotta_army_reveals_the_scored_card_or_the_hand_proving_none() {
    let mut l = Layout::default();
    let army = top(&mut l, "A019", A);
    l.cards[usize::from(army.0)].effects.remove(0);
    let plain = l.tech(Color::Blue, 2, HAND_A, &[]);
    let deck = l.tech(Color::Green, 2, base_deck(2), &[]);
    let mut game = l.game();
    dogma(&mut game, army);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [plain]);
    assert_eq!(game.position().cards(Zone::Junk), &[deck]);

    let mut l = Layout::default();
    let army = top(&mut l, "A019", A);
    l.cards[usize::from(army.0)].effects.remove(0);
    let authority = l.tech(Color::Blue, 2, HAND_A, &[Icon::Authority]);
    let mut game = l.game();
    dogma(&mut game, army);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [authority]);
    assert_eq!(moves(&events).len(), 1, "Terracotta tucks itself");
}

#[test]
fn shroud_of_turin_reveals_both_hidden_returns_or_the_score_proving_none() {
    let mut l = Layout::default();
    let shroud = top(&mut l, "A035", A);
    let hand = l.tech(Color::Red, 2, HAND_A, &[]);
    l.tech(Color::Red, 3, board(A, Color::Red), &[]);
    let scored = l.tech(Color::Red, 4, SCORE_A, &[]);
    let ach = l.tech(Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
    dogma(&mut game, shroud);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(revealed(&events), [hand, scored]);
    assert_eq!(game.position().cards(Zone::Achievements(A)), &[ach]);

    let mut l = Layout::default();
    let shroud = top(&mut l, "A035", A);
    let hand = l.tech(Color::Red, 2, HAND_A, &[]);
    let blue = l.tech(Color::Blue, 4, SCORE_A, &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let shroud = top(&mut l, "A035", A);
    l.figure(Figure::WernherVonBraun, A);
    let occupied = [
        artifacts_card("A035").unwrap().color,
        figure_card(Figure::WernherVonBraun.code()).unwrap().color,
    ];
    let color = Color::ALL
        .into_iter()
        .find(|c| !occupied.contains(c))
        .unwrap();
    let other = Color::ALL
        .into_iter()
        .find(|c| !occupied.contains(c) && *c != color)
        .unwrap();
    let hand = l.tech(color, 2, HAND_A, &[]);
    l.tech(color, 3, board(A, color), &[]);
    let matching_junk = l.tech(color, 4, Zone::Junk, &[]);
    let other_junk = l.tech(other, 5, Zone::Junk, &[]);
    let ach = l.tech(other, 1, Zone::AvailableAchievements, &[]);
    let mut game = l.game_with(Expansions::ARTIFACTS.union(Expansions::FIGURES));
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
    let mut l = Layout::default();
    let delft = top(&mut l, "A043", A);
    let scored = l.tech(Color::Red, 3, SCORE_A, &[Icon::Authority]);
    let five = l.tech(Color::Green, 5, base_deck(5), &[Icon::Prosperity]);
    let six = l.tech(Color::Blue, 6, base_deck(6), &[Icon::Prosperity]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let buttonwood = top(&mut l, "A061", A);
    let red_scored = l.tech(Color::Red, 5, SCORE_A, &[]);
    let blue_scored = l.tech(Color::Blue, 2, SCORE_A, &[]);
    let drawn = l.tech(Color::Red, 8, base_deck(8), &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let singer = top(&mut l, "A069", A);
    let tucked = l.tech(Color::Red, 2, HAND_A, &[]);
    l.tech(Color::Red, 1, board(A, Color::Red), &[]);
    let red_scored = l.tech(Color::Red, 3, SCORE_A, &[]);
    let blue_scored = l.tech(Color::Blue, 4, SCORE_A, &[]);
    l.tech(Color::Green, 1, Zone::AvailableAchievements, &[]);
    let mut game = l.game();
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
    let mut l = Layout::default();
    let caravel = top(&mut l, "A040", A);
    l.cards[usize::from(caravel.0)].effects.remove(0);
    let drawn: Vec<_> = [Color::Blue, Color::Red, Color::Yellow]
        .into_iter()
        .map(|color| l.tech(color, 4, base_deck(4), &[]))
        .collect();
    let mut game = l.game();
    dogma(&mut game, caravel);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert!(drawn.iter().all(|card| shown.contains(card)), "{shown:?}");

    // Hansen Writing Ball's compel: four non-blue draws, nothing to meld.
    let mut l = Layout::default();
    let hansen = top(&mut l, "A071", A);
    l.cards[usize::from(hansen.0)].effects.remove(1);
    l.tech(
        Color::Green,
        1,
        board(B, Color::Green),
        &sharing_icons("A071"),
    );
    let drawn: Vec<_> = (0..4)
        .map(|_| l.tech(Color::Red, 7, base_deck(7), &[]))
        .collect();
    let mut game = l.game();
    dogma(&mut game, hansen);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let shown = revealed(&events);
    assert!(drawn.iter().all(|card| shown.contains(card)), "{shown:?}");
    assert_eq!(game.position().cards(HAND_A).len(), 4);

    // Dragon's Lair: no hand card of the melded color, so the hand proves it.
    let mut l = Layout::default();
    let lair = top(&mut l, "A105", A);
    l.tech(
        Color::Blue,
        1,
        board(B, Color::Blue),
        &sharing_icons("A105"),
    );
    l.tech(Color::Green, 5, SCORE_B, &[]);
    let red = l.tech(Color::Red, 2, HAND_B, &[]);
    let mut game = l.game();
    dogma(&mut game, lair);
    let (status, events) = any(&mut game);
    assert_eq!(status, Status::Winner(A));
    assert_eq!(revealed(&events), [red]);
}
