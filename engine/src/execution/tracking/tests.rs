use super::*;

pub(super) fn pause(game: &mut Game) {
    // A harmless real choice suspends a parent operation without changing its
    // operands. White-box fixture, not a production card or rules shortcut.
    game.frames.push(Frame::Choose {
        player: Player::One,
        options: DecisionOptions::new(
            vec![Alternative::Colors(1), Alternative::Colors(2)],
            Context::new(Player::One),
        ),
        tasks: [Color::Red, Color::Blue]
            .into_iter()
            .map(|color| {
                Task::Operation(
                    Operation::Splay {
                        player: Player::One,
                        color,
                        direction: Splay::Left,
                    },
                    Context::new(Player::One),
                )
            })
            .collect(),
    });
}

#[test]
fn known_meld_reference_survives_shuffle_without_pinning_its_deck_position() {
    let cards = vec![base_card("B001").unwrap(), base_card("B002").unwrap()];
    let color = cards[0].color;
    let hand = Zone::Hand(Player::One);
    let deck = Zone::Deck(Deck::base(1));
    let position =
        Position::new(cards, &[(CardId(0), hand), (CardId(1), deck)], Player::One).unwrap();
    let mut game = Game::new(position, Expansions::FIGURES)
        .unwrap()
        .with_tracking()
        .unwrap();
    let target = Target {
        card: CardId(0),
        physical: hand,
        logical: LogicalZone::Hand(Player::One),
    };
    game.frames.push(Frame::Operation {
        request: Operation::Move {
            verb: Verb::Meld,
            target,
            destination: Zone::Board(Player::One, color),
        },
        context: Context::new(Player::One),
        phase: Phase::Commit,
        knowledge: Some(OperandKnowledge {
            evidence: Evidence::known_to(Player::One),
            conditioned: false,
            private_query: None,
            verify: false,
        }),
        short: false,
    });
    game.commit(
        Operation::Move {
            verb: Verb::Return,
            target,
            destination: deck,
        },
        Context::new(Player::One),
        false,
        &mut Vec::new(),
    )
    .unwrap();
    game.commit(
        Operation::Shuffle {
            deck: Deck::base(1),
        },
        Context::new(Player::One),
        false,
        &mut Vec::new(),
    )
    .unwrap();
    game.refresh_tracking().unwrap();
    pause(&mut game);
    let mut positions = Vec::new();
    for seed in 0..16 {
        let mut world = game.determinize(Player::One, seed).unwrap();
        assert_eq!(
            world.pending_melds(Player::One)[0].card,
            CardView::Known(CardId(0))
        );
        positions.push(
            world
                .position
                .cards(deck)
                .iter()
                .position(|&c| c == CardId(0))
                .unwrap(),
        );
        world.choose(0).unwrap();
        assert!(matches!(
            world.advance(1000, &mut Vec::new()).unwrap(),
            Status::Complete(_)
        ));
        assert_eq!(world.position.top(Player::One, color), Some(CardId(0)));
        assert!(
            world
                .tracker(Player::One)
                .unwrap()
                .check_truth(&world.position)
        );
    }
    positions.sort();
    positions.dedup();
    assert_eq!(positions, [0, 1]);
}

#[test]
fn a_hidden_register_follows_its_pick_without_pinning_identity() {
    let cards = vec![
        base_card("B001").unwrap(),
        base_card("B002").unwrap(),
        base_card("B003").unwrap(),
    ];
    let color = cards[0].color;
    let deck = Zone::Deck(Deck::base(1));
    let position = Position::new(
        cards,
        &[
            (CardId(0), Zone::Board(Player::One, color)),
            (CardId(1), deck),
            (CardId(2), deck),
        ],
        Player::One,
    )
    .unwrap();
    let mut game = Game::new(position, Expansions::BASE)
        .unwrap()
        .with_tracking()
        .unwrap();
    let retained = game.capture(CardId(1));
    game.frames.push(Frame::Program {
        source: Source::Effect {
            card: CardId(0),
            effect: 0,
        },
        pc: 0,
        context: Context::new(Player::One),
        last: HeldReturn::UNIT,
        retained: Some(retained),
        chosen: SelectionParameter::None,
        boxed: BoxedValues::default(),
    });
    pause(&mut game);
    let before = game.clone();
    let mut identities = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let world = game.determinize(Player::One, seed).unwrap();
        let Frame::Program { retained, .. } = &world.frames[0] else {
            panic!("program");
        };
        let retained = retained.unwrap().inner;
        assert_eq!(world.position.locate(retained), Some(deck));
        identities.insert(retained);
    }
    assert_eq!(identities.len(), 2, "either unseen deck card");
    assert_eq!(game, before);
}
