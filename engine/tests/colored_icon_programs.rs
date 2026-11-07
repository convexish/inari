use inno_engine::*;
mod common;
use common::*;

fn base(f: &mut Fixture, code: &str, zone: Zone) -> CardId {
    f.add(base_card(code).unwrap(), zone)
}
fn invoke(g: &mut Game, card: CardId, effect: u16) {
    g.start(
        Operation::ExecuteEffect {
            card,
            effect,
            boxed: BoxedValues::default(),
        },
        Context::new(A),
    )
    .unwrap();
}
fn finish(g: &mut Game, events: &mut Vec<Event>) {
    for _ in 0..100 {
        match settle(g, events) {
            Status::Decision(menu) => {
                assert!(!menu.options.contains(&Alternative::Decline));
                g.choose(0).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                assert!(g.position().check_conservation());
                return;
            }
        }
    }
    panic!("unfinished colored-icon program");
}
fn decline(g: &mut Game, events: &mut Vec<Event>) {
    let menu = decision(g, events);
    g.choose(
        menu.options
            .iter()
            .position(|o| *o == Alternative::Decline)
            .unwrap(),
    )
    .unwrap();
}

fn fermenting_draws<const F: bool>() {
    for splay in [Splay::None, Splay::Left, Splay::Right, Splay::Up] {
        let mut f = Fixture::default();
        let source = base(&mut f, "B019", Zone::Board(A, Color::Yellow));
        base(&mut f, "B008", Zone::Board(A, Color::Green));
        base(&mut f, "B021", Zone::Board(A, Color::Green));
        base(&mut f, "B010", Zone::Board(B, Color::Blue));
        let supply: Vec<_> = ["B016", "B020", "B018"]
            .into_iter()
            .map(|code| base(&mut f, code, Zone::Deck(Deck::base(2))))
            .collect();
        let mut g = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        if splay != Splay::None {
            g.start(
                Operation::Splay {
                    player: A,
                    color: Color::Green,
                    direction: splay,
                },
                Context::new(A),
            )
            .unwrap();
            settle(&mut g, &mut Vec::new());
        }
        invoke(&mut g, source, 0);
        finish(&mut g, &mut Vec::new());
        let expected = if matches!(splay, Splay::Left | Splay::Up) {
            2
        } else {
            1
        };
        assert_eq!(g.position().cards(Zone::Hand(A)).len(), expected);
        assert_eq!(
            g.position().cards(Zone::Deck(Deck::base(2))).len(),
            supply.len() - expected
        );
    }
    let mut f = Fixture::default();
    let source = base(&mut f, "B019", Zone::Junk);
    let mut g = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut g, source, 0);
    finish(&mut g, &mut Vec::new());
    assert!(g.position().cards(Zone::Hand(A)).is_empty());
}
#[test]
fn fermenting_counts_live_visible_colors_not_icons_or_opponent_colors() {
    fermenting_draws::<false>();
    fermenting_draws::<true>();
}

fn fermenting_payment<const F: bool>() {
    for owner in [A, B] {
        for buried in [false, true] {
            let mut f = Fixture::default();
            let source = base(&mut f, "B019", Zone::Board(owner, Color::Yellow));
            if buried {
                base(&mut f, "B004", Zone::Board(owner, Color::Yellow));
            }
            let green = base(&mut f, "B008", Zone::Hand(A));
            let deck = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
            let spare = base(&mut f, "B028", Zone::Deck(Deck::base(3)));
            let mut g = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
            invoke(&mut g, source, 1);
            let mut paid = g.clone();
            choose_card(&mut paid, &mut Vec::new(), green);
            finish(&mut paid, &mut Vec::new());
            assert_eq!(paid.position().top(A, Color::Green), Some(green));
            assert_eq!(paid.position().cards(Zone::Deck(Deck::base(2))), [deck]);
            assert!(
                paid.position()
                    .cards(Zone::Board(owner, Color::Yellow))
                    .contains(&source)
            );
            decline(&mut g, &mut Vec::new());
            finish(&mut g, &mut Vec::new());
            assert!(g.position().cards(Zone::Junk).contains(&deck));
            assert_eq!(g.position().cards(Zone::Junk).contains(&source), !buried);
            assert_eq!(g.position().cards(Zone::Deck(Deck::base(3))), [spare]);
            assert_eq!(g.position().cards(Zone::Hand(A)), [green]);
        }
    }
}
#[test]
fn fermenting_decline_uses_live_named_top_on_either_board_and_payment_skips_junk() {
    fermenting_payment::<false>();
    fermenting_payment::<true>();
}

#[test]
fn fermenting_archimedes_changes_both_boxes_not_the_color_quota() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B019", Zone::Board(A, Color::Yellow));
    f.figure(Figure::Archimedes, Zone::Board(A, Color::Blue));
    let untouched = base(&mut f, "B021", Zone::Deck(Deck::base(2)));
    let junked = base(&mut f, "B028", Zone::Deck(Deck::base(3)));
    let drawn = base(&mut f, "B026", Zone::Deck(Deck::base(3)));
    let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut g, source, 0);
    finish(&mut g, &mut Vec::new());
    assert_eq!(g.position().cards(Zone::Hand(A)), [drawn]);
    invoke(&mut g, source, 1);
    finish(&mut g, &mut Vec::new());
    assert!(g.position().cards(Zone::Junk).contains(&junked));
    assert_eq!(g.position().cards(Zone::Deck(Deck::base(2))), [untouched]);
}

fn perspective<const F: bool>() {
    for on_board in [false, true] {
        let mut f = Fixture::default();
        let source = base(
            &mut f,
            "B039",
            if on_board {
                Zone::Board(A, Color::Yellow)
            } else {
                Zone::Junk
            },
        );
        if on_board {
            base(&mut f, "B012", Zone::Board(A, Color::Blue));
        }
        base(&mut f, "B001", Zone::Board(B, Color::Red));
        let payment = base(&mut f, "B004", Zone::Hand(A));
        let score = base(&mut f, "B008", Zone::Hand(A));
        let mut g = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
        invoke(&mut g, source, 0);
        let mut declined = g.clone();
        decline(&mut declined, &mut Vec::new());
        finish(&mut declined, &mut Vec::new());
        assert_eq!(declined.position().cards(Zone::Hand(A)), [payment, score]);
        choose_card(&mut g, &mut Vec::new(), payment);
        finish(&mut g, &mut Vec::new());
        assert_eq!(g.position().cards(Zone::Deck(Deck::base(1))), [payment]);
        assert_eq!(
            g.position().cards(Zone::Score(A)),
            if on_board { vec![score] } else { vec![] }
        );
    }
}
#[test]
fn perspective_optional_original_return_then_mandatory_as_possible_snapshot() {
    perspective::<false>();
    perspective::<true>();
}

#[test]
fn perspective_homer_replacement_does_not_pay_with_a_reaction_child() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B039", Zone::Board(A, Color::Yellow));
    f.figure(Figure::Homer, Zone::Board(A, Color::Purple));
    let payment = f.figure(Figure::Gilgamesh, Zone::Hand(A));
    let stays = base(&mut f, "B004", Zone::Hand(A));
    let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut g, source, 0);
    choose_card(&mut g, &mut Vec::new(), payment);
    finish(&mut g, &mut Vec::new());
    assert_eq!(g.position().cards(Zone::Hand(A)), [stays]);
    assert!(g.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(g.position().top(A, Color::Red), Some(payment));
}

fn rocketry<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B083", Zone::Board(A, Color::Blue));
    base(&mut f, "B075", Zone::Board(A, Color::Purple));
    let first = base(&mut f, "B001", Zone::Score(B));
    let second = base(&mut f, "B002", Zone::Score(B));
    let third = base(&mut f, "B004", Zone::Score(B));
    let mut g = Game::seeded(f.position(), Expansions::with_figures(F), 37).unwrap();
    invoke(&mut g, source, 0);
    // A single public back group autoexecutes despite three hidden identities.
    assert_eq!(
        settle(&mut g, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    assert_eq!(g.position().cards(Zone::Score(B)).len(), 1);
    let returned = g.position().cards(Zone::Deck(Deck::base(1)));
    assert_eq!(returned.len(), 2);
    assert!(
        returned
            .iter()
            .all(|card| [first, second, third].contains(card))
    );
}
#[test]
fn rocketry_samples_hidden_score_backs_and_counts_colors_not_symbols() {
    rocketry::<false>();
    rocketry::<true>();
}

/// Catherine's added Efficiency is not on the board: only Rocketry's own blue pile
/// shows Efficiency.
#[test]
fn rocketry_counts_no_color_from_catherines_additions() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B083", Zone::Board(A, Color::Blue));
    f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    base(&mut f, "B001", Zone::Board(A, Color::Red));
    for code in ["B002", "B004", "B008", "B010"] {
        base(&mut f, code, Zone::Score(B));
    }
    let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut g, source, 0);
    finish(&mut g, &mut Vec::new());
    assert_eq!(g.position().cards(Zone::Score(B)).len(), 3);
    assert_eq!(g.position().cards(Zone::Deck(Deck::base(1))).len(), 1);
}

/// Neither Catherine's nor Tran's or Maxwell's additions give a color
/// Efficiency; only Maxwell's printed icon does.
#[test]
fn each_additions_give_rocketry_no_colors() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B083", Zone::Junk);
    f.figure(Figure::CatherineTheGreat, Zone::Board(A, Color::Purple));
    f.figure(Figure::TranHuangDao, Zone::Board(A, Color::Red));
    f.figure(Figure::JamesClerkMaxwell, Zone::Board(A, Color::Blue));
    base(&mut f, "B008", Zone::Hand(A));
    base(&mut f, "B080", Zone::Deck(Deck::base(8)));
    base(&mut f, "B081", Zone::Deck(Deck::base(8)));
    for code in ["B001", "B002", "B004"] {
        base(&mut f, code, Zone::Score(B));
    }
    let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut g, source, 0);
    finish(&mut g, &mut Vec::new());
    assert_eq!(g.position().cards(Zone::Score(B)).len(), 2);
}

#[test]
fn perspective_replaced_scores_consume_attempts_without_recounting_or_child_credit() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B039", Zone::Board(A, Color::Yellow));
    base(&mut f, "B012", Zone::Board(A, Color::Blue));
    base(&mut f, "B002", Zone::Board(A, Color::Red));
    base(&mut f, "B016", Zone::Board(A, Color::Red));
    base(&mut f, "B021", Zone::Board(A, Color::Green));
    f.figure(Figure::ShenKuo, Zone::Board(A, Color::Green));
    let payment = base(&mut f, "B004", Zone::Hand(A));
    let red = base(&mut f, "B001", Zone::Hand(A));
    let green = base(&mut f, "B008", Zone::Hand(A));
    let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
    invoke(&mut g, source, 0);
    choose_card(&mut g, &mut Vec::new(), payment);
    finish(&mut g, &mut Vec::new());
    assert_eq!(g.position().cards(Zone::Hand(A)), [red, green]);
    assert!(g.position().cards(Zone::Score(A)).is_empty());
    assert_eq!(g.position().splay(A, Color::Red), Splay::Right);
    assert_eq!(g.position().splay(A, Color::Green), Splay::Right);
}

#[test]
fn rocketry_respects_opposing_logical_score_membership_and_zero_own_colors() {
    for on_board in [false, true] {
        let mut f = Fixture::default();
        let source = base(
            &mut f,
            "B083",
            if on_board {
                Zone::Board(A, Color::Blue)
            } else {
                Zone::Junk
            },
        );
        f.figure(Figure::Michelangelo, Zone::Board(B, Color::Yellow));
        base(&mut f, "B075", Zone::Board(B, Color::Purple));
        let hand_score = base(&mut f, "B001", Zone::Hand(B));
        let mut g = Game::new(f.position(), Expansions::FIGURES).unwrap();
        invoke(&mut g, source, 0);
        finish(&mut g, &mut Vec::new());
        assert_eq!(
            g.position().cards(Zone::Hand(B)).contains(&hand_score),
            !on_board
        );
        assert_eq!(
            g.position()
                .cards(Zone::Deck(Deck::base(1)))
                .contains(&hand_score),
            on_board
        );
    }
}

fn rocketry_zero<const F: bool>() {
    let mut f = Fixture::default();
    let source = base(&mut f, "B083", Zone::Junk);
    base(&mut f, "B075", Zone::Board(B, Color::Purple));
    let stays = base(&mut f, "B001", Zone::Score(B));
    let mut g = Game::new(f.position(), Expansions::with_figures(F)).unwrap();
    invoke(&mut g, source, 0);
    finish(&mut g, &mut Vec::new());
    assert_eq!(g.position().cards(Zone::Score(B)), [stays]);
    assert!(g.position().cards(Zone::Deck(Deck::base(1))).is_empty());
}

#[test]
fn rocketry_zero_colors_does_not_borrow_opponents_efficiency() {
    rocketry_zero::<false>();
    rocketry_zero::<true>();
}
