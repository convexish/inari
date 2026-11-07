use inno_engine::*;

mod common;
use common::*;

fn artifacts_with(expansions: Expansions) -> Expansions {
    Expansions::ARTIFACTS.union(expansions)
}

/// A synthetic card printing these icons in its first positions.
fn card(set: CardSet, color: Color, value: u8, icons: &[Icon], effects: Vec<Effect>) -> Card {
    let mut symbols = [Symbol::Blank; SLOTS];
    for (slot, &icon) in symbols.iter_mut().zip(icons) {
        *slot = Symbol::Icon(icon);
    }
    Card {
        code: "synthetic",
        set,
        color,
        value,
        figure: None,
        echo: None,
        symbols,
        featured: icons.first().copied(),
        effects,
    }
}

fn effect(kind: EffectKind, instructions: Vec<Instruction>) -> Effect {
    Effect { kind, instructions }
}

fn non_demand(instructions: Vec<Instruction>) -> Effect {
    effect(EffectKind::NonDemand, instructions)
}

/// A blank Base card.
fn blank(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.add(card(CardSet::Base, color, value, &[], vec![]), zone)
}

/// A blank Base card printing `count` of one icon.
fn icons(f: &mut Fixture, color: Color, icon: Icon, count: usize, zone: Zone) -> CardId {
    f.add(
        card(CardSet::Base, color, 1, &vec![icon; count], vec![]),
        zone,
    )
}

/// A blank synthetic Artifact.
fn artifact(f: &mut Fixture, color: Color, value: u8, zone: Zone) -> CardId {
    f.add(card(CardSet::Artifacts, color, value, &[], vec![]), zone)
}

/// A printed Artifact, Base, Echoes, City or Unseen card.
fn printed(f: &mut Fixture, card: Card, zone: Zone) -> CardId {
    f.add(card, zone)
}

/// A printed card as that player's top of its own color.
fn on_board(f: &mut Fixture, card: Card, player: Player) -> CardId {
    let zone = Zone::Board(player, card.color);
    f.add(card, zone)
}

fn yamato() -> Card {
    artifacts_card("A077").unwrap()
}

fn base_deck(value: u8) -> Zone {
    Zone::Deck(Deck::base(value))
}

fn artifacts_deck(value: u8) -> Zone {
    Zone::Deck(Deck {
        set: CardSet::Artifacts,
        age: value,
    })
}

fn new_game(f: Fixture, expansions: Expansions) -> Game {
    Game::new(f.position(), expansions).unwrap()
}

fn dogma(game: &mut Game, card: CardId) {
    game.take_action(
        Action::Dogma {
            card,
            auspice: false,
        },
        ActionKind::Regular,
    )
    .unwrap();
}

fn meld_action(game: &mut Game, card: CardId) {
    game.take_action(Action::Meld { card }, ActionKind::Regular)
        .unwrap();
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

/// Who drew each card, in order.
fn drawers(events: &[Event]) -> Vec<Player> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(movement)
                if movement.verb.is_none() && matches!(movement.source, Zone::Deck(_)) =>
            {
                Some(movement.executor)
            }
            _ => None,
        })
        .collect()
}

fn revealed(events: &[Event], card: CardId) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::Revealed { card: c, .. } if *c == card))
        .count()
}

/// Whether this option names the card: by identity, or by its public back
/// when it is the only card with that back in its zone.
fn names(option: &Alternative, card: CardId, back: Option<Deck>) -> bool {
    let target = |descriptor: &TargetDescriptor| match *descriptor {
        TargetDescriptor::Known { card: c, .. } => c == card,
        TargetDescriptor::Hidden { back: b, .. } => Some(b) == back,
    };
    match option {
        Alternative::Move { target: t, .. }
        | Alternative::Select { target: t }
        | Alternative::Reveal { target: t } => target(t),
        Alternative::SelfExecute(c) => *c == card,
        Alternative::BlindSelect { back: b, .. } | Alternative::BlindMove { back: b, .. } => {
            Some(*b) == back
        }
        _ => false,
    }
}

fn option_naming(decision: &Decision, card: CardId) -> Option<usize> {
    decision
        .options
        .iter()
        .position(|option| names(option, card, None))
}

// ------------------------------------------------------------------ Digs

/// A Meld action over A's red 3 (Base 3 and an Artifact 3 in their decks
/// unless removed), with B storing an Artifact 3 in a Museum.
fn seizure(display: bool, base_three: bool) -> (Game, CardId, CardId) {
    let mut f = Fixture::default();
    blank(&mut f, Color::Red, 3, Zone::Board(A, Color::Red));
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
    if base_three {
        blank(&mut f, Color::Blue, 3, base_deck(3));
    }
    artifact(&mut f, Color::Green, 3, artifacts_deck(3));
    let stored = artifact(&mut f, Color::Yellow, 3, Zone::Museum(B));
    if display {
        artifact(&mut f, Color::Purple, 2, Zone::Display(A));
    }
    let mut game = new_game(f, Expansions::ARTIFACTS);
    meld_action(&mut game, melded);
    (game, melded, stored)
}

/// Seizing is offered at the original covered value "even when digging would
/// otherwise be ignored": an occupied display, or no Base deck from the
/// covered value up (ARTIFACTS_RULES.md; old `seize_remains_available...`).
#[test]
fn a_seizure_is_offered_when_the_dig_itself_would_be_ignored() {
    for (display, base_three) in [(true, true), (false, false)] {
        for seize in [true, false] {
            let (mut game, melded, stored) = seizure(display, base_three);
            let shown = game.position().cards(Zone::Display(A)).to_vec();
            let mut offers = 0;
            let (status, events) = play(&mut game, |decision| {
                offers += 1;
                assert_eq!(
                    decision.options,
                    [
                        Alternative::Move {
                            verb: Verb::Seize,
                            target: TargetDescriptor::Known {
                                card: stored,
                                source: Zone::Museum(B),
                            },
                        },
                        Alternative::Decline,
                    ]
                );
                Some(usize::from(!seize))
            });
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(offers, 1, "{display} {base_three}");
            let mut expected = vec![(
                Some(Verb::Meld),
                melded,
                Zone::Hand(A),
                Zone::Board(A, Color::Red),
            )];
            if seize {
                expected.push((Some(Verb::Seize), stored, Zone::Museum(B), Zone::Museum(A)));
            }
            // Declining finds nothing to dig either way.
            assert_eq!(moves(&events), expected, "{display} {base_three} {seize}");
            assert_eq!(game.position().cards(Zone::Display(A)), shown.as_slice());
        }
    }
}

/// Battleship Yamato is 11 in a Museum, so it is seizable by a dig of
/// covered value 11 and not by one of its age-8 back (old
/// `seize_uses_museum_yamato_value_eleven_instead_of_its_back`).
#[test]
fn a_museum_yamato_is_seizable_at_its_face_value_eleven() {
    for covered in [11, 8] {
        let mut f = Fixture::default();
        blank(&mut f, Color::Red, covered, Zone::Board(A, Color::Red));
        let melded = blank(&mut f, Color::Red, 1, Zone::Hand(A));
        blank(&mut f, Color::Blue, covered, base_deck(covered));
        let dug = artifact(&mut f, Color::Green, covered, artifacts_deck(covered));
        let yamato = printed(&mut f, yamato(), Zone::Museum(B));
        let mut game = new_game(f, Expansions::ARTIFACTS);
        meld_action(&mut game, melded);
        let (status, events) = play(&mut game, |decision| option_naming(decision, yamato));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        if covered == 11 {
            assert_eq!(
                moves(&events)[1],
                (Some(Verb::Seize), yamato, Zone::Museum(B), Zone::Museum(A))
            );
            assert_eq!(game.position().value(yamato), 11);
            assert!(game.position().cards(Zone::Display(A)).is_empty());
        } else {
            // No offer: the dig proceeds.
            assert_eq!(
                moves(&events)[1],
                (Some(Verb::Dig), dug, artifacts_deck(8), Zone::Display(A))
            );
            assert_eq!(game.position().cards(Zone::Museum(B)), [yamato]);
        }
    }
}

/// The p9 trigger order: the City's icon (Alexandria's Plus draws a 3), then the dig at
/// the covered value, then Echoes promotion, whose meld covers the City.
#[test]
fn a_city_icon_precedes_the_dig_and_the_dig_precedes_promotion() {
    let mut f = Fixture::default();
    blank(&mut f, Color::Blue, 3, Zone::Board(A, Color::Blue));
    let city = printed(&mut f, city_card("C022").unwrap(), Zone::Hand(A));
    let promoted = blank(&mut f, Color::Blue, 1, Zone::Forecast(A));
    blank(&mut f, Color::Yellow, 3, base_deck(3));
    let plus = blank(&mut f, Color::Yellow, 3, base_deck(3));
    let dug = artifact(&mut f, Color::Green, 3, artifacts_deck(3));
    let mut game = new_game(
        f,
        artifacts_with(Expansions::CITIES.union(Expansions::ECHOES)),
    );
    meld_action(&mut game, city);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    let at = |card: CardId| moved.iter().position(|m| m.1 == card).unwrap();
    assert_eq!(moved[at(plus)].2, base_deck(3));
    assert_eq!(
        moved[at(dug)],
        (Some(Verb::Dig), dug, artifacts_deck(3), Zone::Display(A))
    );
    assert_eq!(
        moved[at(promoted)],
        (
            Some(Verb::Meld),
            promoted,
            Zone::Forecast(A),
            Zone::Board(A, Color::Blue)
        )
    );
    assert!(at(city) < at(plus) && at(plus) < at(dug) && at(dug) < at(promoted));
    assert_eq!(game.position().top(A, Color::Blue), Some(promoted));
}

/// Only the Meld action's own meld digs: the promotion meld over a higher
/// card neither digs nor offers a seizure (ARTIFACTS_RULES.md "A Meld
/// **action** triggers a dig").
#[test]
fn a_promotion_meld_does_not_dig() {
    let mut f = Fixture::default();
    let melded = blank(&mut f, Color::Blue, 2, Zone::Hand(A));
    let promoted = blank(&mut f, Color::Blue, 1, Zone::Forecast(A));
    blank(&mut f, Color::Yellow, 2, base_deck(2));
    artifact(&mut f, Color::Green, 2, artifacts_deck(2));
    let stored = artifact(&mut f, Color::Yellow, 2, Zone::Museum(B));
    let mut game = new_game(f, artifacts_with(Expansions::ECHOES));
    meld_action(&mut game, melded);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (
                Some(Verb::Meld),
                melded,
                Zone::Hand(A),
                Zone::Board(A, Color::Blue)
            ),
            (
                Some(Verb::Meld),
                promoted,
                Zone::Forecast(A),
                Zone::Board(A, Color::Blue)
            ),
        ]
    );
    assert!(game.position().cards(Zone::Display(A)).is_empty());
    assert_eq!(game.position().cards(Zone::Museum(B)), [stored]);
}

// -------------------------------------------------------------- Compels

/// A card with a demand, a compel and a non-demand effect, each drawing a 1;
/// B prints enough of its icon to share (and so to be compelled).
fn three_kinds() -> (Fixture, CardId) {
    let mut f = Fixture::default();
    let source = on_board(
        &mut f,
        card(
            CardSet::Artifacts,
            Color::Red,
            1,
            &[Icon::Authority],
            vec![
                effect(EffectKind::Demand, vec![build::draw(1)]),
                effect(EffectKind::Compel, vec![build::draw(1)]),
                non_demand(vec![build::draw(1)]),
            ],
        ),
        A,
    );
    icons(
        &mut f,
        Color::Blue,
        Icon::Authority,
        3,
        Zone::Board(B, Color::Blue),
    );
    for _ in 0..3 {
        blank(&mut f, Color::Green, 1, base_deck(1));
    }
    (f, source)
}

/// Self-execution skips demands and compels; super-execution runs demands against the
/// opponent but still skips compels.
#[test]
fn self_and_super_execution_skip_compels() {
    for (mode, expected) in [
        (ExecutionMode::SelfExecute, vec![A]),
        (ExecutionMode::SuperExecute, vec![B, A]),
    ] {
        let (f, source) = three_kinds();
        let mut game = new_game(f, Expansions::ARTIFACTS);
        match mode {
            ExecutionMode::SuperExecute => game.super_execute(source, Context::new(A)),
            _ => game.self_execute(source, Context::new(A)),
        }
        .unwrap();
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert_eq!(drawers(&events), expected, "{mode:?}");
    }
}

/// An endorsed Dogma repeats its compel against the sharers locked at the
/// start, and the owner runs the non-demand twice while B shares it once
/// (Cities p23 endorsement; old
/// `endorsed_compel_repeats_with_locked_victims_and_non_demand_shares_once`).
#[test]
fn an_endorsed_compel_repeats_against_the_sharer() {
    let mut f = Fixture::default();
    let source = on_board(
        &mut f,
        card(
            CardSet::Artifacts,
            Color::Red,
            1,
            &[Icon::Concept],
            vec![
                effect(EffectKind::Compel, vec![build::draw(1)]),
                non_demand(vec![build::draw(1)]),
            ],
        ),
        A,
    );
    // Babylon: a Concept City of value 1, so a 1 in hand pays.
    on_board(&mut f, city_card("C007").unwrap(), A);
    let payment = blank(&mut f, Color::Yellow, 1, Zone::Hand(A));
    // Four Concept: as many as A's source and Babylon.
    icons(
        &mut f,
        Color::Blue,
        Icon::Concept,
        4,
        Zone::Board(B, Color::Blue),
    );
    for _ in 0..6 {
        blank(&mut f, Color::Green, 1, base_deck(1));
    }
    let mut game = new_game(f, artifacts_with(Expansions::CITIES));
    dogma(&mut game, source);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Endorsed { player: A, .. }))
    );
    assert_eq!(game.position().cards(Zone::Junk), [payment]);
    // Compel twice (B), shared non-demand (B), owner twice (A), then A's
    // sharing draw.
    assert_eq!(drawers(&events), [B, B, B, A, A, A]);
}

/// "Demand effect" includes compels (a compel is a demand subtype,
/// ARTIFACTS_RULES.md): Crusader Rabbit takes the two bottom cards under a
/// compel top (old
/// `crusader_rabbit_transfers_two_original_bottoms_including_compel_tops`).
#[test]
fn crusader_rabbit_counts_a_compel_top_as_a_demand_effect() {
    let mut f = Fixture::default();
    let mut rabbit = artifacts_card("A086").unwrap();
    rabbit.effects.truncate(1);
    let rabbit = on_board(&mut f, rabbit, A);
    let bottom = blank(&mut f, Color::Red, 1, Zone::Board(B, Color::Red));
    let middle = blank(&mut f, Color::Red, 2, Zone::Board(B, Color::Red));
    let hunley = on_board(&mut f, artifacts_card("A066").unwrap(), B);
    icons(
        &mut f,
        Color::Blue,
        Icon::Health,
        3,
        Zone::Board(B, Color::Blue),
    );
    let mut game = new_game(f, Expansions::ARTIFACTS);
    dogma(&mut game, rabbit);
    let (status, events) = play(&mut game, |_| Some(0));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let moved = moves(&events);
    assert_eq!(moved.len(), 2, "{moved:?}");
    assert!(
        moved
            .iter()
            .all(|m| m.0 == Some(Verb::Transfer) && m.3 == Zone::Score(A))
    );
    let mut scored = game.position().cards(Zone::Score(A)).to_vec();
    scored.sort();
    assert_eq!(scored, [bottom, middle]);
    assert_eq!(game.position().cards(Zone::Board(B, Color::Red)), [hunley]);
}

/// Treaty of Kadesh's compel returns a compel top, and its non-demand scores
/// the owner's non-blue compel top: both are "a demand effect".
#[test]
fn treaty_of_kadesh_counts_compel_tops_as_demand_effects() {
    // The compel: B returns its compel top, not its plain one.
    let mut f = Fixture::default();
    let mut treaty = artifacts_card("A010").unwrap();
    treaty.effects.truncate(1);
    let treaty = on_board(&mut f, treaty, A);
    let hunley = on_board(&mut f, artifacts_card("A066").unwrap(), B);
    icons(
        &mut f,
        Color::Green,
        Icon::Authority,
        3,
        Zone::Board(B, Color::Green),
    );
    let mut game = new_game(f, Expansions::ARTIFACTS);
    dogma(&mut game, treaty);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Return),
            hunley,
            Zone::Board(B, Color::Red),
            artifacts_deck(7)
        )]
    );

    // The non-demand: A scores its own red compel top.
    let mut f = Fixture::default();
    let mut treaty = artifacts_card("A010").unwrap();
    treaty.effects.remove(0);
    let treaty = on_board(&mut f, treaty, A);
    let hunley = on_board(&mut f, artifacts_card("A066").unwrap(), A);
    let mut game = new_game(f, Expansions::ARTIFACTS);
    dogma(&mut game, treaty);
    let (status, events) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [(
            Some(Verb::Score),
            hunley,
            Zone::Board(A, Color::Red),
            Zone::Score(A)
        )]
    );
}

// ------------------------------------------------------------- Showcase

/// The showcase's free Dogma has no color-pile echo prelude: an echo on the
/// board pile of the displayed Artifact's color does not run
/// (ARTIFACTS_RULES.md; old `showcase_does_not_prepend_same_color_board_echoes`).
#[test]
fn a_showcase_runs_no_echo_from_the_same_colored_pile() {
    let mut f = Fixture::default();
    let shown = printed(
        &mut f,
        card(
            CardSet::Artifacts,
            Color::Green,
            2,
            &[Icon::Concept],
            vec![non_demand(vec![build::draw(1)])],
        ),
        Zone::Display(A),
    );
    let mut echo_top = card(CardSet::Echoes, Color::Green, 3, &[], vec![]);
    echo_top.echo = Some(Box::new(non_demand(vec![build::draw(2)])));
    on_board(&mut f, echo_top, A);
    let one = blank(&mut f, Color::Red, 1, base_deck(1));
    let two = blank(&mut f, Color::Red, 2, base_deck(2));
    let mut game = new_game(f, artifacts_with(Expansions::ECHOES));
    game.begin_turn(A).unwrap();
    let (status, events) = play(&mut game, |decision| {
        decision.options.iter().position(|option| {
            *option
                == Alternative::Showcase {
                    card: shown,
                    auspice: false,
                }
        })
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(
        moves(&events),
        [
            (None, one, base_deck(1), Zone::Hand(A)),
            (Some(Verb::Rotate), shown, Zone::Display(A), Zone::Museum(A)),
        ]
    );
    assert_eq!(game.position().cards(base_deck(2)), [two]);
}

/// The new turn's resets come before its showcase: the turn's endorsement is
/// on offer again, and the showcase's draw is the turn's first draw, from the
/// Unseen deck (old `turn_reset_precedes_showcase_endorsement_and_unseen_first_draw`).
#[test]
fn the_turn_reset_precedes_the_showcase() {
    let mut f = Fixture::default();
    // B's turn: a Draw action, then an endorsed Dogma that A shares.
    let b_source = on_board(
        &mut f,
        card(
            CardSet::Base,
            Color::Blue,
            1,
            &[Icon::Concept],
            vec![non_demand(vec![build::draw(1)])],
        ),
        B,
    );
    on_board(&mut f, city_card("C007").unwrap(), B);
    blank(&mut f, Color::Yellow, 1, Zone::Hand(B));
    // A: seven Concept against B's four, so A shares and B cannot.
    icons(
        &mut f,
        Color::Blue,
        Icon::Concept,
        4,
        Zone::Board(A, Color::Blue),
    );
    on_board(&mut f, city_card("C001").unwrap(), A);
    blank(&mut f, Color::Yellow, 1, Zone::Hand(A));
    let shown = printed(
        &mut f,
        card(
            CardSet::Artifacts,
            Color::Yellow,
            2,
            &[Icon::Concept],
            vec![non_demand(vec![build::draw(1)])],
        ),
        Zone::Display(A),
    );
    let unseen_one = Zone::Deck(Deck {
        set: CardSet::Unseen,
        age: 1,
    });
    for _ in 0..3 {
        f.add(
            card(CardSet::Unseen, Color::Purple, 1, &[], vec![]),
            unseen_one,
        );
    }
    for _ in 0..4 {
        blank(&mut f, Color::Green, 1, base_deck(1));
    }
    let position = Position::new(f.cards, &f.placements, B).unwrap();
    let mut game = Game::new(
        position,
        artifacts_with(Expansions::CITIES.union(Expansions::UNSEEN)),
    )
    .unwrap();
    game.begin_turn(B).unwrap();
    let mut events = Vec::new();
    game.take_action(Action::Draw, ActionKind::Regular).unwrap();
    assert_eq!(
        settle(&mut game, &mut events),
        Status::Complete(ReturnValue::Unit)
    );
    dogma(&mut game, b_source);
    let (status, more) = play(&mut game, |decision| {
        decision
            .options
            .iter()
            .position(|option| {
                *option
                    == Alternative::Showcase {
                        card: shown,
                        auspice: false,
                    }
            })
            .or(Some(0))
    });
    events.extend(more);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().turn_owner(), A);
    let endorsers: Vec<Player> = events
        .iter()
        .filter_map(|event| match event {
            Event::Endorsed { player, .. } => Some(*player),
            _ => None,
        })
        .collect();
    assert_eq!(endorsers, [B, A]);
    // A's draws: sharing B's Dogma (A's first of B's turn), then the endorsed
    // showcase Dogma's two, the first of them A's first of A's turn.
    let a_sources: Vec<Zone> = events
        .iter()
        .filter_map(|event| match event {
            Event::Moved(m) if m.verb.is_none() && m.executor == A => Some(m.source),
            _ => None,
        })
        .collect();
    assert_eq!(a_sources, [unseen_one, unseen_one, base_deck(1)]);
    assert_eq!(game.position().cards(Zone::Museum(A)), [shown]);
}

/// A showcased Martian Internet ends the game in a draw at once: no rotation into a
/// Museum follows.
#[test]
fn a_showcased_martian_internet_ends_the_game_before_rotation() {
    let mut f = Fixture::default();
    let martian = printed(&mut f, artifacts_card("A113").unwrap(), Zone::Display(A));
    blank(&mut f, Color::Red, 5, Zone::Score(A));
    let mut game = new_game(f, Expansions::ARTIFACTS);
    game.begin_turn(A).unwrap();
    let (status, events) = play(&mut game, |decision| {
        decision.options.iter().position(|option| {
            *option
                == Alternative::Showcase {
                    card: martian,
                    auspice: false,
                }
        })
    });
    assert_eq!(status, Status::Draw(DrawReason::Effect));
    assert!(moves(&events).is_empty());
    assert_eq!(game.position().cards(Zone::Display(A)), [martian]);
    assert!(game.position().cards(Zone::Museum(A)).is_empty());
    assert_eq!(game.position().available_museums(), 5);
}

// ---------------------------------------------------------------- Yamato

/// Submarine Hunley returns the melded color only for an even bottom card: a board
/// Yamato is 11, odd.
#[test]
fn hunley_reads_a_bottom_yamato_as_odd_eleven() {
    for yamato_bottom in [true, false] {
        let mut f = Fixture::default();
        let hunley = on_board(&mut f, artifacts_card("A066").unwrap(), A);
        let bottom = if yamato_bottom {
            printed(&mut f, yamato(), Zone::Board(B, Color::Red))
        } else {
            blank(&mut f, Color::Red, 8, Zone::Board(B, Color::Red))
        };
        icons(
            &mut f,
            Color::Blue,
            Icon::Industry,
            3,
            Zone::Board(B, Color::Blue),
        );
        let seven = blank(&mut f, Color::Red, 7, base_deck(7));
        let mut game = new_game(f, Expansions::ARTIFACTS);
        dogma(&mut game, hunley);
        let (status, events) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert_eq!(revealed(&events, bottom), 1);
        let red = game.position().cards(Zone::Board(B, Color::Red));
        if yamato_bottom {
            assert_eq!(red, [bottom, seven]);
        } else {
            assert!(red.is_empty());
        }
    }
}

/// Jumbo Kingdom scores a junked Yamato at its back's 8, fewer than eleven
/// points, so it repeats (old
/// `jumbo_kingdom_scores_hidden_yamato_as_eight_and_therefore_repeats`).
#[test]
fn jumbo_kingdom_scores_a_junked_yamato_as_eight_and_repeats() {
    let mut f = Fixture::default();
    let mut jumbo = artifacts_card("A114").unwrap();
    jumbo.effects.remove(0);
    let jumbo = on_board(&mut f, jumbo, A);
    let yamato = printed(&mut f, yamato(), Zone::Junk);
    let one = blank(&mut f, Color::Red, 1, Zone::Junk);
    let mut game = new_game(f, Expansions::ARTIFACTS);
    dogma(&mut game, jumbo);
    let back = Some(Deck {
        set: CardSet::Artifacts,
        age: 8,
    });
    let mut first = true;
    let (status, events) = play(&mut game, |decision| {
        let pick = first.then(|| {
            decision
                .options
                .iter()
                .position(|option| names(option, yamato, back))
                .expect("Yamato is offered")
        });
        first = false;
        pick
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let scored: Vec<CardId> = moves(&events)
        .iter()
        .filter(|m| m.0 == Some(Verb::Score))
        .map(|m| m.1)
        .collect();
    assert_eq!(scored, [yamato, one]);
    assert_eq!(game.position().value(yamato), 8);
    assert_eq!(game.position().score(A), 9);
    assert!(game.position().cards(Zone::Junk).is_empty());
}

/// Moylough Belt Shrine junks the deck of the transferred card's value: a
/// Yamato transferred to a board is 11 (old
/// `moylough_demander_chooses_from_revealed_hand_and_yamato_junks_eleven`).
#[test]
fn moylough_belt_shrine_junks_the_eleven_deck_for_a_transferred_yamato() {
    let mut f = Fixture::default();
    let shrine = on_board(&mut f, artifacts_card("A028").unwrap(), A);
    icons(
        &mut f,
        Color::Blue,
        Icon::Authority,
        3,
        Zone::Board(B, Color::Blue),
    );
    let yamato = printed(&mut f, yamato(), Zone::Hand(B));
    blank(&mut f, Color::Green, 3, Zone::Hand(B));
    let eleven = blank(&mut f, Color::Green, 11, base_deck(11));
    let eight = blank(&mut f, Color::Green, 8, base_deck(8));
    let mut game = new_game(f, Expansions::ARTIFACTS);
    dogma(&mut game, shrine);
    let (status, _) = play(&mut game, |decision| option_naming(decision, yamato));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().top(A, Color::Red), Some(yamato));
    assert_eq!(game.position().value(yamato), 11);
    assert_eq!(game.position().cards(Zone::Junk), [eleven]);
    assert_eq!(game.position().cards(base_deck(8)), [eight]);
}

/// A board Yamato is 11 for Echoes' "single highest top card": tied with
/// another 11 the ordinary draw stays Base, alone above a 3 it comes from
/// the Echoes deck (old
/// `yamato_highest_top_enables_standard_achievement_and_echo_replacement`).
#[test]
fn a_board_yamato_counts_eleven_for_the_echoes_draw_rule() {
    for (source_value, echoes_draw) in [(11, false), (3, true)] {
        let mut f = Fixture::default();
        printed(&mut f, yamato(), Zone::Board(A, Color::Red));
        let source = on_board(
            &mut f,
            card(
                CardSet::Base,
                Color::Blue,
                source_value,
                &[Icon::Authority],
                vec![non_demand(vec![build::draw(1)])],
            ),
            A,
        );
        let base = blank(&mut f, Color::Green, 1, base_deck(1));
        let echo = f.add(
            card(CardSet::Echoes, Color::Green, 1, &[], vec![]),
            Zone::Deck(Deck {
                set: CardSet::Echoes,
                age: 1,
            }),
        );
        let mut game = new_game(f, artifacts_with(Expansions::ECHOES));
        dogma(&mut game, source);
        let (status, _) = no_decisions(&mut game);
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let drawn = if echoes_draw { echo } else { base };
        assert_eq!(
            game.position().cards(Zone::Hand(A)),
            [drawn],
            "{source_value}"
        );
    }
}

/// Blackmail (A's) on B: B holds Yamato and the card A will have B self-execute, in
/// hand or score pile, and a 1 in hand to meld. Blackmail's reveal shows Yamato's face,
/// 11, through that self-executed child.
fn blackmail(executed: Card, in_score: bool) -> (Fixture, [CardId; 4]) {
    let mut f = Fixture::default();
    let blackmail = on_board(&mut f, unseen_card("U040").unwrap(), A);
    let pile = if in_score {
        Zone::Score(B)
    } else {
        Zone::Hand(B)
    };
    let yamato = printed(&mut f, yamato(), pile);
    let executed = printed(&mut f, executed, pile);
    let melded = blank(&mut f, Color::Red, 1, Zone::Hand(B));
    (f, [blackmail, yamato, executed, melded])
}

/// A melds the plain 1 from B's hand and has B execute the chosen card; B
/// then picks Yamato, or the value 11. A lone option is taken unasked.
fn blackmail_choices(
    yamato: CardId,
    executed: CardId,
    melded: CardId,
) -> impl FnMut(&Decision) -> Option<usize> {
    move |decision| {
        let meld = decision.options.iter().position(|option| {
            matches!(
                option,
                Alternative::Move { target: TargetDescriptor::Known { card, source: Zone::Hand(_) }, .. }
                | Alternative::Select { target: TargetDescriptor::Known { card, source: Zone::Hand(_) } }
                    if *card == melded
            )
        });
        let execute = (decision.player == A)
            .then(|| option_naming(decision, executed))
            .flatten();
        meld.or(execute)
            .or_else(|| option_naming(decision, yamato))
            .or_else(|| {
                decision
                    .options
                    .iter()
                    .position(|option| *option == Alternative::Value(11))
            })
    }
}

/// Holy Grail returns the revealed Yamato and claims an achievement of its
/// value, 11, not its back's 8 (old
/// `holy_grail_uses_inherited_hand_reveal_value_for_claim`).
#[test]
fn holy_grail_under_blackmail_claims_at_a_revealed_yamatos_eleven() {
    let (mut f, [blackmail, yamato, grail, melded]) =
        blackmail(artifacts_card("A018").unwrap(), false);
    let eight = blank(&mut f, Color::Green, 8, Zone::AvailableAchievements);
    let eleven = blank(&mut f, Color::Green, 11, Zone::AvailableAchievements);
    let mut game = new_game(f, artifacts_with(Expansions::UNSEEN));
    dogma(&mut game, blackmail);
    let (status, events) = play(&mut game, blackmail_choices(yamato, grail, melded));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(
        moves(&events)
            .iter()
            .any(|m| m.0 == Some(Verb::Return) && m.1 == yamato)
    );
    assert_eq!(game.position().cards(Zone::Achievements(B)), [eleven]);
    assert_eq!(game.position().cards(Zone::AvailableAchievements), [eight]);
    assert_eq!(game.position().value(yamato), 8);
}

/// Sliced Bread returns the revealed scored Yamato and draws and scores two
/// cards one below 11 (old `sliced_bread_snapshots_revealed_score_value_before_return`).
#[test]
fn sliced_bread_under_blackmail_draws_below_a_revealed_yamatos_eleven() {
    let expansions = Expansions::UNSEEN.union(Expansions::ECHOES);
    let (mut f, [blackmail, yamato, bread, melded]) = blackmail(echoes_card("E081").unwrap(), true);
    for _ in 0..2 {
        blank(&mut f, Color::Yellow, 8, base_deck(8));
        blank(&mut f, Color::Yellow, 7, base_deck(7));
        blank(&mut f, Color::Yellow, 10, base_deck(10));
    }
    let mut game = new_game(f, artifacts_with(expansions));
    dogma(&mut game, blackmail);
    let (status, _) = play(&mut game, blackmail_choices(yamato, bread, melded));
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    let position = game.position();
    let scored: Vec<u8> = position
        .cards(Zone::Score(B))
        .iter()
        .filter(|&&card| card != bread)
        .map(|&card| position.value(card))
        .collect();
    assert_eq!(scored, [10, 10]);
    assert!(position.cards(artifacts_deck(8)).contains(&yamato));
}

/// Joy Buzzer's value choice sees the revealed hand Yamato as 11 and scores
/// it alone (old `joy_buzzer_scores_revealed_yamato_as_eleven`).
#[test]
fn joy_buzzer_under_blackmail_scores_a_revealed_yamato_as_eleven() {
    let (f, [blackmail, yamato, buzzer, melded]) = blackmail(unseen_card("U084").unwrap(), false);
    let mut game = new_game(f, artifacts_with(Expansions::UNSEEN));
    dogma(&mut game, blackmail);
    let mut offered = Vec::new();
    let mut choose = blackmail_choices(yamato, buzzer, melded);
    let (status, _) = play(&mut game, |decision| {
        offered.extend(decision.options.iter().filter_map(|option| match option {
            Alternative::Value(value) => Some(*value),
            _ => None,
        }));
        choose(decision)
    });
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert!(offered.contains(&11), "{offered:?}");
    assert_eq!(game.position().cards(Zone::Score(B)), [yamato]);
    assert_eq!(game.position().cards(Zone::Hand(B)), [buzzer]);
}

// ------------------------------------------------------- Further cards

/// Garland's Ruby Slippers wins on melding an effectless card: every age-8
/// City (old `ruby_slippers_wins_with_every_age_eight_city`).
#[test]
fn ruby_slippers_wins_on_melding_an_age_eight_city() {
    let cities: Vec<Card> = (1..=150)
        .filter_map(|n| city_card(&format!("C{n:03}")).ok())
        .filter(|city| city.value == 8)
        .collect();
    assert!(!cities.is_empty());
    for city in cities {
        let code = city.code;
        let mut f = Fixture::default();
        let slippers = on_board(&mut f, artifacts_card("A084").unwrap(), A);
        let city = printed(&mut f, city, Zone::Hand(A));
        let mut game = new_game(f, artifacts_with(Expansions::CITIES));
        dogma(&mut game, slippers);
        let (status, _) = no_decisions(&mut game);
        assert_eq!(status, Status::Winner(A), "{code}");
        assert!(game.position().cards(Zone::Hand(A)).is_empty(), "{code}");
        let color = game.position().card(city).color;
        assert_eq!(game.position().top(A, color), Some(city), "{code}");
    }
}

/// The Big Bang counts a reveal as a change: a blue top that only reveals the hand
/// repeats until the [10] draws run out; with nothing to reveal it stops at once (old
/// `big_bang_stops_after_no_change_but_reveal_only_execution_repeats`).
#[test]
fn the_big_bang_repeats_on_a_reveal_alone() {
    for hand in [true, false] {
        let mut f = Fixture::default();
        let bang = on_board(&mut f, artifacts_card("A094").unwrap(), A);
        on_board(
            &mut f,
            card(
                CardSet::Base,
                Color::Blue,
                1,
                &[],
                vec![non_demand(vec![build::reveal_all(build::HAND)])],
            ),
            A,
        );
        let held = hand.then(|| blank(&mut f, Color::Red, 1, Zone::Hand(A)));
        let ten = blank(&mut f, Color::Green, 10, base_deck(10));
        let mut game = new_game(f, Expansions::ARTIFACTS);
        dogma(&mut game, bang);
        let (status, events) = no_decisions(&mut game);
        if let Some(held) = held {
            // Reveal, draw and junk the 10, reveal again, then the next [10]
            // draw finds every deck from 10 up empty.
            assert!(matches!(
                status,
                Status::Winner(_) | Status::Draw(DrawReason::Exhaustion)
            ));
            assert_eq!(revealed(&events, held), 2);
            assert_eq!(game.position().cards(Zone::Junk), [ten]);
        } else {
            assert_eq!(status, Status::Complete(ReturnValue::Unit));
            assert_eq!(game.position().cards(base_deck(10)), [ten]);
        }
    }
}

/// "You win" wins for whoever executes it: a sharing opponent executes
/// Where's Waldo first and wins (old
/// `waldo_wins_for_executing_opponent_without_turn_ownership`).
#[test]
fn wheres_waldo_wins_for_a_sharer() {
    let mut f = Fixture::default();
    let waldo = on_board(&mut f, artifacts_card("A099").unwrap(), A);
    icons(
        &mut f,
        Color::Blue,
        Icon::Health,
        3,
        Zone::Board(B, Color::Blue),
    );
    let mut game = new_game(f, Expansions::ARTIFACTS);
    dogma(&mut game, waldo);
    assert_eq!(no_decisions(&mut game).0, Status::Winner(B));
}

/// Dolly the Sheep junks the available standard and special achievements, not the
/// Museums.
#[test]
fn dolly_the_sheep_keeps_the_available_museums() {
    let mut f = Fixture::default();
    let mut dolly = artifacts_card("A098").unwrap();
    dolly.effects.remove(0);
    let dolly = on_board(&mut f, dolly, A);
    let standard = blank(&mut f, Color::Green, 3, Zone::AvailableAchievements);
    let mut game = new_game(f, Expansions::ARTIFACTS);
    assert_eq!(game.position().available_museums(), 5);
    let specials: Vec<Special> = game.position().specials().collect();
    assert!(!specials.is_empty());
    dogma(&mut game, dolly);
    let (status, _) = no_decisions(&mut game);
    assert_eq!(status, Status::Complete(ReturnValue::Unit));
    assert_eq!(game.position().cards(Zone::Junk), [standard]);
    for special in specials {
        assert_eq!(
            game.position().special_location(special),
            SpecialLocation::Junk
        );
    }
    assert_eq!(game.position().available_museums(), 5);
}

/// World Cup Final Ball's compel returns a claimed standard achievement only:
/// a Museum B claimed in a Museum Check and a special B owns stay (old
/// `world_cup_compel_returns_standard_but_preserves_special_and_museum`).
#[test]
fn world_cup_final_ball_returns_neither_a_claimed_museum_nor_a_special() {
    for standard in [false, true] {
        let mut f = Fixture::default();
        let mut ball = artifacts_card("A085").unwrap();
        ball.effects.truncate(1);
        let ball = on_board(&mut f, ball, A);
        icons(
            &mut f,
            Color::Blue,
            Icon::Health,
            3,
            Zone::Board(B, Color::Blue),
        );
        // Four stored Artifacts (B three, A one) and A's displayed one fill
        // the last Museum at A's showcase: B holds the most and claims one.
        for value in 1..=3 {
            artifact(&mut f, Color::Green, value, Zone::Museum(B));
        }
        artifact(&mut f, Color::Green, 4, Zone::Museum(A));
        artifact(&mut f, Color::Yellow, 5, Zone::Display(A));
        let claimed = standard.then(|| blank(&mut f, Color::Red, 2, Zone::Achievements(B)));
        let mut game = new_game(f, Expansions::ARTIFACTS);
        let mut events = Vec::new();
        game.start(
            Operation::ClaimSpecial {
                special: Special::Monument,
                check_eligibility: false,
            },
            Context::new(B),
        )
        .unwrap();
        settle(&mut game, &mut events);
        game.begin_turn(A).unwrap();
        let (status, _) = play(&mut game, |_| Some(0));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        assert_eq!(game.position().claimed_museums(B), 1);
        dogma(&mut game, ball);
        let (status, events) = play(&mut game, |_| Some(0));
        assert_eq!(status, Status::Complete(ReturnValue::Unit));
        let returned: Vec<CardId> = moves(&events)
            .iter()
            .filter(|m| m.0 == Some(Verb::Return))
            .map(|m| m.1)
            .collect();
        assert_eq!(returned, claimed.into_iter().collect::<Vec<_>>());
        assert!(game.position().cards(Zone::Achievements(B)).is_empty());
        assert_eq!(game.position().claimed_museums(B), 1);
        assert_eq!(
            game.position().special_location(Special::Monument),
            SpecialLocation::Owned(B)
        );
    }
}
