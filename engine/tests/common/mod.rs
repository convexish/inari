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
        self.add(Card::from_code("B004").unwrap(), zone)
    }

    /// Printed B003 and its phase-local shortcut declaration. The second
    /// effect reads this execution's preceding demand, not its last draw.
    pub fn oars(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B003").unwrap(), zone)
    }

    /// B011's first printed effect, isolated for the return-order proof.
    pub fn tools_first(&mut self, zone: Zone) -> CardId {
        let mut card = Card::from_code("B011").unwrap();
        card.effects.truncate(1);
        self.add(card, zone)
    }

    /// Complete printed B011 program. Its second payment is a genuine optional
    /// single-card choice, with no order reduction to apply.
    pub fn tools(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B011").unwrap(), zone)
    }

    /// B088 Ecology, transcribed from data/base.json. Its return and score
    /// are genuine single-card choices; only the whole-deck sweep is reduced.
    pub fn ecology(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B088").unwrap(), zone)
    }

    /// B010 Pottery: the computed age uses successful original returns, not
    /// attempted selections, printed returned ages or reaction movements.
    pub fn pottery(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B010").unwrap(), zone)
    }

    /// B002 from data/base.json; repeat depends on the revealed icon, not
    /// whether the following score survived a replacement.
    pub fn metalworking(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B002").unwrap(), zone)
    }

    /// B012 Writing, including the printed featured icon and spatial symbols.
    pub fn writing(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B012").unwrap(), zone)
    }

    /// Printed B008: draw and meld are separate semantic operations.
    pub fn sailing(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B008").unwrap(), zone)
    }

    /// Printed B042: the draw box changes with the invocation; meld uses its result.
    pub fn experimentation(&mut self, zone: Zone) -> CardId {
        self.add(Card::from_code("B042").unwrap(), zone)
    }

    /// The printed card with this publisher code, from any set.
    pub fn card(&mut self, code: &str, zone: Zone) -> CardId {
        self.add(Card::from_code(code).unwrap(), zone)
    }

    pub fn figure(&mut self, figure: Figure, zone: Zone) -> CardId {
        self.add(Card::from_code(figure.code()).unwrap(), zone)
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

/// A blank Base card of this value.
pub fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.tech(color, value, zone, vec![])
}

pub fn complete(status: Status) {
    assert!(matches!(status, Status::Complete(_)), "{status:?}");
}

/// A full Dogma of the demand by A against B, so the owner's effect reads
/// the demand's results.
pub fn dogma(game: &mut Game, card: CardId) {
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
}

/// Start `card`'s effect `index` for `executor`, as `demander`'s Dogma.
pub fn effect(game: &mut Game, card: CardId, index: u16, executor: Player, demander: Player) {
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

/// Each self-execution keyword earns a Chain draw of an [11].
pub fn elevens(f: &mut Fixture, count: usize) {
    for i in 0..count {
        f.tech(Color::ALL[i % 5], 11, Zone::Deck(Deck::base(11)), vec![]);
    }
}

/// A draw that took a card above its requested age (an empty age was
/// skipped): performed, but short of the requested value.
pub fn escalated(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: true,
    }))
}

pub fn icons(f: &mut Fixture, card: CardId, icons: &[Icon]) {
    for (slot, &icon) in icons.iter().enumerate() {
        f.cards[usize::from(card.0)].symbols[slot] = Symbol::Icon(icon);
    }
}

pub fn meld_action(game: &mut Game, card: CardId) {
    game.take_action(Action::Meld { card }, ActionKind::Regular)
        .unwrap();
}

pub fn performed(card: CardId, value: u8) -> Status {
    Status::Complete(ReturnValue::Operation(OperationOutcome {
        disposition: Disposition::Performed,
        card: Some(card),
        value: Some(value),
        short: false,
    }))
}

pub fn pick_known(menu: &Decision, card: CardId) -> Option<usize> {
    menu.options.iter().position(|option| {
        matches!(
            option,
            Alternative::Select { target: TargetDescriptor::Known { card: c, .. } }
            | Alternative::Move { target: TargetDescriptor::Known { card: c, .. }, .. }
            if *c == card
        )
    })
}

/// Settle, answering each decision with `decide`; a lone option is taken.
pub fn play(
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

pub fn revealed(events: &[Event]) -> Vec<CardId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

/// Play to the end, preferring these cards and otherwise the first option.
pub fn run(game: &mut Game, events: &mut Vec<Event>, preferred: &[CardId]) -> Status {
    for _ in 0..300 {
        match settle(game, events) {
            Status::Decision(menu) => {
                let index = preferred
                    .iter()
                    .find_map(|&card| pick_known(&menu, card))
                    .unwrap_or(0);
                game.choose(index).unwrap();
            }
            status => return status,
        }
    }
    panic!("did not terminate");
}
