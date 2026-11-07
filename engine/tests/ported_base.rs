use inno_engine::*;

mod common;
use common::*;

/// Place a printed card on its own color's pile.
fn printed(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = base_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

fn icons(f: &mut Fixture, card: CardId, icon: Icon, count: usize) {
    let symbols = &mut f.cards[usize::from(card.0)].symbols;
    *symbols = [Symbol::Blank; SLOTS];
    symbols[..count].fill(Symbol::Icon(icon));
}

fn effect(game: &mut Game, card: CardId, index: u16, executor: Player, demander: Player) {
    let mut context = Context::new(executor);
    context.demander = demander;
    context.source_owner = demander;
    game.start(
        Operation::ExecuteEffect {
            card,
            effect: index,
            boxed: BoxedValues::default(),
        },
        context,
    )
    .unwrap();
}

fn finish(game: &mut Game, events: &mut Vec<Event>) -> Status {
    for _ in 0..100 {
        match settle(game, events) {
            Status::Decision(_) => game.choose(0).unwrap(),
            status => return status,
        }
    }
    panic!("did not terminate");
}

fn choose(game: &mut Game, menu: &Decision, option: &Alternative) {
    let index = menu
        .options
        .iter()
        .position(|candidate| candidate == option)
        .unwrap_or_else(|| panic!("{option:?} not offered: {:?}", menu.options));
    game.choose(index).unwrap();
}

fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
            | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
            if *c == card
        )
    })
}

const ECHOES_ONE: Deck = Deck {
    set: CardSet::Echoes,
    age: 1,
};

/// B079: after a successful demand, Skyscrapers moves from the demander's board, where
/// it tops, to the demander's hand.
#[test]
fn skyscrapers_takes_itself_from_the_demanders_board_after_a_successful_demand() {
    let mut f = Fixture::default();
    let skyscrapers = printed(&mut f, "B079", A);
    let bottom = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let below = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
    let efficient = f.tech(Color::Blue, 8, Zone::Board(B, Color::Blue), vec![]);
    icons(&mut f, efficient, Icon::Efficiency, 1);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, skyscrapers, 0, B, A);
    assert_eq!(
        finish(&mut game, &mut Vec::new()),
        Status::Complete(ReturnValue::Unit)
    );
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Blue)), [efficient]);
    assert_eq!(position.cards(Zone::Score(B)), [below]);
    assert_eq!(position.cards(Zone::Deck(Deck::base(1))), [bottom]);
    assert!(position.cards(Zone::Board(B, Color::Blue)).is_empty());
    assert_eq!(position.cards(Zone::Hand(A)), [skyscrapers]);
    assert!(position.cards(Zone::Board(A, Color::Yellow)).is_empty());
}

/// B079 (old base_tests): with no qualifying top the demand transfers
/// nothing, so nothing is scored or returned and Skyscrapers stays put.
#[test]
fn skyscrapers_stays_on_the_board_when_the_demand_transfers_nothing() {
    let mut f = Fixture::default();
    let skyscrapers = printed(&mut f, "B079", A);
    let bottom = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
    let top = f.tech(Color::Blue, 8, Zone::Board(B, Color::Blue), vec![]);
    let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
    effect(&mut game, skyscrapers, 0, B, A);
    let mut events = Vec::new();
    assert_eq!(
        finish(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    assert!(events.is_empty());
    let position = game.position();
    assert_eq!(position.cards(Zone::Board(A, Color::Yellow)), [skyscrapers]);
    assert_eq!(position.cards(Zone::Board(B, Color::Blue)), [bottom, top]);
    assert!(position.cards(Zone::Hand(A)).is_empty());
}

/// B079: "return all cards of that color" lets the victim order same-deck returns, a
/// covered card first included.
#[test]
fn skyscrapers_returns_same_deck_cards_in_the_victims_chosen_order() {
    for first_bottom in [true, false] {
        let mut f = Fixture::default();
        let skyscrapers = printed(&mut f, "B079", A);
        let bottom = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
        let above = f.tech(Color::Blue, 1, Zone::Board(B, Color::Blue), vec![]);
        let scored = f.tech(Color::Blue, 3, Zone::Board(B, Color::Blue), vec![]);
        let efficient = f.tech(Color::Blue, 8, Zone::Board(B, Color::Blue), vec![]);
        icons(&mut f, efficient, Icon::Efficiency, 1);
        let position = f.position().with_splay(B, Color::Blue, Splay::Up).unwrap();
        let mut game = Game::new(position, Expansions::BASE).unwrap();
        effect(&mut game, skyscrapers, 0, B, A);
        let mut events = Vec::new();
        let (first, second) = if first_bottom {
            (bottom, above)
        } else {
            (above, bottom)
        };
        choose_card(&mut game, &mut events, first);
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        let position = game.position();
        assert_eq!(position.cards(Zone::Score(B)), [scored]);
        // Returns go to the bottom of the deck: the second one lies below.
        assert_eq!(position.cards(Zone::Deck(Deck::base(1))), [second, first]);
        assert!(position.cards(Zone::Board(B, Color::Blue)).is_empty());
        assert_eq!(position.splay(B, Color::Blue), Splay::None);
        assert_eq!(position.cards(Zone::Hand(A)), [skyscrapers]);
    }
}

/// Special achievements are cards: a sharer junking or restoring one with Publications
/// (B073) does something, so the activating player gets the free Draw. Declining earns
/// nothing.
#[test]
fn a_sharers_publications_special_toggle_earns_the_share_draw() {
    let junk = Alternative::JunkSpecial(Special::Universe);
    let restore = Alternative::RestoreSpecial(Special::Universe);
    for sharer_choice in [junk.clone(), restore.clone(), Alternative::Decline] {
        let mut f = Fixture::default();
        let publications = printed(&mut f, "B073", A);
        let sharer = f.tech(Color::Red, 1, Zone::Board(B, Color::Red), vec![]);
        icons(&mut f, sharer, Icon::Concept, 2);
        let reward = f.tech(Color::Green, 7, Zone::Deck(Deck::base(7)), vec![]);
        let mut game = Game::new(f.position(), Expansions::BASE).unwrap();
        let mut events = Vec::new();
        if sharer_choice == restore {
            game.start(
                Operation::JunkAvailableSpecial {
                    special: Special::Universe,
                },
                Context::new(A),
            )
            .unwrap();
            settle(&mut game, &mut events);
        }
        game.take_action(
            Action::Dogma {
                card: publications,
                auspice: false,
            },
            ActionKind::Regular,
        )
        .unwrap();
        let mut sharer_chose = false;
        for _ in 0..100 {
            match settle(&mut game, &mut events) {
                Status::Decision(menu) => {
                    let option = if menu.player == B {
                        sharer_chose = true;
                        &sharer_choice
                    } else {
                        &Alternative::Decline
                    };
                    choose(&mut game, &menu, option);
                }
                status => {
                    assert_eq!(status, Status::Complete(ReturnValue::Unit));
                    break;
                }
            }
        }
        assert!(sharer_chose);
        let drew = sharer_choice != Alternative::Decline;
        assert_eq!(
            game.position().cards(Zone::Hand(A)).contains(&reward),
            drew,
            "{sharer_choice:?}"
        );
    }
}

/// Sanitation (B069): the victim can't see the demander's hand, but a lowest tie across
/// sets shows different backs, so the victim names the set.
#[test]
fn sanitation_victim_names_the_set_of_a_mixed_lowest_hand_tie() {
    for take_echoes in [false, true] {
        let mut f = Fixture::default();
        let sanitation = f.add(base_card("B069").unwrap(), Zone::Junk);
        let given = f.tech(Color::Red, 5, Zone::Hand(B), vec![]);
        let base_low = f.tech(Color::Blue, 1, Zone::Hand(A), vec![]);
        let echoes_low = f.add(echoes_card("E001").unwrap(), Zone::Hand(A));
        let higher = f.tech(Color::Green, 2, Zone::Hand(A), vec![]);
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        effect(&mut game, sanitation, 0, B, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        assert_eq!(menu.player, B);
        let backs: Vec<_> = menu
            .options
            .iter()
            .map(|option| match option {
                Alternative::BlindSelect {
                    source: Zone::Hand(A),
                    back,
                } => *back,
                other => panic!("only public backs are offered: {other:?}"),
            })
            .collect();
        assert_eq!(backs.len(), 2);
        assert!(backs.contains(&Deck::base(1)) && backs.contains(&ECHOES_ONE));
        let (back, taken, kept) = if take_echoes {
            (ECHOES_ONE, echoes_low, base_low)
        } else {
            (Deck::base(1), base_low, echoes_low)
        };
        choose(
            &mut game,
            &menu,
            &Alternative::BlindSelect {
                source: Zone::Hand(A),
                back,
            },
        );
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().cards(Zone::Hand(B)), [taken]);
        let mut hand = game.position().cards(Zone::Hand(A)).to_vec();
        hand.sort_by_key(|card| card.0);
        let mut expected = vec![kept, higher, given];
        expected.sort_by_key(|card| card.0);
        assert_eq!(hand, expected);
    }
}

/// Rocketry (B083) returns from a face-down score pile by public back, so same-value
/// cards of different sets are a choice of set.
#[test]
fn rocketry_names_the_set_of_a_mixed_set_score_back() {
    for take_echoes in [false, true] {
        let mut f = Fixture::default();
        let rocketry = printed(&mut f, "B083", A);
        let base_one = f.tech(Color::Red, 1, Zone::Score(B), vec![]);
        let echoes_one = f.add(echoes_card("E001").unwrap(), Zone::Score(B));
        let mut game = Game::new(f.position(), Expansions::ECHOES).unwrap();
        effect(&mut game, rocketry, 0, A, A);
        let mut events = Vec::new();
        let menu = decision(&mut game, &mut events);
        let backs: Vec<_> = menu
            .options
            .iter()
            .map(|option| match option {
                Alternative::BlindMove {
                    verb: Verb::Return,
                    source: Zone::Score(B),
                    back,
                } => *back,
                other => panic!("only public backs are offered: {other:?}"),
            })
            .collect();
        assert_eq!(backs.len(), 2);
        assert!(backs.contains(&Deck::base(1)) && backs.contains(&ECHOES_ONE));
        let (back, taken, kept) = if take_echoes {
            (ECHOES_ONE, echoes_one, base_one)
        } else {
            (Deck::base(1), base_one, echoes_one)
        };
        choose(
            &mut game,
            &menu,
            &Alternative::BlindMove {
                verb: Verb::Return,
                source: Zone::Score(B),
                back,
            },
        );
        assert_eq!(
            finish(&mut game, &mut events),
            Status::Complete(ReturnValue::Unit)
        );
        assert_eq!(game.position().cards(Zone::Score(B)), [kept]);
        assert_eq!(game.position().cards(Zone::Deck(back)), [taken]);
    }
}

/// Rocketry names a back, not a card: which same-back card leaves the face-down pile is
/// random, never fixed by scoring order.
#[test]
fn rocketrys_pick_within_one_back_is_random_across_seeds() {
    let mut returned = Vec::new();
    for seed in 0..40 {
        let mut f = Fixture::default();
        let rocketry = printed(&mut f, "B083", A);
        let older = f.tech(Color::Red, 1, Zone::Score(B), vec![]);
        let newer = f.tech(Color::Green, 1, Zone::Score(B), vec![]);
        let mut game = Game::seeded(f.position(), Expansions::BASE, seed).unwrap();
        effect(&mut game, rocketry, 0, A, A);
        assert_eq!(
            finish(&mut game, &mut Vec::new()),
            Status::Complete(ReturnValue::Unit)
        );
        let deck = game.position().cards(Zone::Deck(Deck::base(1)));
        assert_eq!(deck.len(), 1);
        assert!([older, newer].contains(&deck[0]));
        if !returned.contains(&deck[0]) {
            returned.push(deck[0]);
        }
    }
    assert_eq!(returned.len(), 2, "both same-back cards must be possible");
}

/// Play a blackmailed card's effects for B: a menu moving cards with `verb`
/// must offer no decline while `done` is false, then stopping is taken.
fn blackmailed_moves_at_least_one(
    game: &mut Game,
    card: CardId,
    verb: Verb,
    done: impl Fn(&Position) -> bool,
) {
    let mut events = Vec::new();
    let mut stopped = false;
    for _ in 0..100 {
        match settle(game, &mut events) {
            Status::Decision(menu) => {
                let decline = menu.options.iter().position(|o| *o == Alternative::Decline);
                let moving = menu
                    .options
                    .iter()
                    .position(|o| matches!(o, Alternative::Move { verb: v, .. } if *v == verb));
                let index = match moving {
                    Some(index) if !done(game.position()) => {
                        assert_eq!(decline, None, "the first {verb:?} is required");
                        index
                    }
                    Some(_) => {
                        stopped = true;
                        decline.expect("stopping after one remains")
                    }
                    None => pick_known(&menu, card).unwrap_or(0),
                };
                game.choose(index).unwrap();
            }
            status => {
                assert_eq!(status, Status::Complete(ReturnValue::Unit));
                break;
            }
        }
    }
    assert!(stopped, "a second {verb:?} was offered and declined");
}

/// Blackmail (U040) makes B self-execute `code` under `must`, with two other
/// [Authority] hand cards and one Base card of each `deck` value to draw.
fn blackmailed(code: &str, deck: &[u8]) -> (Game, CardId) {
    let mut f = Fixture::default();
    let blackmail = printed_unseen(&mut f, "U040", A);
    let card = f.add(base_card(code).unwrap(), Zone::Hand(B));
    for (value, color) in [(1, Color::Red), (2, Color::Green)] {
        let id = f.tech(color, value, Zone::Hand(B), vec![]);
        icons(&mut f, id, Icon::Authority, 1);
    }
    for &value in deck {
        f.tech(Color::Purple, value, Zone::Deck(Deck::base(value)), vec![]);
    }
    let mut game = Game::new(f.position(), Expansions::UNSEEN).unwrap();
    effect(&mut game, blackmail, 0, B, A);
    (game, card)
}

fn printed_unseen(f: &mut Fixture, code: &str, player: Player) -> CardId {
    let card = unseen_card(code).unwrap();
    let color = card.color;
    f.add(card, Zone::Board(player, color))
}

fn hand_below_two(position: &Position) -> bool {
    position.cards(Zone::Hand(B)).len() < 2
}

/// A blackmailed Pottery returns at least one card, then may stop.
#[test]
fn pottery_returns_at_least_one_card_under_blackmail() {
    let (mut game, pottery) = blackmailed("B010", &[1, 1]);
    blackmailed_moves_at_least_one(&mut game, pottery, Verb::Return, hand_below_two);
    // One return scores a [1]; the second effect draws a [1].
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 1);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 2);
}

/// The same ruling for Masonry: a blackmailed "may meld any number" melds at least one
/// [Authority] card.
#[test]
fn masonry_melds_at_least_one_card_under_blackmail() {
    let (mut game, masonry) = blackmailed("B006", &[]);
    blackmailed_moves_at_least_one(&mut game, masonry, Verb::Meld, hand_below_two);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
}

/// The same ruling for Democracy: one return beats the opponent's none and scores an
/// [8].
#[test]
fn democracy_returns_at_least_one_card_under_blackmail() {
    let (mut game, democracy) = blackmailed("B064", &[8]);
    blackmailed_moves_at_least_one(&mut game, democracy, Verb::Return, hand_below_two);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 1);
}

/// The same ruling for Lighting: one tucked value scores a [7].
#[test]
fn lighting_tucks_at_least_one_card_under_blackmail() {
    let (mut game, lighting) = blackmailed("B074", &[7]);
    blackmailed_moves_at_least_one(&mut game, lighting, Verb::Tuck, hand_below_two);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 1);
}

/// The same ruling for Antibiotics: one returned value draws two [8].
#[test]
fn antibiotics_returns_at_least_one_card_under_blackmail() {
    let (mut game, antibiotics) = blackmailed("B078", &[8, 8]);
    blackmailed_moves_at_least_one(&mut game, antibiotics, Verb::Return, hand_below_two);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 3);
}

/// The same ruling for Quantum Theory: one return, then stopping, earns no [10].
#[test]
fn quantum_theory_returns_at_least_one_card_under_blackmail() {
    let (mut game, quantum) = blackmailed("B082", &[]);
    blackmailed_moves_at_least_one(&mut game, quantum, Verb::Return, hand_below_two);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
}

/// The same ruling for Suburbia: one tuck scores a [1].
#[test]
fn suburbia_tucks_at_least_one_card_under_blackmail() {
    let (mut game, suburbia) = blackmailed("B089", &[1]);
    blackmailed_moves_at_least_one(&mut game, suburbia, Verb::Tuck, hand_below_two);
    assert_eq!(game.position().cards(Zone::Hand(B)).len(), 1);
    assert_eq!(game.position().cards(Zone::Score(B)).len(), 1);
}
