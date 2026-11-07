#![allow(dead_code)]

use inno_engine::*;

pub fn moved(event: &Event) -> &Movement {
    let Event::Moved(movement) = event else {
        panic!("expected a movement");
    };
    movement
}

pub fn exchanged(event: &Event) -> &[ExchangeMovement] {
    let Event::Exchanged { movements, .. } = event else {
        panic!("expected one exchange");
    };
    movements
}

pub const A: Player = Player::One;
pub const B: Player = Player::Two;

#[derive(Default)]
pub struct Fixture {
    pub cards: Vec<Card>,
    pub placements: Vec<(CardId, Zone)>,
}

impl Fixture {
    pub fn tech(
        &mut self,
        color: Color,
        value: u8,
        zone: Zone,
        instructions: Vec<Instruction>,
    ) -> CardId {
        self.add(
            Card {
                code: "synthetic-base-program",
                set: CardSet::Base,
                color,
                value,
                figure: None,
                echo: None,
                symbols: [Symbol::Blank; SLOTS],
                featured: Some(Icon::Authority),
                effects: vec![Effect {
                    kind: EffectKind::NonDemand,
                    instructions,
                }],
            },
            zone,
        )
    }

    /// Native printed B004 program; placement remains fixture-specific.
    pub fn agriculture(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B004").unwrap(), zone)
    }

    /// Printed B003 and its phase-local shortcut declaration. The second
    /// effect reads this execution's preceding demand, not its last draw.
    pub fn oars(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B003").unwrap(), zone)
    }

    /// B011's first printed effect, isolated for the return-order proof.
    pub fn tools_first(&mut self, zone: Zone) -> CardId {
        let mut card = base_card("B011").unwrap();
        card.effects.truncate(1);
        self.add(card, zone)
    }

    /// Complete printed B011 program. Its second payment is a genuine optional
    /// single-card choice, with no order reduction to apply.
    pub fn tools(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B011").unwrap(), zone)
    }

    /// B088 Ecology, transcribed from data/cards.json. Its return and score
    /// are genuine single-card choices; only the whole-deck sweep is reduced.
    pub fn ecology(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B088").unwrap(), zone)
    }

    /// B010 Pottery: the computed age uses successful original returns, not
    /// attempted selections, printed returned ages or reaction movements.
    pub fn pottery(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B010").unwrap(), zone)
    }

    /// B002 from data/cards.json; repeat depends on the revealed icon, not
    /// whether the following score survived a replacement.
    pub fn metalworking(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B002").unwrap(), zone)
    }

    /// B012 Writing, including the printed featured icon and spatial symbols.
    pub fn writing(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B012").unwrap(), zone)
    }

    /// Printed B008: draw and meld are separate semantic operations.
    pub fn sailing(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B008").unwrap(), zone)
    }

    /// Printed B042: the draw box changes with the invocation; meld uses its result.
    pub fn experimentation(&mut self, zone: Zone) -> CardId {
        self.add(base_card("B042").unwrap(), zone)
    }

    pub fn figure(&mut self, figure: Figure, zone: Zone) -> CardId {
        self.add(figure_card(figure.code()).unwrap(), zone)
    }

    pub fn add(&mut self, card: Card, zone: Zone) -> CardId {
        let id = CardId(self.cards.len() as u16);
        self.cards.push(card);
        self.placements.push((id, zone));
        id
    }

    pub fn position(self) -> Position {
        Position::new(self.cards, &self.placements, A).unwrap()
    }
}

pub fn target(card: CardId, physical: Zone) -> Target {
    Target {
        card,
        physical,
        logical: LogicalZone::Physical(physical),
    }
}

pub fn meld(card: CardId, source: Zone, player: Player, color: Color) -> Operation {
    Operation::Move {
        verb: Verb::Meld,
        target: target(card, source),
        destination: Zone::Board(player, color),
    }
}

pub fn settle(game: &mut Game, events: &mut Vec<Event>) -> Status {
    game.advance(1000, events).unwrap()
}

pub fn decision(game: &mut Game, events: &mut Vec<Event>) -> Decision {
    let Status::Decision(decision) = settle(game, events) else {
        panic!("expected a decision");
    };
    decision
}

pub fn choose_card(game: &mut Game, events: &mut Vec<Event>, card: CardId) {
    let decision = decision(game, events);
    let index = decision.options.iter().position(|option| matches!(option,
        Alternative::Move { target: TargetDescriptor::Known { card: candidate, .. }, .. } if *candidate == card)).unwrap();
    game.choose(index).unwrap();
}

pub fn advance_to_events(game: &mut Game, events: &mut Vec<Event>, count: usize) {
    for _ in 0..1000 {
        if events.len() == count {
            return;
        }
        assert_eq!(game.advance(1, events).unwrap(), Status::WorkLimit);
        assert!(game.position().check_conservation());
    }
    panic!("did not reach the expected physical move");
}

/// A score pile filtered to the chosen value, as Statistics-like cards use it.
#[allow(dead_code)]
pub const CHOSEN_SCORE: Selector = build::filtered(
    &build::SCORE,
    &[Filter::Has(Test::Value(Cmp::Equal, Expr::Chosen))],
);
